use iroha_crypto::Hash;
use ivm::ProgramMetadata;
use ivm::analysis::{ProgramAnalysis, ProgramAnalysisError};
use ivm::runtime::IvmConfig;
use parking_lot::{Condvar, Mutex};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ops::{Deref, DerefMut},
    sync::Arc,
};
/// Counters for the bounded prepared-contract artifact store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparedContractCacheStats {
    /// Content-addressed cache hits.
    pub hits: u64,
    /// Content-addressed cache misses.
    pub misses: u64,
    /// Full parse, hash, validation, and predecode operations.
    pub preparations: u64,
    /// Entries removed to enforce the configured capacity.
    pub evictions: u64,
    /// Nested-call VM checkouts served by a warmed runtime.
    pub runtime_hits: u64,
    /// Nested-call VM checkouts that required a new runtime.
    pub runtime_misses: u64,
    /// Prepared programs loaded into newly allocated nested runtimes.
    pub runtime_prepared_loads: u64,
    /// Pristine nested runtime baselines built for dirty-page reset.
    pub runtime_template_builds: u64,
    /// Nested runtimes restored and returned to the shared pool.
    pub runtime_dirty_resets: u64,
}
#[derive(Debug)]
struct PreparedContractStore {
    entries: BTreeMap<Hash, Arc<ivm::PreparedContract>>,
    preparing: BTreeSet<Hash>,
    order: VecDeque<Hash>,
    nested_runtimes: BTreeMap<RuntimeKey, SharedRuntimePool>,
    nested_runtime_order: VecDeque<RuntimeKey>,
    capacity: usize,
    stats: PreparedContractCacheStats,
}
/// Cloneable bounded store of immutable prepared contract artifacts.
///
/// The handle is independent from runtime-pool borrowing, so an executing VM
/// can resolve nested contracts without re-entering [`IvmCache`].
#[derive(Clone, Debug)]
pub struct PreparedContractCache {
    inner: Arc<Mutex<PreparedContractStore>>,
    ready: Arc<Condvar>,
}
impl PreparedContractCache {
    /// Construct a store with the same entry bound used by the runtime cache.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PreparedContractStore {
                entries: BTreeMap::new(),
                preparing: BTreeSet::new(),
                order: VecDeque::new(),
                nested_runtimes: BTreeMap::new(),
                nested_runtime_order: VecDeque::new(),
                capacity,
                stats: PreparedContractCacheStats::default(),
            })),
            ready: Arc::new(Condvar::new()),
        }
    }
    /// Resolve or prepare the artifact identified by `code_hash`.
    ///
    /// Hits do not inspect `bytecode`. Misses validate the complete artifact
    /// hash before publishing the prepared value.
    ///
    /// # Errors
    /// Returns [`ivm::VMError::InvalidMetadata`] for malformed artifacts,
    /// [`ivm::VMError::ArtifactAbiHashMismatch`] for stale ABI bindings, or an
    /// invalid-metadata error for an expected artifact-hash mismatch.
    pub fn get_or_prepare(
        &self,
        code_hash: Hash,
        bytecode: &[u8],
    ) -> Result<Arc<ivm::PreparedContract>, ivm::VMError> {
        self.get_or_prepare_with_status(code_hash, bytecode)
            .map(|(contract, _)| contract)
    }
    /// Resolve an already prepared artifact by its trusted content address.
    ///
    /// This lookup deliberately takes no byte slice. Nested-call dispatch uses
    /// it before loading contract bytes from world state, so a warm invocation
    /// performs neither a bytecode clone nor another parse/hash/predecode pass.
    #[must_use]
    pub fn get(&self, code_hash: Hash) -> Option<Arc<ivm::PreparedContract>> {
        let mut store = self.inner.lock();
        let contract = store.entries.get(&code_hash).cloned()?;
        store.stats.hits = store.stats.hits.saturating_add(1);
        store.touch(code_hash);
        Some(contract)
    }
    fn get_or_prepare_with_status(
        &self,
        code_hash: Hash,
        bytecode: &[u8],
    ) -> Result<(Arc<ivm::PreparedContract>, bool), ivm::VMError> {
        let mut store = self.inner.lock();
        loop {
            if let Some(contract) = store.entries.get(&code_hash).cloned() {
                store.stats.hits = store.stats.hits.saturating_add(1);
                store.touch(code_hash);
                return Ok((contract, false));
            }
            if store.preparing.insert(code_hash) {
                store.stats.misses = store.stats.misses.saturating_add(1);
                break;
            }
            self.ready.wait(&mut store);
        }
        drop(store);
        let prepared = ivm::prepare_contract(Arc::from(bytecode))
            .map_err(ivm::ContractArtifactError::into_vm_error);
        let mut store = self.inner.lock();
        store.preparing.remove(&code_hash);
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.ready.notify_all();
                return Err(error);
            }
        };
        store.stats.preparations = store.stats.preparations.saturating_add(1);
        if prepared.code_hash() != code_hash {
            self.ready.notify_all();
            return Err(ivm::VMError::InvalidMetadata);
        }
        let prepared = Arc::new(prepared);
        if let Some(existing) = store.entries.get(&code_hash).cloned() {
            if existing.artifact() != prepared.artifact() {
                self.ready.notify_all();
                return Err(ivm::VMError::InvalidMetadata);
            }
            store.touch(code_hash);
            self.ready.notify_all();
            return Ok((existing, false));
        }
        store.insert(code_hash, Arc::clone(&prepared));
        self.ready.notify_all();
        Ok((prepared, true))
    }
    fn publish(&self, contract: Arc<ivm::PreparedContract>) -> Result<(), ivm::VMError> {
        let code_hash = contract.code_hash();
        let mut store = self.inner.lock();
        if let Some(existing) = store.entries.get(&code_hash).cloned() {
            if existing.artifact() != contract.artifact() {
                return Err(ivm::VMError::InvalidMetadata);
            }
            store.touch(code_hash);
            return Ok(());
        }
        store.insert(code_hash, contract);
        Ok(())
    }
    /// Check out a VM for a nested contract invocation.
    ///
    /// The pool is shared by every host carrying this prepared-cache handle.
    /// Cache hits reuse the loaded program and restore only memory chunks
    /// dirtied by the previous invocation. Re-entrant calls allocate another
    /// runtime when the matching pool is temporarily empty rather than
    /// aliasing mutable VM state. `heap_limit` is part of the runtime identity,
    /// so governance changes cannot reuse a VM carrying stale heap authority.
    pub fn checkout_runtime(
        &self,
        contract: &ivm::PreparedContract,
        gas_limit: u64,
        heap_limit: u64,
    ) -> Result<PreparedRuntimeLease, ivm::VMError> {
        let key = RuntimeKey::new(
            contract.code_hash(),
            stack_limit_for_gas(gas_limit),
            heap_limit,
        );
        let cached = {
            let mut store = self.inner.lock();
            let cached = store.nested_runtimes.get_mut(&key).and_then(|pool| {
                pool.available
                    .pop()
                    .map(|vm| (Arc::clone(&pool.baseline), vm))
            });
            if cached.is_some() {
                store.stats.runtime_hits = store.stats.runtime_hits.saturating_add(1);
                store.touch_nested_runtime(key);
            } else {
                store.stats.runtime_misses = store.stats.runtime_misses.saturating_add(1);
            }
            cached
        };
        if let Some((baseline, mut vm)) = cached {
            vm.set_gas_limit(gas_limit);
            return Ok(PreparedRuntimeLease {
                cache: self.clone(),
                key,
                baseline,
                vm: Some(vm),
            });
        }
        let mut vm = ivm::IVM::new(gas_limit);
        vm.set_zk_trace_enabled(false);
        vm.memory.set_heap_max_limit(heap_limit)?;
        vm.load_prepared(contract)?;
        vm.set_gas_limit(gas_limit);
        let mut store = self.inner.lock();
        store.stats.runtime_prepared_loads = store.stats.runtime_prepared_loads.saturating_add(1);
        let cacheable = store.capacity != 0 && store.entries.contains_key(&key.code_hash);
        let baseline = if let Some(pool) = store.nested_runtimes.get(&key) {
            Arc::clone(&pool.baseline)
        } else {
            store.stats.runtime_template_builds =
                store.stats.runtime_template_builds.saturating_add(1);
            let baseline = Arc::new(vm.runtime_template());
            if cacheable {
                store.insert_nested_runtime(
                    key,
                    SharedRuntimePool {
                        baseline: Arc::clone(&baseline),
                        available: Vec::new(),
                    },
                );
            }
            baseline
        };
        drop(store);
        Ok(PreparedRuntimeLease {
            cache: self.clone(),
            key,
            baseline,
            vm: Some(vm),
        })
    }
    fn return_runtime(
        &self,
        key: RuntimeKey,
        baseline: Arc<ivm::RuntimeTemplate>,
        mut vm: ivm::IVM,
    ) {
        if vm.reset_from_runtime_template(&baseline).is_err() {
            return;
        }
        let mut store = self.inner.lock();
        store.stats.runtime_dirty_resets = store.stats.runtime_dirty_resets.saturating_add(1);
        if store.capacity == 0 || !store.entries.contains_key(&key.code_hash) {
            store.nested_runtimes.remove(&key);
            store
                .nested_runtime_order
                .retain(|candidate| *candidate != key);
            return;
        }
        let pool = store
            .nested_runtimes
            .entry(key)
            .or_insert_with(|| SharedRuntimePool {
                baseline,
                available: Vec::new(),
            });
        // One idle runtime per key is sufficient. Concurrent/re-entrant calls
        // may create extra workers, which are discarded as they return.
        if pool.available.is_empty() {
            pool.available.push(vm);
        }
        store.touch_nested_runtime(key);
        store.evict_nested_runtimes();
    }
    /// Return current prepared-artifact cache counters.
    #[must_use]
    pub fn stats(&self) -> PreparedContractCacheStats {
        self.inner.lock().stats
    }
}
impl Default for PreparedContractCache {
    fn default() -> Self {
        Self::with_capacity(iroha_config::parameters::defaults::pipeline::CACHE_SIZE)
    }
}
impl PreparedContractStore {
    fn touch(&mut self, code_hash: Hash) {
        if let Some(position) = self
            .order
            .iter()
            .position(|candidate| *candidate == code_hash)
        {
            self.order.remove(position);
        }
        self.order.push_back(code_hash);
    }
    fn insert(&mut self, code_hash: Hash, contract: Arc<ivm::PreparedContract>) {
        if self.capacity == 0 {
            return;
        }
        self.entries.insert(code_hash, contract);
        self.touch(code_hash);
        while self.entries.len() > self.capacity {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            if self.entries.remove(&evicted).is_some() {
                self.stats.evictions = self.stats.evictions.saturating_add(1);
                self.remove_nested_runtimes_for(evicted);
            }
        }
    }
    fn touch_nested_runtime(&mut self, key: RuntimeKey) {
        if let Some(position) = self
            .nested_runtime_order
            .iter()
            .position(|candidate| *candidate == key)
        {
            self.nested_runtime_order.remove(position);
        }
        self.nested_runtime_order.push_back(key);
    }
    fn insert_nested_runtime(&mut self, key: RuntimeKey, pool: SharedRuntimePool) {
        if self.capacity == 0 {
            return;
        }
        self.nested_runtimes.insert(key, pool);
        self.touch_nested_runtime(key);
        self.evict_nested_runtimes();
    }
    fn evict_nested_runtimes(&mut self) {
        while self.nested_runtimes.len() > self.capacity {
            let Some(evicted) = self.nested_runtime_order.pop_front() else {
                break;
            };
            if self.nested_runtimes.remove(&evicted).is_some() {
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
    }
    fn remove_nested_runtimes_for(&mut self, code_hash: Hash) {
        self.nested_runtimes
            .retain(|key, _| key.code_hash != code_hash);
        self.nested_runtime_order
            .retain(|key| key.code_hash != code_hash);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SummaryKey {
    code_hash: Hash,
}
impl SummaryKey {
    fn new(code_hash: Hash) -> Self {
        Self { code_hash }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RuntimeKey {
    code_hash: Hash,
    stack_limit: u64,
    heap_limit: u64,
}
impl RuntimeKey {
    fn new(code_hash: Hash, stack_limit: u64, heap_limit: u64) -> Self {
        Self {
            code_hash,
            stack_limit,
            heap_limit,
        }
    }
    fn summary_key(&self) -> SummaryKey {
        SummaryKey::new(self.code_hash)
    }
}
fn stack_limit_for_gas(gas_limit: u64) -> u64 {
    IvmConfig::new(gas_limit).stack_limit_for_gas()
}
/// Return whether a syscall is available to a contract-less IVM program.
///
/// The canonical policy lives in `ivm_abi` and is hashed into ABI V1. Core
/// delegates to it at both admission and host dispatch so the two enforcement
/// points cannot drift.
#[must_use]
pub(crate) fn is_generic_syscall_allowed(number: u32) -> bool {
    ivm::syscalls::is_generic_program_syscall_allowed(ivm::SyscallPolicy::AbiV1, number)
}
/// Summary of a compiled IVM program derived during admission.
#[derive(Clone, Debug)]
pub struct ProgramSummary {
    prepared: Arc<ivm::PreparedContract>,
    prepared_cache: PreparedContractCache,
    /// Parsed program metadata.
    pub metadata: ProgramMetadata,
    /// Offset to the start of the decoded instructions (after header + literal prefix).
    pub code_offset: usize,
    /// Length of the program header.
    pub header_len: usize,
    /// Domain-separated hash of the complete deployable artifact.
    pub code_hash: Hash,
    /// ABI hash derived from the declared ABI version.
    pub abi_hash: Hash,
    /// Hash of the encoded metadata header.
    pub meta_hash: Hash,
}
/// Fully validated ABI-bound generic IVM program.
///
/// Generic programs deliberately have no `CNTR` interface, contract identity,
/// entrypoints, or durable-state schema. They are used for low-level IVM
/// executables such as system triggers and state-free low-level programs. The
/// authenticated fixed header still binds them to the exact local ABI.
#[derive(Clone, Debug)]
pub struct GenericProgramSummary {
    program: Arc<[u8]>,
    /// Parsed program metadata.
    pub metadata: ProgramMetadata,
    /// Offset to the first decoded instruction.
    pub code_offset: usize,
    /// Length of the authenticated fixed header.
    pub header_len: usize,
    /// Domain-separated hash of the complete program image.
    pub code_hash: Hash,
    /// ABI hash authenticated by the fixed header.
    pub abi_hash: Hash,
    /// Hash of the canonical encoded metadata header.
    pub meta_hash: Hash,
}
impl GenericProgramSummary {
    /// Return the complete validated program image.
    #[must_use]
    pub fn program(&self) -> &[u8] {
        &self.program
    }
    /// Clone the shared immutable program image without copying its bytes.
    #[must_use]
    pub fn shared_program(&self) -> Arc<[u8]> {
        Arc::clone(&self.program)
    }
}
/// Admission result for either a self-describing contract or a generic IVM
/// program.
#[derive(Clone, Debug)]
pub enum ExecutableProgramSummary {
    /// A deployable, self-describing `CNTR` contract.
    Contract(ProgramSummary),
    /// An ABI-authenticated generic program without contract identity.
    Generic(GenericProgramSummary),
}
impl ExecutableProgramSummary {
    /// Return the parsed metadata shared by both program kinds.
    #[must_use]
    pub fn metadata(&self) -> &ProgramMetadata {
        match self {
            Self::Contract(summary) => &summary.metadata,
            Self::Generic(summary) => &summary.metadata,
        }
    }
    /// Return the instruction offset shared by both program kinds.
    #[must_use]
    pub fn code_offset(&self) -> usize {
        match self {
            Self::Contract(summary) => summary.code_offset,
            Self::Generic(summary) => summary.code_offset,
        }
    }
    /// Return the complete program hash shared by both program kinds.
    #[must_use]
    pub fn code_hash(&self) -> Hash {
        match self {
            Self::Contract(summary) => summary.code_hash,
            Self::Generic(summary) => summary.code_hash,
        }
    }
    /// Return the authenticated ABI hash shared by both program kinds.
    #[must_use]
    pub fn abi_hash(&self) -> Hash {
        match self {
            Self::Contract(summary) => summary.abi_hash,
            Self::Generic(summary) => summary.abi_hash,
        }
    }
}
impl ProgramSummary {
    /// Prepare and summarize one complete deployable contract artifact.
    ///
    /// This is the public construction boundary for callers that need a
    /// self-contained summary without managing an [`IvmCache`]. It validates
    /// and predecodes the artifact and initializes the private prepared-runtime
    /// cache carried by the returned summary.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] when the bytes are not a valid deployable IVM
    /// contract artifact.
    pub fn from_artifact(bytecode: &[u8]) -> Result<Self, ivm::VMError> {
        IvmCache::new().summarize_program(bytecode)
    }
    /// Return the immutable validated contract shared by analysis and runtimes.
    #[must_use]
    pub fn prepared_contract(&self) -> &ivm::PreparedContract {
        &self.prepared
    }
    /// Return the shared bounded cache used for nested contract preparation.
    #[must_use]
    pub fn prepared_contract_cache(&self) -> PreparedContractCache {
        self.prepared_cache.clone()
    }
    /// Check out a warmed runtime backed by the shared prepared-artifact pool.
    ///
    /// The owned lease does not hold the cache mutex while guest code runs.
    /// Dropping it on any success, error, or unwind path restores dirty memory
    /// chunks and returns the VM to the pool.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if a cold runtime cannot load the validated
    /// prepared contract or `heap_limit` lies outside the ABI heap window.
    pub fn checkout_runtime(
        &self,
        gas_limit: u64,
        heap_limit: u64,
    ) -> Result<PreparedRuntimeLease, ivm::VMError> {
        self.prepared_cache
            .checkout_runtime(self.prepared_contract(), gas_limit, heap_limit)
    }
}
struct RuntimePool {
    baseline: Arc<ivm::RuntimeTemplate>,
    available: Vec<ivm::IVM>,
}
struct SharedRuntimePool {
    baseline: Arc<ivm::RuntimeTemplate>,
    available: Vec<ivm::IVM>,
}
impl std::fmt::Debug for SharedRuntimePool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedRuntimePool")
            .field("baseline", &"<runtime-template>")
            .field("available_runtimes", &self.available.len())
            .finish()
    }
}
/// Checked-out nested-call runtime returned to the shared prepared cache on
/// every success, error, and unwind path.
pub struct PreparedRuntimeLease {
    cache: PreparedContractCache,
    key: RuntimeKey,
    baseline: Arc<ivm::RuntimeTemplate>,
    vm: Option<ivm::IVM>,
}
impl Deref for PreparedRuntimeLease {
    type Target = ivm::IVM;
    fn deref(&self) -> &Self::Target {
        self.vm
            .as_ref()
            .expect("prepared runtime lease always owns a VM")
    }
}
impl DerefMut for PreparedRuntimeLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vm
            .as_mut()
            .expect("prepared runtime lease always owns a VM")
    }
}
impl Drop for PreparedRuntimeLease {
    fn drop(&mut self) {
        if let Some(vm) = self.vm.take() {
            self.cache
                .return_runtime(self.key, Arc::clone(&self.baseline), vm);
        }
    }
}
/// Checked-out warmed runtime that automatically returns to its cache.
///
/// Dropping the lease restores only dirty memory chunks and makes the same VM
/// available to the next invocation. This avoids cloning the VM's complete
/// stack, heap, code image, and Merkle tree on cache hits.
pub struct RuntimeLease<'a> {
    cache: &'a mut IvmCache,
    key: RuntimeKey,
    baseline: Arc<ivm::RuntimeTemplate>,
    vm: Option<ivm::IVM>,
}
impl Deref for RuntimeLease<'_> {
    type Target = ivm::IVM;
    fn deref(&self) -> &Self::Target {
        self.vm.as_ref().expect("runtime lease always owns a VM")
    }
}
impl DerefMut for RuntimeLease<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vm.as_mut().expect("runtime lease always owns a VM")
    }
}
impl Drop for RuntimeLease<'_> {
    fn drop(&mut self) {
        if let Some(vm) = self.vm.take() {
            self.cache
                .return_runtime(self.key, Arc::clone(&self.baseline), vm);
        }
    }
}
/// Lightweight cache counters for diagnostics and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Metadata cache hits.
    pub metadata_hits: u64,
    /// Metadata cache misses.
    pub metadata_misses: u64,
    /// Runtime template hits.
    pub runtime_hits: u64,
    /// Runtime template misses.
    pub runtime_misses: u64,
    /// Static analysis cache hits.
    pub analysis_hits: u64,
    /// Static analysis cache misses.
    pub analysis_misses: u64,
    /// Full-artifact hashes computed by the byte-slice convenience path.
    pub artifact_hashes: u64,
    /// Complete preparations performed, each including parse, hash, and predecode.
    pub preparations: u64,
    /// Cold VMs populated from a cached prepared contract.
    pub prepared_loads: u64,
    /// Pristine runtime baselines built for a program/stack configuration.
    pub template_builds: u64,
    /// Warm runtimes restored through dirty-page reset before pooling.
    pub dirty_resets: u64,
    /// Evictions triggered by capacity limits.
    pub evictions: u64,
}
/// Admission-time cache for IVM program summaries and warmed runtimes.
pub struct IvmCache {
    prepared_contracts: PreparedContractCache,
    summaries: BTreeMap<SummaryKey, ProgramSummary>,
    generic_summaries: BTreeMap<SummaryKey, GenericProgramSummary>,
    runtime_templates: BTreeMap<RuntimeKey, RuntimePool>,
    analyses: BTreeMap<SummaryKey, ProgramAnalysis>,
    summary_order: VecDeque<SummaryKey>,
    runtime_order: VecDeque<RuntimeKey>,
    capacity: usize,
    stats: CacheStats,
}
impl Default for IvmCache {
    fn default() -> Self {
        Self::new()
    }
}
impl IvmCache {
    /// Constructor with a default capacity of 64 entries.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(iroha_config::parameters::defaults::pipeline::CACHE_SIZE)
    }
    /// Construct a cache with a specific maximum number of entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_prepared_contract_cache(capacity, PreparedContractCache::with_capacity(capacity))
    }
    /// Construct a worker-local summary/analysis cache backed by a shared
    /// immutable prepared-contract and owned-runtime pool.
    ///
    /// This keeps cheap LRU bookkeeping local while ensuring parallel workers
    /// prepare each content-addressed artifact only once.
    #[must_use]
    pub fn with_prepared_contract_cache(
        capacity: usize,
        prepared_contracts: PreparedContractCache,
    ) -> Self {
        Self {
            prepared_contracts,
            summaries: BTreeMap::new(),
            generic_summaries: BTreeMap::new(),
            runtime_templates: BTreeMap::new(),
            analyses: BTreeMap::new(),
            summary_order: VecDeque::new(),
            runtime_order: VecDeque::new(),
            capacity,
            stats: CacheStats::default(),
        }
    }
    /// Prepare a contract and cache its summary by the complete artifact hash.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if the bytecode is not a valid deployable contract.
    pub fn summarize_program(&mut self, bytecode: &[u8]) -> Result<ProgramSummary, ivm::VMError> {
        let code_hash = ivm::contract_code_hash(bytecode);
        self.stats.artifact_hashes = self.stats.artifact_hashes.saturating_add(1);
        self.summarize_program_with_hash(code_hash, bytecode)
    }
    /// Validate and summarize either a self-describing contract or a generic
    /// ABI-bound IVM program.
    ///
    /// The presence of a canonical `CNTR` section is the only discriminator.
    /// Contract artifacts retain the stronger full artifact verifier and
    /// prepared-contract cache; generic programs are fully loaded once to
    /// validate literals, instructions, control flow, and syscall policy.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] when the header, ABI binding, contract section,
    /// literal table, or instruction stream is invalid.
    pub fn summarize_executable(
        &mut self,
        bytecode: &[u8],
    ) -> Result<ExecutableProgramSummary, ivm::VMError> {
        let parsed = ProgramMetadata::parse(bytecode)?;
        if parsed.contract_interface.is_some() {
            self.summarize_program(bytecode)
                .map(ExecutableProgramSummary::Contract)
        } else {
            self.summarize_generic_program(bytecode)
                .map(ExecutableProgramSummary::Generic)
        }
    }
    /// Validate and summarize an ABI-bound program that has no `CNTR` section.
    ///
    /// # Errors
    /// Returns [`ivm::VMError::InvalidMetadata`] if a contract interface is
    /// present or any generic-program validation fails.
    pub fn summarize_generic_program(
        &mut self,
        bytecode: &[u8],
    ) -> Result<GenericProgramSummary, ivm::VMError> {
        let code_hash = ivm::contract_code_hash(bytecode);
        self.stats.artifact_hashes = self.stats.artifact_hashes.saturating_add(1);
        let key = SummaryKey::new(code_hash);
        if let Some(hit) = self.generic_summaries.get(&key).cloned() {
            if hit.program() != bytecode {
                return Err(ivm::VMError::InvalidMetadata);
            }
            self.stats.metadata_hits = self.stats.metadata_hits.saturating_add(1);
            self.touch_summary(key);
            return Ok(hit);
        }
        let parsed = ProgramMetadata::parse(bytecode)?;
        if parsed.contract_interface.is_some() {
            return Err(ivm::VMError::InvalidMetadata);
        }
        self.finish_generic_program_summary(
            bytecode,
            code_hash,
            parsed.metadata,
            parsed.code_offset,
            parsed.header_len,
        )
    }
    /// Validate and summarize a generic program whose metadata was already parsed by the caller.
    ///
    /// `code_hash` must be the authenticated complete-program hash retained by world state. The
    /// verifier recomputes and compares that hash while loading the program, so callers cannot
    /// substitute parsed fields from another image. This entry point avoids repeating the
    /// metadata parse needed to distinguish a contract artifact from a generic program.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if full loading, control-flow analysis, syscall policy, or the
    /// authenticated hash check fails.
    pub(crate) fn summarize_generic_program_with_parsed_metadata(
        &mut self,
        bytecode: &[u8],
        code_hash: Hash,
        metadata: ProgramMetadata,
        code_offset: usize,
        header_len: usize,
    ) -> Result<GenericProgramSummary, ivm::VMError> {
        let key = SummaryKey::new(code_hash);
        if let Some(hit) = self.generic_summaries.get(&key).cloned() {
            if hit.program() != bytecode {
                return Err(ivm::VMError::InvalidMetadata);
            }
            self.stats.metadata_hits = self.stats.metadata_hits.saturating_add(1);
            self.touch_summary(key);
            return Ok(hit);
        }
        self.finish_generic_program_summary(bytecode, code_hash, metadata, code_offset, header_len)
    }
    fn finish_generic_program_summary(
        &mut self,
        bytecode: &[u8],
        code_hash: Hash,
        metadata: ProgramMetadata,
        code_offset: usize,
        header_len: usize,
    ) -> Result<GenericProgramSummary, ivm::VMError> {
        let key = SummaryKey::new(code_hash);
        // Loading performs the same literal, instruction, control-flow, and
        // syscall validation used at execution. The global immutable predecode
        // cache makes subsequent loads deterministic and inexpensive.
        let mut verifier = ivm::IVM::new(0);
        verifier.set_zk_trace_enabled(false);
        verifier.load_program(bytecode)?;
        if Hash::prehashed(verifier.code_hash()) != code_hash {
            return Err(ivm::VMError::InvalidMetadata);
        }
        let analysis =
            ivm::analysis::analyze_program(bytecode).map_err(|_| ivm::VMError::InvalidMetadata)?;
        if let Some(forbidden) = analysis
            .syscalls
            .iter()
            .find(|usage| !is_generic_syscall_allowed(usage.number))
        {
            return Err(ivm::VMError::GenericSyscallNotAllowed {
                syscall: forbidden.number,
            });
        }
        let summary = GenericProgramSummary {
            program: Arc::from(bytecode),
            code_offset,
            header_len,
            abi_hash: Hash::prehashed(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)),
            meta_hash: Hash::new(metadata.encode()),
            metadata,
            code_hash,
        };
        self.insert_generic_summary(key, summary.clone());
        self.stats.metadata_misses = self.stats.metadata_misses.saturating_add(1);
        self.stats.preparations = self.stats.preparations.saturating_add(1);
        Ok(summary)
    }
    /// Return the prepared summary for a trusted content-addressed artifact.
    ///
    /// Cache hits use only `code_hash` and do not inspect, parse, hash, or
    /// predecode `bytecode`. On a miss, preparation recomputes and verifies the
    /// complete artifact hash before publishing the entry.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if preparation fails or `bytecode` does not
    /// match `code_hash`.
    pub fn summarize_program_with_hash(
        &mut self,
        code_hash: Hash,
        bytecode: &[u8],
    ) -> Result<ProgramSummary, ivm::VMError> {
        if let Some(hit) = self.cached_program_summary(code_hash)? {
            return Ok(hit);
        }
        let key = SummaryKey::new(code_hash);
        let (prepared, prepared_now) = self
            .prepared_contracts
            .get_or_prepare_with_status(code_hash, bytecode)?;
        if prepared_now {
            self.stats.preparations = self.stats.preparations.saturating_add(1);
        }
        let metadata = prepared.metadata().clone();
        let code_offset = prepared.code_offset();
        let header_len = prepared.header_len();
        let meta_hash = Hash::new(metadata.encode());
        // Contract preparation has already compared the authenticated CNTR
        // binding with the local descriptor. Preserve the artifact-carried
        // value here so manifest checks never substitute node-local metadata.
        let abi_hash = Hash::prehashed(prepared.contract_interface().abi_hash);
        let summary = ProgramSummary {
            prepared,
            prepared_cache: self.prepared_contracts.clone(),
            metadata,
            code_offset,
            header_len,
            code_hash,
            abi_hash,
            meta_hash,
        };
        self.insert_summary(key, summary.clone());
        self.stats.metadata_misses = self.stats.metadata_misses.saturating_add(1);
        Ok(summary)
    }
    /// Resolve a locally cached summary by a trusted admitted content address.
    ///
    /// This path takes no byte slice, so callers can check a world-state
    /// binding before copying or borrowing the stored artifact. `Ok(None)`
    /// means the exact bytes must be supplied to
    /// [`Self::summarize_program_with_hash`].
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if the local summary conflicts with the shared
    /// immutable prepared-artifact store.
    pub fn cached_program_summary(
        &mut self,
        code_hash: Hash,
    ) -> Result<Option<ProgramSummary>, ivm::VMError> {
        let key = SummaryKey::new(code_hash);
        let Some(hit) = self.summaries.get(&key).cloned() else {
            return Ok(None);
        };
        self.prepared_contracts.publish(Arc::clone(&hit.prepared))?;
        self.stats.metadata_hits = self.stats.metadata_hits.saturating_add(1);
        self.touch_summary(key);
        Ok(Some(hit))
    }
    /// Resolve a locally cached generic-program summary by its authenticated content address.
    ///
    /// The caller remains responsible for comparing the retained shared image with its
    /// authoritative storage binding. This lookup itself performs no hashing, metadata parsing,
    /// program loading, or byte copying.
    #[must_use]
    pub(crate) fn cached_generic_program_summary(
        &mut self,
        code_hash: Hash,
    ) -> Option<GenericProgramSummary> {
        let key = SummaryKey::new(code_hash);
        let hit = self.generic_summaries.get(&key).cloned()?;
        self.stats.metadata_hits = self.stats.metadata_hits.saturating_add(1);
        self.touch_summary(key);
        Some(hit)
    }
    /// Analyze a program once per cached summary and return a reusable static AMX summary.
    ///
    /// # Errors
    /// Returns [`ProgramAnalysisError`] when metadata parsing or instruction decoding fails.
    pub fn analyze_program(
        &mut self,
        summary: &ProgramSummary,
        _bytecode: &[u8],
    ) -> Result<ProgramAnalysis, ProgramAnalysisError> {
        let key = SummaryKey::new(summary.code_hash);
        if let Some(hit) = self.analyses.get(&key).cloned() {
            self.stats.analysis_hits = self.stats.analysis_hits.saturating_add(1);
            self.touch_summary(key);
            return Ok(hit);
        }
        self.stats.analysis_misses = self.stats.analysis_misses.saturating_add(1);
        let analysis = ivm::analysis::analyze_prepared(summary.prepared_contract());
        if self.capacity != 0 {
            self.analyses.insert(key, analysis.clone());
            self.touch_summary(key);
            self.evict_summaries_if_needed();
        }
        Ok(analysis)
    }
    /// Analyze a validated generic program once per content-addressed summary.
    ///
    /// # Errors
    /// Returns [`ProgramAnalysisError`] if the stored validated image cannot be
    /// decoded consistently.
    pub fn analyze_generic_program(
        &mut self,
        summary: &GenericProgramSummary,
    ) -> Result<ProgramAnalysis, ProgramAnalysisError> {
        let key = SummaryKey::new(summary.code_hash);
        if let Some(hit) = self.analyses.get(&key).cloned() {
            self.stats.analysis_hits = self.stats.analysis_hits.saturating_add(1);
            self.touch_summary(key);
            return Ok(hit);
        }
        self.stats.analysis_misses = self.stats.analysis_misses.saturating_add(1);
        let analysis = ivm::analysis::analyze_program(summary.program())?;
        if self.capacity != 0 {
            self.analyses.insert(key, analysis.clone());
            self.touch_summary(key);
            self.evict_summaries_if_needed();
        }
        Ok(analysis)
    }
    /// Check out a warmed runtime for `summary.code_hash`, loading it if needed.
    ///
    /// The returned lease restores and returns the VM automatically on every
    /// exit path. Callers should attach a fresh host before execution.
    ///
    /// # Errors
    /// Propagates [`ivm::VMError`] when loading the runtime or applying the
    /// governed heap ceiling fails.
    pub fn checkout_runtime<'a>(
        &'a mut self,
        summary: &ProgramSummary,
        _bytecode: &[u8],
        gas_limit: u64,
        heap_limit: u64,
    ) -> Result<RuntimeLease<'a>, ivm::VMError> {
        let (key, baseline, vm) = self.take_runtime(summary, gas_limit, heap_limit)?;
        Ok(RuntimeLease {
            cache: self,
            key,
            baseline,
            vm: Some(vm),
        })
    }
    /// Check out a warmed runtime for a validated generic IVM program.
    ///
    /// The returned lease restores all mutated runtime state before returning
    /// the VM to the bounded pool. Generic programs remain contract-less: no
    /// interface, identity, or entrypoint metadata is synthesized.
    ///
    /// # Errors
    /// Propagates [`ivm::VMError`] if the validated program cannot be loaded
    /// or its governed heap ceiling is outside the ABI address window.
    pub fn checkout_generic_runtime<'a>(
        &'a mut self,
        summary: &GenericProgramSummary,
        gas_limit: u64,
        heap_limit: u64,
    ) -> Result<RuntimeLease<'a>, ivm::VMError> {
        let stack_limit = stack_limit_for_gas(gas_limit);
        let key = RuntimeKey::new(summary.code_hash, stack_limit, heap_limit);
        let cached = self.runtime_templates.get_mut(&key).and_then(|pool| {
            pool.available
                .pop()
                .map(|vm| (Arc::clone(&pool.baseline), vm))
        });
        let (baseline, vm) = if let Some((baseline, mut vm)) = cached {
            self.stats.runtime_hits = self.stats.runtime_hits.saturating_add(1);
            self.touch_runtime(key);
            vm.set_gas_limit(gas_limit);
            (baseline, vm)
        } else {
            self.stats.runtime_misses = self.stats.runtime_misses.saturating_add(1);
            let mut vm = ivm::IVM::new(gas_limit);
            vm.set_zk_trace_enabled(false);
            vm.memory.set_heap_max_limit(heap_limit)?;
            vm.load_program(summary.program())?;
            self.stats.prepared_loads = self.stats.prepared_loads.saturating_add(1);
            vm.set_gas_limit(gas_limit);
            let baseline = if let Some(pool) = self.runtime_templates.get(&key) {
                Arc::clone(&pool.baseline)
            } else {
                self.stats.template_builds = self.stats.template_builds.saturating_add(1);
                Arc::new(vm.runtime_template())
            };
            if !self.runtime_templates.contains_key(&key) {
                self.insert_runtime_pool(
                    key,
                    RuntimePool {
                        baseline: Arc::clone(&baseline),
                        available: Vec::new(),
                    },
                );
            }
            (baseline, vm)
        };
        Ok(RuntimeLease {
            cache: self,
            key,
            baseline,
            vm: Some(vm),
        })
    }
    fn take_runtime(
        &mut self,
        summary: &ProgramSummary,
        gas_limit: u64,
        heap_limit: u64,
    ) -> Result<(RuntimeKey, Arc<ivm::RuntimeTemplate>, ivm::IVM), ivm::VMError> {
        let stack_limit = stack_limit_for_gas(gas_limit);
        let key = RuntimeKey::new(summary.code_hash, stack_limit, heap_limit);
        let cached = self.runtime_templates.get_mut(&key).and_then(|pool| {
            pool.available
                .pop()
                .map(|vm| (Arc::clone(&pool.baseline), vm))
        });
        if let Some((baseline, mut vm)) = cached {
            self.stats.runtime_hits = self.stats.runtime_hits.saturating_add(1);
            self.touch_runtime(key);
            vm.set_gas_limit(gas_limit);
            return Ok((key, baseline, vm));
        }
        self.stats.runtime_misses = self.stats.runtime_misses.saturating_add(1);
        let mut vm = ivm::IVM::new(gas_limit);
        vm.set_zk_trace_enabled(false);
        vm.memory.set_heap_max_limit(heap_limit)?;
        vm.load_prepared(summary.prepared_contract())?;
        self.stats.prepared_loads = self.stats.prepared_loads.saturating_add(1);
        if gas_limit > 0 {
            vm.set_gas_limit(gas_limit);
        }
        let baseline = if let Some(pool) = self.runtime_templates.get(&key) {
            Arc::clone(&pool.baseline)
        } else {
            self.stats.template_builds = self.stats.template_builds.saturating_add(1);
            Arc::new(vm.runtime_template())
        };
        if !self.runtime_templates.contains_key(&key) {
            self.insert_runtime_pool(
                key,
                RuntimePool {
                    baseline: Arc::clone(&baseline),
                    available: Vec::new(),
                },
            );
        }
        Ok((key, baseline, vm))
    }
    /// Return a snapshot of cache counters.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }
    /// Return the prepared-artifact store shared with contract hosts.
    #[must_use]
    pub fn prepared_contract_cache(&self) -> PreparedContractCache {
        self.prepared_contracts.clone()
    }
    fn insert_summary(&mut self, key: SummaryKey, summary: ProgramSummary) {
        if self.capacity == 0 {
            return;
        }
        self.summaries.insert(key, summary);
        self.touch_summary(key);
        self.evict_summaries_if_needed();
    }
    fn insert_generic_summary(&mut self, key: SummaryKey, summary: GenericProgramSummary) {
        if self.capacity == 0 {
            return;
        }
        self.generic_summaries.insert(key, summary);
        self.touch_summary(key);
        self.evict_summaries_if_needed();
    }
    fn insert_runtime_pool(&mut self, key: RuntimeKey, pool: RuntimePool) {
        if self.capacity == 0 {
            return;
        }
        self.runtime_templates.insert(key, pool);
        self.touch_runtime(key);
        self.evict_runtimes_if_needed();
    }
    fn return_runtime(
        &mut self,
        key: RuntimeKey,
        baseline: Arc<ivm::RuntimeTemplate>,
        mut vm: ivm::IVM,
    ) {
        if self.capacity == 0 {
            return;
        }
        if vm.reset_from_runtime_template(&baseline).is_err() {
            return;
        }
        self.stats.dirty_resets = self.stats.dirty_resets.saturating_add(1);
        let pool = self
            .runtime_templates
            .entry(key)
            .or_insert_with(|| RuntimePool {
                baseline,
                available: Vec::new(),
            });
        if pool.available.is_empty() {
            pool.available.push(vm);
        }
        self.touch_runtime(key);
        self.evict_runtimes_if_needed();
    }
    fn touch_summary(&mut self, key: SummaryKey) {
        if self.capacity == 0 {
            return;
        }
        if let Some(pos) = self.summary_order.iter().position(|k| *k == key) {
            self.summary_order.remove(pos);
        }
        self.summary_order.push_back(key);
    }
    fn touch_runtime(&mut self, key: RuntimeKey) {
        if self.capacity == 0 {
            return;
        }
        if let Some(pos) = self.runtime_order.iter().position(|k| *k == key) {
            self.runtime_order.remove(pos);
        }
        self.runtime_order.push_back(key);
        self.touch_summary(key.summary_key());
    }
    fn evict_summaries_if_needed(&mut self) {
        while self.capacity != 0 && self.summary_order.len() > self.capacity {
            if let Some(old) = self.summary_order.pop_front() {
                self.summaries.remove(&old);
                self.generic_summaries.remove(&old);
                self.analyses.remove(&old);
                self.prune_runtime_for_summary(old);
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
    }
    fn evict_runtimes_if_needed(&mut self) {
        while self.capacity != 0 && self.runtime_order.len() > self.capacity {
            if let Some(old) = self.runtime_order.pop_front() {
                self.runtime_templates.remove(&old);
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
    }
    fn prune_runtime_for_summary(&mut self, key: SummaryKey) {
        self.runtime_templates
            .retain(|runtime_key, _| runtime_key.summary_key() != key);
        self.runtime_order
            .retain(|runtime_key| runtime_key.summary_key() != key);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    use ivm::runtime::IvmConfig;
    const HEAP_LIMIT: u64 = ivm::Memory::HEAP_MAX_SIZE;
    /// Assemble a minimal program containing only a HALT instruction.
    fn minimal_program() -> Vec<u8> {
        let mut program = ivm::ProgramMetadata::default().encode();
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "CacheFixture".to_owned(),
            compiler_fingerprint: "iroha-core-cache-tests".to_owned(),
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "inspect".to_owned(),
                kind: EntryPointKind::View,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        program.extend_from_slice(&interface.encode_section());
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }
    fn minimal_generic_program() -> Vec<u8> {
        let mut program = ivm::ProgramMetadata {
            max_cycles: 10_000,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }
    #[test]
    fn executable_summary_distinguishes_contracts_from_generic_programs() {
        let mut cache = IvmCache::with_capacity(4);
        assert!(matches!(
            cache
                .summarize_executable(&minimal_program())
                .expect("contract summary"),
            ExecutableProgramSummary::Contract(_)
        ));
        assert!(matches!(
            cache
                .summarize_executable(&minimal_generic_program())
                .expect("generic summary"),
            ExecutableProgramSummary::Generic(_)
        ));
        assert!(
            cache.summarize_generic_program(&minimal_program()).is_err(),
            "CNTR artifacts must never be downgraded to generic programs"
        );
    }
    #[test]
    fn generic_runtime_is_validated_reset_and_reused() {
        const GAS_LIMIT: u64 = 10_000;
        const GOVERNED_HEAP_LIMIT: u64 = 64;
        let program = minimal_generic_program();
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache
            .summarize_generic_program(&program)
            .expect("generic summary");
        {
            let mut runtime = cache
                .checkout_generic_runtime(&summary, GAS_LIMIT, GOVERNED_HEAP_LIMIT)
                .expect("generic runtime");
            assert_eq!(runtime.memory.heap_limit(), GOVERNED_HEAP_LIMIT);
            assert_eq!(runtime.memory.heap_max_limit(), GOVERNED_HEAP_LIMIT);
            runtime.set_register(3, 77);
            runtime.run().expect("generic HALT program");
        }
        let runtime = cache
            .checkout_generic_runtime(&summary, GAS_LIMIT, GOVERNED_HEAP_LIMIT)
            .expect("reused generic runtime");
        assert_eq!(runtime.register(3), 0);
        assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
        assert_eq!(runtime.memory.heap_max_limit(), GOVERNED_HEAP_LIMIT);
        drop(runtime);
        assert_eq!(cache.stats().runtime_hits, 1);
    }
    #[test]
    fn generic_summary_rejects_disallowed_syscalls_during_preparation() {
        let mut program = ivm::ProgramMetadata {
            max_cycles: 10_000,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_syscallx(0x00ff_ffff).to_le_bytes());
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        assert!(
            IvmCache::new().summarize_generic_program(&program).is_err(),
            "unknown generic-program syscalls must fail before execution"
        );
    }
    #[test]
    fn generic_summary_rejects_contract_only_syscalls_with_stable_reason() {
        for syscall in [
            ivm::syscalls::SYSCALL_GRANT_CONTRACT_ENTRYPOINT,
            ivm::syscalls::SYSCALL_REVOKE_CONTRACT_ENTRYPOINT,
            ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE,
            ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES,
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE,
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
            ivm::syscalls::SYSCALL_STATE_GET,
            ivm::syscalls::SYSCALL_STATE_SET,
            ivm::syscalls::SYSCALL_STATE_DEL,
            ivm::syscalls::SYSCALL_STATE_KEYS,
            ivm::syscalls::SYSCALL_STATE_HAS,
            ivm::syscalls::SYSCALL_STATE_LEN,
            ivm::syscalls::SYSCALL_STATE_COUNT,
            ivm::syscalls::SYSCALL_CALL_CONTRACT,
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS,
            ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_SUBJECT,
            ivm::syscalls::SYSCALL_SYSVAR_ENTRYPOINT,
        ] {
            let mut program = ivm::ProgramMetadata {
                max_cycles: 10_000,
                ..ivm::ProgramMetadata::default()
            }
            .encode();
            program.extend_from_slice(&ivm::encoding::wide::encode_syscallx(syscall).to_le_bytes());
            program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
            let error = IvmCache::new()
                .summarize_generic_program(&program)
                .expect_err("generic syscall profile must reject contract-only calls");
            assert_eq!(
                error,
                ivm::VMError::GenericSyscallNotAllowed { syscall },
                "generic syscall profile must reject 0x{syscall:02x}"
            );
        }
    }
    #[test]
    fn generic_summary_accepts_unconditional_and_context_gated_syscalls() {
        for syscall in [
            ivm::syscalls::SYSCALL_REGISTER_DOMAIN,
            ivm::syscalls::SYSCALL_INT_ADD,
            ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL,
            ivm::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE,
        ] {
            let mut program = ivm::ProgramMetadata {
                max_cycles: 10_000,
                ..ivm::ProgramMetadata::default()
            }
            .encode();
            program.extend_from_slice(&ivm::encoding::wide::encode_syscallx(syscall).to_le_bytes());
            program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
            IvmCache::new()
                .summarize_generic_program(&program)
                .expect("system trigger syscall belongs to the generic V1 profile");
        }
    }
    #[test]
    fn runtime_is_reused_across_transactions() {
        const TEST_REGISTER: usize = 1;
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let mut cache = IvmCache::with_capacity(2);
        // First transaction warms both summary and runtime template.
        let summary = cache.summarize_program(&program).expect("summary");
        let memory_allocation = {
            let mut runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, HEAP_LIMIT)
                .expect("VM should be created");
            let allocation = runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr();
            runtime.set_register(TEST_REGISTER, 42);
            runtime.gas_remaining = 1;
            runtime
                .memory
                .preload_input(0, &[0xA5])
                .expect("mutate input memory");
            allocation
        };
        // Cache stats should reflect misses.
        let stats = cache.stats();
        assert_eq!(stats.metadata_misses, 1);
        assert_eq!(stats.runtime_misses, 1);
        assert_eq!(stats.runtime_hits, 0);
        // Second transaction should reuse the cached template and preserve code load.
        let summary = cache.summarize_program(&program).expect("cached summary");
        let runtime2 = cache
            .checkout_runtime(&summary, &program, GAS_LIMIT, HEAP_LIMIT)
            .expect("VM should be reused");
        assert_eq!(runtime2.register(TEST_REGISTER), 0);
        assert_eq!(
            runtime2.remaining_gas(),
            GAS_LIMIT,
            "warm checkout must replenish the invocation gas budget"
        );
        assert_eq!(
            runtime2
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr(),
            memory_allocation,
            "warm checkout must reuse the same memory allocation"
        );
        assert_eq!(
            runtime2
                .memory
                .load_region(0x0020_0000, 1)
                .expect("input memory"),
            &[0],
            "dirty input chunk must be restored from the baseline"
        );
        drop(runtime2);
        let stats = cache.stats();
        assert_eq!(stats.metadata_hits, 1);
        assert_eq!(stats.runtime_hits, 1);
        assert_eq!(stats.runtime_misses, 1);
    }
    #[test]
    fn runtime_pool_discards_a_vm_with_mismatched_template_geometry() {
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache.summarize_program(&program).expect("summary");
        {
            let mut runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, HEAP_LIMIT)
                .expect("cold runtime");
            runtime.memory = ivm::Memory::new_with_stack_limit(0, ivm::Memory::STACK_ALIGNMENT);
        }
        let after_mismatch = cache.stats();
        assert_eq!(after_mismatch.runtime_misses, 1);
        assert_eq!(after_mismatch.runtime_hits, 0);
        assert_eq!(
            after_mismatch.dirty_resets, 0,
            "a rejected reset must not count or pool the mismatched VM"
        );
        {
            let runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, HEAP_LIMIT)
                .expect("replacement runtime");
            assert_ne!(runtime.memory.stack_limit(), ivm::Memory::STACK_ALIGNMENT);
        }
        let after_replacement = cache.stats();
        assert_eq!(after_replacement.runtime_misses, 2);
        assert_eq!(after_replacement.runtime_hits, 0);
        assert_eq!(after_replacement.dirty_resets, 1);
    }
    #[test]
    fn runtime_pool_never_reuses_stale_heap_authority() {
        const GAS_LIMIT: u64 = 10_000;
        const SMALL_HEAP_LIMIT: u64 = 64;
        const LARGE_HEAP_LIMIT: u64 = 128;
        let program = minimal_program();
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache.summarize_program(&program).expect("summary");
        {
            let runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, SMALL_HEAP_LIMIT)
                .expect("small governed runtime");
            assert_eq!(runtime.memory.heap_max_limit(), SMALL_HEAP_LIMIT);
        }
        {
            let runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, LARGE_HEAP_LIMIT)
                .expect("large governed runtime");
            assert_eq!(runtime.memory.heap_max_limit(), LARGE_HEAP_LIMIT);
        }
        let after_distinct_limits = cache.stats();
        assert_eq!(after_distinct_limits.runtime_misses, 2);
        assert_eq!(after_distinct_limits.runtime_hits, 0);
        let runtime = cache
            .checkout_runtime(&summary, &program, GAS_LIMIT, SMALL_HEAP_LIMIT)
            .expect("warm small governed runtime");
        assert_eq!(runtime.memory.heap_max_limit(), SMALL_HEAP_LIMIT);
        drop(runtime);
        assert_eq!(cache.stats().runtime_hits, 1);
    }
    #[test]
    fn public_program_summary_constructor_initializes_prepared_runtime_state() {
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let summary = ProgramSummary::from_artifact(&program).expect("public summary constructor");
        assert_eq!(summary.code_hash, ivm::contract_code_hash(&program));
        assert_eq!(summary.prepared_contract().code_hash(), summary.code_hash);
        assert_eq!(summary.prepared_contract().artifact(), program.as_slice());
        let runtime = summary
            .checkout_runtime(GAS_LIMIT, HEAP_LIMIT)
            .expect("summary owns initialized prepared runtime state");
        assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
    }
    #[test]
    fn content_addressed_hit_skips_repreparation_and_reuses_dirty_reset_runtime() {
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let code_hash = ivm::contract_code_hash(&program);
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache
            .summarize_program_with_hash(code_hash, &program)
            .expect("first preparation");
        let memory_allocation = {
            let mut runtime = cache
                .checkout_runtime(&summary, &program, GAS_LIMIT, HEAP_LIMIT)
                .expect("first runtime");
            let allocation = runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr();
            runtime.set_register(7, 99);
            runtime
                .memory
                .preload_input(0, &[0xA5])
                .expect("dirty input page");
            allocation
        };
        let after_first = cache.stats();
        assert_eq!(after_first.artifact_hashes, 0);
        assert_eq!(after_first.preparations, 1);
        assert_eq!(after_first.prepared_loads, 1);
        assert_eq!(after_first.template_builds, 1);
        assert_eq!(after_first.dirty_resets, 1);
        // A content-addressed hit does not even inspect the byte slice. The
        // canonical bytes and all decoded state come from ProgramSummary.
        let summary = cache
            .summarize_program_with_hash(code_hash, &[])
            .expect("summary cache hit");
        {
            let runtime = cache
                .checkout_runtime(&summary, &[], GAS_LIMIT, HEAP_LIMIT)
                .expect("warm runtime");
            assert_eq!(runtime.register(7), 0);
            assert_eq!(
                runtime
                    .memory
                    .load_region(0, 1)
                    .expect("code memory")
                    .as_ptr(),
                memory_allocation
            );
            assert_eq!(
                runtime
                    .memory
                    .load_region(0x0020_0000, 1)
                    .expect("input memory"),
                &[0]
            );
        }
        let after_second = cache.stats();
        assert_eq!(after_second.metadata_hits, after_first.metadata_hits + 1);
        assert_eq!(after_second.runtime_hits, after_first.runtime_hits + 1);
        assert_eq!(after_second.artifact_hashes, after_first.artifact_hashes);
        assert_eq!(after_second.preparations, after_first.preparations);
        assert_eq!(after_second.prepared_loads, after_first.prepared_loads);
        assert_eq!(after_second.template_builds, after_first.template_builds);
        assert_eq!(after_second.dirty_resets, after_first.dirty_resets + 1);
    }
    #[test]
    fn second_nested_resolution_reuses_shared_prepared_artifact() {
        let outer_program = minimal_program();
        let mut outer_cache = IvmCache::with_capacity(2);
        let outer_summary = outer_cache
            .summarize_program(&outer_program)
            .expect("outer contract summary");
        let nested_cache = outer_summary.prepared_contract_cache();
        let before_nested = nested_cache.stats();
        let mut nested_program = minimal_program();
        nested_program[8..16].copy_from_slice(&17u64.to_le_bytes());
        let nested_hash = ivm::contract_code_hash(&nested_program);
        let first = nested_cache
            .get_or_prepare(nested_hash, &nested_program)
            .expect("first nested resolution");
        let after_first = nested_cache.stats();
        assert_eq!(after_first.misses, before_nested.misses + 1);
        assert_eq!(after_first.preparations, before_nested.preparations + 1);
        // A second nested call resolves solely by the trusted content address:
        // no byte inspection, artifact hash, parse, validation, or predecode.
        let second = nested_cache
            .get(nested_hash)
            .expect("second nested resolution");
        assert!(Arc::ptr_eq(&first, &second));
        let after_second = nested_cache.stats();
        assert_eq!(after_second.hits, after_first.hits + 1);
        assert_eq!(after_second.misses, after_first.misses);
        assert_eq!(after_second.preparations, after_first.preparations);
    }
    #[test]
    fn worker_caches_share_one_preparation_and_owned_runtime_pool() {
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let code_hash = ivm::contract_code_hash(&program);
        let shared = PreparedContractCache::with_capacity(4);
        let mut first_worker = IvmCache::with_prepared_contract_cache(4, shared.clone());
        let mut second_worker = IvmCache::with_prepared_contract_cache(4, shared.clone());
        let first = first_worker
            .summarize_program_with_hash(code_hash, &program)
            .expect("first worker preparation");
        let allocation = {
            let runtime = first
                .checkout_runtime(GAS_LIMIT, HEAP_LIMIT)
                .expect("first runtime");
            runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr()
        };
        // The second worker has no local summary. Supplying no bytes proves it
        // resolves through the shared content-addressed prepared store.
        let second = second_worker
            .summarize_program_with_hash(code_hash, &[])
            .expect("shared prepared hit");
        let runtime = second
            .checkout_runtime(GAS_LIMIT, HEAP_LIMIT)
            .expect("warm runtime");
        assert_eq!(
            runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr(),
            allocation
        );
        let stats = shared.stats();
        assert_eq!(stats.preparations, 1);
        assert_eq!(stats.runtime_prepared_loads, 1);
        assert_eq!(stats.runtime_template_builds, 1);
        assert_eq!(stats.runtime_hits, 1);
        assert_eq!(first_worker.stats().preparations, 1);
        assert_eq!(second_worker.stats().preparations, 0);
    }
    #[test]
    fn concurrent_workers_singleflight_contract_preparation() {
        const WORKERS: usize = 8;
        let program = Arc::new(minimal_program());
        let code_hash = ivm::contract_code_hash(program.as_slice());
        let cache = PreparedContractCache::with_capacity(4);
        let barrier = Arc::new(std::sync::Barrier::new(WORKERS));
        let handles = (0..WORKERS)
            .map(|_| {
                let program = Arc::clone(&program);
                let cache = cache.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    cache
                        .get_or_prepare(code_hash, program.as_slice())
                        .expect("singleflight preparation")
                })
            })
            .collect::<Vec<_>>();
        let prepared = handles
            .into_iter()
            .map(|handle| handle.join().expect("worker must not panic"))
            .collect::<Vec<_>>();
        let artifact = prepared[0].artifact().as_ptr();
        assert!(
            prepared
                .iter()
                .all(|contract| contract.artifact().as_ptr() == artifact)
        );
        let stats = cache.stats();
        assert_eq!(stats.preparations, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, (WORKERS - 1) as u64);
    }
    #[test]
    fn nested_runtime_pool_reuses_allocation_and_dirty_resets_memory() {
        const GAS_LIMIT: u64 = 10_000;
        const GOVERNED_HEAP_LIMIT: u64 = 96;
        let program = minimal_program();
        let code_hash = ivm::contract_code_hash(&program);
        let cache = PreparedContractCache::with_capacity(2);
        let prepared = cache
            .get_or_prepare(code_hash, &program)
            .expect("prepare nested contract");
        let memory_allocation = {
            let mut runtime = cache
                .checkout_runtime(prepared.as_ref(), GAS_LIMIT, GOVERNED_HEAP_LIMIT)
                .expect("cold nested runtime");
            assert_eq!(runtime.memory.heap_max_limit(), GOVERNED_HEAP_LIMIT);
            let allocation = runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr();
            runtime.set_register(7, 99);
            runtime
                .memory
                .preload_input(0, &[0xA5])
                .expect("dirty nested input page");
            allocation
        };
        let after_first = cache.stats();
        assert_eq!(after_first.runtime_misses, 1);
        assert_eq!(after_first.runtime_prepared_loads, 1);
        assert_eq!(after_first.runtime_template_builds, 1);
        assert_eq!(after_first.runtime_dirty_resets, 1);
        {
            let runtime = cache
                .checkout_runtime(prepared.as_ref(), GAS_LIMIT, GOVERNED_HEAP_LIMIT)
                .expect("warm nested runtime");
            assert_eq!(runtime.register(7), 0);
            assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
            assert_eq!(runtime.memory.heap_max_limit(), GOVERNED_HEAP_LIMIT);
            assert_eq!(
                runtime
                    .memory
                    .load_region(0, 1)
                    .expect("code memory")
                    .as_ptr(),
                memory_allocation,
                "warm nested invocation must reuse the VM allocation"
            );
            assert_eq!(
                runtime
                    .memory
                    .load_region(0x0020_0000, 1)
                    .expect("input memory"),
                &[0],
                "dirty nested input must be restored from the baseline"
            );
        }
        let after_second = cache.stats();
        assert_eq!(after_second.runtime_hits, after_first.runtime_hits + 1);
        assert_eq!(
            after_second.runtime_prepared_loads,
            after_first.runtime_prepared_loads
        );
        assert_eq!(
            after_second.runtime_template_builds,
            after_first.runtime_template_builds
        );
        assert_eq!(
            after_second.runtime_dirty_resets,
            after_first.runtime_dirty_resets + 1
        );
    }
    #[test]
    fn program_summary_owned_lease_reuses_runtime_on_early_return() {
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache.summarize_program(&program).expect("summary");
        assert_eq!(
            summary.prepared_contract().manifest().code_hash,
            Some(summary.code_hash)
        );
        let prepared_cache = summary.prepared_contract_cache();
        fn dirty_then_return(summary: &ProgramSummary) -> Result<(), *const u8> {
            let mut runtime = summary
                .checkout_runtime(GAS_LIMIT, HEAP_LIMIT)
                .expect("prepared runtime");
            let allocation = runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr();
            runtime.set_register(7, 99);
            runtime
                .memory
                .preload_input(0, &[0xA5])
                .expect("dirty input page");
            Err(allocation)
        }
        let allocation = dirty_then_return(&summary).expect_err("early return");
        let after_error = prepared_cache.stats();
        assert_eq!(after_error.runtime_dirty_resets, 1);
        let runtime = summary
            .checkout_runtime(GAS_LIMIT, HEAP_LIMIT)
            .expect("warm runtime");
        assert_eq!(runtime.register(7), 0);
        assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
        assert_eq!(
            runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr(),
            allocation
        );
        assert_eq!(prepared_cache.stats().runtime_hits, 1);
    }
    #[test]
    fn prepared_store_evicts_lru_and_rejects_hash_mismatches() {
        let mut first_program = minimal_program();
        first_program[8..16].copy_from_slice(&23u64.to_le_bytes());
        let first_hash = ivm::contract_code_hash(&first_program);
        let mut second_program = minimal_program();
        second_program[8..16].copy_from_slice(&29u64.to_le_bytes());
        let second_hash = ivm::contract_code_hash(&second_program);
        let cache = PreparedContractCache::with_capacity(1);
        assert!(matches!(
            cache.get_or_prepare(first_hash, &second_program),
            Err(ivm::VMError::InvalidMetadata)
        ));
        cache
            .get_or_prepare(first_hash, &first_program)
            .expect("first valid artifact");
        cache
            .get_or_prepare(second_hash, &second_program)
            .expect("second valid artifact");
        let after_eviction = cache.stats();
        assert_eq!(after_eviction.evictions, 1);
        // Once evicted, a hash-only lookup cannot silently reuse a stale or
        // different artifact. The caller must supply the exact bytes again.
        assert!(matches!(
            cache.get_or_prepare(first_hash, &[]),
            Err(ivm::VMError::InvalidMetadata)
        ));
        let final_stats = cache.stats();
        assert_eq!(final_stats.misses, after_eviction.misses + 1);
        assert_eq!(final_stats.preparations, after_eviction.preparations);
    }
    #[test]
    fn analysis_is_reused_across_transactions() {
        let program = minimal_program();
        let mut cache = IvmCache::with_capacity(2);
        let summary = cache.summarize_program(&program).expect("summary");
        let first = cache
            .analyze_program(&summary, &program)
            .expect("first analysis");
        let second = cache
            .analyze_program(&summary, &program)
            .expect("second analysis");
        assert_eq!(first.instruction_count, second.instruction_count);
        assert_eq!(first.metadata.max_cycles, second.metadata.max_cycles);
        let stats = cache.stats();
        assert_eq!(stats.analysis_misses, 1);
        assert_eq!(stats.analysis_hits, 1);
    }
    #[test]
    fn metadata_cache_distinguishes_header_changes() {
        let mut cache = IvmCache::with_capacity(4);
        // Same body but different metadata (max_cycles) must not share cache entries.
        let mut program = minimal_program();
        program[8..16].copy_from_slice(&1u64.to_le_bytes());
        let summary1 = cache.summarize_program(&program).expect("first summary");
        let mut program2 = program.clone();
        program2[8..16].copy_from_slice(&2u64.to_le_bytes());
        let summary2 = cache.summarize_program(&program2).expect("second summary");
        assert_ne!(summary1.meta_hash, summary2.meta_hash);
        let stats = cache.stats();
        assert_eq!(stats.metadata_hits, 0);
        assert_eq!(stats.metadata_misses, 2);
        {
            let vm1 = cache
                .checkout_runtime(&summary1, &program, 1_000, HEAP_LIMIT)
                .expect("runtime for first variant");
            assert_eq!(vm1.metadata().max_cycles, 1);
        }
        let vm2 = cache
            .checkout_runtime(&summary2, &program2, 1_000, HEAP_LIMIT)
            .expect("runtime for second variant");
        assert_eq!(vm2.metadata().max_cycles, 2);
    }
    #[test]
    fn runtime_stack_limit_tracks_gas_limit() {
        let mut cache = IvmCache::with_capacity(1);
        let mut program = minimal_program();
        program[8..16].copy_from_slice(&14u64.to_le_bytes());
        program[16] = 1; // abi_version
        let summary = cache.summarize_program(&program).expect("summary");
        let gas_limit = 100_000;
        let vm = cache
            .checkout_runtime(&summary, &program, gas_limit, HEAP_LIMIT)
            .expect("runtime");
        let expected = IvmConfig::new(gas_limit).stack_limit_for_gas();
        assert!(
            expected > 64 * 1024,
            "expected stack limit to exceed 64KiB; got {expected}"
        );
        assert_eq!(vm.memory.stack_limit(), expected);
    }
    #[test]
    fn eviction_prunes_runtimes_for_evicted_summary() {
        let mut cache = IvmCache::with_capacity(1);
        let gas_limit = 50_000;
        let mut program1 = minimal_program();
        program1[8..16].copy_from_slice(&1u64.to_le_bytes());
        let summary1 = cache.summarize_program(&program1).expect("summary1");
        {
            let _runtime = cache
                .checkout_runtime(&summary1, &program1, gas_limit, HEAP_LIMIT)
                .expect("runtime1");
        }
        let mut program2 = minimal_program();
        program2[8..16].copy_from_slice(&2u64.to_le_bytes());
        let summary2 = cache.summarize_program(&program2).expect("summary2");
        let summary1_key = SummaryKey::new(summary1.code_hash);
        let summary2_key = SummaryKey::new(summary2.code_hash);
        assert!(!cache.summaries.contains_key(&summary1_key));
        assert!(cache.summaries.contains_key(&summary2_key));
        let runtime1_key = RuntimeKey::new(
            summary1.code_hash,
            stack_limit_for_gas(gas_limit),
            HEAP_LIMIT,
        );
        assert!(!cache.runtime_templates.contains_key(&runtime1_key));
        {
            let _runtime = cache
                .checkout_runtime(&summary2, &program2, gas_limit, HEAP_LIMIT)
                .expect("runtime2");
        }
        let runtime2_key = RuntimeKey::new(
            summary2.code_hash,
            stack_limit_for_gas(gas_limit),
            HEAP_LIMIT,
        );
        assert!(cache.runtime_templates.contains_key(&runtime2_key));
    }
}
