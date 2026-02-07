//! Parallel execution components implementing the deterministic block
//! execution engine described in the "Iroha VM Parallel Block Execution – Rust
//! Implementation Specification".
//!
//! This module implements the hybrid STM/HTM approach described in the
//! architecture spec. On x86_64 hosts that advertise Intel RTM support we
//! attempt hardware transactions; other targets (or CPUs without RTM) fall back
//! to the deterministic mutex path.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::ThreadPool;

use crate::vector::{SimdChoice, set_thread_forced_simd};

#[cfg(all(
    feature = "htm",
    target_arch = "x86_64",
    not(any(target_os = "ios", target_os = "tvos", target_os = "watchos"))
))]
mod htm_util {
    use core::arch::x86_64::{_XBEGIN_STARTED, _xbegin, _xend};
    use std::{collections::HashSet, sync::LazyLock};

    use parking_lot::Mutex;

    static STM_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    static ACTIVE_TAGS: LazyLock<Mutex<HashSet<usize>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    pub fn with_transaction<F, T>(tags: &HashSet<usize>, f: F) -> Result<T, ()>
    where
        F: FnOnce() -> T,
    {
        {
            let mut active = ACTIVE_TAGS.lock();
            if !active.is_disjoint(tags) {
                return Err(());
            }
            for &t in tags {
                active.insert(t);
            }
        }

        let status = unsafe { _xbegin() };
        if status == _XBEGIN_STARTED {
            let r = f();
            unsafe { _xend() };
            let mut active = ACTIVE_TAGS.lock();
            for t in tags {
                active.remove(t);
            }
            Ok(r)
        } else {
            let mut active = ACTIVE_TAGS.lock();
            for t in tags {
                active.remove(t);
            }
            Err(())
        }
    }

    pub fn with_mutex<F, T>(tags: &HashSet<usize>, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _lock = STM_MUTEX.lock();
        {
            let mut active = ACTIVE_TAGS.lock();
            for &t in tags {
                active.insert(t);
            }
        }
        let r = f();
        {
            let mut active = ACTIVE_TAGS.lock();
            for t in tags {
                active.remove(t);
            }
        }
        r
    }
}

#[cfg(not(all(
    feature = "htm",
    target_arch = "x86_64",
    not(any(target_os = "ios", target_os = "tvos", target_os = "watchos"))
)))]
mod htm_util {
    use std::{collections::HashSet, sync::LazyLock};

    use parking_lot::Mutex;

    static STM_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    static ACTIVE_TAGS: LazyLock<Mutex<HashSet<usize>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    #[allow(dead_code)]
    pub fn with_transaction<F, T>(_tags: &HashSet<usize>, _f: F) -> Result<T, ()>
    where
        F: FnOnce() -> T,
    {
        Err(())
    }

    pub fn with_mutex<F, T>(tags: &HashSet<usize>, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _lock = STM_MUTEX.lock();
        {
            let mut active = ACTIVE_TAGS.lock();
            for &t in tags {
                active.insert(t);
            }
        }
        let r = f();
        {
            let mut active = ACTIVE_TAGS.lock();
            for t in tags {
                active.remove(t);
            }
        }
        r
    }
}

/// Number of general purpose registers used by the execution contexts.
///
/// Matches the main VM register file size.
pub const REGISTER_COUNT: usize = 256;

/// Identifier for a state entry in the world state.  For the purposes of this
/// crate it is simply a string key but in a real integration this could be a
/// complex type.
pub type StateKey = String;

/// Generic state value type.
pub type Value = u64;

/// Shared key-value state accessed by transactions.
#[derive(Clone, Default)]
pub struct State(Arc<DashMap<StateKey, Value>>);

impl State {
    /// Create an empty state.
    pub fn new() -> Self {
        Self(Arc::new(DashMap::new()))
    }

    /// Read a value from the state.
    pub fn get(&self, key: &StateKey) -> Option<Value> {
        self.0.get(key).map(|v| *v)
    }

    /// Apply a batch of updates atomically.
    pub fn apply(&self, updates: &[StateUpdate]) {
        let mut sorted: Vec<_> = updates.iter().collect();
        sorted.sort_by(|a, b| a.key.cmp(&b.key));
        for upd in sorted {
            self.0.insert(upd.key.clone(), upd.value);
        }
    }

    /// Apply a batch of updates using hardware transactional memory when
    /// available. If HTM is unsupported or the transaction aborts, a
    /// fallback lock is used to ensure atomicity.
    pub fn apply_atomic(&self, updates: &[StateUpdate], tags: &HashSet<usize>, _use_htm: bool) {
        #[cfg(all(
            feature = "htm",
            target_arch = "x86_64",
            not(any(target_os = "ios", target_os = "tvos", target_os = "watchos"))
        ))]
        if _use_htm && crate::ivm::rtm_available() {
            use core::arch::x86_64::_xtest;
            unsafe {
                if _xtest() != 0 {
                    for upd in updates {
                        self.0.insert(upd.key.clone(), upd.value);
                    }
                    return;
                }
            }
            let res = htm_util::with_transaction(tags, || {
                for upd in updates {
                    self.0.insert(upd.key.clone(), upd.value);
                }
            });
            if res.is_ok() {
                return;
            }
        }

        htm_util::with_mutex(tags, || {
            for upd in updates {
                self.0.insert(upd.key.clone(), upd.value);
            }
        });
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
    pub fn read(&self, key: &StateKey) -> Option<Value> {
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

/// Metadata about a single instruction and its scheduling state.
#[derive(Clone, Debug)]
pub struct InstructionNode {
    pub tx_index: usize,
    pub instr_index: usize,
    pub opcode: u8,
    pub read_set: HashSet<StateKey>,
    pub write_set: HashSet<StateKey>,
    pub gas_cost: u64,
    pub dependents: Vec<usize>,
    pub dep_count: usize,
}

impl InstructionNode {
    pub fn new(tx_index: usize, instr_index: usize, opcode: u8) -> Self {
        Self {
            tx_index,
            instr_index,
            opcode,
            read_set: HashSet::new(),
            write_set: HashSet::new(),
            gas_cost: 0,
            dependents: Vec::new(),
            dep_count: 0,
        }
    }

    pub fn add_dependent(&mut self, idx: usize) {
        self.dependents.push(idx);
    }

    pub fn decrement(&mut self) {
        if self.dep_count > 0 {
            self.dep_count -= 1;
        }
    }
}

/// Directed acyclic graph describing instruction dependencies.
#[derive(Clone, Debug, Default)]
pub struct DependencyGraph {
    pub tx_count: usize,
    pub adj: Vec<Vec<usize>>,
    pub indegree: Vec<usize>,
    pub access_list: Vec<StateAccessSet>,
}

impl DependencyGraph {
    /// Build a dependency graph from a block according to transaction
    /// read/write sets. Edges are added between conflicting transactions in
    /// block order so that the resulting graph is deterministic.
    pub fn build_from_block(block: &Block) -> Self {
        let tx_count = block.transactions.len();
        let mut graph = DependencyGraph {
            tx_count,
            adj: vec![Vec::new(); tx_count],
            indegree: vec![0; tx_count],
            access_list: block
                .transactions
                .iter()
                .map(|t| t.access.clone())
                .collect(),
        };

        use rayon::prelude::*;

        let edges: Vec<(usize, usize)> = (0..tx_count)
            .into_par_iter()
            .flat_map(|i| {
                let a = &graph.access_list[i];
                (i + 1..tx_count)
                    .filter_map(|j| {
                        let b = &graph.access_list[j];
                        if Self::conflicts(a, b) {
                            Some((i, j))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut edges = edges;
        edges.sort_unstable();
        for (i, j) in edges {
            graph.add_edge(i, j);
        }

        graph
    }

    /// Detect if two access sets conflict.
    fn conflicts(a: &StateAccessSet, b: &StateAccessSet) -> bool {
        for key in &a.write_keys {
            if b.write_keys.contains(key) || b.read_keys.contains(key) {
                return true;
            }
        }
        for key in &a.read_keys {
            if b.write_keys.contains(key) {
                return true;
            }
        }
        if !a.reg_tags.is_disjoint(&b.reg_tags) {
            return true;
        }
        false
    }

    /// Add a directed edge from `i` to `j` in the graph.
    fn add_edge(&mut self, i: usize, j: usize) {
        self.adj[i].push(j);
        self.indegree[j] += 1;
    }
}

/// Thread-safe buffer used to collect instruction results.
pub struct ResultBuffer {
    sender: crossbeam_channel::Sender<(usize, TxResult)>,
    receiver: crossbeam_channel::Receiver<(usize, TxResult)>,
}

impl ResultBuffer {
    pub fn new(size: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(size);
        Self { sender, receiver }
    }

    pub fn store(&self, idx: usize, result: TxResult) {
        let _ = self.sender.send((idx, result));
    }

    pub fn take_ready(&self) -> Option<(usize, TxResult)> {
        self.receiver.try_recv().ok()
    }
}

/// Scheduler driving instruction-level parallelism.
///
/// This type provides a very small implementation of the scheduling
/// behaviour described in the "Iroha VM Parallel Block Execution – Rust
/// Implementation Specification".  A dependency graph is built for the
/// transactions in a block and tasks whose dependencies are satisfied are
/// spawned on a thread pool whose size may grow or shrink dynamically.
/// Results are committed in block order
/// via a `ResultBuffer`.  If any error occurs the scheduler falls back to a
/// sequential execution of the remaining transactions.  The thread count
/// defaults to the number of available CPU cores but can be limited by the
/// caller.
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
    htm: bool,
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
        let htm = cfg!(feature = "htm") && crate::ivm::rtm_available();
        Self::new_dynamic_with_htm_flag(min_threads, max_threads, htm)
    }

    /// Testing helper to create a scheduler with the HTM flag explicitly set.
    pub fn new_with_htm_flag(num_threads: usize, htm: bool) -> Self {
        Self::new_dynamic_with_htm_flag(num_threads, num_threads, htm)
    }

    /// Create a scheduler with dynamic limits and explicit HTM flag.
    pub fn new_dynamic_with_htm_flag(min_threads: usize, max_threads: usize, htm: bool) -> Self {
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
            htm,
            #[cfg(feature = "cuda")]
            gpu_manager: crate::gpu_manager::GpuManager::init(),
            forced_simd: AtomicU8::new(0),
        }
    }

    /// Returns `true` if hardware transactional memory is available on this
    /// host CPU.
    pub fn htm_available(&self) -> bool {
        self.htm
    }

    /// Configure a forced SIMD backend applied to worker threads.
    pub fn set_forced_simd(&self, choice: Option<SimdChoice>) {
        self.forced_simd
            .store(Self::encode_simd(choice), Ordering::SeqCst);
    }

    fn run_with_simd_override<T, F: FnOnce() -> T>(&self, f: F) -> T {
        let forced = Self::decode_simd(self.forced_simd.load(Ordering::SeqCst));
        let prev = set_thread_forced_simd(forced);
        let result = f();
        set_thread_forced_simd(prev);
        result
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

    fn execute_tx<F>(&self, func: F) -> TxResult
    where
        F: FnOnce() -> TxResult + std::panic::UnwindSafe,
    {
        self.run_with_simd_override(|| {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(func)) {
                Ok(r) => r,
                Err(_) => TxResult {
                    success: false,
                    gas_used: 0,
                },
            }
        })
    }

    /// Execute all transactions in `block` respecting dependencies derived from
    /// their access sets. Transactions without conflicts are executed in
    /// parallel on the thread pool. Results are returned in block order.
    pub fn schedule_block<F>(&self, block: Block, exec: F) -> BlockResult
    where
        F: Copy
            + Fn(Transaction) -> TxResult
            + Send
            + Sync
            + std::panic::RefUnwindSafe
            + std::panic::UnwindSafe,
    {
        let graph = DependencyGraph::build_from_block(&block);
        let tx_count = graph.tx_count;
        let mut indegree = graph.indegree.clone();
        let txs = block.transactions;

        let result_buf = Arc::new(ResultBuffer::new(tx_count));
        let mut completed = vec![false; tx_count];
        let mut results = vec![TxResult::default(); tx_count];
        let mut pending: BTreeSet<usize> = (0..tx_count).collect();

        while !pending.is_empty() {
            let ready: Vec<usize> = pending
                .iter()
                .filter(|&&idx| indegree[idx] == 0)
                .copied()
                .collect();

            if ready.is_empty() {
                // Cycle in graph or unexpected state; execute sequentially
                let idx = *pending.iter().next().unwrap();
                let tx = txs[idx].clone();
                let exec_fn = exec;
                let tx_clone = tx.clone();
                let res = self.execute_tx(move || {
                    let exec_fn = std::panic::AssertUnwindSafe(exec_fn);
                    (*exec_fn)(tx_clone.clone())
                });
                result_buf.store(idx, res);

                if let Some((i, r)) = result_buf.take_ready() {
                    results[i] = r;
                    completed[i] = true;
                    pending.remove(&i);
                    for &j in &graph.adj[i] {
                        indegree[j] = indegree[j].saturating_sub(1);
                    }
                }
            } else {
                let result_buf_for_scope = Arc::clone(&result_buf);
                if let Some(pool) = self.pool.read().as_ref() {
                    pool.scope(|s| {
                        for &idx in &ready {
                            let tx = txs[idx].clone();
                            let buf = Arc::clone(&result_buf_for_scope);
                            let exec_fn = exec;
                            let scheduler = self;
                            s.spawn(move |_| {
                                let r = scheduler.execute_tx(move || exec_fn(tx));
                                buf.store(idx, r);
                            });
                        }
                    });
                } else {
                    // Deterministic sequential fallback if no worker pool is available.
                    for &idx in &ready {
                        let tx = txs[idx].clone();
                        let exec_fn = exec;
                        let res = self.execute_tx(move || exec_fn(tx));
                        result_buf_for_scope.store(idx, res);
                    }
                }

                let mut step_results = Vec::with_capacity(ready.len());
                for _ in 0..ready.len() {
                    if let Some(res) = result_buf.take_ready() {
                        step_results.push(res);
                    }
                }
                step_results.sort_by_key(|&(i, _)| i);
                for (idx, res) in step_results {
                    results[idx] = res;
                    completed[idx] = true;
                    pending.remove(&idx);
                    for &j in &graph.adj[idx] {
                        indegree[j] = indegree[j].saturating_sub(1);
                    }
                }
            }
        }

        // Drain any remaining results
        while let Some((idx, res)) = result_buf.take_ready() {
            results[idx] = res;
        }

        self.record_load(tx_count);
        self.adjust_pool();

        BlockResult {
            tx_results: results,
        }
    }

    /// Fast path for conflict-free blocks (or pre-grouped blocks).
    ///
    /// Spawns all transactions in parallel and collects results in order
    /// without building a dependency graph. The caller must ensure there are
    /// no internal conflicts in the provided `block`.
    pub fn schedule_block_conflict_free<F>(&self, block: Block, exec: F) -> BlockResult
    where
        F: Copy
            + Fn(Transaction) -> TxResult
            + Send
            + Sync
            + std::panic::RefUnwindSafe
            + std::panic::UnwindSafe,
    {
        let txs = block.transactions;
        let tx_count = txs.len();
        if tx_count == 0 {
            return BlockResult {
                tx_results: Vec::new(),
            };
        }

        let result_buf = Arc::new(ResultBuffer::new(tx_count));
        let result_buf_for_scope = Arc::clone(&result_buf);
        if let Some(pool) = self.pool.read().as_ref() {
            pool.scope(|s| {
                for (idx, tx) in txs.into_iter().enumerate() {
                    let exec_fn = exec;
                    let buf = Arc::clone(&result_buf_for_scope);
                    let scheduler = self;
                    s.spawn(move |_| {
                        let r = scheduler.execute_tx(move || exec_fn(tx));
                        buf.store(idx, r);
                    });
                }
            });
        } else {
            // Deterministic sequential fallback if no worker pool is available.
            for (idx, tx) in txs.into_iter().enumerate() {
                let exec_fn = exec;
                let r = self.execute_tx(move || exec_fn(tx));
                result_buf_for_scope.store(idx, r);
            }
        }

        // All tasks posted to the scope have completed here; drain results.
        let mut results = vec![TxResult::default(); tx_count];
        let mut received = 0usize;
        while received < tx_count {
            if let Some((i, r)) = result_buf.take_ready() {
                results[i] = r;
                received += 1;
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

    /// Determine if this access set conflicts with another based on read/write overlaps.
    pub fn conflicts(&self, other: &StateAccessSet) -> bool {
        for key in &self.write_keys {
            if other.write_keys.contains(key) || other.read_keys.contains(key) {
                return true;
            }
        }
        for key in &self.read_keys {
            if other.write_keys.contains(key) {
                return true;
            }
        }
        if !self.reg_tags.is_disjoint(&other.reg_tags) {
            return true;
        }
        false
    }
}

impl Default for StateAccessSet {
    fn default() -> Self {
        Self::new()
    }
}

pub trait StateAccess {
    fn access_set(&self) -> &StateAccessSet;
}

impl StateAccess for Transaction {
    fn access_set(&self) -> &StateAccessSet {
        &self.access
    }
}

/// Group of non-conflicting transactions executed together.
///
/// A simple deterministic grouping algorithm partitions transactions so that
/// no group contains two transactions which conflict on their read/write sets.
/// Groups are formed in block order: if adding a transaction to the current
/// group would introduce a conflict, the group is closed and a new one is
/// started.  Each group can then be executed in parallel using an
/// [`Scheduler`] and the results committed sequentially.
pub struct TransactionGroup {
    pub transactions: Vec<Transaction>,
}

impl TransactionGroup {
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
        }
    }

    /// Partition a block of transactions into deterministic conflict-free
    /// groups based on their access sets.
    pub fn group_block(block: Block) -> Vec<TransactionGroup> {
        let mut groups = Vec::new();
        let mut current = TransactionGroup::new();
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();

        for tx in block.transactions.into_iter() {
            let conflict = tx
                .access
                .write_keys
                .iter()
                .any(|k| writes.contains(k) || reads.contains(k))
                || tx.access.read_keys.iter().any(|k| writes.contains(k));

            if conflict && !current.transactions.is_empty() {
                groups.push(current);
                current = TransactionGroup::new();
                reads.clear();
                writes.clear();
            }

            reads.extend(tx.access.read_keys.iter().cloned());
            writes.extend(tx.access.write_keys.iter().cloned());
            current.transactions.push(tx);
        }

        if !current.transactions.is_empty() {
            groups.push(current);
        }

        groups
    }

    /// Greedy conflict prediction grouping that tracks aggregate group access
    /// sets to avoid O(n^2) per-group membership checks.
    pub fn group_block_predicted(block: Block) -> Vec<TransactionGroup> {
        let mut groups: Vec<TransactionGroup> = Vec::new();
        // Aggregate access sets per group for fast conflict checks.
        let mut agg_reads: Vec<HashSet<StateKey>> = Vec::new();
        let mut agg_writes: Vec<HashSet<StateKey>> = Vec::new();
        let mut agg_tags: Vec<HashSet<usize>> = Vec::new();

        for tx in block.transactions.into_iter() {
            // Find the first group with which this tx does not conflict.
            let mut target: Option<usize> = None;
            for i in 0..groups.len() {
                let conflict = tx
                    .access
                    .write_keys
                    .iter()
                    .any(|k| agg_writes[i].contains(k) || agg_reads[i].contains(k))
                    || tx
                        .access
                        .read_keys
                        .iter()
                        .any(|k| agg_writes[i].contains(k))
                    || !tx.access.reg_tags.is_disjoint(&agg_tags[i]);
                if !conflict {
                    target = Some(i);
                    break;
                }
            }

            if let Some(i) = target {
                // Safe to move tx (we only evaluated predicates on references).
                // Update aggregate sets for the group.
                for k in tx.access.read_keys.iter() {
                    agg_reads[i].insert(k.clone());
                }
                for k in tx.access.write_keys.iter() {
                    agg_writes[i].insert(k.clone());
                }
                for t in tx.access.reg_tags.iter() {
                    agg_tags[i].insert(*t);
                }
                groups[i].transactions.push(tx);
            } else {
                // Create a new group seeded with this transaction and its access sets.
                let mut r = HashSet::new();
                let mut w = HashSet::new();
                let mut tags = HashSet::new();
                r.extend(tx.access.read_keys.iter().cloned());
                w.extend(tx.access.write_keys.iter().cloned());
                tags.extend(tx.access.reg_tags.iter().copied());
                groups.push(TransactionGroup {
                    transactions: vec![tx],
                });
                agg_reads.push(r);
                agg_writes.push(w);
                agg_tags.push(tags);
            }
        }
        groups
    }
}

impl Default for TransactionGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a block in groups using the provided scheduler.  Groups are derived
/// deterministically with [`TransactionGroup::group_block`].
pub fn execute_block_grouped<F>(scheduler: &Scheduler, block: Block, exec: F) -> BlockResult
where
    F: Copy
        + Fn(Transaction) -> TxResult
        + Send
        + Sync
        + std::panic::RefUnwindSafe
        + std::panic::UnwindSafe,
{
    let groups = TransactionGroup::group_block(block);
    let mut results = Vec::new();
    for group in groups {
        // Groups are constructed to be conflict-free; take the fast path.
        let r = scheduler.schedule_block_conflict_free(
            Block {
                transactions: group.transactions,
            },
            exec,
        );
        results.extend(r.tx_results);
    }
    BlockResult {
        tx_results: results,
    }
}

/// Execute a block using greedy conflict prediction to reduce serialization.
pub fn execute_block_predicted<F>(scheduler: &Scheduler, block: Block, exec: F) -> BlockResult
where
    F: Copy
        + Fn(Transaction) -> TxResult
        + Send
        + Sync
        + std::panic::RefUnwindSafe
        + std::panic::UnwindSafe,
{
    let groups = TransactionGroup::group_block_predicted(block);
    let mut results = Vec::new();
    for group in groups {
        // Groups are constructed to be conflict-free; take the fast path.
        let r = scheduler.schedule_block_conflict_free(
            Block {
                transactions: group.transactions,
            },
            exec,
        );
        results.extend(r.tx_results);
    }
    BlockResult {
        tx_results: results,
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
        let scheduler = Scheduler::new_with_htm_flag(1, false);
        let pool_guard = scheduler.pool.read();
        let pool = pool_guard
            .as_ref()
            .expect("scheduler should build a thread pool in tests");
        pool.install(|| deep_recurse(4096));
    }

    #[test]
    fn group_block_predicted_single_group_when_no_conflicts() {
        // Build a block with unique write keys so there are no conflicts.
        let mut txs = Vec::new();
        for i in 0..100usize {
            let mut access = StateAccessSet::new();
            access.write_keys.insert(format!("k{i}"));
            txs.push(Transaction {
                code: vec![],
                gas_limit: 0,
                access,
            });
        }
        let groups = TransactionGroup::group_block_predicted(Block { transactions: txs });
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].transactions.len(), 100);
    }

    #[test]
    fn schedule_block_conflict_free_preserves_order() {
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
        let scheduler = Scheduler::new_with_htm_flag(2, false);
        let r =
            scheduler.schedule_block_conflict_free(Block { transactions: txs }, |tx| TxResult {
                success: true,
                gas_used: tx.gas_limit,
            });
        // Results must align with block order
        for (i, tr) in r.tx_results.iter().enumerate() {
            assert_eq!(tr.gas_used as usize, i);
        }
    }

    #[test]
    fn scheduler_applies_forced_simd_on_worker_threads() {
        let _simd_guard = crate::vector::forced_simd_test_lock();
        let scheduler = Scheduler::new_with_htm_flag(2, false);
        scheduler.set_forced_simd(Some(SimdChoice::Scalar));
        let r = scheduler.schedule_block_conflict_free(
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
}
