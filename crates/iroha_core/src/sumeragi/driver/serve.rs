//! Serving and body fetching off the event loop (`specs/sumeragi.md` §6.9, §12.2, §12.3 O5):
//! `ServeBlocks` answers from the block store (consecutive heights, stopping at the first
//! missing one, within the byte budget; an empty `SyncResponse` means "nothing held at
//! `from_height`"), `ServeBody` from the body store or the block store (or not at all), and
//! `FetchBody` looks in the local stores first and otherwise asks the given peers.

use iroha_sumeragi::{
    api::{Action, Event},
    message::{Block, BlockRequest, BlockResponse, SyncEntry, SyncResponse, WireMessage},
    types::{Hash32, PublicKey},
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

/// Carry out one serving request: responses and body requests go to `net`; a body found locally
/// for `FetchBody` is returned as `BodyAvailable`.
pub fn serve(
    request: ServeRequest,
    instance: Hash32,
    bodies: &(impl BodyStore + ?Sized),
    blocks: &(impl BlockStore + ?Sized),
    net: &(impl Net + ?Sized),
) -> Option<Event> {
    match request {
        ServeRequest::Blocks {
            to,
            from_height,
            max_count,
            max_bytes,
        } => {
            let blocks = entries(blocks, from_height, max_count, max_bytes);
            let msg = WireMessage::SyncResponse(SyncResponse { instance, blocks });
            if let Some(frame) = frame(&msg) {
                net.send(&to, &frame);
            }
            None
        }
        ServeRequest::Body {
            to,
            height,
            block_hash,
        } => {
            let block = local_body(bodies, blocks, height, &block_hash)?;
            let msg = WireMessage::BlockResponse(BlockResponse { instance, block });
            if let Some(frame) = frame(&msg) {
                net.send(&to, &frame);
            }
            None
        }
        ServeRequest::Fetch {
            height,
            block_hash,
            peers,
        } => {
            if let Some(block) = local_body(bodies, blocks, height, &block_hash) {
                return Some(Event::BodyAvailable { block });
            }
            let msg = WireMessage::BlockRequest(BlockRequest {
                instance,
                height,
                block_hash,
            });
            if let Some(frame) = frame(&msg) {
                for peer in &peers {
                    net.send(peer, &frame);
                }
            }
            None
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
    /// only if held, and for a fetch the local body or requests to every peer.
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
        assert_eq!(serve(blocks_req, instance, &bodies, &blocks, &net), None);
        let body = ServeRequest::Body {
            to: key(2),
            height: 2,
            block_hash: hash(&chain[1]),
        };
        assert_eq!(serve(body, instance, &bodies, &blocks, &net), None);
        let missing = ServeRequest::Body {
            to: key(2),
            height: 9,
            block_hash: Hash32::ZERO,
        };
        assert_eq!(serve(missing, instance, &bodies, &blocks, &net), None);
        let fetch_local = ServeRequest::Fetch {
            height: 1,
            block_hash: hash(&chain[0]),
            peers: vec![key(3)],
        };
        assert_eq!(
            serve(fetch_local, instance, &bodies, &blocks, &net),
            Some(Event::BodyAvailable {
                block: chain[0].clone()
            })
        );
        let fetch_remote = ServeRequest::Fetch {
            height: 5,
            block_hash: Hash32::ZERO,
            peers: vec![key(3), key(4)],
        };
        assert_eq!(serve(fetch_remote, instance, &bodies, &blocks, &net), None);
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
