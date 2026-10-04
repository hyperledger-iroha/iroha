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
use iroha_crypto::{HashOf, MerkleTree};
use rayon::prelude::*;
use std::sync::{
    LazyLock, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
mod cycle_roots;
mod delta_rows;
mod runtime_trace;
mod trace_storage;
pub use delta_rows::{DeltaEntry, DeltaTraceLog};
pub use runtime_trace::RuntimeTraceCapture;
pub(crate) use trace_storage::PcTraceLog;
mod diagnostic_snapshot;
mod register_authentication;
pub(crate) use crate::cache_memory::SharedRegLog;
pub(crate) use cycle_roots::StepLog;
pub use diagnostic_snapshot::{
    DiagnosticMemoryEvent, DiagnosticRegisterEvent, DiagnosticRegisterSource,
    DiagnosticTraceSnapshot, DiagnosticTraceSource,
};
mod register_batches;
mod register_events;
pub(crate) use register_batches::{
    RegEventBatch, RegLoggerGuard, event_reg_logger, record_register_event, scoped_reg_logger,
    scoped_reg_logger_enabled,
};
pub use register_events::RegLog;
static PROVER_THREADS: AtomicUsize = AtomicUsize::new(0);
static PROVER_STACK_SIZE: LazyLock<AtomicUsize> =
    LazyLock::new(|| AtomicUsize::new(crate::parallel::thread_stack_size()));
static PROVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
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
    fn register_event_fixture() -> RegEvent {
        RegEvent::Read {
            index: 0,
            value: 0,
            tag: false,
            path: [[0; 32]; crate::REGISTER_MERKLE_PATH_DEPTH],
            root: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([0])),
        }
    }
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

        let mut registers = RegLog::new(None);
        registers.prepare_events(1, None).unwrap();
        registers.record_reserved(RegEvent::Write {
            index: 7,
            value: 21,
            tag: true,
            path: [[3; 32]; crate::REGISTER_MERKLE_PATH_DEPTH],
            root,
        });
        let copied_registers = registers
            .try_clone_allocation(None)
            .expect("bounded register paths");
        assert_eq!(copied_registers.as_slice(), registers.as_slice());
        assert_eq!(
            copied_registers
                .allocated_bytes()
                .expect("checked capacity"),
            copied_registers.capacity() * std::mem::size_of::<RegEvent>()
        );
        let mut changed = registers.as_slice()[0].clone();
        if let RegEvent::Write { path, .. } = &mut changed {
            path[0] = [9; 32];
        }
        registers.scrub();
        registers.record_reserved(changed);
        assert_ne!(copied_registers.as_slice(), registers.as_slice());

        let mut trace = DeltaTraceLog::new(None);
        let mut gpr = [0; 256];
        gpr[7] = 21;
        trace.prepare_batch(1, 256, 0, None).unwrap();
        trace.record_reserved(4, gpr, [false; 256]);
        gpr[7] = 22;
        trace.prepare_batch(1, 1, 0, None).unwrap();
        trace.record_reserved(8, gpr, [false; 256]);
        let copied_trace = trace
            .try_clone_allocation(None)
            .expect("bounded delta trace");
        assert!(copied_trace.entries().eq(trace.entries()));
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
        assert_eq!(capture(&copied_trace).states(), capture(&trace).states());
        assert!(copied_trace.allocated_bytes().expect("checked capacity") > 0);

        let mut steps = StepLog::new(None);
        steps.prepare_cycles(1).expect("bounded steps");
        steps.record_reserved(4, root, root);
        let copied_steps = steps.try_clone_allocation().expect("bounded steps");
        assert_eq!(copied_steps.as_slice(), steps.as_slice());
        assert!(copied_steps.allocated_bytes().expect("checked capacity") > 0);
    }
    #[test]
    fn reg_logger_guard_clears_on_drop() {
        let log = SharedRegLog::try_new(None).expect("test logger allocation");
        {
            let _guard = RegLoggerGuard::install(Some(log.clone()));
            let _batch = RegEventBatch::begin(1).unwrap();
            let mut observed = false;
            record_register_event(|| {
                observed = true;
                register_event_fixture()
            });
            assert!(observed, "guard must expose logger while active");
        }
        let mut ran_after_drop = false;
        record_register_event(|| {
            ran_after_drop = true;
            register_event_fixture()
        });
        assert!(!ran_after_drop, "logger should be cleared after guard drop");
    }
    #[test]
    fn reg_logger_guard_clears_on_unwind() {
        let log = SharedRegLog::try_new(None).expect("test logger allocation");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = RegLoggerGuard::install(Some(log.clone()));
            panic!("intentional");
        }));
        assert!(result.is_err(), "expected panic to be captured");
        let mut ran_after_panic = false;
        record_register_event(|| {
            ran_after_panic = true;
            register_event_fixture()
        });
        assert!(!ran_after_panic, "logger should be cleared after panic");
    }
    #[test]
    fn nested_reg_logger_install_restores_outer_logger() {
        let outer_log = SharedRegLog::try_new(None).expect("test logger allocation");
        let outer_address = {
            let log = outer_log.lock();
            std::ptr::from_ref(&*log) as usize
        };
        let guard = RegLoggerGuard::install(Some(outer_log.clone()));
        let inner_log = SharedRegLog::try_new(None).expect("test logger allocation");
        let inner_address = {
            let log = inner_log.lock();
            std::ptr::from_ref(&*log) as usize
        };
        {
            let _nested = RegLoggerGuard::install(Some(inner_log.clone()));
            let _batch = RegEventBatch::begin(1).unwrap();
            record_register_event(|| {
                let installed = event_reg_logger().unwrap();
                let installed = installed.lock();
                assert_eq!(
                    std::ptr::from_ref(&*installed) as usize,
                    inner_address,
                    "nested logger must be active in its scope"
                );
                register_event_fixture()
            });
        }
        let batch = RegEventBatch::begin(1).unwrap();
        record_register_event(|| {
            let installed = event_reg_logger().unwrap();
            let installed = installed.lock();
            assert_eq!(
                std::ptr::from_ref(&*installed) as usize,
                outer_address,
                "nested scope must restore the outer logger"
            );
            register_event_fixture()
        });
        drop(batch);
        drop(guard);
    }
    #[test]
    fn masked_nested_reg_logger_scope_restores_outer_logger() {
        let outer_log = SharedRegLog::try_new(None).expect("test logger allocation");
        let _outer = RegLoggerGuard::install(Some(outer_log.clone()));
        assert_eq!(scoped_reg_logger_enabled(), Some(true));
        {
            let _masked = RegLoggerGuard::install(None);
            assert_eq!(scoped_reg_logger_enabled(), Some(false));
            let mut observed = false;
            record_register_event(|| {
                observed = true;
                register_event_fixture()
            });
            assert!(
                !observed,
                "masked nested scope must suppress the outer logger"
            );
        }
        assert_eq!(scoped_reg_logger_enabled(), Some(true));
        assert!(SharedRegLog::ptr_eq(
            &scoped_reg_logger().expect("outer logger restored"),
            &outer_log
        ));
    }
    #[test]
    fn callback_mask_suppresses_events_but_retains_invocation_identity() {
        let outer_log = SharedRegLog::try_new(None).expect("test logger allocation");
        let _outer = RegLoggerGuard::install(Some(outer_log.clone()));
        {
            let _masked = RegLoggerGuard::mask();
            assert_eq!(scoped_reg_logger_enabled(), Some(true));
            assert!(SharedRegLog::ptr_eq(
                &scoped_reg_logger().expect("invocation logger retained"),
                &outer_log
            ));
            let mut observed = false;
            record_register_event(|| {
                observed = true;
                register_event_fixture()
            });
            assert!(!observed, "callback mask must suppress register events");
        }
        let _batch = RegEventBatch::begin(1).unwrap();
        let mut observed = false;
        record_register_event(|| {
            observed = true;
            register_event_fixture()
        });
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
/// Record of a register access with the canonical eight-sibling path stored inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegEvent {
    Read {
        index: usize,
        value: u64,
        tag: bool,
        path: [[u8; 32]; crate::REGISTER_MERKLE_PATH_DEPTH],
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
    Write {
        index: usize,
        value: u64,
        tag: bool,
        path: [[u8; 32]; crate::REGISTER_MERKLE_PATH_DEPTH],
        root: HashOf<MerkleTree<[u8; 32]>>,
    },
}
/// Snapshot of the VM state for one cycle used when generating ZK proofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterState {
    pub pc: u64,
    pub gpr: [u64; 256],
    pub tags: [bool; 256],
}
/// Merkle roots of registers and memory for a single cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepEntry {
    pub pc: u64,
    pub reg_root: HashOf<MerkleTree<[u8; 32]>>,
    pub mem_root: HashOf<MerkleTree<[u8; 32]>>,
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
                register_authentication::check(event)
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
