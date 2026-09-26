//! Serving and body fetching off the event loop (`specs/sumeragi.md` §6.9, §12.2, §12.3 O5,
//! O6): `ServeBlocks` answers from the block store (consecutive heights, stopping at the first
//! missing one, within the byte budget; an empty `SyncResponse` means "nothing held at
//! `from_height`"), `ServeBody` from the body store or the block store (or not at all), and
//! `FetchBody` looks in the local stores first and otherwise asks the given peers.
//!
//! [`ServeSched`] decides what the serve thread does next, one request at a time: the node's own
//! `FetchBody`s first (so its body recovery never waits behind serving others), then the peers'
//! requests round-robin — at most one `ServeBlocks` and one `ServeBody` pending per peer, a newer
//! one replacing it — each peer within a token bucket of response bytes (§12.2 per-peer rate
//! limits). Requests beyond those bounds are dropped (O6: the requester retries), so a peer
//! streaming `SyncRequest`s costs a bounded queue and its own byte quota, nothing more.

use std::{
    collections::{BTreeMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
};

use iroha_sumeragi::{
    api::{Action, Event},
    message::{Block, BlockRequest, BlockResponse, SyncEntry, SyncResponse, WireMessage},
    types::{Hash32, Millis, PublicKey},
};

use super::traits::{BlockStore, BodyStore, Frame, Net};

/// Bytes a sync entry adds to a response beyond its own encoding (length prefixes), counted
/// generously; the frame header fits in the 64 KiB the receiver allows above its byte cap.
const ENTRY_FRAMING: usize = 16;

/// A serving request (a gated action released by the O2 barrier).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServeRequest {
    /// `ServeBlocks`.
    Blocks {
        /// Requester.
        to: PublicKey,
        /// First height.
        from_height: u64,
        /// Entry limit.
        max_count: u16,
        /// Byte limit.
        max_bytes: u32,
    },
    /// `ServeBody`.
    Body {
        /// Requester.
        to: PublicKey,
        /// Height.
        height: u64,
        /// Block hash.
        block_hash: Hash32,
    },
    /// `FetchBody`.
    Fetch {
        /// Height.
        height: u64,
        /// Block hash.
        block_hash: Hash32,
        /// Peers to ask when no local store holds it.
        peers: Vec<PublicKey>,
    },
}

impl ServeRequest {
    /// The request of a serving action; the action itself otherwise.
    ///
    /// # Errors
    /// The action, when it is not a serving action.
    pub fn from_action(action: Action) -> Result<Self, Action> {
        match action {
            Action::ServeBlocks {
                to,
                from_height,
                max_count,
                max_bytes,
            } => Ok(Self::Blocks {
                to,
                from_height,
                max_count,
                max_bytes,
            }),
            Action::ServeBody {
                to,
                height,
                block_hash,
            } => Ok(Self::Body {
                to,
                height,
                block_hash,
            }),
            Action::FetchBody {
                height,
                block_hash,
                peers,
            } => Ok(Self::Fetch {
                height,
                block_hash,
                peers,
            }),
            other => Err(other),
        }
    }

    /// The core action this request came from.
    pub fn into_action(self) -> Action {
        match self {
            Self::Blocks {
                to,
                from_height,
                max_count,
                max_bytes,
            } => Action::ServeBlocks {
                to,
                from_height,
                max_count,
                max_bytes,
            },
            Self::Body {
                to,
                height,
                block_hash,
            } => Action::ServeBody {
                to,
                height,
                block_hash,
            },
            Self::Fetch {
                height,
                block_hash,
                peers,
            } => Action::FetchBody {
                height,
                block_hash,
                peers,
            },
        }
    }
}

/// Per-peer serving limits (§12.2 per-peer rate limits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServeLimits {
    /// Response bytes a peer is served per second on average (its token bucket's refill).
    pub bytes_per_sec: u64,
    /// Response bytes a peer may be served in a burst (its bucket's size).
    pub burst_bytes: u64,
    /// Peers tracked at once (with a pending request or a bucket not yet refilled); a request
    /// of a further peer is dropped.
    pub max_peers: usize,
}

impl Default for ServeLimits {
    fn default() -> Self {
        Self {
            bytes_per_sec: 32 << 20,
            burst_bytes: 64 << 20,
            max_peers: 1_024,
        }
    }
}

/// The pending requests and the token bucket of one requesting peer.
#[derive(Debug)]
struct PeerServe {
    body: Option<ServeRequest>,
    blocks: Option<ServeRequest>,
    /// Response bytes the peer may still be served (negative: the debt of a large response).
    tokens: i64,
    /// Local time of the last refill.
    at: Millis,
    /// In the round-robin order.
    queued: bool,
}

impl PeerServe {
    fn refill(&mut self, limits: ServeLimits, now: Millis) {
        let elapsed = now.saturating_sub(self.at);
        self.at = self.at.max(now);
        let gained = u128::from(elapsed) * u128::from(limits.bytes_per_sec) / 1_000;
        let gained = i64::try_from(gained).unwrap_or(i64::MAX);
        self.tokens = self.tokens.saturating_add(gained).min(burst(limits));
    }

    fn pending(&self) -> usize {
        usize::from(self.body.is_some()) + usize::from(self.blocks.is_some())
    }
}

fn burst(limits: ServeLimits) -> i64 {
    i64::try_from(limits.burst_bytes).unwrap_or(i64::MAX)
}

/// The serving scheduler of one instance (see the module documentation): what the serve
/// thread does next, one request at a time.
#[derive(Debug)]
pub struct ServeSched {
    limits: ServeLimits,
    /// The node's own `FetchBody`s, one per wanted body (the latest), served first.
    fetch: VecDeque<ServeRequest>,
    peers: BTreeMap<PublicKey, PeerServe>,
    /// Peers with a pending request, in round-robin order.
    order: VecDeque<PublicKey>,
    /// The request in flight: `Some(None)` for a fetch, `Some(Some(peer))` for serving `peer`.
    in_flight: Option<Option<PublicKey>>,
    dropped: u64,
}

impl ServeSched {
    /// An empty scheduler with `limits`.
    pub fn new(limits: ServeLimits) -> Self {
        Self {
            limits,
            fetch: VecDeque::new(),
            peers: BTreeMap::new(),
            order: VecDeque::new(),
            in_flight: None,
            dropped: 0,
        }
    }

    /// Queue a request at local time `now`; returns whether it was kept. A `FetchBody`
    /// replaces a pending one of the same body and is never dropped (the core bounds its
    /// wants). A peer's `ServeBlocks` or `ServeBody` replaces its pending one of the same kind
    /// (the older is dropped); it is dropped if the peer's bucket is empty or too many peers
    /// are tracked.
    pub fn push(&mut self, request: ServeRequest, now: Millis) -> bool {
        let to = match &request {
            ServeRequest::Fetch { block_hash, .. } => {
                let hash = *block_hash;
                self.fetch.retain(
                    |r| !matches!(r, ServeRequest::Fetch { block_hash, .. } if *block_hash == hash),
                );
                self.fetch.push_back(request);
                return true;
            }
            ServeRequest::Blocks { to, .. } | ServeRequest::Body { to, .. } => to.clone(),
        };
        if !self.peers.contains_key(&to) && self.peers.len() >= self.limits.max_peers {
            self.evict_idle(now);
            if self.peers.len() >= self.limits.max_peers {
                self.dropped += 1;
                return false;
            }
        }
        let limits = self.limits;
        let peer = self.peers.entry(to.clone()).or_insert_with(|| PeerServe {
            body: None,
            blocks: None,
            tokens: burst(limits),
            at: now,
            queued: false,
        });
        peer.refill(limits, now);
        if peer.tokens <= 0 {
            self.dropped += 1;
            return false;
        }
        let slot = if matches!(request, ServeRequest::Blocks { .. }) {
            &mut peer.blocks
        } else {
            &mut peer.body
        };
        if slot.replace(request).is_some() {
            self.dropped += 1;
        }
        if !peer.queued {
            peer.queued = true;
            self.order.push_back(to);
        }
        true
    }

    /// Forget the peers that have nothing pending or in flight and a full bucket (nothing is
    /// lost with them).
    fn evict_idle(&mut self, now: Millis) {
        let limits = self.limits;
        let busy = self.in_flight.clone().flatten();
        self.peers.retain(|key, peer| {
            peer.refill(limits, now);
            peer.queued || busy.as_ref() == Some(key) || peer.tokens < burst(limits)
        });
    }

    /// The next request for the serve thread at local time `now`, if it is idle: a pending
    /// `FetchBody` first, then the next peer in round-robin order whose bucket is not empty (a
    /// body before a block range). The pending requests of a peer whose bucket ran empty are
    /// dropped.
    pub fn next(&mut self, now: Millis) -> Option<ServeRequest> {
        if self.in_flight.is_some() {
            return None;
        }
        if let Some(fetch) = self.fetch.pop_front() {
            self.in_flight = Some(None);
            return Some(fetch);
        }
        while let Some(key) = self.order.pop_front() {
            let Some(peer) = self.peers.get_mut(&key) else {
                continue;
            };
            peer.refill(self.limits, now);
            if peer.tokens <= 0 {
                self.dropped += u64::try_from(peer.pending()).unwrap_or(u64::MAX);
                peer.body = None;
                peer.blocks = None;
                peer.queued = false;
                continue;
            }
            let Some(request) = peer.body.take().or_else(|| peer.blocks.take()) else {
                peer.queued = false;
                continue;
            };
            if peer.pending() > 0 {
                self.order.push_back(key.clone());
            } else {
                peer.queued = false;
            }
            self.in_flight = Some(Some(key));
            return Some(request);
        }
        None
    }

    /// The serve thread finished the request in flight at local time `now`, having sent
    /// `bytes` of responses: charged to the requester's bucket.
    pub fn done(&mut self, now: Millis, bytes: u64) {
        let Some(Some(key)) = self.in_flight.take() else {
            return;
        };
        if let Some(peer) = self.peers.get_mut(&key) {
            peer.refill(self.limits, now);
            let bytes = i64::try_from(bytes).unwrap_or(i64::MAX);
            peer.tokens = peer.tokens.saturating_sub(bytes);
        }
    }

    /// Requests pending (not in flight).
    pub fn len(&self) -> usize {
        self.fetch.len() + self.peers.values().map(PeerServe::pending).sum::<usize>()
    }

    /// Whether nothing is pending.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a request is in flight.
    pub fn busy(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Requests dropped by the bounds so far (replaced, over a bucket or over the peer limit).
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

/// Committed entries from `from_height` on: consecutive, at most `max_count`, stopping at the
/// first missing height, and within `max_bytes` (encoded sizes; a single entry may exceed it,
/// §3.5).
pub fn entries(
    blocks: &(impl BlockStore + ?Sized),
    from_height: u64,
    max_count: u16,
    max_bytes: u32,
) -> Vec<SyncEntry> {
    let mut out = Vec::new();
    let mut bytes = 0usize;
    let limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    for height in (from_height..).take(usize::from(max_count)) {
        let Some(entry) = blocks.entry(height) else {
            break;
        };
        let size = norito::codec::Encode::encoded_len(&entry).saturating_add(ENTRY_FRAMING);
        if !out.is_empty() && bytes.saturating_add(size) > limit {
            break;
        }
        bytes = bytes.saturating_add(size);
        out.push(entry);
    }
    out
}

/// The body of `(height, block_hash)` from the body store, else from the block store.
pub fn local_body(
    bodies: &(impl BodyStore + ?Sized),
    blocks: &(impl BlockStore + ?Sized),
    height: u64,
    block_hash: &Hash32,
) -> Option<Block> {
    bodies.get(height, block_hash).or_else(|| {
        blocks
            .entry(height)
            .filter(|entry| entry.commit_qc.block_hash == *block_hash)
            .map(|entry| entry.block)
    })
}

/// Encode `msg` once for the transport (`None`: it cannot be encoded, a local bug; logged).
pub fn frame(msg: &WireMessage) -> Option<Frame> {
    match msg.encode() {
        Ok(bytes) => Some(Frame {
            instance: *msg.instance(),
            class: msg.traffic_class(),
            bytes: bytes.into(),
        }),
        Err(error) => {
            iroha_logger::error!(%error, "sumeragi message does not encode");
            None
        }
    }
}

/// What the serve thread did for one request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Served {
    /// `BodyAvailable` for a `FetchBody` satisfied from the local stores.
    pub event: Option<Event>,
    /// Bytes of responses sent (charged to the requester's bucket).
    pub bytes: u64,
}

/// `f`, or `None` if it panicked (a store read that panics fails like a missing entry).
fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|_| iroha_logger::error!(what, "sumeragi store read panicked"))
        .ok()
}

/// Send `msg` to `to`; returns the frame size.
fn send(net: &(impl Net + ?Sized), to: &PublicKey, msg: &WireMessage) -> u64 {
    frame(msg).map_or(0, |frame| {
        net.send(to, &frame);
        u64::try_from(frame.bytes.len()).unwrap_or(u64::MAX)
    })
}

/// Carry out one serving request: responses and body requests go to `net`; a body found locally
/// for `FetchBody` is returned as `BodyAvailable`. A store read that panics is a missing entry:
/// no response, or for `FetchBody` the requests to the peers.
pub fn serve(
    request: ServeRequest,
    instance: Hash32,
    bodies: &(impl BodyStore + ?Sized),
    blocks: &(impl BlockStore + ?Sized),
    net: &(impl Net + ?Sized),
) -> Served {
    match request {
        ServeRequest::Blocks {
            to,
            from_height,
            max_count,
            max_bytes,
        } => {
            let Some(entries) = guarded("sync entries", || {
                entries(blocks, from_height, max_count, max_bytes)
            }) else {
                return Served::default();
            };
            let msg = WireMessage::SyncResponse(SyncResponse {
                instance,
                blocks: entries,
            });
            Served {
                event: None,
                bytes: send(net, &to, &msg),
            }
        }
        ServeRequest::Body {
            to,
            height,
            block_hash,
        } => {
            let found = guarded("body", || local_body(bodies, blocks, height, &block_hash));
            let Some(block) = found.flatten() else {
                return Served::default();
            };
            let msg = WireMessage::BlockResponse(BlockResponse { instance, block });
            Served {
                event: None,
                bytes: send(net, &to, &msg),
            }
        }
        ServeRequest::Fetch {
            height,
            block_hash,
            peers,
        } => {
            let found = guarded("body", || local_body(bodies, blocks, height, &block_hash));
            if let Some(block) = found.flatten() {
                return Served {
                    event: Some(Event::BodyAvailable { block }),
                    bytes: 0,
                };
            }
            let msg = WireMessage::BlockRequest(BlockRequest {
                instance,
                height,
                block_hash,
            });
            for peer in &peers {
                send(net, peer, &msg);
            }
            Served::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_sumeragi::message::TrafficClass;

    use super::{
        super::tests::{
            block, commit_qc,
            fakes::{FakeBlocks, FakeBodies, FakeNet},
            hash,
        },
        *,
    };

    fn key(byte: u8) -> PublicKey {
        PublicKey::new(vec![byte; 32]).unwrap()
    }

    /// A chain of `n` blocks with 1000-byte payloads in a block store.
    fn chain(n: u64) -> (FakeBlocks, Vec<Block>) {
        let blocks = FakeBlocks::default();
        let mut out = Vec::new();
        let (mut parent, mut result) = (Hash32([1; 32]), Hash32([2; 32]));
        for h in 1..=n {
            let b = block(h, parent, result, vec![7; 1000]);
            let r = Hash32([u8::try_from(h).unwrap(); 32]);
            blocks.append(&b, &commit_qc(&b, r)).unwrap();
            (parent, result) = (hash(&b), r);
            out.push(b);
        }
        (blocks, out)
    }

    /// Consecutive entries within the count and byte budgets (the first always), stopping at
    /// the first missing height.
    #[test]
    fn entries_respect_count_bytes_and_gaps() {
        let (blocks, _) = chain(5);
        assert_eq!(entries(&blocks, 1, 3, u32::MAX).len(), 3);
        assert_eq!(
            entries(&blocks, 2, 64, u32::MAX).len(),
            4,
            "stops at the tip"
        );
        assert_eq!(
            entries(&blocks, 1, 64, 1).len(),
            1,
            "a single entry may exceed"
        );
        let two = norito::codec::Encode::encoded_len(&blocks.entry(1).unwrap()) * 2 + 40;
        assert_eq!(
            entries(&blocks, 1, 64, u32::try_from(two).unwrap()).len(),
            2
        );
        assert!(entries(&blocks, 6, 64, u32::MAX).is_empty());
        assert!(
            entries(&blocks, 0, 64, u32::MAX).is_empty(),
            "no genesis entry"
        );
    }

    /// Bodies come from the body store first, then from the block store if the hash matches.
    #[test]
    fn local_body_from_either_store() {
        let (blocks, chain) = chain(2);
        let bodies = FakeBodies::default();
        let fresh = block(3, hash(&chain[1]), Hash32([2; 32]), vec![1]);
        bodies.put(&hash(&fresh), &fresh).unwrap();
        assert_eq!(local_body(&bodies, &blocks, 3, &hash(&fresh)), Some(fresh));
        assert_eq!(
            local_body(&bodies, &blocks, 2, &hash(&chain[1])),
            Some(chain[1].clone())
        );
        assert_eq!(local_body(&bodies, &blocks, 2, &Hash32([9; 32])), None);
    }

    /// Each request produces its response: a (possibly empty) sync response, a body response
    /// only if held, and for a fetch the local body or requests to every peer; the bytes sent
    /// are reported (a fetch is the node's own request and costs nothing).
    #[test]
    fn serve_requests() {
        let (blocks, chain) = chain(2);
        let bodies = FakeBodies::default();
        let net = FakeNet::default();
        let instance = Hash32([5; 32]);
        let blocks_req = ServeRequest::Blocks {
            to: key(1),
            from_height: 1,
            max_count: 8,
            max_bytes: u32::MAX,
        };
        let served = serve(blocks_req, instance, &bodies, &blocks, &net);
        assert!(served.event.is_none() && served.bytes > 2_000, "{served:?}");
        let body = ServeRequest::Body {
            to: key(2),
            height: 2,
            block_hash: hash(&chain[1]),
        };
        let served = serve(body, instance, &bodies, &blocks, &net);
        assert!(served.event.is_none() && served.bytes > 1_000, "{served:?}");
        let missing = ServeRequest::Body {
            to: key(2),
            height: 9,
            block_hash: Hash32::ZERO,
        };
        assert_eq!(
            serve(missing, instance, &bodies, &blocks, &net),
            Served::default()
        );
        let fetch_local = ServeRequest::Fetch {
            height: 1,
            block_hash: hash(&chain[0]),
            peers: vec![key(3)],
        };
        assert_eq!(
            serve(fetch_local, instance, &bodies, &blocks, &net).event,
            Some(Event::BodyAvailable {
                block: chain[0].clone()
            })
        );
        let fetch_remote = ServeRequest::Fetch {
            height: 5,
            block_hash: Hash32::ZERO,
            peers: vec![key(3), key(4)],
        };
        assert_eq!(
            serve(fetch_remote, instance, &bodies, &blocks, &net),
            Served::default()
        );
        let sent = net.sent();
        assert_eq!(sent.len(), 4, "{sent:?}");
        assert!(
            matches!(&sent[0], (to, WireMessage::SyncResponse(r)) if *to == key(1) && r.blocks.len() == 2)
        );
        assert!(
            matches!(&sent[1], (to, WireMessage::BlockResponse(r)) if *to == key(2) && r.block == chain[1])
        );
        assert!(
            matches!(&sent[2], (to, WireMessage::BlockRequest(r)) if *to == key(3) && r.height == 5)
        );
        assert!(matches!(&sent[3], (to, WireMessage::BlockRequest(_)) if *to == key(4)));
    }

    /// A block store whose reads panic fails like a missing entry: no response to a peer, and
    /// a fetch still asks the peers (the serve thread survives, §12.5).
    #[test]
    fn panicking_reads_are_missing_entries() {
        let (blocks, chain) = chain(2);
        let bodies = FakeBodies::default();
        let net = FakeNet::default();
        let instance = Hash32([5; 32]);
        blocks.panic_reads(3);
        let sync = ServeRequest::Blocks {
            to: key(1),
            from_height: 1,
            max_count: 8,
            max_bytes: u32::MAX,
        };
        assert_eq!(
            serve(sync, instance, &bodies, &blocks, &net),
            Served::default()
        );
        let body = ServeRequest::Body {
            to: key(1),
            height: 1,
            block_hash: hash(&chain[0]),
        };
        assert_eq!(
            serve(body, instance, &bodies, &blocks, &net),
            Served::default()
        );
        let fetch = ServeRequest::Fetch {
            height: 1,
            block_hash: hash(&chain[0]),
            peers: vec![key(3)],
        };
        assert_eq!(
            serve(fetch, instance, &bodies, &blocks, &net),
            Served::default()
        );
        let sent = net.sent();
        assert!(
            matches!(&sent[..], [(to, WireMessage::BlockRequest(_))] if *to == key(3)),
            "{sent:?}"
        );
    }

    fn sync_req(to: u8, from_height: u64) -> ServeRequest {
        ServeRequest::Blocks {
            to: key(to),
            from_height,
            max_count: 64,
            max_bytes: u32::MAX,
        }
    }

    fn body_req(to: u8, height: u64) -> ServeRequest {
        ServeRequest::Body {
            to: key(to),
            height,
            block_hash: Hash32::ZERO,
        }
    }

    fn fetch_req(tag: u8, peer: u8) -> ServeRequest {
        ServeRequest::Fetch {
            height: 1,
            block_hash: Hash32([tag; 32]),
            peers: vec![key(peer)],
        }
    }

    /// The node's own fetches go first, one per body (the latest); a flooding peer keeps one
    /// pending request per kind (the latest) and is served round-robin with the others; one
    /// request is in flight at a time.
    #[test]
    fn fetch_first_and_one_request_per_peer_and_kind() {
        let mut sched = ServeSched::new(ServeLimits::default());
        for h in 0..10_000 {
            assert!(sched.push(sync_req(1, h), 0));
        }
        sched.push(body_req(1, 3), 0);
        sched.push(sync_req(2, 7), 0);
        sched.push(fetch_req(9, 1), 0);
        sched.push(fetch_req(8, 1), 0);
        sched.push(fetch_req(9, 2), 0);
        assert_eq!(
            sched.len(),
            3 + 2,
            "bounded: two fetches and three peer requests"
        );
        assert_eq!(
            sched.dropped(),
            9_999,
            "the older sync requests were replaced"
        );
        let mut order = Vec::new();
        while let Some(request) = sched.next(0) {
            assert!(sched.next(0).is_none(), "one request in flight");
            order.push(request);
            sched.done(0, 0);
        }
        assert_eq!(
            order,
            vec![
                fetch_req(8, 1),
                fetch_req(9, 2),
                body_req(1, 3),
                sync_req(2, 7),
                sync_req(1, 9_999),
            ]
        );
        assert!(sched.is_empty() && !sched.busy());
    }

    /// A peer's token bucket: served while it holds tokens, then its requests are dropped until
    /// the refill; other peers are unaffected; the debt of a large response is paid first.
    #[test]
    fn token_bucket_per_peer() {
        let limits = ServeLimits {
            bytes_per_sec: 1_000,
            burst_bytes: 2_000,
            max_peers: 16,
        };
        let mut sched = ServeSched::new(limits);
        assert!(sched.push(sync_req(1, 1), 0));
        assert_eq!(sched.next(0), Some(sync_req(1, 1)));
        sched.done(0, 5_000);
        assert!(
            !sched.push(sync_req(1, 2), 1_000),
            "3 000 bytes of debt, 1 000 repaid"
        );
        assert!(sched.push(sync_req(2, 1), 1_000), "another peer is served");
        assert_eq!(sched.next(1_000), Some(sync_req(2, 1)));
        sched.done(1_000, 10);
        assert!(!sched.push(sync_req(1, 3), 3_000));
        assert!(sched.push(sync_req(1, 4), 3_001), "debt repaid");
        // A request queued with tokens left is dropped at its turn once the bucket ran empty.
        assert!(sched.push(body_req(1, 5), 3_001));
        assert_eq!(sched.next(3_001), Some(body_req(1, 5)));
        sched.done(3_001, 4_000);
        assert_eq!(
            sched.next(3_002),
            None,
            "the pending sync request is dropped"
        );
        assert!(sched.is_empty());
        assert_eq!(sched.dropped(), 3);
    }

    /// Past `max_peers` tracked peers a new peer's request is dropped, unless an idle peer
    /// with a full bucket can be forgotten.
    #[test]
    fn tracked_peers_are_bounded() {
        let limits = ServeLimits {
            bytes_per_sec: 1_000,
            burst_bytes: 1_000,
            max_peers: 2,
        };
        let mut sched = ServeSched::new(limits);
        assert!(sched.push(sync_req(1, 1), 0));
        assert!(sched.push(sync_req(2, 1), 0));
        assert!(!sched.push(sync_req(3, 1), 0), "two peers pending");
        assert_eq!(sched.next(0), Some(sync_req(1, 1)));
        sched.done(0, 500);
        assert!(!sched.push(sync_req(3, 1), 0), "peer 1 still owes a refill");
        assert!(
            sched.push(sync_req(3, 1), 500),
            "peer 1 is idle and refilled"
        );
        assert_eq!(sched.len(), 2);
    }

    /// Serving actions and requests convert both ways; other actions are refused.
    #[test]
    fn requests_from_and_into_actions() {
        let actions = [
            Action::ServeBlocks {
                to: key(1),
                from_height: 3,
                max_count: 2,
                max_bytes: 9,
            },
            Action::ServeBody {
                to: key(1),
                height: 3,
                block_hash: Hash32::ZERO,
            },
            Action::FetchBody {
                height: 3,
                block_hash: Hash32::ZERO,
                peers: vec![key(2)],
            },
        ];
        for action in actions {
            let request = ServeRequest::from_action(action.clone()).unwrap();
            assert_eq!(request.into_action(), action);
        }
        let other = Action::LocalFault(iroha_sumeragi::api::LocalFault::RecordMissing);
        assert_eq!(ServeRequest::from_action(other.clone()), Err(other));
    }

    /// A frame carries the exact encoding, the instance and the class.
    #[test]
    fn frames_carry_instance_and_class() {
        let msg = WireMessage::BlockRequest(BlockRequest {
            instance: Hash32([5; 32]),
            height: 1,
            block_hash: Hash32::ZERO,
        });
        let frame = frame(&msg).unwrap();
        assert_eq!(frame.instance, Hash32([5; 32]));
        assert_eq!(frame.class, TrafficClass::Control);
        assert_eq!(WireMessage::decode(&frame.bytes, usize::MAX).unwrap(), msg);
    }
}
