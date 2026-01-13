//! This module contains structures and messages for synchronization of blocks between peers.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    num::{NonZeroU32, NonZeroU64},
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_config::parameters::actual::{BlockSync as Config, ConsensusMode};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
    prelude::*,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::prelude::*;
use iroha_macro::*;
use norito::codec::{Decode, Encode};
use rand::seq::SliceRandom;
use tokio::sync::mpsc;

use crate::{
    IrohaNetwork, NetworkMessage,
    kura::Kura,
    state::{State, StateReadOnly, StateView, WorldReadOnly},
    sumeragi::{
        SumeragiHandle,
        consensus::{
            NPOS_TAG, PERMISSIONED_TAG, Phase, QcAggregate, ValidatorIndex, qc_signer_count,
        },
        network_topology::Topology,
        stake_snapshot::{CommitStakeSnapshot, stake_quorum_reached_for_snapshot},
        status,
    },
    telemetry::Telemetry,
};

const BLOCK_SYNC_QUEUE_CAP_MULTIPLIER: usize = 4;
const BLOCK_SYNC_QUEUE_CAP_FLOOR: usize = 4;
const BLOCK_SYNC_REQUEST_MAX_PENDING: u8 = 2;
const BLOCK_SYNC_REQUEST_TTL_FLOOR_MS: u64 = 1_000;

fn block_sync_channel_cap(gossip_size: NonZeroU32) -> usize {
    let base = usize::try_from(gossip_size.get()).unwrap_or(1);
    base.saturating_mul(BLOCK_SYNC_QUEUE_CAP_MULTIPLIER)
        .max(BLOCK_SYNC_QUEUE_CAP_FLOOR)
}

fn block_sync_request_ttl(gossip_period: Duration, gossip_max_period: Duration) -> Duration {
    let ttl = gossip_max_period.saturating_add(gossip_period);
    ttl.max(Duration::from_millis(BLOCK_SYNC_REQUEST_TTL_FLOOR_MS))
}

#[derive(Debug, Clone)]
struct BlockSyncRequestState {
    last_request: Instant,
    pending: u8,
}

#[derive(Debug)]
struct BlockSyncRequestTracker {
    pending: BTreeMap<PeerId, BlockSyncRequestState>,
    ttl: Duration,
}

impl BlockSyncRequestTracker {
    fn new(ttl: Duration) -> Self {
        Self {
            pending: BTreeMap::new(),
            ttl,
        }
    }

    fn record_request(&mut self, peer_id: PeerId, now: Instant) {
        self.prune_expired(now);
        let entry = self
            .pending
            .entry(peer_id)
            .or_insert(BlockSyncRequestState {
                last_request: now,
                pending: 0,
            });
        entry.last_request = now;
        entry.pending = entry
            .pending
            .saturating_add(1)
            .min(BLOCK_SYNC_REQUEST_MAX_PENDING);
    }

    fn allow_response(&mut self, peer_id: &PeerId, now: Instant) -> bool {
        self.prune_expired(now);
        let Some(entry) = self.pending.get_mut(peer_id) else {
            return false;
        };
        if now.saturating_duration_since(entry.last_request) > self.ttl || entry.pending == 0 {
            self.pending.remove(peer_id);
            return false;
        }
        entry.pending = entry.pending.saturating_sub(1);
        if entry.pending == 0 {
            self.pending.remove(peer_id);
        }
        true
    }

    fn prune_expired(&mut self, now: Instant) {
        self.pending
            .retain(|_, entry| now.saturating_duration_since(entry.last_request) <= self.ttl);
    }
}

fn should_share_unknown_prev_hash(
    cache: &mut BTreeMap<PeerId, (HashOf<BlockHeader>, u64)>,
    peer_id: &PeerId,
    prev_hash: HashOf<BlockHeader>,
    height: u64,
) -> bool {
    match cache.get(peer_id) {
        Some((last_hash, last_height)) if *last_hash == prev_hash && *last_height == height => {
            false
        }
        _ => {
            cache.insert(peer_id.clone(), (prev_hash, height));
            true
        }
    }
}

/// [`BlockSynchronizer`] actor handle.
#[derive(Clone)]
pub struct BlockSynchronizerHandle {
    message_sender: mpsc::Sender<message::Message>,
}

impl BlockSynchronizerHandle {
    /// Send [`message::Message`] to [`BlockSynchronizer`] actor.
    ///
    /// Messages are best-effort: if the queue is full, the message is dropped
    /// to avoid blocking consensus traffic.
    pub fn message(&self, message: message::Message) {
        let kind = match &message {
            message::Message::GetBlocksAfter(_) => "GetBlocksAfter",
            message::Message::ShareBlocks(_) => "ShareBlocks",
        };
        match self.message_sender.try_send(message) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                iroha_logger::debug!(kind, "block sync queue full; dropping block sync message");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                iroha_logger::warn!(
                    kind,
                    "block sync channel disconnected; dropping block sync message"
                );
            }
        }
    }
}

#[cfg(test)]
mod handle_tests {
    use std::collections::BTreeSet;

    use iroha_crypto::KeyPair;
    use iroha_data_model::peer::PeerId;
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn message_drops_when_queue_full() {
        let (tx, mut rx) = mpsc::channel(1);
        let handle = BlockSynchronizerHandle {
            message_sender: tx.clone(),
        };

        let peer_id_one = PeerId::new(KeyPair::random().public_key().clone());
        let peer_id_two = PeerId::new(KeyPair::random().public_key().clone());
        let msg1 = message::Message::GetBlocksAfter(message::GetBlocksAfter::new(
            peer_id_one.clone(),
            None,
            None,
            BTreeSet::new(),
        ));
        let msg2 = message::Message::GetBlocksAfter(message::GetBlocksAfter::new(
            peer_id_two.clone(),
            None,
            None,
            BTreeSet::new(),
        ));

        tx.try_send(msg1).expect("queue has space");
        handle.message(msg2);

        let received = rx.try_recv().expect("expected queued message");
        match received {
            message::Message::GetBlocksAfter(message::GetBlocksAfter { peer_id: got, .. }) => {
                assert_eq!(got, peer_id_one);
            }
            other => panic!("unexpected message queued: {other:?}"),
        }
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
}

#[cfg(test)]
mod seen_blocks_tests {
    use std::{collections::BTreeSet, num::NonZeroU64};

    use iroha_crypto::{Hash, HashOf};

    use super::*;

    fn entry(height: u64, byte: u8) -> (NonZeroU64, HashOf<BlockHeader>) {
        let height = NonZeroU64::new(height).expect("height must be non-zero");
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]));
        (height, hash)
    }

    #[test]
    fn prune_seen_blocks_keeps_entries_on_stable_height() {
        let mut seen = BTreeSet::from([entry(5, 0x11), entry(6, 0x22)]);
        BlockSynchronizer::prune_seen_blocks(&mut seen, 5, 5);
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn prune_seen_blocks_clears_on_height_regression() {
        let mut seen = BTreeSet::from([entry(5, 0x11)]);
        BlockSynchronizer::prune_seen_blocks(&mut seen, 4, 5);
        assert!(seen.is_empty());
    }

    #[test]
    fn prune_seen_blocks_drops_below_current_height() {
        let mut seen = BTreeSet::from([entry(3, 0x11), entry(6, 0x22)]);
        BlockSynchronizer::prune_seen_blocks(&mut seen, 5, 4);
        let heights: Vec<_> = seen.iter().map(|(height, _)| height.get()).collect();
        assert_eq!(heights, vec![6]);
    }
}

#[cfg(test)]
mod queue_cap_tests {
    use std::num::NonZeroU32;

    use super::*;

    #[test]
    fn block_sync_channel_cap_scales_with_gossip_size() {
        let cap_small = block_sync_channel_cap(NonZeroU32::new(1).expect("non-zero"));
        assert_eq!(cap_small, BLOCK_SYNC_QUEUE_CAP_FLOOR);

        let cap_medium = block_sync_channel_cap(NonZeroU32::new(4).expect("non-zero"));
        assert_eq!(cap_medium, 4 * BLOCK_SYNC_QUEUE_CAP_MULTIPLIER);

        let cap_large = block_sync_channel_cap(NonZeroU32::new(32).expect("non-zero"));
        assert_eq!(cap_large, 32 * BLOCK_SYNC_QUEUE_CAP_MULTIPLIER);
    }
}

#[cfg(test)]
mod request_tracker_tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::peer::PeerId;

    use super::*;

    #[test]
    fn block_sync_request_tracker_allows_single_response() {
        let mut tracker = BlockSyncRequestTracker::new(Duration::from_secs(5));
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let now = Instant::now();

        tracker.record_request(peer_id.clone(), now);
        assert!(tracker.allow_response(&peer_id, now));
        assert!(!tracker.allow_response(&peer_id, now));
    }

    #[test]
    fn block_sync_request_tracker_caps_pending_responses() {
        let mut tracker = BlockSyncRequestTracker::new(Duration::from_secs(5));
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let now = Instant::now();

        tracker.record_request(peer_id.clone(), now);
        tracker.record_request(peer_id.clone(), now + Duration::from_millis(10));
        assert!(tracker.allow_response(&peer_id, now + Duration::from_millis(20)));
        assert!(tracker.allow_response(&peer_id, now + Duration::from_millis(20)));
        assert!(!tracker.allow_response(&peer_id, now + Duration::from_millis(20)));
    }

    #[test]
    fn block_sync_request_tracker_expires_requests() {
        let mut tracker = BlockSyncRequestTracker::new(Duration::from_millis(50));
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let now = Instant::now();

        tracker.record_request(peer_id.clone(), now);
        let later = now + Duration::from_millis(75);
        assert!(!tracker.allow_response(&peer_id, later));
    }
}

#[cfg(test)]
mod gossip_backoff_tests {
    use std::{collections::BTreeSet, num::NonZeroU32, sync::Arc, time::Instant};

    use iroha_crypto::KeyPair;
    use iroha_data_model::peer::{Peer, PeerId};

    use super::*;
    use crate::prelude::World;
    use crate::query::store::LiveQueryStore;
    use crate::sumeragi::test_sumeragi_handle;

    fn dummy_block_sync() -> BlockSynchronizer {
        let (sumeragi, _rx) = test_sumeragi_handle(0);
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        ));
        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let peer = Peer::new(
            "127.0.0.1:0".parse().expect("valid socket address"),
            peer_id,
        );
        BlockSynchronizer {
            sumeragi,
            kura,
            peer,
            gossip_period: Duration::from_secs(1),
            gossip_max_period: Duration::from_secs(8),
            gossip_size: NonZeroU32::new(1).expect("non-zero gossip size"),
            gossip_backoff: Duration::from_secs(1),
            gossip_next_deadline: Instant::now(),
            network: crate::IrohaNetwork::closed_for_tests(),
            relay_ttl: 1,
            block_sync_frame_cap: 1024,
            state,
            telemetry: None,
            seen_blocks: BTreeSet::new(),
            unknown_prev_hashes: BTreeMap::new(),
            request_tracker: BlockSyncRequestTracker::new(block_sync_request_ttl(
                Duration::from_secs(1),
                Duration::from_secs(8),
            )),
            latest_height: 0,
            last_peers: BTreeSet::new(),
            last_drop_count: 0,
            last_drop_at: None,
            fallback_consensus_mode: ConsensusMode::Permissioned,
        }
    }

    #[test]
    fn block_sync_backoff_doubles_until_max() {
        let min = Duration::from_secs(1);
        let max = Duration::from_secs(8);
        let backoff = BlockSynchronizer::next_block_sync_backoff(min, min, max, false);
        assert_eq!(backoff, Duration::from_secs(2));
        let backoff = BlockSynchronizer::next_block_sync_backoff(backoff, min, max, false);
        assert_eq!(backoff, Duration::from_secs(4));
        let backoff = BlockSynchronizer::next_block_sync_backoff(backoff, min, max, false);
        assert_eq!(backoff, Duration::from_secs(8));
        let reset = BlockSynchronizer::next_block_sync_backoff(backoff, min, max, true);
        assert_eq!(reset, min);
    }

    #[test]
    fn peer_set_changed_detects_updates() {
        let mut sync = dummy_block_sync();
        assert!(!sync.peer_set_changed(&[]));
        let peer = PeerId::new(KeyPair::random().public_key().clone());
        assert!(sync.peer_set_changed(&[peer.clone()]));
        assert!(!sync.peer_set_changed(&[peer]));
    }
}

#[cfg(test)]
mod unknown_prev_hash_tests {
    use std::collections::BTreeMap;

    use iroha_crypto::{Hash, HashOf, KeyPair};

    use super::*;

    fn hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
    }

    #[test]
    fn should_share_unknown_prev_hash_tracks_peer_and_height() {
        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let other_peer = PeerId::new(KeyPair::random().public_key().clone());
        let hash1 = hash(0x11);
        let hash2 = hash(0x22);
        let mut cache: BTreeMap<PeerId, (HashOf<BlockHeader>, u64)> = BTreeMap::new();

        assert!(should_share_unknown_prev_hash(&mut cache, &peer, hash1, 10));
        assert!(!should_share_unknown_prev_hash(
            &mut cache, &peer, hash1, 10
        ));
        assert!(should_share_unknown_prev_hash(&mut cache, &peer, hash1, 11));
        assert!(should_share_unknown_prev_hash(&mut cache, &peer, hash2, 11));
        assert!(should_share_unknown_prev_hash(
            &mut cache,
            &other_peer,
            hash1,
            11
        ));
    }
}

/// Structure responsible for block synchronization between peers.
pub struct BlockSynchronizer {
    sumeragi: SumeragiHandle,
    kura: Arc<Kura>,
    peer: Peer,
    gossip_period: Duration,
    gossip_max_period: Duration,
    gossip_size: NonZeroU32,
    gossip_backoff: Duration,
    gossip_next_deadline: std::time::Instant,
    network: IrohaNetwork,
    relay_ttl: u8,
    block_sync_frame_cap: usize,
    state: Arc<State>,
    telemetry: Option<Telemetry>,
    seen_blocks: BTreeSet<(NonZeroU64, HashOf<BlockHeader>)>,
    unknown_prev_hashes: BTreeMap<PeerId, (HashOf<BlockHeader>, u64)>,
    request_tracker: BlockSyncRequestTracker,
    latest_height: u64,
    last_peers: BTreeSet<PeerId>,
    last_drop_count: u64,
    last_drop_at: Option<std::time::Instant>,
    fallback_consensus_mode: ConsensusMode,
}

pub use message::RosterMetadata;

impl BlockSynchronizer {
    /// Start [`Self`] actor.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> (BlockSynchronizerHandle, Child) {
        let cap = block_sync_channel_cap(self.gossip_size);
        let (message_sender, message_receiver) = mpsc::channel(cap);
        (
            BlockSynchronizerHandle { message_sender },
            Child::new(
                tokio::spawn(self.run(message_receiver, shutdown_signal)),
                OnShutdown::Abort,
            ),
        )
    }

    /// [`Self`] task.
    async fn run(
        mut self,
        mut message_receiver: mpsc::Receiver<message::Message>,
        shutdown_signal: ShutdownSignal,
    ) {
        let mut gossip_period = tokio::time::interval(self.gossip_period);
        loop {
            tokio::select! {
                _ = gossip_period.tick() => self.request_block().await,
                Some(msg) = message_receiver.recv() => {
                    msg.handle_message(&mut self).await;
                }
                () = shutdown_signal.receive() => {
                    debug!("Shutting down block sync");
                    break;
                },
            }
            tokio::task::yield_now().await;
        }
    }

    fn prune_seen_blocks(
        seen_blocks: &mut BTreeSet<(NonZeroU64, HashOf<BlockHeader>)>,
        now_height: u64,
        previous_height: u64,
    ) {
        if now_height < previous_height {
            // Height regression implies a rollback; drop seen blocks to allow refetch.
            seen_blocks.clear();
        }
        seen_blocks.retain(|(height, _hash)| height.get() >= now_height);
    }

    fn next_block_sync_backoff(
        current: Duration,
        min: Duration,
        max: Duration,
        progress: bool,
    ) -> Duration {
        if progress {
            return min;
        }
        current.saturating_mul(2).min(max)
    }

    fn peer_set_changed(&mut self, peers: &[PeerId]) -> bool {
        let mut set = BTreeSet::new();
        set.extend(peers.iter().cloned());
        if set == self.last_peers {
            return false;
        }
        self.last_peers = set;
        true
    }

    fn block_sync_backpressure_active(&mut self, now: std::time::Instant) -> bool {
        let current = iroha_p2p::network::subscriber_queue_full_count();
        if current > self.last_drop_count {
            self.last_drop_count = current;
            self.last_drop_at = Some(now);
        }
        self.last_drop_at
            .is_some_and(|last| now.saturating_duration_since(last) < self.gossip_backoff)
    }

    /// Sends requests for the latest blocks to a subset of online peers
    async fn request_block(&mut self) {
        let now = std::time::Instant::now();
        let now_height = match u64::try_from(self.state.view().height()) {
            Ok(height) => height,
            Err(_) => {
                warn!("block sync: state height exceeds u64::MAX; saturating");
                u64::MAX
            }
        };

        let previous_height = self.latest_height;
        Self::prune_seen_blocks(&mut self.seen_blocks, now_height, previous_height);
        if now_height < previous_height {
            self.unknown_prev_hashes.clear();
        }
        self.latest_height = now_height;

        let peers = self
            .network
            .online_peers(|set| set.iter().map(|peer| peer.id().clone()).collect::<Vec<_>>());
        let peers_changed = self.peer_set_changed(&peers);
        let has_unknown_prev = !self.unknown_prev_hashes.is_empty();
        let height_changed = now_height != previous_height;
        let progress = height_changed || peers_changed || has_unknown_prev;
        if progress {
            self.gossip_backoff = self.gossip_period;
            let next = now.checked_add(self.gossip_period).unwrap_or(now);
            if next < self.gossip_next_deadline {
                self.gossip_next_deadline = next;
            }
        }
        if now < self.gossip_next_deadline {
            return;
        }
        if self.block_sync_backpressure_active(now) {
            self.gossip_next_deadline = now.checked_add(self.gossip_backoff).unwrap_or(now);
            iroha_logger::trace!(
                drops = self.last_drop_count,
                cooldown_ms = self.gossip_backoff.as_millis(),
                "block sync skipping request due to relay backpressure"
            );
            return;
        }
        if peers.is_empty() {
            self.gossip_backoff = Self::next_block_sync_backoff(
                self.gossip_backoff,
                self.gossip_period,
                self.gossip_max_period,
                progress,
            );
            self.gossip_next_deadline = now.checked_add(self.gossip_backoff).unwrap_or(now);
            return;
        }

        let (targets, stray_targets, gossip_size, world_known) = {
            let world_peers: BTreeSet<_> =
                self.state.view().world.peers().iter().cloned().collect();
            let mut rng = rand::rng();
            let gossip_size = usize::try_from(self.gossip_size.get()).unwrap_or(usize::MAX);
            let (targets, stray_targets) =
                select_block_sync_targets(&peers, &world_peers, gossip_size, &mut rng);
            (targets, stray_targets, gossip_size, world_peers.len())
        };
        if targets.is_empty() {
            iroha_logger::debug!(
                height = now_height,
                online = peers.len(),
                gossip_limit = gossip_size,
                "block sync: no peers sampled for gossip"
            );
        } else {
            iroha_logger::info!(
                height = now_height,
                online = peers.len(),
                seen = self.seen_blocks.len(),
                gossip_limit = gossip_size,
                target_count = targets.len(),
                world_known,
                stray_target_count = stray_targets.len(),
                stray_targets = ?stray_targets,
                targets = ?targets,
                "block sync sampling peers for latest blocks"
            );
        }
        for peer_id in targets {
            self.request_latest_blocks_from_peer(peer_id).await;
        }
        self.gossip_backoff = Self::next_block_sync_backoff(
            self.gossip_backoff,
            self.gossip_period,
            self.gossip_max_period,
            progress,
        );
        self.gossip_next_deadline = now.checked_add(self.gossip_backoff).unwrap_or(now);
    }

    /// Sends request for latest blocks to a chosen peer
    async fn request_latest_blocks_from_peer(&mut self, peer_id: PeerId) {
        let (prev_hash, latest_hash) = {
            let state_view = self.state.view();
            (state_view.prev_block_hash(), state_view.latest_block_hash())
        };
        self.request_tracker
            .record_request(peer_id.clone(), Instant::now());
        message::Message::GetBlocksAfter(message::GetBlocksAfter::new(
            self.peer.id().clone(),
            prev_hash,
            latest_hash,
            self.seen_blocks
                .iter()
                .map(|(_height, hash)| *hash)
                .collect(),
        ))
        .send_to(&self.network, peer_id)
        .await;
    }
    /// Create [`Self`] from [`Config`]
    #[allow(clippy::too_many_arguments)]
    pub fn from_config(
        config: &Config,
        sumeragi: SumeragiHandle,
        kura: Arc<Kura>,
        peer: Peer,
        network: IrohaNetwork,
        state: Arc<State>,
        telemetry: Option<Telemetry>,
        fallback_consensus_mode: ConsensusMode,
        relay_ttl: u8,
        block_sync_frame_cap: usize,
    ) -> Self {
        let gossip_period = config.gossip_period;
        let gossip_max_period = config.gossip_max_period.max(gossip_period);
        let now = std::time::Instant::now();
        Self {
            peer,
            sumeragi,
            kura,
            gossip_period,
            gossip_max_period,
            gossip_size: config.gossip_size,
            gossip_backoff: gossip_period,
            gossip_next_deadline: now,
            network,
            relay_ttl,
            block_sync_frame_cap,
            state,
            telemetry,
            seen_blocks: BTreeSet::new(),
            unknown_prev_hashes: BTreeMap::new(),
            request_tracker: BlockSyncRequestTracker::new(block_sync_request_ttl(
                gossip_period,
                gossip_max_period,
            )),
            latest_height: 0,
            last_peers: BTreeSet::new(),
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            fallback_consensus_mode,
        }
    }

    /// Build a bitmap from validator indices sized for `roster_len`.
    fn build_signers_bitmap(signers: &BTreeSet<ValidatorIndex>, roster_len: usize) -> Vec<u8> {
        if roster_len == 0 {
            return Vec::new();
        }
        let mut bitmap = vec![0u8; roster_len.div_ceil(8)];
        for signer in signers {
            let Ok(idx) = usize::try_from(*signer) else {
                continue;
            };
            if idx >= roster_len {
                continue;
            }
            let byte = idx / 8;
            let bit = idx % 8;
            bitmap[byte] |= 1u8 << bit;
        }
        bitmap
    }

    /// Compute the minimum votes required for a commit for a roster of length `len`.
    const fn min_votes_for_len(len: usize) -> usize {
        if len > 3 {
            ((len.saturating_sub(1)) / 3) * 2 + 1
        } else {
            len
        }
    }

    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn qc_from_signers(
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
        mode_tag: &str,
        validator_set: Vec<PeerId>,
        block_hash: HashOf<BlockHeader>,
        parent_state_root: Hash,
        post_state_root: Hash,
        height: u64,
        view: u64,
        epoch: u64,
        signers: BTreeSet<ValidatorIndex>,
        aggregate_signature: Vec<u8>,
    ) -> Option<Qc> {
        let roster_len = validator_set.len();
        if roster_len == 0 {
            return None;
        }
        if aggregate_signature.is_empty() {
            return None;
        }
        match consensus_mode {
            ConsensusMode::Permissioned => {
                let required = Self::min_votes_for_len(roster_len);
                if signers.len() < required {
                    return None;
                }
            }
            ConsensusMode::Npos => {
                let snapshot = stake_snapshot?;
                let mut signer_peers = BTreeSet::new();
                for signer in &signers {
                    let Ok(idx) = usize::try_from(*signer) else {
                        return None;
                    };
                    let Some(peer) = validator_set.get(idx) else {
                        return None;
                    };
                    signer_peers.insert(peer.clone());
                }
                match stake_quorum_reached_for_snapshot(snapshot, &validator_set, &signer_peers) {
                    Ok(true) => {}
                    _ => return None,
                }
            }
        }
        let signers_bitmap = Self::build_signers_bitmap(&signers, roster_len);
        Some(Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap,
                bls_aggregate_signature: aggregate_signature,
            },
        })
    }

    pub(crate) fn block_sync_qc_for(
        state_view: &StateView<'_>,
        fallback_consensus_mode: ConsensusMode,
        block: &SignedBlock,
    ) -> Option<Qc> {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let (consensus_mode, expected_mode_tag) = {
            let consensus_mode = crate::sumeragi::effective_consensus_mode_for_height(
                &state_view,
                height,
                fallback_consensus_mode,
            );
            let mode_tag = match consensus_mode {
                ConsensusMode::Permissioned => PERMISSIONED_TAG,
                ConsensusMode::Npos => NPOS_TAG,
            };
            (consensus_mode, mode_tag)
        };

        if let Some(record) = status::precommit_signers_for(block_hash) {
            if record.height != height || record.view != view {
                iroha_logger::info!(
                    height,
                    view,
                    record_height = record.height,
                    record_view = record.view,
                    record_epoch = record.epoch,
                    "block sync: cached precommit signer record does not match block metadata"
                );
                return None;
            }
            if record.mode_tag != expected_mode_tag {
                iroha_logger::info!(
                    height,
                    view,
                    expected_mode = expected_mode_tag,
                    record_mode = %record.mode_tag,
                    "block sync: cached precommit signer record uses unexpected mode tag"
                );
                return None;
            }
            if let Some(qc) = Self::qc_from_signers(
                consensus_mode,
                record.stake_snapshot.as_ref(),
                &record.mode_tag,
                record.validator_set.clone(),
                block_hash,
                record.parent_state_root,
                record.post_state_root,
                record.height,
                record.view,
                record.epoch,
                record.signers.clone(),
                record.bls_aggregate_signature.clone(),
            ) {
                iroha_logger::info!(
                    height,
                    view,
                    roster_len = record.roster_len,
                    signers = record.signers.len(),
                    "block sync: reusing cached precommit signer set for QC"
                );
                return Some(qc);
            }

            iroha_logger::info!(
                height,
                view,
                roster_len = record.roster_len,
                signers = record.signers.len(),
                "block sync: cached precommit signer set could not build QC"
            );
        }
        iroha_logger::info!(
            height,
            view,
            block_signatures = block.signatures().count(),
            "block sync: no QC available for block"
        );
        None
    }

    /// Validate block signatures against the provided topology.
    pub(crate) fn block_signatures_valid(
        block: &SignedBlock,
        topology: &Topology,
        state: &State,
    ) -> Result<(), crate::block::SignatureVerificationError> {
        crate::block::ValidBlock::validate_signatures_subset_without_state(block, topology)?;
        let state_view = state.view();
        crate::block::ValidBlock::enforce_consensus_key_lifecycle(block, topology, &state_view)
    }
}

fn align_topology_for_block_signatures(
    mode_tag: &str,
    topology: &Topology,
    block: &SignedBlock,
    prf_seed: Option<[u8; 32]>,
) -> Topology {
    let mut rotated = topology.clone();
    let height = block.header().height().get();
    let view = block.header().view_change_index();
    match mode_tag {
        PERMISSIONED_TAG => {
            rotated.nth_rotation(view);
        }
        NPOS_TAG => {
            if let Some(seed) = prf_seed {
                let leader = rotated.leader_index_prf(seed, height, view);
                rotated.rotate_preserve_view_to_front(leader);
            } else {
                warn!(
                    height,
                    view, "skipping topology rotation for block signatures: missing PRF seed"
                );
            }
        }
        _ => {}
    }
    rotated
}

fn sample_block_sync_targets(
    peers: &[PeerId],
    gossip_size: usize,
    rng: &mut impl rand::Rng,
) -> Vec<PeerId> {
    if peers.is_empty() || gossip_size == 0 {
        return Vec::new();
    }

    let mut shuffled = peers.to_vec();
    shuffled.shuffle(rng);
    let limit = usize::min(gossip_size, shuffled.len());
    shuffled.truncate(limit);
    shuffled
}

fn select_block_sync_targets(
    peers: &[PeerId],
    world_peers: &BTreeSet<PeerId>,
    gossip_size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<PeerId>, Vec<PeerId>) {
    if peers.is_empty() || gossip_size == 0 {
        return (Vec::new(), Vec::new());
    }

    let mut stray_targets: Vec<_> = peers
        .iter()
        .filter(|peer| !world_peers.contains(*peer))
        .cloned()
        .collect();
    if stray_targets.len() > gossip_size {
        stray_targets.shuffle(rng);
        stray_targets.truncate(gossip_size);
    }

    let mut targets = stray_targets.clone();
    let remaining_budget = gossip_size.saturating_sub(targets.len());
    if remaining_budget > 0 {
        let world_targets: Vec<_> = peers
            .iter()
            .filter(|peer| world_peers.contains(*peer))
            .cloned()
            .collect();
        let sampled = sample_block_sync_targets(&world_targets, remaining_budget, rng);
        targets.extend(sampled);
    }

    (targets, stray_targets)
}

#[cfg(test)]
mod sample_targets_tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    #[test]
    fn samples_at_most_gossip_size() {
        let mut rng = StdRng::seed_from_u64(0xB10C_5EED);
        let peers: Vec<PeerId> = (0..5)
            .map(|_| {
                let (pk, _) = iroha_crypto::KeyPair::random().into_parts();
                PeerId::new(pk)
            })
            .collect();
        let sample = sample_block_sync_targets(&peers, 3, &mut rng);
        assert_eq!(sample.len(), 3);

        let mut rng_same = StdRng::seed_from_u64(0xB10C_5EED);
        let repeat = sample_block_sync_targets(&peers, 3, &mut rng_same);
        assert_eq!(sample, repeat);
    }

    #[test]
    fn samples_all_when_under_limit() {
        let mut rng = StdRng::seed_from_u64(0xCAFE_BABE);
        let peers: Vec<PeerId> = (0..2)
            .map(|_| {
                let (pk, _) = iroha_crypto::KeyPair::random().into_parts();
                PeerId::new(pk)
            })
            .collect();
        let sample = sample_block_sync_targets(&peers, 5, &mut rng);
        assert_eq!(sample.len(), peers.len());
        for peer in peers {
            assert!(sample.contains(&peer));
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    fn peers(count: usize) -> Vec<PeerId> {
        (0..count)
            .map(|_| {
                let (pk, _) = iroha_crypto::KeyPair::random().into_parts();
                PeerId::new(pk)
            })
            .collect()
    }

    #[test]
    fn caps_stray_targets_to_gossip_size() {
        let all_peers = peers(5);
        let world_peers = BTreeSet::new();
        let mut rng = StdRng::seed_from_u64(0xDEAD_BEEF);

        let (targets, stray) = select_block_sync_targets(&all_peers, &world_peers, 3, &mut rng);
        assert_eq!(targets.len(), 3);
        assert_eq!(stray.len(), 3);
        for peer in &targets {
            assert!(all_peers.contains(peer));
        }
    }

    #[test]
    fn fills_remaining_budget_from_world_peers() {
        let all_peers = peers(4);
        let world_peers: BTreeSet<_> = all_peers.iter().take(3).cloned().collect();
        let mut rng = StdRng::seed_from_u64(0xC0FF_EE);

        let (targets, stray) = select_block_sync_targets(&all_peers, &world_peers, 3, &mut rng);
        assert_eq!(stray.len(), 1);
        assert_eq!(targets.len(), 3);
        assert!(targets.iter().any(|peer| !world_peers.contains(peer)));
        assert_eq!(
            targets
                .iter()
                .filter(|peer| world_peers.contains(*peer))
                .count(),
            2
        );
    }
}

#[cfg(test)]
mod signature_topology_tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        block::{BlockHeader, builder::BlockBuilder},
        peer::PeerId,
    };
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn align_topology_for_block_signatures_rotates_npos_prf() {
        let seed = [0x42; 32];
        let keypairs = (0..4).map(|_| KeyPair::random()).collect::<Vec<_>>();
        let peers: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 2);
        let block = BlockBuilder::new(header).build_with_signature(0, keypairs[0].private_key());

        let rotated = align_topology_for_block_signatures(NPOS_TAG, &topology, &block, Some(seed));
        let expected_leader = topology.leader_index_prf(seed, 3, 2);

        assert_eq!(
            rotated.as_ref().first(),
            topology.as_ref().get(expected_leader),
            "PRF leader should be at index 0"
        );
    }
}

#[cfg(test)]
mod prf_seed_tests {
    use std::sync::Arc;

    use iroha_data_model::{
        consensus::{VrfEpochRecord, VrfParticipantRecord},
        parameter::{Parameter, system::SumeragiNposParameters},
    };

    use super::*;
    use crate::{
        prelude::World,
        query::store::LiveQueryStore,
        state::State,
        sumeragi::{npos_seed_for_height, npos_seed_for_height_from_world},
    };

    #[test]
    fn npos_seed_for_height_falls_back_to_chain_hash() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        let seed = npos_seed_for_height(&view, 1);
        let seed_from_world = {
            let world_view = state.world.view();
            npos_seed_for_height_from_world(&world_view, view.chain_id(), 1)
        };
        let chain = view.chain_id().clone().into_inner();
        let expected = {
            let hash = iroha_crypto::Hash::new(chain.as_bytes());
            <[u8; 32]>::from(hash)
        };
        assert_eq!(seed, expected);
        assert_eq!(seed_from_world, expected);
    }

    #[test]
    fn npos_seed_for_height_prefers_epoch_record() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let mut world = state.world.block();
        let params = SumeragiNposParameters {
            epoch_length_blocks: 10,
            ..Default::default()
        };
        world
            .parameters
            .set_parameter(Parameter::Custom(params.into_custom_parameter()));
        world.vrf_epochs.insert(
            2,
            VrfEpochRecord {
                epoch: 2,
                seed: [0xAB; 32],
                epoch_length: 10,
                commit_deadline_offset: 3,
                reveal_deadline_offset: 6,
                roster_len: 0,
                finalized: false,
                updated_at_height: 20,
                participants: Vec::new(),
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            },
        );
        world.commit();

        let view = state.view();
        let seed = npos_seed_for_height(&view, 21);
        let seed_from_world =
            crate::sumeragi::npos_seed_for_height_from_world(view.world(), view.chain_id(), 21);
        assert_eq!(seed, [0xAB; 32]);
        assert_eq!(seed_from_world, [0xAB; 32]);
    }

    #[test]
    fn npos_seed_for_height_falls_back_to_epoch_seed() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let mut world = state.world.block();
        let params = SumeragiNposParameters::default().with_epoch_seed([0xEE; 32]);
        world
            .parameters
            .set_parameter(Parameter::Custom(params.into_custom_parameter()));
        world.commit();

        let view = state.view();
        let seed = npos_seed_for_height(&view, 1);
        let seed_from_world =
            crate::sumeragi::npos_seed_for_height_from_world(view.world(), view.chain_id(), 1);
        assert_eq!(seed, [0xEE; 32]);
        assert_eq!(seed_from_world, [0xEE; 32]);
    }

    #[test]
    fn npos_seed_for_height_derives_next_epoch_seed_after_restart_gap() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let mut world = state.world.block();
        let params = SumeragiNposParameters {
            epoch_length_blocks: 10,
            epoch_seed: [0xEE; 32],
            ..Default::default()
        };
        world
            .parameters
            .set_parameter(Parameter::Custom(params.into_custom_parameter()));
        let record = VrfEpochRecord {
            epoch: 0,
            seed: [0x11; 32],
            epoch_length: 10,
            commit_deadline_offset: 3,
            reveal_deadline_offset: 6,
            roster_len: 3,
            finalized: true,
            updated_at_height: 10,
            participants: vec![
                VrfParticipantRecord {
                    signer: 2,
                    commitment: Some([0x22; 32]),
                    reveal: Some([0x33; 32]),
                    last_updated_height: 10,
                },
                VrfParticipantRecord {
                    signer: 0,
                    commitment: Some([0x44; 32]),
                    reveal: Some([0x55; 32]),
                    last_updated_height: 10,
                },
            ],
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        world.vrf_epochs.insert(0, record.clone());
        world.commit();

        let expected = {
            use iroha_crypto::blake2::{Blake2b512, Digest as _};

            let mut h = Blake2b512::new();
            iroha_crypto::blake2::digest::Update::update(&mut h, &record.seed);
            let mut reveals = vec![(2_u32, [0x33; 32]), (0_u32, [0x55; 32])];
            reveals.sort_by_key(|(signer, _)| *signer);
            for (signer, reveal) in reveals {
                iroha_crypto::blake2::digest::Update::update(&mut h, &signer.to_be_bytes());
                iroha_crypto::blake2::digest::Update::update(&mut h, &reveal);
            }
            let digest = iroha_crypto::blake2::Digest::finalize(h);
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest[..32]);
            out
        };

        let view = state.view();
        let seed = npos_seed_for_height(&view, 11);
        let seed_from_world =
            crate::sumeragi::npos_seed_for_height_from_world(view.world(), view.chain_id(), 11);
        assert_eq!(seed, expected);
        assert_eq!(seed_from_world, expected);
    }
}

#[cfg(test)]
mod roster_metadata_tests {
    use std::sync::Arc;

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
        peer::PeerId,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{prelude::World, query::store::LiveQueryStore, sumeragi::status};

    fn sample_roster_artifacts() -> (Qc, ValidatorSetCheckpoint) {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block_hash = header.hash();
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer = PeerId::new(kp.public_key().clone());
        let roster = vec![peer];
        let signers_bitmap = vec![0b0000_0001];
        let bls_aggregate_signature = vec![0xAA; 96];
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let commit_qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 1,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: bls_aggregate_signature.clone(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            1,
            commit_qc.view,
            block_hash,
            commit_qc.parent_state_root,
            commit_qc.post_state_root,
            roster,
            signers_bitmap,
            bls_aggregate_signature,
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        (commit_qc, checkpoint)
    }

    #[test]
    fn metadata_falls_back_to_commit_roster_journal() {
        status::reset_commit_certs_for_tests();
        status::reset_validator_checkpoints_for_tests();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let (commit_qc, checkpoint) = sample_roster_artifacts();
        let block_hash = commit_qc.subject_block_hash;
        let roster = commit_qc.validator_set.clone();
        state
            .commit_roster_journal
            .write()
            .upsert(commit_qc.clone(), checkpoint.clone(), None);

        let metadata = super::message::roster_metadata_from_state(
            &state,
            kura.as_ref(),
            1,
            block_hash,
            ConsensusMode::Permissioned,
        )
        .expect("metadata should be available from journal");
        assert_eq!(metadata.commit_qc, Some(commit_qc));
        assert_eq!(metadata.validator_checkpoint, Some(checkpoint));
        assert_eq!(metadata.roster_snapshot().expect("roster"), roster);
    }

    #[test]
    fn metadata_fills_missing_npos_stake_snapshot() {
        status::reset_commit_certs_for_tests();
        status::reset_validator_checkpoints_for_tests();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let (mut commit_qc, checkpoint) = sample_roster_artifacts();
        commit_qc.mode_tag = NPOS_TAG.to_string();
        let block_hash = commit_qc.subject_block_hash;
        state
            .commit_roster_journal
            .write()
            .upsert(commit_qc, checkpoint, None);

        let metadata = super::message::roster_metadata_from_state(
            &state,
            kura.as_ref(),
            1,
            block_hash,
            ConsensusMode::Npos,
        )
        .expect("metadata should be available");
        assert!(
            metadata.stake_snapshot.is_some(),
            "missing stake snapshot should be filled"
        );
    }

    #[test]
    fn metadata_accepts_npos_with_stake_snapshot() {
        status::reset_commit_certs_for_tests();
        status::reset_validator_checkpoints_for_tests();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let (mut commit_qc, checkpoint) = sample_roster_artifacts();
        commit_qc.mode_tag = NPOS_TAG.to_string();
        let block_hash = commit_qc.subject_block_hash;
        let snapshot =
            CommitStakeSnapshot::from_roster(state.view().world(), &commit_qc.validator_set)
                .expect("stake snapshot");
        state.commit_roster_journal.write().upsert(
            commit_qc.clone(),
            checkpoint.clone(),
            Some(snapshot.clone()),
        );

        let metadata = super::message::roster_metadata_from_state(
            &state,
            kura.as_ref(),
            1,
            block_hash,
            ConsensusMode::Npos,
        )
        .expect("metadata should be available");
        assert_eq!(metadata.commit_qc, Some(commit_qc));
        assert_eq!(metadata.validator_checkpoint, Some(checkpoint));
        assert_eq!(metadata.stake_snapshot, Some(snapshot));
    }

    #[test]
    fn effective_roster_metadata_prefers_incoming() {
        let (commit_qc, checkpoint) = sample_roster_artifacts();
        let incoming = RosterMetadata {
            commit_qc: Some(commit_qc.clone()),
            validator_checkpoint: None,
            stake_snapshot: None,
        };
        let fallback = RosterMetadata {
            commit_qc: None,
            validator_checkpoint: Some(checkpoint),
            stake_snapshot: None,
        };

        let effective = super::message::effective_roster_metadata(
            Some(&incoming),
            Some(fallback),
            ConsensusMode::Permissioned,
            commit_qc.height,
            commit_qc.subject_block_hash,
        );
        assert_eq!(effective, Some(incoming));
    }

    #[test]
    fn effective_roster_metadata_falls_back_when_incoming_empty() {
        let (_commit_qc, checkpoint) = sample_roster_artifacts();
        let incoming = RosterMetadata {
            commit_qc: None,
            validator_checkpoint: None,
            stake_snapshot: None,
        };
        let fallback = RosterMetadata {
            commit_qc: None,
            validator_checkpoint: Some(checkpoint.clone()),
            stake_snapshot: None,
        };

        let effective = super::message::effective_roster_metadata(
            Some(&incoming),
            Some(fallback.clone()),
            ConsensusMode::Permissioned,
            checkpoint.height,
            checkpoint.block_hash,
        );
        assert_eq!(effective, Some(fallback));
    }

    #[test]
    fn effective_roster_metadata_returns_none_when_empty() {
        let (commit_qc, _checkpoint) = sample_roster_artifacts();
        let incoming = RosterMetadata {
            commit_qc: None,
            validator_checkpoint: None,
            stake_snapshot: None,
        };
        let effective = super::message::effective_roster_metadata(
            Some(&incoming),
            None,
            ConsensusMode::Permissioned,
            commit_qc.height,
            commit_qc.subject_block_hash,
        );
        assert!(effective.is_none());
    }

    #[test]
    fn effective_roster_metadata_requires_snapshot_in_npos() {
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let (mut commit_qc, _checkpoint) = sample_roster_artifacts();
        commit_qc.mode_tag = NPOS_TAG.to_string();
        let snapshot =
            CommitStakeSnapshot::from_roster(state.view().world(), &commit_qc.validator_set)
                .expect("stake snapshot");
        let incoming = RosterMetadata {
            commit_qc: Some(commit_qc.clone()),
            validator_checkpoint: None,
            stake_snapshot: None,
        };
        let fallback = RosterMetadata {
            commit_qc: Some(commit_qc.clone()),
            validator_checkpoint: None,
            stake_snapshot: Some(snapshot.clone()),
        };

        let effective = super::message::effective_roster_metadata(
            Some(&incoming),
            Some(fallback.clone()),
            ConsensusMode::Npos,
            commit_qc.height,
            commit_qc.subject_block_hash,
        );
        assert_eq!(effective, Some(fallback));
    }
}

#[cfg(test)]
mod qc_build_tests {
    use std::collections::BTreeSet;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        block::{BlockHeader, builder::BlockBuilder},
        peer::PeerId,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{query::store::LiveQueryStore, state::World};

    fn qc_preimage(
        chain_id: &ChainId,
        mode_tag: &str,
        phase: Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> Vec<u8> {
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let vote = crate::sumeragi::consensus::Vote {
            phase,
            block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height,
            view,
            epoch,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        crate::sumeragi::consensus::vote_preimage(chain_id, mode_tag, &vote)
    }

    #[allow(clippy::too_many_arguments)]
    fn aggregate_signature_for_signers(
        chain_id: &ChainId,
        mode_tag: &str,
        phase: Phase,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        epoch: u64,
        signers: &BTreeSet<ValidatorIndex>,
        topology: &Topology,
        keypairs: &[KeyPair],
    ) -> Vec<u8> {
        if signers.is_empty() {
            return Vec::new();
        }
        let preimage = qc_preimage(chain_id, mode_tag, phase, block_hash, height, view, epoch);
        let mut signatures = Vec::with_capacity(signers.len());
        for signer in signers {
            let idx = usize::try_from(*signer).expect("signer fits usize");
            let peer = topology.as_ref().get(idx).expect("signer in topology");
            let kp = keypairs
                .iter()
                .find(|kp| kp.public_key() == peer.public_key())
                .expect("matching keypair");
            let sig = Signature::new(kp.private_key(), &preimage);
            signatures.push(sig.payload().to_vec());
        }
        let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
        iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate succeeds")
    }

    #[test]
    fn qc_from_signers_requires_quorum_and_signature() {
        let chain_id = ChainId::from("qc-from-signers");
        let mode_tag = PERMISSIONED_TAG;
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
        let peers: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([1; 32]));
        let height = 1;
        let view = 0;
        let epoch = 0;
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let mut signers = BTreeSet::new();
        signers.insert(ValidatorIndex::try_from(0).expect("index 0"));
        signers.insert(ValidatorIndex::try_from(1).expect("index 1"));

        let aggregate = aggregate_signature_for_signers(
            &chain_id,
            mode_tag,
            Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers,
            &topology,
            &keypairs,
        );
        let qc = BlockSynchronizer::qc_from_signers(
            ConsensusMode::Permissioned,
            None,
            mode_tag,
            peers.clone(),
            block_hash,
            zero_root,
            zero_root,
            height,
            view,
            epoch,
            signers.clone(),
            aggregate,
        );
        assert!(qc.is_some(), "QC should be built with quorum signers");

        let qc_empty = BlockSynchronizer::qc_from_signers(
            ConsensusMode::Permissioned,
            None,
            mode_tag,
            peers.clone(),
            block_hash,
            zero_root,
            zero_root,
            height,
            view,
            epoch,
            signers.clone(),
            Vec::new(),
        );
        assert!(
            qc_empty.is_none(),
            "empty aggregate signature must be rejected"
        );

        let mut partial_signers = BTreeSet::new();
        partial_signers.insert(ValidatorIndex::try_from(0).expect("index 0"));
        let partial = BlockSynchronizer::qc_from_signers(
            ConsensusMode::Permissioned,
            None,
            mode_tag,
            peers,
            block_hash,
            zero_root,
            zero_root,
            height,
            view,
            epoch,
            partial_signers,
            vec![0xAA; 48],
        );
        assert!(partial.is_none(), "insufficient signers must be rejected");
    }

    #[test]
    fn qc_from_signers_requires_stake_snapshot_in_npos() {
        let chain_id = ChainId::from("qc-from-signers-npos");
        let mode_tag = NPOS_TAG;
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
        let peers: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let topology = Topology::new(peers.clone());
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([2; 32]));
        let height = 1;
        let view = 0;
        let epoch = 0;
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let mut signers = BTreeSet::new();
        signers.insert(ValidatorIndex::try_from(0).expect("index 0"));
        signers.insert(ValidatorIndex::try_from(1).expect("index 1"));

        let aggregate = aggregate_signature_for_signers(
            &chain_id,
            mode_tag,
            Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers,
            &topology,
            &keypairs,
        );
        let stake_snapshot = CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&peers),
            entries: peers
                .iter()
                .cloned()
                .map(
                    |peer_id| crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
                        peer_id,
                        stake: iroha_primitives::numeric::Numeric::from(1_u64),
                    },
                )
                .collect(),
        };

        let qc = BlockSynchronizer::qc_from_signers(
            ConsensusMode::Npos,
            Some(&stake_snapshot),
            mode_tag,
            peers.clone(),
            block_hash,
            zero_root,
            zero_root,
            height,
            view,
            epoch,
            signers.clone(),
            aggregate.clone(),
        );
        assert!(qc.is_some(), "stake snapshot should enable NPoS QC");

        let missing_snapshot = BlockSynchronizer::qc_from_signers(
            ConsensusMode::Npos,
            None,
            mode_tag,
            peers,
            block_hash,
            zero_root,
            zero_root,
            height,
            view,
            epoch,
            signers,
            aggregate,
        );
        assert!(
            missing_snapshot.is_none(),
            "missing stake snapshot should reject QC"
        );
    }

    #[test]
    fn block_sync_qc_for_uses_cached_record() {
        let chain_id = ChainId::from("block-sync-qc");
        let mode_tag = PERMISSIONED_TAG;
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
        let peers: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let topology = Topology::new(peers);
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let block = BlockBuilder::new(header).build_with_signature(0, keypairs[0].private_key());
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let state_view = state.view();

        let mut signers = BTreeSet::new();
        signers.insert(ValidatorIndex::try_from(0).expect("index 0"));
        signers.insert(ValidatorIndex::try_from(1).expect("index 1"));
        let aggregate = aggregate_signature_for_signers(
            &chain_id,
            mode_tag,
            Phase::Commit,
            block_hash,
            height,
            view,
            0,
            &signers,
            &topology,
            &keypairs,
        );
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        status::record_precommit_signers(status::PrecommitSignerRecord {
            block_hash,
            height,
            view,
            epoch: 0,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            signers: signers.clone(),
            roster_len: topology.as_ref().len(),
            mode_tag: mode_tag.to_string(),
            bls_aggregate_signature: aggregate.clone(),
            validator_set: topology.as_ref().to_vec(),
            stake_snapshot: None,
        });

        let qc =
            BlockSynchronizer::block_sync_qc_for(&state_view, ConsensusMode::Permissioned, &block)
                .expect("cached QC should be built");
        assert_eq!(qc.subject_block_hash, block_hash);
        assert_eq!(qc.aggregate.bls_aggregate_signature, aggregate);
    }

    #[test]
    fn block_sync_qc_for_accepts_nonzero_epoch() {
        let chain_id = ChainId::from("block-sync-qc-epoch");
        let mode_tag = NPOS_TAG;
        let keypairs = vec![
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
            KeyPair::random_with_algorithm(Algorithm::BlsNormal),
        ];
        let peers: Vec<_> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let topology = Topology::new(peers);
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        let block = BlockBuilder::new(header).build_with_signature(0, keypairs[0].private_key());
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let epoch = 4;
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let state = State::new_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let state_view = state.view();

        let mut signers = BTreeSet::new();
        signers.insert(ValidatorIndex::try_from(0).expect("index 0"));
        signers.insert(ValidatorIndex::try_from(1).expect("index 1"));
        let aggregate = aggregate_signature_for_signers(
            &chain_id,
            mode_tag,
            Phase::Commit,
            block_hash,
            height,
            view,
            epoch,
            &signers,
            &topology,
            &keypairs,
        );
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let stake_snapshot =
            CommitStakeSnapshot::from_roster(state_view.world(), topology.as_ref())
                .expect("stake snapshot");
        status::record_precommit_signers(status::PrecommitSignerRecord {
            block_hash,
            height,
            view,
            epoch,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            signers: signers.clone(),
            roster_len: topology.as_ref().len(),
            mode_tag: mode_tag.to_string(),
            bls_aggregate_signature: aggregate.clone(),
            validator_set: topology.as_ref().to_vec(),
            stake_snapshot: Some(stake_snapshot),
        });

        let qc = BlockSynchronizer::block_sync_qc_for(&state_view, ConsensusMode::Npos, &block)
            .expect("cached QC should be built");
        assert_eq!(qc.subject_block_hash, block_hash);
        assert_eq!(qc.epoch, epoch);
        assert_eq!(qc.aggregate.bls_aggregate_signature, aggregate);
    }
}

pub mod message {
    //! Module containing messages for [`BlockSynchronizer`].

    use super::*;
    use crate::sumeragi::network_topology::Role;

    /// Certified roster hints shipped alongside block sync payloads.
    #[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
    pub struct RosterMetadata {
        /// Optional commit certificate for the block.
        pub commit_qc: Option<Qc>,
        /// Optional validator checkpoint for the block.
        pub validator_checkpoint: Option<ValidatorSetCheckpoint>,
        /// Optional stake snapshot aligned to the validator set.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub stake_snapshot: Option<CommitStakeSnapshot>,
    }

    impl RosterMetadata {
        fn validator_set(&self) -> Option<&[PeerId]> {
            self.commit_qc
                .as_ref()
                .map(|cert| cert.validator_set.as_slice())
                .or_else(|| {
                    self.validator_checkpoint
                        .as_ref()
                        .map(|checkpoint| checkpoint.validator_set.as_slice())
                })
        }

        #[cfg(test)]
        pub(crate) fn roster_snapshot(&self) -> Option<Vec<PeerId>> {
            self.commit_qc
                .as_ref()
                .map(|cert| cert.validator_set.clone())
                .or_else(|| {
                    self.validator_checkpoint
                        .as_ref()
                        .map(|chkpt| chkpt.validator_set.clone())
                })
        }
    }

    /// Get blocks after some block.
    #[derive(Debug, Clone, Encode)]
    pub struct GetBlocksAfter {
        /// Peer id
        pub peer_id: PeerId,
        /// Hash of second to latest block
        pub prev_hash: Option<HashOf<BlockHeader>>,
        /// Hash of latest available block
        pub latest_hash: Option<HashOf<BlockHeader>>,
        /// The block hashes already seen
        pub seen_blocks: BTreeSet<HashOf<BlockHeader>>,
    }

    // Derive Encode/Decode above

    impl GetBlocksAfter {
        /// Construct [`GetBlocksAfter`].
        pub const fn new(
            peer_id: PeerId,
            prev_hash: Option<HashOf<BlockHeader>>,
            latest_hash: Option<HashOf<BlockHeader>>,
            seen_blocks: BTreeSet<HashOf<BlockHeader>>,
        ) -> Self {
            Self {
                peer_id,
                prev_hash,
                latest_hash,
                seen_blocks,
            }
        }
    }

    fn roster_metadata_has_hints(metadata: &RosterMetadata) -> bool {
        metadata.commit_qc.is_some() || metadata.validator_checkpoint.is_some()
    }

    fn roster_metadata_validation_error(
        metadata: &RosterMetadata,
        consensus_mode: ConsensusMode,
    ) -> Option<&'static str> {
        if !roster_metadata_has_hints(metadata) {
            return Some("empty");
        }
        if matches!(consensus_mode, ConsensusMode::Npos) {
            if let Some(snapshot) = metadata.stake_snapshot.as_ref() {
                let Some(roster) = metadata.validator_set() else {
                    return Some("missing_validator_set");
                };
                if !snapshot.matches_roster(roster) {
                    return Some("stake_snapshot_mismatch");
                }
            }
        }
        None
    }

    /// Message used to share blocks to a peer.
    #[derive(Debug, Clone, Encode)]
    pub struct ShareBlocks {
        /// Peer id
        pub peer_id: PeerId,
        /// Blocks
        pub blocks: Vec<SignedBlock>,
        /// Optional precommit QCs for the blocks, best-effort.
        pub qcs: Vec<Option<Qc>>,
        /// Optional roster metadata for each shared block.
        pub rosters: Vec<RosterMetadata>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ShareBlocksLengthError {
        /// Number of blocks does not match number of QCs.
        QcLengthMismatch,
        /// Number of blocks does not match number of roster entries.
        RosterLengthMismatch,
    }

    /// Ensure the block, QC, and roster vectors are aligned for a block sync batch.
    fn validate_share_blocks_lengths(
        blocks_len: usize,
        qcs_len: usize,
        rosters_len: usize,
    ) -> Result<(), ShareBlocksLengthError> {
        if blocks_len != qcs_len {
            return Err(ShareBlocksLengthError::QcLengthMismatch);
        }
        if blocks_len != rosters_len {
            return Err(ShareBlocksLengthError::RosterLengthMismatch);
        }
        Ok(())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum GetBlocksAfterValidationError {
        PrevHashWithoutLatest,
    }

    fn validate_get_blocks_after_request(
        prev_hash: &Option<HashOf<BlockHeader>>,
        latest_hash: &Option<HashOf<BlockHeader>>,
    ) -> Result<(), GetBlocksAfterValidationError> {
        if prev_hash.is_some() && latest_hash.is_none() {
            return Err(GetBlocksAfterValidationError::PrevHashWithoutLatest);
        }
        Ok(())
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ShareBlocksSequenceError {
        Empty,
        HeightMissed,
        PrevBlockHashMismatch,
    }

    fn validate_share_blocks_sequence(
        blocks: &[SignedBlock],
    ) -> Result<(), ShareBlocksSequenceError> {
        if blocks.is_empty() {
            return Err(ShareBlocksSequenceError::Empty);
        }

        for window in blocks.windows(2) {
            if window[1].header().height().get() != window[0].header().height().get() + 1 {
                return Err(ShareBlocksSequenceError::HeightMissed);
            }
            if window[1].header().prev_block_hash() != Some(window[0].hash()) {
                return Err(ShareBlocksSequenceError::PrevBlockHashMismatch);
            }
        }

        Ok(())
    }

    impl ShareBlocks {
        /// Construct [`ShareBlocks`].
        pub const fn new(
            blocks: Vec<SignedBlock>,
            peer_id: PeerId,
            qcs: Vec<Option<Qc>>,
            rosters: Vec<RosterMetadata>,
        ) -> Self {
            Self {
                peer_id,
                blocks,
                qcs,
                rosters,
            }
        }
    }

    // Derive Encode/Decode above

    /// Messages used by peers to communicate during block synchronization.
    #[derive(Debug, Clone, Decode, Encode, FromVariant)]
    pub enum Message {
        /// Request for blocks after the block with `Hash` for the peer with `PeerId`.
        GetBlocksAfter(GetBlocksAfter),
        /// The response to `GetBlocksAfter`. Contains the requested blocks and the id of the peer who shared them.
        ShareBlocks(ShareBlocks),
    }

    // Derive Encode/Decode above

    pub(super) fn roster_metadata_from_state(
        state: &State,
        kura: &Kura,
        block_height: u64,
        block_hash: HashOf<BlockHeader>,
        fallback_consensus_mode: ConsensusMode,
    ) -> Option<RosterMetadata> {
        let consensus_mode = {
            let view = state.view();
            consensus_mode_for_block_sync(&view, block_height, fallback_consensus_mode)
        };
        let fill_snapshot = |mut metadata: RosterMetadata| {
            if matches!(consensus_mode, ConsensusMode::Npos) && metadata.stake_snapshot.is_none() {
                let roster = metadata.validator_set().map(|roster| roster.to_vec());
                if let Some(roster) = roster.as_ref() {
                    metadata.stake_snapshot =
                        CommitStakeSnapshot::from_roster(state.view().world(), roster);
                }
            }
            metadata
        };
        let filter_metadata = |metadata: RosterMetadata, source: &'static str| {
            if let Some(reason) = roster_metadata_validation_error(&metadata, consensus_mode) {
                if reason != "empty" {
                    warn!(
                        height = block_height,
                        block = %block_hash,
                        source,
                        reason,
                        "dropping block sync roster metadata"
                    );
                }
                return None;
            }
            Some(metadata)
        };
        if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
            return filter_metadata(
                fill_snapshot(RosterMetadata {
                    commit_qc: Some(snapshot.commit_qc),
                    validator_checkpoint: Some(snapshot.validator_checkpoint),
                    stake_snapshot: snapshot.stake_snapshot,
                }),
                "commit_roster_journal",
            );
        }

        if let Some(meta) = kura.read_roster_metadata(block_height).and_then(|meta| {
            if meta.block_hash == block_hash {
                Some(meta)
            } else {
                warn!(
                    expected = %block_hash,
                    stored = %meta.block_hash,
                    height = block_height,
                    "ignoring roster sidecar with mismatched hash"
                );
                None
            }
        }) {
            return filter_metadata(
                fill_snapshot(RosterMetadata {
                    commit_qc: meta.commit_qc,
                    validator_checkpoint: meta.validator_checkpoint,
                    stake_snapshot: meta.stake_snapshot,
                }),
                "roster_sidecar",
            );
        }

        if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
            return filter_metadata(
                fill_snapshot(RosterMetadata {
                    commit_qc: Some(snapshot.commit_qc),
                    validator_checkpoint: Some(snapshot.validator_checkpoint),
                    stake_snapshot: snapshot.stake_snapshot,
                }),
                "commit_roster_journal",
            );
        }

        let commit_qc = status::commit_qc_history()
            .into_iter()
            .find(|cert| cert.height == block_height && cert.subject_block_hash == block_hash);
        let validator_checkpoint = status::validator_checkpoint_history()
            .into_iter()
            .find(|chk| chk.height == block_height && chk.block_hash == block_hash);

        match (commit_qc, validator_checkpoint) {
            (Some(cert), checkpoint) => filter_metadata(
                fill_snapshot(RosterMetadata {
                    commit_qc: Some(cert),
                    validator_checkpoint: checkpoint,
                    stake_snapshot: None,
                }),
                "commit_qc_history",
            ),
            (None, Some(checkpoint)) => filter_metadata(
                fill_snapshot(RosterMetadata {
                    commit_qc: None,
                    validator_checkpoint: Some(checkpoint),
                    stake_snapshot: None,
                }),
                "validator_checkpoint_history",
            ),
            _ => None,
        }
    }

    /// Choose roster metadata to attach to block sync updates.
    pub(super) fn effective_roster_metadata(
        incoming: Option<&RosterMetadata>,
        fallback: Option<RosterMetadata>,
        consensus_mode: ConsensusMode,
        block_height: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<RosterMetadata> {
        let incoming_error =
            incoming.and_then(|meta| roster_metadata_validation_error(meta, consensus_mode));
        let incoming_valid = incoming
            .is_some_and(|meta| roster_metadata_validation_error(meta, consensus_mode).is_none());
        if let Some(reason) = incoming_error {
            if reason != "empty" {
                warn!(
                    height = block_height,
                    block = %block_hash,
                    source = "incoming",
                    reason,
                    "dropping incoming block sync roster metadata"
                );
            }
        }
        let fallback_valid = fallback
            .as_ref()
            .is_some_and(|meta| roster_metadata_validation_error(meta, consensus_mode).is_none());
        if matches!(consensus_mode, ConsensusMode::Npos) {
            let incoming_has_snapshot = incoming
                .and_then(|meta| meta.stake_snapshot.as_ref())
                .is_some();
            let fallback_has_snapshot = fallback
                .as_ref()
                .and_then(|meta| meta.stake_snapshot.as_ref())
                .is_some();
            if incoming_valid && incoming_has_snapshot {
                return incoming.cloned();
            }
            if fallback_valid && fallback_has_snapshot {
                return fallback;
            }
            if incoming_valid {
                return incoming.cloned();
            }
            return fallback.filter(|_| fallback_valid);
        }
        if incoming_valid {
            return incoming.cloned();
        }
        fallback.filter(|_| fallback_valid)
    }

    struct BlockSyncValidationContext {
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view_idx: u64,
        block_view: u64,
        signature_topology: Topology,
        commit_quorum: usize,
        commit_signer_count: usize,
        topology_len: usize,
    }

    fn prf_seed_for_block_sync(
        mode_tag: &str,
        state_view: &StateView<'_>,
        block_height: u64,
    ) -> Option<[u8; 32]> {
        (mode_tag == NPOS_TAG)
            .then(|| crate::sumeragi::npos_seed_for_height(state_view, block_height))
    }

    fn consensus_mode_for_block_sync(
        state_view: &StateView<'_>,
        block_height: u64,
        fallback_consensus_mode: ConsensusMode,
    ) -> ConsensusMode {
        crate::sumeragi::effective_consensus_mode_for_height(
            state_view,
            block_height,
            fallback_consensus_mode,
        )
    }

    fn mode_tag_for_block_sync(
        state_view: &StateView<'_>,
        block_height: u64,
        fallback_consensus_mode: ConsensusMode,
    ) -> &'static str {
        match consensus_mode_for_block_sync(state_view, block_height, fallback_consensus_mode) {
            ConsensusMode::Permissioned => PERMISSIONED_TAG,
            ConsensusMode::Npos => NPOS_TAG,
        }
    }

    impl BlockSyncValidationContext {
        fn new(
            block: &SignedBlock,
            topology: &Topology,
            state_view: &StateView<'_>,
            mode_tag: &str,
        ) -> Self {
            let block_hash = block.hash();
            let block_height = block.header().height().get();
            let block_view_idx = block.header().view_change_index();
            let block_view = u64::from(block_view_idx);
            let prf_seed = prf_seed_for_block_sync(mode_tag, state_view, block_height);
            let signature_topology =
                align_topology_for_block_signatures(mode_tag, topology, block, prf_seed);
            let commit_quorum = topology.min_votes_for_commit().max(1);
            let commit_signer_count = signature_topology
                .filter_signatures_by_roles(
                    &[
                        Role::Leader,
                        Role::ValidatingPeer,
                        Role::SetBValidator,
                        Role::ProxyTail,
                    ],
                    block.signatures(),
                )
                .count();
            Self {
                block_hash,
                block_height,
                block_view_idx,
                block_view,
                signature_topology,
                commit_quorum,
                commit_signer_count,
                topology_len: topology.as_ref().len(),
            }
        }
    }

    fn sanitize_block_sync_qc(
        state_view: &StateView<'_>,
        fallback_consensus_mode: ConsensusMode,
        block: &SignedBlock,
        incoming: Option<Qc>,
        context: &BlockSyncValidationContext,
        topology: &Topology,
        block_signers: &BTreeSet<ValidatorIndex>,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> Option<Qc> {
        let consensus_mode = consensus_mode_for_block_sync(
            state_view,
            context.block_height,
            fallback_consensus_mode,
        );
        let mode_tag =
            mode_tag_for_block_sync(state_view, context.block_height, fallback_consensus_mode);
        let expected_epoch = match consensus_mode {
            ConsensusMode::Permissioned => 0,
            ConsensusMode::Npos => crate::sumeragi::epoch_for_height_from_world(
                &state_view.world,
                context.block_height,
            ),
        };
        let derived_qc =
            BlockSynchronizer::block_sync_qc_for(state_view, fallback_consensus_mode, block);
        let candidate = match (incoming, derived_qc) {
            (Some(incoming), Some(derived)) if incoming == derived => Some(incoming),
            (Some(_incoming), Some(derived)) => {
                status::inc_block_sync_qc_replaced();
                warn!(
                    height = context.block_height,
                    view = context.block_view,
                    hash = %context.block_hash,
                    "replacing block sync QC that does not match cached aggregate"
                );
                Some(derived)
            }
            (Some(incoming), None) => {
                status::inc_block_sync_qc_derive_failed();
                let incoming_qc_signers = qc_signer_count(&incoming);
                warn!(
                    height = context.block_height,
                    view = context.block_view,
                    hash = %context.block_hash,
                    block_signers = context.commit_signer_count,
                    commit_quorum = context.commit_quorum,
                    incoming_qc_signers,
                    "keeping incoming block sync QC; no cached precommit signer record"
                );
                Some(incoming)
            }
            (None, derived) => derived,
        };

        let qc = candidate?;
        if qc.subject_block_hash != context.block_hash {
            warn!(
                height = context.block_height,
                view = context.block_view,
                hash = %context.block_hash,
                qc_hash = %qc.subject_block_hash,
                "dropping block sync QC with mismatched block hash"
            );
            return None;
        }
        if qc.height != context.block_height {
            warn!(
                height = context.block_height,
                view = context.block_view,
                qc_height = qc.height,
                "dropping block sync QC with mismatched height"
            );
            return None;
        }
        if qc.view != context.block_view {
            warn!(
                height = context.block_height,
                view = context.block_view,
                qc_view = qc.view,
                "dropping block sync QC with mismatched view"
            );
            return None;
        }
        if qc.epoch != expected_epoch {
            warn!(
                height = context.block_height,
                view = context.block_view,
                expected_epoch,
                qc_epoch = qc.epoch,
                "dropping block sync QC with mismatched epoch"
            );
            return None;
        }
        if !matches!(qc.phase, Phase::Commit) {
            warn!(
                height = context.block_height,
                view = context.block_view,
                phase = ?qc.phase,
                "dropping block sync QC with non-commit phase"
            );
            return None;
        }
        let prf_seed = prf_seed_for_block_sync(mode_tag, state_view, context.block_height);
        if let Err(err) = crate::sumeragi::main_loop::validate_block_sync_qc(
            &qc,
            topology,
            state_view.world(),
            block_signers,
            context.block_view,
            state_view.chain_id(),
            consensus_mode,
            stake_snapshot,
            mode_tag,
            prf_seed,
        ) {
            warn!(
                ?err,
                height = context.block_height,
                view = context.block_view,
                hash = %context.block_hash,
                "dropping block sync QC after validation failure"
            );
            return None;
        }
        Some(qc)
    }

    fn should_drop_block_sync_entry(
        block: &SignedBlock,
        context: &BlockSyncValidationContext,
        signature_check: Result<(), crate::block::SignatureVerificationError>,
        sanitized_qc: Option<&Qc>,
    ) -> bool {
        if let Err(err) = signature_check {
            if sanitized_qc.is_none() {
                status::inc_block_sync_drop_invalid_signatures();
                warn!(
                    ?err,
                    height = context.block_height,
                    view = context.block_view_idx,
                    hash = %context.block_hash,
                    signatures = block.signatures().count(),
                    topology_len = context.topology_len,
                    "dropping block sync update with invalid signatures"
                );
                return true;
            }
            warn!(
                ?err,
                height = context.block_height,
                view = context.block_view_idx,
                hash = %context.block_hash,
                signatures = block.signatures().count(),
                topology_len = context.topology_len,
                "accepting block sync update based on QC despite invalid block signatures"
            );
        }

        let has_commit_signatures =
            crate::block::ValidBlock::is_commit(block, &context.signature_topology).is_ok();
        if !has_commit_signatures && sanitized_qc.is_none() {
            status::inc_block_sync_drop_invalid_signatures();
            warn!(
                height = context.block_height,
                view = context.block_view,
                hash = %context.block_hash,
                signatures = block.signatures().count(),
                "dropping block sync update without quorum evidence"
            );
            return true;
        }

        false
    }

    impl Message {
        #[allow(dead_code)]
        fn commit_role_signers_all(
            block: &SignedBlock,
            topology: &Topology,
        ) -> BTreeSet<ValidatorIndex> {
            let mut signers = BTreeSet::new();
            for signature in topology.filter_signatures_by_roles(
                &[
                    Role::Leader,
                    Role::ValidatingPeer,
                    Role::SetBValidator,
                    Role::ProxyTail,
                ],
                block.signatures(),
            ) {
                if let Ok(signer) = ValidatorIndex::try_from(signature.index()) {
                    signers.insert(signer);
                }
            }
            signers
        }

        #[allow(dead_code)]
        fn commit_role_signers(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Option<BTreeSet<ValidatorIndex>> {
            let signers = Self::commit_role_signers_all(block, topology);
            let quorum = topology.min_votes_for_commit().max(1);
            (signers.len() >= quorum).then_some(signers)
        }

        fn filter_blocks_with_valid_signatures(
            entries: Vec<(SignedBlock, Option<Qc>)>,
            rosters: &BTreeMap<HashOf<BlockHeader>, RosterMetadata>,
            fallback_topology: Option<&Topology>,
            state: &State,
            fallback_consensus_mode: ConsensusMode,
        ) -> (Vec<(SignedBlock, Option<Qc>)>, usize) {
            let mut dropped = 0usize;
            let fallback_topology = fallback_topology.cloned();
            let filtered = entries
                .into_iter()
                .filter_map(|(block, qc)| {
                    let block_hash = block.hash();
                    let block_height = block.header().height().get();
                    let roster_topology = rosters
                        .get(&block_hash)
                        .and_then(RosterMetadata::validator_set)
                        .filter(|roster| !roster.is_empty())
                        .map(|roster| Topology::new(roster.iter().cloned()));
                    let topology = roster_topology.or_else(|| fallback_topology.clone());
                    let Some(topology) = topology else {
                        return Some((block, qc));
                    };
                    let stake_snapshot = rosters
                        .get(&block_hash)
                        .and_then(|meta| meta.stake_snapshot.as_ref());
                    let context = {
                        // Keep the state view short-lived to reduce lock contention under load.
                        let state_view = state.view();
                        let mode_tag = mode_tag_for_block_sync(
                            &state_view,
                            block_height,
                            fallback_consensus_mode,
                        );
                        BlockSyncValidationContext::new(&block, &topology, &state_view, mode_tag)
                    };
                    let signature_check = BlockSynchronizer::block_signatures_valid(
                        &block,
                        &context.signature_topology,
                        state,
                    );
                    let block_signers = if signature_check.is_ok() {
                        Self::commit_role_signers_all(&block, &context.signature_topology)
                    } else {
                        BTreeSet::new()
                    };
                    let sanitized_qc = {
                        let state_view = state.view();
                        sanitize_block_sync_qc(
                            &state_view,
                            fallback_consensus_mode,
                            &block,
                            qc,
                            &context,
                            &topology,
                            &block_signers,
                            stake_snapshot,
                        )
                    };
                    if should_drop_block_sync_entry(
                        &block,
                        &context,
                        signature_check,
                        sanitized_qc.as_ref(),
                    ) {
                        dropped += 1;
                        return None;
                    }

                    Some((block, sanitized_qc))
                })
                .collect();
            (filtered, dropped)
        }

        fn select_blocks_for_share(
            blocks: impl Iterator<Item = Arc<SignedBlock>>,
            seen_blocks: &BTreeSet<HashOf<BlockHeader>>,
            max: usize,
        ) -> Vec<SignedBlock> {
            let mut selected = Vec::new();
            let mut skipping_seen_prefix = true;
            for block in blocks {
                let hash = block.hash();
                if skipping_seen_prefix && seen_blocks.contains(&hash) {
                    continue;
                }
                skipping_seen_prefix = false;
                selected.push((*block).clone());
                if selected.len() >= max {
                    break;
                }
            }
            selected
        }

        fn share_blocks_wire_len(
            origin: &PeerId,
            target: &PeerId,
            ttl: u8,
            blocks: &[SignedBlock],
            qcs: &[Option<Qc>],
            rosters: &[RosterMetadata],
        ) -> usize {
            let payload =
                NetworkMessage::BlockSync(Box::new(Message::ShareBlocks(ShareBlocks::new(
                    blocks.to_vec(),
                    origin.clone(),
                    qcs.to_vec(),
                    rosters.to_vec(),
                ))));
            iroha_p2p::network::data_frame_wire_len(
                origin,
                Some(target),
                ttl,
                iroha_p2p::Priority::Low,
                &payload,
            )
        }

        fn trim_share_blocks_to_frame_cap(
            origin: &PeerId,
            target: &PeerId,
            ttl: u8,
            frame_cap: usize,
            blocks: &mut Vec<SignedBlock>,
            qcs: &mut Vec<Option<Qc>>,
            rosters: &mut Vec<RosterMetadata>,
        ) -> bool {
            if blocks.is_empty() {
                return false;
            }
            if let Err(err) = validate_share_blocks_lengths(blocks.len(), qcs.len(), rosters.len())
            {
                warn!(
                    error = ?err,
                    "block sync response has mismatched lengths; dropping"
                );
                return false;
            }
            if frame_cap == 0 {
                warn!(
                    peer = %target,
                    "block sync response dropped because frame cap is zero"
                );
                return false;
            }

            let initial_size =
                Self::share_blocks_wire_len(origin, target, ttl, blocks, qcs, rosters);
            if initial_size <= frame_cap {
                return true;
            }

            let total = blocks.len();
            while blocks.len() > 1 {
                blocks.pop();
                qcs.pop();
                rosters.pop();
                let size = Self::share_blocks_wire_len(origin, target, ttl, blocks, qcs, rosters);
                if size <= frame_cap {
                    let dropped = total - blocks.len();
                    debug!(
                        peer = %target,
                        kept = blocks.len(),
                        dropped,
                        initial_bytes = initial_size,
                        final_bytes = size,
                        cap = frame_cap,
                        "trimmed block sync response to fit frame cap"
                    );
                    return true;
                }
            }

            let size = Self::share_blocks_wire_len(origin, target, ttl, blocks, qcs, rosters);
            warn!(
                peer = %target,
                initial_bytes = initial_size,
                final_bytes = size,
                cap = frame_cap,
                "block sync response exceeds frame cap even with single block; dropping"
            );
            false
        }

        /// Handles the incoming message.
        #[iroha_futures::telemetry_future]
        pub(super) async fn handle_message(&self, block_sync: &mut BlockSynchronizer) {
            match self {
                Message::GetBlocksAfter(GetBlocksAfter {
                    peer_id,
                    prev_hash,
                    latest_hash,
                    seen_blocks,
                }) => {
                    if let Err(err) = validate_get_blocks_after_request(prev_hash, latest_hash) {
                        warn!(
                            error = ?err,
                            "rejecting block sync request with invalid hash dependencies"
                        );
                        return;
                    }

                    let local_latest_block_hash = block_sync.state.view().latest_block_hash();

                    if *latest_hash == local_latest_block_hash
                        || *prev_hash == local_latest_block_hash
                    {
                        return;
                    }

                    let mut share_unknown_prev = true;
                    let mut requested_latest = false;
                    let start_height = if let Some(hash) = *prev_hash {
                        if let Some(height) = block_sync.kura.get_block_height_by_hash(hash) {
                            height
                                .checked_add(1)
                                .expect("INTERNAL BUG: Blockchain height overflow")
                        } else {
                            let now_height =
                                u64::try_from(block_sync.state.view().height()).unwrap_or(u64::MAX);
                            share_unknown_prev = should_share_unknown_prev_hash(
                                &mut block_sync.unknown_prev_hashes,
                                peer_id,
                                hash,
                                now_height,
                            );
                            if share_unknown_prev {
                                let has_latest = latest_hash
                                    .as_ref()
                                    .and_then(|hash| {
                                        block_sync.kura.get_block_height_by_hash(*hash)
                                    })
                                    .is_some();
                                if !has_latest {
                                    block_sync
                                        .request_latest_blocks_from_peer(peer_id.clone())
                                        .await;
                                    requested_latest = true;
                                }
                                warn!(
                                    peer = %block_sync.peer,
                                    requester = %peer_id,
                                    block = %hash,
                                    requested_latest,
                                    "Block hash not found; sharing from genesis"
                                );
                            } else {
                                debug!(
                                    peer = %block_sync.peer,
                                    requester = %peer_id,
                                    block = %hash,
                                    "skipping repeated block sync response for unknown prev hash"
                                );
                            }
                            nonzero_ext::nonzero!(1_usize)
                        }
                    } else {
                        nonzero_ext::nonzero!(1_usize)
                    };

                    if !share_unknown_prev {
                        return;
                    }

                    let blocks = {
                        let state_view = block_sync.state.view();
                        let blocks_iter = state_view
                            .all_blocks(start_height)
                            .skip_while(|block| Some(block.hash()) == *latest_hash);
                        Self::select_blocks_for_share(
                            blocks_iter,
                            seen_blocks,
                            block_sync.gossip_size.get() as usize,
                        )
                    };

                    if !blocks.is_empty() {
                        trace!(hash=?prev_hash, "Sharing blocks after hash");

                        let qcs: Vec<Option<Qc>> = {
                            let state_view = block_sync.state.view();
                            blocks
                                .iter()
                                .map(|block| {
                                    BlockSynchronizer::block_sync_qc_for(
                                        &state_view,
                                        block_sync.fallback_consensus_mode,
                                        block,
                                    )
                                })
                                .collect()
                        };
                        let rosters: Vec<RosterMetadata> = blocks
                            .iter()
                            .map(|block| {
                                let height = block.header().height().get();
                                let hash = block.hash();
                                roster_metadata_from_state(
                                    &block_sync.state,
                                    &block_sync.kura,
                                    height,
                                    hash,
                                    block_sync.fallback_consensus_mode,
                                )
                                .unwrap_or(RosterMetadata {
                                    commit_qc: None,
                                    validator_checkpoint: None,
                                    stake_snapshot: None,
                                })
                            })
                            .collect();
                        let mut blocks = blocks;
                        let mut qcs = qcs;
                        let mut rosters = rosters;
                        if !Self::trim_share_blocks_to_frame_cap(
                            block_sync.peer.id(),
                            peer_id,
                            block_sync.relay_ttl,
                            block_sync.block_sync_frame_cap,
                            &mut blocks,
                            &mut qcs,
                            &mut rosters,
                        ) {
                            return;
                        }
                        Message::ShareBlocks(ShareBlocks::new(
                            blocks,
                            block_sync.peer.id().clone(),
                            qcs,
                            rosters,
                        ))
                        .send_to(&block_sync.network, peer_id.clone())
                        .await;
                    }
                }
                Message::ShareBlocks(ShareBlocks {
                    peer_id,
                    blocks,
                    qcs,
                    rosters,
                }) => {
                    use crate::sumeragi::message::BlockSyncUpdate;

                    if !block_sync
                        .request_tracker
                        .allow_response(&peer_id, Instant::now())
                    {
                        debug!(
                            peer = %peer_id,
                            total = blocks.len(),
                            "dropping unsolicited block sync batch"
                        );
                        status::inc_block_sync_roster_drop_unsolicited_share_blocks();
                        if let Some(telemetry) = block_sync.telemetry.as_ref() {
                            telemetry.note_block_sync_unsolicited_share_blocks_drop();
                        }
                        return;
                    }

                    let total = blocks.len();
                    if let Err(err) = validate_share_blocks_sequence(blocks) {
                        warn!(
                            error = ?err,
                            total,
                            "rejecting block sync batch: invalid block sequence"
                        );
                        return;
                    }
                    let roster_by_hash: BTreeMap<_, _> = blocks
                        .iter()
                        .zip(rosters.iter())
                        .map(|(block, roster)| (block.hash(), roster.clone()))
                        .collect();
                    if let Err(err) =
                        validate_share_blocks_lengths(blocks.len(), qcs.len(), rosters.len())
                    {
                        warn!(
                            error = ?err,
                            blocks = blocks.len(),
                            qcs = qcs.len(),
                            rosters = rosters.len(),
                            "rejecting block sync batch: mismatched block/QC/roster lengths"
                        );
                        return;
                    }
                    let paired: Vec<_> = blocks.iter().cloned().zip(qcs.iter().cloned()).collect();
                    let fallback_topology = {
                        let state_view = block_sync.state.view();
                        let commit_topology: Vec<_> =
                            state_view.commit_topology().iter().cloned().collect();
                        (!commit_topology.is_empty()).then(|| Topology::new(commit_topology))
                    };
                    let (filtered_blocks, dropped) = Self::filter_blocks_with_valid_signatures(
                        paired,
                        &roster_by_hash,
                        fallback_topology.as_ref(),
                        &block_sync.state,
                        block_sync.fallback_consensus_mode,
                    );
                    if dropped > 0 {
                        warn!(
                            dropped,
                            total, "filtered invalid blocks from block sync batch"
                        );
                    }

                    for (block, incoming_qc) in filtered_blocks {
                        let block_height = block.header().height().get();
                        let block_hash = block.hash();
                        if let Some(nz_height) = NonZeroU64::new(block_height) {
                            block_sync.seen_blocks.insert((nz_height, block_hash));
                        } else {
                            warn!(
                                block_height,
                                "skipping block sync update with zero block height"
                            );
                            continue;
                        }
                        let mut msg = BlockSyncUpdate::from(&block);
                        let incoming_roster = roster_by_hash.get(&block_hash);
                        let fallback = roster_metadata_from_state(
                            &block_sync.state,
                            &block_sync.kura,
                            block_height,
                            block_hash,
                            block_sync.fallback_consensus_mode,
                        );
                        let consensus_mode = {
                            let view = block_sync.state.view();
                            consensus_mode_for_block_sync(
                                &view,
                                block_height,
                                block_sync.fallback_consensus_mode,
                            )
                        };
                        if let Some(metadata) = effective_roster_metadata(
                            incoming_roster,
                            fallback,
                            consensus_mode,
                            block_height,
                            block_hash,
                        ) {
                            msg.commit_qc.clone_from(&metadata.commit_qc);
                            msg.validator_checkpoint
                                .clone_from(&metadata.validator_checkpoint);
                            msg.stake_snapshot.clone_from(&metadata.stake_snapshot);
                        }
                        let derived_qc = if msg.commit_qc.is_none() && incoming_qc.is_none() {
                            let view = block_sync.state.view();
                            BlockSynchronizer::block_sync_qc_for(
                                &view,
                                block_sync.fallback_consensus_mode,
                                &block,
                            )
                        } else {
                            None
                        };
                        if msg.commit_qc.is_none() {
                            if let Some(qc) = incoming_qc.or(derived_qc) {
                                let attach_qc = match consensus_mode {
                                    ConsensusMode::Permissioned => true,
                                    ConsensusMode::Npos => {
                                        if let Some(snapshot) = msg.stake_snapshot.as_ref() {
                                            if snapshot.matches_roster(&qc.validator_set) {
                                                true
                                            } else {
                                                warn!(
                                                    height = block_height,
                                                    block = %block_hash,
                                                    "dropping block sync QC with mismatched stake snapshot"
                                                );
                                                false
                                            }
                                        } else {
                                            warn!(
                                                height = block_height,
                                                block = %block_hash,
                                                "dropping block sync QC without stake snapshot"
                                            );
                                            false
                                        }
                                    }
                                };
                                if attach_qc {
                                    msg.commit_qc = Some(qc);
                                }
                            }
                        }
                        let update = crate::sumeragi::message::BlockMessage::BlockSyncUpdate(msg);
                        let sumeragi = block_sync.sumeragi.clone();
                        let enqueue = tokio::task::spawn_blocking(move || {
                            sumeragi.incoming_block_message(update);
                        });
                        // Avoid blocking the async runtime while waiting on Sumeragi backpressure.
                        if let Err(err) = enqueue.await {
                            warn!(
                                ?err,
                                height = block_height,
                                block = %block_hash,
                                "failed to enqueue block sync update"
                            );
                        }
                    }
                }
            }
        }

        /// Send this message over the network to the specified `peer`.
        #[iroha_futures::telemetry_future]
        #[log("TRACE")]
        pub(super) async fn send_to(self, network: &IrohaNetwork, peer: PeerId) {
            let data = NetworkMessage::BlockSync(Box::new(self));
            network.post(iroha_p2p::Post {
                data,
                peer_id: peer.clone(),
                priority: iroha_p2p::Priority::Low,
            });
        }
    }

    #[cfg(test)]
    mod selection_tests {
        use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

        use iroha_crypto::{KeyPair, PrivateKey};
        use iroha_data_model::peer::PeerId;

        use super::*;
        use crate::block::ValidBlock;

        fn make_block(
            leader_private_key: &PrivateKey,
            height: u64,
            prev_hash: Option<HashOf<BlockHeader>>,
        ) -> SignedBlock {
            let block = ValidBlock::new_dummy_and_modify_header(leader_private_key, |header| {
                let height = NonZeroU64::new(height).expect("height must be non-zero");
                header.set_height(height);
                header.set_prev_block_hash(prev_hash);
            });
            block.into()
        }

        #[test]
        fn select_blocks_skips_seen_prefix() {
            let keypair = KeyPair::random();
            let block1 = make_block(keypair.private_key(), 1, None);
            let block2 = make_block(keypair.private_key(), 2, Some(block1.hash()));
            let block3 = make_block(keypair.private_key(), 3, Some(block2.hash()));
            let block4 = make_block(keypair.private_key(), 4, Some(block3.hash()));
            let blocks = vec![
                Arc::new(block1.clone()),
                Arc::new(block2.clone()),
                Arc::new(block3.clone()),
                Arc::new(block4.clone()),
            ];
            let seen = BTreeSet::from([block1.hash(), block2.hash()]);

            let selected = Message::select_blocks_for_share(blocks.iter().cloned(), &seen, 10);
            let heights: Vec<_> = selected
                .iter()
                .map(|block| block.header().height().get())
                .collect();
            assert_eq!(heights, vec![3, 4]);
        }

        #[test]
        fn select_blocks_keeps_contiguous_after_first_unseen() {
            let keypair = KeyPair::random();
            let block1 = make_block(keypair.private_key(), 1, None);
            let block2 = make_block(keypair.private_key(), 2, Some(block1.hash()));
            let block3 = make_block(keypair.private_key(), 3, Some(block2.hash()));
            let block4 = make_block(keypair.private_key(), 4, Some(block3.hash()));
            let blocks = vec![
                Arc::new(block1.clone()),
                Arc::new(block2.clone()),
                Arc::new(block3.clone()),
                Arc::new(block4.clone()),
            ];
            let seen = BTreeSet::from([block3.hash()]);

            let selected = Message::select_blocks_for_share(blocks.iter().cloned(), &seen, 10);
            let heights: Vec<_> = selected
                .iter()
                .map(|block| block.header().height().get())
                .collect();
            assert_eq!(heights, vec![1, 2, 3, 4]);
        }

        #[test]
        fn trim_share_blocks_to_frame_cap_truncates() {
            let keypair = KeyPair::random();
            let origin = PeerId::new(KeyPair::random().public_key().clone());
            let target = PeerId::new(KeyPair::random().public_key().clone());
            let block1 = make_block(keypair.private_key(), 1, None);
            let block2 = make_block(keypair.private_key(), 2, Some(block1.hash()));
            let mut blocks = vec![block1.clone(), block2];
            let mut qcs = vec![None, None];
            let roster = RosterMetadata {
                commit_qc: None,
                validator_checkpoint: None,
                stake_snapshot: None,
            };
            let mut rosters = vec![roster.clone(), roster];
            let ttl = 8;

            let cap_one = Message::share_blocks_wire_len(
                &origin,
                &target,
                ttl,
                &blocks[..1],
                &qcs[..1],
                &rosters[..1],
            );
            let cap_two =
                Message::share_blocks_wire_len(&origin, &target, ttl, &blocks, &qcs, &rosters);
            assert!(cap_two > cap_one);

            let trimmed = Message::trim_share_blocks_to_frame_cap(
                &origin,
                &target,
                ttl,
                cap_one,
                &mut blocks,
                &mut qcs,
                &mut rosters,
            );

            assert!(trimmed);
            assert_eq!(blocks.len(), 1);
            assert_eq!(qcs.len(), 1);
            assert_eq!(rosters.len(), 1);
            assert_eq!(blocks[0].hash(), block1.hash());
        }
    }

    #[cfg(test)]
    mod share_blocks_runtime_tests {
        use std::{
            collections::{BTreeMap, BTreeSet},
            num::{NonZeroU32, NonZeroU64},
            sync::Arc,
            time::{Duration, Instant},
        };

        use iroha_config::parameters::actual::ConsensusMode;
        use iroha_crypto::KeyPair;
        use iroha_data_model::peer::{Peer, PeerId};

        use super::*;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
            sumeragi::test_sumeragi_handle,
        };

        #[test]
        fn share_blocks_enqueue_does_not_block_tokio_timer() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("tokio runtime");

            runtime.block_on(async {
                let (sumeragi, block_rx) = test_sumeragi_handle(0);
                let kura = Kura::blank_kura_for_testing();
                let state = Arc::new(State::new_for_testing(
                    World::new(),
                    Arc::clone(&kura),
                    LiveQueryStore::start_test(),
                ));
                let peer_id = PeerId::new(KeyPair::random().public_key().clone());
                let peer = Peer::new(
                    "127.0.0.1:0".parse().expect("valid socket address"),
                    peer_id.clone(),
                );
                let sender_peer_id = PeerId::new(KeyPair::random().public_key().clone());
                let mut block_sync = BlockSynchronizer {
                    sumeragi,
                    kura,
                    peer,
                    gossip_period: Duration::from_secs(1),
                    gossip_max_period: Duration::from_secs(1),
                    gossip_size: NonZeroU32::new(1).expect("non-zero gossip size"),
                    gossip_backoff: Duration::from_secs(1),
                    gossip_next_deadline: Instant::now(),
                    network: crate::IrohaNetwork::closed_for_tests(),
                    relay_ttl: 1,
                    block_sync_frame_cap: 1024,
                    state,
                    telemetry: None,
                    seen_blocks: BTreeSet::new(),
                    unknown_prev_hashes: BTreeMap::new(),
                    request_tracker: BlockSyncRequestTracker::new(block_sync_request_ttl(
                        Duration::from_secs(1),
                        Duration::from_secs(1),
                    )),
                    latest_height: 0,
                    last_peers: BTreeSet::new(),
                    last_drop_count: 0,
                    last_drop_at: None,
                    fallback_consensus_mode: ConsensusMode::Permissioned,
                };
                block_sync
                    .request_tracker
                    .record_request(sender_peer_id.clone(), Instant::now());

                let keypair = KeyPair::random();
                let block =
                    ValidBlock::new_dummy_and_modify_header(keypair.private_key(), |header| {
                        let height = NonZeroU64::new(1).expect("non-zero height");
                        header.set_height(height);
                        header.set_prev_block_hash(None);
                    });
                let blocks = vec![block.into()];
                let qcs = vec![None];
                let rosters = vec![RosterMetadata {
                    commit_qc: None,
                    validator_checkpoint: None,
                    stake_snapshot: None,
                }];
                let msg = message::Message::ShareBlocks(message::ShareBlocks::new(
                    blocks,
                    sender_peer_id,
                    qcs,
                    rosters,
                ));

                let unblock_delay = Duration::from_millis(200);
                let unblock = std::thread::spawn(move || {
                    std::thread::sleep(unblock_delay);
                    block_rx
                        .recv_timeout(Duration::from_secs(1))
                        .expect("expected block sync update");
                });

                let start = Instant::now();
                let handle_message = msg.handle_message(&mut block_sync);
                tokio::pin!(handle_message);
                let timer = tokio::time::sleep(Duration::from_millis(20));
                tokio::pin!(timer);

                tokio::select! {
                    biased;
                    _ = &mut handle_message => {
                        panic!("block sync update should wait for queue capacity");
                    }
                    _ = &mut timer => {
                        let elapsed = start.elapsed();
                        assert!(
                            elapsed < Duration::from_millis(150),
                            "timer delayed by block sync enqueue: {elapsed:?}"
                        );
                    }
                }

                handle_message.await;
                unblock.join().expect("unblock thread");
            });
        }

        #[test]
        fn unsolicited_share_blocks_increments_telemetry() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("tokio runtime");

            runtime.block_on(async {
                let (sumeragi, _block_rx) = test_sumeragi_handle(0);
                let kura = Kura::blank_kura_for_testing();
                let state = Arc::new(State::new_for_testing(
                    World::new(),
                    Arc::clone(&kura),
                    LiveQueryStore::start_test(),
                ));
                let peer_id = PeerId::new(KeyPair::random().public_key().clone());
                let peer = Peer::new(
                    "127.0.0.1:0".parse().expect("valid socket address"),
                    peer_id,
                );
                let sender_peer_id = PeerId::new(KeyPair::random().public_key().clone());
                let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
                let telemetry = Telemetry::new(metrics.clone(), true);
                let mut block_sync = BlockSynchronizer {
                    sumeragi,
                    kura,
                    peer,
                    gossip_period: Duration::from_secs(1),
                    gossip_max_period: Duration::from_secs(1),
                    gossip_size: NonZeroU32::new(1).expect("non-zero gossip size"),
                    gossip_backoff: Duration::from_secs(1),
                    gossip_next_deadline: Instant::now(),
                    network: crate::IrohaNetwork::closed_for_tests(),
                    relay_ttl: 1,
                    block_sync_frame_cap: 1024,
                    state,
                    telemetry: Some(telemetry),
                    seen_blocks: BTreeSet::new(),
                    unknown_prev_hashes: BTreeMap::new(),
                    request_tracker: BlockSyncRequestTracker::new(block_sync_request_ttl(
                        Duration::from_secs(1),
                        Duration::from_secs(1),
                    )),
                    latest_height: 0,
                    last_peers: BTreeSet::new(),
                    last_drop_count: 0,
                    last_drop_at: None,
                    fallback_consensus_mode: ConsensusMode::Permissioned,
                };

                let keypair = KeyPair::random();
                let block =
                    ValidBlock::new_dummy_and_modify_header(keypair.private_key(), |header| {
                        let height = NonZeroU64::new(1).expect("non-zero height");
                        header.set_height(height);
                        header.set_prev_block_hash(None);
                    });
                let blocks = vec![block.into()];
                let qcs = vec![None];
                let rosters = vec![RosterMetadata {
                    commit_qc: None,
                    validator_checkpoint: None,
                    stake_snapshot: None,
                }];
                let msg = message::Message::ShareBlocks(message::ShareBlocks::new(
                    blocks,
                    sender_peer_id,
                    qcs,
                    rosters,
                ));

                msg.handle_message(&mut block_sync).await;

                assert_eq!(
                    metrics
                        .sumeragi_block_sync_share_blocks_unsolicited_total
                        .get(),
                    1
                );
            });
        }
    }

    mod candidate {
        use super::*;

        #[derive(Decode)]
        struct GetBlocksAfterCandidate {
            peer: PeerId,
            prev_hash: Option<HashOf<BlockHeader>>,
            latest_hash: Option<HashOf<BlockHeader>>,
            seen_blocks: BTreeSet<HashOf<BlockHeader>>,
        }

        #[derive(Decode)]
        struct ShareBlocksCandidate {
            peer: PeerId,
            blocks: Vec<SignedBlock>,
            qcs: Vec<Option<Qc>>,
            rosters: Vec<RosterMetadata>,
        }

        #[derive(Debug)]
        enum ShareBlocksError {
            HeightMissed,
            PrevBlockHashMismatch,
            Empty,
            QcLengthMismatch,
            RosterLengthMismatch,
        }

        impl GetBlocksAfterCandidate {
            fn validate(self) -> Result<GetBlocksAfter, norito::codec::Error> {
                if self.prev_hash.is_some() && self.latest_hash.is_none() {
                    return Err(norito::codec::Error::from(
                        "Latest hash must be defined if previous hash is",
                    ));
                }

                Ok(GetBlocksAfter {
                    peer_id: self.peer,
                    prev_hash: self.prev_hash,
                    latest_hash: self.latest_hash,
                    seen_blocks: self.seen_blocks,
                })
            }
        }

        impl From<ShareBlocksError> for norito::codec::Error {
            fn from(value: ShareBlocksError) -> Self {
                match value {
                    ShareBlocksError::Empty => "Blocks are empty",
                    ShareBlocksError::HeightMissed => "There is a gap between blocks",
                    ShareBlocksError::PrevBlockHashMismatch => {
                        "Mismatch between previous block in the header and actual hash"
                    }
                    ShareBlocksError::QcLengthMismatch => "Blocks and QCs lengths differ",
                    ShareBlocksError::RosterLengthMismatch => "Blocks and rosters lengths differ",
                }
                .into()
            }
        }

        impl From<ShareBlocksLengthError> for ShareBlocksError {
            fn from(value: ShareBlocksLengthError) -> Self {
                match value {
                    ShareBlocksLengthError::QcLengthMismatch => ShareBlocksError::QcLengthMismatch,
                    ShareBlocksLengthError::RosterLengthMismatch => {
                        ShareBlocksError::RosterLengthMismatch
                    }
                }
            }
        }

        impl ShareBlocksCandidate {
            fn validate(self) -> Result<ShareBlocks, ShareBlocksError> {
                if self.blocks.is_empty() {
                    return Err(ShareBlocksError::Empty);
                }
                validate_share_blocks_lengths(
                    self.blocks.len(),
                    self.qcs.len(),
                    self.rosters.len(),
                )
                .map_err(ShareBlocksError::from)?;

                self.blocks.windows(2).try_for_each(|wnd| {
                    if wnd[1].header().height().get() != wnd[0].header().height().get() + 1 {
                        return Err(ShareBlocksError::HeightMissed);
                    }
                    if wnd[1].header().prev_block_hash() != Some(wnd[0].hash()) {
                        return Err(ShareBlocksError::PrevBlockHashMismatch);
                    }
                    Ok(())
                })?;

                Ok(ShareBlocks {
                    peer_id: self.peer,
                    blocks: self.blocks,
                    qcs: self.qcs,
                    rosters: self.rosters,
                })
            }
        }

        impl<'de> norito::NoritoDeserialize<'de> for GetBlocksAfter {
            fn deserialize(archived: &'de norito::Archived<GetBlocksAfter>) -> Self {
                let candidate = <GetBlocksAfterCandidate as norito::NoritoDeserialize>::deserialize(
                    archived.cast(),
                );
                GetBlocksAfter {
                    peer_id: candidate.peer,
                    prev_hash: candidate.prev_hash,
                    latest_hash: candidate.latest_hash,
                    seen_blocks: candidate.seen_blocks,
                }
            }

            fn try_deserialize(
                archived: &'de norito::Archived<GetBlocksAfter>,
            ) -> Result<Self, norito::Error> {
                let candidate = <GetBlocksAfterCandidate as norito::NoritoDeserialize>::deserialize(
                    archived.cast(),
                );
                candidate.validate()
            }
        }

        impl<'de> norito::NoritoDeserialize<'de> for ShareBlocks {
            fn deserialize(archived: &'de norito::Archived<ShareBlocks>) -> Self {
                let candidate = <ShareBlocksCandidate as norito::NoritoDeserialize>::deserialize(
                    archived.cast(),
                );
                ShareBlocks {
                    peer_id: candidate.peer,
                    blocks: candidate.blocks,
                    qcs: candidate.qcs,
                    rosters: candidate.rosters,
                }
            }

            fn try_deserialize(
                archived: &'de norito::Archived<ShareBlocks>,
            ) -> Result<Self, norito::Error> {
                let candidate = <ShareBlocksCandidate as norito::NoritoDeserialize>::deserialize(
                    archived.cast(),
                );
                candidate.validate().map_err(Into::into)
            }
        }

        #[cfg(test)]
        mod tests {
            use std::collections::BTreeSet;

            use iroha_crypto::{Hash, HashOf, KeyPair};
            use norito::codec::{Decode, Encode};

            use super::*;
            use crate::block::ValidBlock;

            #[test]
            fn candidate_empty() {
                let (leader_public_key, _) = KeyPair::random().into_parts();
                let leader_peer = PeerId::new(leader_public_key);
                let candidate = ShareBlocksCandidate {
                    blocks: Vec::new(),
                    peer: leader_peer,
                    qcs: Vec::new(),
                    rosters: Vec::new(),
                };
                assert!(matches!(candidate.validate(), Err(ShareBlocksError::Empty)))
            }

            #[test]
            fn candidate_height_missed() {
                let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
                let leader_peer_id = PeerId::new(leader_public_key);
                let block0: SignedBlock = ValidBlock::new_dummy(&leader_private_key).into();
                let block1 =
                    ValidBlock::new_dummy_and_modify_header(&leader_private_key, |header| {
                        header.set_height(block0.header().height().checked_add(2).unwrap());
                    })
                    .into();
                let candidate = ShareBlocksCandidate {
                    blocks: vec![block0, block1],
                    peer: leader_peer_id,
                    qcs: vec![None, None],
                    rosters: vec![
                        RosterMetadata {
                            commit_qc: None,
                            validator_checkpoint: None,
                            stake_snapshot: None,
                        };
                        2
                    ],
                };
                assert!(matches!(
                    candidate.validate(),
                    Err(ShareBlocksError::HeightMissed)
                ))
            }

            #[test]
            fn candidate_prev_block_hash_mismatch() {
                let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
                let leader_peer_id = PeerId::new(leader_public_key);
                let block0: SignedBlock = ValidBlock::new_dummy(&leader_private_key).into();
                let wrong_prev_hash =
                    HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
                let block1 =
                    ValidBlock::new_dummy_and_modify_header(&leader_private_key, |header| {
                        header.set_height(block0.header().height().checked_add(1).unwrap());
                        header.set_prev_block_hash(Some(wrong_prev_hash));
                    })
                    .into();
                let candidate = ShareBlocksCandidate {
                    blocks: vec![block0, block1],
                    peer: leader_peer_id,
                    qcs: vec![None, None],
                    rosters: vec![
                        RosterMetadata {
                            commit_qc: None,
                            validator_checkpoint: None,
                            stake_snapshot: None,
                        };
                        2
                    ],
                };
                assert!(matches!(
                    candidate.validate(),
                    Err(ShareBlocksError::PrevBlockHashMismatch)
                ))
            }

            #[test]
            fn candidate_roster_length_mismatch() {
                let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
                let leader_peer_id = PeerId::new(leader_public_key);
                let block0: SignedBlock = ValidBlock::new_dummy(&leader_private_key).into();
                let candidate = ShareBlocksCandidate {
                    blocks: vec![block0],
                    peer: leader_peer_id,
                    qcs: vec![None],
                    rosters: Vec::new(),
                };
                assert!(matches!(
                    candidate.validate(),
                    Err(ShareBlocksError::RosterLengthMismatch)
                ))
            }

            #[test]
            fn candidate_qc_length_mismatch() {
                let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
                let leader_peer_id = PeerId::new(leader_public_key);
                let block0: SignedBlock = ValidBlock::new_dummy(&leader_private_key).into();
                let candidate = ShareBlocksCandidate {
                    blocks: vec![block0],
                    peer: leader_peer_id,
                    qcs: Vec::new(),
                    rosters: vec![RosterMetadata {
                        commit_qc: None,
                        validator_checkpoint: None,
                        stake_snapshot: None,
                    }],
                };
                assert!(matches!(
                    candidate.validate(),
                    Err(ShareBlocksError::QcLengthMismatch)
                ))
            }

            #[test]
            fn candidate_ok() {
                let (leader_public_key, leader_private_key) = KeyPair::random().into_parts();
                let leader_peer_id = PeerId::new(leader_public_key);
                let block0: SignedBlock = ValidBlock::new_dummy(&leader_private_key).into();
                let block1 =
                    ValidBlock::new_dummy_and_modify_header(&leader_private_key, |header| {
                        header.set_height(block0.header().height().checked_add(1).unwrap());
                        header.set_prev_block_hash(Some(block0.hash()));
                    })
                    .into();
                let candidate = ShareBlocksCandidate {
                    blocks: vec![block0, block1],
                    peer: leader_peer_id,
                    qcs: vec![None, None],
                    rosters: vec![
                        RosterMetadata {
                            commit_qc: None,
                            validator_checkpoint: None,
                            stake_snapshot: None,
                        };
                        2
                    ],
                };
                assert!(candidate.validate().is_ok())
            }

            #[test]
            fn decode_invalid_get_blocks_after_returns_error() {
                let (leader_public_key, _) = KeyPair::random().into_parts();
                let leader_peer = PeerId::new(leader_public_key);
                let prev_hash =
                    HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
                let invalid =
                    GetBlocksAfter::new(leader_peer, Some(prev_hash), None, BTreeSet::new());
                let bytes = invalid.encode();

                let decoded = GetBlocksAfter::decode(&mut bytes.as_slice());
                assert!(decoded.is_err());
            }

            #[test]
            fn decode_invalid_share_blocks_returns_error() {
                let (leader_public_key, _) = KeyPair::random().into_parts();
                let leader_peer = PeerId::new(leader_public_key);
                let invalid = ShareBlocks::new(Vec::new(), leader_peer, Vec::new(), Vec::new());
                let bytes = invalid.encode();

                let decoded = ShareBlocks::decode(&mut bytes.as_slice());
                assert!(decoded.is_err());
            }
        }
    }

    #[cfg(test)]
    mod validation_tests {
        use core::num::NonZeroU64;

        use iroha_crypto::{Hash, HashOf, KeyPair, PrivateKey};

        use super::*;
        use crate::block::ValidBlock;

        fn make_block(
            leader_private_key: &PrivateKey,
            height: u64,
            prev_hash: Option<HashOf<BlockHeader>>,
        ) -> SignedBlock {
            let block = ValidBlock::new_dummy_and_modify_header(leader_private_key, |header| {
                let height = NonZeroU64::new(height).expect("height must be non-zero");
                header.set_height(height);
                header.set_prev_block_hash(prev_hash);
            });
            block.into()
        }

        #[test]
        fn validate_get_blocks_after_rejects_prev_hash_without_latest() {
            let prev_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
            let err = validate_get_blocks_after_request(&Some(prev_hash), &None)
                .expect_err("prev hash without latest must be rejected");
            assert_eq!(err, GetBlocksAfterValidationError::PrevHashWithoutLatest);
        }

        #[test]
        fn validate_share_blocks_sequence_rejects_empty() {
            let err = validate_share_blocks_sequence(&[])
                .expect_err("empty block batch must be rejected");
            assert_eq!(err, ShareBlocksSequenceError::Empty);
        }

        #[test]
        fn validate_share_blocks_sequence_rejects_height_gap() {
            let keypair = KeyPair::random();
            let block1 = make_block(keypair.private_key(), 1, None);
            let block2 = make_block(keypair.private_key(), 3, Some(block1.hash()));
            let err = validate_share_blocks_sequence(&[block1, block2])
                .expect_err("height gap must be rejected");
            assert_eq!(err, ShareBlocksSequenceError::HeightMissed);
        }

        #[test]
        fn validate_share_blocks_sequence_rejects_prev_hash_mismatch() {
            let keypair = KeyPair::random();
            let block1 = make_block(keypair.private_key(), 1, None);
            let wrong_prev = HashOf::from_untyped_unchecked(Hash::prehashed([0xCD; Hash::LENGTH]));
            let block2 = make_block(keypair.private_key(), 2, Some(wrong_prev));
            let err = validate_share_blocks_sequence(&[block1, block2])
                .expect_err("prev hash mismatch must be rejected");
            assert_eq!(err, ShareBlocksSequenceError::PrevBlockHashMismatch);
        }
    }

    #[cfg(test)]
    mod filter_tests {
        use std::{
            collections::{BTreeMap, BTreeSet},
            num::NonZeroU64,
            str::FromStr,
            sync::atomic::{AtomicU64, Ordering},
        };

        use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature, SignatureOf};
        use iroha_data_model::{
            block::BlockSignature,
            consensus::{
                ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
                VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint,
            },
            parameter::{Parameter, Parameters, system::SumeragiNposParameters},
            peer::PeerId,
        };
        use iroha_schema::Ident;

        use super::*;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
            sumeragi::{consensus::ValidatorIndex, network_topology::Topology},
        };

        fn test_chain_config() -> (ChainId, String) {
            (
                ChainId::from("00000000-0000-0000-0000-000000000000"),
                PERMISSIONED_TAG.to_owned(),
            )
        }

        fn state_with_consensus_keys(
            peers: &[(PublicKey, &str)],
            activation_height: u64,
            expiry_height: Option<u64>,
            status: ConsensusKeyStatus,
            overlap_grace: u64,
            expiry_grace: u64,
        ) -> State {
            let mut world = World::new();
            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = overlap_grace;
            params.sumeragi.key_expiry_grace_blocks = expiry_grace;
            world.parameters = mv::cell::Cell::new(params);
            for (pk, name) in peers {
                let id = ConsensusKeyId::new(
                    ConsensusKeyRole::Validator,
                    Ident::from_str(name).expect("consensus key name parses"),
                );
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: pk.clone(),
                    pop: None,
                    activation_height,
                    expiry_height,
                    hsm: None,
                    replaces: None,
                    status,
                };
                world.consensus_keys.insert(id.clone(), record.clone());
                world.consensus_keys_by_pk.insert(pk.to_string(), vec![id]);
            }
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            State::new_for_testing(world, kura, query)
        }

        fn state_with_consensus_key_pops(keypairs: &[&KeyPair]) -> State {
            let world = World::new();
            {
                let mut block = world.block();
                for keypair in keypairs {
                    let pk = keypair.public_key();
                    let pop = iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("pop for peer key");
                    let id = crate::state::derive_validator_key_id(pk);
                    let record = ConsensusKeyRecord {
                        id: id.clone(),
                        public_key: pk.clone(),
                        pop: Some(pop),
                        activation_height: 0,
                        expiry_height: None,
                        hsm: None,
                        replaces: None,
                        status: ConsensusKeyStatus::Active,
                    };
                    block.consensus_keys.insert(id.clone(), record);
                    block.consensus_keys_by_pk.insert(pk.to_string(), vec![id]);
                }
                block.commit();
            }
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            State::new_for_testing(world, kura, query)
        }

        fn unique_dummy_block(
            leader_private_key: &PrivateKey,
            f: impl FnOnce(&mut iroha_data_model::block::BlockHeader),
        ) -> ValidBlock {
            static COUNTER: AtomicU64 = AtomicU64::new(1);
            ValidBlock::new_dummy_and_modify_header(leader_private_key, |header| {
                // Avoid cross-test hash collisions when global QC cache is shared.
                header.creation_time_ms = COUNTER.fetch_add(1, Ordering::Relaxed);
                f(header);
            })
        }

        fn qc_preimage(
            chain_id: &ChainId,
            mode_tag: &str,
            phase: Phase,
            block_hash: HashOf<BlockHeader>,
            height: u64,
            view: u64,
            epoch: u64,
        ) -> Vec<u8> {
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            let vote = crate::sumeragi::consensus::Vote {
                phase,
                block_hash,
                parent_state_root: zero_root,
                post_state_root: zero_root,
                height,
                view,
                epoch,
                highest_qc: None,
                signer: 0,
                bls_sig: Vec::new(),
            };
            crate::sumeragi::consensus::vote_preimage(chain_id, mode_tag, &vote)
        }

        #[allow(clippy::too_many_arguments)]
        fn aggregate_signature_for_signers(
            chain_id: &ChainId,
            mode_tag: &str,
            phase: Phase,
            block_hash: HashOf<BlockHeader>,
            height: u64,
            view: u64,
            epoch: u64,
            signers: &BTreeSet<ValidatorIndex>,
            topology: &Topology,
            keypairs: &[KeyPair],
        ) -> Vec<u8> {
            if signers.is_empty() {
                return Vec::new();
            }
            let preimage = qc_preimage(chain_id, mode_tag, phase, block_hash, height, view, epoch);
            let mut signatures = Vec::with_capacity(signers.len());
            for signer in signers {
                let idx = usize::try_from(*signer).expect("signer fits usize");
                let peer = topology.as_ref().get(idx).expect("signer in topology");
                let kp = keypairs
                    .iter()
                    .find(|kp| kp.public_key() == peer.public_key())
                    .expect("matching keypair");
                let sig = Signature::new(kp.private_key(), &preimage);
                signatures.push(sig.payload().to_vec());
            }
            let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
            iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signature")
        }

        #[allow(clippy::too_many_arguments)]
        fn qc_from_signers_with_aggregate(
            chain_id: &ChainId,
            mode_tag: &str,
            block_hash: HashOf<BlockHeader>,
            height: u64,
            view: u64,
            epoch: u64,
            signers: BTreeSet<ValidatorIndex>,
            topology: &Topology,
            keypairs: &[KeyPair],
        ) -> Qc {
            let aggregate_signature = aggregate_signature_for_signers(
                chain_id,
                mode_tag,
                Phase::Commit,
                block_hash,
                height,
                view,
                epoch,
                &signers,
                topology,
                keypairs,
            );
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            BlockSynchronizer::qc_from_signers(
                ConsensusMode::Permissioned,
                None,
                mode_tag,
                topology.as_ref().to_vec(),
                block_hash,
                zero_root,
                zero_root,
                height,
                view,
                epoch,
                signers,
                aggregate_signature,
            )
            .expect("QC should build from signers")
        }

        #[test]
        fn validation_context_captures_commit_signers() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
            });
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let state_view = state.view();
            let (_, mode_tag) = test_chain_config();
            let context =
                super::BlockSyncValidationContext::new(&block, &topology, &state_view, &mode_tag);

            assert_eq!(context.commit_quorum, 2);
            assert_eq!(context.commit_signer_count, 2);
            assert_eq!(context.topology_len, 2);
            assert_eq!(context.block_height, 2);
            assert_eq!(context.block_view_idx, 0);
        }

        #[test]
        fn sanitize_block_sync_qc_keeps_incoming_without_cached_signers() {
            crate::sumeragi::status::reset_precommit_signer_history_for_tests();
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |_| {});
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();
            let signers = super::Message::commit_role_signers(&block, &topology)
                .expect("commit quorum should be met");
            let qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                signers.clone(),
                &topology,
                &[kp_leader.clone(), kp_validator.clone()],
            );
            let context =
                super::BlockSyncValidationContext::new(&block, &topology, &state_view, &mode_tag);
            let sanitized = super::sanitize_block_sync_qc(
                &state_view,
                ConsensusMode::Permissioned,
                &block,
                Some(qc.clone()),
                &context,
                &topology,
                &signers,
                None,
            );

            assert_eq!(sanitized, Some(qc));
        }

        #[test]
        fn sanitize_block_sync_qc_drops_view_mismatch() {
            crate::sumeragi::status::reset_precommit_signer_history_for_tests();
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |_| {});
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();
            let signers = super::Message::commit_role_signers(&block, &topology)
                .expect("commit quorum should be met");
            let qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index().saturating_add(1),
                0,
                signers.clone(),
                &topology,
                &[kp_leader.clone(), kp_validator.clone()],
            );
            let context =
                super::BlockSyncValidationContext::new(&block, &topology, &state_view, &mode_tag);
            let sanitized = super::sanitize_block_sync_qc(
                &state_view,
                ConsensusMode::Permissioned,
                &block,
                Some(qc),
                &context,
                &topology,
                &signers,
                None,
            );

            assert!(sanitized.is_none());
        }

        #[test]
        fn block_sync_keeps_qc_on_bad_signatures() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_wrong = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
            });
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let mut invalid_block = block.clone();
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(kp_wrong.private_key(), invalid_block.hash()),
            ));
            invalid_block
                .replace_signatures(signatures)
                .expect("signature replacement succeeds");

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();
            let signers = super::Message::commit_role_signers(&block, &topology)
                .expect("commit quorum should be met");
            let qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                signers,
                &topology,
                &[kp_leader.clone(), kp_validator.clone()],
            );
            let context = super::BlockSyncValidationContext::new(
                &invalid_block,
                &topology,
                &state_view,
                &mode_tag,
            );
            let signature_check = BlockSynchronizer::block_signatures_valid(
                &invalid_block,
                &context.signature_topology,
                &state,
            );

            assert!(
                !super::should_drop_block_sync_entry(
                    &invalid_block,
                    &context,
                    signature_check,
                    Some(&qc),
                ),
                "QC should keep the block sync update despite invalid signatures"
            );
        }

        #[test]
        fn validate_signatures_subset_without_state_rejects_invalid_signatures() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_wrong = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![PeerId::new(kp_leader.public_key().clone())]);

            let valid_block: SignedBlock = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
            })
            .into();
            let mut invalid_block = valid_block.clone();

            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(kp_wrong.private_key(), invalid_block.hash()),
            ));
            invalid_block
                .replace_signatures(signatures)
                .expect("signature replacement succeeds");

            assert!(
                ValidBlock::validate_signatures_subset_without_state(&valid_block, &topology)
                    .is_ok()
            );
            assert!(
                ValidBlock::validate_signatures_subset_without_state(&invalid_block, &topology)
                    .is_err()
            );
        }

        #[test]
        fn filter_blocks_drops_blocks_without_quorum_or_qc() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut valid_block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
            });
            valid_block.sign(&kp_validator, &topology);
            let valid_block: SignedBlock = valid_block.into();

            let insufficient_block: SignedBlock =
                unique_dummy_block(kp_leader.private_key(), |_| {}).into();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(insufficient_block, None), (valid_block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 1);
            assert_eq!(filtered, vec![(valid_block, None)]);
        }

        #[test]
        fn filter_blocks_rejects_tampered_signature() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_wrong = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![PeerId::new(kp_leader.public_key().clone())]);

            let valid_block: SignedBlock = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(3_u64));
            })
            .into();
            let mut invalid_block = valid_block.clone();

            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(kp_wrong.private_key(), invalid_block.hash()),
            ));
            invalid_block
                .replace_signatures(signatures)
                .expect("signature replacement succeeds");

            let state = state_with_consensus_key_pops(&[&kp_leader]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(invalid_block, None), (valid_block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 1);
            assert_eq!(filtered, vec![(valid_block, None)]);
        }

        #[test]
        fn filter_blocks_rotates_topology_for_permissioned_view() {
            let kp_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_a.public_key().clone()),
                PeerId::new(kp_b.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_b.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
                header.set_view_change_index(1);
            });
            let mut rotated = topology.clone();
            rotated.nth_rotation(1);
            block.sign(&kp_a, &rotated);
            let block: SignedBlock = block.into();

            let state = state_with_consensus_key_pops(&[&kp_a, &kp_b]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, None)]);
        }

        #[test]
        fn filter_blocks_rotates_topology_for_npos_view() {
            let kp_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_a.public_key().clone()),
                PeerId::new(kp_b.public_key().clone()),
            ]);
            let height = 2_u64;

            let mut chosen = None;
            for seed in [[0x5A; 32], [0xA5; 32], [0x3C; 32]] {
                for view in 0_u64..=2 {
                    let mut permissioned = topology.clone();
                    permissioned.nth_rotation(view);
                    let mut npos = topology.clone();
                    let leader = npos.leader_index_prf(seed, height, view);
                    npos.rotate_preserve_view_to_front(leader);
                    if permissioned.as_ref() != npos.as_ref() {
                        chosen = Some((seed, view, npos));
                        break;
                    }
                }
                if chosen.is_some() {
                    break;
                }
            }
            let (seed, view, rotated) = chosen.expect("seed should yield distinct NPoS rotation");

            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let mut world = state.world.block();
            let params = SumeragiNposParameters::default().with_epoch_seed(seed);
            world
                .parameters
                .set_parameter(Parameter::Custom(params.into_custom_parameter()));
            world.commit();

            let height_nz = NonZeroU64::new(height).expect("height must be non-zero");
            let mut block: SignedBlock = unique_dummy_block(kp_a.private_key(), |header| {
                header.set_height(height_nz);
                header.set_view_change_index(view);
            })
            .into();
            let block_hash = block.hash();
            let mut signatures = BTreeSet::new();
            for kp in [&kp_a, &kp_b] {
                let idx = rotated
                    .position(kp.public_key())
                    .expect("signer in rotated topology");
                signatures.insert(BlockSignature::new(
                    idx as u64,
                    SignatureOf::from_hash(kp.private_key(), block_hash),
                ));
            }
            block
                .replace_signatures(signatures)
                .expect("signature replacement succeeds");

            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Npos,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, None)]);
        }

        #[test]
        fn filter_blocks_prefers_roster_metadata() {
            let kp_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let fallback_topology = Topology::new(vec![
                PeerId::new(kp_a.public_key().clone()),
                PeerId::new(kp_b.public_key().clone()),
            ]);

            let mut block: SignedBlock = unique_dummy_block(kp_b.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(4_u64));
            })
            .into();
            let block_hash = block.hash();
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(kp_b.private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(kp_a.private_key(), block_hash),
            ));
            block
                .replace_signatures(signatures)
                .expect("signature replacement succeeds");
            let roster = vec![
                PeerId::new(kp_b.public_key().clone()),
                PeerId::new(kp_a.public_key().clone()),
            ];
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            let checkpoint = ValidatorSetCheckpoint::new(
                block.header().height().get(),
                block.header().view_change_index(),
                block.hash(),
                zero_root,
                zero_root,
                roster,
                Vec::new(),
                Vec::new(),
                VALIDATOR_SET_HASH_VERSION_V1,
                None,
            );
            let mut rosters = BTreeMap::new();
            rosters.insert(
                block.hash(),
                RosterMetadata {
                    commit_qc: None,
                    validator_checkpoint: Some(checkpoint),
                    stake_snapshot: None,
                },
            );

            let state = state_with_consensus_key_pops(&[&kp_a, &kp_b]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), None)],
                &BTreeMap::new(),
                Some(&fallback_topology),
                &state,
                ConsensusMode::Permissioned,
            );
            assert_eq!(dropped, 1);
            assert!(filtered.is_empty());

            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), None)],
                &rosters,
                Some(&fallback_topology),
                &state,
                ConsensusMode::Permissioned,
            );
            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, None)]);
        }

        #[test]
        fn filter_blocks_rejects_expired_consensus_keys() {
            crate::sumeragi::status::reset_precommit_signer_history_for_tests();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(leader.public_key().clone()),
                PeerId::new(validator.public_key().clone()),
                PeerId::new(proxy.public_key().clone()),
            ]);

            let state = state_with_consensus_keys(
                &[
                    (leader.public_key().clone(), "leader"),
                    (validator.public_key().clone(), "validator"),
                    (proxy.public_key().clone(), "proxy"),
                ],
                1,
                Some(3),
                ConsensusKeyStatus::Active,
                0,
                0,
            );
            let mut expired_block = unique_dummy_block(leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(4_u64));
            });
            expired_block.sign(&validator, &topology);
            expired_block.sign(&proxy, &topology);
            let expired_block: SignedBlock = expired_block.into();

            let mut fresh_block = unique_dummy_block(leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(2_u64));
            });
            fresh_block.sign(&validator, &topology);
            fresh_block.sign(&proxy, &topology);
            let fresh_block: SignedBlock = fresh_block.into();

            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(expired_block, None), (fresh_block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 1, "expired keys should be rejected");
            assert_eq!(filtered, vec![(fresh_block, None)]);
        }

        #[test]
        fn commit_role_signers_require_proxy_tail_quorum() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            // Block only carries the leader signature; commit quorum not satisfied.
            let block: SignedBlock = unique_dummy_block(kp_leader.private_key(), |_| {}).into();

            let signers = super::Message::commit_role_signers(&block, &topology);
            assert!(
                signers.is_none(),
                "commit-role quorum must require leader + proxy-tail signatures"
            );
        }

        #[test]
        fn commit_role_signers_all_keeps_partial_set() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let block: SignedBlock = unique_dummy_block(kp_leader.private_key(), |_| {}).into();
            let signers = super::Message::commit_role_signers_all(&block, &topology);

            assert_eq!(signers.len(), 1);
            assert!(signers.contains(&ValidatorIndex::try_from(0u32).expect("index")));
        }

        #[test]
        fn commit_role_signers_accept_set_b_quorum() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_set_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
                PeerId::new(kp_proxy.public_key().clone()),
                PeerId::new(kp_set_b.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |_| {});
            block.sign(&kp_validator, &topology);
            block.sign(&kp_set_b, &topology);
            let block: SignedBlock = block.into();

            let signers = super::Message::commit_role_signers(&block, &topology)
                .expect("set B quorum should be accepted");
            assert_eq!(signers.len(), 3);
            assert!(signers.contains(&ValidatorIndex::try_from(3u32).expect("index")));
        }

        #[test]
        fn filter_blocks_replaces_qc_using_cached_signers() {
            crate::sumeragi::status::reset_block_sync_counters_for_tests();
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_proxy.public_key().clone()),
            ]);
            let mut block = unique_dummy_block(kp_leader.private_key(), |_| {});
            block.sign(&kp_proxy, &topology);
            let block: SignedBlock = block.into();
            let block_hash = block.hash();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_proxy]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();

            let commit_signers = super::Message::commit_role_signers(&block, &topology)
                .expect("commit-role quorum available");
            let aggregate_signature = aggregate_signature_for_signers(
                &chain_id,
                &mode_tag,
                Phase::Commit,
                block_hash,
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                &commit_signers,
                &topology,
                &[kp_leader.clone(), kp_proxy.clone()],
            );
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash,
                    height: block.header().height().get(),
                    view: block.header().view_change_index(),
                    epoch: 0,
                    parent_state_root: zero_root,
                    post_state_root: zero_root,
                    signers: commit_signers.clone(),
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    bls_aggregate_signature: aggregate_signature.clone(),
                    validator_set: topology.as_ref().to_vec(),
                    stake_snapshot: None,
                },
            );
            let derived_qc = BlockSynchronizer::block_sync_qc_for(
                &state_view,
                ConsensusMode::Permissioned,
                &block,
            )
            .expect("cached QC available");

            // Forge a mismatched bitmap to force replacement.
            let mut forged_qc = derived_qc.clone();
            forged_qc.aggregate.signers_bitmap = vec![0b0000_0001];

            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), Some(forged_qc))],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, Some(derived_qc))]);
        }

        #[test]
        fn filter_blocks_replaces_mismatched_qc_with_local_aggregate() {
            crate::sumeragi::status::reset_precommit_signer_history_for_tests();
            crate::sumeragi::status::reset_block_sync_counters_for_tests();
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(10_u64));
            });
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();
            let mut recorded_signers = BTreeSet::new();
            recorded_signers
                .insert(ValidatorIndex::try_from(0u32).expect("validator index parses"));
            recorded_signers
                .insert(ValidatorIndex::try_from(1u32).expect("validator index parses"));
            let aggregate_signature = aggregate_signature_for_signers(
                &chain_id,
                &mode_tag,
                Phase::Commit,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                &recorded_signers,
                &topology,
                &[kp_leader.clone(), kp_validator.clone()],
            );
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash: block.hash(),
                    height: block.header().height().get(),
                    view: block.header().view_change_index(),
                    epoch: 0,
                    parent_state_root: zero_root,
                    post_state_root: zero_root,
                    signers: recorded_signers,
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    bls_aggregate_signature: aggregate_signature.clone(),
                    validator_set: topology.as_ref().to_vec(),
                    stake_snapshot: None,
                },
            );
            assert!(
                crate::sumeragi::status::precommit_signers_for(block.hash()).is_some(),
                "precommit signer record should be visible to block sync QC builder"
            );
            let derived_qc = BlockSynchronizer::block_sync_qc_for(
                &state_view,
                ConsensusMode::Permissioned,
                &block,
            )
            .expect("derived QC should be available for valid block");

            let mut forged_qc = derived_qc.clone();
            forged_qc.aggregate.bls_aggregate_signature = vec![0xFF; 48];

            let before = crate::sumeragi::status::snapshot();
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), Some(forged_qc))],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, Some(derived_qc))]);
            let after = crate::sumeragi::status::snapshot();
            assert!(
                after.block_sync_qc_replaced_total >= before.block_sync_qc_replaced_total + 1,
                "expected block sync QC replacement counter to increase"
            );
        }

        #[test]
        fn filter_blocks_replaces_qc_with_wrong_signer_bitmap() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_extra = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
                PeerId::new(kp_proxy.public_key().clone()),
                PeerId::new(kp_extra.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(11_u64));
            });
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let state =
                state_with_consensus_key_pops(&[&kp_leader, &kp_validator, &kp_proxy, &kp_extra]);
            let state_view = state.view();
            let (chain_id, mode_tag) = test_chain_config();
            let mut recorded_signers = BTreeSet::new();
            recorded_signers
                .insert(ValidatorIndex::try_from(0u32).expect("validator index parses"));
            recorded_signers
                .insert(ValidatorIndex::try_from(1u32).expect("validator index parses"));
            recorded_signers
                .insert(ValidatorIndex::try_from(2u32).expect("validator index parses"));
            let aggregate_signature = aggregate_signature_for_signers(
                &chain_id,
                &mode_tag,
                Phase::Commit,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                &recorded_signers,
                &topology,
                &[
                    kp_leader.clone(),
                    kp_validator.clone(),
                    kp_proxy.clone(),
                    kp_extra.clone(),
                ],
            );
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash: block.hash(),
                    height: block.header().height().get(),
                    view: block.header().view_change_index(),
                    epoch: 0,
                    parent_state_root: zero_root,
                    post_state_root: zero_root,
                    signers: recorded_signers,
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    bls_aggregate_signature: aggregate_signature.clone(),
                    validator_set: topology.as_ref().to_vec(),
                    stake_snapshot: None,
                },
            );
            assert!(
                crate::sumeragi::status::precommit_signers_for(block.hash()).is_some(),
                "precommit signer record should be visible to block sync QC builder"
            );
            let derived_qc = BlockSynchronizer::block_sync_qc_for(
                &state_view,
                ConsensusMode::Permissioned,
                &block,
            )
            .expect("derived QC should be available for valid block");

            let mut forged_signers = BTreeSet::new();
            forged_signers.insert(ValidatorIndex::try_from(0u32).expect("validator index parses"));
            forged_signers.insert(ValidatorIndex::try_from(1u32).expect("validator index parses"));
            forged_signers.insert(ValidatorIndex::try_from(3u32).expect("validator index parses"));
            let forged_qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                forged_signers,
                &topology,
                &[
                    kp_leader.clone(),
                    kp_validator.clone(),
                    kp_proxy.clone(),
                    kp_extra.clone(),
                ],
            );

            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), Some(forged_qc))],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, Some(derived_qc))]);
        }

        #[test]
        fn filter_blocks_accepts_qc_without_commit_signatures() {
            crate::sumeragi::status::reset_precommit_signer_history_for_tests();
            crate::sumeragi::status::reset_block_sync_counters_for_tests();
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
            ]);

            // Block only carries the leader signature, so it fails commit validation.
            let block: SignedBlock = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(6_u64));
            })
            .into();
            let mut signers = BTreeSet::new();
            signers.insert(ValidatorIndex::try_from(0).expect("validator index parses"));
            signers.insert(ValidatorIndex::try_from(1).expect("validator index parses"));

            let (chain_id, mode_tag) = test_chain_config();
            let qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                signers,
                &topology,
                &[kp_leader.clone(), kp_validator.clone()],
            );

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), Some(qc.clone()))],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered, vec![(block, Some(qc))]);
        }

        #[test]
        fn filter_blocks_accepts_missing_proxy_tail_with_qc() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
                PeerId::new(kp_proxy.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(7_u64));
            });
            block.sign(&kp_validator, &topology);
            let block: SignedBlock = block.into();

            let mut signers = BTreeSet::new();
            signers.insert(ValidatorIndex::try_from(0u32).expect("validator index parses"));
            signers.insert(ValidatorIndex::try_from(1u32).expect("validator index parses"));
            signers.insert(ValidatorIndex::try_from(2u32).expect("validator index parses"));
            let (chain_id, mode_tag) = test_chain_config();
            let forged_qc = qc_from_signers_with_aggregate(
                &chain_id,
                &mode_tag,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                signers,
                &topology,
                &[kp_leader.clone(), kp_validator.clone(), kp_proxy.clone()],
            );

            let state = state_with_consensus_key_pops(&[&kp_leader, &kp_validator, &kp_proxy]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block, Some(forged_qc))],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered.len(), 1);
        }

        #[test]
        fn filter_blocks_retains_cached_qc_even_with_signer_mismatch() {
            let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let kp_extra = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(kp_leader.public_key().clone()),
                PeerId::new(kp_validator.public_key().clone()),
                PeerId::new(kp_proxy.public_key().clone()),
                PeerId::new(kp_extra.public_key().clone()),
            ]);

            let mut block = unique_dummy_block(kp_leader.private_key(), |header| {
                header.set_height(nonzero_ext::nonzero!(12_u64));
            });
            block.sign(&kp_validator, &topology);
            block.sign(&kp_proxy, &topology);
            let block: SignedBlock = block.into();

            let (chain_id, mode_tag) = test_chain_config();
            let mut recorded_signers = BTreeSet::new();
            recorded_signers
                .insert(ValidatorIndex::try_from(0u32).expect("validator index parses"));
            recorded_signers
                .insert(ValidatorIndex::try_from(1u32).expect("validator index parses"));
            recorded_signers
                .insert(ValidatorIndex::try_from(3u32).expect("validator index parses"));
            let aggregate_signature = aggregate_signature_for_signers(
                &chain_id,
                &mode_tag,
                Phase::Commit,
                block.hash(),
                block.header().height().get(),
                block.header().view_change_index(),
                0,
                &recorded_signers,
                &topology,
                &[
                    kp_leader.clone(),
                    kp_validator.clone(),
                    kp_proxy.clone(),
                    kp_extra.clone(),
                ],
            );
            let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
            crate::sumeragi::status::record_precommit_signers(
                crate::sumeragi::status::PrecommitSignerRecord {
                    block_hash: block.hash(),
                    height: block.header().height().get(),
                    view: block.header().view_change_index(),
                    epoch: 0,
                    parent_state_root: zero_root,
                    post_state_root: zero_root,
                    signers: recorded_signers,
                    roster_len: topology.as_ref().len(),
                    mode_tag: mode_tag.to_string(),
                    bls_aggregate_signature: aggregate_signature,
                    validator_set: topology.as_ref().to_vec(),
                    stake_snapshot: None,
                },
            );
            assert!(
                crate::sumeragi::status::precommit_signers_for(block.hash()).is_some(),
                "precommit signer record should be visible to block sync QC builder"
            );

            let state =
                state_with_consensus_key_pops(&[&kp_leader, &kp_validator, &kp_proxy, &kp_extra]);
            let (filtered, dropped) = super::Message::filter_blocks_with_valid_signatures(
                vec![(block.clone(), None)],
                &BTreeMap::new(),
                Some(&topology),
                &state,
                ConsensusMode::Permissioned,
            );

            assert_eq!(dropped, 0);
            assert_eq!(filtered.len(), 1);
            let (_, qc) = filtered
                .into_iter()
                .next()
                .expect("block should be retained");
            assert!(qc.is_some(), "cached QC should be attached for block sync");
        }
    }
}
