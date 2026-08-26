//! Parallel execution components implementing the deterministic block execution engine described in
//! the "Iroha VM Parallel Block Execution – Rust Implementation Specification".
//!
//! Transactions execute against isolated contexts and publish ordered write sets
//! through a software-owned state lock. The execution and commit order is
//! independent of host CPU features.
use crate::vector::{SimdChoice, set_thread_forced_simd};
use parking_lot::{Mutex, RwLock};
use rayon::ThreadPool;
use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};
struct ThreadSimdOverrideGuard(Option<SimdChoice>);
impl Drop for ThreadSimdOverrideGuard {
    fn drop(&mut self) {
        set_thread_forced_simd(self.0);
    }
}
/// Number of general purpose registers used by the execution contexts.
///
/// Matches the main VM register file size.
pub const REGISTER_COUNT: usize = 256;
/// Identifier for a state entry in the world state. For the purposes of this crate it is simply a
/// string key but in a real integration this could be a complex type.
pub type StateKey = String;
/// Generic state value type.
pub type Value = u64;
/// Shared key-value state accessed by transactions.
#[derive(Clone, Default)]
pub struct State(Arc<RwLock<HashMap<StateKey, Value>>>);
impl State {
    /// Create an empty state.
    pub fn new() -> Self {
        Self::default()
    }
    /// Read a value from the state.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.0.read().get(key).copied()
    }
    /// Apply a batch of updates atomically.
    pub fn apply(&self, mut updates: Vec<StateUpdate>) {
        updates.sort_by(|a, b| a.key.cmp(&b.key));
        let mut state = self.0.write();
        for update in updates {
            state.insert(update.key, update.value);
        }
    }
}
/// Update produced by a transaction execution.
#[derive(Clone, Debug)]
pub struct StateUpdate {
    pub key: StateKey,
    pub value: Value,
}
/// Snapshot of state values available to a transaction while executing.
pub type StateSnapshot = HashMap<StateKey, Value>;
/// Result of a single transaction.
#[derive(Clone, Debug, Default)]
pub struct TxResult {
    pub success: bool,
    pub gas_used: u64,
}
/// Result of executing a whole block.
#[derive(Clone, Debug, Default)]
pub struct BlockResult {
    pub tx_results: Vec<TxResult>,
}
/// Simple transaction type used by the scheduler.  Real transactions would
/// include signatures and additional metadata.
#[derive(Clone, Debug)]
pub struct Transaction {
    pub code: Vec<u8>,
    pub gas_limit: u64,
    pub access: StateAccessSet,
}
/// A block is just a list of transactions.
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub transactions: Vec<Transaction>,
}
/// Execution context owned by a worker thread.
///
/// Each worker executing a transaction operates on its own instance of this
/// struct so registers and scratch memory do not leak across transactions.
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub registers: [u64; REGISTER_COUNT],
    pub memory: Vec<u8>,
    pub pc: usize,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub write_set: Vec<StateUpdate>,
    pub read_set: StateSnapshot,
    pub result: Option<TxResult>,
}
impl ExecutionContext {
    /// Create a fresh empty context.
    pub fn new() -> Self {
        Self {
            registers: [0u64; REGISTER_COUNT],
            memory: Vec::new(),
            pc: 0,
            gas_used: 0,
            gas_limit: 0,
            write_set: Vec::new(),
            read_set: HashMap::new(),
            result: None,
        }
    }
    /// Reset registers and transient memory between transactions.
    pub fn reset(&mut self) {
        self.registers = [0u64; REGISTER_COUNT];
        self.memory.clear();
        self.pc = 0;
        self.gas_used = 0;
        self.gas_limit = 0;
        self.write_set.clear();
        self.read_set.clear();
        self.result = None;
    }
    /// Prepare the context for executing `tx`.
    pub fn init_for_transaction(&mut self, tx: &Transaction, state: &State) {
        self.reset();
        self.gas_limit = tx.gas_limit;
        for key in tx.access.read_keys.iter() {
            if let Some(v) = state.get(key) {
                self.read_set.insert(key.clone(), v);
            }
        }
    }
    /// Read a value from the prefetched state snapshot.
    pub fn read(&self, key: &str) -> Option<Value> {
        self.read_set.get(key).copied()
    }
    /// Record a state write for later commit.
    pub fn write(&mut self, key: StateKey, value: Value) {
        self.write_set.push(StateUpdate {
            key: key.clone(),
            value,
        });
        self.read_set.insert(key, value);
    }
}
impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}
/// Directed acyclic graph describing transaction dependencies.
#[derive(Clone, Debug, Default)]
pub struct DependencyGraph {
    pub tx_count: usize,
    pub adj: Vec<Vec<usize>>,
    pub indegree: Vec<usize>,
}
impl DependencyGraph {
    /// Build a dependency graph from a block according to transaction
    /// read/write sets. Edges are added between conflicting transactions in
    /// block order so that the resulting graph is deterministic.
    pub fn build_from_block(block: &Block) -> Self {
        let tx_count = block.transactions.len();
        let transactions = &block.transactions;
        use rayon::prelude::*;
        let edges: Vec<(usize, usize)> = (0..tx_count)
            .into_par_iter()
            .flat_map(|i| {
                let a = &transactions[i].access;
                (i + 1..tx_count)
                    .filter_map(|j| {
                        let b = &transactions[j].access;
                        if Self::conflicts(a, b) {
                            Some((i, j))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut graph = DependencyGraph {
            tx_count,
            adj: vec![Vec::new(); tx_count],
            indegree: vec![0; tx_count],
        };
        let mut edges = edges;
        edges.sort_unstable();
        for (i, j) in edges {
            graph.add_edge(i, j);
        }
        graph
    }
    /// Detect if two access sets conflict.
    fn conflicts(a: &StateAccessSet, b: &StateAccessSet) -> bool {
        !a.write_keys.is_disjoint(&b.write_keys)
            || !a.write_keys.is_disjoint(&b.read_keys)
            || !a.read_keys.is_disjoint(&b.write_keys)
            || !a.reg_tags.is_disjoint(&b.reg_tags)
    }
    /// Add a directed edge from `i` to `j` in the graph.
    fn add_edge(&mut self, i: usize, j: usize) {
        self.adj[i].push(j);
        self.indegree[j] += 1;
    }
}
/// Scheduler for deterministic transaction-level block execution.
///
/// This type provides a very small implementation of the scheduling behaviour described in the
/// "Iroha VM Parallel Block Execution – Rust Implementation Specification". A dependency graph is
/// built for the transactions in a block and tasks whose dependencies are satisfied are spawned on
/// a thread pool whose size may grow or shrink dynamically. Its deferred-output path lets the
/// coordinator publish successful writes in block order. The thread count defaults to the number
/// of available CPU cores but can be limited by the caller.
const LOAD_WINDOW: usize = 8;
/// Stack size allocated for Rayon worker threads used by the scheduler.
///
/// This value is mutable via [`set_thread_stack_size`] to allow hosts to align
/// worker stacks with deployment policy.
static THREAD_STACK_SIZE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(32 * 1024 * 1024);
/// Current Rayon worker stack size used by scheduler pools.
pub fn thread_stack_size() -> usize {
    THREAD_STACK_SIZE.load(std::sync::atomic::Ordering::Relaxed)
}
/// Override the Rayon worker stack size used by scheduler pools.
pub fn set_thread_stack_size(bytes: usize) {
    THREAD_STACK_SIZE.store(bytes.max(1), std::sync::atomic::Ordering::Relaxed);
}
pub struct Scheduler {
    pool: RwLock<Option<ThreadPool>>,
    min_threads: usize,
    max_threads: usize,
    current_threads: AtomicUsize,
    load_history: Mutex<VecDeque<usize>>,
    #[cfg(feature = "cuda")]
    gpu_manager: Option<crate::gpu_manager::GpuManager>,
    forced_simd: AtomicU8,
}
impl Scheduler {
    fn encode_simd(choice: Option<SimdChoice>) -> u8 {
        match choice {
            None => 0,
            Some(SimdChoice::Scalar) => 1,
            Some(SimdChoice::Sse2) => 2,
            Some(SimdChoice::Avx2) => 3,
            Some(SimdChoice::Avx512) => 4,
            Some(SimdChoice::Neon) => 5,
        }
    }
    fn decode_simd(val: u8) -> Option<SimdChoice> {
        match val {
            1 => Some(SimdChoice::Scalar),
            2 => Some(SimdChoice::Sse2),
            3 => Some(SimdChoice::Avx2),
            4 => Some(SimdChoice::Avx512),
            5 => Some(SimdChoice::Neon),
            _ => None,
        }
    }
    /// Create a scheduler using a fixed-size thread pool.  If `num_threads` is
    /// zero all physical CPU cores are used.
    pub fn new(num_threads: usize) -> Self {
        Self::new_dynamic(num_threads, num_threads)
    }
    /// Create a scheduler with dynamic thread limits. When either limit is
    /// zero all physical CPU cores are used for that bound.
    pub fn new_dynamic(min_threads: usize, max_threads: usize) -> Self {
        let phys = num_cpus::get_physical().max(1);
        let mut min = if min_threads == 0 { phys } else { min_threads };
        let mut max = if max_threads == 0 { phys } else { max_threads };
        min = min.max(1);
        max = max.max(1);
        if max < min {
            // Respect the configured max bound; clamp the minimum down.
            min = max;
        }
        let stack = thread_stack_size();
        let mut built_threads = min;
        let mut pool: Option<ThreadPool> = None;
        // Try progressively smaller pools to avoid crashing on misconfiguration
        // (e.g., huge thread counts or stack sizes that cannot be allocated).
        while built_threads > 1 {
            if let Ok(p) = rayon::ThreadPoolBuilder::new()
                .num_threads(built_threads)
                .stack_size(stack)
                .build()
            {
                pool = Some(p);
                break;
            }
            if let Ok(p) = rayon::ThreadPoolBuilder::new()
                .num_threads(built_threads)
                .build()
            {
                pool = Some(p);
                break;
            }
            built_threads = (built_threads / 2).max(1);
        }
        if pool.is_none() {
            pool = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .stack_size(stack)
                .build()
                .or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(1).build())
                .ok();
        }
        if pool.is_none() {
            // If we cannot spawn even a single worker thread, fall back to
            // deterministic sequential execution.
            min = 1;
            max = 1;
        } else if built_threads < min {
            // If we couldn't satisfy the requested min due to resource limits,
            // disable growth to avoid repeated failed rebuild attempts.
            min = built_threads;
            max = built_threads;
        }
        Self {
            pool: RwLock::new(pool),
            min_threads: min,
            max_threads: max,
            current_threads: AtomicUsize::new(min),
            load_history: Mutex::new(VecDeque::new()),
            #[cfg(feature = "cuda")]
            gpu_manager: crate::gpu_manager::GpuManager::init(),
            forced_simd: AtomicU8::new(0),
        }
    }
    /// Configure a forced SIMD backend applied to worker threads.
    pub fn set_forced_simd(&self, choice: Option<SimdChoice>) {
        self.forced_simd
            .store(Self::encode_simd(choice), Ordering::SeqCst);
    }
    fn run_with_simd_override<T, F: FnOnce() -> T>(&self, f: F) -> T {
        let forced = Self::decode_simd(self.forced_simd.load(Ordering::SeqCst));
        let _restore_override = ThreadSimdOverrideGuard(set_thread_forced_simd(forced));
        f()
    }
    /// Number of GPUs detected when the scheduler was created.
    #[cfg(not(feature = "cuda"))]
    pub fn gpu_count(&self) -> usize {
        0
    }
    /// Number of GPUs detected when the scheduler was created.
    #[cfg(feature = "cuda")]
    pub fn gpu_count(&self) -> usize {
        self.gpu_manager
            .as_ref()
            .map(|g| g.device_count())
            .unwrap_or(0)
    }
    /// Current number of threads in the pool.
    pub fn thread_count(&self) -> usize {
        self.current_threads.load(Ordering::SeqCst)
    }
    fn execute_tx<T, F: FnOnce() -> T>(&self, func: F) -> T {
        self.run_with_simd_override(func)
    }
    /// Execute all transactions in `block` respecting dependencies derived from
    /// their access sets. Transactions without conflicts are executed in
    /// parallel on the thread pool. Results are returned in block order.
    pub fn schedule_block<F>(&self, block: Block, exec: F) -> BlockResult
    where
        F: Copy + Fn(Transaction) -> TxResult + Send + Sync,
    {
        self.schedule_block_with_ordered_commit(block, move |tx| (exec(tx), ()), |_, _, ()| {})
    }
    /// Execute transactions in parallel and publish successful deferred outputs
    /// from the coordinator in original block order.
    ///
    /// `exec` must not publish consensus-visible state itself. Instead it returns
    /// that transaction's buffered output alongside its [`TxResult`]. The
    /// `commit` callback runs on the coordinating thread only after every lower
    /// transaction index has completed. Outputs from failed transactions are
    /// discarded. Dependency edges are released after publication, so a
    /// conflicting successor observes its predecessor's committed state.
    pub(crate) fn schedule_block_with_ordered_commit<O, F, C>(
        &self,
        block: Block,
        exec: F,
        mut commit: C,
    ) -> BlockResult
    where
        O: Send,
        F: Copy + Fn(Transaction) -> (TxResult, O) + Send + Sync,
        C: FnMut(usize, &TxResult, O),
    {
        let graph = DependencyGraph::build_from_block(&block);
        let tx_count = graph.tx_count;
        let mut indegree = graph.indegree.clone();
        let mut txs: Vec<Option<Transaction>> = block.transactions.into_iter().map(Some).collect();
        let (sender, receiver) = crossbeam_channel::bounded(tx_count);
        let mut completed: Vec<Option<(TxResult, O)>> = (0..tx_count).map(|_| None).collect();
        let mut results = vec![TxResult::default(); tx_count];
        let mut pending: BTreeSet<usize> = (0..tx_count).collect();
        let mut next_commit = 0usize;
        while next_commit < tx_count {
            let mut ready: Vec<usize> = pending
                .iter()
                .filter(|&&idx| indegree[idx] == 0)
                .copied()
                .collect();
            if ready.is_empty() {
                // The dependency graph only contains forward edges, so this is
                // an internal-consistency fallback rather than an expected path.
                ready.push(*pending.iter().next().expect("uncommitted transaction"));
            }
            for &idx in &ready {
                pending.remove(&idx);
            }
            if let Some(pool) = self.pool.read().as_ref() {
                pool.scope(|s| {
                    for &idx in &ready {
                        let tx = txs[idx]
                            .take()
                            .expect("a transaction is scheduled exactly once");
                        let exec_fn = exec;
                        let result_sender = sender.clone();
                        let scheduler = self;
                        s.spawn(move |_| {
                            let output = scheduler.execute_tx(move || exec_fn(tx));
                            result_sender
                                .send((idx, output))
                                .unwrap_or_else(|_| panic!("block result receiver remains alive"));
                        });
                    }
                });
            } else {
                // Deterministic sequential fallback if no worker pool is available.
                for &idx in &ready {
                    let tx = txs[idx]
                        .take()
                        .expect("a transaction is scheduled exactly once");
                    let exec_fn = exec;
                    let output = self.execute_tx(move || exec_fn(tx));
                    sender
                        .send((idx, output))
                        .unwrap_or_else(|_| panic!("block result receiver remains alive"));
                }
            }
            for _ in 0..ready.len() {
                let (idx, output) = receiver
                    .recv()
                    .expect("every scheduled transaction sends one result");
                completed[idx] = Some(output);
            }
            while let Some((result, output)) = completed[next_commit].take() {
                if result.success {
                    commit(next_commit, &result, output);
                }
                results[next_commit] = result;
                for &dependent in &graph.adj[next_commit] {
                    indegree[dependent] = indegree[dependent].saturating_sub(1);
                }
                next_commit += 1;
                if next_commit == tx_count {
                    break;
                }
            }
        }
        self.record_load(tx_count);
        self.adjust_pool();
        BlockResult {
            tx_results: results,
        }
    }
    fn record_load(&self, tx_count: usize) {
        let mut hist = self.load_history.lock();
        hist.push_back(tx_count);
        if hist.len() > LOAD_WINDOW {
            hist.pop_front();
        }
    }
    fn adjust_pool(&self) {
        let avg: usize = {
            let hist = self.load_history.lock();
            if hist.is_empty() {
                return;
            }
            hist.iter().sum::<usize>() / hist.len()
        };
        let cur = self.current_threads.load(Ordering::SeqCst);
        let mut new_size = cur;
        if avg > cur * 4 && cur < self.max_threads {
            new_size = std::cmp::min(cur * 2, self.max_threads);
        } else if avg < cur / 2 && cur > self.min_threads {
            new_size = std::cmp::max(cur / 2, self.min_threads);
        }
        if new_size != cur
            && let Ok(pool) = rayon::ThreadPoolBuilder::new()
                .num_threads(new_size)
                .stack_size(thread_stack_size())
                .build()
        {
            *self.pool.write() = Some(pool);
            self.current_threads.store(new_size, Ordering::SeqCst);
        }
    }
}
// Global defaults for scheduler thread limits (set by the host/node).
// 0 means "auto" (use physical cores).
static DEFAULT_SCHED_MIN: AtomicUsize = AtomicUsize::new(0);
static DEFAULT_SCHED_MAX: AtomicUsize = AtomicUsize::new(0);
/// Set global default scheduler thread limits used by [`IVM::new`].
///
/// - Pass `None` to keep "auto" for that bound (uses physical cores).
/// - Bounds are clamped to at least 1 and `min <= max` is enforced by clamping `min` down.
pub fn set_default_scheduler_limits(min_threads: Option<usize>, max_threads: Option<usize>) {
    let min = min_threads.unwrap_or(0);
    let max = max_threads.unwrap_or(0);
    // Store raw values (0 = auto) and validate when read.
    DEFAULT_SCHED_MIN.store(min, Ordering::SeqCst);
    DEFAULT_SCHED_MAX.store(max, Ordering::SeqCst);
}
/// Read global default scheduler limits as concrete `(min, max)` counts.
///
/// 0 values are resolved to the current number of physical cores.
pub fn default_scheduler_limits() -> (usize, usize) {
    let phys = num_cpus::get_physical().max(1);
    let mut min = DEFAULT_SCHED_MIN.load(Ordering::SeqCst);
    let mut max = DEFAULT_SCHED_MAX.load(Ordering::SeqCst);
    if min == 0 {
        min = phys;
    }
    if max == 0 {
        max = phys;
    }
    min = min.max(1);
    max = max.max(1);
    if max < min {
        // Respect the configured max bound; clamp the minimum down.
        min = max;
    }
    (min, max)
}
/// Read and write sets associated with a transaction for conflict detection.
#[derive(Clone, Debug)]
pub struct StateAccessSet {
    pub read_keys: HashSet<StateKey>,
    pub write_keys: HashSet<StateKey>,
    /// Optional register tags used for additional conflict detection.
    pub reg_tags: HashSet<usize>,
}
impl StateAccessSet {
    pub fn new() -> Self {
        Self {
            read_keys: HashSet::new(),
            write_keys: HashSet::new(),
            reg_tags: HashSet::new(),
        }
    }
}
impl Default for StateAccessSet {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn deep_recurse(n: usize) {
        let buf = [0u8; 1024];
        std::hint::black_box(&buf);
        if n > 0 {
            deep_recurse(n - 1);
        }
    }
    #[test]
    fn scheduler_threads_have_sufficient_stack() {
        let scheduler = Scheduler::new(1);
        let pool_guard = scheduler.pool.read();
        let pool = pool_guard
            .as_ref()
            .expect("scheduler should build a thread pool in tests");
        pool.install(|| deep_recurse(4096));
    }
    #[test]
    fn schedule_block_preserves_order_for_conflict_free_transactions() {
        // Gas limit carries the original index for verification.
        let mut txs = Vec::new();
        for i in 0..64u64 {
            let mut access = StateAccessSet::new();
            access.write_keys.insert(format!("k{i}"));
            txs.push(Transaction {
                code: vec![],
                gas_limit: i,
                access,
            });
        }
        let scheduler = Scheduler::new(2);
        let r = scheduler.schedule_block(Block { transactions: txs }, |tx| TxResult {
            success: true,
            gas_used: tx.gas_limit,
        });
        // Results must align with block order
        for (i, tr) in r.tx_results.iter().enumerate() {
            assert_eq!(tr.gas_used as usize, i);
        }
    }
    #[test]
    fn ordered_commit_buffers_later_ready_transactions() {
        let transaction = |id: u8, read_keys: &[&str], write_keys: &[&str]| {
            let mut access = StateAccessSet::new();
            access
                .read_keys
                .extend(read_keys.iter().map(|key| (*key).to_owned()));
            access
                .write_keys
                .extend(write_keys.iter().map(|key| (*key).to_owned()));
            Transaction {
                code: vec![id],
                gas_limit: 0,
                access,
            }
        };
        let block = Block {
            transactions: vec![
                transaction(0, &[], &["a"]),
                transaction(1, &["a"], &["b"]),
                transaction(2, &[], &["c"]),
            ],
        };
        let state = State::new();
        let scheduler = Scheduler::new(2);
        let mut commit_order = Vec::new();
        let result = scheduler.schedule_block_with_ordered_commit(
            block,
            |tx| {
                let id = tx.code[0];
                if id == 1 {
                    assert_eq!(state.get("a"), Some(10));
                }
                let key = match id {
                    0 => "a",
                    1 => "b",
                    2 => "c",
                    _ => unreachable!("fixture transaction id"),
                };
                (
                    TxResult {
                        success: true,
                        gas_used: u64::from(id),
                    },
                    vec![StateUpdate {
                        key: key.to_owned(),
                        value: u64::from(id) + 10,
                    }],
                )
            },
            |index, _, updates| {
                state.apply(updates);
                commit_order.push(index);
                if index < 2 {
                    assert_eq!(state.get("c"), None);
                }
            },
        );

        assert_eq!(commit_order, vec![0, 1, 2]);
        assert_eq!(state.get("a"), Some(10));
        assert_eq!(state.get("b"), Some(11));
        assert_eq!(state.get("c"), Some(12));
        assert_eq!(
            result
                .tx_results
                .iter()
                .map(|tx| tx.gas_used)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }
    #[test]
    fn ordered_commit_discards_failed_transaction_output() {
        let transactions = (0_u8..3)
            .map(|id| Transaction {
                code: vec![id],
                gas_limit: 0,
                access: StateAccessSet::new(),
            })
            .collect();
        let scheduler = Scheduler::new(2);
        let mut committed = Vec::new();

        let result = scheduler.schedule_block_with_ordered_commit(
            Block { transactions },
            |tx| {
                let id = tx.code[0];
                (
                    TxResult {
                        success: id != 1,
                        gas_used: u64::from(id),
                    },
                    id,
                )
            },
            |index, _, output| committed.push((index, output)),
        );

        assert_eq!(committed, vec![(0, 0), (2, 2)]);
        assert!(!result.tx_results[1].success);
    }
    #[test]
    fn scheduler_applies_forced_simd_on_worker_threads() {
        let _simd_guard = crate::vector::forced_simd_test_lock();
        let scheduler = Scheduler::new(2);
        scheduler.set_forced_simd(Some(SimdChoice::Scalar));
        let r = scheduler.schedule_block(
            Block {
                transactions: vec![Transaction {
                    code: vec![],
                    gas_limit: 0,
                    access: StateAccessSet::new(),
                }],
            },
            |_tx| TxResult {
                success: true,
                gas_used: if crate::vector::simd_choice() == SimdChoice::Scalar {
                    1
                } else {
                    0
                },
            },
        );
        assert_eq!(r.tx_results[0].gas_used, 1);
    }
    #[test]
    fn scheduler_propagates_panics_and_restores_thread_simd_override() {
        let _simd_guard = crate::vector::forced_simd_test_lock();
        let scheduler = Scheduler::new(1);
        scheduler.set_forced_simd(Some(SimdChoice::Scalar));
        let previous_override = set_thread_forced_simd(Some(SimdChoice::Sse2));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scheduler.execute_tx(|| panic!("executor panic must propagate"));
        }));
        let restored_override = set_thread_forced_simd(previous_override);
        assert!(panic.is_err());
        assert_eq!(restored_override, Some(SimdChoice::Sse2));
    }
}
