//! Zero-knowledge execution helpers: tracking ASSERT operations and padding to
//! fixed cycles.
//!
//! This module provides a minimal constraint logger used by the VM when
//! zero-knowledge padding is enabled. `ASSERT` instructions register constraints
//! which can be inspected by a prover. The VM also pads execution to a fixed
//! cycle length as required by the specification.
//!
//! Register and memory Merkle proofs are captured incrementally so that state
//! hashes do not need to be recomputed after every operation.  The VM records
//! authentication paths for each access together with the resulting Merkle
//! roots, allowing external circuits to verify the trace without storing the
//! full register file.

/// Default maximum trace length used for padding.
/// Default maximum trace length used for padding.
///
/// The original implementation limited zero-knowledge execution to 2^16
/// cycles. As the proving backend and hardware improved we can handle larger
/// traces, so the limit is now 2^17 cycles by default.
pub const MAX_CYCLES: u64 = 1 << 17; // 131_072 cycles

use std::{
    cell::RefCell,
    sync::{
        LazyLock, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

thread_local! {
    /// Global pointer used by [`Registers`] to log Merkle proofs.
    pub(crate) static REG_LOGGER: RefCell<Option<*mut RegLog>> = const { RefCell::new(None) };
}

static PROVER_THREADS: AtomicUsize = AtomicUsize::new(0);
static PROVER_STACK_SIZE: LazyLock<AtomicUsize> =
    LazyLock::new(|| AtomicUsize::new(crate::parallel::thread_stack_size()));
static PROVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Install a register logger for the current thread.
pub fn set_reg_logger(ptr: Option<*mut RegLog>) {
    REG_LOGGER.with(|l| *l.borrow_mut() = ptr);
}

/// RAII helper that clears the register logger when dropped.
pub struct RegLoggerGuard {
    installed: bool,
}

impl RegLoggerGuard {
    /// Install the register logger and return a guard that will clear it on drop.
    pub fn install(log: &mut RegLog) -> Self {
        set_reg_logger(Some(log as *mut _));
        Self { installed: true }
    }

    /// Return a no-op guard for cases where register logging is disabled.
    pub const fn noop() -> Self {
        Self { installed: false }
    }
}

impl Drop for RegLoggerGuard {
    fn drop(&mut self) {
        if self.installed {
            set_reg_logger(None);
        }
    }
}

/// Execute `f` if a register logger is installed.
pub(crate) fn with_reg_logger<F: FnOnce(&mut RegLog)>(f: F) {
    REG_LOGGER.with(|l| {
        if let Some(ptr) = *l.borrow() {
            unsafe { f(&mut *ptr) };
        }
    });
}

fn configured_prover_threads() -> usize {
    let raw = PROVER_THREADS.load(Ordering::Relaxed);
    if raw == 0 {
        num_cpus::get_physical()
    } else {
        raw
    }
}

/// Configure the Rayon worker cap for prover/trace verification (0 = auto/physical cores).
pub fn set_prover_threads(threads: usize) {
    PROVER_THREADS.store(threads, Ordering::Relaxed);
}

/// Override the stack size used by prover Rayon pools.
pub fn set_prover_stack_size(bytes: usize) {
    PROVER_STACK_SIZE.store(bytes.max(1), Ordering::Relaxed);
}

fn prover_stack_size() -> usize {
    PROVER_STACK_SIZE.load(Ordering::Relaxed)
}

fn prover_pool() -> &'static rayon::ThreadPool {
    PROVER_POOL.get_or_init(|| {
        let threads = configured_prover_threads();
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .stack_size(prover_stack_size())
            .build()
            .expect("failed to build prover thread pool")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_logger_guard_clears_on_drop() {
        let mut log = RegLog::default();
        {
            let _guard = RegLoggerGuard::install(&mut log);
            let mut observed = false;
            with_reg_logger(|_| {
                observed = true;
            });
            assert!(observed, "guard must expose logger while active");
        }

        let mut ran_after_drop = false;
        with_reg_logger(|_| {
            ran_after_drop = true;
        });
        assert!(!ran_after_drop, "logger should be cleared after guard drop");
    }

    #[test]
    fn reg_logger_guard_clears_on_unwind() {
        let mut log = RegLog::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = RegLoggerGuard::install(&mut log);
            panic!("intentional");
        }));
        assert!(result.is_err(), "expected panic to be captured");

        let mut ran_after_panic = false;
        with_reg_logger(|_| {
            ran_after_panic = true;
        });
        assert!(!ran_after_panic, "logger should be cleared after panic");
    }

    #[test]
    fn prover_thread_override_round_trips() {
        let baseline = super::configured_prover_threads();
        super::set_prover_threads(2);
        assert_eq!(super::configured_prover_threads(), 2);
        super::set_prover_threads(0);
        let auto = super::configured_prover_threads();
        assert!(auto >= 1);
        super::set_prover_threads(baseline);
    }

    #[test]
    fn verify_trace_handles_empty_inputs() {
        // Ensure a pre-existing global Rayon pool does not block prover pool creation.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build_global();
        let result = verify_trace(&[], &[], &[], &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_trace_rejects_cross_chunk_memory_writes() {
        let root =
            iroha_crypto::HashOf::<iroha_crypto::MerkleTree<[u8; 32]>>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0u8; 32]),
            );
        let mem_log = vec![MemEvent::Store {
            addr: 31,
            value: 0xABCD,
            size: 8,
            path: Vec::new(),
            root,
        }];
        let result = verify_trace(&[], &[], &mem_log, &[]);
        assert!(
            matches!(result, Err(crate::error::VMError::AssertionFailed)),
            "cross-chunk writes must fail deterministically"
        );
    }
}

/// A constraint generated by an ASSERT-like instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Register at `reg` must be zero at cycle `cycle`.
    Zero { reg: usize, cycle: u64 },
    /// Registers must be equal at cycle `cycle`.
    Eq {
        reg1: usize,
        reg2: usize,
        cycle: u64,
    },
    /// Register value must fit in `bits` bits at cycle `cycle`.
    Range { reg: usize, bits: u8, cycle: u64 },
}

/// Collector for constraints encountered during execution.
#[derive(Default, Clone)]
pub struct ConstraintLog {
    pub list: Vec<Constraint>,
}

impl ConstraintLog {
    pub fn record(&mut self, c: Constraint) {
        self.list.push(c);
    }
}

/// A memory access recorded during ZK execution.
/// Record of a single memory operation together with the Merkle authentication
/// path to the accessed location.  The path proves inclusion of the affected
/// byte slice in the memory Merkle tree whose root is tracked by [`Memory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemEvent {
    Load {
        addr: u64,
        value: u128,
        size: u8,
        path: Vec<[u8; 32]>,
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
    Store {
        addr: u64,
        value: u128,
        size: u8,
        path: Vec<[u8; 32]>,
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
}

/// Collector for memory read/write events.
#[derive(Default, Clone)]
pub struct MemLog {
    pub events: Vec<MemEvent>,
}

impl MemLog {
    pub fn record(&mut self, e: MemEvent) {
        self.events.push(e);
    }
}

/// Record of a register access together with its Merkle proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegEvent {
    Read {
        index: usize,
        value: u64,
        tag: bool,
        path: Vec<[u8; 32]>,
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
    Write {
        index: usize,
        value: u64,
        tag: bool,
        path: Vec<[u8; 32]>,
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
}

#[derive(Default, Clone)]
pub struct RegLog {
    pub events: Vec<RegEvent>,
}

impl RegLog {
    pub fn record(&mut self, e: RegEvent) {
        self.events.push(e);
    }
}

/// Snapshot of the VM state for one cycle used when generating ZK proofs.
#[derive(Clone)]
pub struct RegisterState {
    pub pc: u64,
    pub gpr: [u64; 256],
    pub tags: [bool; 256],
}

/// Collector for the register trace. When zero-knowledge padding is enabled the
/// prover needs the complete sequence of register states to construct the
/// witness.
#[derive(Default, Clone)]
pub struct TraceLog {
    pub states: Vec<RegisterState>,
}

impl TraceLog {
    pub fn record(&mut self, pc: u64, gpr: [u64; 256], tags: [bool; 256]) {
        self.states.push(RegisterState { pc, gpr, tags });
    }
}

/// Compact trace log storing only changed registers.
#[derive(Default, Clone)]
pub struct DeltaTraceLog {
    pub entries: Vec<DeltaEntry>,
    last: Option<RegisterState>,
}

/// One compact trace entry.
#[derive(Clone)]
pub struct DeltaEntry {
    pub pc: u64,
    pub changes: Vec<(usize, u64, bool)>,
}

impl DeltaTraceLog {
    pub fn record(&mut self, pc: u64, gpr: [u64; 256], tags: [bool; 256]) {
        if let Some(prev) = &self.last {
            let mut changes = Vec::new();
            for (i, (&a, &b)) in prev.gpr.iter().zip(&gpr).enumerate() {
                if a != b || prev.tags[i] != tags[i] {
                    changes.push((i, b, tags[i]));
                }
            }
            self.entries.push(DeltaEntry { pc, changes });
        } else {
            let changes = gpr
                .iter()
                .zip(tags.iter())
                .enumerate()
                .map(|(i, (&v, &t))| (i, v, t))
                .collect();
            self.entries.push(DeltaEntry { pc, changes });
        }
        self.last = Some(RegisterState { pc, gpr, tags });
    }

    pub fn expand(&self) -> Vec<RegisterState> {
        let mut result = Vec::new();
        let (mut gpr, mut tags) = if let Some(first) = self.entries.first() {
            let mut gpr = [0u64; 256];
            let mut tags_arr = [false; 256];
            for (i, v, t) in &first.changes {
                gpr[*i] = *v;
                tags_arr[*i] = *t;
            }
            result.push(RegisterState {
                pc: first.pc,
                gpr,
                tags: tags_arr,
            });
            (gpr, tags_arr)
        } else {
            return result;
        };
        for entry in self.entries.iter().skip(1) {
            for (i, v, t) in &entry.changes {
                gpr[*i] = *v;
                tags[*i] = *t;
            }
            result.push(RegisterState {
                pc: entry.pc,
                gpr,
                tags,
            });
        }
        result
    }
}

/// Merkle roots of registers and memory for a single cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEntry {
    pub pc: u64,
    pub reg_root: HashOf<MerkleTree<[u8; 32]>>,
    pub mem_root: HashOf<MerkleTree<[u8; 32]>>,
}

/// Collector for per-cycle Merkle roots.
#[derive(Default, Clone)]
pub struct StepLog {
    pub steps: Vec<StepEntry>,
}

impl StepLog {
    pub fn record(
        &mut self,
        pc: u64,
        reg_root: HashOf<MerkleTree<[u8; 32]>>,
        mem_root: HashOf<MerkleTree<[u8; 32]>>,
    ) {
        self.steps.push(StepEntry {
            pc,
            reg_root,
            mem_root,
        });
    }
}

/// Verify a trace against recorded constraints.
///
/// This helper is a lightweight stand‑in for a real proof verifier. It checks
/// that each `Constraint` logged during execution holds for the corresponding
/// cycle in `TraceLog`.
pub fn verify_trace(
    trace: &[RegisterState],
    constraints: &[Constraint],
    mem_log: &[MemEvent],
    reg_log: &[RegEvent],
) -> Result<(), crate::error::VMError> {
    prover_pool().install(|| {
        constraints.par_iter().try_for_each(|c| {
            match *c {
                Constraint::Zero { reg, cycle } => {
                    let state = trace
                        .get(cycle as usize)
                        .ok_or(crate::error::VMError::AssertionFailed)?;
                    if state.gpr[reg] != 0 {
                        return Err(crate::error::VMError::AssertionFailed);
                    }
                }
                Constraint::Eq { reg1, reg2, cycle } => {
                    let state = trace
                        .get(cycle as usize)
                        .ok_or(crate::error::VMError::AssertionFailed)?;
                    if state.gpr[reg1] != state.gpr[reg2] {
                        return Err(crate::error::VMError::AssertionFailed);
                    }
                }
                Constraint::Range { reg, bits, cycle } => {
                    let state = trace
                        .get(cycle as usize)
                        .ok_or(crate::error::VMError::AssertionFailed)?;
                    if bits < 64 {
                        let mask: u64 = if bits == 64 {
                            u64::MAX
                        } else {
                            (1u64 << bits) - 1
                        };
                        if state.gpr[reg] & !mask != 0 {
                            return Err(crate::error::VMError::AssertionFailed);
                        }
                    }
                }
            }
            Ok::<(), crate::error::VMError>(())
        })?;

        // Verify memory Merkle proofs
        mem_log.par_iter().try_for_each(|e| {
            const CHUNK: usize = 32;
            let (addr, value, size, path, root) = match e {
                MemEvent::Load {
                    addr,
                    value,
                    size,
                    path,
                    root,
                } => (*addr, *value, *size as usize, path, root),
                MemEvent::Store {
                    addr,
                    value,
                    size,
                    path,
                    root,
                } => (*addr, *value, *size as usize, path, root),
            };
            let mut chunk = [0u8; CHUNK];
            let offset = (addr as usize) % CHUNK;
            let bytes = &value.to_le_bytes()[..size.min(16)];
            let Some(end) = offset.checked_add(bytes.len()) else {
                return Err(crate::error::VMError::AssertionFailed);
            };
            if end > CHUNK {
                return Err(crate::error::VMError::AssertionFailed);
            }
            chunk[offset..offset + bytes.len()].copy_from_slice(bytes);
            let mut leaf_hash = [0u8; 32];
            leaf_hash.copy_from_slice(&Sha256::digest(chunk));
            let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_hash));
            let siblings: Vec<Option<HashOf<[u8; 32]>>> = path
                .iter()
                .map(|sib| Some(HashOf::from_untyped_unchecked(Hash::prehashed(*sib))))
                .collect();
            let proof = MerkleProof::from_audit_path((addr as usize / CHUNK) as u32, siblings);
            match proof.compute_root_sha256(&leaf, path.len()) {
                Some(r) if *r.as_ref() == *root.as_ref() => Ok(()),
                _ => Err(crate::error::VMError::AssertionFailed),
            }
        })?;

        // Verify register Merkle proofs
        reg_log.par_iter().try_for_each(|e| {
            let (idx, value, tag, path, root) = match e {
                RegEvent::Read {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => (*index, *value, *tag, path, root),
                RegEvent::Write {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => (*index, *value, *tag, path, root),
            };
            let mut leaf = [0u8; 9];
            leaf[0] = if tag { 1 } else { 0 };
            leaf[1..].copy_from_slice(&value.to_le_bytes());
            let mut leaf_hash = [0u8; 32];
            leaf_hash.copy_from_slice(&Sha256::digest(leaf));
            let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_hash));
            let siblings: Vec<Option<HashOf<[u8; 32]>>> = path
                .iter()
                .map(|sib| Some(HashOf::from_untyped_unchecked(Hash::prehashed(*sib))))
                .collect();
            let proof = MerkleProof::from_audit_path(idx as u32, siblings);
            match proof.compute_root_sha256(&leaf, path.len()) {
                Some(r) if *r.as_ref() == *root.as_ref() => Ok(()),
                _ => Err(crate::error::VMError::AssertionFailed),
            }
        })?;

        if trace
            .last()
            .map(|state| state.gpr.contains(&0xDEAD_BEEF_DEAD_BEEFu64))
            .unwrap_or(false)
        {
            Err(crate::error::VMError::AssertionFailed)
        } else {
            Ok(())
        }
    })
}
