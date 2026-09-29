//! The Sumeragi driver's transport over Iroha P2P (`specs/sumeragi.md` §12.2, §12.3 O5–O10;
//! integration map §5).
//!
//! - **Envelope.** [`NetworkMessage::Sumeragi`] carries one [`SumeragiFrame`]: the exact
//!   canonical consensus bytes, including source-bound application control, and the
//!   32-byte instance id. The driver enforces canonical, size-limited decoding with
//!   `WireMessage::decode`; the network codec sees an opaque byte string.
//! - **Classes.** A frame's traffic class comes from ONE helper, [`frame_class`] (the core's
//!   `traffic_class_of_frame` over the exact bytes), used by the decoded path
//!   ([`SumeragiFrame::topic`], hence `NetworkMessage::topic`/`admission_class`) and by the raw
//!   pre-decode path ([`inbound_frame_topic`], [`inbound_decode_limits`]) alike, so the P2P
//!   reader's raw/decoded comparison always agrees. Control → `ConsensusSafety` (the reserved
//!   safety FIFO), Proposal → `ConsensusPayload`, Bulk → `BlockSync` ([`topic_of_class`]).
//! - **Egress.** [`P2pNet`] implements the driver's [`Net`] with one `post_recoverable` per
//!   recipient ([`SumeragiTransport`]); a backpressured post is dropped (its ticket cancels on
//!   drop and the core rebroadcasts, "state, not custody"). It never uses `post()` (which
//!   asserts on reliable routes) or `broadcast_recoverable` (which targets the P2P topology, not
//!   the core's explicit recipients). The envelope is built once per frame and shared by every
//!   recipient.
//! - **Ingress.** The frames arrive on the driver's own P2P FIFOs — one per class on
//!   `SubscriberRoute::Sumeragi` ([`subscribe`]) — and [`SumeragiIngress`] hands each to the
//!   driver of its instance (the driver's ingress is bounded per peer and class, O6), dropping
//!   the P2P retention at once so credits never stall.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
};

use iroha_config::parameters::defaults::network::{
    MAX_FRAME_BYTES_CONTROL, MAX_PLAINTEXT_FRAME_BYTES,
};
use iroha_model_base::peer::PeerId;
use iroha_p2p::{
    Priority,
    network::{
        NetworkActorAdmissionError, SubscriberFilter,
        message::{Post, SubscriberRoute, Topic, TransportAdmissionClass},
    },
    peer::message::PeerMessage,
};
use iroha_sumeragi::{
    message::{TrafficClass, traffic_class_of_frame},
    pacemaker::FRAME_OVERHEAD,
    types::{Hash32, PublicKey},
};
use norito::codec::{Decode, Encode};
use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc;

use super::{
    crypto::{core_key, iroha_key},
    driver::{
        DriverHandle,
        traits::{Frame, Net},
    },
};
use crate::{IrohaNetwork, NetworkMessage};

/// One Sumeragi wire frame in the P2P envelope: the exact canonical `WireMessage` encoding and
/// the instance it belongs to. The bytes are never decoded by the network codec.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::net::SumeragiFrame")]
pub struct SumeragiFrame {
    instance: [u8; 32],
    frame: Vec<u8>,
}

impl core::fmt::Debug for SumeragiFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SumeragiFrame")
            .field("instance", &Hash32(self.instance))
            .field("len", &self.frame.len())
            .field("class", &self.class())
            .finish()
    }
}

impl SumeragiFrame {
    /// A frame of `instance` with the exact encoded bytes `frame`.
    pub fn new(instance: Hash32, frame: Vec<u8>) -> Self {
        Self {
            instance: instance.0,
            frame,
        }
    }

    /// The envelope of a driver frame.
    pub fn from_frame(frame: &Frame) -> Self {
        Self::new(frame.instance, frame.bytes.to_vec())
    }

    /// The instance the frame is routed to.
    pub fn instance(&self) -> Hash32 {
        Hash32(self.instance)
    }

    /// The exact encoded `WireMessage`.
    pub fn bytes(&self) -> &[u8] {
        &self.frame
    }

    /// The frame's traffic class ([`frame_class`]); `None` if it is not a classifiable frame.
    pub fn class(&self) -> Option<TrafficClass> {
        frame_class(&self.frame)
    }

    /// The P2P topic of the frame; `Topic::Other` if it is not classifiable (such a frame is
    /// never sent and is rejected by the raw classifier on receipt).
    pub fn topic(&self) -> Topic {
        self.class().map_or(Topic::Other, topic_of_class)
    }
}

/// The traffic class of a canonical consensus frame (§12.3 O8): the single
/// classifier of both the decoded and the raw P2P paths.
pub fn frame_class(frame: &[u8]) -> Option<TrafficClass> {
    traffic_class_of_frame(frame)
}

/// The P2P topic of a traffic class: control → `ConsensusSafety` (reserved safety FIFO),
/// proposal → `ConsensusPayload`, bulk → `BlockSync` (integration map §5).
pub const fn topic_of_class(class: TrafficClass) -> Topic {
    match class {
        TrafficClass::Control => Topic::ConsensusSafety,
        TrafficClass::Proposal => Topic::ConsensusPayload,
        TrafficClass::Bulk => Topic::BlockSync,
    }
}

/// The P2P admission class of a traffic class (`Safety`, `Payload`, `BlockSync`).
pub const fn admission_of_class(class: TrafficClass) -> TransportAdmissionClass {
    TransportAdmissionClass::ordinary_for_topic(topic_of_class(class))
}

/// The P2P send priority of a traffic class (the actor re-derives it from the topic).
pub const fn priority_of_class(class: TrafficClass) -> Priority {
    match class {
        TrafficClass::Control | TrafficClass::Proposal => Priority::High,
        TrafficClass::Bulk => Priority::Low,
    }
}

/// Frame caps per traffic class, in bytes of the encoded `WireMessage` (integration map §5,
/// spec §12.3 O10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCaps {
    /// Control frames.
    pub control: usize,
    /// Proposal frames (`max_block_bytes + 64 KiB`).
    pub proposal: usize,
    /// Bulk frames (`sync_max_bytes + 64 KiB`).
    pub bulk: usize,
}

impl FrameCaps {
    /// The transport's static caps (its topic frame caps): control 2 MiB, proposal and bulk
    /// the payload/block-sync cap. The chain parameters are validated against them (O10).
    pub const TRANSPORT: Self = Self {
        control: MAX_FRAME_BYTES_CONTROL.get(),
        proposal: MAX_PLAINTEXT_FRAME_BYTES.get(),
        bulk: MAX_PLAINTEXT_FRAME_BYTES.get(),
    };

    /// The caps for `max_block_bytes` and `sync_max_bytes`: control 2 MiB, proposal
    /// `max_block_bytes + 64 KiB`, bulk `sync_max_bytes + 64 KiB`, each at most the transport's.
    pub fn for_params(max_block_bytes: u32, sync_max_bytes: u32) -> Self {
        let with_overhead = |bytes: u32| {
            usize::try_from(u64::from(bytes) + u64::from(FRAME_OVERHEAD)).unwrap_or(usize::MAX)
        };
        Self {
            control: Self::TRANSPORT.control,
            proposal: with_overhead(max_block_bytes).min(Self::TRANSPORT.proposal),
            bulk: with_overhead(sync_max_bytes).min(Self::TRANSPORT.bulk),
        }
    }

    /// The cap of `class`.
    pub const fn of(&self, class: TrafficClass) -> usize {
        match class {
            TrafficClass::Control => self.control,
            TrafficClass::Proposal => self.proposal,
            TrafficClass::Bulk => self.bulk,
        }
    }
}

/// The exact `WireMessage` bytes inside the bare `SumeragiFrame` payload `field` (the value
/// after the `NetworkMessage` variant and `Arc` boundaries), without copying or decoding.
fn raw_frame(field: &[u8], flags: u8) -> Result<&[u8], norito::core::Error> {
    use norito::core::Error;
    // The sole native envelope has exactly two length-prefixed fields. Parse
    // both boundaries before exposing the byte sequence; no retired envelope
    // decoder or heuristic layout selection participates in admission.
    let mut remaining = field;
    let mut frame = None;
    for index in 0..2 {
        let (length, prefix) = norito::core::read_len_from_slice_with_flags(remaining, flags)?;
        let end = prefix.checked_add(length).ok_or(Error::LengthMismatch)?;
        let value = remaining.get(prefix..end).ok_or(Error::LengthMismatch)?;
        remaining = remaining.get(end..).ok_or(Error::LengthMismatch)?;
        if index == 1 {
            frame = Some(value);
        }
    }
    if !remaining.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let frame = frame.ok_or(Error::LengthMismatch)?;
    let count: [u8; 8] = frame
        .get(..8)
        .ok_or(Error::LengthMismatch)?
        .try_into()
        .map_err(|_| Error::LengthMismatch)?;
    let count = usize::try_from(u64::from_le_bytes(count)).map_err(|_| Error::LengthMismatch)?;
    if count.checked_add(8) != Some(frame.len()) {
        return Err(Error::LengthMismatch);
    }
    frame.get(8..).ok_or(Error::LengthMismatch)
}

/// Raw (pre-decode) P2P topic of a bare `SumeragiFrame` payload: [`frame_class`] of the exact
/// bytes, as for the decoded value. An unclassifiable frame is an error (rejected unread).
///
/// # Errors
/// A malformed envelope or an unclassifiable frame.
pub fn inbound_frame_topic(field: &[u8], flags: u8) -> Result<Topic, norito::core::Error> {
    frame_class(raw_frame(field, flags)?)
        .map(topic_of_class)
        .ok_or_else(|| norito::core::Error::Message("unclassifiable Sumeragi frame".to_owned()))
}

/// Decode limits of a `NetworkMessage::Sumeragi` P2P frame of `framed_len` bytes whose bare
/// `SumeragiFrame` payload is `field`: the class's transport cap bounds the frame, its one
/// byte string and the total allocation.
///
/// # Errors
/// A malformed envelope, an unclassifiable frame, or a frame above its class cap.
pub fn inbound_decode_limits(
    field: &[u8],
    framed_len: usize,
    flags: u8,
) -> Result<norito::DecodeLimits, norito::core::Error> {
    let class = frame_class(raw_frame(field, flags)?)
        .ok_or_else(|| norito::core::Error::Message("unclassifiable Sumeragi frame".to_owned()))?;
    let cap = FrameCaps::TRANSPORT.of(class);
    if framed_len > cap {
        return Err(norito::core::Error::ArchiveLengthExceeded {
            length: u64::try_from(framed_len).unwrap_or(u64::MAX),
            limit: u64::try_from(cap).unwrap_or(u64::MAX),
        });
    }
    Ok(norito::DecodeLimits::new(
        cap,
        cap,
        cap.saturating_add(64),
        cap.saturating_mul(12),
        64,
    ))
}

/// What became of one posted frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostOutcome {
    /// Admitted by the network actor.
    Admitted,
    /// Temporary pressure: dropped (the core rebroadcasts).
    Backpressured,
    /// The network is shut down.
    Closed,
    /// Permanently refused (e.g. not a reliable route, a frame the P2P layer refuses).
    Rejected,
}

/// The non-blocking reliable post the driver needs from the P2P network (a test seam).
pub trait SumeragiTransport: Send + Sync {
    /// Post `message` to `to` once, without blocking and without retaining it on pressure.
    fn post_frame(&self, to: PeerId, message: NetworkMessage, priority: Priority) -> PostOutcome;
}

impl SumeragiTransport for IrohaNetwork {
    fn post_frame(&self, to: PeerId, message: NetworkMessage, priority: Priority) -> PostOutcome {
        let post = Post {
            data: message,
            peer_id: to,
            priority,
        };
        match self.post_recoverable(post, None) {
            Ok(()) => PostOutcome::Admitted,
            // The message and its ticket are dropped here: the ticket cancels itself.
            Err(NetworkActorAdmissionError::Backpressured { .. }) => PostOutcome::Backpressured,
            Err(NetworkActorAdmissionError::Closed { .. }) => PostOutcome::Closed,
            Err(NetworkActorAdmissionError::Rejected { reason, .. }) => {
                iroha_logger::debug!(?reason, "sumeragi frame refused by the P2P actor");
                PostOutcome::Rejected
            }
        }
    }
}

/// Egress counters of a [`P2pNet`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetStats {
    /// Posts admitted.
    pub sent: u64,
    /// Posts dropped under backpressure.
    pub backpressured: u64,
    /// Posts dropped because the network is closed.
    pub closed: u64,
    /// Posts refused.
    pub rejected: u64,
    /// Recipients that are not valid BLS-normal keys.
    pub bad_recipient: u64,
}

#[derive(Default)]
struct Counters {
    sent: AtomicU64,
    backpressured: AtomicU64,
    closed: AtomicU64,
    rejected: AtomicU64,
    bad_recipient: AtomicU64,
}

/// Largest number of recipient `PeerId`s cached by a [`P2pNet`].
const PEER_CACHE: usize = 4096;

/// The driver's [`Net`] over the P2P network (see the module documentation).
pub struct P2pNet<T> {
    transport: T,
    /// The envelope of the frame sent last (one per frame, shared by its recipients).
    envelope: Mutex<Option<(Arc<[u8]>, Arc<SumeragiFrame>)>>,
    /// Recipient keys already converted to `PeerId`s.
    peers: Mutex<HashMap<PublicKey, PeerId>>,
    counters: Counters,
}

impl<T> core::fmt::Debug for P2pNet<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("P2pNet")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl<T> P2pNet<T> {
    /// A transport over `transport` (the node's `IrohaNetwork`).
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            envelope: Mutex::new(None),
            peers: Mutex::new(HashMap::new()),
            counters: Counters::default(),
        }
    }

    /// The egress counters.
    pub fn stats(&self) -> NetStats {
        let c = &self.counters;
        NetStats {
            sent: c.sent.load(Ordering::Relaxed),
            backpressured: c.backpressured.load(Ordering::Relaxed),
            closed: c.closed.load(Ordering::Relaxed),
            rejected: c.rejected.load(Ordering::Relaxed),
            bad_recipient: c.bad_recipient.load(Ordering::Relaxed),
        }
    }

    /// The envelope of `frame`: built once and shared (`Arc`) by every recipient.
    fn envelope(&self, frame: &Frame) -> Arc<SumeragiFrame> {
        let mut cached = self.envelope.lock();
        if let Some((bytes, envelope)) = cached.as_ref()
            && Arc::ptr_eq(bytes, &frame.bytes)
            && envelope.instance() == frame.instance
        {
            return Arc::clone(envelope);
        }
        let envelope = Arc::new(SumeragiFrame::from_frame(frame));
        *cached = Some((Arc::clone(&frame.bytes), Arc::clone(&envelope)));
        envelope
    }

    /// The `PeerId` of a recipient key; `None` if the key is not a valid BLS-normal key.
    fn peer(&self, key: &PublicKey) -> Option<PeerId> {
        if let Some(peer) = self.peers.lock().get(key) {
            return Some(peer.clone());
        }
        let peer = PeerId::new(iroha_key(key).ok()?);
        let mut peers = self.peers.lock();
        if peers.len() >= PEER_CACHE {
            peers.clear();
        }
        peers.insert(key.clone(), peer.clone());
        Some(peer)
    }
}

impl<T: SumeragiTransport> Net for P2pNet<T> {
    fn send(&self, to: &PublicKey, frame: &Frame) {
        let Some(peer) = self.peer(to) else {
            self.counters.bad_recipient.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let message = NetworkMessage::Sumeragi(self.envelope(frame));
        let counter = match self
            .transport
            .post_frame(peer, message, priority_of_class(frame.class))
        {
            PostOutcome::Admitted => &self.counters.sent,
            PostOutcome::Backpressured => &self.counters.backpressured,
            PostOutcome::Closed => &self.counters.closed,
            PostOutcome::Rejected => &self.counters.rejected,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Where the ingress hands a frame: the driver of one instance (a test seam).
pub trait FrameSink: Send + Sync {
    /// Deliver the encoded frame `frame` from the authenticated peer `from`; returns whether it
    /// was queued (never blocks; the driver decodes and bounds it).
    fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool;
}

impl FrameSink for DriverHandle {
    fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool {
        DriverHandle::deliver(self, from, frame)
    }
}

/// What the ingress did with one inbound message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routed {
    /// Queued by the instance's driver.
    Delivered,
    /// The driver refused it (undecodable, another instance inside, the node's own, ...).
    Refused,
    /// Not a Sumeragi frame.
    NotSumeragi,
    /// Relayed (origin differs from the authenticated connection): not accepted, so the ingress
    /// stays bounded by the authenticated peers.
    Relayed,
    /// The sender's key is not a BLS-normal consensus key.
    BadSender,
    /// No driver runs the frame's instance.
    UnknownInstance,
    /// Unclassifiable, or above its class cap.
    Oversize,
}

/// Ingress counters of a [`SumeragiIngress`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IngressStats {
    /// Frames delivered.
    pub delivered: u64,
    /// Frames not delivered.
    pub dropped: u64,
}

/// Routes inbound Sumeragi frames to the driver of their instance (O9).
pub struct SumeragiIngress {
    routes: RwLock<HashMap<Hash32, Arc<dyn FrameSink>>>,
    caps: FrameCaps,
    delivered: AtomicU64,
    dropped: AtomicU64,
}

impl core::fmt::Debug for SumeragiIngress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SumeragiIngress")
            .field("instances", &self.routes.read().len())
            .field("caps", &self.caps)
            .finish_non_exhaustive()
    }
}

impl SumeragiIngress {
    /// An ingress with no instance, dropping frames above `caps`.
    pub fn new(caps: FrameCaps) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            caps,
            delivered: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }

    /// Route the frames of `instance` to `sink` (its driver).
    pub fn register(&self, instance: Hash32, sink: Arc<dyn FrameSink>) {
        self.routes.write().insert(instance, sink);
    }

    /// Stop routing `instance`; returns whether it was routed.
    pub fn unregister(&self, instance: &Hash32) -> bool {
        self.routes.write().remove(instance).is_some()
    }

    /// The ingress counters.
    pub fn stats(&self) -> IngressStats {
        IngressStats {
            delivered: self.delivered.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }

    /// Route one message from the P2P subscription. Its retention (P2P credit) is released
    /// before this returns.
    ///
    /// Only a [`PeerMessage`] reaches a driver, so every routed frame carries the peer of the
    /// authenticated connection it arrived on (and is dropped as [`Routed::Relayed`] when its
    /// origin differs). A decoded [`SumeragiFrame`] carries no sender and cannot be routed:
    ///
    /// ```compile_fail
    /// use iroha_core::sumeragi::net::{SumeragiFrame, SumeragiIngress};
    ///
    /// fn route_senderless_frame(ingress: &SumeragiIngress, frame: SumeragiFrame) {
    ///     ingress.route(frame);
    /// }
    /// ```
    ///
    /// ```no_run
    /// use iroha_core::{NetworkMessage, sumeragi::net::SumeragiIngress};
    /// use iroha_p2p::peer::message::PeerMessage;
    ///
    /// fn route_authenticated(ingress: &SumeragiIngress, message: PeerMessage<NetworkMessage>) {
    ///     ingress.route(message);
    /// }
    /// ```
    pub fn route(&self, message: PeerMessage<NetworkMessage>) -> Routed {
        let (origin, authenticated_via, payload, _bytes, retention) = message.into_parts();
        drop(retention);
        let routed = self.route_parts(origin.id(), &authenticated_via, &payload);
        let counter = if routed == Routed::Delivered {
            &self.delivered
        } else {
            &self.dropped
        };
        counter.fetch_add(1, Ordering::Relaxed);
        routed
    }

    /// Route a frame an in-process transport carries from the authenticated consensus key
    /// `from` (the same instance routing and class caps as [`Self::route`]).
    pub fn deliver(&self, from: &PublicKey, frame: &super::driver::traits::Frame) -> Routed {
        let routed = if frame.bytes.len() > self.caps.of(frame.class) {
            Routed::Oversize
        } else {
            match self.routes.read().get(&frame.instance).cloned() {
                None => Routed::UnknownInstance,
                Some(sink) if sink.deliver(from, &frame.bytes) => Routed::Delivered,
                Some(_) => Routed::Refused,
            }
        };
        let counter = if routed == Routed::Delivered {
            &self.delivered
        } else {
            &self.dropped
        };
        counter.fetch_add(1, Ordering::Relaxed);
        routed
    }

    /// [`SumeragiIngress::route`] of a message from `origin`, received over the
    /// authenticated connection of `via`.
    fn route_parts(&self, origin: &PeerId, via: &PeerId, payload: &NetworkMessage) -> Routed {
        let NetworkMessage::Sumeragi(frame) = payload else {
            return Routed::NotSumeragi;
        };
        if origin != via {
            return Routed::Relayed;
        }
        let Ok(from) = core_key(&via.public_key) else {
            return Routed::BadSender;
        };
        match frame.class() {
            Some(class) if frame.bytes().len() <= self.caps.of(class) => {}
            _ => return Routed::Oversize,
        }
        let Some(sink) = self.routes.read().get(&frame.instance()).cloned() else {
            return Routed::UnknownInstance;
        };
        if sink.deliver(&from, frame.bytes()) {
            Routed::Delivered
        } else {
            Routed::Refused
        }
    }
}

/// The driver's P2P FIFOs: one per traffic class on `SubscriberRoute::Sumeragi`.
#[derive(Debug)]
pub struct SumeragiSubscription {
    /// Control class (`Safety` admission, the reserved FIFO).
    pub control: mpsc::Receiver<PeerMessage<NetworkMessage>>,
    /// Proposal class (`Payload` admission).
    pub proposal: mpsc::Receiver<PeerMessage<NetworkMessage>>,
    /// Bulk class (`BlockSync` admission).
    pub bulk: mpsc::Receiver<PeerMessage<NetworkMessage>>,
}

/// The subscription filter of each traffic class's FIFO.
pub fn subscription_filters() -> [(TrafficClass, SubscriberFilter); 3] {
    [
        TrafficClass::Control,
        TrafficClass::Proposal,
        TrafficClass::Bulk,
    ]
    .map(|class| {
        (
            class,
            SubscriberFilter::SemanticClass {
                class: admission_of_class(class),
                route: SubscriberRoute::Sumeragi,
            },
        )
    })
}

/// Why the driver could not subscribe to its FIFOs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the P2P network refused the sumeragi subscription (shut down or queue full)")]
pub struct SubscribeError;

/// Subscribe the driver to its three FIFOs on `network`, each holding up to `capacity`
/// messages (the P2P layer bounds them further by bytes and credits).
///
/// # Errors
/// The network actor is shut down or its registration queue is full.
pub fn subscribe(
    network: &IrohaNetwork,
    capacity: usize,
) -> Result<SumeragiSubscription, SubscribeError> {
    let [control, proposal, bulk] = subscription_filters().map(|(_, filter)| {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        network
            .subscribe_to_peers_messages_with_filter(tx, filter)
            .map(|()| rx)
            .map_err(|_| SubscribeError)
    });
    Ok(SumeragiSubscription {
        control: control?,
        proposal: proposal?,
        bulk: bulk?,
    })
}

/// Drain `subscription` into `ingress` on a dedicated thread (decoding stays off the async
/// runtime), control first, then proposal, then bulk. The thread ends when every FIFO closes.
///
/// # Errors
/// The thread could not be spawned.
pub fn spawn_ingress(
    subscription: SumeragiSubscription,
    ingress: Arc<SumeragiIngress>,
) -> std::io::Result<JoinHandle<()>> {
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    std::thread::Builder::new()
        .name("sumeragi-ingress".to_owned())
        .spawn(move || {
            let SumeragiSubscription {
                mut control,
                mut proposal,
                mut bulk,
            } = subscription;
            runtime.block_on(async move {
                loop {
                    let message = tokio::select! {
                        biased;
                        Some(message) = control.recv() => message,
                        Some(message) = proposal.recv() => message,
                        Some(message) = bulk.recv() => message,
                        else => break,
                    };
                    ingress.route(message);
                }
            });
        })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::peer::Peer;
    use iroha_p2p::network::message::ClassifyTopic;
    use iroha_sumeragi::{
        message::{
            Block, BlockHeader, BlockRequest, BlockResponse, Proposal, Qc, Status, SyncEntry,
            SyncRequest, SyncResponse, TcEntry, TimeoutCert, TimeoutVote, Vote, VoteKind,
            WireMessage,
        },
        types::{AggregateSignature, Bitmap, EpochId, SIGNATURE_LEN, Signature},
    };
    use norito::core as ncore;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    use super::*;

    fn h(rng: &mut StdRng) -> Hash32 {
        Hash32(rng.random())
    }

    fn epoch(rng: &mut StdRng) -> EpochId {
        EpochId {
            epoch: rng.random_range(0..100),
            context: h(rng),
        }
    }

    fn sig(rng: &mut StdRng) -> Signature {
        Signature([rng.random(); SIGNATURE_LEN])
    }

    fn qc(rng: &mut StdRng, instance: Hash32) -> Qc {
        Qc {
            kind: if rng.random() {
                VoteKind::Prepare
            } else {
                VoteKind::Commit
            },
            instance,
            epoch: epoch(rng),
            height: rng.random_range(1..1_000),
            view: rng.random_range(0..10),
            block_hash: h(rng),
            result: h(rng),
            attest: false,
            signers: Bitmap::new(rng.random_range(1..20)),
            agg_sig: AggregateSignature([rng.random(); SIGNATURE_LEN]),
            attestations: Vec::new(),
            attestation_witness: None,
        }
    }

    fn block(rng: &mut StdRng, instance: Hash32) -> Block {
        let payload: Vec<u8> = (0..rng.random_range(0..300))
            .map(|_| rng.random())
            .collect();
        Block {
            header: BlockHeader {
                instance,
                epoch: epoch(rng),
                height: rng.random_range(1..1_000),
                origin_view: 0,
                parent_hash: h(rng),
                parent_result: h(rng),
                payload_hash: h(rng),
                payload_len: u32::try_from(payload.len()).unwrap(),
                proposer: 0,
                skipped_leaders: Vec::new(),
                control_witness: iroha_sumeragi::types::ControlWitness::empty(),
                attest: false,
            },
            payload,
        }
    }

    /// A random message of a random kind (tags 0..=9).
    fn message(rng: &mut StdRng) -> WireMessage {
        let instance = h(rng);
        let height = rng.random_range(1..1_000);
        let view = rng.random_range(0..10);
        match rng.random_range(0..10) {
            0 => {
                let block = block(rng, instance);
                let payload = rng.random::<bool>().then_some(block.payload);
                WireMessage::Proposal(Box::new(Proposal {
                    instance,
                    height,
                    view,
                    header: block.header,
                    justify: None,
                    parent_qc: rng.random::<bool>().then(|| qc(rng, instance)),
                    payload,
                    sig: sig(rng),
                }))
            }
            1 => WireMessage::Vote(Vote {
                kind: VoteKind::Prepare,
                instance,
                epoch: epoch(rng),
                height,
                view,
                block_hash: h(rng),
                result: h(rng),
                attest: false,
                signer: rng.random_range(0..10),
                sig: sig(rng),
                attestation: None,
            }),
            2 => WireMessage::Qc(qc(rng, instance)),
            3 => WireMessage::Timeout(Box::new(TimeoutVote {
                instance,
                epoch: epoch(rng),
                height,
                view,
                high_pqc: None,
                signer: 1,
                sig: sig(rng),
            })),
            4 => WireMessage::Tc(Box::new(TimeoutCert {
                instance,
                epoch: epoch(rng),
                height,
                view,
                entries: vec![TcEntry {
                    signer: 0,
                    hq: None,
                }],
                agg_sig: AggregateSignature([7; SIGNATURE_LEN]),
                high_pqc: None,
            })),
            5 => WireMessage::Status(Box::new(Status {
                instance,
                height,
                view,
                committed_qc: None,
                high_pqc: rng.random::<bool>().then(|| qc(rng, instance)),
                high_tc: None,
                proposal_hash: Some(h(rng)),
                want_proposal: rng.random(),
                probe: None,
                echo: None,
            })),
            6 => WireMessage::SyncRequest(SyncRequest {
                instance,
                from_height: height,
                max_count: 8,
                max_bytes: 1 << 20,
            }),
            7 => WireMessage::SyncResponse(SyncResponse {
                instance,
                blocks: (0..rng.random_range(0..3))
                    .map(|_| SyncEntry {
                        block: block(rng, instance),
                        commit_qc: qc(rng, instance),
                    })
                    .collect(),
            }),
            8 => WireMessage::BlockRequest(BlockRequest {
                instance,
                height,
                block_hash: h(rng),
            }),
            _ => WireMessage::BlockResponse(BlockResponse {
                instance,
                block: block(rng, instance),
            }),
        }
    }

    const LAYOUTS: [u8; 2] = [0, ncore::header_flags::COMPACT_LEN];

    /// The raw (pre-decode) topic, admission class and decode limits of a network message
    /// encoded under `layout`, and the message decoded back.
    fn raw_and_decoded(
        message: &NetworkMessage,
        layout: u8,
    ) -> (
        Result<Option<Topic>, ncore::Error>,
        Result<TransportAdmissionClass, ncore::Error>,
        NetworkMessage,
    ) {
        let encoded = {
            let _layout = ncore::DecodeFlagsGuard::enter(layout);
            ncore::to_bytes(message).unwrap()
        };
        let view = ncore::from_bytes_view(&encoded).unwrap();
        let topic = NetworkMessage::inbound_topic(view.as_bytes(), view.flags());
        let class = NetworkMessage::inbound_admission_class(view.as_bytes(), view.flags());
        if topic.is_ok() {
            let limits =
                NetworkMessage::inbound_decode_limits(view.as_bytes(), encoded.len(), view.flags())
                    .unwrap()
                    .expect("a classified Sumeragi frame installs decode limits");
            let bounded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
                .expect("the limits admit the frame");
            assert!(matches!(bounded, NetworkMessage::Sumeragi(_)));
        }
        let decoded = ncore::decode_from_bytes::<NetworkMessage>(&encoded).unwrap();
        (topic, class, decoded)
    }

    /// Property: for random frames of every kind, and for random corruptions of them, the raw
    /// classification agrees with the decoded one under every layout (one shared helper), and a
    /// valid frame's class is the core's `traffic_class` mapped by `topic_of_class`.
    #[test]
    fn raw_and_decoded_classes_agree() {
        let mut rng = StdRng::seed_from_u64(0x5eed_5eed);
        for round in 0..400 {
            let msg = message(&mut rng);
            let mut bytes = msg.encode().unwrap();
            let valid = round % 4 != 3;
            if !valid {
                for _ in 0..rng.random_range(1..4) {
                    let at = rng.random_range(0..bytes.len());
                    bytes[at] = rng.random();
                }
            }
            let frame = SumeragiFrame::new(*msg.instance(), bytes);
            let network = NetworkMessage::Sumeragi(Arc::new(frame.clone()));
            for layout in LAYOUTS {
                let (raw_topic, raw_class, decoded) = raw_and_decoded(&network, layout);
                assert_eq!(decoded.subscriber_route(), SubscriberRoute::Sumeragi);
                match raw_topic {
                    Ok(topic) => {
                        assert_eq!(topic, Some(decoded.topic()), "round {round} {layout:#x}");
                        assert_eq!(raw_class.unwrap(), decoded.admission_class());
                        assert!(decoded.is_outbound_allowed());
                    }
                    Err(_) => {
                        assert!(raw_class.is_err());
                        assert_eq!(decoded.topic(), Topic::Other);
                        assert!(!decoded.is_outbound_allowed(), "never sent");
                    }
                }
                if valid {
                    let expected = topic_of_class(msg.traffic_class());
                    assert_eq!(decoded.topic(), expected);
                    assert_eq!(raw_topic.unwrap(), Some(expected));
                    assert_eq!(
                        decoded.admission_class(),
                        admission_of_class(msg.traffic_class())
                    );
                }
            }
        }
    }

    #[test]
    fn class_table() {
        use TrafficClass as C;
        assert_eq!(topic_of_class(C::Control), Topic::ConsensusSafety);
        assert_eq!(topic_of_class(C::Proposal), Topic::ConsensusPayload);
        assert_eq!(topic_of_class(C::Bulk), Topic::BlockSync);
        assert_eq!(
            admission_of_class(C::Control),
            TransportAdmissionClass::Safety
        );
        assert_eq!(
            admission_of_class(C::Proposal),
            TransportAdmissionClass::Payload
        );
        assert_eq!(
            admission_of_class(C::Bulk),
            TransportAdmissionClass::BlockSync
        );
        assert_eq!(priority_of_class(C::Control), Priority::High);
        assert_eq!(priority_of_class(C::Proposal), Priority::High);
        assert_eq!(priority_of_class(C::Bulk), Priority::Low);
        for (class, filter) in subscription_filters() {
            assert_eq!(
                filter,
                SubscriberFilter::SemanticClass {
                    class: admission_of_class(class),
                    route: SubscriberRoute::Sumeragi,
                }
            );
        }
        let network = NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(Hash32::ZERO, vec![])));
        assert_eq!(
            network.progress_reconstruction(),
            iroha_p2p::network::message::ProgressReconstruction::Retransmit
        );
        assert!(
            iroha_p2p::network::reliable_progress_class(
                Topic::ConsensusSafety,
                SubscriberRoute::Sumeragi
            )
            .is_some()
        );
    }

    #[test]
    fn frame_caps() {
        let caps = FrameCaps::for_params(4 << 20, 16 << 20);
        assert_eq!(caps.control, 2 << 20);
        assert_eq!(
            caps.proposal,
            (4 << 20) + usize::try_from(FRAME_OVERHEAD).unwrap()
        );
        assert_eq!(
            caps.bulk,
            (16 << 20) + usize::try_from(FRAME_OVERHEAD).unwrap()
        );
        assert_eq!(caps.of(TrafficClass::Bulk), caps.bulk);
        let huge = FrameCaps::for_params(u32::MAX, u32::MAX);
        assert_eq!(huge.proposal, FrameCaps::TRANSPORT.proposal);
        assert_eq!(huge.bulk, FrameCaps::TRANSPORT.bulk);
        // A frame above its class's transport cap is refused before decode.
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: Hash32::ZERO,
            height: 1,
            block_hash: Hash32::ZERO,
        });
        let network = NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(
            Hash32::ZERO,
            msg.encode().unwrap(),
        )));
        let encoded = ncore::to_bytes(&network).unwrap();
        let view = ncore::from_bytes_view(&encoded).unwrap();
        let (_, remaining) = crate::inbound_enum_parts(view.as_bytes()).unwrap();
        let field = crate::inbound_owned_enum_field(remaining, view.flags()).unwrap();
        assert!(
            inbound_decode_limits(field, FrameCaps::TRANSPORT.control + 1, view.flags()).is_err()
        );
        assert!(inbound_decode_limits(field, 100, view.flags()).is_ok());
        assert_eq!(
            inbound_frame_topic(field, view.flags()).unwrap(),
            Topic::ConsensusSafety
        );
        assert!(raw_frame(&field[..field.len() - 1], view.flags()).is_err());
    }

    #[test]
    fn raw_native_envelope_rejects_suffix_and_byte_count_substitution() {
        let network = NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(
            Hash32::ZERO,
            WireMessage::BlockRequest(BlockRequest {
                instance: Hash32::ZERO,
                height: 1,
                block_hash: Hash32::ZERO,
            })
            .encode()
            .unwrap(),
        )));
        let encoded = ncore::to_bytes(&network).unwrap();
        let view = ncore::from_bytes_view(&encoded).unwrap();
        let (_, remaining) = crate::inbound_enum_parts(view.as_bytes()).unwrap();
        let field = crate::inbound_owned_enum_field(remaining, view.flags()).unwrap();
        let mut suffixed = field.to_vec();
        suffixed.push(0);
        assert!(raw_frame(&suffixed, view.flags()).is_err());
        let (instance_len, prefix_len) =
            ncore::read_len_from_slice_with_flags(field, view.flags()).unwrap();
        let frame_field_offset = prefix_len + instance_len;
        let (_, frame_prefix_len) =
            ncore::read_len_from_slice_with_flags(&field[frame_field_offset..], view.flags())
                .unwrap();
        let count_offset = frame_field_offset + frame_prefix_len;
        for count in [0_u64, u64::MAX] {
            let mut changed = field.to_vec();
            changed[count_offset..count_offset + 8].copy_from_slice(&count.to_le_bytes());
            assert!(raw_frame(&changed, view.flags()).is_err());
        }
        for end in 0..field.len() {
            assert!(raw_frame(&field[..end], view.flags()).is_err());
        }
    }

    #[test]
    fn frame_accessors() {
        let msg = WireMessage::SyncRequest(SyncRequest {
            instance: Hash32([3; 32]),
            from_height: 1,
            max_count: 1,
            max_bytes: 1,
        });
        let bytes: Arc<[u8]> = msg.encode().unwrap().into();
        let frame = Frame {
            instance: Hash32([3; 32]),
            class: msg.traffic_class(),
            bytes: Arc::clone(&bytes),
        };
        let envelope = SumeragiFrame::from_frame(&frame);
        assert_eq!(envelope.instance(), Hash32([3; 32]));
        assert_eq!(envelope.bytes(), &bytes[..]);
        assert_eq!(envelope.class(), Some(TrafficClass::Control));
        assert_eq!(envelope.topic(), Topic::ConsensusSafety);
        assert!(format!("{envelope:?}").contains("SumeragiFrame"));
        assert_eq!(
            SumeragiFrame::new(Hash32::ZERO, vec![1, 2]).topic(),
            Topic::Other
        );
        assert_eq!(frame_class(&bytes), Some(TrafficClass::Control));
    }

    /// A transport that records posts and answers with a scripted outcome.
    #[derive(Default)]
    struct FakeTransport {
        posts: Mutex<Vec<(PeerId, NetworkMessage, Priority)>>,
        outcome: Mutex<Option<PostOutcome>>,
    }

    impl SumeragiTransport for FakeTransport {
        fn post_frame(
            &self,
            to: PeerId,
            message: NetworkMessage,
            priority: Priority,
        ) -> PostOutcome {
            self.posts.lock().push((to, message, priority));
            self.outcome.lock().unwrap_or(PostOutcome::Admitted)
        }
    }

    fn bls(seed: u8) -> KeyPair {
        KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal)
    }

    fn frame_of(msg: &WireMessage) -> Frame {
        Frame {
            instance: *msg.instance(),
            class: msg.traffic_class(),
            bytes: msg.encode().unwrap().into(),
        }
    }

    /// Egress: one post per recipient, sharing one envelope; a backpressured (or closed or
    /// refused) post is dropped and counted, never retried or blocked on; an invalid recipient
    /// key is skipped.
    #[test]
    fn egress_posts_per_recipient_and_drops_on_backpressure() {
        let net = P2pNet::new(FakeTransport::default());
        let (a, b) = (bls(1), bls(2));
        let (ka, kb) = (
            core_key(a.public_key()).unwrap(),
            core_key(b.public_key()).unwrap(),
        );
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: Hash32([9; 32]),
            height: 3,
            block_hash: Hash32::ZERO,
        });
        let frame = frame_of(&msg);
        net.send(&ka, &frame);
        net.send(&kb, &frame);
        {
            let posts = net.transport.posts.lock();
            assert_eq!(posts.len(), 2);
            assert_eq!(posts[0].0, PeerId::new(a.public_key().clone()));
            assert_eq!(posts[1].0, PeerId::new(b.public_key().clone()));
            assert_eq!(posts[0].2, Priority::High);
            let (NetworkMessage::Sumeragi(x), NetworkMessage::Sumeragi(y)) =
                (&posts[0].1, &posts[1].1)
            else {
                panic!("not a Sumeragi frame");
            };
            assert!(Arc::ptr_eq(x, y), "one envelope per frame");
            assert_eq!(WireMessage::decode(x.bytes(), usize::MAX).unwrap(), msg);
        }
        for (outcome, expected) in [
            (
                PostOutcome::Backpressured,
                NetStats {
                    sent: 2,
                    backpressured: 1,
                    ..NetStats::default()
                },
            ),
            (
                PostOutcome::Closed,
                NetStats {
                    sent: 2,
                    backpressured: 1,
                    closed: 1,
                    ..NetStats::default()
                },
            ),
            (
                PostOutcome::Rejected,
                NetStats {
                    sent: 2,
                    backpressured: 1,
                    closed: 1,
                    rejected: 1,
                    ..NetStats::default()
                },
            ),
        ] {
            *net.transport.outcome.lock() = Some(outcome);
            net.send(&ka, &frame);
            assert_eq!(net.stats(), expected);
        }
        assert_eq!(
            net.transport.posts.lock().len(),
            5,
            "each post attempted once"
        );
        net.send(&PublicKey::new(vec![1; 48]).unwrap(), &frame);
        assert_eq!(net.stats().bad_recipient, 1);
        assert_eq!(net.transport.posts.lock().len(), 5);
        // A new frame gets a new envelope; bulk frames go low priority.
        let bulk = frame_of(&WireMessage::SyncResponse(SyncResponse {
            instance: Hash32([9; 32]),
            blocks: Vec::new(),
        }));
        net.send(&kb, &bulk);
        let posts = net.transport.posts.lock();
        assert_eq!(posts[5].2, Priority::Low);
        assert!(format!("{net:?}").contains("P2pNet"));
    }

    /// Egress through the real P2P actor admission (`post_recoverable`): an admitted post
    /// reaches the actor queue as the exact frame; a full queue or a peer outside the reliable
    /// topology backpressures and the frame is dropped at once (never retained or retried);
    /// after the actor drains, the next frame is admitted; a closed network drops too.
    #[test]
    fn egress_over_the_p2p_actor_drops_on_backpressure() {
        let (target, stranger) = (bls(4), bls(5));
        let target_peer = PeerId::new(target.public_key().clone());
        let (network, mut actor) = IrohaNetwork::actor_admission_for_tests(
            PeerId::new(bls(3).public_key().clone()),
            std::collections::HashSet::from([target_peer.clone()]),
            std::num::NonZeroUsize::new(1).unwrap(),
        );
        let net = P2pNet::new(network);
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: Hash32([9; 32]),
            height: 3,
            block_hash: Hash32::ZERO,
        });
        let frame = frame_of(&msg);
        let to = core_key(target.public_key()).unwrap();
        net.send(&to, &frame);
        assert_eq!(net.stats().sent, 1);
        net.send(&to, &frame);
        assert_eq!(net.stats().backpressured, 1, "queue full: dropped");
        net.send(&core_key(stranger.public_key()).unwrap(), &frame);
        assert_eq!(
            net.stats().backpressured,
            2,
            "no reliable membership: dropped"
        );
        let mut seen = Vec::new();
        let drained = actor.drain_posts(|post| {
            let NetworkMessage::Sumeragi(envelope) = &post.data else {
                panic!("not a Sumeragi frame");
            };
            seen.push((post.peer_id.clone(), envelope.bytes().to_vec()));
        });
        assert_eq!(drained, 1);
        assert_eq!(seen, vec![(target_peer, msg.encode().unwrap())]);
        net.send(&to, &frame);
        assert_eq!(
            net.stats().sent,
            2,
            "admitted again after the actor drained"
        );
        let closed = P2pNet::new(IrohaNetwork::closed_for_tests());
        closed.send(&to, &frame);
        let stats = closed.stats();
        assert_eq!(stats.sent, 0);
        assert_eq!(stats.closed + stats.backpressured + stats.rejected, 1);
    }

    /// A sink that records deliveries.
    #[derive(Default)]
    struct Sink {
        frames: Mutex<Vec<(PublicKey, Vec<u8>)>>,
        refuse: bool,
    }

    impl FrameSink for Sink {
        fn deliver(&self, from: &PublicKey, frame: &[u8]) -> bool {
            self.frames.lock().push((from.clone(), frame.to_vec()));
            !self.refuse
        }
    }

    fn peer_message(kp: &KeyPair, payload: NetworkMessage) -> PeerMessage<NetworkMessage> {
        let peer = Peer::new("127.0.0.1:1337".parse().unwrap(), kp.public_key().clone());
        PeerMessage::new(peer, payload, 0)
    }

    /// Ingress routing: frames go to their instance's driver as `(from, exact bytes)`; other
    /// instances, other messages, non-BLS senders, relayed and oversize frames are dropped.
    #[test]
    fn ingress_routes_by_instance() {
        let caps = FrameCaps::for_params(1024, 1024);
        let ingress = SumeragiIngress::new(caps);
        let (i, j) = (Hash32([1; 32]), Hash32([2; 32]));
        let sink = Arc::new(Sink::default());
        ingress.register(i, sink.clone());
        let sender = bls(5);
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: i,
            height: 3,
            block_hash: Hash32::ZERO,
        });
        let bytes = msg.encode().unwrap();
        let frame = |instance| {
            NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(instance, bytes.clone())))
        };
        assert_eq!(
            ingress.route(peer_message(&sender, frame(i))),
            Routed::Delivered
        );
        {
            let frames = sink.frames.lock();
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].0, core_key(sender.public_key()).unwrap());
            assert_eq!(frames[0].1, bytes);
        }
        assert_eq!(
            ingress.route(peer_message(&sender, frame(j))),
            Routed::UnknownInstance
        );
        assert_eq!(
            ingress.route(peer_message(&sender, NetworkMessage::Health)),
            Routed::NotSumeragi
        );
        let ed = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        assert_eq!(
            ingress.route(peer_message(&ed, frame(i))),
            Routed::BadSender
        );
        let big = WireMessage::BlockResponse(BlockResponse {
            instance: i,
            block: Block {
                header: BlockHeader {
                    instance: i,
                    epoch: EpochId {
                        epoch: 0,
                        context: Hash32([0x61; 32]),
                    },
                    height: 1,
                    origin_view: 0,
                    parent_hash: Hash32::ZERO,
                    parent_result: Hash32::ZERO,
                    payload_hash: Hash32::ZERO,
                    payload_len: 0,
                    proposer: 0,
                    skipped_leaders: Vec::new(),
                    control_witness: iroha_sumeragi::types::ControlWitness::empty(),
                    attest: false,
                },
                payload: vec![0; 70 * 1024],
            },
        });
        let oversize =
            NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(i, big.encode().unwrap())));
        assert_eq!(
            ingress.route(peer_message(&sender, oversize)),
            Routed::Oversize
        );
        let junk = NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(i, vec![1, 2, 3])));
        assert_eq!(ingress.route(peer_message(&sender, junk)), Routed::Oversize);
        let other = PeerId::new(bls(6).public_key().clone());
        let via = PeerId::new(sender.public_key().clone());
        assert_eq!(
            ingress.route_parts(&other, &via, &frame(i)),
            Routed::Relayed
        );
        assert_eq!(
            ingress.stats(),
            IngressStats {
                delivered: 1,
                dropped: 5
            }
        );
        let refusing = Arc::new(Sink {
            refuse: true,
            ..Sink::default()
        });
        ingress.register(i, refusing);
        assert_eq!(
            ingress.route(peer_message(&sender, frame(i))),
            Routed::Refused
        );
        assert!(ingress.unregister(&i));
        assert!(!ingress.unregister(&i));
        assert!(format!("{ingress:?}").contains("SumeragiIngress"));
    }

    /// The ingress thread drains the three FIFOs into the router and ends when they close.
    #[test]
    fn ingress_thread_drains_fifos() {
        let ingress = Arc::new(SumeragiIngress::new(FrameCaps::TRANSPORT));
        let i = Hash32([1; 32]);
        let sink = Arc::new(Sink::default());
        ingress.register(i, sink.clone());
        let (ctx, crx) = mpsc::channel(4);
        let (ptx, prx) = mpsc::channel(4);
        let (btx, brx) = mpsc::channel(4);
        let thread = spawn_ingress(
            SumeragiSubscription {
                control: crx,
                proposal: prx,
                bulk: brx,
            },
            Arc::clone(&ingress),
        )
        .unwrap();
        let sender = bls(7);
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: i,
            height: 3,
            block_hash: Hash32::ZERO,
        });
        let frame =
            || NetworkMessage::Sumeragi(Arc::new(SumeragiFrame::new(i, msg.encode().unwrap())));
        for tx in [&ctx, &ptx, &btx] {
            tx.blocking_send(peer_message(&sender, frame())).unwrap();
        }
        drop((ctx, ptx, btx));
        thread.join().unwrap();
        assert_eq!(sink.frames.lock().len(), 3);
        assert_eq!(ingress.stats().delivered, 3);
    }
}

#[cfg(test)]
#[test]
fn retired_beacon_sideframes_have_no_transport_classifier() {
    let mut retired = b"IROHA-BEACON\x01".to_vec();
    retired.extend_from_slice(&[0; 256]);
    assert_eq!(frame_class(&retired), None);
    assert_eq!(SumeragiFrame::new(Hash32([1; 32]), retired).class(), None);
}
