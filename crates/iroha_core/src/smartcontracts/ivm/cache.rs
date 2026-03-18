use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use iroha_crypto::Hash;
use ivm::runtime::IvmConfig;
use ivm::{ProgramMetadata, SyscallPolicy};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SummaryKey {
    code_hash: Hash,
    meta_hash: Hash,
}

impl SummaryKey {
    fn new(code_hash: Hash, meta_hash: Hash) -> Self {
        Self {
            code_hash,
            meta_hash,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct RuntimeKey {
    code_hash: Hash,
    meta_hash: Hash,
    stack_limit: u64,
}

impl RuntimeKey {
    fn new(code_hash: Hash, meta_hash: Hash, stack_limit: u64) -> Self {
        Self {
            code_hash,
            meta_hash,
            stack_limit,
        }
    }

    fn summary_key(&self) -> SummaryKey {
        SummaryKey::new(self.code_hash, self.meta_hash)
    }
}

fn stack_limit_for_gas(gas_limit: u64) -> u64 {
    IvmConfig::new(gas_limit).stack_limit_for_gas()
}

/// Summary of a compiled IVM program derived during admission.
#[derive(Clone, Debug)]
pub struct ProgramSummary {
    /// Parsed program metadata.
    pub metadata: ProgramMetadata,
    /// Offset to the start of the decoded instructions (after header + literal prefix).
    pub code_offset: usize,
    /// Length of the program header.
    pub header_len: usize,
    /// Code hash computed over the program body (bytes after the header).
    pub code_hash: Hash,
    /// ABI hash derived from the declared ABI version.
    pub abi_hash: Hash,
    /// Hash of the encoded metadata header.
    pub meta_hash: Hash,
}

/// Runtime wrapper carrying the cached program summary alongside the VM instance.
#[derive(Clone)]
pub struct CachedRuntime {
    /// Cached program metadata and derived hashes.
    pub summary: ProgramSummary,
    /// Runtime prepared for execution (host to be attached by the caller).
    pub vm: ivm::IVM,
    /// Original bytecode used to rebuild a clean template when returning to the cache.
    bytecode: Arc<Vec<u8>>,
    /// Gas limit used to size the guest stack for this runtime template.
    stack_gas_limit: u64,
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
    /// Evictions triggered by capacity limits.
    pub evictions: u64,
}

/// Admission-time cache for IVM program summaries and warmed runtimes.
pub struct IvmCache {
    summaries: BTreeMap<SummaryKey, ProgramSummary>,
    runtime_templates: BTreeMap<RuntimeKey, ivm::IVM>,
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
        Self {
            summaries: BTreeMap::new(),
            runtime_templates: BTreeMap::new(),
            summary_order: VecDeque::new(),
            runtime_order: VecDeque::new(),
            capacity,
            stats: CacheStats::default(),
        }
    }

    /// Parse program metadata and compute derived hashes, caching the result by `code_hash` + `meta_hash`.
    ///
    /// # Errors
    /// Returns [`ivm::VMError`] if the bytecode cannot be parsed.
    pub fn summarize_program(&mut self, bytecode: &[u8]) -> Result<ProgramSummary, ivm::VMError> {
        let parsed = ProgramMetadata::parse(bytecode)?;
        if parsed.header_len > bytecode.len() {
            return Err(ivm::VMError::InvalidMetadata);
        }
        let body = &bytecode[parsed.header_len..];
        let code_hash = Hash::new(body);
        let meta_hash = Hash::new(parsed.metadata.encode());
        let key = SummaryKey::new(code_hash, meta_hash);
        if let Some(hit) = self.summaries.get(&key).cloned() {
            self.stats.metadata_hits = self.stats.metadata_hits.saturating_add(1);
            self.touch_summary(key);
            return Ok(hit);
        }

        // The first release has a single canonical ABI hash; admission rejects
        // non-v1 headers after summary extraction.
        let abi_hash = Hash::prehashed(ivm::syscalls::compute_abi_hash(SyscallPolicy::AbiV1));

        let summary = ProgramSummary {
            metadata: parsed.metadata,
            code_offset: parsed.code_offset,
            header_len: parsed.header_len,
            code_hash,
            abi_hash,
            meta_hash,
        };
        self.insert_summary(key, summary.clone());
        self.stats.metadata_misses = self.stats.metadata_misses.saturating_add(1);
        Ok(summary)
    }

    /// Obtain a cloned runtime template for `summary.code_hash`, warming the cache if needed.
    ///
    /// Templates are stored without a host; callers should attach a fresh host before execution.
    ///
    /// # Errors
    /// Propagates [`ivm::VMError`] when loading the runtime fails.
    pub fn clone_runtime(
        &mut self,
        summary: &ProgramSummary,
        bytecode: &[u8],
        gas_limit: u64,
    ) -> Result<ivm::IVM, ivm::VMError> {
        let stack_limit = stack_limit_for_gas(gas_limit);
        let key = RuntimeKey::new(summary.code_hash, summary.meta_hash, stack_limit);
        if let Some(hit) = self.runtime_templates.get(&key).cloned() {
            self.stats.runtime_hits = self.stats.runtime_hits.saturating_add(1);
            self.touch_runtime(key);
            return Ok(hit);
        }

        self.stats.runtime_misses = self.stats.runtime_misses.saturating_add(1);
        let mut vm = ivm::IVM::new(gas_limit);
        vm.load_program(bytecode)?;
        if gas_limit > 0 {
            vm.set_gas_limit(gas_limit);
        }
        self.insert_runtime(key, vm.clone());
        Ok(vm)
    }

    /// Borrow a warmed runtime paired with its summary, producing a fresh instance when needed.
    ///
    /// # Errors
    /// Returns a [`ivm::VMError`] when parsing or runtime preparation fails.
    pub fn take_or_create_cached_runtime(
        &mut self,
        bytecode: &[u8],
        gas_limit: u64,
    ) -> Result<CachedRuntime, ivm::VMError> {
        let summary = self.summarize_program(bytecode)?;
        let vm = self.clone_runtime(&summary, bytecode, gas_limit)?;
        Ok(CachedRuntime {
            summary,
            vm,
            bytecode: Arc::new(bytecode.to_vec()),
            stack_gas_limit: gas_limit,
        })
    }

    /// Reset and store a runtime template for reuse.
    pub fn put_cached_runtime(&mut self, runtime: &CachedRuntime) {
        if self.capacity == 0 {
            return;
        }
        // Rebuild a fresh template to drop any mutated state (registers, memory bump, logs).
        let stack_gas_limit = runtime.stack_gas_limit;
        let stack_limit = stack_limit_for_gas(stack_gas_limit);
        let summary_key = SummaryKey::new(runtime.summary.code_hash, runtime.summary.meta_hash);
        if self.summaries.contains_key(&summary_key) {
            self.touch_summary(summary_key);
        } else {
            self.insert_summary(summary_key, runtime.summary.clone());
        }
        let mut vm = ivm::IVM::new(stack_gas_limit);
        vm.set_host(ivm::host::DefaultHost::default());
        let key = RuntimeKey::new(
            runtime.summary.code_hash,
            runtime.summary.meta_hash,
            stack_limit,
        );
        if vm.load_program(&runtime.bytecode).is_ok() {
            if stack_gas_limit > 0 {
                vm.set_gas_limit(stack_gas_limit);
            }
            self.insert_runtime(key, vm);
        }
    }

    /// Return a snapshot of cache counters.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    fn insert_summary(&mut self, key: SummaryKey, summary: ProgramSummary) {
        if self.capacity == 0 {
            return;
        }
        self.summaries.insert(key, summary);
        self.touch_summary(key);
        self.evict_summaries_if_needed();
    }

    fn insert_runtime(&mut self, key: RuntimeKey, vm: ivm::IVM) {
        if self.capacity == 0 {
            return;
        }
        self.runtime_templates.insert(key, vm);
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
    use ivm::runtime::IvmConfig;

    use super::*;

    /// Assemble a minimal program containing only a HALT instruction.
    fn minimal_program() -> Vec<u8> {
        fn assemble(code: &[u8]) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(b"IVM\0");
            v.push(1); // version major
            v.push(0); // version minor
            v.push(0); // mode flags
            v.push(4); // default vector length
            v.extend_from_slice(&0u64.to_le_bytes()); // max_cycles = 0 (unspecified)
            v.push(0); // abi_version
            v.extend_from_slice(code);
            v
        }

        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        assemble(&code)
    }

    #[test]
    fn runtime_is_reused_across_transactions() {
        const TEST_REGISTER: usize = 1;
        const GAS_LIMIT: u64 = 10_000;
        let program = minimal_program();

        let mut cache = IvmCache::with_capacity(2);

        // First transaction warms both summary and runtime template.
        let mut runtime = cache
            .take_or_create_cached_runtime(&program, GAS_LIMIT)
            .expect("VM should be created");
        runtime.vm.set_register(TEST_REGISTER, 42);
        cache.put_cached_runtime(&runtime);

        // Cache stats should reflect misses.
        let stats = cache.stats();
        assert_eq!(stats.metadata_misses, 1);
        assert_eq!(stats.runtime_misses, 1);
        assert_eq!(stats.runtime_hits, 0);

        // Second transaction should reuse the cached template and preserve code load.
        let runtime2 = cache
            .take_or_create_cached_runtime(&program, GAS_LIMIT)
            .expect("VM should be reused");
        assert_eq!(runtime2.vm.register(TEST_REGISTER), 0); // fresh template clone
        cache.put_cached_runtime(&runtime2);
        let stats = cache.stats();
        assert_eq!(stats.metadata_hits, 1);
        assert_eq!(stats.runtime_hits, 1);
        assert_eq!(stats.runtime_misses, 1);
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

        let vm1 = cache
            .clone_runtime(&summary1, &program, 1_000)
            .expect("runtime for first variant");
        let vm2 = cache
            .clone_runtime(&summary2, &program2, 1_000)
            .expect("runtime for second variant");

        assert_eq!(vm1.metadata().max_cycles, 1);
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
            .clone_runtime(&summary, &program, gas_limit)
            .expect("runtime");

        let expected = IvmConfig::new(gas_limit).stack_limit_for_gas();
        assert!(
            expected > 64 * 1024,
            "expected stack limit to exceed 64KiB; got {expected}"
        );
        assert_eq!(vm.memory.stack_limit(), expected);
    }

    #[test]
    fn cached_runtime_tracks_stack_gas_limit() {
        let mut cache = IvmCache::with_capacity(2);
        let mut program = minimal_program();
        program[8..16].copy_from_slice(&9u64.to_le_bytes());
        let gas_limit = 50_000;

        let runtime = cache
            .take_or_create_cached_runtime(&program, gas_limit)
            .expect("runtime");
        assert_eq!(runtime.stack_gas_limit, gas_limit);
    }

    #[test]
    fn eviction_prunes_runtimes_for_evicted_summary() {
        let mut cache = IvmCache::with_capacity(1);
        let gas_limit = 50_000;

        let mut program1 = minimal_program();
        program1[8..16].copy_from_slice(&1u64.to_le_bytes());
        let summary1 = cache.summarize_program(&program1).expect("summary1");
        let _ = cache
            .clone_runtime(&summary1, &program1, gas_limit)
            .expect("runtime1");

        let mut program2 = minimal_program();
        program2[8..16].copy_from_slice(&2u64.to_le_bytes());
        let summary2 = cache.summarize_program(&program2).expect("summary2");

        let summary1_key = SummaryKey::new(summary1.code_hash, summary1.meta_hash);
        let summary2_key = SummaryKey::new(summary2.code_hash, summary2.meta_hash);
        assert!(!cache.summaries.contains_key(&summary1_key));
        assert!(cache.summaries.contains_key(&summary2_key));

        let runtime1_key = RuntimeKey::new(
            summary1.code_hash,
            summary1.meta_hash,
            stack_limit_for_gas(gas_limit),
        );
        assert!(!cache.runtime_templates.contains_key(&runtime1_key));

        let _ = cache
            .clone_runtime(&summary2, &program2, gas_limit)
            .expect("runtime2");
        let runtime2_key = RuntimeKey::new(
            summary2.code_hash,
            summary2.meta_hash,
            stack_limit_for_gas(gas_limit),
        );
        assert!(cache.runtime_templates.contains_key(&runtime2_key));
    }
}
