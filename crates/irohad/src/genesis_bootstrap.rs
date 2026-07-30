//! Genesis bootstrap orchestration for fetching and serving genesis over P2P.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use iroha_config::parameters::actual::Genesis as GenesisConfig;
use iroha_core::{
    IrohaNetwork, NetworkMessage,
    genesis::{GenesisRequest, GenesisRequestKind, GenesisResponse, GenesisResponseError},
    validate_genesis_block,
};
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{BlockHeader, decode_framed_signed_block},
    peer::{Peer, PeerId},
};
use iroha_genesis::GenesisBlock;
use iroha_logger::prelude::*;
use iroha_p2p::{
    Post, Priority, UpdatePeers, UpdateTopology,
    network::{
        NetworkActorAdmissionError, NetworkActorAdmissionTicket, NetworkReplyAdmissionOutcome,
        NetworkReplyRoute, RELIABLE_PROGRESS_GENESIS_FETCH_PRODUCERS_PER_SOURCE,
        RELIABLE_PROGRESS_GENESIS_REPLY_LISTENER_PRODUCERS, SubscriberFilter,
        genesis_reply_waiters_per_source,
        message::{SubscriberRoute, Topic},
    },
    peer::message::PeerMessage,
};
use iroha_primitives::addr::SocketAddr;
use norito::codec::Encode as _;
use tokio::{
    sync::mpsc,
    time::{self, Instant},
};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

const _: () = assert!(RELIABLE_PROGRESS_GENESIS_REPLY_LISTENER_PRODUCERS == 1);
const _: () = assert!(RELIABLE_PROGRESS_GENESIS_FETCH_PRODUCERS_PER_SOURCE == 1);

fn genesis_message_filter() -> SubscriberFilter {
    // Requests are small control messages, while responses may carry the full
    // genesis payload and therefore use the bounded block-sync transport class.
    SubscriberFilter::topics_for_route(
        [Topic::Control, Topic::BlockSync],
        SubscriberRoute::GenesisBootstrap,
    )
}

/// Minimal network surface needed for genesis bootstrap.
pub trait GenesisNetwork: Clone + Send + Sync + 'static {
    /// Opaque authenticated route retained for one request occurrence.
    type ReplyRoute: Clone + Send + Sync + 'static;
    /// Opaque authenticated transport owner used for fair reply scheduling.
    type ReplySource: Clone + Eq + std::hash::Hash + Send + Sync + 'static;

    /// Admit a progress message while returning exact ownership under backpressure.
    ///
    /// # Errors
    ///
    /// Returns the exact post for temporary pressure, actor closure, or permanent rejection.
    fn post_recoverable(
        &self,
        msg: Post<NetworkMessage>,
        ticket: Option<NetworkActorAdmissionTicket>,
    ) -> Result<(), Box<NetworkActorAdmissionError<Post<NetworkMessage>>>>;
    /// Admit a reply over the exact authenticated route of its request.
    fn post_reply_recoverable(
        &self,
        msg: Post<NetworkMessage>,
        reply_route: &Self::ReplyRoute,
        ticket: Option<NetworkActorAdmissionTicket>,
    ) -> Result<NetworkReplyAdmissionOutcome, Box<NetworkActorAdmissionError<Post<NetworkMessage>>>>;
    /// Extract the authenticated reply route carried by one inbound message.
    fn reply_route(&self, message: &PeerMessage<NetworkMessage>) -> Option<Self::ReplyRoute>;
    /// Derive the stable authenticated-source owner of an opaque reply route.
    fn reply_source(&self, reply_route: &Self::ReplyRoute) -> Self::ReplySource;
    /// Subscribe to peer messages delivered by the P2P layer.
    fn subscribe(
        &self,
        sender: mpsc::Sender<PeerMessage<NetworkMessage>>,
    ) -> Result<(), mpsc::Sender<PeerMessage<NetworkMessage>>>;
    /// Subscribe to peer messages delivered by the P2P layer using a topic filter.
    fn subscribe_with_filter(
        &self,
        sender: mpsc::Sender<PeerMessage<NetworkMessage>>,
        filter: SubscriberFilter,
    ) -> Result<(), mpsc::Sender<PeerMessage<NetworkMessage>>> {
        let _ = filter;
        self.subscribe(sender)
    }
    /// Configured queue capacity for P2P subscribers.
    fn subscriber_queue_cap(&self) -> NonZeroUsize;
    /// Update the gossip topology (used to seed trusted peers for bootstrap).
    fn update_topology(&self, update: UpdateTopology);
    /// Update peer addresses (used to seed trusted peers for bootstrap).
    fn update_peers_addresses(&self, update: UpdatePeers);
}

impl GenesisNetwork for IrohaNetwork {
    type ReplyRoute = NetworkReplyRoute;
    type ReplySource = iroha_p2p::network::NetworkReplySourceKey;

    fn post_recoverable(
        &self,
        msg: Post<NetworkMessage>,
        ticket: Option<NetworkActorAdmissionTicket>,
    ) -> Result<(), Box<NetworkActorAdmissionError<Post<NetworkMessage>>>> {
        iroha_p2p::network::NetworkBaseHandle::post_recoverable(self, msg, ticket).map_err(Box::new)
    }

    fn post_reply_recoverable(
        &self,
        msg: Post<NetworkMessage>,
        reply_route: &NetworkReplyRoute,
        ticket: Option<NetworkActorAdmissionTicket>,
    ) -> Result<NetworkReplyAdmissionOutcome, Box<NetworkActorAdmissionError<Post<NetworkMessage>>>>
    {
        iroha_p2p::network::NetworkBaseHandle::post_reply_recoverable(
            self,
            msg,
            reply_route,
            ticket,
        )
        .map_err(Box::new)
    }

    fn reply_route(&self, message: &PeerMessage<NetworkMessage>) -> Option<Self::ReplyRoute> {
        message.reply_route().cloned()
    }

    fn reply_source(&self, reply_route: &Self::ReplyRoute) -> Self::ReplySource {
        reply_route.source_key()
    }

    fn subscribe(
        &self,
        sender: mpsc::Sender<PeerMessage<NetworkMessage>>,
    ) -> Result<(), mpsc::Sender<PeerMessage<NetworkMessage>>> {
        self.subscribe_to_peers_messages(sender)
    }

    fn subscribe_with_filter(
        &self,
        sender: mpsc::Sender<PeerMessage<NetworkMessage>>,
        filter: SubscriberFilter,
    ) -> Result<(), mpsc::Sender<PeerMessage<NetworkMessage>>> {
        self.subscribe_to_peers_messages_with_filter(sender, filter)
    }

    fn subscriber_queue_cap(&self) -> NonZeroUsize {
        self.subscriber_queue_cap()
    }

    fn update_topology(&self, update: UpdateTopology) {
        self.update_topology(update);
    }

    fn update_peers_addresses(&self, update: UpdatePeers) {
        self.update_peers_addresses(update);
    }
}

/// Encoded genesis payload and associated metadata.
#[derive(Clone)]
pub struct GenesisPayload {
    /// Parsed genesis block.
    pub block: GenesisBlock,
    /// Canonical Norito encoding of the genesis block.
    pub bytes: Vec<u8>,
    /// Hash of the genesis header.
    pub hash: HashOf<BlockHeader>,
    /// Public key that signed the genesis payload.
    pub signer: PublicKey,
}

impl GenesisPayload {
    /// Build a payload from a signed genesis block.
    pub fn from_block(
        block: &GenesisBlock,
        expected_pubkey: &PublicKey,
    ) -> Result<Self, BootstrapError> {
        let hash = block.0.hash();
        let wire = block
            .0
            .canonical_wire()
            .map_err(|err| BootstrapError::Decode(err.to_string()))?;
        let Some(signature) = block.0.signatures().next() else {
            return Err(BootstrapError::InvalidGenesis(
                "genesis block missing signature".into(),
            ));
        };
        signature
            .signature()
            .verify_hash(expected_pubkey, hash)
            .map_err(|_| BootstrapError::InvalidGenesis("invalid genesis signature".into()))?;
        Ok(Self {
            block: block.clone(),
            bytes: wire.into_vec(),
            hash,
            signer: expected_pubkey.clone(),
        })
    }

    /// Length of the canonical payload.
    pub fn size_bytes(&self) -> u64 {
        u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
    }
}

/// Result of a successful bootstrap fetch.
#[derive(Clone)]
pub struct FetchResult {
    /// Parsed genesis block.
    pub block: GenesisBlock,
    /// Canonical encoded bytes.
    pub bytes: Vec<u8>,
    /// Hash of the genesis header.
    pub hash: HashOf<BlockHeader>,
}

/// Error taxonomy for genesis bootstrap.
#[derive(Debug)]
pub enum BootstrapError {
    /// No peer responded within the allotted window.
    NoResponse,
    /// Another clone already owns the single bounded fetch producer.
    FetchAlreadyActive,
    /// Conflicting genesis hashes were observed across peers.
    ConflictingHashes,
    /// Peer advertised a hash that did not match the expected one.
    HashMismatch {
        /// Expected hash from config or preflight.
        expected: HashOf<BlockHeader>,
        /// Hash returned by peer.
        got: HashOf<BlockHeader>,
    },
    /// Peer advertised a signer that did not match the configured genesis key.
    SignerMismatch {
        /// Expected signer public key from config.
        expected: PublicKey,
        /// Signer returned by peer (if advertised).
        advertised: Option<PublicKey>,
    },
    /// Peer advertised a payload size exceeding the local cap.
    PayloadTooLarge {
        /// Size hint from peer.
        hint: u64,
        /// Local size cap.
        cap: u64,
    },
    /// Peer returned a payload that failed to decode.
    Decode(String),
    /// Peer returned a payload that failed structural validation.
    InvalidGenesis(String),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoResponse => write!(f, "no genesis response from trusted peers"),
            Self::FetchAlreadyActive => {
                write!(f, "another genesis bootstrap fetch is already active")
            }
            Self::ConflictingHashes => write!(f, "peers returned conflicting genesis hashes"),
            Self::HashMismatch { expected, got } => write!(
                f,
                "expected genesis hash {expected:?} but peer responded with {got:?}"
            ),
            Self::SignerMismatch {
                expected,
                advertised,
            } => {
                if let Some(advertised) = advertised {
                    write!(
                        f,
                        "genesis signer mismatch: expected {expected}, got {advertised}"
                    )
                } else {
                    write!(
                        f,
                        "genesis signer mismatch: expected {expected}, responder omitted signer"
                    )
                }
            }
            Self::PayloadTooLarge { hint, cap } => write!(
                f,
                "genesis payload exceeds allowed size (hint {hint} bytes, cap {cap} bytes)"
            ),
            Self::Decode(err) => write!(f, "failed to decode genesis payload: {err}"),
            Self::InvalidGenesis(err) => write!(f, "genesis payload rejected: {err}"),
        }
    }
}

impl std::error::Error for BootstrapError {}

struct GenesisProducerPermit {
    active: Arc<AtomicBool>,
}

impl GenesisProducerPermit {
    fn try_acquire(active: &Arc<AtomicBool>) -> Option<Self> {
        active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(Self {
            active: Arc::clone(active),
        })
    }
}

impl Drop for GenesisProducerPermit {
    fn drop(&mut self) {
        let was_active = self.active.swap(false, Ordering::Release);
        debug_assert!(
            was_active,
            "exclusive genesis producer permit released twice"
        );
    }
}

/// Request/response orchestration for genesis bootstrap.
#[derive(Clone)]
pub struct GenesisBootstrapper<N: GenesisNetwork = IrohaNetwork> {
    network: N,
    chain_id: ChainId,
    expected_pubkey: PublicKey,
    expected_hash: Option<HashOf<BlockHeader>>,
    max_bytes: u64,
    request_timeout: Duration,
    retry_interval: Duration,
    max_attempts: u32,
    throttle: Duration,
    enabled: bool,
    allowlist: HashSet<PeerId>,
    trusted: Arc<Mutex<HashSet<PeerId>>>,
    responder: Arc<Mutex<ResponderState>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<InboundResponse>>>>,
    response_queue_cap: NonZeroUsize,
    listener_active: Arc<AtomicBool>,
    fetch_active: Arc<AtomicBool>,
}

struct ResponderState {
    payload: Option<GenesisPayload>,
    last_response: HashMap<PeerId, LastGenesisResponse>,
    last_response_order: VecDeque<PeerId>,
}

struct LastGenesisResponse {
    at: Instant,
    request_id: u64,
    request_hash: HashOf<GenesisRequest>,
}

struct ResponderCtx<'a> {
    chain_id: &'a ChainId,
    allowlist: &'a HashSet<PeerId>,
    trusted_fallback: &'a HashSet<PeerId>,
    expected_pubkey: &'a PublicKey,
    expected_hash: Option<&'a HashOf<BlockHeader>>,
    max_bytes: u64,
    throttle: Duration,
    response_history_cap: usize,
}

struct InboundResponse {
    peer: Peer,
    response: GenesisResponse,
}

struct PreflightOutcome {
    hash: HashOf<BlockHeader>,
    responders: Vec<PeerId>,
    size_bytes: u64,
}

struct PendingGenesisRequestPost {
    message: Post<NetworkMessage>,
    ticket: Option<NetworkActorAdmissionTicket>,
}

struct PendingGenesisReplyPost<R> {
    message: Post<NetworkMessage>,
    ticket: Option<NetworkActorAdmissionTicket>,
    reply_route: R,
    encoded_bytes: u64,
}

/// Bounded round-robin retry ownership, isolated by authenticated transport source.
struct PendingGenesisReplies<R, S> {
    by_source: HashMap<S, VecDeque<PendingGenesisReplyPost<R>>>,
    source_order: VecDeque<S>,
    len: usize,
    capacity: usize,
    per_source_capacity: usize,
    encoded_bytes: u64,
    encoded_bytes_by_source: HashMap<S, u64>,
    encoded_byte_capacity: u64,
    encoded_byte_capacity_per_source: u64,
}

impl<R, S> PendingGenesisReplies<R, S>
where
    S: Clone + Eq + std::hash::Hash,
{
    fn new(capacity: NonZeroUsize, max_payload_bytes: u64) -> Self {
        const REPLY_ENVELOPE_ALLOWANCE: u64 = 64 * 1024;

        let capacity = capacity.get();
        // Preserve capacity for several independent authenticated sources while
        // still allowing a trusted hub to carry more than one bootstrap client.
        let per_source_capacity = genesis_reply_waiters_per_source(capacity)
            .expect("non-zero subscriber queue has an exact per-source genesis reply share");
        let encoded_byte_capacity_per_source =
            max_payload_bytes.saturating_add(REPLY_ENVELOPE_ALLOWANCE);
        let encoded_byte_capacity = encoded_byte_capacity_per_source.saturating_mul(2);
        Self {
            by_source: HashMap::new(),
            source_order: VecDeque::new(),
            len: 0,
            capacity,
            per_source_capacity,
            encoded_bytes: 0,
            encoded_bytes_by_source: HashMap::new(),
            encoded_byte_capacity,
            encoded_byte_capacity_per_source,
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push(
        &mut self,
        source: S,
        pending: PendingGenesisReplyPost<R>,
    ) -> Result<(), Box<PendingGenesisReplyPost<R>>> {
        let source_bytes = self
            .encoded_bytes_by_source
            .get(&source)
            .copied()
            .unwrap_or(0);
        let Some(next_bytes) = self.encoded_bytes.checked_add(pending.encoded_bytes) else {
            return Err(Box::new(pending));
        };
        let Some(next_source_bytes) = source_bytes.checked_add(pending.encoded_bytes) else {
            return Err(Box::new(pending));
        };
        if self.len >= self.capacity
            || self
                .by_source
                .get(&source)
                .is_some_and(|queue| queue.len() >= self.per_source_capacity)
            || next_bytes > self.encoded_byte_capacity
            || next_source_bytes > self.encoded_byte_capacity_per_source
        {
            return Err(Box::new(pending));
        }
        let queue = self.by_source.entry(source.clone()).or_insert_with(|| {
            self.source_order.push_back(source.clone());
            VecDeque::new()
        });
        queue.push_back(pending);
        self.len = self
            .len
            .checked_add(1)
            .expect("bounded genesis reply count cannot overflow");
        self.encoded_bytes = next_bytes;
        self.encoded_bytes_by_source
            .insert(source, next_source_bytes);
        Ok(())
    }

    fn pop(&mut self) -> Option<(S, PendingGenesisReplyPost<R>)> {
        let source = self.source_order.pop_front()?;
        let queue = self
            .by_source
            .get_mut(&source)
            .expect("genesis reply source order mirrors its map");
        let pending = queue
            .pop_front()
            .expect("genesis reply source queues are never empty");
        self.len = self
            .len
            .checked_sub(1)
            .expect("genesis reply count mirrors its queues");
        self.encoded_bytes = self
            .encoded_bytes
            .checked_sub(pending.encoded_bytes)
            .expect("genesis reply byte count mirrors its queues");
        let source_bytes = self
            .encoded_bytes_by_source
            .get_mut(&source)
            .expect("genesis reply source byte count mirrors its queues");
        *source_bytes = source_bytes
            .checked_sub(pending.encoded_bytes)
            .expect("genesis reply source byte count cannot underflow");
        if queue.is_empty() {
            self.by_source.remove(&source);
            self.encoded_bytes_by_source.remove(&source);
        } else {
            self.source_order.push_back(source.clone());
        }
        Some((source, pending))
    }
}

fn post_or_retain_genesis_reply<N: GenesisNetwork>(
    network: &N,
    pending: &mut PendingGenesisReplies<N::ReplyRoute, N::ReplySource>,
    response: PendingGenesisReplyPost<N::ReplyRoute>,
) -> bool {
    let PendingGenesisReplyPost {
        message,
        ticket,
        reply_route,
        encoded_bytes,
    } = response;
    let semantic_target = message.peer_id.clone();
    match network.post_reply_recoverable(message, &reply_route, ticket) {
        Ok(NetworkReplyAdmissionOutcome::Admitted) => true,
        Ok(NetworkReplyAdmissionOutcome::ReplyWriterUnavailable) => {
            iroha_logger::debug!(
                peer = %semantic_target,
                "genesis reply writer drained before admission; requester retransmission will rematerialize the response on a newly admitted route"
            );
            true
        }
        Err(error) => match *error {
            NetworkActorAdmissionError::Backpressured {
                message, ticket, ..
            } => {
                let source = network.reply_source(&reply_route);
                if let Err(dropped) = pending.push(
                    source,
                    PendingGenesisReplyPost {
                        message,
                        ticket,
                        reply_route,
                        encoded_bytes,
                    },
                ) {
                    iroha_logger::debug!(
                        peer = %dropped.message.peer_id,
                        "bounded genesis reply retry queue is full for this authenticated source; requester retransmission remains the response-rematerialization witness"
                    );
                }
                true
            }
            NetworkActorAdmissionError::Closed { .. } => {
                iroha_logger::warn!("genesis response corridor closed with the P2P actor");
                false
            }
            NetworkActorAdmissionError::Rejected {
                message, reason, ..
            } => {
                iroha_logger::warn!(
                    peer = %message.peer_id,
                    ?reason,
                    "genesis response permanently rejected by P2P actor admission"
                );
                true
            }
        },
    }
}

struct GenesisRequestTarget {
    peer_id: PeerId,
    pending: Option<PendingGenesisRequestPost>,
    permanently_rejected: bool,
}

struct GenesisRequestFanout {
    message: NetworkMessage,
    targets: Vec<GenesisRequestTarget>,
}

impl GenesisRequestFanout {
    fn new(peers: &[PeerId], request: GenesisRequest) -> Self {
        let mut seen_targets = HashSet::with_capacity(peers.len());
        Self {
            message: NetworkMessage::GenesisRequest(Box::new(request)),
            targets: peers
                .iter()
                .filter(|peer_id| seen_targets.insert((**peer_id).clone()))
                .cloned()
                .map(|peer_id| GenesisRequestTarget {
                    peer_id,
                    pending: None,
                    permanently_rejected: false,
                })
                .collect(),
        }
    }

    /// Try every target once without allowing a backpressured target to lose
    /// either its exact message or FIFO actor-admission position.
    fn service<N: GenesisNetwork>(&mut self, network: &N) -> bool {
        let mut actor_open = true;
        for target in &mut self.targets {
            if target.permanently_rejected {
                continue;
            }
            let PendingGenesisRequestPost { message, ticket } = target
                .pending
                .take()
                .unwrap_or_else(|| PendingGenesisRequestPost {
                    message: Post {
                        data: self.message.clone(),
                        peer_id: target.peer_id.clone(),
                        priority: Priority::High,
                    },
                    ticket: None,
                });
            match network.post_recoverable(message, ticket) {
                Ok(()) => {}
                Err(error) => match *error {
                    NetworkActorAdmissionError::Backpressured {
                        message, ticket, ..
                    } => {
                        target.pending = Some(PendingGenesisRequestPost { message, ticket });
                    }
                    NetworkActorAdmissionError::Closed { .. } => {
                        actor_open = false;
                        break;
                    }
                    NetworkActorAdmissionError::Rejected {
                        message, reason, ..
                    } => {
                        target.permanently_rejected = true;
                        iroha_logger::warn!(
                            peer = %message.peer_id,
                            ?reason,
                            "genesis request permanently rejected by P2P actor admission"
                        );
                    }
                },
            }
        }
        actor_open
    }
}

impl<N: GenesisNetwork> GenesisBootstrapper<N> {
    /// Construct a new bootstrapper.
    pub fn new(config: &GenesisConfig, network: N, chain_id: ChainId) -> Self {
        let allowlist: HashSet<_> = config.bootstrap_allowlist.iter().cloned().collect();
        let response_queue_cap = network.subscriber_queue_cap();
        Self {
            network,
            chain_id,
            expected_pubkey: config.public_key.clone(),
            expected_hash: config.expected_hash,
            max_bytes: config.bootstrap_max_bytes,
            request_timeout: config.bootstrap_request_timeout,
            retry_interval: config.bootstrap_retry_interval,
            max_attempts: config.bootstrap_max_attempts.max(1),
            throttle: config.bootstrap_response_throttle,
            enabled: config.bootstrap_enabled,
            allowlist,
            trusted: Arc::new(Mutex::new(HashSet::new())),
            responder: Arc::new(Mutex::new(ResponderState {
                payload: None,
                last_response: HashMap::new(),
                last_response_order: VecDeque::new(),
            })),
            pending: Arc::new(Mutex::new(HashMap::new())),
            response_queue_cap,
            listener_active: Arc::new(AtomicBool::new(false)),
            fetch_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Seed the network topology and address book so bootstrap requests can be dispatched immediately.
    pub fn seed_topology(&self, peers: &[(PeerId, SocketAddr)]) {
        let ids: HashSet<_> = peers.iter().map(|(id, _)| id.clone()).collect();
        self.network.update_topology(UpdateTopology(ids.clone()));
        self.network
            .update_peers_addresses(UpdatePeers(peers.to_vec()));
        if let Ok(mut guard) = self.trusted.lock() {
            guard.extend(ids);
        }
    }

    /// Spawn a listener that handles inbound genesis requests/responses.
    pub async fn spawn_listener(&self) {
        let Some(listener_permit) = GenesisProducerPermit::try_acquire(&self.listener_active)
        else {
            iroha_logger::warn!(
                "genesis bootstrap listener is already active; refusing to multiply its bounded reply producer"
            );
            return;
        };
        let (mut sender, mut rx) = mpsc::channel(self.network.subscriber_queue_cap().get());
        let filter = genesis_message_filter();
        let mut backoff_ms = 50;
        while let Err(returned) = self.network.subscribe_with_filter(sender, filter.clone()) {
            sender = returned;
            time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(500);
        }
        let responder = Arc::clone(&self.responder);
        let allowlist = self.allowlist.clone();
        let trusted = Arc::clone(&self.trusted);
        let max_bytes = self.max_bytes;
        let throttle = self.throttle;
        let expected_pubkey = self.expected_pubkey.clone();
        let expected_hash = self.expected_hash;
        let response_history_cap = self.response_queue_cap.get();
        let reply_queue_cap = self.response_queue_cap;
        let chain_id = self.chain_id.clone();
        let pending = Arc::clone(&self.pending);
        let network = self.network.clone();
        let retry_period = self.retry_interval.max(Duration::from_millis(1));
        tokio::spawn(async move {
            let _listener_permit = listener_permit;
            let mut pending_replies = PendingGenesisReplies::new(reply_queue_cap, max_bytes);
            let mut retry_tick = time::interval(retry_period);
            retry_tick.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            let mut rx_open = true;
            'listener: loop {
                // A due retry has deterministic precedence over a saturated
                // inbound stream. `Delay` schedules the following tick in the
                // future, so the precedence cannot starve inbound handling.
                let msg = tokio::select! {
                    biased;
                    _ = retry_tick.tick(), if !pending_replies.is_empty() => {
                        let (_source, response) = pending_replies
                            .pop()
                            .expect("non-empty genesis reply scheduler has one item");
                        if !post_or_retain_genesis_reply(
                            &network,
                            &mut pending_replies,
                            response,
                        ) {
                            break 'listener;
                        }
                        None
                    }
                    message = rx.recv(), if rx_open => {
                        if let Some(message) = message {
                            Some(message)
                        } else {
                            rx_open = false;
                            None
                        }
                    }
                    else => break 'listener,
                };
                let Some(msg) = msg else {
                    continue;
                };
                let reply_route = network.reply_route(&msg);
                match msg.payload {
                    NetworkMessage::GenesisRequest(request) => {
                        let trusted_guard = trusted
                            .lock()
                            .unwrap_or_else(|_| panic!("bootstrap trusted guard poisoned"));
                        let response = responder
                            .lock()
                            .expect("responder mutex poisoned")
                            .prepare_response(
                                &msg.peer,
                                request.as_ref(),
                                &ResponderCtx {
                                    chain_id: &chain_id,
                                    expected_pubkey: &expected_pubkey,
                                    expected_hash: expected_hash.as_ref(),
                                    allowlist: &allowlist,
                                    trusted_fallback: &*trusted_guard,
                                    max_bytes,
                                    throttle,
                                    response_history_cap,
                                },
                            );
                        if let Some(response) = response {
                            if let Some(err) = response.error {
                                debug!(
                                    %msg.peer,
                                    ?err,
                                    request_id = response.request_id,
                                    "denying genesis request"
                                );
                            }
                            let post = Post {
                                data: NetworkMessage::GenesisResponse(Box::new(response)),
                                peer_id: msg.peer.id().clone(),
                                priority: Priority::High,
                            };
                            let Some(reply_route) = reply_route else {
                                iroha_logger::debug!(
                                    peer = %msg.peer,
                                    "dropping genesis response without authenticated reply route"
                                );
                                continue;
                            };
                            if !post_or_retain_genesis_reply(
                                &network,
                                &mut pending_replies,
                                PendingGenesisReplyPost {
                                    encoded_bytes: u64::try_from(post.data.encode().len())
                                        .unwrap_or(u64::MAX),
                                    message: post,
                                    ticket: None,
                                    reply_route,
                                },
                            ) {
                                break 'listener;
                            }
                        }
                    }
                    NetworkMessage::GenesisResponse(response) => {
                        let guard = pending.lock().expect("pending mutex poisoned");
                        if let Some(sender) = guard.get(&response.request_id) {
                            if let Ok(permit) = sender.try_reserve() {
                                permit.send(InboundResponse {
                                    peer: msg.peer.clone(),
                                    response: response.as_ref().clone(),
                                });
                            } else {
                                iroha_logger::debug!(
                                    %msg.peer,
                                    request_id = response.request_id,
                                    "bounded genesis response queue could not admit a duplicate; requester retransmission remains active"
                                );
                            }
                        } else {
                            debug!(
                                %msg.peer,
                                request_id = response.request_id,
                                "received genesis response for unknown request"
                            );
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    /// Record a validated genesis payload so future requests can be served.
    pub async fn set_payload(&self, block: &GenesisBlock) -> Result<(), BootstrapError> {
        let payload = GenesisPayload::from_block(block, &self.expected_pubkey)?;
        if let Some(expected) = &self.expected_hash
            && expected != &payload.hash
        {
            return Err(BootstrapError::HashMismatch {
                expected: *expected,
                got: payload.hash,
            });
        }
        let mut guard = self.responder.lock().expect("responder mutex poisoned");
        guard.payload = Some(payload);
        Ok(())
    }

    /// Fetch genesis from trusted peers using the bootstrap protocol.
    ///
    /// When bootstrap is enabled, transient no-response windows are retried across diagnostic
    /// cycles so a partition that heals after the first cycle cannot permanently fail startup.
    /// Permanent validation errors are returned to the caller.
    pub async fn fetch_genesis(
        &self,
        peers: &[PeerId],
        genesis_account: &AccountId,
        expected_hash: Option<HashOf<BlockHeader>>,
    ) -> Result<FetchResult, BootstrapError> {
        if !self.enabled {
            return Err(BootstrapError::NoResponse);
        }
        let Some(_fetch_permit) = GenesisProducerPermit::try_acquire(&self.fetch_active) else {
            return Err(BootstrapError::FetchAlreadyActive);
        };
        let expected_hash = expected_hash.or(self.expected_hash);
        let mut no_response_windows = 0_u32;
        loop {
            match self.try_preflight(peers, expected_hash).await {
                Ok(preflight) => {
                    let payload = self
                        .request_payload(&preflight, peers, genesis_account)
                        .await?;
                    return Ok(payload);
                }
                Err(BootstrapError::NoResponse) => {
                    no_response_windows = no_response_windows.saturating_add(1);
                    if no_response_windows.is_multiple_of(self.max_attempts) {
                        iroha_logger::warn!(
                            no_response_windows,
                            "genesis bootstrap remains pending; continuing after the configured diagnostic retry cycle"
                        );
                    }
                    time::sleep(self.retry_interval).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn try_preflight(
        &self,
        peers: &[PeerId],
        expected_hash: Option<HashOf<BlockHeader>>,
    ) -> Result<PreflightOutcome, BootstrapError> {
        if peers.is_empty() {
            return Err(BootstrapError::NoResponse);
        }
        let request_id = next_request_id();
        let mut rx = self.register_request(request_id).await;
        let request = GenesisRequest {
            request_id,
            chain_id: self.chain_id.clone(),
            expected_hash,
            expected_pubkey: Some(self.expected_pubkey.clone()),
            kind: GenesisRequestKind::Preflight,
        };
        let mut fanout = GenesisRequestFanout::new(peers, request);
        if !fanout.service(&self.network) {
            self.unregister_request(request_id).await;
            return Err(BootstrapError::NoResponse);
        }
        let deadline = Instant::now() + self.request_timeout;
        let retry_period = self.retry_interval.max(Duration::from_millis(1));
        let mut retry = time::interval_at(Instant::now() + retry_period, retry_period);
        retry.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        let mut hashes = HashSet::new();
        let mut responders: Vec<PeerId> = Vec::new();
        let mut selected: Option<(HashOf<BlockHeader>, u64)> = None;
        loop {
            let inbound = tokio::select! {
                biased;
                () = time::sleep_until(deadline) => break,
                inbound = rx.recv() => {
                    let Some(inbound) = inbound else {
                        break;
                    };
                    inbound
                }
                _ = retry.tick() => {
                    // Keep the same registered request id live until its deadline. A dropped
                    // actor admission or response can therefore be rematerialized from this
                    // caller-owned request instead of consuming an attempt irreversibly.
                    if !fanout.service(&self.network) {
                        break;
                    }
                    continue;
                }
            };
            match validate_preflight_response(
                &inbound.response,
                &self.expected_pubkey,
                expected_hash,
                self.max_bytes,
                &self.chain_id,
            ) {
                Ok(Some(validated)) => {
                    hashes.insert(validated.hash);
                    if !responders.contains(&inbound.peer.id) {
                        responders.push(inbound.peer.id.clone());
                    }
                    if let Some((selected_hash, size)) = &selected {
                        if selected_hash != &validated.hash {
                            self.unregister_request(request_id).await;
                            return Err(BootstrapError::ConflictingHashes);
                        }
                        if validated.size_bytes > *size {
                            selected = Some((validated.hash, validated.size_bytes));
                        }
                    } else {
                        selected = Some((validated.hash, validated.size_bytes));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    self.unregister_request(request_id).await;
                    return Err(err);
                }
            }
        }
        self.unregister_request(request_id).await;
        if hashes.len() > 1 {
            return Err(BootstrapError::ConflictingHashes);
        }
        if let Some((hash, size_bytes)) = selected {
            return Ok(PreflightOutcome {
                hash,
                responders,
                size_bytes,
            });
        }
        Err(BootstrapError::NoResponse)
    }

    async fn request_payload(
        &self,
        preflight: &PreflightOutcome,
        peers: &[PeerId],
        genesis_account: &AccountId,
    ) -> Result<FetchResult, BootstrapError> {
        let initial_backoff = self.retry_interval.max(Duration::from_millis(100));
        let max_backoff = self.request_timeout.max(initial_backoff);
        let mut backoff = initial_backoff;
        let mut no_response_windows = 0_u32;
        loop {
            let request_id = next_request_id();
            let mut rx = self.register_request(request_id).await;
            let request = GenesisRequest {
                request_id,
                chain_id: self.chain_id.clone(),
                expected_hash: Some(preflight.hash),
                expected_pubkey: Some(self.expected_pubkey.clone()),
                kind: GenesisRequestKind::Fetch,
            };
            // Prefer peers that answered preflight, but retain every configured peer as a
            // recovery source. A responder may fail after advertising metadata while another
            // peer becomes responsive only after GST.
            let mut targets = preflight.responders.clone();
            for peer in peers {
                if !targets.contains(peer) {
                    targets.push(peer.clone());
                }
            }
            let mut fanout = GenesisRequestFanout::new(&targets, request);
            if !fanout.service(&self.network) {
                self.unregister_request(request_id).await;
                return Err(BootstrapError::NoResponse);
            }
            let deadline = Instant::now() + self.request_timeout;
            let retry_period = self.retry_interval.max(Duration::from_millis(1));
            let mut retry = time::interval_at(Instant::now() + retry_period, retry_period);
            retry.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            loop {
                let inbound = tokio::select! {
                    biased;
                    () = time::sleep_until(deadline) => break,
                    inbound = rx.recv() => {
                        let Some(inbound) = inbound else {
                            break;
                        };
                        inbound
                    }
                    _ = retry.tick() => {
                        if !fanout.service(&self.network) {
                            break;
                        }
                        continue;
                    }
                };
                match validate_payload_response(
                    &inbound.response,
                    &self.chain_id,
                    &self.expected_pubkey,
                    &preflight.hash,
                    self.max_bytes,
                ) {
                    Ok(Some((block, bytes))) => {
                        self.unregister_request(request_id).await;
                        validate_genesis_block(&block.0, genesis_account, &self.chain_id)
                            .map_err(|err| BootstrapError::InvalidGenesis(err.to_string()))?;
                        return Ok(FetchResult {
                            hash: preflight.hash.clone(),
                            block,
                            bytes,
                        });
                    }
                    Ok(None) => {}
                    Err(err) => {
                        self.unregister_request(request_id).await;
                        return Err(err);
                    }
                }
            }
            self.unregister_request(request_id).await;
            time::sleep(backoff).await;
            no_response_windows = no_response_windows.saturating_add(1);
            if no_response_windows.is_multiple_of(self.max_attempts) {
                iroha_logger::warn!(
                    no_response_windows,
                    "genesis payload fetch remains pending; resetting bounded backoff and continuing"
                );
                backoff = initial_backoff;
            } else {
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }

    async fn register_request(&self, request_id: u64) -> mpsc::Receiver<InboundResponse> {
        let (tx, rx) = mpsc::channel(self.response_queue_cap.get());
        let mut guard = self.pending.lock().expect("pending mutex poisoned");
        guard.insert(request_id, tx);
        rx
    }

    async fn unregister_request(&self, request_id: u64) {
        let mut guard = self.pending.lock().expect("pending mutex poisoned");
        guard.remove(&request_id);
    }
}

impl ResponderState {
    fn prepare_response(
        &mut self,
        peer: &Peer,
        request: &GenesisRequest,
        ctx: &ResponderCtx<'_>,
    ) -> Option<GenesisResponse> {
        let allowed = if !ctx.allowlist.is_empty() {
            ctx.allowlist.contains(peer.id())
        } else if !ctx.trusted_fallback.is_empty() {
            ctx.trusted_fallback.contains(peer.id())
        } else {
            true
        };
        if !allowed {
            return Some(error_response(
                ctx.chain_id.clone(),
                request.request_id,
                GenesisResponseError::NotAllowed,
            ));
        }
        if request.chain_id != *ctx.chain_id {
            return Some(error_response(
                ctx.chain_id.clone(),
                request.request_id,
                GenesisResponseError::MismatchedChain,
            ));
        }
        if ctx.throttle != Duration::ZERO {
            let now = Instant::now();
            let request_hash = HashOf::new(request);
            // An exact request is deliberately idempotent: a requester keeps one canonical
            // request registered and retransmits it until its deadline, so losing the first
            // response cannot poison that request. Binding the exemption to the full request hash
            // prevents same-id/different-body reuse from bypassing the throttle. The one-record-
            // per-peer cache contains no genesis bytes and is retained only for the active
            // throttle window, so it cannot become historical unbounded state.
            while let Some(oldest_peer) = self.last_response_order.front() {
                let expired = self
                    .last_response
                    .get(oldest_peer)
                    .is_none_or(|last| now.saturating_duration_since(last.at) >= ctx.throttle);
                if !expired {
                    break;
                }
                let oldest_peer = self
                    .last_response_order
                    .pop_front()
                    .expect("front entry exists");
                self.last_response.remove(&oldest_peer);
            }
            if let Some(last) = self.last_response.get(peer.id()) {
                if last.request_id != request.request_id || last.request_hash != request_hash {
                    return Some(error_response(
                        ctx.chain_id.clone(),
                        request.request_id,
                        GenesisResponseError::RateLimited,
                    ));
                }
            } else {
                while self.last_response.len() >= ctx.response_history_cap {
                    let oldest_peer = self
                        .last_response_order
                        .pop_front()
                        .expect("bounded response history order mirrors its map");
                    self.last_response.remove(&oldest_peer);
                }
                self.last_response.insert(
                    peer.id().clone(),
                    LastGenesisResponse {
                        at: now,
                        request_id: request.request_id,
                        request_hash,
                    },
                );
                self.last_response_order.push_back(peer.id().clone());
            }
        } else {
            self.last_response.clear();
            self.last_response_order.clear();
        }

        let Some(payload) = self.payload.clone() else {
            return Some(error_response(
                ctx.chain_id.clone(),
                request.request_id,
                GenesisResponseError::MissingGenesis,
            ));
        };

        let size_bytes = payload.size_bytes();
        if size_bytes > ctx.max_bytes {
            return Some(metadata_response(
                ctx.chain_id.clone(),
                request.request_id,
                payload.signer.clone(),
                Some(payload.hash),
                Some(size_bytes),
                Some(GenesisResponseError::TooLarge),
            ));
        }
        if &payload.signer != ctx.expected_pubkey {
            return Some(metadata_response(
                ctx.chain_id.clone(),
                request.request_id,
                payload.signer.clone(),
                Some(payload.hash),
                Some(size_bytes),
                Some(GenesisResponseError::MismatchedPubkey),
            ));
        }
        if let Some(expect) = ctx.expected_hash {
            if expect != &payload.hash {
                return Some(metadata_response(
                    ctx.chain_id.clone(),
                    request.request_id,
                    payload.signer.clone(),
                    Some(payload.hash),
                    Some(size_bytes),
                    Some(GenesisResponseError::MismatchedHash),
                ));
            }
        }

        if let Some(expect) = &request.expected_pubkey {
            if expect != &payload.signer {
                return Some(metadata_response(
                    ctx.chain_id.clone(),
                    request.request_id,
                    payload.signer.clone(),
                    Some(payload.hash),
                    Some(size_bytes),
                    Some(GenesisResponseError::MismatchedPubkey),
                ));
            }
        }
        if let Some(expect) = request.expected_hash {
            if expect != payload.hash {
                return Some(metadata_response(
                    ctx.chain_id.clone(),
                    request.request_id,
                    payload.signer.clone(),
                    Some(payload.hash),
                    Some(size_bytes),
                    Some(GenesisResponseError::MismatchedHash),
                ));
            }
        }

        match request.kind {
            GenesisRequestKind::Preflight => Some(metadata_response(
                ctx.chain_id.clone(),
                request.request_id,
                payload.signer,
                Some(payload.hash),
                Some(size_bytes),
                None,
            )),
            GenesisRequestKind::Fetch => Some(full_response(
                ctx.chain_id.clone(),
                request.request_id,
                payload.signer,
                payload.hash,
                size_bytes,
                payload.bytes,
            )),
        }
    }
}

struct ValidatedPreflight {
    hash: HashOf<BlockHeader>,
    size_bytes: u64,
}

fn validate_preflight_response(
    response: &GenesisResponse,
    expected_pubkey: &PublicKey,
    expected_hash: Option<HashOf<BlockHeader>>,
    max_bytes: u64,
    chain_id: &ChainId,
) -> Result<Option<ValidatedPreflight>, BootstrapError> {
    if response.chain_id != *chain_id {
        return Ok(None);
    }
    if let Some(error) = response.error {
        return match error {
            GenesisResponseError::NotAllowed
            | GenesisResponseError::RateLimited
            | GenesisResponseError::MissingGenesis
            | GenesisResponseError::MismatchedChain
            | GenesisResponseError::DuplicateRequest => Ok(None),
            GenesisResponseError::MismatchedPubkey => Err(BootstrapError::SignerMismatch {
                expected: expected_pubkey.clone(),
                advertised: response.public_key.clone(),
            }),
            GenesisResponseError::MismatchedHash => {
                if let Some(expected) = expected_hash {
                    let got = response.hash.clone().unwrap_or(expected);
                    Err(BootstrapError::HashMismatch { expected, got })
                } else {
                    Ok(None)
                }
            }
            GenesisResponseError::TooLarge => Err(BootstrapError::PayloadTooLarge {
                hint: response.size_hint.unwrap_or(max_bytes.saturating_add(1)),
                cap: max_bytes,
            }),
        };
    }
    let Some(hash) = response.hash.clone() else {
        return Ok(None);
    };
    let Some(size_bytes) = response.size_hint else {
        return Ok(None);
    };
    if size_bytes > max_bytes {
        return Err(BootstrapError::PayloadTooLarge {
            hint: size_bytes,
            cap: max_bytes,
        });
    }
    let advertised = response.public_key.as_ref();
    match advertised {
        Some(pubkey) if pubkey == expected_pubkey => {}
        Some(pubkey) => {
            return Err(BootstrapError::SignerMismatch {
                expected: expected_pubkey.clone(),
                advertised: Some(pubkey.clone()),
            });
        }
        None => {
            return Err(BootstrapError::SignerMismatch {
                expected: expected_pubkey.clone(),
                advertised: None,
            });
        }
    }
    if let Some(expected) = expected_hash {
        if expected != hash {
            return Err(BootstrapError::HashMismatch {
                expected,
                got: hash,
            });
        }
    }
    Ok(Some(ValidatedPreflight { hash, size_bytes }))
}

fn validate_payload_response(
    response: &GenesisResponse,
    chain_id: &ChainId,
    expected_pubkey: &PublicKey,
    expected_hash: &HashOf<BlockHeader>,
    max_bytes: u64,
) -> Result<Option<(GenesisBlock, Vec<u8>)>, BootstrapError> {
    if response.chain_id != *chain_id {
        return Ok(None);
    }
    if let Some(error) = response.error {
        return match error {
            GenesisResponseError::NotAllowed
            | GenesisResponseError::RateLimited
            | GenesisResponseError::MissingGenesis
            | GenesisResponseError::MismatchedChain
            | GenesisResponseError::DuplicateRequest => Ok(None),
            GenesisResponseError::TooLarge => Err(BootstrapError::PayloadTooLarge {
                hint: response.size_hint.unwrap_or(max_bytes.saturating_add(1)),
                cap: max_bytes,
            }),
            GenesisResponseError::MismatchedHash => Err(BootstrapError::HashMismatch {
                expected: expected_hash.clone(),
                got: response.hash.clone().unwrap_or(expected_hash.clone()),
            }),
            GenesisResponseError::MismatchedPubkey => Err(BootstrapError::SignerMismatch {
                expected: expected_pubkey.clone(),
                advertised: response.public_key.clone(),
            }),
        };
    }
    if let Some(size_bytes) = response.size_hint {
        if size_bytes > max_bytes {
            return Err(BootstrapError::PayloadTooLarge {
                hint: size_bytes,
                cap: max_bytes,
            });
        }
    }
    let advertised_pubkey =
        response
            .public_key
            .as_ref()
            .ok_or_else(|| BootstrapError::SignerMismatch {
                expected: expected_pubkey.clone(),
                advertised: None,
            })?;
    if advertised_pubkey != expected_pubkey {
        return Err(BootstrapError::SignerMismatch {
            expected: expected_pubkey.clone(),
            advertised: Some(advertised_pubkey.clone()),
        });
    }
    let Some(payload) = response.payload.clone() else {
        return Ok(None);
    };
    if (payload.len() as u64) > max_bytes {
        return Err(BootstrapError::PayloadTooLarge {
            hint: payload.len() as u64,
            cap: max_bytes,
        });
    }
    if let Some(advertised_hash) = response.hash.as_ref() {
        if advertised_hash != expected_hash {
            return Err(BootstrapError::HashMismatch {
                expected: expected_hash.clone(),
                got: advertised_hash.clone(),
            });
        }
    }
    let block = decode_payload(&payload)?;
    let block_hash = block.0.hash();
    if &block_hash != expected_hash {
        return Err(BootstrapError::HashMismatch {
            expected: expected_hash.clone(),
            got: block_hash,
        });
    }
    let Some(signature) = block.0.signatures().next() else {
        return Err(BootstrapError::InvalidGenesis(
            "genesis block missing signature".into(),
        ));
    };
    signature
        .signature()
        .verify_hash(expected_pubkey, block_hash)
        .map_err(|_| BootstrapError::InvalidGenesis("invalid genesis signature".into()))?;
    Ok(Some((block, payload)))
}

fn decode_payload(payload: &[u8]) -> Result<GenesisBlock, BootstrapError> {
    decode_framed_signed_block(payload)
        .map(GenesisBlock)
        .map_err(|err| BootstrapError::Decode(err.to_string()))
}

fn error_response(
    chain_id: ChainId,
    request_id: u64,
    error: GenesisResponseError,
) -> GenesisResponse {
    GenesisResponse {
        chain_id,
        request_id,
        public_key: None,
        hash: None,
        size_hint: None,
        payload: None,
        error: Some(error),
    }
}

fn metadata_response(
    chain_id: ChainId,
    request_id: u64,
    public_key: PublicKey,
    hash: Option<HashOf<BlockHeader>>,
    size_hint: Option<u64>,
    error: Option<GenesisResponseError>,
) -> GenesisResponse {
    GenesisResponse {
        chain_id,
        request_id,
        public_key: Some(public_key),
        hash,
        size_hint,
        payload: None,
        error,
    }
}

fn full_response(
    chain_id: ChainId,
    request_id: u64,
    public_key: PublicKey,
    hash: HashOf<BlockHeader>,
    size_hint: u64,
    bytes: Vec<u8>,
) -> GenesisResponse {
    GenesisResponse {
        chain_id,
        request_id,
        public_key: Some(public_key),
        hash: Some(hash),
        size_hint: Some(size_hint),
        payload: Some(bytes),
        error: None,
    }
}

fn next_request_id() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{Level, block::SignedBlock, isi::Log};
    use iroha_p2p::peer::message::PeerMessage;
    use norito::codec::Encode as NoritoEncode;

    #[derive(Clone)]
    struct MockNetwork {
        sender: Arc<Mutex<Option<mpsc::Sender<PeerMessage<NetworkMessage>>>>>,
        posted: Arc<Mutex<Vec<Post<NetworkMessage>>>>,
        backpressure_remaining: Arc<std::sync::atomic::AtomicUsize>,
        blocked_reply_peers: Arc<Mutex<HashSet<PeerId>>>,
        unavailable_reply_peers: Arc<Mutex<HashSet<PeerId>>>,
        subscriptions: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Default for MockNetwork {
        fn default() -> Self {
            Self {
                sender: Arc::new(Mutex::new(None)),
                posted: Arc::new(Mutex::new(Vec::new())),
                backpressure_remaining: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                blocked_reply_peers: Arc::new(Mutex::new(HashSet::new())),
                unavailable_reply_peers: Arc::new(Mutex::new(HashSet::new())),
                subscriptions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }
    }

    impl MockNetwork {
        fn push_response(&self, peer: Peer, response: GenesisResponse) {
            if let Some(tx) = self.sender.lock().expect("sender mutex").as_ref() {
                let size = NoritoEncode::encode(&response).len();
                let payload = NetworkMessage::GenesisResponse(Box::new(response));
                let msg = PeerMessage::new(peer, payload, size);
                let _ = tx.try_send(msg);
            }
        }

        fn backpressure_next_posts(&self, count: usize) {
            self.backpressure_remaining
                .store(count, std::sync::atomic::Ordering::SeqCst);
        }

        fn block_reply_peer(&self, peer: PeerId) {
            self.blocked_reply_peers
                .lock()
                .expect("blocked reply peers")
                .insert(peer);
        }

        fn unblock_reply_peer(&self, peer: &PeerId) {
            self.blocked_reply_peers
                .lock()
                .expect("blocked reply peers")
                .remove(peer);
        }

        fn make_reply_writer_unavailable(&self, peer: PeerId) {
            self.unavailable_reply_peers
                .lock()
                .expect("unavailable reply peers")
                .insert(peer);
        }
    }

    impl GenesisNetwork for MockNetwork {
        type ReplyRoute = PeerId;
        type ReplySource = PeerId;

        fn post_recoverable(
            &self,
            msg: Post<NetworkMessage>,
            _ticket: Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), Box<NetworkActorAdmissionError<Post<NetworkMessage>>>> {
            if self
                .backpressure_remaining
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |remaining| remaining.checked_sub(1),
                )
                .is_ok()
            {
                return Err(Box::new(NetworkActorAdmissionError::Backpressured {
                    message: msg,
                    ticket: None,
                    rank: 1,
                }));
            }
            self.posted.lock().expect("posted mutex").push(msg);
            Ok(())
        }

        fn post_reply_recoverable(
            &self,
            msg: Post<NetworkMessage>,
            reply_route: &Self::ReplyRoute,
            ticket: Option<NetworkActorAdmissionTicket>,
        ) -> Result<
            NetworkReplyAdmissionOutcome,
            Box<NetworkActorAdmissionError<Post<NetworkMessage>>>,
        > {
            if self
                .unavailable_reply_peers
                .lock()
                .expect("unavailable reply peers")
                .contains(reply_route)
            {
                drop((msg, ticket));
                return Ok(NetworkReplyAdmissionOutcome::ReplyWriterUnavailable);
            }
            if self
                .blocked_reply_peers
                .lock()
                .expect("blocked reply peers")
                .contains(reply_route)
            {
                return Err(Box::new(NetworkActorAdmissionError::Backpressured {
                    message: msg,
                    ticket,
                    rank: 1,
                }));
            }
            self.post_recoverable(msg, ticket)
                .map(|()| NetworkReplyAdmissionOutcome::Admitted)
        }

        fn reply_route(&self, message: &PeerMessage<NetworkMessage>) -> Option<Self::ReplyRoute> {
            Some(message.peer.id().clone())
        }

        fn reply_source(&self, reply_route: &Self::ReplyRoute) -> Self::ReplySource {
            reply_route.clone()
        }

        fn subscribe(
            &self,
            sender: mpsc::Sender<PeerMessage<NetworkMessage>>,
        ) -> Result<(), mpsc::Sender<PeerMessage<NetworkMessage>>> {
            self.subscriptions
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            *self.sender.lock().expect("sender mutex") = Some(sender);
            Ok(())
        }

        fn subscriber_queue_cap(&self) -> NonZeroUsize {
            NonZeroUsize::new(8).expect("nonzero")
        }

        fn update_topology(&self, _update: UpdateTopology) {}

        fn update_peers_addresses(&self, _update: UpdatePeers) {}
    }

    fn checked_genesis_bootstrap_key_fixture() -> KeyPair {
        KeyPair::try_random().expect("generate checked genesis bootstrap key fixture")
    }

    #[test]
    fn genesis_bootstrap_fixture_uses_checked_random_key_generation() {
        let key_pair = checked_genesis_bootstrap_key_fixture();
        let algorithm = key_pair
            .public_key()
            .try_algorithm()
            .expect("genesis bootstrap fixture key advertises a valid algorithm");

        assert_eq!(algorithm, Algorithm::default());
    }

    fn sample_peer() -> Peer {
        let kp = checked_genesis_bootstrap_key_fixture();
        Peer::new(
            "127.0.0.1:1337".parse().expect("socket address"),
            kp.public_key().clone(),
        )
    }

    fn sample_block(chain_id: &ChainId, signer: &KeyPair) -> GenesisBlock {
        let tx = iroha_data_model::transaction::TransactionBuilder::new(
            chain_id.clone(),
            AccountId::new(signer.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "hello".to_owned())])
        .sign(signer.private_key());
        let signed_block = SignedBlock::genesis(vec![tx], signer.private_key(), None, None);
        GenesisBlock(signed_block)
    }

    fn pending_reply(peer: PeerId, encoded_bytes: u64) -> PendingGenesisReplyPost<PeerId> {
        PendingGenesisReplyPost {
            message: Post {
                data: NetworkMessage::Health,
                peer_id: peer.clone(),
                priority: Priority::High,
            },
            ticket: None,
            reply_route: peer,
            encoded_bytes,
        }
    }

    #[test]
    fn unavailable_reply_writer_uses_requester_retransmission_without_parking_old_route() {
        let network = MockNetwork::default();
        let peer = sample_peer().id().clone();
        network.make_reply_writer_unavailable(peer.clone());
        let mut pending =
            PendingGenesisReplies::new(NonZeroUsize::new(8).expect("non-zero queue"), 1024);

        assert!(post_or_retain_genesis_reply(
            &network,
            &mut pending,
            pending_reply(peer, 1),
        ));
        assert!(
            pending.is_empty(),
            "an obsolete reply route must not be parked forever"
        );
        assert!(
            network.posted.lock().expect("posted").is_empty(),
            "no-ownership outcome must not be reported as actor admission"
        );
    }

    #[test]
    fn pending_reply_bytes_are_bounded_per_source_without_consuming_other_source_capacity() {
        let first = sample_peer().id().clone();
        let second = sample_peer().id().clone();
        let mut pending =
            PendingGenesisReplies::new(NonZeroUsize::new(8).expect("non-zero queue"), 0);
        assert_eq!(
            pending.per_source_capacity,
            genesis_reply_waiters_per_source(8).expect("non-zero shared reply geometry"),
            "the runtime queue and actor waiter reserve must use one source-share derivation"
        );
        assert!(
            pending
                .push(first.clone(), pending_reply(first.clone(), 40_000))
                .is_ok()
        );
        assert!(
            pending
                .push(first.clone(), pending_reply(first, 40_000))
                .is_err(),
            "one source cannot retain more than its explicit byte budget"
        );
        assert!(
            pending
                .push(second.clone(), pending_reply(second, 40_000))
                .is_ok(),
            "an independent source retains its reserved byte ownership"
        );
        assert_eq!(pending.len, 2);
        assert_eq!(pending.encoded_bytes, 80_000);
    }

    #[test]
    fn pending_reply_count_uses_shared_per_source_waiter_geometry() {
        let source_a = sample_peer().id().clone();
        let source_b = sample_peer().id().clone();
        let mut pending =
            PendingGenesisReplies::new(NonZeroUsize::new(8).expect("non-zero queue"), 1_048_576);
        let per_source =
            genesis_reply_waiters_per_source(8).expect("non-zero shared genesis reply geometry");
        assert_eq!(per_source, 2);
        for request in 0..per_source {
            assert!(
                pending
                    .push(
                        source_a.clone(),
                        pending_reply(
                            source_a.clone(),
                            u64::try_from(request + 1).expect("small fixture size"),
                        ),
                    )
                    .is_ok()
            );
        }
        assert!(
            pending
                .push(source_a.clone(), pending_reply(source_a, 1))
                .is_err(),
            "one source cannot exceed the exact actor waiter share"
        );
        assert!(
            pending
                .push(source_b.clone(), pending_reply(source_b, 1))
                .is_ok(),
            "the saturated source cannot consume another source's share"
        );
    }

    #[test]
    fn genesis_request_fanout_deduplicates_same_source_targets() {
        let target = sample_peer().id().clone();
        let signer = checked_genesis_bootstrap_key_fixture();
        let request = GenesisRequest {
            request_id: 17,
            chain_id: ChainId::from("deduplicated-genesis-fanout"),
            expected_hash: None,
            expected_pubkey: Some(signer.public_key().clone()),
            kind: GenesisRequestKind::Preflight,
        };
        let fanout =
            GenesisRequestFanout::new(&[target.clone(), target.clone(), target.clone()], request);

        assert_eq!(fanout.targets.len(), 1);
        assert_eq!(fanout.targets[0].peer_id, target);
    }

    #[tokio::test]
    async fn bootstrapper_clones_cannot_multiply_listener_producers() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("single-genesis-listener");
        let signer = checked_genesis_bootstrap_key_fixture();
        let cfg = GenesisConfig {
            public_key: signer.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: Vec::new(),
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id);
        let clone = bootstrapper.clone();

        bootstrapper.spawn_listener().await;
        clone.spawn_listener().await;

        assert_eq!(
            network
                .subscriptions
                .load(std::sync::atomic::Ordering::SeqCst),
            1,
            "all clones must share one listener and one bounded reply scheduler"
        );
    }

    #[tokio::test]
    async fn bootstrapper_clones_cannot_multiply_fetch_fanouts() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("single-genesis-fetch");
        let signer = checked_genesis_bootstrap_key_fixture();
        let target = sample_peer();
        let cfg = GenesisConfig {
            public_key: signer.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![target.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_secs(5),
            bootstrap_retry_interval: Duration::from_secs(1),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id);
        let clone = bootstrapper.clone();
        let genesis_account = AccountId::new(signer.public_key().clone());

        {
            let first_fetch = bootstrapper.fetch_genesis(
                std::slice::from_ref(target.id()),
                &genesis_account,
                None,
            );
            tokio::pin!(first_fetch);
            tokio::select! {
                _ = &mut first_fetch => {
                    panic!("the first fetch must remain active without a responder")
                }
                () = wait_for_posts(&network, 1) => {}
            }
            assert_eq!(network.posted.lock().expect("posted").len(), 1);

            let second = clone
                .fetch_genesis(std::slice::from_ref(target.id()), &genesis_account, None)
                .await;
            assert!(matches!(second, Err(BootstrapError::FetchAlreadyActive)));
            assert_eq!(
                network.posted.lock().expect("posted").len(),
                1,
                "a rejected clone must not create another target fanout"
            );
        }

        assert!(
            !bootstrapper
                .fetch_active
                .load(std::sync::atomic::Ordering::Acquire),
            "cancelling the owning fetch future must release the single producer"
        );
    }

    async fn wait_for_posts(network: &MockNetwork, expected: usize) {
        for _ in 0..50 {
            if network.posted.lock().expect("posted").len() >= expected {
                return;
            }
            time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn preflight_retransmits_the_same_registered_request_until_deadline() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-retry-witness");
        let kp = checked_genesis_bootstrap_key_fixture();
        let peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_millis(80),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id);
        network.backpressure_next_posts(1);

        assert!(matches!(
            bootstrapper.try_preflight(&[peer.id().clone()], None).await,
            Err(BootstrapError::NoResponse)
        ));

        let posted = network.posted.lock().expect("posted");
        assert!(posted.len() >= 2, "initial post must have a retry witness");
        let request_ids = posted
            .iter()
            .map(|post| match &post.data {
                NetworkMessage::GenesisRequest(request) => request.request_id,
                other => panic!("unexpected bootstrap message: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert!(request_ids.iter().all(|id| *id == request_ids[0]));
    }

    #[tokio::test]
    async fn responder_retains_exact_response_until_actor_capacity_opens() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-response-retry-witness");
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");
        network.backpressure_next_posts(1);

        let request = GenesisRequest {
            request_id: 41,
            chain_id,
            expected_hash: None,
            expected_pubkey: Some(kp.public_key().clone()),
            kind: GenesisRequestKind::Preflight,
        };
        let sender = network
            .sender
            .lock()
            .expect("sender")
            .clone()
            .expect("listener registered");
        sender
            .send(PeerMessage::new(
                peer,
                NetworkMessage::GenesisRequest(Box::new(request)),
                0,
            ))
            .await
            .expect("send genesis request");

        wait_for_posts(&network, 1).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 1);
        match &posted[0].data {
            NetworkMessage::GenesisResponse(response) => {
                assert_eq!(response.request_id, 41);
                assert_eq!(response.error, None);
            }
            other => panic!("unexpected bootstrap message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn blocked_reply_source_does_not_starve_other_requests_or_inbound_response() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-response-source-fairness");
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let blocked = sample_peer();
        let responsive = sample_peer();
        let inbound_peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![blocked.id().clone(), responsive.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        let mut inbound = bootstrapper.register_request(99).await;
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");
        network.block_reply_peer(blocked.id().clone());

        let sender = network
            .sender
            .lock()
            .expect("sender")
            .clone()
            .expect("listener registered");
        for (peer, request_id) in [(blocked.clone(), 41), (responsive.clone(), 42)] {
            sender
                .send(PeerMessage::new(
                    peer,
                    NetworkMessage::GenesisRequest(Box::new(GenesisRequest {
                        request_id,
                        chain_id: chain_id.clone(),
                        expected_hash: None,
                        expected_pubkey: Some(kp.public_key().clone()),
                        kind: GenesisRequestKind::Preflight,
                    })),
                    0,
                ))
                .await
                .expect("send genesis request");
        }
        network.push_response(
            inbound_peer,
            error_response(chain_id, 99, GenesisResponseError::MissingGenesis),
        );

        let received = time::timeout(Duration::from_secs(1), inbound.recv())
            .await
            .expect("blocked outbound reply must not stall inbound response polling")
            .expect("registered inbound response remains deliverable");
        assert_eq!(received.response.request_id, 99);
        wait_for_posts(&network, 1).await;
        {
            let posted = network.posted.lock().expect("posted");
            assert_eq!(posted.len(), 1);
            let NetworkMessage::GenesisResponse(response) = &posted[0].data else {
                panic!("unexpected bootstrap message: {:?}", posted[0].data)
            };
            assert_eq!(response.request_id, 42);
        }

        network.unblock_reply_peer(blocked.id());
        wait_for_posts(&network, 2).await;
        let mut response_ids = network
            .posted
            .lock()
            .expect("posted")
            .iter()
            .map(|post| match &post.data {
                NetworkMessage::GenesisResponse(response) => response.request_id,
                other => panic!("unexpected bootstrap message: {other:?}"),
            })
            .collect::<Vec<_>>();
        response_ids.sort_unstable();
        assert_eq!(response_ids, vec![41, 42]);
    }

    #[tokio::test]
    async fn registered_response_queue_is_bounded_and_recovers_capacity() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-bounded-response-queue");
        let kp = checked_genesis_bootstrap_key_fixture();
        let peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::ZERO,
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network, chain_id.clone());
        let mut receiver = bootstrapper.register_request(55).await;
        let sender = bootstrapper
            .pending
            .lock()
            .expect("pending")
            .get(&55)
            .expect("registered request")
            .clone();
        let response = error_response(chain_id, 55, GenesisResponseError::MissingGenesis);

        for _ in 0..bootstrapper.response_queue_cap.get() {
            assert!(
                sender
                    .try_send(InboundResponse {
                        peer: peer.clone(),
                        response: response.clone(),
                    })
                    .is_ok()
            );
        }
        assert!(
            sender
                .try_send(InboundResponse {
                    peer: peer.clone(),
                    response: response.clone(),
                })
                .is_err(),
            "one item beyond the configured queue cap must fail closed"
        );

        assert!(receiver.recv().await.is_some());
        assert!(
            sender.try_send(InboundResponse { peer, response }).is_ok(),
            "service must release one exact queue slot"
        );
    }

    #[tokio::test]
    async fn responder_throttle_history_is_hard_bounded_under_identity_churn() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-bounded-responder-history");
        let kp = checked_genesis_bootstrap_key_fixture();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: Vec::new(),
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::from_secs(3_600),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.spawn_listener().await;
        let sender = network
            .sender
            .lock()
            .expect("sender")
            .clone()
            .expect("listener registered");
        let history_cap = bootstrapper.response_queue_cap.get();
        let mut peer_ids = Vec::new();
        for request_id in 0..(history_cap + 3) {
            let peer = sample_peer();
            peer_ids.push(peer.id().clone());
            sender
                .send(PeerMessage::new(
                    peer,
                    NetworkMessage::GenesisRequest(Box::new(GenesisRequest {
                        request_id: u64::try_from(request_id).expect("small request id"),
                        chain_id: chain_id.clone(),
                        expected_hash: None,
                        expected_pubkey: None,
                        kind: GenesisRequestKind::Preflight,
                    })),
                    0,
                ))
                .await
                .expect("send churn request");
        }
        wait_for_posts(&network, history_cap + 3).await;

        let responder = bootstrapper.responder.lock().expect("responder");
        assert_eq!(responder.last_response.len(), history_cap);
        assert_eq!(responder.last_response_order.len(), history_cap);
        assert!(!responder.last_response.contains_key(&peer_ids[0]));
        assert!(
            responder
                .last_response
                .contains_key(peer_ids.last().expect("at least one churn fixture peer"))
        );
    }

    #[test]
    fn genesis_message_filter_is_exact_and_registers_sender() {
        let network = MockNetwork::default();
        let (tx, _rx) = mpsc::channel(1);
        let filter = genesis_message_filter();
        assert_eq!(
            filter,
            SubscriberFilter::topics_for_route(
                [Topic::Control, Topic::BlockSync],
                SubscriberRoute::GenesisBootstrap,
            )
        );
        network
            .subscribe_with_filter(tx, filter)
            .expect("subscribe");
        assert!(
            network.sender.lock().expect("sender mutex").is_some(),
            "subscriber sender should be registered"
        );
    }

    #[tokio::test]
    async fn preflight_mismatched_pubkey_is_error() {
        let expected = checked_genesis_bootstrap_key_fixture();
        let other = checked_genesis_bootstrap_key_fixture();
        let response = GenesisResponse {
            request_id: 1,
            chain_id: ChainId::from("chain"),
            public_key: Some(other.public_key().clone()),
            hash: Some(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0u8; 32]),
            )),
            size_hint: Some(10),
            payload: None,
            error: Some(GenesisResponseError::MismatchedPubkey),
        };
        let result = validate_preflight_response(
            &response,
            expected.public_key(),
            None,
            1024,
            &ChainId::from("chain"),
        );
        assert!(matches!(result, Err(BootstrapError::SignerMismatch { .. })));
    }

    #[test]
    fn preflight_too_large_hint_saturates() {
        let kp = checked_genesis_bootstrap_key_fixture();
        let response = GenesisResponse {
            request_id: 2,
            chain_id: ChainId::from("chain"),
            public_key: Some(kp.public_key().clone()),
            hash: None,
            size_hint: None,
            payload: None,
            error: Some(GenesisResponseError::TooLarge),
        };
        let result = validate_preflight_response(
            &response,
            kp.public_key(),
            None,
            u64::MAX,
            &ChainId::from("chain"),
        );
        assert!(matches!(
            result,
            Err(BootstrapError::PayloadTooLarge { hint, cap })
            if hint == u64::MAX && cap == u64::MAX
        ));
    }

    #[test]
    fn preflight_rate_limited_is_ignored() {
        let kp = checked_genesis_bootstrap_key_fixture();
        let response = GenesisResponse {
            request_id: 3,
            chain_id: ChainId::from("chain"),
            public_key: Some(kp.public_key().clone()),
            hash: None,
            size_hint: None,
            payload: None,
            error: Some(GenesisResponseError::RateLimited),
        };
        let result = validate_preflight_response(
            &response,
            kp.public_key(),
            None,
            1024,
            &ChainId::from("chain"),
        );
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn payload_rate_limited_is_ignored() {
        let kp = checked_genesis_bootstrap_key_fixture();
        let expected_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([7u8; 32]));
        let response = GenesisResponse {
            request_id: 4,
            chain_id: ChainId::from("chain"),
            public_key: Some(kp.public_key().clone()),
            hash: Some(expected_hash.clone()),
            size_hint: Some(10),
            payload: None,
            error: Some(GenesisResponseError::RateLimited),
        };
        let result = validate_payload_response(
            &response,
            &ChainId::from("chain"),
            kp.public_key(),
            &expected_hash,
            1024,
        );
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn responder_rejects_unlisted_peer() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("test-chain");
        let kp = checked_genesis_bootstrap_key_fixture();
        let allow_other = checked_genesis_bootstrap_key_fixture();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![PeerId::new(allow_other.public_key().clone())],
            bootstrap_max_bytes: 1024,
            bootstrap_response_throttle: Duration::from_secs(0),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.seed_topology(&[]);
        bootstrapper.spawn_listener().await;

        let request = GenesisRequest {
            request_id: 7,
            chain_id,
            expected_hash: None,
            expected_pubkey: None,
            kind: GenesisRequestKind::Preflight,
        };
        let sender = network.sender.lock().expect("sender").clone();
        if let Some(sender) = sender {
            let _ = sender
                .send(PeerMessage::new(
                    sample_peer(),
                    NetworkMessage::GenesisRequest(Box::new(request)),
                    0,
                ))
                .await;
        }

        wait_for_posts(&network, 1).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 1);
        if let Some(NetworkMessage::GenesisResponse(resp)) = posted.first().map(|post| &post.data) {
            assert_eq!(resp.error, Some(GenesisResponseError::NotAllowed));
        } else {
            panic!("unexpected message posted");
        }
    }

    #[tokio::test]
    async fn responder_rate_limits_peer_requests() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-rate-limit");
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::from_millis(200),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.seed_topology(&[]);
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");

        let sender = network.sender.lock().expect("sender").clone();
        if let Some(sender) = sender {
            for request_id in 0..2u64 {
                let request = GenesisRequest {
                    request_id,
                    chain_id: chain_id.clone(),
                    expected_hash: None,
                    expected_pubkey: None,
                    kind: GenesisRequestKind::Preflight,
                };
                sender
                    .send(PeerMessage::new(
                        peer.clone(),
                        NetworkMessage::GenesisRequest(Box::new(request)),
                        0,
                    ))
                    .await
                    .expect("send genesis request");
            }
        }

        wait_for_posts(&network, 2).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 2);
        let mut errors = posted.iter().filter_map(|post| {
            if let NetworkMessage::GenesisResponse(resp) = &post.data {
                Some(resp.error)
            } else {
                None
            }
        });
        assert_eq!(errors.next(), Some(None));
        assert_eq!(errors.next(), Some(Some(GenesisResponseError::RateLimited)));
        assert!(errors.next().is_none());
    }

    #[tokio::test]
    async fn responder_flags_too_large_payload() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-too-large");
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let payload = GenesisPayload::from_block(&block, kp.public_key()).expect("payload");
        let peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: payload.size_bytes().saturating_sub(1),
            bootstrap_response_throttle: Duration::from_secs(0),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.seed_topology(&[]);
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");

        let request = GenesisRequest {
            request_id: 8,
            chain_id,
            expected_hash: None,
            expected_pubkey: None,
            kind: GenesisRequestKind::Preflight,
        };
        let sender = network.sender.lock().expect("sender").clone();
        if let Some(sender) = sender {
            sender
                .send(PeerMessage::new(
                    peer.clone(),
                    NetworkMessage::GenesisRequest(Box::new(request)),
                    0,
                ))
                .await
                .expect("send genesis request");
        }

        wait_for_posts(&network, 1).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 1);
        match posted.first().map(|post| &post.data) {
            Some(NetworkMessage::GenesisResponse(resp)) => {
                assert_eq!(resp.error, Some(GenesisResponseError::TooLarge));
                assert_eq!(resp.size_hint, Some(payload.size_bytes()));
                assert!(resp.payload.is_none());
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn fetch_genesis_continues_across_diagnostic_retry_cycles() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let payload = GenesisPayload::from_block(&block, kp.public_key()).expect("payload");
        let peer = sample_peer();
        let fallback_peer = sample_peer();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: Some(payload.hash.clone()),
            bootstrap_allowlist: vec![peer.id().clone(), fallback_peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::from_secs(0),
            bootstrap_request_timeout: Duration::from_millis(30),
            bootstrap_retry_interval: Duration::from_millis(5),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.seed_topology(&[]);
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");

        let genesis_account = AccountId::new(kp.public_key().clone());
        let peers = [peer.id().clone(), fallback_peer.id().clone()];
        let fetch = bootstrapper.fetch_genesis(&peers, &genesis_account, None);
        let posted = network.posted.clone();
        tokio::spawn(async move {
            let mut first_request_id = None;
            let mut preflight_req = None;
            for _ in 0..100 {
                {
                    let guard = posted.lock().expect("posted");
                    for request in guard.iter().filter_map(|post| match &post.data {
                        NetworkMessage::GenesisRequest(request)
                            if matches!(request.kind, GenesisRequestKind::Preflight) =>
                        {
                            Some(request)
                        }
                        _ => None,
                    }) {
                        if let Some(first) = first_request_id {
                            if request.request_id != first {
                                preflight_req = Some(request.clone());
                                break;
                            }
                        } else {
                            first_request_id = Some(request.request_id);
                        }
                    }
                }
                if preflight_req.is_some() {
                    break;
                }
                time::sleep(Duration::from_millis(5)).await;
            }
            let preflight_req = preflight_req
                .expect("bootstrap must continue with a new request after one diagnostic cycle");
            let preflight = GenesisResponse {
                chain_id: preflight_req.chain_id.clone(),
                request_id: preflight_req.request_id,
                hash: Some(payload.hash.clone()),
                public_key: Some(payload.signer.clone()),
                size_hint: Some(payload.bytes.len() as u64),
                payload: None,
                error: None,
            };
            network.push_response(peer.clone(), preflight);

            let mut payload_req = None;
            for _ in 0..100 {
                payload_req =
                    posted
                        .lock()
                        .expect("posted")
                        .iter()
                        .find_map(|post| match &post.data {
                            NetworkMessage::GenesisRequest(req)
                                if matches!(req.kind, GenesisRequestKind::Fetch)
                                    && post.peer_id.eq(fallback_peer.id()) =>
                            {
                                Some(req.clone())
                            }
                            _ => None,
                        });
                if payload_req.is_some() {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }
            let payload_req =
                payload_req.expect("a non-responder must remain a payload recovery target");
            let payload_response = GenesisResponse {
                chain_id: payload_req.chain_id.clone(),
                request_id: payload_req.request_id,
                hash: Some(payload.hash.clone()),
                public_key: Some(payload.signer.clone()),
                size_hint: Some(payload.bytes.len() as u64),
                payload: Some(payload.bytes.clone()),
                error: None,
            };
            network.push_response(fallback_peer, payload_response);
        });

        let result = time::timeout(Duration::from_secs(2), fetch)
            .await
            .expect("bootstrap must not stall on the failed preflight responder")
            .expect("fetch succeeds");
        assert_eq!(result.hash, payload.hash);
        assert!(!result.bytes.is_empty());
    }

    #[tokio::test]
    async fn preflight_mismatched_chain_is_rejected() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-a");
        let peer = sample_peer();
        let kp = checked_genesis_bootstrap_key_fixture();
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1024,
            bootstrap_response_throttle: Duration::from_secs(0),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id);
        bootstrapper.spawn_listener().await;

        let request = GenesisRequest {
            request_id: 9,
            chain_id: ChainId::from("other-chain"),
            expected_hash: None,
            expected_pubkey: None,
            kind: GenesisRequestKind::Preflight,
        };
        let sender = network.sender.lock().expect("sender").clone();
        if let Some(sender) = sender {
            sender
                .send(PeerMessage::new(
                    peer.clone(),
                    NetworkMessage::GenesisRequest(Box::new(request)),
                    0,
                ))
                .await
                .expect("send genesis request");
        }

        wait_for_posts(&network, 1).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 1);
        match posted.first().map(|post| &post.data) {
            Some(NetworkMessage::GenesisResponse(resp)) => {
                assert_eq!(resp.error, Some(GenesisResponseError::MismatchedChain));
                assert_eq!(resp.request_id, 9);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn request_with_wrong_expected_hash_is_rejected() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-hash");
        let peer = sample_peer();
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let payload = GenesisPayload::from_block(&block, kp.public_key()).expect("payload");
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: payload.size_bytes().saturating_add(1),
            bootstrap_response_throttle: Duration::from_secs(0),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");

        let bad_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1u8; 32]));
        let request = GenesisRequest {
            request_id: 10,
            chain_id,
            expected_hash: Some(bad_hash),
            expected_pubkey: Some(payload.signer.clone()),
            kind: GenesisRequestKind::Preflight,
        };
        let sender = network.sender.lock().expect("sender").clone();
        if let Some(sender) = sender {
            sender
                .send(PeerMessage::new(
                    peer.clone(),
                    NetworkMessage::GenesisRequest(Box::new(request)),
                    0,
                ))
                .await
                .expect("send genesis request");
        }

        wait_for_posts(&network, 1).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 1);
        match posted.first().map(|post| &post.data) {
            Some(NetworkMessage::GenesisResponse(resp)) => {
                assert_eq!(resp.error, Some(GenesisResponseError::MismatchedHash));
                assert_eq!(resp.hash, Some(payload.hash));
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn exact_duplicate_rebuilds_response_but_same_id_body_reuse_is_rate_limited() {
        let network = MockNetwork::default();
        let chain_id = ChainId::from("chain-dup");
        let peer = sample_peer();
        let kp = checked_genesis_bootstrap_key_fixture();
        let block = sample_block(&chain_id, &kp);
        let cfg = GenesisConfig {
            public_key: kp.public_key().clone(),
            file: None,
            manifest_json: None,
            expected_hash: None,
            bootstrap_allowlist: vec![peer.id().clone()],
            bootstrap_max_bytes: 1_048_576,
            bootstrap_response_throttle: Duration::from_millis(200),
            bootstrap_request_timeout: Duration::from_secs(1),
            bootstrap_retry_interval: Duration::from_millis(10),
            bootstrap_max_attempts: 1,
            bootstrap_enabled: true,
        };
        let bootstrapper = GenesisBootstrapper::new(&cfg, network.clone(), chain_id.clone());
        bootstrapper.spawn_listener().await;
        bootstrapper.set_payload(&block).await.expect("payload set");

        for _ in 0..2 {
            let request = GenesisRequest {
                request_id: 11,
                chain_id: chain_id.clone(),
                expected_hash: None,
                expected_pubkey: Some(kp.public_key().clone()),
                kind: GenesisRequestKind::Preflight,
            };
            let sender = network.sender.lock().expect("sender").clone();
            if let Some(sender) = sender {
                sender
                    .send(PeerMessage::new(
                        peer.clone(),
                        NetworkMessage::GenesisRequest(Box::new(request)),
                        0,
                    ))
                    .await
                    .expect("send genesis request");
            }
        }

        wait_for_posts(&network, 2).await;
        let changed_request = GenesisRequest {
            request_id: 11,
            chain_id: chain_id.clone(),
            expected_hash: None,
            expected_pubkey: Some(kp.public_key().clone()),
            kind: GenesisRequestKind::Fetch,
        };
        let sender = network
            .sender
            .lock()
            .expect("sender")
            .clone()
            .expect("listener registered");
        sender
            .send(PeerMessage::new(
                peer,
                NetworkMessage::GenesisRequest(Box::new(changed_request)),
                0,
            ))
            .await
            .expect("send same-id request with a changed body");

        wait_for_posts(&network, 3).await;
        let posted = network.posted.lock().expect("posted");
        assert_eq!(posted.len(), 3);
        let responses = posted
            .iter()
            .filter_map(|post| {
                if let NetworkMessage::GenesisResponse(resp) = &post.data {
                    Some(resp.as_ref())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[0], responses[1]);
        assert_eq!(responses[0].request_id, 11);
        assert_eq!(responses[0].error, None);
        assert_eq!(responses[2].request_id, 11);
        assert_eq!(responses[2].error, Some(GenesisResponseError::RateLimited));
    }
}
