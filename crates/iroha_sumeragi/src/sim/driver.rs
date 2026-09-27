//! Building blocks of the fake driver (§12.3): the node clock with offset and drift, ingress
//! lanes with O5 priorities and O6 bounds, the O2 persist-before-effect barrier, the write
//! device, the executor honouring O4, and the transaction encoding of the payload builder. The
//! lanes and the barrier belong to the hosted node ([`super::host::FakeHost`]); the device, the
//! executor and the builder are the world's fake backends.

use std::collections::{BTreeMap, VecDeque};

use crate::message::TrafficClass;
use crate::{
    api::{Action, Event, ExecOutcome},
    message::{Block, Qc, WireMessage},
    safety::SafetyRecord,
    testing::{Executed, sha256},
    types::{Hash32, Millis, PublicKey},
};

/// Every local clock starts far from zero so that negative offsets stay representable.
pub const CLOCK_BASE: i64 = 1_000_000;

/// A node's monotonic clock: `local(t) = BASE + offset + t + t·drift_ppm / 10^6` (§1.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    /// Offset in ms (e.g. ±10 s).
    pub offset: i64,
    /// Rate error in parts per million (e.g. ±10 000 = ±1 %).
    pub drift_ppm: i64,
}

impl Clock {
    /// Local time at global time `t`.
    pub fn local(&self, t: Millis) -> Millis {
        let t = i128::from(t);
        let value = i128::from(CLOCK_BASE)
            + i128::from(self.offset)
            + t
            + t * i128::from(self.drift_ppm) / 1_000_000;
        Millis::try_from(value.max(0)).unwrap_or(Millis::MAX)
    }

    /// The earliest global time at which the local clock reads at least `local`.
    pub fn global_at(&self, local: Millis) -> Millis {
        if local == Millis::MAX {
            return Millis::MAX;
        }
        let target = i128::from(local) - i128::from(CLOCK_BASE) - i128::from(self.offset);
        let rate = 1_000_000 + i128::from(self.drift_ppm);
        let estimate = (target * 1_000_000 / rate.max(1)).max(0);
        let mut guess = Millis::try_from(estimate).unwrap_or(Millis::MAX);
        while guess > 0 && self.local(guess - 1) >= local {
            guess -= 1;
        }
        while guess < Millis::MAX && self.local(guess) < local {
            guess += 1;
        }
        guess
    }
}

/// Ingress bounds per `(peer, class)` (O6).
const INGRESS_CAP: [usize; 3] = [256, 16, 8];

/// One class lane: a queue per peer, served round-robin (per-peer fairness, §12.2).
#[derive(Debug, Default)]
struct Lane {
    peers: BTreeMap<PublicKey, VecDeque<WireMessage>>,
    /// The peer served last.
    cursor: Option<PublicKey>,
    len: usize,
}

impl Lane {
    fn push(&mut self, from: PublicKey, msg: WireMessage, cap: usize) -> bool {
        let queue = self.peers.entry(from).or_default();
        if matches!(msg, WireMessage::Status(_))
            && let Some(slot) = queue
                .iter_mut()
                .find(|m| matches!(m, WireMessage::Status(_)))
        {
            *slot = msg;
            return false;
        }
        let dropped = queue.len() >= cap;
        if dropped {
            queue.pop_front();
            self.len -= 1;
        }
        queue.push_back(msg);
        self.len += 1;
        dropped
    }

    fn pop(&mut self) -> Option<(PublicKey, WireMessage)> {
        if self.len == 0 {
            return None;
        }
        let next = match &self.cursor {
            Some(last) => self
                .peers
                .range::<PublicKey, _>((
                    std::ops::Bound::Excluded(last),
                    std::ops::Bound::Unbounded,
                ))
                .find(|(_, q)| !q.is_empty())
                .or_else(|| self.peers.iter().find(|(_, q)| !q.is_empty()))
                .map(|(k, _)| k.clone()),
            None => self
                .peers
                .iter()
                .find(|(_, q)| !q.is_empty())
                .map(|(k, _)| k.clone()),
        }?;
        let queue = self.peers.get_mut(&next)?;
        let msg = queue.pop_front()?;
        if queue.is_empty() {
            self.peers.remove(&next);
        }
        self.len -= 1;
        self.cursor = Some(next.clone());
        Some((next, msg))
    }

    fn clear(&mut self) {
        self.peers.clear();
        self.cursor = None;
        self.len = 0;
    }
}

/// Ingress lanes of one replica (O5, O6, O8): local events first, then control, proposal and
/// bulk messages with strict priority and a minimum share for bulk; within a class, peers are
/// served round-robin with a bound per peer (the oldest message is dropped, the latest
/// `Status` replaces the previous one).
#[derive(Debug, Default)]
pub struct Lanes {
    /// FIFO mode: one queue for everything and ticks wait behind queued messages (only to
    /// show that `det_l12` detects the ML12 mutation).
    pub fifo: bool,
    local: VecDeque<Event>,
    classes: [Lane; 3],
    fifo_queue: VecDeque<(PublicKey, WireMessage)>,
    fifo_counts: BTreeMap<PublicKey, usize>,
    since_bulk: u32,
    /// Messages dropped by the ingress bounds.
    pub dropped: u64,
}

impl Lanes {
    /// Queue a local (non-message) event; never dropped (O6).
    pub fn push_local(&mut self, event: Event) {
        self.local.push_back(event);
    }

    /// Queue a message in its class lane.
    pub fn push_message(&mut self, from: PublicKey, msg: WireMessage, class: TrafficClass) {
        if self.fifo {
            // One global FIFO with a bound per peer.
            let count = self.fifo_counts.entry(from.clone()).or_insert(0);
            if *count >= INGRESS_CAP[0] {
                self.dropped += 1;
                return;
            }
            *count += 1;
            self.fifo_queue.push_back((from, msg));
            return;
        }
        let lane = super::net::lane(class);
        if self.classes[lane].push(from, msg, INGRESS_CAP[lane]) {
            self.dropped += 1;
        }
    }

    /// Whether anything is queued.
    pub fn is_empty(&self) -> bool {
        self.local.is_empty()
            && self.fifo_queue.is_empty()
            && self.classes.iter().all(|l| l.len == 0)
    }

    /// Queued events and messages.
    pub fn len(&self) -> usize {
        self.local.len() + self.fifo_queue.len() + self.classes.iter().map(|l| l.len).sum::<usize>()
    }

    /// Next event by priority.
    pub fn pop(&mut self) -> Option<Event> {
        if let Some(event) = self.local.pop_front() {
            return Some(event);
        }
        if let Some((from, msg)) = self.fifo_queue.pop_front() {
            if let Some(count) = self.fifo_counts.get_mut(&from) {
                *count = count.saturating_sub(1);
            }
            return Some(Event::Message { from, msg });
        }
        let bulk_turn = self.since_bulk >= 9 && self.classes[2].len > 0;
        let order: [usize; 3] = if bulk_turn { [2, 0, 1] } else { [0, 1, 2] };
        for lane in order {
            if let Some((from, msg)) = self.classes[lane].pop() {
                self.since_bulk = if lane == 2 { 0 } else { self.since_bulk + 1 };
                return Some(Event::Message { from, msg });
            }
        }
        None
    }

    /// Drop everything (crash).
    pub fn clear(&mut self) {
        self.local.clear();
        self.fifo_queue.clear();
        self.fifo_counts.clear();
        for lane in &mut self.classes {
            lane.clear();
        }
    }
}

/// A pending durable write.
#[derive(Clone, Debug)]
pub enum Write {
    /// A safety record for a key.
    Record(Box<SafetyRecord>, Vec<u8>),
    /// A block body.
    Body(Box<Block>),
    /// A committed block and its `CommitQC` (block store), then apply.
    Commit(Box<(Block, Qc)>),
    /// A write of a host that owns its scheduling (§13.5): operation id, whether it succeeds
    /// (a failed write stores nothing), and what it writes.
    Owned {
        /// The host's operation id.
        op: u64,
        /// Whether the write succeeds.
        ok: bool,
        /// What it writes.
        write: OwnedWrite,
    },
}

/// What a write of a host that owns its scheduling stores (§13.5).
#[derive(Clone, Debug)]
pub enum OwnedWrite {
    /// A safety record and its encoding.
    Record(Box<SafetyRecord>, Vec<u8>),
    /// A block body.
    Body(Box<Block>),
    /// A committed block and its `CommitQC`, appended to the block store (no apply).
    Append(Box<(Block, Qc)>),
}

/// The write device of a replica: FIFO completion; a write is durable at its completion and
/// lost if the machine crashes first.
#[derive(Debug, Default)]
pub struct Io {
    next_id: u64,
    /// Pending writes in completion order.
    pub pending: VecDeque<(u64, Millis, Write)>,
    last_done: Millis,
}

impl Io {
    /// Enqueue a write that becomes durable `latency` after both `now` and the previous write.
    /// Returns `(id, completion time)`.
    pub fn write(&mut self, now: Millis, latency: Millis, write: Write) -> (u64, Millis) {
        self.next_id += 1;
        let id = self.next_id;
        let done = self.last_done.max(now).saturating_add(latency);
        self.last_done = done;
        self.pending.push_back((id, done, write));
        (id, done)
    }

    /// Complete the writes up to `id` and return them in order.
    pub fn complete(&mut self, id: u64) -> Vec<Write> {
        let mut done = Vec::new();
        while self.pending.front().is_some_and(|(w, _, _)| *w <= id) {
            if let Some((_, _, write)) = self.pending.pop_front() {
                done.push(write);
            }
        }
        done
    }

    /// Lose every non-durable write (crash).
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// A body in a pending (not yet durable) write.
    pub fn pending_body(&self, bh: &Hash32, crypto: &dyn crate::crypto::Crypto) -> Option<Block> {
        self.pending.iter().find_map(|(_, _, w)| match w {
            Write::Body(block)
            | Write::Owned {
                write: OwnedWrite::Body(block),
                ..
            } if block.hash(crypto) == *bh => Some((**block).clone()),
            _ => None,
        })
    }
}

/// The O2 persist-before-effect barrier (§12.3) of the fake driver: after a `PersistSafety`
/// write, every externally visible effect waits until that record is durable, and is lost
/// with it on a crash.
#[derive(Debug, Default)]
pub struct Barrier {
    barrier: Option<u64>,
    /// Effects waiting for the barrier: `(record write id, action)`.
    pub held: VecDeque<(u64, Action)>,
}

impl Barrier {
    /// The safety record of write `write` is on its way to the device: later effects wait
    /// for it.
    pub fn persisting(&mut self, write: u64) {
        self.barrier = Some(write);
    }

    /// Whether an effect must wait; if so it is held.
    pub fn hold(&mut self, action: Action) -> Option<Action> {
        // MS24: the O2 barrier holds only `Send`/`Broadcast`.
        #[cfg(sumeragi_mutation = "MS24")]
        if !matches!(action, Action::Send { .. } | Action::Broadcast { .. }) {
            return Some(action);
        }
        match self.barrier {
            Some(id) => {
                self.held.push_back((id, action));
                None
            }
            None => Some(action),
        }
    }

    /// The writes up to `write` are durable: release the effects waiting for them, in order.
    pub fn release(&mut self, write: u64) -> Vec<Action> {
        if self.barrier.is_some_and(|b| b <= write) {
            self.barrier = None;
        }
        let mut released = Vec::new();
        while self.held.front().is_some_and(|(w, _)| *w <= write) {
            if let Some((_, action)) = self.held.pop_front() {
                released.push(action);
            }
        }
        released
    }

    /// Lose every held effect (crash).
    pub fn clear(&mut self) {
        self.held.clear();
        self.barrier = None;
    }
}

/// One execution request.
#[derive(Clone, Debug)]
pub struct Job {
    /// Block hash.
    pub bh: Hash32,
    /// Request id of the `Execute`.
    pub req: u64,
    /// The block.
    pub block: Block,
    /// Cancelled while running.
    pub cancelled: bool,
    /// Job id.
    pub id: u64,
    /// Outcome, fixed when the job starts.
    pub outcome: Option<ExecOutcome>,
}

/// The executor of a replica (O4): one job at a time, most recent request first, every
/// request answered exactly once, `Cancelled` for discarded work, and a post-state cache keyed
/// by block hash.
#[derive(Debug, Default)]
pub struct Executor {
    /// Queued jobs (served LIFO).
    pub queue: Vec<Job>,
    /// Jobs waiting for their parent's post-state.
    pub parked: Vec<Job>,
    /// The running job and its finish time.
    pub running: Option<(Job, Millis)>,
    /// Post-states: block hash → (height, result).
    pub cache: BTreeMap<Hash32, (u64, Hash32)>,
    /// The statements of the `Valid` executions, shared with the node's execution-gated
    /// attestor (§3.7 A2: `R`'s preimage comes from the node's own execution).
    pub executed: Executed,
    /// For a host that owns its scheduling (§13.5): the block whose post-state its last
    /// `Prepare` made ready to commit. Any other executor operation drops it (the application
    /// may hold one live overlay), and a `Commit` needs it.
    pub prepared: Option<Hash32>,
    next_job: u64,
}

impl Executor {
    /// Queue an `Execute`.
    pub fn submit(&mut self, bh: Hash32, req: u64, block: Block) {
        self.next_job += 1;
        self.queue.push(Job {
            bh,
            req,
            block,
            cancelled: false,
            id: self.next_job,
            outcome: None,
        });
    }

    /// `DiscardExecution{height, keep}`: returns the `(bh, req)` of queued or parked jobs to
    /// answer `Cancelled` at once; a running job is flagged and answered when it finishes, or,
    /// with `abort`, aborted and answered at once too (the executor is then free).
    pub fn discard(&mut self, height: u64, keep: &[Hash32], abort: bool) -> Vec<(Hash32, u64)> {
        let drop = |job: &Job| job.block.header.height == height && !keep.contains(&job.bh);
        let mut cancelled = Vec::new();
        for list in [&mut self.queue, &mut self.parked] {
            list.retain(|job| {
                if drop(job) {
                    cancelled.push((job.bh, job.req));
                    false
                } else {
                    true
                }
            });
        }
        if let Some((job, _)) = self.running.as_mut()
            && drop(job)
        {
            job.cancelled = true;
            if abort {
                cancelled.push((job.bh, job.req));
                self.running = None;
            }
        }
        self.cache
            .retain(|bh, (h, _)| *h != height || keep.contains(bh));
        cancelled
    }

    /// Lose all volatile state (crash).
    pub fn clear(&mut self) {
        self.queue.clear();
        self.parked.clear();
        self.running = None;
        self.cache.clear();
        self.executed.clear();
        self.prepared = None;
    }

    /// Outstanding requests (queued, parked, running).
    pub fn outstanding(&self) -> usize {
        self.queue.len() + self.parked.len() + usize::from(self.running.is_some())
    }
}

/// Transaction tag byte.
const TX_TAG: u8 = 0x54;
/// Fixed transaction header: tag, id, flags, padding length.
pub const TX_HEADER: usize = 1 + 8 + 1 + 2;

/// Transaction flag: the transaction is poison (every block holding it is `Invalid`).
const TX_POISON: u8 = 0x01;
/// Transaction flag: the transaction needs mint finality, so its block is flagged (§3.7, F37).
const TX_MINT: u8 = 0x02;

/// Encode a transaction: `0x54 ‖ be64(id) ‖ flags ‖ be16(pad) ‖ pad bytes`; flag bit 0 = poison.
pub fn encode_tx(id: u64, poison: bool, pad: u16) -> Vec<u8> {
    encode_tx_flagged(id, poison, false, pad)
}

/// [`encode_tx`] with flag bit 1 = the transaction needs mint finality (§3.7, F37).
pub fn encode_tx_flagged(id: u64, poison: bool, mint: bool, pad: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(TX_HEADER + usize::from(pad));
    out.push(TX_TAG);
    out.extend_from_slice(&id.to_be_bytes());
    out.push(if poison { TX_POISON } else { 0 } | if mint { TX_MINT } else { 0 });
    out.extend_from_slice(&pad.to_be_bytes());
    out.extend(std::iter::repeat_n(0xab, usize::from(pad)));
    out
}

/// Decode the transactions of a payload as `(id, poison)`; trailing garbage is ignored.
pub fn decode_txs(payload: &[u8]) -> Vec<(u64, bool)> {
    let mut out = Vec::new();
    let mut rest = payload;
    while rest.len() >= TX_HEADER && rest[0] == TX_TAG {
        let Ok(id) = <[u8; 8]>::try_from(&rest[1..9]) else {
            break;
        };
        let poison = rest[9] & TX_POISON != 0;
        let pad = usize::from(u16::from_be_bytes([rest[10], rest[11]]));
        let len = TX_HEADER + pad;
        if rest.len() < len {
            break;
        }
        out.push((u64::from_be_bytes(id), poison));
        rest = &rest[len..];
    }
    out
}

/// The application's flag rule of the simulator (§3.7 A1): a block needs attestations iff its
/// payload carries a mint transaction (`EMPTY` never does).
pub fn payload_mints(payload: &[u8]) -> bool {
    let mut rest = payload;
    while rest.len() >= TX_HEADER && rest[0] == TX_TAG {
        if rest[9] & TX_MINT != 0 {
            return true;
        }
        let len = TX_HEADER + usize::from(u16::from_be_bytes([rest[10], rest[11]]));
        let Some(tail) = rest.get(len..) else {
            break;
        };
        rest = tail;
    }
    false
}

/// The simulator's `exec` of a block (§4.1, §4.2): `Invalid` if its header flag differs from
/// the application's flag rule ([`payload_mints`]), otherwise [`reference_exec`].
pub fn block_exec(parent_result: &Hash32, block: &Block) -> ExecOutcome {
    if block.header.attest != payload_mints(&block.payload) {
        return ExecOutcome::Invalid;
    }
    reference_exec(parent_result, &block.payload)
}

/// The deterministic reference execution `R = H(parent_R ‖ payload)`; `Invalid` iff the
/// payload carries a poison transaction (§13.1).
pub fn reference_exec(parent_result: &Hash32, payload: &[u8]) -> ExecOutcome {
    if decode_txs(payload).iter().any(|(_, poison)| *poison) {
        return ExecOutcome::Invalid;
    }
    let mut input = parent_result.0.to_vec();
    input.extend_from_slice(payload);
    ExecOutcome::Valid(Hash32(sha256(&input)))
}

/// A divergent executor's result (F21): a different commitment for the same input, or
/// `Invalid` for about half of the blocks (by block hash).
pub fn divergent_exec(parent_result: &Hash32, payload: &[u8], block_hash: &Hash32) -> ExecOutcome {
    match reference_exec(parent_result, payload) {
        ExecOutcome::Valid(_) if block_hash.0[0] % 2 == 1 => ExecOutcome::Invalid,
        ExecOutcome::Valid(r) => {
            let mut input = r.0.to_vec();
            input.push(0xd1);
            ExecOutcome::Valid(Hash32(sha256(&input)))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::BlockHeader;

    #[test]
    fn clock_roundtrip() {
        for clock in [
            Clock::default(),
            Clock {
                offset: -10_000,
                drift_ppm: -10_000,
            },
            Clock {
                offset: 9_999,
                drift_ppm: 10_000,
            },
        ] {
            let mut prev = 0;
            for t in (0..50_000).step_by(997) {
                let l = clock.local(t);
                assert!(l >= prev, "monotonic");
                prev = l;
                let g = clock.global_at(l);
                assert!(clock.local(g) >= l && g <= t);
                assert!(g == 0 || clock.local(g - 1) < l);
            }
        }
        assert_eq!(Clock::default().global_at(Millis::MAX), Millis::MAX);
    }

    #[test]
    fn lanes_priority_and_bounds() {
        let k = PublicKey::new(vec![1; 32]).unwrap();
        let mut lanes = Lanes::default();
        let req = WireMessage::BlockRequest(crate::message::BlockRequest {
            instance: Hash32::ZERO,
            height: 1,
            block_hash: Hash32::ZERO,
        });
        for _ in 0..300 {
            lanes.push_message(k.clone(), req.clone(), TrafficClass::Control);
        }
        assert_eq!(lanes.len(), 256);
        assert_eq!(lanes.dropped, 44);
        lanes.push_local(Event::Tick);
        assert_eq!(lanes.pop(), Some(Event::Tick));
        assert!(matches!(lanes.pop(), Some(Event::Message { .. })));
        lanes.clear();
        assert!(lanes.is_empty());
    }

    #[test]
    fn io_barrier() {
        let mut io = Io::default();
        let mut barrier = Barrier::default();
        let block = Block {
            header: BlockHeader {
                instance: Hash32::ZERO,
                height: 1,
                origin_view: 0,
                parent_hash: Hash32::ZERO,
                parent_result: Hash32::ZERO,
                payload_hash: Hash32::ZERO,
                payload_len: 0,
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
            },
            payload: Vec::new(),
        };
        let (b, t1) = io.write(0, 5, Write::Body(Box::new(block.clone())));
        assert!(
            barrier
                .hold(Action::Halt(crate::api::HaltReason::SafetyRecordCorrupt))
                .is_some()
        );
        let record =
            SafetyRecord::fresh(Hash32::ZERO, PublicKey::new(vec![1; 32]).unwrap(), 0, None);
        let (r, t2) = io.write(0, 5, Write::Record(Box::new(record), Vec::new()));
        barrier.persisting(r);
        assert!(t2 >= t1 + 5);
        assert!(
            barrier
                .hold(Action::LocalFault(crate::api::LocalFault::RecordMissing))
                .is_none()
        );
        assert_eq!((io.complete(b).len(), barrier.release(b).len()), (1, 0));
        assert_eq!((io.complete(r).len(), barrier.release(r).len()), (1, 1));
        assert!(
            barrier
                .hold(Action::LocalFault(crate::api::LocalFault::RecordMissing))
                .is_some()
        );
        io.clear();
        barrier.clear();
        assert!(io.pending.is_empty() && barrier.held.is_empty());
    }

    #[test]
    fn mint_flag_rule() {
        let mut payload = encode_tx(1, false, 2);
        assert!(!payload_mints(&payload));
        payload.extend(encode_tx_flagged(2, false, true, 1));
        assert!(payload_mints(&payload));
        assert_eq!(decode_txs(&payload), vec![(1, false), (2, false)]);
        assert!(!payload_mints(&[]));
        assert!(!payload_mints(&[TX_TAG, 0, 0]), "truncated");
        let header = BlockHeader {
            instance: Hash32::ZERO,
            height: 1,
            origin_view: 0,
            parent_hash: Hash32::ZERO,
            parent_result: Hash32::ZERO,
            payload_hash: Hash32::ZERO,
            payload_len: 0,
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: false,
        };
        let block = |attest: bool, payload: Vec<u8>| Block {
            header: BlockHeader {
                attest,
                ..header.clone()
            },
            payload,
        };
        let parent = Hash32([1; 32]);
        assert_eq!(
            block_exec(&parent, &block(false, Vec::new())),
            reference_exec(&parent, &[])
        );
        assert_eq!(
            block_exec(&parent, &block(true, Vec::new())),
            ExecOutcome::Invalid
        );
        assert_eq!(
            block_exec(&parent, &block(false, payload.clone())),
            ExecOutcome::Invalid
        );
        assert_eq!(
            block_exec(&parent, &block(true, payload.clone())),
            reference_exec(&parent, &payload)
        );
    }

    #[test]
    fn txs_and_exec() {
        let mut payload = encode_tx(7, false, 3);
        payload.extend(encode_tx(9, true, 0));
        payload.push(0xff);
        assert_eq!(decode_txs(&payload), vec![(7, false), (9, true)]);
        assert_eq!(
            reference_exec(&Hash32::ZERO, &payload),
            ExecOutcome::Invalid
        );
        let good = encode_tx(1, false, 0);
        assert_ne!(
            reference_exec(&Hash32::ZERO, &good),
            divergent_exec(&Hash32::ZERO, &good, &Hash32([2; 32]))
        );
        assert_eq!(
            divergent_exec(&Hash32::ZERO, &good, &Hash32([1; 32])),
            ExecOutcome::Invalid
        );
        assert!(matches!(
            reference_exec(&Hash32::ZERO, &[]),
            ExecOutcome::Valid(_)
        ));
    }
}
