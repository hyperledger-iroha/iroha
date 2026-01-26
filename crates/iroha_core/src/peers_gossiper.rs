//! Peers gossiper is actor which is responsible for gossiping addresses of peers.
//!
//! E.g. peer A changes address, connects to peer B,
//! and then peer B will broadcast address of peer A to other peers.

#![allow(clippy::disallowed_types)]

#[allow(clippy::disallowed_types)]
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io::Write,
    time::Duration,
};

use iroha_config::parameters::actual::TrustedPeers;
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::peer::{Peer, PeerId};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_p2p::{Broadcast, UpdatePeers, UpdateTopology, UpdateTrustedPeers};
use iroha_primitives::{addr::SocketAddr, unique_vec::UniqueVec};
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode, core as ncore};
use tokio::sync::mpsc;

use crate::{IrohaNetwork, NetworkMessage};

/// Trust tracking entry with decay metadata.
#[derive(Debug, Clone)]
struct TrustEntry {
    score: i64,
    last_updated: std::time::Instant,
}

#[derive(Debug, Clone, Copy)]
struct TrustPenalties {
    bad_gossip: i64,
    unknown_peer: i64,
}

const PENALTY_REASON_INVALID_SIGNATURE: &str = "invalid_signature";
const PENALTY_REASON_UNKNOWN_PEER: &str = "unknown_peer";

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum TrustPenaltyKind {
    BadGossip,
    UnknownPeer,
}

impl TrustPenaltyKind {
    fn amount(self, penalties: &TrustPenalties) -> i64 {
        match self {
            Self::BadGossip => penalties.bad_gossip,
            Self::UnknownPeer => penalties.unknown_peer,
        }
    }
}

/// Deterministic trust tracker used by the peers gossiper.
#[derive(Debug, Clone)]
struct TrustBook {
    entries: BTreeMap<PeerId, TrustEntry>,
    decay_half_life: Duration,
    penalties: TrustPenalties,
    min_score: i64,
}

impl TrustBook {
    fn new(decay_half_life: Duration, penalties: TrustPenalties, min_score: i32) -> Self {
        Self {
            entries: BTreeMap::new(),
            decay_half_life,
            penalties,
            min_score: i64::from(min_score),
        }
    }

    fn seed<I: IntoIterator<Item = PeerId>>(&mut self, peers: I, now: std::time::Instant) {
        for peer in peers {
            self.entries.entry(peer).or_insert(TrustEntry {
                score: 0,
                last_updated: now,
            });
        }
    }

    fn decay_entry(
        entry: &mut TrustEntry,
        decay_half_life: std::time::Duration,
        now: std::time::Instant,
    ) -> u64 {
        if decay_half_life.is_zero() {
            entry.last_updated = now;
            return 0;
        }
        let elapsed = now.saturating_duration_since(entry.last_updated);
        let steps = elapsed.as_millis() / decay_half_life.as_millis();
        if steps == 0 {
            return 0;
        }
        let steps_u64 = u64::try_from(steps).unwrap_or(u64::MAX);
        let mut remaining_steps = steps_u64;
        while remaining_steps > 0 && entry.score != 0 {
            entry.score /= 2;
            remaining_steps -= 1;
        }
        entry.last_updated = entry
            .last_updated
            .checked_add(decay_half_life * u32::try_from(steps_u64).unwrap_or(u32::MAX))
            .unwrap_or(now);
        steps_u64
    }

    fn with_entry<R, F>(&mut self, peer: &PeerId, now: std::time::Instant, mut f: F) -> R
    where
        F: FnMut(&mut TrustEntry) -> R,
    {
        let (result, ticks) = {
            let entry = self.entries.entry(peer.clone()).or_insert(TrustEntry {
                score: 0,
                last_updated: now,
            });
            let ticks = Self::decay_entry(entry, self.decay_half_life, now);
            let result = f(entry);
            (result, ticks)
        };
        self.record_decay(peer, ticks);
        result
    }

    fn score(&mut self, peer: &PeerId, now: std::time::Instant) -> i64 {
        let score = self.with_entry(peer, now, |entry| entry.score);
        self.record_score(peer, score);
        score
    }

    fn penalize(
        &mut self,
        peer: &PeerId,
        kind: TrustPenaltyKind,
        reason_label: &str,
        now: std::time::Instant,
    ) -> i64 {
        let penalty = kind.amount(&self.penalties);
        let score = self.with_entry(peer, now, |entry| {
            entry.score = entry.score.saturating_sub(penalty);
            entry.score
        });
        self.record_score(peer, score);
        self.record_penalty(reason_label);
        score
    }

    fn min_score(&self) -> i64 {
        self.min_score
    }

    fn should_drop(&mut self, peer: &PeerId, now: std::time::Instant) -> bool {
        self.score(peer, now) <= self.min_score
    }

    fn record_score(&self, peer: &PeerId, score: i64) {
        let _ = self;
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            let peer_label = peer.to_string();
            metrics
                .p2p_trust_score
                .with_label_values(&[peer_label.as_str()])
                .set(score);
        }
    }

    fn record_decay(&self, peer: &PeerId, ticks: u64) {
        if ticks == 0 {
            return;
        }
        let _ = self;
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            let peer_label = peer.to_string();
            metrics
                .p2p_trust_decay_ticks_total
                .with_label_values(&[peer_label.as_str()])
                .inc_by(ticks);
        }
    }

    fn record_penalty(&self, reason: &str) {
        let _ = self;
        if let Some(metrics) = iroha_telemetry::metrics::global() {
            metrics
                .p2p_trust_penalties_total
                .with_label_values(&[reason])
                .inc();
        }
    }
}

/// [`PeersGossiper`] actor handle.
#[derive(Debug)]
enum GossipEvent {
    Peers { gossip: PeersGossip, from: Peer },
    Trust { gossip: PeerTrustGossip, from: Peer },
}

/// Handle to interact with the peers gossiper actor.
#[derive(Clone)]
pub struct PeersGossiperHandle {
    message_sender: mpsc::Sender<GossipEvent>,
    update_topology_sender: mpsc::UnboundedSender<UpdateTopology>,
}

impl PeersGossiperHandle {
    /// Send [`PeersGossip`] to actor.
    ///
    /// Messages are best-effort: if the queue is full, the gossip is dropped
    /// to avoid blocking consensus traffic.
    pub fn gossip(&self, gossip: PeersGossip, peer: Peer) {
        let peer_id = peer.id().clone();
        match self
            .message_sender
            .try_send(GossipEvent::Peers { gossip, from: peer })
        {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                iroha_logger::debug!(
                    peer = %peer_id,
                    "peers gossiper queue full; dropping peers gossip message"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                iroha_logger::warn!(
                    peer = %peer_id,
                    "peers gossiper channel closed; dropping peers gossip message"
                );
            }
        }
    }

    /// Send signed trust gossip to actor.
    ///
    /// Messages are best-effort: if the queue is full, the gossip is dropped
    /// to avoid blocking consensus traffic.
    pub fn gossip_trust(&self, gossip: PeerTrustGossip, peer: Peer) {
        let peer_id = peer.id().clone();
        match self
            .message_sender
            .try_send(GossipEvent::Trust { gossip, from: peer })
        {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                iroha_logger::debug!(
                    peer = %peer_id,
                    "peers gossiper queue full; dropping trust gossip message"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                iroha_logger::warn!(
                    peer = %peer_id,
                    "peers gossiper channel closed; dropping trust gossip message"
                );
            }
        }
    }

    /// Send [`UpdateTopology`] message on network actor.
    pub fn update_topology(&self, topology: UpdateTopology) {
        if let Err(err) = self.update_topology_sender.send(topology) {
            iroha_logger::warn!(?err, "Peers gossiper dropped topology update");
        }
    }

    /// Build a handle that drops all gossip messages, for unit tests that do not need a live
    /// gossiper task.
    #[cfg(test)]
    pub(crate) fn closed_for_tests() -> Self {
        let (message_sender, message_receiver) = mpsc::channel(1);
        drop(message_receiver);
        let (update_topology_sender, update_topology_receiver) = mpsc::unbounded_channel();
        drop(update_topology_receiver);
        Self {
            message_sender,
            update_topology_sender,
        }
    }
}

/// Actor which gossips peers addresses.
pub struct PeersGossiper {
    /// Id of the current peer
    peer_id: PeerId,
    /// Peers provided at startup
    initial_peers: BTreeMap<PeerId, SocketAddr>,
    /// Consensus mode to decide observer admission policy.
    consensus_mode: iroha_config::parameters::actual::ConsensusMode,
    /// Trusted peers configured locally
    trusted_peers: BTreeSet<PeerId>,
    /// Configured peers that stay trusted even if topology excludes them (e.g., observers).
    static_trusted_peers: BTreeSet<PeerId>,
    /// Peers we can re-promote once trust decays above the floor.
    trust_candidates: BTreeSet<PeerId>,
    /// Peers received via gossiping from other peers
    /// First-level key corresponds to `SocketAddr`
    /// Second-level key - peer from which such `SocketAddr` was received
    gossip_peers: BTreeMap<PeerId, BTreeMap<PeerId, SocketAddr>>,
    current_topology: BTreeSet<PeerId>,
    key_pair: KeyPair,
    gossip_period: Duration,
    gossip_max_period: Duration,
    gossip_backoff: Duration,
    gossip_next_deadline: std::time::Instant,
    gossip_pending: bool,
    last_drop_count: u64,
    last_drop_at: Option<std::time::Instant>,
    trust: TrustBook,
    network: IrohaNetwork,
}

struct TrustGossipOutcome {
    newly_trusted: BTreeSet<PeerId>,
    drop_sender: bool,
}

/// Terminology:
/// * Topology - public keys of current network derived from blockchain (Register/Unregister Peer Isi)
/// * Peers addresses - currently known addresses for peers in topology. Might be unknown for some peer.
///
/// There are three sources of peers addresses:
/// 1. Provided at iroha startup (`TRUSTED_PEERS` env var)
/// 2. Currently connected online peers.
///    Some peer might change address and connect to our peer,
///    such connection will be accepted if peer public key is in topology.
/// 3. Received via gossiping from other peers.
impl PeersGossiper {
    /// Determine the gossip interval using the configured value.
    fn gossip_interval(configured: Duration) -> Duration {
        configured
    }

    fn next_gossip_backoff(
        current: Duration,
        min: Duration,
        max: Duration,
        had_pending: bool,
    ) -> Duration {
        if had_pending {
            return min;
        }
        current.saturating_mul(2).min(max)
    }

    fn note_gossip_change(&mut self, now: std::time::Instant) {
        self.gossip_pending = true;
        self.gossip_backoff = self.gossip_period;
        let next = now.checked_add(self.gossip_period).unwrap_or(now);
        if next < self.gossip_next_deadline {
            self.gossip_next_deadline = next;
        }
    }

    fn drop_backpressure_active(&mut self, now: std::time::Instant) -> bool {
        let current = iroha_p2p::network::subscriber_queue_full_count();
        if current > self.last_drop_count {
            self.last_drop_count = current;
            self.last_drop_at = Some(now);
        }
        self.last_drop_at
            .is_some_and(|last| now.saturating_duration_since(last) < self.gossip_backoff)
    }

    fn maybe_gossip(&mut self, now: std::time::Instant) {
        if now < self.gossip_next_deadline {
            return;
        }
        if self.drop_backpressure_active(now) {
            self.gossip_next_deadline = now.checked_add(self.gossip_backoff).unwrap_or(now);
            iroha_logger::trace!(
                drops = self.last_drop_count,
                cooldown_ms = self.gossip_backoff.as_millis(),
                "peers gossiper skipping gossip due to relay backpressure"
            );
            return;
        }
        self.gossip_peers();
        self.network_update_peers_addresses();
        let had_pending = self.gossip_pending;
        self.gossip_pending = false;
        self.gossip_backoff = Self::next_gossip_backoff(
            self.gossip_backoff,
            self.gossip_period,
            self.gossip_max_period,
            had_pending,
        );
        self.gossip_next_deadline = now.checked_add(self.gossip_backoff).unwrap_or(now);
    }
    /// Start actor.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        peer_id: PeerId,
        trusted_peers: TrustedPeers,
        key_pair: KeyPair,
        gossip_period: Duration,
        gossip_max_period: Duration,
        consensus_mode: iroha_config::parameters::actual::ConsensusMode,
        trust_decay_half_life: Duration,
        trust_penalty_bad_gossip: i32,
        trust_penalty_unknown_peer: i32,
        trust_min_score: i32,
        network: IrohaNetwork,
        shutdown_signal: ShutdownSignal,
    ) -> (PeersGossiperHandle, Child) {
        let trusted_list = trusted_peers.others;
        let initial_peers = trusted_list
            .iter()
            .map(|peer| (peer.id().clone(), peer.address().clone()))
            .collect();
        let mut trusted_set: BTreeSet<_> =
            trusted_list.iter().map(|peer| peer.id().clone()).collect();
        trusted_set.insert(peer_id.clone());
        let static_trusted_peers = trusted_set.clone();
        let initial_topology: BTreeSet<_> = trusted_set.clone();
        let trust_candidates = trusted_set.clone();
        let gossip_max_period = gossip_max_period.max(gossip_period);
        let now = std::time::Instant::now();
        let mut gossiper = Self {
            peer_id,
            initial_peers,
            consensus_mode,
            trusted_peers: trusted_set,
            static_trusted_peers,
            trust_candidates,
            gossip_peers: BTreeMap::new(),
            current_topology: initial_topology.clone(),
            key_pair,
            gossip_period,
            gossip_max_period,
            gossip_backoff: gossip_period,
            gossip_next_deadline: now,
            gossip_pending: true,
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            trust: TrustBook::new(
                trust_decay_half_life,
                TrustPenalties {
                    bad_gossip: i64::from(trust_penalty_bad_gossip),
                    unknown_peer: i64::from(trust_penalty_unknown_peer),
                },
                trust_min_score,
            ),
            network,
        };
        // Seed the network with the initial topology so peers start dialing immediately.
        if !initial_topology.is_empty() {
            let topology_update: HashSet<_> =
                initial_topology.iter().cloned().collect::<HashSet<_>>();
            gossiper
                .network
                .update_topology(UpdateTopology(topology_update));
        }
        gossiper
            .trust
            .seed(gossiper.trusted_peers.clone(), std::time::Instant::now());
        gossiper.network_update_peers_addresses();
        gossiper.update_trusted_peers_on_network();

        let (message_sender, message_receiver) = mpsc::channel(1);
        let (update_topology_sender, update_topology_receiver) = mpsc::unbounded_channel();
        (
            PeersGossiperHandle {
                message_sender,
                update_topology_sender,
            },
            Child::new(
                tokio::task::spawn(gossiper.run(
                    message_receiver,
                    update_topology_receiver,
                    shutdown_signal,
                )),
                OnShutdown::Abort,
            ),
        )
    }

    async fn run(
        mut self,
        mut message_receiver: mpsc::Receiver<GossipEvent>,
        mut update_topology_receiver: mpsc::UnboundedReceiver<UpdateTopology>,
        shutdown_signal: ShutdownSignal,
    ) {
        let mut gossip_period = tokio::time::interval(Self::gossip_interval(self.gossip_period));
        loop {
            let should_check_gossip = tokio::select! {
                Some(update_topology) = update_topology_receiver.recv() => {
                    let changed = self.set_current_topology(update_topology);
                    if changed {
                        self.note_gossip_change(std::time::Instant::now());
                    }
                    true
                }
                _ = gossip_period.tick() => {
                    true
                }
                result = self.network.wait_online_peers_update(|_| ()) => {
                    match result {
                        Ok(()) => {
                            self.network_update_peers_addresses();
                            self.note_gossip_change(std::time::Instant::now());
                            true
                        }
                        Err(err) => {
                            iroha_logger::debug!(?err, "Network online peers channel closed");
                            break;
                        }
                    }
                }
                Some(event) = message_receiver.recv() => {
                    match event {
                        GossipEvent::Peers { gossip, from } => {
                            if self.handle_peers_gossip(gossip, &from) {
                                self.note_gossip_change(std::time::Instant::now());
                            }
                        }
                        GossipEvent::Trust { gossip, from } => {
                            if self.handle_trust_gossip(gossip, &from) {
                                self.note_gossip_change(std::time::Instant::now());
                            }
                        }
                    }
                    true
                }
                () = shutdown_signal.receive() => {
                    iroha_logger::debug!("Shutting down peers gossiper");
                    break;
                },
            };
            if should_check_gossip {
                self.maybe_gossip(std::time::Instant::now());
            }
            tokio::task::yield_now().await;
        }
    }

    fn set_current_topology(&mut self, UpdateTopology(topology): UpdateTopology) -> bool {
        let mut new_topology: BTreeSet<_> = topology.into_iter().collect();
        new_topology.extend(self.static_trusted_peers.iter().cloned());

        self.gossip_peers.retain(|peer, map| {
            if !new_topology.contains(peer) {
                return false;
            }

            map.retain(|peer, _| new_topology.contains(peer));
            !map.is_empty()
        });

        let removed: Vec<_> = self
            .current_topology
            .difference(&new_topology)
            .cloned()
            .collect();
        let unchanged = new_topology == self.current_topology;

        if !removed.is_empty() {
            let mut trust_changed = false;
            for peer in removed {
                if self.static_trusted_peers.contains(&peer) {
                    continue;
                }
                trust_changed |= self.trusted_peers.remove(&peer);
                self.trust_candidates.remove(&peer);
                self.trust.entries.remove(&peer);
            }
            if trust_changed {
                self.update_trusted_peers_on_network();
            }
        }

        self.current_topology.clone_from(&new_topology);
        // Keep the network dial set in sync with the current topology so removed peers
        // are disconnected promptly and new peers are contacted.
        self.network
            .update_topology(UpdateTopology(new_topology.iter().cloned().collect()));
        self.network_update_peers_addresses();
        if unchanged {
            iroha_logger::debug!(
                topology_len = self.current_topology.len(),
                "received unchanged topology; re-advertising to enforce dial set"
            );
        }
        !unchanged
    }

    fn gossip_peers(&mut self) {
        let online_peers = self.network.online_peers(Clone::clone);
        let online_peers = UniqueVec::from_iter(online_peers);
        let now = std::time::Instant::now();
        let peers_msg = NetworkMessage::PeersGossiper(Box::new(PeersGossip {
            peers: online_peers,
        }));
        self.network.broadcast(Broadcast {
            data: peers_msg,
            priority: iroha_p2p::Priority::Low,
        });

        let trust = self.sign_trust_entries(now);
        if !trust.is_empty() {
            let trust_msg = NetworkMessage::PeerTrustGossip(Box::new(PeerTrustGossip { trust }));
            self.network.broadcast(Broadcast {
                data: trust_msg,
                priority: iroha_p2p::Priority::Low,
            });
        }
    }

    fn trust_payload(info: &PeerTrustInfo) -> Vec<u8> {
        let mut payload = Vec::from(b"iroha-peer-trust-v1".as_slice());
        payload.extend_from_slice(&info.peer_id.encode());
        payload.extend_from_slice(&info.trusted.encode());
        payload.extend_from_slice(&info.score.encode());
        payload
    }

    fn sign_trust_entries(&mut self, now: std::time::Instant) -> Vec<SignedPeerTrust> {
        self.trusted_peers
            .iter()
            .map(|peer_id| {
                let score = self.trust.score(peer_id, now);
                let clamped = i32::try_from(score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
                    .expect("clamped trust score must fit in i32");
                PeerTrustInfo {
                    peer_id: peer_id.clone(),
                    trusted: score > self.trust.min_score,
                    score: clamped,
                }
            })
            .map(|info| {
                let signature =
                    Signature::new(self.key_pair.private_key(), &Self::trust_payload(&info));
                SignedPeerTrust {
                    info,
                    signature: signature.payload().to_vec(),
                }
            })
            .collect()
    }

    fn evict_if_low_trust(&mut self, peer_id: &PeerId, now: std::time::Instant) -> bool {
        if self.trust.should_drop(peer_id, now) {
            if self.trusted_peers.remove(peer_id) {
                self.update_trusted_peers_on_network();
            }
            iroha_logger::warn!(peer=%peer_id, "Dropping gossip from low-trust peer");
            true
        } else {
            false
        }
    }

    fn restore_if_recovered(&mut self, peer_id: &PeerId, now: std::time::Instant) {
        if self.trust_candidates.contains(peer_id)
            && !self.trusted_peers.contains(peer_id)
            && self.trust.score(peer_id, now) > self.trust.min_score
        {
            iroha_logger::debug!(peer=%peer_id, "Reinstating peer after trust decay recovery");
            self.trusted_peers.insert(peer_id.clone());
            self.update_trusted_peers_on_network();
        }
    }

    fn handle_peers_gossip(
        &mut self,
        PeersGossip { peers }: PeersGossip,
        from_peer: &Peer,
    ) -> bool {
        let is_trusted = self.trusted_peers.contains(from_peer.id());
        let allow_public = matches!(
            self.consensus_mode,
            iroha_config::parameters::actual::ConsensusMode::Npos
        );
        if !allow_public && !self.current_topology.contains(from_peer.id()) && !is_trusted {
            return false;
        }
        let now = std::time::Instant::now();
        if self.evict_if_low_trust(from_peer.id(), now) {
            return false;
        }
        self.restore_if_recovered(from_peer.id(), now);
        let mut changed = false;
        for peer in peers {
            let peer_allowed = self.current_topology.contains(peer.id())
                || allow_public
                || self.trusted_peers.contains(peer.id());
            if !peer_allowed {
                // Ignore gossip about peers outside the active topology.
                continue;
            }
            let map = self.gossip_peers.entry(peer.id().clone()).or_default();
            let address = peer.address().clone();
            let previous = map.insert(from_peer.id().clone(), address.clone());
            if previous.as_ref() != Some(&address) {
                changed = true;
            }
        }
        if changed {
            self.network_update_peers_addresses();
        }
        changed
    }

    fn handle_trust_gossip(
        &mut self,
        PeerTrustGossip { trust }: PeerTrustGossip,
        from_peer: &Peer,
    ) -> bool {
        let allow_public = matches!(
            self.consensus_mode,
            iroha_config::parameters::actual::ConsensusMode::Npos
        );
        if !self.trusted_peers.contains(from_peer.id())
            && !allow_public
            && !self.current_topology.contains(from_peer.id())
        {
            return false;
        }
        let now = std::time::Instant::now();
        if self.evict_if_low_trust(from_peer.id(), now) {
            return false;
        }
        self.restore_if_recovered(from_peer.id(), now);

        let outcome = process_trust_records(
            trust,
            from_peer,
            allow_public,
            &self.current_topology,
            &self.trusted_peers,
            &mut self.trust,
            now,
        );
        if outcome.drop_sender && self.evict_if_low_trust(from_peer.id(), now) {
            return false;
        }
        let newly_trusted = outcome.newly_trusted;
        let trust_changed = !newly_trusted.is_empty();
        if trust_changed {
            self.trusted_peers.extend(newly_trusted.iter().cloned());
            self.trust_candidates.extend(newly_trusted);
        }
        if trust_changed {
            self.update_trusted_peers_on_network();
        }
        trust_changed
    }

    fn network_update_peers_addresses(&self) {
        let mut peers = Vec::new();
        for (id, address) in &self.initial_peers {
            peers.push((id.clone(), address.clone()));
        }
        for (id, addresses) in &self.gossip_peers {
            peers.push((id.clone(), choose_address_majority_rule(addresses)));
        }

        // Always send the full set of known addresses; the network layer will
        // avoid redundant dials to already-connected peers.
        let update = UpdatePeers(
            peers
                .into_iter()
                .filter(|(id, _)| id != &self.peer_id && self.current_topology.contains(id))
                .collect(),
        );
        self.network.update_peers_addresses(update);
    }

    fn update_trusted_peers_on_network(&self) {
        self.network.update_trusted_peers(UpdateTrustedPeers(
            self.trusted_peers.clone().into_iter().collect(),
        ));
    }
}

fn process_trust_records(
    trust: Vec<SignedPeerTrust>,
    from_peer: &Peer,
    allow_public: bool,
    current_topology: &BTreeSet<PeerId>,
    trusted_peers: &BTreeSet<PeerId>,
    trust_book: &mut TrustBook,
    now: std::time::Instant,
) -> TrustGossipOutcome {
    let mut newly_trusted = BTreeSet::new();
    for SignedPeerTrust { info, signature } in trust {
        let sig = Signature::from_bytes(&signature);
        let payload = PeersGossiper::trust_payload(&info);
        if sig.verify(from_peer.id().public_key(), &payload).is_err() {
            iroha_logger::warn!(peer=%from_peer, "Rejected trust gossip with invalid signature");
            let score = trust_book.penalize(
                from_peer.id(),
                TrustPenaltyKind::BadGossip,
                PENALTY_REASON_INVALID_SIGNATURE,
                now,
            );
            if score <= trust_book.min_score() {
                return TrustGossipOutcome {
                    newly_trusted,
                    drop_sender: true,
                };
            }
            continue;
        }
        let peer_allowed = current_topology.contains(&info.peer_id)
            || allow_public
            || trusted_peers.contains(&info.peer_id);
        if !peer_allowed {
            if !allow_public {
                iroha_logger::warn!(
                    peer=%from_peer,
                    advertised_peer=%info.peer_id,
                    "Penalizing trust gossip about peer outside the active topology"
                );
                let score = trust_book.penalize(
                    from_peer.id(),
                    TrustPenaltyKind::UnknownPeer,
                    PENALTY_REASON_UNKNOWN_PEER,
                    now,
                );
                if score <= trust_book.min_score() {
                    return TrustGossipOutcome {
                        newly_trusted,
                        drop_sender: true,
                    };
                }
            }
            continue;
        }
        if info.trusted {
            newly_trusted.insert(info.peer_id.clone());
        }
    }
    TrustGossipOutcome {
        newly_trusted,
        drop_sender: false,
    }
}

fn choose_address_majority_rule(addresses: &BTreeMap<PeerId, SocketAddr>) -> SocketAddr {
    let mut count_map = BTreeMap::new();
    for address in addresses.values() {
        *count_map.entry(address).or_insert(0) += 1;
    }
    count_map
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(address, _)| address)
        .expect("There must be no empty inner map in addresses")
        .clone()
}

/// Message for gossiping peers addresses.
#[derive(Debug, Clone)]
pub struct PeersGossip {
    /// Peers known to the sender, deduplicated but encoded in insertion order.
    pub peers: UniqueVec<Peer>,
}

/// Signed trust gossip payload.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub struct PeerTrustGossip {
    /// Signed trust reports emitted by the sender.
    pub trust: Vec<SignedPeerTrust>,
}

/// Trust information about a peer as reported by the sender.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub struct PeerTrustInfo {
    /// Id of the peer the trust info is about.
    pub peer_id: PeerId,
    /// Whether the sender currently considers the peer trusted.
    pub trusted: bool,
    /// Trust score recorded by the sender.
    pub score: i32,
}

/// Trust report bundled with a signature from the sender.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub struct SignedPeerTrust {
    /// Reported trust values.
    pub info: PeerTrustInfo,
    /// Signature proving authenticity of the trust record.
    pub signature: Vec<u8>,
}

impl NoritoSerialize for PeersGossip {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), ncore::Error> {
        // Serialize peers as Vec to preserve insertion order.
        let vec: Vec<Peer> = self.peers.iter().map(Clone::clone).collect();
        vec.serialize(writer)
    }
}

impl<'a> NoritoDeserialize<'a> for PeersGossip {
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        let peers: Vec<Peer> = NoritoDeserialize::deserialize(archived.cast());
        Self {
            peers: UniqueVec::from_iter(peers),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet, HashSet},
        time::Instant,
    };

    use iroha_config::{
        base::WithOrigin,
        parameters::actual::{
            ConsensusMode, LaneProfile, Network as NetworkConfig, RelayMode, SoranetHandshake,
            SoranetPrivacy, SoranetVpn, TrustedPeers as TrustedPeersConfig,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::ChainId;
    use iroha_futures::supervisor::ShutdownSignal;

    use super::*;
    #[test]
    fn peers_gossip_roundtrip() {
        // Use seeded keypairs to produce valid Ed25519 public keys deterministically.
        let kp1 = KeyPair::from_seed(vec![1, 2, 3, 4], Algorithm::Ed25519);
        let kp2 = KeyPair::from_seed(vec![5, 6, 7, 8], Algorithm::Ed25519);

        let peer1 = Peer::new("127.0.0.1:8080".parse().unwrap(), kp1.public_key().clone());
        let peer2 = Peer::new("127.0.0.1:8081".parse().unwrap(), kp2.public_key().clone());

        let gossip = PeersGossip {
            peers: UniqueVec::from_iter(vec![peer1.clone(), peer2.clone()]),
        };
        let bytes = ncore::to_bytes(&gossip).expect("serialize gossip");
        let decoded: PeersGossip = ncore::decode_from_bytes(&bytes).expect("decode gossip");

        let decoded_peers: Vec<_> = decoded.peers.into_iter().collect();
        assert_eq!(decoded_peers, vec![peer1.clone(), peer2.clone()]);
    }

    #[test]
    fn peer_trust_gossip_roundtrip() {
        let kp = KeyPair::from_seed(vec![9, 10, 11, 12], Algorithm::Ed25519);
        let peer = Peer::new("127.0.0.1:8082".parse().unwrap(), kp.public_key().clone());
        let info = PeerTrustInfo {
            peer_id: peer.id().clone(),
            trusted: true,
            score: 2,
        };
        let sig = Signature::new(kp.private_key(), &PeersGossiper::trust_payload(&info))
            .payload()
            .to_vec();
        let gossip = PeerTrustGossip {
            trust: vec![SignedPeerTrust {
                info: info.clone(),
                signature: sig.clone(),
            }],
        };
        let bytes = ncore::to_bytes(&gossip).expect("serialize trust gossip");
        let decoded: PeerTrustGossip =
            ncore::decode_from_bytes(&bytes).expect("decode trust gossip");

        assert_eq!(decoded.trust.len(), 1);
        assert_eq!(decoded.trust[0].info.peer_id, info.peer_id);
        assert_eq!(decoded.trust[0].signature, sig);
    }

    #[test]
    fn trust_book_penalty_and_decay() {
        let mut trust = TrustBook::new(
            Duration::from_millis(200),
            TrustPenalties {
                bad_gossip: 4,
                unknown_peer: 3,
            },
            -4,
        );
        let kp = KeyPair::from_seed(vec![13, 14, 15, 16], Algorithm::Ed25519);
        let peer = Peer::new("127.0.0.1:8083".parse().unwrap(), kp.public_key().clone());
        let peer_id = peer.id().clone();
        let now = Instant::now();
        trust.seed([peer_id.clone()], now);

        assert_eq!(trust.score(&peer_id, now), 0);

        let after_penalty =
            trust.penalize(&peer_id, TrustPenaltyKind::BadGossip, "bad_gossip", now);
        assert_eq!(after_penalty, -4);
        assert!(trust.should_drop(&peer_id, now));

        let later = now + Duration::from_millis(200);
        assert_eq!(trust.score(&peer_id, later), -2);

        let drop_score = trust.penalize(&peer_id, TrustPenaltyKind::BadGossip, "bad_gossip", later);
        assert_eq!(drop_score, -6);
        let later_again = later + Duration::from_millis(400);
        assert_eq!(trust.score(&peer_id, later_again), -1);
        assert!(!trust.should_drop(&peer_id, later_again));

        let unknown = trust.penalize(
            &peer_id,
            TrustPenaltyKind::UnknownPeer,
            PENALTY_REASON_UNKNOWN_PEER,
            later_again,
        );
        assert_eq!(unknown, -4);
        assert!(trust.should_drop(&peer_id, later_again));
    }

    #[test]
    fn gossip_interval_respects_config_value() {
        let configured = Duration::from_millis(250);
        assert_eq!(PeersGossiper::gossip_interval(configured), configured);
    }

    #[test]
    fn gossip_backoff_doubles_and_resets() {
        let min = Duration::from_millis(100);
        let max = Duration::from_millis(800);
        let backoff = PeersGossiper::next_gossip_backoff(min, min, max, false);
        assert_eq!(backoff, Duration::from_millis(200));
        let backoff = PeersGossiper::next_gossip_backoff(backoff, min, max, false);
        assert_eq!(backoff, Duration::from_millis(400));
        let backoff = PeersGossiper::next_gossip_backoff(backoff, min, max, false);
        assert_eq!(backoff, Duration::from_millis(800));
        let reset = PeersGossiper::next_gossip_backoff(backoff, min, max, true);
        assert_eq!(reset, min);
    }

    #[test]
    fn gossiper_handle_drops_messages_when_receiver_closed() {
        let handle = PeersGossiperHandle::closed_for_tests();
        let kp = KeyPair::from_seed(vec![99, 98, 97, 96], Algorithm::Ed25519);
        let peer = Peer::new(
            "127.0.0.1:9999".parse().expect("addr"),
            kp.public_key().clone(),
        );

        handle.gossip(
            PeersGossip {
                peers: UniqueVec::from_iter(vec![peer.clone()]),
            },
            peer.clone(),
        );
        handle.gossip_trust(PeerTrustGossip { trust: Vec::new() }, peer);
        handle.update_topology(UpdateTopology(HashSet::new()));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn topology_update_preserves_static_trusted_peers() {
        let shutdown = ShutdownSignal::new();
        let listen_addr: SocketAddr = "127.0.0.1:0".parse().expect("addr");
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::from(key_pair.public_key().clone());

        let network_cfg = NetworkConfig {
            address: WithOrigin::inline(listen_addr.clone()),
            public_address: WithOrigin::inline(listen_addr.clone()),
            relay_mode: RelayMode::Disabled,
            relay_hub_address: None,
            relay_ttl: iroha_config::parameters::defaults::network::RELAY_TTL,
            soranet_handshake: SoranetHandshake::default(),
            soranet_privacy: SoranetPrivacy::default(),
            soranet_vpn: SoranetVpn::default(),
            lane_profile: LaneProfile::Core,
            require_sm_handshake_match:
                iroha_config::parameters::defaults::network::REQUIRE_SM_HANDSHAKE_MATCH,
            require_sm_openssl_preview_match:
                iroha_config::parameters::defaults::network::REQUIRE_SM_OPENSSL_PREVIEW_MATCH,
            idle_timeout: iroha_config::parameters::defaults::network::IDLE_TIMEOUT,
            connect_startup_delay:
                iroha_config::parameters::defaults::network::CONNECT_STARTUP_DELAY,
            peer_gossip_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            peer_gossip_max_period: iroha_config::parameters::defaults::network::PEER_GOSSIP_PERIOD,
            trust_gossip: iroha_config::parameters::defaults::network::TRUST_GOSSIP,
            trust_decay_half_life:
                iroha_config::parameters::defaults::network::TRUST_DECAY_HALF_LIFE,
            trust_penalty_bad_gossip:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_BAD_GOSSIP,
            trust_penalty_unknown_peer:
                iroha_config::parameters::defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
            trust_min_score: iroha_config::parameters::defaults::network::TRUST_MIN_SCORE,
            dns_refresh_interval: None,
            dns_refresh_ttl: None,
            quic_enabled: false,
            tls_enabled: false,
            tls_listen_address: None,
            prefer_ws_fallback: false,
            p2p_queue_cap_high: iroha_config::parameters::defaults::network::P2P_QUEUE_CAP_HIGH,
            p2p_queue_cap_low: iroha_config::parameters::defaults::network::P2P_QUEUE_CAP_LOW,
            p2p_post_queue_cap: iroha_config::parameters::defaults::network::P2P_POST_QUEUE_CAP,
            p2p_subscriber_queue_cap:
                iroha_config::parameters::defaults::network::P2P_SUBSCRIBER_QUEUE_CAP,
            consensus_ingress_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RATE_PER_SEC,
            consensus_ingress_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BURST,
            consensus_ingress_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_PER_SEC,
            consensus_ingress_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_BYTES_BURST,
            consensus_ingress_critical_rate_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_RATE_PER_SEC,
            consensus_ingress_critical_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BURST,
            consensus_ingress_critical_bytes_per_sec:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_PER_SEC,
            consensus_ingress_critical_bytes_burst:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_CRITICAL_BYTES_BURST,
            consensus_ingress_rbc_session_limit:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_RBC_SESSION_LIMIT,
            consensus_ingress_penalty_threshold:
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_THRESHOLD,
            consensus_ingress_penalty_window: Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_WINDOW_MS,
            ),
            consensus_ingress_penalty_cooldown: Duration::from_millis(
                iroha_config::parameters::defaults::network::CONSENSUS_INGRESS_PENALTY_COOLDOWN_MS,
            ),
            happy_eyeballs_stagger:
                iroha_config::parameters::defaults::network::HAPPY_EYEBALLS_STAGGER,
            addr_ipv6_first: false,
            max_incoming: None,
            max_total_connections: None,
            accept_rate_per_ip_per_sec: None,
            accept_burst_per_ip: None,
            max_accept_buckets: iroha_config::parameters::defaults::network::MAX_ACCEPT_BUCKETS,
            accept_bucket_idle: iroha_config::parameters::defaults::network::ACCEPT_BUCKET_IDLE,
            accept_prefix_v4_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V4_BITS,
            accept_prefix_v6_bits:
                iroha_config::parameters::defaults::network::ACCEPT_PREFIX_V6_BITS,
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
            disconnect_on_post_overflow:
                iroha_config::parameters::defaults::network::DISCONNECT_ON_POST_OVERFLOW,
            max_frame_bytes: iroha_config::parameters::defaults::network::MAX_FRAME_BYTES.get(),
            tcp_nodelay: iroha_config::parameters::defaults::network::TCP_NODELAY,
            tcp_keepalive: Some(iroha_config::parameters::defaults::network::TCP_KEEPALIVE),
            max_frame_bytes_consensus:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get(),
            max_frame_bytes_control:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
            max_frame_bytes_block_sync:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC.get(),
            max_frame_bytes_tx_gossip:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_TX_GOSSIP.get(),
            max_frame_bytes_peer_gossip:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_PEER_GOSSIP.get(),
            max_frame_bytes_health:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_HEALTH.get(),
            max_frame_bytes_other:
                iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_OTHER.get(),
            tls_only_v1_3: true,
            quic_max_idle_timeout: None,
        };

        let (network, _child) = IrohaNetwork::start(
            key_pair.clone(),
            network_cfg.clone(),
            Some(ChainId::from("gossiper-topology-test")),
            None,
            None,
            shutdown.clone(),
        )
        .await
        .expect("network starts");

        let local_peer = Peer::new(listen_addr, peer_id.clone());
        let observer_kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let observer_peer = Peer::new(
            "127.0.0.1:9101".parse().expect("addr"),
            observer_kp.public_key().clone(),
        );
        let trusted_peers = TrustedPeersConfig {
            myself: local_peer,
            others: UniqueVec::from_iter(vec![observer_peer.clone()]),
            pops: BTreeMap::new(),
        };

        let trusted_list = trusted_peers.others.clone();
        let initial_peers = trusted_list
            .iter()
            .map(|peer| (peer.id().clone(), peer.address().clone()))
            .collect();
        let trusted_set: BTreeSet<_> = trusted_list.iter().map(|peer| peer.id().clone()).collect();
        let current_topology = trusted_set.clone();
        let trust_candidates = trusted_set.clone();
        let mut gossiper = PeersGossiper {
            peer_id,
            initial_peers,
            consensus_mode: ConsensusMode::Permissioned,
            trusted_peers: trusted_set.clone(),
            static_trusted_peers: trusted_set.clone(),
            trust_candidates,
            gossip_peers: BTreeMap::new(),
            current_topology,
            key_pair: key_pair.clone(),
            gossip_period: network_cfg.peer_gossip_period,
            gossip_max_period: network_cfg.peer_gossip_max_period,
            gossip_backoff: network_cfg.peer_gossip_period,
            gossip_next_deadline: std::time::Instant::now(),
            gossip_pending: true,
            last_drop_count: iroha_p2p::network::subscriber_queue_full_count(),
            last_drop_at: None,
            trust: TrustBook::new(
                network_cfg.trust_decay_half_life,
                TrustPenalties {
                    bad_gossip: i64::from(network_cfg.trust_penalty_bad_gossip),
                    unknown_peer: i64::from(network_cfg.trust_penalty_unknown_peer),
                },
                network_cfg.trust_min_score,
            ),
            network: network.clone(),
        };

        let dynamic_kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let dynamic_peer_id = PeerId::from(dynamic_kp.public_key().clone());
        gossiper.trusted_peers.insert(dynamic_peer_id.clone());
        gossiper.trust_candidates.insert(dynamic_peer_id.clone());
        gossiper.current_topology.insert(dynamic_peer_id.clone());
        gossiper.trust.entries.insert(
            dynamic_peer_id.clone(),
            TrustEntry {
                score: 0,
                last_updated: Instant::now(),
            },
        );

        gossiper.set_current_topology(UpdateTopology(HashSet::new()));

        assert!(
            gossiper.trusted_peers.contains(observer_peer.id()),
            "configured observer should remain trusted even if excluded from topology"
        );
        assert!(
            gossiper.current_topology.contains(observer_peer.id()),
            "static trusted peers should remain in the p2p topology"
        );
        assert!(
            !gossiper.trusted_peers.contains(&dynamic_peer_id),
            "non-static peer should be dropped from trust when removed from topology"
        );
        assert!(
            !gossiper.current_topology.contains(&dynamic_peer_id),
            "dynamic peers removed from topology should not remain in the p2p topology"
        );
        assert!(
            gossiper.trust_candidates.contains(observer_peer.id()),
            "configured observer should remain a trust candidate"
        );
        assert!(
            !gossiper.trust_candidates.contains(&dynamic_peer_id),
            "removed dynamic peer should not remain a trust candidate"
        );

        shutdown.send();
    }

    #[test]
    fn trust_gossip_penalizes_unknown_peer_in_permissioned_mode() {
        let kp_sender = KeyPair::from_seed(vec![17, 18, 19, 20], Algorithm::Ed25519);
        let kp_unknown = KeyPair::from_seed(vec![21, 22, 23, 24], Algorithm::Ed25519);
        let from_peer = Peer::new(
            "127.0.0.1:9000".parse().expect("addr"),
            kp_sender.public_key().clone(),
        );
        let unknown_peer = Peer::new(
            "127.0.0.1:9001".parse().expect("addr"),
            kp_unknown.public_key().clone(),
        );

        let info = PeerTrustInfo {
            peer_id: unknown_peer.id().clone(),
            trusted: true,
            score: 1,
        };
        let signature = Signature::new(
            kp_sender.private_key(),
            &PeersGossiper::trust_payload(&info),
        )
        .payload()
        .to_vec();
        let trust = vec![SignedPeerTrust { info, signature }];
        let now = Instant::now();
        let mut trust_book = TrustBook::new(
            Duration::from_millis(0),
            TrustPenalties {
                bad_gossip: 4,
                unknown_peer: 3,
            },
            -2,
        );
        trust_book.seed([from_peer.id().clone()], now);

        let outcome = process_trust_records(
            trust,
            &from_peer,
            false,
            &BTreeSet::new(),
            &BTreeSet::from([from_peer.id().clone()]),
            &mut trust_book,
            now,
        );

        assert!(
            outcome.drop_sender,
            "sender should be dropped after unknown peer penalty"
        );
        assert!(outcome.newly_trusted.is_empty());
        assert!(trust_book.score(from_peer.id(), now) <= -2);
    }

    #[test]
    fn unknown_peer_penalty_evicts_and_recovers_after_decay() {
        let kp_sender = KeyPair::from_seed(vec![33, 34, 35, 36], Algorithm::Ed25519);
        let kp_unknown = KeyPair::from_seed(vec![37, 38, 39, 40], Algorithm::Ed25519);
        let from_peer = Peer::new(
            "127.0.0.1:9200".parse().expect("addr"),
            kp_sender.public_key().clone(),
        );
        let unknown_peer = Peer::new(
            "127.0.0.1:9201".parse().expect("addr"),
            kp_unknown.public_key().clone(),
        );

        let info = PeerTrustInfo {
            peer_id: unknown_peer.id().clone(),
            trusted: true,
            score: 1,
        };
        let signature = Signature::new(
            kp_sender.private_key(),
            &PeersGossiper::trust_payload(&info),
        )
        .payload()
        .to_vec();
        let trust = vec![SignedPeerTrust { info, signature }];
        let now = Instant::now();
        let mut trust_book = TrustBook::new(
            Duration::from_millis(100),
            TrustPenalties {
                bad_gossip: 4,
                unknown_peer: 3,
            },
            -2,
        );
        trust_book.seed([from_peer.id().clone()], now);

        let outcome = process_trust_records(
            trust,
            &from_peer,
            false,
            &BTreeSet::new(),
            &BTreeSet::from([from_peer.id().clone()]),
            &mut trust_book,
            now,
        );

        assert!(outcome.drop_sender);
        assert!(trust_book.should_drop(from_peer.id(), now));

        // After decay, the sender can clear the floor and be reinstated via the recovery path.
        let recovered_at = now + Duration::from_millis(300);
        let recovered_score = trust_book.score(from_peer.id(), recovered_at);
        assert!(
            recovered_score > trust_book.min_score(),
            "decay should allow recovery after penalties"
        );

        let mut trusted_peers = BTreeSet::new();
        let trust_candidates = BTreeSet::from([from_peer.id().clone()]);
        if trust_candidates.contains(from_peer.id())
            && trust_book.score(from_peer.id(), recovered_at) > trust_book.min_score()
        {
            trusted_peers.insert(from_peer.id().clone());
        }

        assert!(
            trusted_peers.contains(from_peer.id()),
            "peer should be eligible for reinstatement after decay"
        );
    }

    #[test]
    fn trust_gossip_allows_public_mode_without_penalty() {
        let kp_sender = KeyPair::from_seed(vec![25, 26, 27, 28], Algorithm::Ed25519);
        let kp_unknown = KeyPair::from_seed(vec![29, 30, 31, 32], Algorithm::Ed25519);
        let from_peer = Peer::new(
            "127.0.0.1:9100".parse().expect("addr"),
            kp_sender.public_key().clone(),
        );
        let unknown_peer = Peer::new(
            "127.0.0.1:9101".parse().expect("addr"),
            kp_unknown.public_key().clone(),
        );

        let info = PeerTrustInfo {
            peer_id: unknown_peer.id().clone(),
            trusted: true,
            score: 2,
        };
        let signature = Signature::new(
            kp_sender.private_key(),
            &PeersGossiper::trust_payload(&info),
        )
        .payload()
        .to_vec();
        let trust = vec![SignedPeerTrust { info, signature }];
        let now = Instant::now();
        let mut trust_book = TrustBook::new(
            Duration::from_millis(0),
            TrustPenalties {
                bad_gossip: 4,
                unknown_peer: 3,
            },
            -4,
        );
        trust_book.seed([from_peer.id().clone()], now);

        let outcome = process_trust_records(
            trust,
            &from_peer,
            true,
            &BTreeSet::new(),
            &BTreeSet::from([from_peer.id().clone()]),
            &mut trust_book,
            now,
        );

        assert!(!outcome.drop_sender);
        assert_eq!(outcome.newly_trusted.len(), 1);
        assert_eq!(
            outcome
                .newly_trusted
                .iter()
                .next()
                .expect("contains one peer"),
            unknown_peer.id()
        );
        assert_eq!(trust_book.score(from_peer.id(), now), 0);
    }
}
