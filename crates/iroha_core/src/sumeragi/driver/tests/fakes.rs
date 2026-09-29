//! In-memory backends for the driver tests (§13.5): a transport that records frames, a record
//! store with the store id and installation-log semantics of §7.4, a body store, a block store
//! (with injectable failures, panics and slow reads), a manual clock, and a deterministic
//! executor `R = H(parent_R ‖ payload)` (the simulator's reference execution) with a post-state
//! cache, execution counts, injectable failures and a gate that holds executions.

use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc, Condvar,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use iroha_sumeragi::{
    api::ExecOutcome,
    message::{Block, Qc, SyncEntry, WireMessage},
    safety::RecordState,
    sim::driver::{block_exec, encode_tx, payload_mints},
    types::{Hash32, HeightConfig, Millis, PublicKey},
};
use parking_lot::Mutex;

use super::super::{
    DriverHandle, FrameLimitExceeded, Worker,
    traits::{
        BlockStore, BodyStore, Clock, Executor, Frame, LogEntry, Net, Observer, PublicationError,
        RecordStore,
    },
};

/// A transport that records every frame.
#[derive(Default)]
pub struct FakeNet {
    sent: Mutex<Vec<(PublicKey, Frame)>>,
}

impl FakeNet {
    /// Frames sent so far, decoded.
    pub fn sent(&self) -> Vec<(PublicKey, WireMessage)> {
        self.sent
            .lock()
            .iter()
            .filter_map(|(to, frame)| {
                WireMessage::decode(&frame.bytes, usize::MAX)
                    .ok()
                    .map(|m| (to.clone(), m))
            })
            .collect()
    }
}

impl Net for FakeNet {
    fn send(&self, to: &PublicKey, frame: &Frame) {
        self.sent.lock().push((to.clone(), frame.clone()));
    }
}

#[derive(Default)]
struct Records {
    files: BTreeMap<(Hash32, PublicKey), Vec<u8>>,
    store_id: Option<u128>,
    log: Vec<LogEntry>,
    fail: u32,
    panic: u32,
    writes: u64,
}

/// Record files, the store-id file and the installation log (§7.4); the next writes can be
/// made to fail (ENOSPC/EIO).
#[derive(Default)]
pub struct FakeRecords(Mutex<Records>);

impl FakeRecords {
    /// Install a key the way key generation does: the id file, then the key entry.
    pub fn install_key(&self, key: &PublicKey, generated: bool, store_id: u128) {
        let mut inner = self.0.lock();
        inner.store_id = Some(store_id);
        inner.log.push(LogEntry::Key {
            key: key.clone(),
            generated,
            store_id,
        });
    }

    /// The installation log.
    pub fn snapshot_log(&self) -> Vec<LogEntry> {
        self.0.lock().log.clone()
    }

    /// Restore the key store (with its log) from a backup; record files and the id stay.
    pub fn restore_log(&self, log: Vec<LogEntry>) {
        self.0.lock().log = log;
    }

    /// A new, empty record store: every record file and the store-id file are gone.
    pub fn replace_records(&self) {
        let mut inner = self.0.lock();
        inner.files.clear();
        inner.store_id = None;
    }

    /// Fail the next `n` record writes.
    pub fn fail_next(&self, n: u32) {
        self.0.lock().fail = n;
    }

    /// Panic in the next `n` record writes.
    pub fn panic_next(&self, n: u32) {
        self.0.lock().panic = n;
    }

    /// The bytes of the record of `(instance, key)`.
    pub fn bytes(&self, instance: &Hash32, key: &PublicKey) -> Option<Vec<u8>> {
        self.0.lock().files.get(&(*instance, key.clone())).cloned()
    }

    /// Successful record writes so far.
    pub fn writes(&self) -> u64 {
        self.0.lock().writes
    }
}

impl RecordStore for FakeRecords {
    fn load(&self, instance: &Hash32, key: &PublicKey) -> io::Result<RecordState> {
        Ok(self
            .bytes(instance, key)
            .map_or(RecordState::Absent, RecordState::Present))
    }

    fn write(&self, instance: &Hash32, key: &PublicKey, bytes: &[u8]) -> io::Result<()> {
        let mut inner = self.0.lock();
        if inner.panic > 0 {
            inner.panic -= 1;
            drop(inner);
            panic!("injected record store panic");
        }
        if inner.fail > 0 {
            inner.fail -= 1;
            return Err(io::Error::other("injected ENOSPC"));
        }
        inner.writes += 1;
        inner.files.insert((*instance, key.clone()), bytes.to_vec());
        Ok(())
    }

    fn store_id(&self) -> io::Result<Option<u128>> {
        Ok(self.0.lock().store_id)
    }

    fn set_store_id(&self, id: u128) -> io::Result<()> {
        self.0.lock().store_id = Some(id);
        Ok(())
    }

    fn log(&self) -> io::Result<Vec<LogEntry>> {
        Ok(self.snapshot_log())
    }

    fn append_log(&self, entry: &LogEntry) -> io::Result<()> {
        self.0.lock().log.push(entry.clone());
        Ok(())
    }
}

/// Block bodies by `(height, hash)`; the next writes can be made to fail.
#[derive(Default)]
pub struct FakeBodies {
    bodies: Mutex<BTreeMap<(u64, Hash32), Block>>,
    fail: Mutex<u32>,
}

impl FakeBodies {
    /// Fail the next `n` writes.
    pub fn fail_next(&self, n: u32) {
        *self.fail.lock() = n;
    }

    /// Bodies held.
    pub fn len(&self) -> usize {
        self.bodies.lock().len()
    }
}

impl BodyStore for FakeBodies {
    fn put(&self, block_hash: &Hash32, block: &Block) -> io::Result<()> {
        let mut fail = self.fail.lock();
        if *fail > 0 {
            *fail -= 1;
            return Err(io::Error::other("injected EIO"));
        }
        self.bodies
            .lock()
            .insert((block.header.height, *block_hash), block.clone());
        Ok(())
    }

    fn get(&self, height: u64, block_hash: &Hash32) -> Option<Block> {
        self.bodies.lock().get(&(height, *block_hash)).cloned()
    }

    fn prune_through(&self, height: u64) -> io::Result<()> {
        self.bodies.lock().retain(|(h, _), _| *h > height);
        Ok(())
    }
}

/// The committed chain above genesis height 0; the next appends can be made to fail or panic,
/// the next reads to panic, and every read to take time.
#[derive(Default)]
pub struct FakeBlocks {
    entries: Mutex<Vec<SyncEntry>>,
    fail: Mutex<u32>,
    panic_appends: Mutex<u32>,
    panic_reads: Mutex<u32>,
    read_delay_ms: AtomicU64,
    reads: AtomicU64,
}

impl FakeBlocks {
    /// Fail the next `n` appends.
    pub fn fail_next(&self, n: u32) {
        *self.fail.lock() = n;
    }

    /// Panic in the next `n` appends.
    pub fn panic_appends(&self, n: u32) {
        *self.panic_appends.lock() = n;
    }

    /// Panic in the next `n` reads (`entry`).
    pub fn panic_reads(&self, n: u32) {
        *self.panic_reads.lock() = n;
    }

    /// Make every read (`entry`) take `ms` milliseconds (a slow disk).
    pub fn set_read_delay(&self, ms: u64) {
        self.read_delay_ms.store(ms, Ordering::SeqCst);
    }

    /// Reads (`entry`) so far.
    pub fn reads(&self) -> u64 {
        self.reads.load(Ordering::SeqCst)
    }
}

/// Take one of `counter`'s injected events, if any is left.
fn take(counter: &Mutex<u32>) -> bool {
    let mut left = counter.lock();
    let hit = *left > 0;
    *left = left.saturating_sub(1);
    hit
}

impl BlockStore for FakeBlocks {
    fn height(&self) -> u64 {
        u64::try_from(self.entries.lock().len()).unwrap_or(u64::MAX)
    }

    fn entry(&self, height: u64) -> Option<SyncEntry> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let delay = self.read_delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        assert!(!take(&self.panic_reads), "injected block store read panic");
        let index = usize::try_from(height.checked_sub(1)?).ok()?;
        self.entries.lock().get(index).cloned()
    }

    fn append(&self, block: &Block, commit_qc: &Qc) -> io::Result<()> {
        assert!(
            !take(&self.panic_appends),
            "injected block store append panic"
        );
        let mut fail = self.fail.lock();
        if *fail > 0 {
            *fail -= 1;
            return Err(io::Error::other("injected EIO"));
        }
        let mut entries = self.entries.lock();
        let next = u64::try_from(entries.len()).unwrap_or(u64::MAX) + 1;
        if block.header.height != next {
            return Err(io::Error::other("append out of order"));
        }
        entries.push(SyncEntry {
            block: block.clone(),
            commit_qc: commit_qc.clone(),
        });
        Ok(())
    }
}

/// A manually advanced clock.
#[derive(Debug, Default)]
pub struct FakeClock(AtomicU64);

impl FakeClock {
    /// Advance by `ms`.
    pub fn advance(&self, ms: Millis) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Millis {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a [`FakeExecutor`] holds; shared with its test.
pub struct ExecState {
    /// Applied `(height, block hash, result)`.
    pub applied: (u64, Hash32, Hash32),
    /// Post-states: block hash → (height, result).
    pub cache: BTreeMap<Hash32, (u64, Hash32)>,
    /// Executions per block (O3: one, not two).
    pub executions: BTreeMap<Hash32, u32>,
    /// Transactions the builder peeks at.
    pub txs: Vec<Vec<u8>>,
    /// Rejected blocks.
    pub rejected: Vec<Hash32>,
    /// Fail the next `prepare`/`commit` calls.
    pub fail_apply: u32,
    /// The configuration every commit schedules.
    pub config: HeightConfig,
}

/// A deterministic executor over a shared [`ExecState`], with a gate that holds executions and
/// builds (a stalled executor) while closed.
#[derive(Clone)]
pub struct FakeExecutor {
    /// The shared state.
    pub state: Arc<Mutex<ExecState>>,
    gate: Arc<(std::sync::Mutex<bool>, Condvar)>,
    waiting: Arc<AtomicUsize>,
}

impl FakeExecutor {
    /// An executor whose applied state is the genesis `(0, genesis_hash, genesis_result)`.
    pub fn new(genesis_hash: Hash32, genesis_result: Hash32, config: HeightConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(ExecState {
                applied: (0, genesis_hash, genesis_result),
                cache: BTreeMap::new(),
                executions: BTreeMap::new(),
                txs: Vec::new(),
                rejected: Vec::new(),
                fail_apply: 0,
                config,
            })),
            gate: Arc::new((std::sync::Mutex::new(true), Condvar::new())),
            waiting: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Hold (`false`) or release (`true`) every execution and build.
    pub fn set_open(&self, open: bool) {
        let (lock, cvar) = &*self.gate;
        *lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = open;
        cvar.notify_all();
    }

    /// Number of original operations currently held behind the test latch.
    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::Acquire)
    }

    fn wait_open(&self) {
        let (lock, cvar) = &*self.gate;
        let mut open = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !*open {
            self.waiting.fetch_add(1, Ordering::AcqRel);
            while !*open {
                open = cvar
                    .wait(open)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            self.waiting.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Add a transaction for the builder.
    pub fn add_tx(&self, id: u64) {
        self.state.lock().txs.push(encode_tx(id, false, 8));
    }

    /// Keep one transaction queued for the builder of the instance behind `handle` until the
    /// returned pump is dropped. Blocks are work-driven and never empty (`specs/sumeragi.md`
    /// §6.10), so a test chain advances only while its builder has work.
    pub fn pump(&self, handle: DriverHandle) -> WorkPump {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (stop, exec) = (Arc::clone(&stop), self.clone());
            std::thread::spawn(move || {
                let mut id = 1_u64 << 32;
                while !stop.load(Ordering::SeqCst) {
                    if exec.state.lock().txs.is_empty() {
                        exec.add_tx(id);
                        id += 1;
                        handle.transactions_available();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            })
        };
        WorkPump {
            stop,
            thread: Some(thread),
        }
    }

    /// Executions of `block_hash` so far.
    pub fn executions(&self, block_hash: &Hash32) -> u32 {
        self.state
            .lock()
            .executions
            .get(block_hash)
            .copied()
            .unwrap_or(0)
    }

    fn run(state: &mut ExecState, parent: &Hash32, block: &Block, bh: &Hash32) -> ExecOutcome {
        *state.executions.entry(*bh).or_default() += 1;
        block_exec(parent, block, &state.config.epoch)
    }
}

impl Executor for FakeExecutor {
    fn build_control_witness(
        &mut self,
        _: &iroha_sumeragi::api::ControlWitnessContext,
    ) -> Result<(iroha_sumeragi::types::ControlWitness, bool), PublicationError> {
        Ok((iroha_sumeragi::types::ControlWitness::empty(), false))
    }
    fn drive_control(
        &mut self,
        _: &iroha_sumeragi::api::ApplicationControlContext,
    ) -> Result<Option<iroha_sumeragi::message::ApplicationControl>, PublicationError> {
        Ok(None)
    }
    fn receive_application_control(
        &mut self,
        _: &PublicKey,
        _: &iroha_sumeragi::message::ApplicationControl,
    ) -> Result<(), PublicationError> {
        Ok(())
    }

    fn execute(&mut self, block: &Block, block_hash: &Hash32) -> Option<ExecOutcome> {
        self.wait_open();
        let mut state = self.state.lock();
        let parent = if block.header.parent_hash == state.applied.1 {
            state.applied.2
        } else {
            state.cache.get(&block.header.parent_hash)?.1
        };
        let outcome = Self::run(&mut state, &parent, block, block_hash);
        if let ExecOutcome::Valid(result) = outcome {
            state
                .cache
                .insert(*block_hash, (block.header.height, result));
        }
        Some(outcome)
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        self.state
            .lock()
            .cache
            .retain(|bh, (h, _)| *h != height || keep.contains(bh));
    }

    fn prepare(
        &mut self,
        block: &Block,
        commit_qc: &Qc,
    ) -> Result<Option<Hash32>, PublicationError> {
        let mut state = self.state.lock();
        if state.fail_apply > 0 {
            state.fail_apply -= 1;
            return Err(PublicationError::Retryable("injected".to_owned()));
        }
        let cached = state.cache.get(&commit_qc.block_hash).map(|(_, r)| *r);
        if cached.is_some() {
            return Ok(cached);
        }
        let parent = state.applied.2;
        match Self::run(&mut state, &parent, block, &commit_qc.block_hash) {
            ExecOutcome::Valid(result) => {
                state
                    .cache
                    .insert(commit_qc.block_hash, (block.header.height, result));
                Ok(Some(result))
            }
            _ => Ok(None),
        }
    }

    fn commit(
        &mut self,
        block: &Block,
        commit_qc: &Qc,
    ) -> Result<iroha_sumeragi::types::AppliedConfig, PublicationError> {
        let mut state = self.state.lock();
        if state.fail_apply > 0 {
            state.fail_apply -= 1;
            return Err(PublicationError::Retryable("injected".to_owned()));
        }
        let height = block.header.height;
        state.applied = (height, commit_qc.block_hash, commit_qc.result);
        state.cache.retain(|_, (h, _)| *h > height);
        let committed: Vec<u64> = iroha_sumeragi::sim::driver::decode_txs(&block.payload)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        state.txs.retain(|tx| {
            iroha_sumeragi::sim::driver::decode_txs(tx)
                .first()
                .is_none_or(|(id, _)| !committed.contains(id))
        });
        Ok(iroha_sumeragi::types::AppliedConfig::Continuation {
            after_next: iroha_sumeragi::types::ConfigSlot::Ready(state.config.clone()),
        })
    }

    fn build(
        &mut self,
        _height: u64,
        _view: u64,
        max_bytes: u32,
        _exec_budget_ms: u32,
    ) -> (Vec<u8>, bool) {
        self.wait_open();
        let state = self.state.lock();
        let mut payload = Vec::new();
        for tx in &state.txs {
            if payload.len() + tx.len() > usize::try_from(max_bytes).unwrap_or(usize::MAX) {
                break;
            }
            payload.extend_from_slice(tx);
        }
        let attest = payload_mints(&payload);
        (payload, attest)
    }

    fn reject(&mut self, _height: u64, _view: u64, block_hash: &Hash32) {
        self.state.lock().rejected.push(*block_hash);
    }
}

/// Keeps work queued for a test instance until dropped (see [`FakeExecutor::pump`]).
pub struct WorkPump {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WorkPump {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// An observer that records the driver's own reports.
#[derive(Default)]
pub struct RecordingObserver {
    /// Threads whose end stopped the instance.
    pub stopped: Mutex<Vec<Worker>>,
    /// Configurations that outgrew the transport.
    pub frame_limits: Mutex<Vec<FrameLimitExceeded>>,
}

impl Observer for RecordingObserver {
    fn stopped(&self, worker: Worker) {
        self.stopped.lock().push(worker);
    }

    fn frame_limit(&self, exceeded: &FrameLimitExceeded) {
        self.frame_limits.lock().push(*exceeded);
    }
}
