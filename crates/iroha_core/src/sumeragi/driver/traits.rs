//! Backend traits of the Sumeragi driver (`specs/sumeragi.md` §12.2, §13.5): the transport,
//! the safety-record store with its store id and installation log, the body store, the block
//! store, the clock, the application's executor and an observer of reports.
//!
//! The node implements them over P2P, files, Kura and State; the tests and the §13.5
//! conformance runs use in-memory fakes. Every method either returns at once or blocks only the
//! driver thread that owns the call (persistence, execution, serving), never the event loop.

use std::{io, sync::Arc, time::Instant};

use iroha_sumeragi::{
    api::{ExecOutcome, HaltReason, LocalFault},
    message::{Block, Evidence, Qc, SyncEntry, TrafficClass},
    safety::RecordState,
    types::{Hash32, HeightConfig, Millis, PublicKey},
};

use super::{FrameLimitExceeded, Worker};

/// An encoded wire message for the transport: the exact `WireMessage::encode()` bytes (encoded
/// once and shared by every recipient), the instance they belong to and their O8 class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Instance id of the message (routing).
    pub instance: Hash32,
    /// Traffic class (§12.3 O8).
    pub class: TrafficClass,
    /// The canonical frame.
    pub bytes: Arc<[u8]>,
}

/// The authenticated transport (§12.2): posts a frame to one peer in its traffic class.
pub trait Net: Send + Sync {
    /// Post `frame` to `to` without blocking; under backpressure the frame is dropped (the core
    /// rebroadcasts, §12.3 O6).
    fn send(&self, to: &PublicKey, frame: &Frame);
}

/// One entry of the installation log (§7.4 record provenance), written with a fresh store id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEntry {
    /// A key installed on the node: generated here and never exported, or imported.
    Key {
        /// The key.
        key: PublicKey,
        /// Generated on this node.
        generated: bool,
        /// Store id drawn for this entry.
        store_id: u128,
    },
    /// Instance `instance` was started with `key` on this node.
    Instance {
        /// Instance id.
        instance: Hash32,
        /// The key.
        key: PublicKey,
        /// Store id drawn for this entry.
        store_id: u128,
    },
}

impl LogEntry {
    /// The store id this entry was written with.
    pub fn store_id(&self) -> u128 {
        match self {
            Self::Key { store_id, .. } | Self::Instance { store_id, .. } => *store_id,
        }
    }

    /// The key of this entry.
    pub fn key(&self) -> &PublicKey {
        match self {
            Self::Key { key, .. } | Self::Instance { key, .. } => key,
        }
    }
}

/// Durable safety records, one per `(instance, key)`, the store-id file next to them, and the
/// installation log kept elsewhere (§7.4). Record files are never backed up or restored.
pub trait RecordStore: Send + Sync {
    /// What is on disk for `(instance, key)`.
    ///
    /// # Errors
    /// A read failure (the driver does not start).
    fn load(&self, instance: &Hash32, key: &PublicKey) -> io::Result<RecordState>;
    /// Atomically and durably replace the record of `(instance, key)` (temp file, fsync,
    /// rename, directory fsync).
    ///
    /// # Errors
    /// A write failure; nothing was replaced (the driver retries, never skips).
    fn write(&self, instance: &Hash32, key: &PublicKey, bytes: &[u8]) -> io::Result<()>;
    /// The store id next to the record files (`None`: no id file).
    ///
    /// # Errors
    /// A read failure.
    fn store_id(&self) -> io::Result<Option<u128>>;
    /// Durably replace the store-id file.
    ///
    /// # Errors
    /// A write failure.
    fn set_store_id(&self, id: u128) -> io::Result<()>;
    /// The installation log, oldest entry first.
    ///
    /// # Errors
    /// A read failure.
    fn log(&self) -> io::Result<Vec<LogEntry>>;
    /// Durably append an entry to the installation log.
    ///
    /// # Errors
    /// A write failure.
    fn append_log(&self, entry: &LogEntry) -> io::Result<()>;
}

/// Durable store of block bodies the core accepted and that are not applied yet (§7.4 body
/// durability), keyed by `(height, block_hash)`.
pub trait BodyStore: Send + Sync {
    /// Durably store `block` (hash `block_hash`).
    ///
    /// # Errors
    /// A write failure (the driver retries, never skips).
    fn put(&self, block_hash: &Hash32, block: &Block) -> io::Result<()>;
    /// The stored body of `(height, block_hash)`, if held.
    fn get(&self, height: u64, block_hash: &Hash32) -> Option<Block>;
    /// Drop every body at or below `height` (applied heights live in the block store).
    ///
    /// # Errors
    /// A write failure (retried like a store).
    fn prune_through(&self, height: u64) -> io::Result<()>;
}

/// The committed chain (Kura in the node): one block and its `CommitQC` per height above
/// genesis, written durably and in height order.
pub trait BlockStore: Send + Sync {
    /// Highest stored height (the genesis height when nothing is stored above it).
    fn height(&self) -> u64;
    /// The committed block and `CommitQC` of `height`, if stored.
    fn entry(&self, height: u64) -> Option<SyncEntry>;
    /// Durably append the next height.
    ///
    /// # Errors
    /// A write failure; nothing was appended (the driver retries, never skips).
    fn append(&self, block: &Block, commit_qc: &Qc) -> io::Result<()>;
}

/// A monotonic local clock in milliseconds (§1.5: no synchronised time is assumed).
pub trait Clock: Send + Sync {
    /// Local time now.
    fn now(&self) -> Millis;
}

/// The process clock: milliseconds since construction.
#[derive(Clone, Copy, Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock that reads 0 now.
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Millis {
        Millis::try_from(self.origin.elapsed().as_millis()).unwrap_or(Millis::MAX)
    }
}

/// The application: speculative execution with a post-state cache keyed by block hash, apply,
/// the payload builder and its quarantine (§12.2, O3, O4). The driver calls it from one thread,
/// one call at a time, and schedules the calls (parking, most recent first, apply in order).
///
/// Apply sequencing: after a successful [`prepare`](Self::prepare) of a block, the driver's next
/// executor call is [`commit`](Self::commit) of the same block (only the block-store append
/// happens in between, retried as long as it fails), so the prepared post-state may be a single
/// live overlay. If `commit` fails, the driver calls `prepare` of that block again before it
/// retries `commit`; the block is then already in the block store.
pub trait Executor: Send {
    /// Execute `block` (hash `block_hash`) on its parent's post-state: the applied state or a
    /// cached post-state. `None` if that post-state is not held (not an error: the driver parks
    /// the request until the parent is applied or executed, O4). A `Valid` post-state is kept
    /// until the block is applied or discarded.
    fn execute(&mut self, block: &Block, block_hash: &Hash32) -> Option<ExecOutcome>;
    /// Drop the post-states of the blocks at `height` other than `keep`.
    fn discard(&mut self, height: u64, keep: &[Hash32]);
    /// The post-state of the next committed block (its parent is the applied state): the cached
    /// one if its commitment equals `commit_qc.result`, otherwise by executing the block (a
    /// missing cache entry is never a divergence, O3). Returns the local commitment, or `None`
    /// if the block does not execute `Valid` locally.
    ///
    /// # Errors
    /// A local failure (I/O, resources); the driver retries.
    fn prepare(&mut self, block: &Block, commit_qc: &Qc) -> Result<Option<Hash32>, String>;
    /// Make the prepared post-state of `block` the applied state (after the block store holds
    /// it) and return the configuration of `height + 2` it schedules. Called only right after
    /// a successful `prepare` of `block` (see the trait documentation).
    ///
    /// # Errors
    /// A local failure; the driver prepares again and retries.
    fn commit(&mut self, block: &Block, commit_qc: &Qc) -> Result<HeightConfig, String>;
    /// Build a payload of at most `max_bytes` for `(height, view)` by peeking at the queue
    /// (never removing transactions) and return it with its commit-attestation flag (§3.7 A1).
    fn build(
        &mut self,
        height: u64,
        view: u64,
        max_bytes: u32,
        exec_budget_ms: u32,
    ) -> (Vec<u8>, bool);
    /// The payload of `block_hash` executed `Invalid`: quarantine the transactions that make a
    /// block invalid on their own.
    fn reject(&mut self, height: u64, view: u64, block_hash: &Hash32);
}

/// Receives the core's reports — evidence of signed misbehaviour, local faults and a halt —
/// and the driver's own (telemetry and the evidence log in the node).
pub trait Observer: Send + Sync {
    /// Evidence the core reports (after its O2 barrier).
    fn evidence(&self, _evidence: &Evidence) {}
    /// A local fault.
    fn fault(&self, _fault: &LocalFault) {}
    /// The instance halted.
    fn halt(&self, _reason: &HaltReason) {}
    /// A thread of the instance stopped, or a worker could not be reached: the instance
    /// stopped with it (a restart recovers).
    fn stopped(&self, _worker: Worker) {}
    /// A committed configuration outgrows the transport's frame limit (O10).
    fn frame_limit(&self, _exceeded: &FrameLimitExceeded) {}
}

/// An observer that ignores every report.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoObserver;

impl Observer for NoObserver {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_entry_accessors() {
        let key = PublicKey::new(vec![1; 32]).unwrap();
        let a = LogEntry::Key {
            key: key.clone(),
            generated: true,
            store_id: 7,
        };
        let b = LogEntry::Instance {
            instance: Hash32([2; 32]),
            key: key.clone(),
            store_id: 9,
        };
        assert_eq!((a.store_id(), b.store_id()), (7, 9));
        assert_eq!((a.key(), b.key()), (&key, &key));
    }

    #[test]
    fn system_clock_is_monotonic() {
        let clock = SystemClock::default();
        let a = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(clock.now() >= a + 1);
    }

    #[test]
    fn no_observer_ignores_reports() {
        let observer = NoObserver;
        observer.fault(&LocalFault::RecordMissing);
        observer.halt(&HaltReason::DriverAnomaly);
        observer.stopped(Worker::Exec);
        observer.frame_limit(&FrameLimitExceeded {
            height: 1,
            needed: 2,
            limit: 1,
        });
    }
}
