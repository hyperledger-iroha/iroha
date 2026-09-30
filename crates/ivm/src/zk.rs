//! Zero-knowledge execution helpers: tracking ASSERT operations and padding to fixed cycles.
//!
//! This module provides a minimal constraint logger used by the VM when zero-knowledge padding is
//! enabled. `ASSERT` instructions register constraints which can be inspected by a prover. The VM
//! also pads execution to a fixed cycle length as required by the specification.
//!
//! Register authentication paths and memory diagnostic root snapshots are
//! captured incrementally so that state hashes do not need to be recomputed
//! after every operation. Memory events do not carry complete leaves or an
//! authenticated tree size and are not independently verifiable proofs.
/// Default maximum trace length used for padding.
///
/// The original implementation limited zero-knowledge execution to 2^16
/// cycles. As the proving backend and hardware improved we can handle larger
/// traces, so the limit is now 2^17 cycles by default.
pub const MAX_CYCLES: u64 = 1 << 17; // 131_072 cycles
use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    marker::PhantomData,
    num::NonZeroU64,
    rc::Rc,
    sync::{
        Arc, LazyLock, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
mod diagnostic_snapshot;
pub use diagnostic_snapshot::{
    DiagnosticMemoryEvent, DiagnosticRegisterEvent, DiagnosticRegisterSource,
    DiagnosticTraceSnapshot, DiagnosticTraceSource,
};
pub(crate) type SharedRegLog = Arc<parking_lot::Mutex<RegLog>>;
#[derive(Clone)]
struct RegLoggerState {
    log: Option<SharedRegLog>,
    logging_enabled: bool,
}
thread_local! {
    /// Thread-local, ownership-safe sink used by [`Registers`] to log Merkle proofs.
    ///
    /// The outer `Option` distinguishes execution outside a VM run from an
    /// explicitly masked run. That distinction prevents an untraced nested VM
    /// from inheriting its caller's logger.
    static REG_LOGGER: RefCell<Option<RegLoggerState>> = const { RefCell::new(None) };
}
static PROVER_THREADS: AtomicUsize = AtomicUsize::new(0);
static PROVER_STACK_SIZE: LazyLock<AtomicUsize> =
    LazyLock::new(|| AtomicUsize::new(crate::parallel::thread_stack_size()));
static PROVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
/// RAII helper that clears the register logger when dropped.
pub(crate) struct RegLoggerGuard {
    previous: Option<RegLoggerState>,
    // A thread-local installation must be removed on the thread that created it.
    _not_send_or_sync: PhantomData<Rc<()>>,
}
impl RegLoggerGuard {
    /// Install an active or explicitly masked register logger scope and restore
    /// any outer scope on drop.
    pub(crate) fn install(log: Option<SharedRegLog>) -> Self {
        let logging_enabled = log.is_some();
        let previous = REG_LOGGER.with(|slot| {
            slot.replace(Some(RegLoggerState {
                log,
                logging_enabled,
            }))
        });
        Self {
            previous,
            _not_send_or_sync: PhantomData,
        }
    }
    /// Temporarily suppress events while retaining the surrounding
    /// invocation's logger identity and fixed trace policy.
    pub(crate) fn mask() -> Self {
        let previous = REG_LOGGER.with(|slot| {
            let masked = slot.borrow().as_ref().map(|state| RegLoggerState {
                log: state.log.clone(),
                logging_enabled: false,
            });
            slot.replace(masked)
        });
        Self {
            previous,
            _not_send_or_sync: PhantomData,
        }
    }
}
impl Drop for RegLoggerGuard {
    fn drop(&mut self) {
        REG_LOGGER.with(|slot| *slot.borrow_mut() = self.previous.take());
    }
}
/// Return whether the current VM invocation fixed trace collection as enabled.
///
/// `None` means execution is outside a VM run and the VM's configured mode
/// should be consulted instead.
pub(crate) fn scoped_reg_logger_enabled() -> Option<bool> {
    REG_LOGGER.with(|slot| slot.borrow().as_ref().map(|state| state.log.is_some()))
}
/// Clone the invocation-owned logger, if trace collection is active.
pub(crate) fn scoped_reg_logger() -> Option<SharedRegLog> {
    REG_LOGGER.with(|slot| slot.borrow().as_ref().and_then(|state| state.log.clone()))
}
/// Clone the invocation logger only when the current scope may emit events.
pub(crate) fn event_reg_logger() -> Option<SharedRegLog> {
    REG_LOGGER.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|state| state.logging_enabled.then(|| state.log.clone()).flatten())
    })
}
/// Execute `f` if a register logger is installed.
pub(crate) fn with_reg_logger<F: FnOnce(&mut RegLog)>(f: F) {
    REG_LOGGER.with(|l| {
        let installed = l
            .borrow()
            .as_ref()
            .and_then(|state| state.logging_enabled.then(|| state.log.clone()).flatten());
        if let Some(log) = installed {
            f(&mut log.lock());
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
/// Return the effective prover/trace verification worker cap.
#[must_use]
pub fn prover_threads() -> usize {
    configured_prover_threads()
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
    fn fallible_trace_copies_preserve_nested_paths_and_snapshots() {
        let root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(vec![[7; 32]])
            .root()
            .expect("one-leaf test tree has a root");
        let mut constraints = ConstraintLog::default();
        constraints.record(Constraint::Zero { reg: 7, cycle: 1 });
        let copied_constraints = constraints
            .try_clone_allocation()
            .expect("bounded constraints");
        assert_eq!(copied_constraints.list, constraints.list);
        assert!(
            copied_constraints
                .allocated_bytes()
                .expect("checked capacity")
                > 0
        );

        let mut memory = MemLog::default();
        memory.record(MemEvent::Load {
            addr: 8,
            value: 13,
            size: 8,
            path: vec![[1; 32], [2; 32]],
            root,
        });
        let copied_memory = memory.try_clone_allocation().expect("bounded memory paths");
        assert_eq!(copied_memory.events, memory.events);
        assert!(copied_memory.allocated_bytes().expect("checked capacity") > 0);
        if let MemEvent::Load { path, .. } = &mut memory.events[0] {
            path[0] = [9; 32];
        }
        assert_ne!(copied_memory.events, memory.events);

        let mut registers = RegLog::default();
        registers.record(RegEvent::Write {
            index: 7,
            value: 21,
            tag: true,
            path: vec![[3; 32]],
            root,
        });
        let copied_registers = registers
            .try_clone_allocation()
            .expect("bounded register paths");
        assert_eq!(copied_registers.events, registers.events);
        assert!(
            copied_registers
                .allocated_bytes()
                .expect("checked capacity")
                > 0
        );

        let mut trace = DeltaTraceLog::default();
        let mut gpr = [0; 256];
        gpr[7] = 21;
        trace.record(4, gpr, [false; 256]);
        gpr[7] = 22;
        trace.record(8, gpr, [false; 256]);
        let copied_trace = trace.try_clone_allocation().expect("bounded delta trace");
        assert_eq!(copied_trace.entries, trace.entries);
        let budget = iroha_allocation::AllocationBudget::new(64 * 1024);
        let capture = |rows| {
            DiagnosticTraceSource {
                registers: DiagnosticRegisterSource::Deltas(rows),
                constraints: &[],
                memory_events: &[],
                register_events: &[],
                steps: &[],
            }
            .try_snapshot(&budget)
            .expect("fund copied trace fixture")
        };
        assert_eq!(
            capture(&copied_trace.entries).states(),
            capture(&trace.entries).states()
        );
        assert!(copied_trace.allocated_bytes().expect("checked capacity") > 0);

        let mut steps = StepLog::default();
        steps.record(4, root, root);
        let copied_steps = steps.try_clone_allocation().expect("bounded steps");
        assert_eq!(copied_steps.steps, steps.steps);
        assert!(copied_steps.allocated_bytes().expect("checked capacity") > 0);
    }
    #[test]
    fn reg_logger_guard_clears_on_drop() {
        let log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        {
            let _guard = RegLoggerGuard::install(Some(Arc::clone(&log)));
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
        let log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = RegLoggerGuard::install(Some(Arc::clone(&log)));
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
    fn nested_reg_logger_install_restores_outer_logger() {
        let outer_log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        let outer_address = {
            let log = outer_log.lock();
            std::ptr::from_ref(&*log) as usize
        };
        let guard = RegLoggerGuard::install(Some(Arc::clone(&outer_log)));
        let inner_log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        let inner_address = {
            let log = inner_log.lock();
            std::ptr::from_ref(&*log) as usize
        };
        {
            let _nested = RegLoggerGuard::install(Some(Arc::clone(&inner_log)));
            with_reg_logger(|installed| {
                assert_eq!(
                    std::ptr::from_mut(installed) as usize,
                    inner_address,
                    "nested logger must be active in its scope"
                );
            });
        }
        with_reg_logger(|installed| {
            assert_eq!(
                std::ptr::from_mut(installed) as usize,
                outer_address,
                "nested scope must restore the outer logger"
            );
        });
        drop(guard);
    }
    #[test]
    fn masked_nested_reg_logger_scope_restores_outer_logger() {
        let outer_log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        let _outer = RegLoggerGuard::install(Some(Arc::clone(&outer_log)));
        assert_eq!(scoped_reg_logger_enabled(), Some(true));
        {
            let _masked = RegLoggerGuard::install(None);
            assert_eq!(scoped_reg_logger_enabled(), Some(false));
            let mut observed = false;
            with_reg_logger(|_| observed = true);
            assert!(
                !observed,
                "masked nested scope must suppress the outer logger"
            );
        }
        assert_eq!(scoped_reg_logger_enabled(), Some(true));
        assert!(Arc::ptr_eq(
            &scoped_reg_logger().expect("outer logger restored"),
            &outer_log
        ));
    }
    #[test]
    fn callback_mask_suppresses_events_but_retains_invocation_identity() {
        let outer_log = Arc::new(parking_lot::Mutex::new(RegLog::default()));
        let _outer = RegLoggerGuard::install(Some(Arc::clone(&outer_log)));
        {
            let _masked = RegLoggerGuard::mask();
            assert_eq!(scoped_reg_logger_enabled(), Some(true));
            assert!(Arc::ptr_eq(
                &scoped_reg_logger().expect("invocation logger retained"),
                &outer_log
            ));
            let mut observed = false;
            with_reg_logger(|_| observed = true);
            assert!(!observed, "callback mask must suppress register events");
        }
        let mut observed = false;
        with_reg_logger(|_| observed = true);
        assert!(observed, "dropping callback mask restores event logging");
    }
    #[test]
    fn prover_thread_override_round_trips() {
        let baseline = super::configured_prover_threads();
        super::set_prover_threads(2);
        assert_eq!(super::configured_prover_threads(), 2);
        assert_eq!(super::prover_threads(), 2);
        super::set_prover_threads(0);
        let auto = super::configured_prover_threads();
        assert!(auto >= 1);
        super::set_prover_threads(baseline);
    }
    fn snapshot_fixture(
        states: &[RegisterState],
        constraints: &[Constraint],
    ) -> DiagnosticTraceSnapshot {
        DiagnosticTraceSource {
            registers: DiagnosticRegisterSource::States(states),
            constraints,
            memory_events: &[],
            register_events: &[],
            steps: &[],
        }
        .try_snapshot(&iroha_allocation::AllocationBudget::new(64 * 1024))
        .expect("fund checker fixture")
    }
    #[test]
    fn diagnostic_trace_check_handles_empty_inputs() {
        // Ensure a pre-existing global Rayon pool does not block prover pool creation.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build_global();
        let result = check_diagnostic_trace(&snapshot_fixture(&[], &[]));
        assert!(result.is_ok());
    }
    #[test]
    fn diagnostic_trace_check_accepts_arbitrary_register_data() {
        let mut gpr = [0u64; 256];
        gpr[0] = 0xDEAD_BEEF_DEAD_BEEF;
        let trace = [RegisterState {
            pc: 0,
            gpr,
            tags: [false; 256],
        }];

        assert!(check_diagnostic_trace(&snapshot_fixture(&trace, &[])).is_ok());
    }
    #[test]
    fn diagnostic_trace_check_rejects_out_of_range_constraints_without_panicking() {
        let trace = vec![RegisterState {
            pc: 0,
            gpr: [0; 256],
            tags: [false; 256],
        }];
        for constraint in [
            Constraint::Zero { reg: 256, cycle: 0 },
            Constraint::Eq {
                reg1: 0,
                reg2: 256,
                cycle: 0,
            },
            Constraint::Range {
                reg: 256,
                bits: 64,
                cycle: 0,
            },
            Constraint::Zero {
                reg: 0,
                cycle: u64::MAX,
            },
        ] {
            let result = std::panic::catch_unwind(|| {
                check_diagnostic_trace(&snapshot_fixture(&trace, &[constraint]))
            });
            assert!(
                matches!(
                    result.expect("malformed diagnostic check must not panic"),
                    Err(crate::error::VMError::AssertionFailed)
                ),
                "malformed constraint must fail closed: {constraint:?}"
            );
        }
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
fn trace_allocation_error() -> crate::error::VMError {
    crate::error::VMError::ExecutionDeferred(crate::error::ExecutionDeferral::AllocationUnavailable)
}
fn trace_vector_bytes<T>(capacity: usize) -> Result<usize, crate::error::VMError> {
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(trace_allocation_error)?;
    (bytes <= isize::MAX as usize)
        .then_some(bytes)
        .ok_or_else(trace_allocation_error)
}
fn trace_add_bytes(left: usize, right: usize) -> Result<usize, crate::error::VMError> {
    left.checked_add(right).ok_or_else(trace_allocation_error)
}
#[cfg(test)]
fn try_copy_trace_slice<T: Clone>(source: &[T]) -> Result<Vec<T>, crate::error::VMError> {
    let _ = trace_vector_bytes::<T>(source.len())?;
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(source.len())
        .map_err(|_| trace_allocation_error())?;
    copied.extend_from_slice(source);
    Ok(copied)
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
    pub(crate) fn allocated_bytes(&self) -> Result<usize, crate::error::VMError> {
        trace_vector_bytes::<Constraint>(self.list.capacity())
    }
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(&self) -> Result<Self, crate::error::VMError> {
        Ok(Self {
            list: try_copy_trace_slice(&self.list)?,
        })
    }
}
/// A memory access recorded during diagnostic trace collection.
///
/// The event carries a path and root snapshot for downstream diagnostics, but
/// it does not contain the complete 32-byte leaf or an authenticated leaf
/// count. It is therefore not an independently verifiable Merkle proof.
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
    pub(crate) fn allocated_bytes(&self) -> Result<usize, crate::error::VMError> {
        self.events.iter().try_fold(
            trace_vector_bytes::<MemEvent>(self.events.capacity())?,
            |bytes, event| {
                let path = match event {
                    MemEvent::Load { path, .. } | MemEvent::Store { path, .. } => path,
                };
                trace_add_bytes(bytes, trace_vector_bytes::<[u8; 32]>(path.capacity())?)
            },
        )
    }
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(&self) -> Result<Self, crate::error::VMError> {
        let _ = trace_vector_bytes::<MemEvent>(self.events.len())?;
        let mut events = Vec::new();
        events
            .try_reserve_exact(self.events.len())
            .map_err(|_| trace_allocation_error())?;
        for event in &self.events {
            events.push(match event {
                MemEvent::Load {
                    addr,
                    value,
                    size,
                    path,
                    root,
                } => MemEvent::Load {
                    addr: *addr,
                    value: *value,
                    size: *size,
                    path: try_copy_trace_slice(path)?,
                    root: *root,
                },
                MemEvent::Store {
                    addr,
                    value,
                    size,
                    path,
                    root,
                } => MemEvent::Store {
                    addr: *addr,
                    value: *value,
                    size: *size,
                    path: try_copy_trace_slice(path)?,
                    root: *root,
                },
            });
        }
        Ok(Self { events })
    }
    /// Zero retained memory values before discarding the event log.
    pub(crate) fn scrub(&mut self) {
        for event in &mut self.events {
            match event {
                MemEvent::Load { value, .. } | MemEvent::Store { value, .. } => *value = 0,
            }
        }
        self.events.clear();
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
    #[cfg(test)]
    pub(crate) fn allocated_bytes(&self) -> Result<usize, crate::error::VMError> {
        self.events.iter().try_fold(
            trace_vector_bytes::<RegEvent>(self.events.capacity())?,
            |bytes, event| {
                let path = match event {
                    RegEvent::Read { path, .. } | RegEvent::Write { path, .. } => path,
                };
                trace_add_bytes(bytes, trace_vector_bytes::<[u8; 32]>(path.capacity())?)
            },
        )
    }
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(&self) -> Result<Self, crate::error::VMError> {
        let _ = trace_vector_bytes::<RegEvent>(self.events.len())?;
        let mut events = Vec::new();
        events
            .try_reserve_exact(self.events.len())
            .map_err(|_| trace_allocation_error())?;
        for event in &self.events {
            events.push(match event {
                RegEvent::Read {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => RegEvent::Read {
                    index: *index,
                    value: *value,
                    tag: *tag,
                    path: try_copy_trace_slice(path)?,
                    root: *root,
                },
                RegEvent::Write {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => RegEvent::Write {
                    index: *index,
                    value: *value,
                    tag: *tag,
                    path: try_copy_trace_slice(path)?,
                    root: *root,
                },
            });
        }
        Ok(Self { events })
    }
    /// Zero retained register values before discarding the event log.
    pub(crate) fn scrub(&mut self) {
        for event in &mut self.events {
            match event {
                RegEvent::Read { value, .. } | RegEvent::Write { value, .. } => *value = 0,
            }
        }
        self.events.clear();
    }
}
/// Snapshot of the VM state for one cycle used when generating ZK proofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterState {
    pub pc: u64,
    pub gpr: [u64; 256],
    pub tags: [bool; 256],
}
/// Collector for the register trace. When zero-knowledge padding is enabled the prover needs the
/// complete sequence of register states to construct the witness.
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeltaEntry {
    pub pc: u64,
    pub changes: Vec<(usize, u64, bool)>,
}
impl DeltaTraceLog {
    pub(crate) fn allocated_bytes(&self) -> Result<usize, crate::error::VMError> {
        self.entries.iter().try_fold(
            trace_vector_bytes::<DeltaEntry>(self.entries.capacity())?,
            |bytes, entry| {
                trace_add_bytes(
                    bytes,
                    trace_vector_bytes::<(usize, u64, bool)>(entry.changes.capacity())?,
                )
            },
        )
    }
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(&self) -> Result<Self, crate::error::VMError> {
        let _ = trace_vector_bytes::<DeltaEntry>(self.entries.len())?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| trace_allocation_error())?;
        for entry in &self.entries {
            entries.push(DeltaEntry {
                pc: entry.pc,
                changes: try_copy_trace_slice(&entry.changes)?,
            });
        }
        Ok(Self {
            entries,
            last: self.last.clone(),
        })
    }
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
    /// Zero retained register values before discarding the compact trace.
    pub(crate) fn scrub(&mut self) {
        for entry in &mut self.entries {
            for (_, value, tag) in &mut entry.changes {
                *value = 0;
                *tag = false;
            }
        }
        if let Some(last) = &mut self.last {
            last.gpr.fill(0);
            last.tags.fill(false);
        }
        self.entries.clear();
        self.last = None;
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
    pub(crate) fn allocated_bytes(&self) -> Result<usize, crate::error::VMError> {
        trace_vector_bytes::<StepEntry>(self.steps.capacity())
    }
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(&self) -> Result<Self, crate::error::VMError> {
        Ok(Self {
            steps: try_copy_trace_slice(&self.steps)?,
        })
    }
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
/// Check locally recorded trace diagnostics.
///
/// This checks each recorded [`Constraint`] against the corresponding register
/// snapshot and authenticates register-log entries against their supplied
/// fixed-size register-tree commitments. It does not verify VM transition
/// semantics, trace completeness, or memory-event membership and must not be
/// used as a proof verifier or admission decision.
pub fn check_diagnostic_trace(
    snapshot: &DiagnosticTraceSnapshot,
) -> Result<(), crate::error::VMError> {
    let trace = snapshot.states();
    let constraints = snapshot.constraints();
    prover_pool().install(|| {
        constraints.par_iter().try_for_each(|c| {
            match *c {
                Constraint::Zero { reg, cycle } => {
                    if trace_register(trace, cycle, reg)? != 0 {
                        return Err(crate::error::VMError::AssertionFailed);
                    }
                }
                Constraint::Eq { reg1, reg2, cycle } => {
                    if trace_register(trace, cycle, reg1)? != trace_register(trace, cycle, reg2)? {
                        return Err(crate::error::VMError::AssertionFailed);
                    }
                }
                Constraint::Range { reg, bits, cycle } => {
                    if bits < 64 {
                        let mask = (1u64 << bits) - 1;
                        if trace_register(trace, cycle, reg)? & !mask != 0 {
                            return Err(crate::error::VMError::AssertionFailed);
                        }
                    } else {
                        // Even an unconstrained 64-bit range must reference a real trace cell.
                        let _ = trace_register(trace, cycle, reg)?;
                    }
                }
            }
            Ok::<(), crate::error::VMError>(())
        })?;
        // Verify register Merkle proofs
        (0..snapshot.register_event_count())
            .into_par_iter()
            .try_for_each(|index| {
                let event = snapshot
                    .register_event(index)
                    .expect("initialized register descriptor");
                let (idx, value, tag, path, root) =
                    (event.index, event.value, event.tag, event.path, event.root);
                let leaf_index = u32::try_from(idx)
                    .ok()
                    .filter(|index| *index < 256)
                    .ok_or(crate::error::VMError::AssertionFailed)?;
                let mut leaf = [0u8; 9];
                leaf[0] = if tag { 1 } else { 0 };
                leaf[1..].copy_from_slice(&value.to_le_bytes());
                let mut leaf_hash = [0u8; 32];
                leaf_hash.copy_from_slice(&Sha256::digest(leaf));
                iroha_crypto::zeroize_value_for_confidential_discard(&mut leaf);
                let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_hash));
                // A complete 256-register tree has exactly eight siblings. Reject
                // both missing and extra paths before filling fixed stack storage.
                let path: &[[u8; 32]; 8] = path
                    .try_into()
                    .map_err(|_| crate::error::VMError::AssertionFailed)?;
                let siblings = path.map(|sibling| {
                    (sibling != [0; 32])
                        .then(|| HashOf::from_untyped_unchecked(Hash::prehashed(sibling)))
                });
                let commitment = MerkleTreeCommitment::new(
                    HashOf::from_untyped_unchecked(Hash::prehashed(*root)),
                    NonZeroU64::new(256).expect("register tree leaf count is non-zero"),
                );
                if MerkleProof::verify_audit_path_sha256(leaf_index, &siblings, &leaf, &commitment)
                {
                    Ok(())
                } else {
                    Err(crate::error::VMError::AssertionFailed)
                }
            })?;
        Ok(())
    })
}

fn trace_register(
    trace: &[RegisterState],
    cycle: u64,
    register: usize,
) -> Result<u64, crate::error::VMError> {
    let cycle = usize::try_from(cycle).map_err(|_| crate::error::VMError::AssertionFailed)?;
    trace
        .get(cycle)
        .and_then(|state| state.gpr.get(register))
        .copied()
        .ok_or(crate::error::VMError::AssertionFailed)
}
