//! Core IVM host adapter.
//!
//! This host is responsible for handling IVM `SCALL`s. It MUST NOT mutate the
//! world state directly. Instead, it translates stateful calls into built‑in
//! ISIs and queues them so that the caller can execute them via the standard
//! executor. This ensures permissions, invariants, and events are enforced
//! uniformly and prevents direct world state mutations from the VM.
//!
//! Helper syscalls that do not touch WSV are forwarded to the IVM default host.

use std::{
    any::Any,
    collections::{BTreeMap, VecDeque},
    io::Cursor,
    mem,
    num::{NonZeroU64, NonZeroUsize},
    str::FromStr,
    sync::Arc,
};

use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use iroha_data_model::{
    DataSpaceId, ValidationFail,
    errors::{AmxStage, AmxTimeout, CanonicalErrorKind},
    isi::{
        Burn, BurnBox, InstructionBox, Mint, MintBox, Register, RegisterBox, SetKeyValue,
        SetKeyValueBox, SetParameter, Transfer, TransferAssetBatch, TransferAssetBatchEntry,
        TransferBox, Unregister, UnregisterBox, smart_contract_code as scode, zk as DMZk,
    },
    nexus::{
        AxtBinding, AxtDescriptor as ModelAxtDescriptor, AxtEnvelopeRecord, AxtHandleFragment,
        AxtHandleReplayKey, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, AxtProofFragment,
        AxtRejectContext, AxtRejectReason, AxtReplayRecord, AxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, ProofBlob as ModelProofBlob,
        TouchManifest as ModelTouchManifest, proof_matches_manifest,
    },
    prelude::{AccountId, *},
    proof::{VerifyingKeyId, VerifyingKeyRecord},
    query::parameters::ForwardCursor,
    zk::BackendTag,
};
#[cfg(test)]
use ivm::VMError;
use ivm::{
    self, CoreHost as IvmCodecHost, IVM, PointerType,
    analysis::{self, AmxLimits, ProgramAnalysis},
    axt::{self, AssetHandle, ProofBlob, RemoteSpendIntent, TouchManifest},
    host::IVMHost,
    is_type_allowed_for_policy,
};
use mv::storage::StorageReadOnly;
use norito::{codec::Decode as NoritoDecode, decode_from_bytes, streaming::CapabilityFlags};
use sha2::{Digest, Sha256};

use crate::state::{StateReadOnly, StateTransaction, WorldReadOnly, current_axt_slot_from_block};
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;

const AXT_PROOF_CACHE_HIT: &str = "hit";
const AXT_PROOF_CACHE_MISS: &str = "miss";
const AXT_PROOF_CACHE_EXPIRED: &str = "expired";
const AXT_PROOF_CACHE_CLEARED: &str = "cleared";
const AXT_PROOF_CACHE_REJECT: &str = "reject";

/// Origin of an AXT policy snapshot when hydrating the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxtPolicyCacheEvent {
    /// Snapshot sourced from the cached WSV map.
    CacheHit,
    /// Snapshot derived from the Space Directory (cache cold).
    CacheMiss,
}

impl AxtPolicyCacheEvent {
    /// Human-readable tag for telemetry/metrics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CacheHit => "cache_hit",
            Self::CacheMiss => "cache_miss",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct CachedProofEntry {
    digest: [u8; 32],
    /// Expiry slot after applying the configured clock-skew allowance.
    expiry_slot: Option<u64>,
    verified_slot: u64,
    manifest_root: Option<[u8; 32]>,
    valid: bool,
    status: &'static str,
}

/// Lightweight view of a cached AXT proof entry for diagnostics/tests.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AxtProofCacheSnapshot {
    /// Dataspace identifier associated with the cached proof.
    pub dsid: DataSpaceId,
    /// Hash of the proof payload.
    pub digest: [u8; 32],
    /// Expiry slot (after applying configured skew).
    pub expiry_slot: Option<u64>,
    /// Slot at which the proof was verified.
    pub verified_slot: u64,
    /// Manifest root bound to the proof, if provided.
    pub manifest_root: Option<[u8; 32]>,
    /// Whether the proof was accepted.
    pub valid: bool,
    /// Cache status label for telemetry.
    pub status: &'static str,
}

impl CachedProofEntry {
    fn is_applicable_for_slot(
        &self,
        slot: Option<u64>,
        manifest_root: Option<[u8; 32]>,
        ttl_slots: NonZeroU64,
    ) -> bool {
        if manifest_root.is_some() && manifest_root != self.manifest_root {
            return false;
        }
        let slot = slot.unwrap_or(0);
        if let Some(expiry) = self.expiry_slot {
            if slot > 0 && slot > expiry {
                return false;
            }
        }
        if slot == 0 || self.verified_slot == 0 {
            return true;
        }
        let ttl = ttl_slots.get().saturating_sub(1);
        let max_slot = self.verified_slot.saturating_add(ttl);
        slot >= self.verified_slot && slot <= max_slot
    }
}

fn current_axt_slot_for_state(state: &(impl StateReadOnly + ?Sized)) -> u64 {
    let slot_length = state.nexus().axt.slot_length_ms;
    if let Some(latest_block) = state.latest_block() {
        return current_axt_slot_from_block(&latest_block.header(), slot_length);
    }
    if state.height() > 0 {
        return state.height() as u64;
    }
    0
}

#[cfg(feature = "sm-ffi-openssl")]
fn apply_sm_openssl_preview(enabled: bool) {
    let previous = iroha_crypto::sm::OpenSslProvider::is_enabled();
    if previous != enabled {
        tracing::info!(
            target = "iroha::crypto",
            preview_enabled = enabled,
            "OpenSSL SM preview {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }
    iroha_crypto::sm::OpenSslProvider::set_preview_enabled(enabled);
}

#[cfg(not(feature = "sm-ffi-openssl"))]
fn apply_sm_openssl_preview(_: bool) {}

/// Core host adapter used by Iroha to run IVM bytecode.
///
/// Stateful operations must be translated into ISIs and executed via the
/// executor. Durable-state syscalls are only forwarded to an in-memory
/// overlay when access logging is enabled for prepass execution.
pub struct CoreHost {
    authority: AccountId,
    default: ivm::host::DefaultHost,
    codec_host: IvmCodecHost,
    access_log_enabled: bool,
    halo2_config: ivm::host::ZkHalo2Config,
    crypto: Arc<iroha_config::parameters::actual::Crypto>,
    queued: Vec<InstructionBox>,
    fastpq_batch_entries: Option<Vec<TransferAssetBatchEntry>>,
    // Snapshot of accounts available for simple iteration helpers used by samples.
    accounts_snapshot: Arc<Vec<AccountId>>,
    // Simple counter for sentinel-generated NFT ids to guarantee uniqueness across calls.
    nft_seq: u64,
    // Optional trigger arguments for by-call entrypoints.
    args: Option<iroha_primitives::json::Json>,
    // Snapshot of durable smart-contract state persisted in WSV.
    durable_state_base: BTreeMap<Name, Vec<u8>>,
    // Overlay of durable state updates staged by the current VM execution.
    durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    // Access log for durable-state reads/writes (prepass).
    state_access_log: ivm::host::AccessLog,
    // Active atomic cross-transaction (AXT) envelope state.
    axt_state: Option<axt::HostAxtState>,
    // Completed AXT envelopes awaiting export into WSV/block artifacts.
    completed_axt: Vec<axt::HostAxtState>,
    // Snapshots of streaming session capabilities advertised by the VM.
    transport_caps_snapshot: Option<TransportCapabilityResolutionSnapshot>,
    negotiated_caps_snapshot: Option<CapabilityFlags>,
    // ZK-proof verification state (gates ISI mutations)
    zk_verified_transfer: VecDeque<[u8; 32]>,
    zk_verified_unshield: VecDeque<[u8; 32]>,
    zk_verified_ballot: VecDeque<[u8; 32]>,
    zk_verified_tally: VecDeque<[u8; 32]>,
    // Snapshots for state-read syscalls
    zk_roots: BTreeMap<AssetDefinitionId, Vec<[u8; 32]>>,
    zk_elections: BTreeMap<String, (bool, Vec<u64>)>,
    // Registry snapshot of verifying keys.
    verifying_keys: BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    // Chain id bytes for domain-tag binding.
    chain_id_bytes: Vec<u8>,
    // Policy hook for AXT validation (deny-wins).
    axt_policy: Arc<dyn ivm::axt::AxtPolicy>,
    // AXT timing configuration sourced from `iroha_config`.
    axt_timing: iroha_config::parameters::actual::NexusAxt,
    // Current manifest id for namespace binding (None -> core/built-in).
    current_manifest_id: Option<String>,
    // Cap for ZK roots read responses
    zk_root_history_cap: usize,
    // Optional: include explicit empty-tree root for assets with no commitments
    zk_include_empty_root_when_empty: bool,
    // Depth used to compute the explicit empty-tree root when the above is enabled
    zk_empty_root_depth: u8,
    // Last seen verify envelope hashes per ZK op type (from pointer‑ABI TLVs)
    zk_last_env_hash_transfer: VecDeque<[u8; 32]>,
    zk_last_env_hash_unshield: VecDeque<[u8; 32]>,
    zk_last_env_hash_ballot: VecDeque<[u8; 32]>,
    zk_last_env_hash_tally: VecDeque<[u8; 32]>,
    // Cached AXT policy snapshot (if available) for telemetry and richer errors.
    axt_policy_snapshot: Option<AxtPolicySnapshot>,
    // Bounded replay ledger hydrated from WSV.
    axt_replay_ledger: BTreeMap<AxtHandleReplayKey, AxtReplayRecord>,
    // Slot for which cached AXT proofs were verified.
    axt_proof_cache_slot: Option<u64>,
    // Cache of per-dataspace proofs validated in the current slot.
    axt_proof_cache: BTreeMap<DataSpaceId, CachedProofEntry>,
    // Last observed AXT rejection for structured error reporting.
    last_axt_reject: Option<AxtRejectContext>,
    // Static analysis of the currently executing program for AMX budgeting.
    amx_analysis: Option<ProgramAnalysis>,
    // Configured AMX execution budgets (per-dataspace and group).
    amx_limits: AmxLimits,
    // Cached AMX budget violation for structured rejection reporting.
    amx_budget_violation: Option<AmxBudgetViolation>,
    #[cfg(feature = "telemetry")]
    telemetry: Option<StateTelemetry>,
}

struct DomainHashInputs<'a> {
    backend: &'a str,
    curve: &'a str,
    vk_commitment: [u8; 32],
    schema_hash: [u8; 32],
    syscall_label: &'a str,
    manifest: &'a str,
    namespace: &'a str,
}

/// Structured details about an AMX budget violation captured by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmxBudgetViolation {
    /// Dataspace associated with the violating slice.
    pub dataspace: DataSpaceId,
    /// Stage where the violation occurred.
    pub stage: AmxStage,
    /// Observed duration in milliseconds (ceil from nanoseconds).
    pub elapsed_ms: u32,
    /// Configured budget for the stage in milliseconds.
    pub budget_ms: u32,
}

impl AmxBudgetViolation {
    /// Render the violation as a canonical AMX timeout error.
    #[must_use]
    pub const fn as_canonical(self) -> CanonicalErrorKind {
        CanonicalErrorKind::AmxTimeout(AmxTimeout {
            dataspace: self.dataspace,
            stage: self.stage,
            elapsed_ms: self.elapsed_ms,
            budget_ms: self.budget_ms,
        })
    }
}

#[allow(clippy::cast_possible_truncation)]
const fn nanoseconds_to_millis(ns: u64) -> u32 {
    let millis = ns.saturating_add(999_999) / 1_000_000;
    if millis > u32::MAX as u64 {
        u32::MAX
    } else {
        millis as u32
    }
}

#[allow(clippy::used_underscore_binding)]
impl CoreHost {
    /// Create a new host for the given authority.
    pub fn new(authority: AccountId) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let mut default = ivm::host::DefaultHost::new();
        default.set_sm_enabled(default_crypto.sm_helpers_enabled());
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: Arc::new(Vec::new()),
            nft_seq: 0,
            args: None,
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_transfer: VecDeque::new(),
            zk_verified_unshield: VecDeque::new(),
            zk_verified_ballot: VecDeque::new(),
            zk_verified_tally: VecDeque::new(),
            zk_roots: BTreeMap::new(),
            zk_elections: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_root_history_cap: iroha_config::parameters::defaults::zk::ledger::ROOT_HISTORY_CAP,
            zk_include_empty_root_when_empty: false,
            zk_empty_root_depth: 0,
            zk_last_env_hash_transfer: VecDeque::new(),
            zk_last_env_hash_unshield: VecDeque::new(),
            zk_last_env_hash_ballot: VecDeque::new(),
            zk_last_env_hash_tally: VecDeque::new(),
            axt_policy_snapshot: None,
            axt_replay_ledger: BTreeMap::new(),
            axt_proof_cache_slot: None,
            axt_proof_cache: BTreeMap::new(),
            last_axt_reject: None,
            amx_analysis: None,
            amx_limits: AmxLimits::default(),
            amx_budget_violation: None,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }

    /// Construct a host that enforces AXT policy from the provided state snapshot.
    pub fn from_state(authority: AccountId, state: &crate::state::State) -> Self {
        let view = state.view();
        let snapshot = view.axt_policy_snapshot();
        let mut host = Self::new(authority);
        host.set_axt_timing(view.nexus.axt);
        host.hydrate_axt_replay_ledger(&view);
        host.set_durable_state_snapshot_from_world(view.world());
        host = host.with_axt_policy_snapshot(&snapshot);
        host.set_amx_limits(Self::amx_limits_from_config(&view.pipeline));
        host
    }

    /// Create host with an account snapshot for iteration helpers.
    pub fn with_accounts(authority: AccountId, accounts: Arc<Vec<AccountId>>) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let mut default = ivm::host::DefaultHost::new();
        default.set_sm_enabled(default_crypto.sm_helpers_enabled());
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            nft_seq: 0,
            args: None,
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_transfer: VecDeque::new(),
            zk_verified_unshield: VecDeque::new(),
            zk_verified_ballot: VecDeque::new(),
            zk_verified_tally: VecDeque::new(),
            zk_roots: BTreeMap::new(),
            zk_elections: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_root_history_cap: iroha_config::parameters::defaults::zk::ledger::ROOT_HISTORY_CAP,
            zk_include_empty_root_when_empty: false,
            zk_empty_root_depth: 0,
            zk_last_env_hash_transfer: VecDeque::new(),
            zk_last_env_hash_unshield: VecDeque::new(),
            zk_last_env_hash_ballot: VecDeque::new(),
            zk_last_env_hash_tally: VecDeque::new(),
            axt_policy_snapshot: None,
            axt_replay_ledger: BTreeMap::new(),
            axt_proof_cache_slot: None,
            axt_proof_cache: BTreeMap::new(),
            last_axt_reject: None,
            amx_analysis: None,
            amx_limits: AmxLimits::default(),
            amx_budget_violation: None,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }

    /// Create host with accounts and trigger args (for by-call entrypoints).
    pub fn with_accounts_and_args(
        authority: AccountId,
        accounts: Arc<Vec<AccountId>>,
        args: iroha_primitives::json::Json,
    ) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let mut default = ivm::host::DefaultHost::new();
        default.set_sm_enabled(default_crypto.sm_helpers_enabled());
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            nft_seq: 0,
            args: Some(args),
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_transfer: VecDeque::new(),
            zk_verified_unshield: VecDeque::new(),
            zk_verified_ballot: VecDeque::new(),
            zk_verified_tally: VecDeque::new(),
            zk_roots: BTreeMap::new(),
            zk_elections: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_root_history_cap: iroha_config::parameters::defaults::zk::ledger::ROOT_HISTORY_CAP,
            zk_include_empty_root_when_empty: false,
            zk_empty_root_depth: 0,
            zk_last_env_hash_transfer: VecDeque::new(),
            zk_last_env_hash_unshield: VecDeque::new(),
            zk_last_env_hash_ballot: VecDeque::new(),
            zk_last_env_hash_tally: VecDeque::new(),
            axt_replay_ledger: BTreeMap::new(),
            axt_policy_snapshot: None,
            axt_proof_cache_slot: None,
            axt_proof_cache: BTreeMap::new(),
            last_axt_reject: None,
            amx_analysis: None,
            amx_limits: AmxLimits::default(),
            amx_budget_violation: None,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }

    /// Enable access logging for prepass execution.
    #[must_use]
    pub fn with_access_logging(mut self) -> Self {
        self.access_log_enabled = true;
        self
    }

    /// Snapshot the transport capabilities negotiated during streaming handshakes.
    pub fn record_transport_caps_snapshot(
        &mut self,
        snapshot: TransportCapabilityResolutionSnapshot,
    ) {
        self.transport_caps_snapshot = Some(snapshot);
    }

    /// Snapshot the negotiated capability flags from streaming negotiation.
    pub fn record_negotiated_caps_snapshot(&mut self, caps: CapabilityFlags) {
        self.negotiated_caps_snapshot = Some(caps);
    }

    /// Latest transport capability snapshot recorded by the host.
    #[must_use]
    pub fn transport_caps_snapshot(&self) -> Option<&TransportCapabilityResolutionSnapshot> {
        self.transport_caps_snapshot.as_ref()
    }

    /// Latest negotiated capability flags recorded by the host.
    #[must_use]
    pub fn negotiated_caps_snapshot(&self) -> Option<&CapabilityFlags> {
        self.negotiated_caps_snapshot.as_ref()
    }

    /// Snapshot the AXT proof cache for diagnostics/tests.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn axt_proof_cache_snapshot(&self) -> Vec<AxtProofCacheSnapshot> {
        self.axt_proof_cache
            .iter()
            .map(|(dsid, entry)| AxtProofCacheSnapshot {
                dsid: *dsid,
                digest: entry.digest,
                expiry_slot: entry.expiry_slot,
                verified_slot: entry.verified_slot,
                manifest_root: entry.manifest_root,
                valid: entry.valid,
                status: entry.status,
            })
            .collect()
    }

    /// Seed the NFT sequence counter used by sample syscalls so ids remain unique across calls.
    pub fn set_nft_seq_base(&mut self, base: u64) {
        self.nft_seq = base;
    }

    /// Update cryptography configuration and propagate toggles to the helper host.
    pub fn set_crypto_config(&mut self, crypto: Arc<iroha_config::parameters::actual::Crypto>) {
        self.crypto = crypto;
        self.default
            .set_sm_enabled(self.crypto.sm_helpers_enabled());
        apply_sm_openssl_preview(self.crypto.enable_sm_openssl_preview);
        self.notify_telemetry_crypto_config();
    }

    /// Override the configured AXT timing (slot length + skew tolerance).
    pub fn set_axt_timing(&mut self, timing: iroha_config::parameters::actual::NexusAxt) {
        self.axt_timing = timing;
        if let Some(snapshot) = self.axt_policy_snapshot.clone() {
            self.refresh_axt_policy_snapshot(&snapshot);
        }
    }

    /// Set the AXT timing configuration and return the updated host (builder style).
    #[must_use]
    pub fn with_axt_timing(mut self, timing: iroha_config::parameters::actual::NexusAxt) -> Self {
        self.axt_timing = timing;
        self
    }

    /// Override the default allow-all AXT policy (e.g., in tests or when wiring UAID manifests).
    #[must_use]
    pub fn with_axt_policy(mut self, policy: Arc<dyn ivm::axt::AxtPolicy>) -> Self {
        self.axt_policy = policy;
        self.axt_policy_snapshot = None;
        for (dsid, entry) in core::mem::take(&mut self.axt_proof_cache) {
            self.clear_axt_proof_cache_state(dsid, &entry);
        }
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = None;
        self
    }

    /// Override the default AXT policy using a Space Directory snapshot.
    #[must_use]
    pub fn with_axt_policy_snapshot(mut self, snapshot: &AxtPolicySnapshot) -> Self {
        self.axt_policy = Arc::new(ivm::axt::SnapshotAxtPolicy::new_with_timing(
            snapshot,
            self.axt_timing.slot_length_ms,
            self.axt_timing.max_clock_skew_ms,
        ));
        self.axt_policy_snapshot = Some(snapshot.clone());
        for (dsid, entry) in core::mem::take(&mut self.axt_proof_cache) {
            self.clear_axt_proof_cache_state(dsid, &entry);
        }
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = Self::policy_current_slot(snapshot);
        self
    }

    /// Refresh the active AXT policy snapshot without rebuilding the host.
    ///
    /// This is used when manifest sets rotate mid-envelope or after a restart so proof
    /// cache entries and policy roots stay in sync with WSV.
    pub fn refresh_axt_policy_snapshot(&mut self, snapshot: &AxtPolicySnapshot) {
        self.axt_policy = Arc::new(ivm::axt::SnapshotAxtPolicy::new_with_timing(
            snapshot,
            self.axt_timing.slot_length_ms,
            self.axt_timing.max_clock_skew_ms,
        ));
        self.axt_policy_snapshot = Some(snapshot.clone());
        for (dsid, entry) in core::mem::take(&mut self.axt_proof_cache) {
            self.clear_axt_proof_cache_state(dsid, &entry);
        }
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = Self::policy_current_slot(snapshot);
        self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
    }

    /// Attach static AMX program analysis for budget enforcement.
    pub fn set_amx_analysis(&mut self, analysis: ProgramAnalysis) {
        self.amx_analysis = Some(analysis);
    }

    /// Override AMX budget limits (used for config plumbing and tests).
    pub fn set_amx_limits(&mut self, limits: AmxLimits) {
        self.amx_limits = limits;
    }

    /// Take and clear the last recorded AMX budget violation (if any).
    pub(crate) fn take_amx_budget_violation(&mut self) -> Option<AmxBudgetViolation> {
        self.amx_budget_violation.take()
    }

    /// Take and clear the last recorded AXT rejection (if any).
    pub(crate) fn take_axt_reject(&mut self) -> Option<AxtRejectContext> {
        self.last_axt_reject.take()
    }

    /// Expose the last recorded AMX budget violation for tests.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn take_amx_budget_violation_for_tests(&mut self) -> Option<AmxBudgetViolation> {
        self.take_amx_budget_violation()
    }

    /// Expose the last recorded AXT rejection for tests.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn take_axt_reject_for_tests(&mut self) -> Option<AxtRejectContext> {
        self.take_axt_reject()
    }

    /// Expose cached proof validity for tests.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn axt_cached_proof_status(&self, dsid: DataSpaceId) -> Option<(bool, Option<[u8; 32]>)> {
        self.axt_proof_cache
            .get(&dsid)
            .map(|entry| (entry.valid, entry.manifest_root))
    }

    /// Expose recorded AXT proofs for tests.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn axt_recorded_proof_payload(&self, dsid: DataSpaceId) -> Option<Vec<u8>> {
        self.axt_state
            .as_ref()
            .and_then(|state| state.proofs().get(&dsid))
            .map(|blob| blob.payload.clone())
    }

    pub(crate) fn amx_limits_from_config(
        pipeline: &iroha_config::parameters::actual::Pipeline,
    ) -> AmxLimits {
        AmxLimits {
            per_dataspace_budget_ms: pipeline.amx_per_dataspace_budget_ms,
            group_budget_ms: pipeline.amx_group_budget_ms,
            per_instruction_ns: pipeline.amx_per_instruction_ns,
            per_memory_access_ns: pipeline.amx_per_memory_access_ns,
            per_syscall_ns: pipeline.amx_per_syscall_ns,
        }
    }

    /// Derive an AXT policy snapshot from the Space Directory and lane catalog.
    ///
    /// The snapshot is built from active capability manifests per dataspace;
    /// entries are keyed by dataspace id and use the lane catalog to map to the
    /// target lane. When multiple manifests are active for the same dataspace,
    /// the newest activation epoch wins. Manifest roots are taken directly from
    /// the recorded manifest hash. Handle-era minima use the manifest activation
    /// epoch and the current slot falls back to the committed block height so
    /// expiry checks remain deterministic even before a dedicated slot clock is
    /// plumbed through. When bindings exist without an active manifest we still
    /// emit a zeroed manifest root to ensure lane gating remains active.
    #[must_use]
    pub fn derive_axt_policy_snapshot_from_directory(
        state: &(impl StateReadOnly + ?Sized),
    ) -> Option<AxtPolicySnapshot> {
        use std::collections::{BTreeMap, btree_map::Entry};

        let mut lane_for_dataspace = BTreeMap::new();
        for entry in state.nexus().lane_config.entries() {
            lane_for_dataspace
                .entry(entry.dataspace_id)
                .or_insert(entry.lane_id);
        }

        if lane_for_dataspace.is_empty() {
            return None;
        }

        let mut bindings: BTreeMap<DataSpaceId, (AxtPolicyEntry, Option<u64>)> = BTreeMap::new();
        let current_slot = current_axt_slot_for_state(state);
        let manifests = WorldReadOnly::space_directory_manifests(state.world());
        let bindings_view = WorldReadOnly::uaid_dataspaces(state.world());
        for (uaid, manifest_set) in manifests.iter() {
            for (dsid, record) in manifest_set.iter() {
                if !record.is_active() {
                    continue;
                }
                if record.manifest_hash.as_ref().iter().all(|byte| *byte == 0) {
                    iroha_logger::warn!(
                        %uaid,
                        dataspace_id = dsid.as_u64(),
                        "Skipping AXT policy entry for Space Directory manifest with zeroed hash"
                    );
                    continue;
                }
                let Some(&target_lane) = lane_for_dataspace.get(dsid) else {
                    continue;
                };

                let mut manifest_root = [0u8; 32];
                manifest_root.copy_from_slice(record.manifest_hash.as_ref());

                let entry = AxtPolicyEntry {
                    manifest_root,
                    target_lane,
                    min_handle_era: record
                        .lifecycle
                        .activated_epoch
                        .unwrap_or(record.manifest.activation_epoch),
                    min_sub_nonce: 0,
                    current_slot,
                };
                let activated_epoch = record.lifecycle.activated_epoch;
                match bindings.entry(*dsid) {
                    Entry::Vacant(slot) => {
                        slot.insert((entry, activated_epoch));
                    }
                    Entry::Occupied(mut slot) => {
                        let replace = match (activated_epoch, slot.get().1) {
                            (Some(newer), Some(prev)) => newer >= prev,
                            (Some(_), None) => true,
                            (None, _) => false,
                        };
                        if replace {
                            slot.insert((entry, activated_epoch));
                        }
                    }
                }
            }
        }

        for (_uaid, binding_for_uaid) in bindings_view.iter() {
            // Ensure each bound dataspace gets a policy entry even when the manifest is missing.
            for (dsid, _) in binding_for_uaid.iter() {
                if bindings.contains_key(dsid) {
                    continue;
                }
                let Some(&target_lane) = lane_for_dataspace.get(dsid) else {
                    continue;
                };
                bindings.insert(
                    *dsid,
                    (
                        AxtPolicyEntry {
                            manifest_root: [0; 32],
                            target_lane,
                            min_handle_era: 0,
                            min_sub_nonce: 0,
                            current_slot,
                        },
                        None,
                    ),
                );
            }
        }

        if bindings.is_empty() {
            return None;
        }

        let entries: Vec<AxtPolicyBinding> = bindings
            .into_iter()
            .map(|(dsid, (policy, _))| AxtPolicyBinding { dsid, policy })
            .collect();
        let version = AxtPolicySnapshot::compute_version(&entries);
        Some(AxtPolicySnapshot { version, entries })
    }

    /// Load an AXT policy snapshot from cached WSV entries when available.
    #[must_use]
    pub fn axt_policy_snapshot_from_state(state: &impl StateReadOnly) -> Option<AxtPolicySnapshot> {
        let current_slot = current_axt_slot_for_state(state);
        let (snapshot, cache_event) = {
            let policies = state.world().axt_policies();
            if policies.is_empty() {
                (
                    Self::derive_axt_policy_snapshot_from_directory(state),
                    AxtPolicyCacheEvent::CacheMiss,
                )
            } else {
                let mut entries: Vec<_> = policies
                    .iter()
                    .map(|(dsid, policy)| {
                        let mut policy = *policy;
                        policy.current_slot = current_slot;
                        AxtPolicyBinding {
                            dsid: *dsid,
                            policy,
                        }
                    })
                    .collect();
                entries.sort_by_key(|entry| entry.dsid);
                let version = AxtPolicySnapshot::compute_version(&entries);
                (
                    Some(AxtPolicySnapshot { version, entries }),
                    AxtPolicyCacheEvent::CacheHit,
                )
            }
        };

        #[cfg(feature = "telemetry")]
        {
            let telemetry = state.metrics();
            telemetry.note_axt_policy_snapshot_cache_event(cache_event.label());
            if let Some(ref snapshot) = snapshot {
                telemetry.set_axt_policy_snapshot_version(snapshot);
            } else {
                let default_snapshot = AxtPolicySnapshot::default();
                telemetry.set_axt_policy_snapshot_version(&default_snapshot);
            }
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = cache_event;

        snapshot
    }

    #[doc(hidden)]
    pub fn force_sm_enabled_for_tests(&mut self, enabled: bool) {
        #[cfg(feature = "sm")]
        {
            use iroha_config::parameters::defaults::crypto as defaults;
            use iroha_crypto::Algorithm;

            fn adjust(cfg: &mut iroha_config::parameters::actual::Crypto, enabled: bool) {
                if enabled {
                    if !cfg
                        .allowed_signing
                        .iter()
                        .any(|algo| matches!(algo, Algorithm::Sm2))
                    {
                        cfg.allowed_signing.push(Algorithm::Sm2);
                    }
                    cfg.allowed_signing.sort();
                    cfg.allowed_signing.dedup();
                    if !cfg.default_hash.eq_ignore_ascii_case("sm3-256") {
                        cfg.default_hash = "sm3-256".to_owned();
                    }
                    if cfg.sm2_distid_default.trim().is_empty() {
                        cfg.sm2_distid_default = defaults::sm2_distid_default();
                    }
                } else {
                    cfg.allowed_signing
                        .retain(|algo| !matches!(algo, Algorithm::Sm2));
                    if cfg.default_hash.eq_ignore_ascii_case("sm3-256") {
                        cfg.default_hash = defaults::default_hash();
                    }
                }
            }

            if let Some(cfg) = Arc::get_mut(&mut self.crypto) {
                adjust(cfg, enabled);
            } else {
                let mut cfg = iroha_config::parameters::actual::Crypto::default();
                adjust(&mut cfg, enabled);
                self.crypto = Arc::new(cfg);
            }
        }
        #[cfg(not(feature = "sm"))]
        {
            let _ = enabled;
        }

        self.default
            .set_sm_enabled(self.crypto.sm_helpers_enabled());
    }

    /// Configure Halo2 verification limits for forwarded `ZK_VERIFY` syscalls
    /// using `iroha_config.zk.halo2` values.
    pub fn set_halo2_config(&mut self, cfg: &iroha_config::parameters::actual::Halo2) {
        let curve = match cfg.curve {
            iroha_config::parameters::actual::ZkCurve::Pallas => ivm::host::ZkCurve::Pallas,
            iroha_config::parameters::actual::ZkCurve::Pasta => ivm::host::ZkCurve::Pasta,
            iroha_config::parameters::actual::ZkCurve::Goldilocks => ivm::host::ZkCurve::Goldilocks,
            iroha_config::parameters::actual::ZkCurve::Bn254 => ivm::host::ZkCurve::Bn254,
        };
        let backend = match cfg.backend {
            iroha_config::parameters::actual::Halo2Backend::Ipa => ivm::host::ZkHalo2Backend::Ipa,
        };
        let new_cfg = ivm::host::ZkHalo2Config {
            enabled: cfg.enabled,
            curve,
            backend,
            max_k: cfg.max_k,
            verifier_budget_ms: cfg.verifier_budget_ms,
            verifier_max_batch: cfg.verifier_max_batch,
            max_envelope_bytes: cfg.max_envelope_bytes,
            max_proof_bytes: cfg.max_proof_bytes,
            max_transcript_label_len: cfg.max_transcript_label_len,
            enforce_transcript_label_ascii: cfg.enforce_transcript_label_ascii,
        };
        let default = ivm::host::DefaultHost::new().with_zk_halo2_config(new_cfg);
        self.halo2_config = new_cfg;
        self.default = default;
        self.notify_telemetry_halo2_config();
    }

    #[cfg(feature = "telemetry")]
    /// Attach a telemetry sink to the host and align it with the current SM state toggle.
    pub fn set_telemetry(&mut self, telemetry: StateTelemetry) {
        self.telemetry = Some(telemetry);
        self.align_telemetry_snapshot();
    }

    #[cfg(feature = "telemetry")]
    fn notify_telemetry_crypto_config(&self) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.set_sm_openssl_preview(self.crypto.enable_sm_openssl_preview);
        }
    }

    #[cfg(not(feature = "telemetry"))]
    fn notify_telemetry_crypto_config(&self) {
        let _ = self;
    }

    #[cfg(feature = "telemetry")]
    fn notify_telemetry_halo2_config(&self) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.set_halo2_runtime_config(self.halo2_config);
        }
    }

    #[cfg(not(feature = "telemetry"))]
    fn notify_telemetry_halo2_config(&self) {
        let _ = self;
    }

    #[cfg(feature = "telemetry")]
    fn align_telemetry_snapshot(&mut self) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.set_sm_openssl_preview(self.crypto.enable_sm_openssl_preview);
            telemetry.set_halo2_runtime_config(self.halo2_config);
        }
    }

    pub(crate) fn clear_axt_reject(&mut self) {
        self.last_axt_reject = None;
    }

    fn record_axt_reject_detail(
        &mut self,
        reason: AxtRejectReason,
        dataspace: Option<DataSpaceId>,
        lane: Option<LaneId>,
        detail: impl Into<String>,
        next_min_handle_era: Option<u64>,
        next_min_sub_nonce: Option<u64>,
    ) {
        let detail = detail.into();
        let ctx = AxtRejectContext {
            reason,
            dataspace,
            lane,
            snapshot_version: self.current_axt_policy_version(),
            detail,
            next_min_handle_era,
            next_min_sub_nonce,
        };
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry.as_ref() {
            if let Some(lane) = lane {
                telemetry.note_axt_policy_reject(lane, reason, ctx.snapshot_version);
            }
            if let (Some(dsid), Some(lane), Some(min_handle_era), Some(min_sub_nonce)) =
                (dataspace, lane, next_min_handle_era, next_min_sub_nonce)
            {
                telemetry.set_axt_reject_hint(dsid, lane, min_handle_era, min_sub_nonce, reason);
            }
        }
        iroha_logger::warn!(
            target = "iroha::axt",
            reason = reason.label(),
            snapshot_version = ctx.snapshot_version,
            ?dataspace,
            ?lane,
            detail = %ctx.detail,
            next_min_handle_era = ?ctx.next_min_handle_era,
            next_min_sub_nonce = ?ctx.next_min_sub_nonce,
            "AXT policy rejection recorded"
        );
        self.last_axt_reject = Some(ctx);
    }

    fn record_axt_reject(
        &mut self,
        reason: AxtRejectReason,
        dataspace: Option<DataSpaceId>,
        lane: Option<LaneId>,
        detail: impl Into<String>,
    ) {
        self.record_axt_reject_detail(reason, dataspace, lane, detail, None, None);
    }
    #[cfg(feature = "telemetry")]
    fn sm_labels(number: u32) -> (&'static str, &'static str) {
        match number {
            ivm::syscalls::SYSCALL_SM3_HASH => ("hash", "-"),
            ivm::syscalls::SYSCALL_SM2_VERIFY => ("verify", "-"),
            ivm::syscalls::SYSCALL_SM4_GCM_SEAL => ("seal", "gcm"),
            ivm::syscalls::SYSCALL_SM4_GCM_OPEN => ("open", "gcm"),
            ivm::syscalls::SYSCALL_SM4_CCM_SEAL => ("seal", "ccm"),
            ivm::syscalls::SYSCALL_SM4_CCM_OPEN => ("open", "ccm"),
            _ => ("other", "-"),
        }
    }

    #[cfg(feature = "telemetry")]
    fn sm_error_reason(error: &ivm::VMError) -> &'static str {
        use ivm::VMError::*;
        match error {
            PermissionDenied => "permission_denied",
            NoritoInvalid => "norito_invalid",
            DecodeError => "decode_error",
            OutOfGas => "out_of_gas",
            OutOfMemory => "out_of_memory",
            MemoryAccessViolation { .. } | MemoryOutOfBounds | MemoryPermissionDenied => {
                "memory_violation"
            }
            NotImplemented { .. } => "not_implemented",
            UnknownSyscall(_) => "unknown_syscall",
            PrivacyViolation => "privacy_violation",
            VectorExtensionDisabled => "vector_disabled",
            ZkExtensionDisabled => "zk_disabled",
            _ => "other",
        }
    }

    #[cfg(feature = "telemetry")]
    fn record_sm_syscall<T>(
        telemetry: &StateTelemetry,
        number: u32,
        result: &Result<T, ivm::VMError>,
    ) {
        let (kind, mode) = Self::sm_labels(number);
        match result {
            Ok(_) => telemetry.note_sm_syscall_success(kind, mode),
            Err(err) => {
                let reason = Self::sm_error_reason(err);
                telemetry.note_sm_syscall_failure(kind, mode, reason);
            }
        }
    }

    /// Set chain id for VRF binding. When set, the underlying `DefaultHost` will
    /// enforce and use this chain id for VRF prehashing.
    pub fn set_chain_id(&mut self, chain: &iroha_data_model::ChainId) {
        // Mutably set chain id in the underlying DefaultHost (no reset of other state)
        self.default
            .set_chain_id_bytes(chain.to_string().into_bytes());
        self.chain_id_bytes = chain.to_string().into_bytes();
    }

    /// Install a read-only snapshot of ZK roots per asset for state-read syscalls.
    pub fn set_zk_roots_snapshot(&mut self, map: BTreeMap<AssetDefinitionId, Vec<[u8; 32]>>) {
        self.zk_roots = map;
    }

    /// Snapshot durable smart-contract state from the provided world view.
    pub fn set_durable_state_snapshot_from_world(&mut self, world: &impl WorldReadOnly) {
        self.durable_state_base = world
            .smart_contract_state()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.durable_state_overlay.clear();
    }

    /// Set the chain id for ZK domain binding.
    pub fn set_chain_id_bytes(&mut self, chain_id: Vec<u8>) {
        self.chain_id_bytes = chain_id;
        self.default.set_chain_id_bytes(self.chain_id_bytes.clone());
    }

    /// Set the current manifest id for namespace binding.
    pub fn set_current_manifest_id(&mut self, manifest: Option<String>) {
        self.current_manifest_id = manifest;
    }

    /// Apply the active VK registry snapshot for verification-time binding.
    ///
    /// # Errors
    /// Returns an error when a stored commitment does not match the inline verifying key bytes.
    pub fn set_verifying_keys(
        &mut self,
        map: BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    ) -> Result<(), ivm::VMError> {
        // Early sanity: ensure commitments match inline keys when present.
        for rec in map.values() {
            if let Some(ref vk) = rec.key {
                let c = crate::zk::hash_vk(vk);
                if c != rec.commitment {
                    return Err(ivm::VMError::NoritoInvalid);
                }
            }
        }
        self.verifying_keys = map;
        Ok(())
    }

    fn backend_label_for_record(rec: &VerifyingKeyRecord) -> String {
        if let Some(ref vk) = rec.key {
            return vk.backend.clone();
        }
        if let Some(prefix) = rec.circuit_id.split(':').next() {
            if !prefix.is_empty() {
                return prefix.to_string();
            }
        }
        match rec.backend {
            BackendTag::Halo2IpaPasta => "halo2/ipa",
            BackendTag::Halo2Bn254 => "halo2/bn254",
            BackendTag::Groth16 => "groth16",
            BackendTag::Stark => "stark",
            BackendTag::Unsupported => "unsupported",
        }
        .to_string()
    }

    fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(backend.as_bytes());
        h.update(bytes);
        h.finalize().into()
    }

    fn compute_domain_hash(&self, inputs: &DomainHashInputs<'_>) -> [u8; 32] {
        use iroha_crypto::Hash;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"iroha.zk.domain/v1");
        buf.extend_from_slice(&self.chain_id_bytes);
        buf.extend_from_slice(inputs.backend.as_bytes());
        buf.extend_from_slice(inputs.curve.as_bytes());
        buf.extend_from_slice(&inputs.vk_commitment);
        buf.extend_from_slice(&inputs.schema_hash);
        buf.extend_from_slice(inputs.syscall_label.as_bytes());
        buf.extend_from_slice(inputs.manifest.as_bytes());
        buf.extend_from_slice(inputs.namespace.as_bytes());
        *Hash::new(&buf).as_ref()
    }

    fn validate_envelope_header(
        &self,
        env: &iroha_zkp_halo2::OpenVerifyEnvelope,
        payload_len: usize,
        expected_label: &str,
    ) -> Result<(), u64> {
        if payload_len > self.halo2_config.max_envelope_bytes {
            return Err(ivm::host::ERR_ENVELOPE_SIZE);
        }
        if env.transcript_label != expected_label {
            return Err(ivm::host::ERR_TRANSCRIPT_LABEL);
        }
        if env.transcript_label.is_empty()
            || env.transcript_label.len() > self.halo2_config.max_transcript_label_len
            || (self.halo2_config.enforce_transcript_label_ascii
                && !env
                    .transcript_label
                    .bytes()
                    .all(|b| matches!(b, 0x21..=0x7e)))
        {
            return Err(ivm::host::ERR_TRANSCRIPT_LABEL);
        }
        if !self.halo2_config.enabled {
            return Err(ivm::host::ERR_DISABLED);
        }
        if self.halo2_config.backend != ivm::host::ZkHalo2Backend::Ipa {
            return Err(ivm::host::ERR_BACKEND);
        }
        let curve_ok = match (self.halo2_config.curve, env.params.curve_id) {
            (ivm::host::ZkCurve::Pallas, cid)
                if cid == iroha_zkp_halo2::ZkCurveId::Pallas.as_u16() =>
            {
                true
            }
            (ivm::host::ZkCurve::Pasta, cid)
                if cid == iroha_zkp_halo2::ZkCurveId::Pasta.as_u16() =>
            {
                true
            }
            (ivm::host::ZkCurve::Goldilocks, cid)
                if cid == iroha_zkp_halo2::ZkCurveId::Goldilocks.as_u16() =>
            {
                true
            }
            (ivm::host::ZkCurve::Bn254, cid)
                if cid == iroha_zkp_halo2::ZkCurveId::Bn254.as_u16() =>
            {
                true
            }
            _ => false,
        };
        if !curve_ok {
            return Err(ivm::host::ERR_CURVE);
        }
        let n = env.params.n as usize;
        let k = if n.is_power_of_two() {
            n.trailing_zeros()
        } else {
            u32::MAX
        };
        if k == u32::MAX || k > self.halo2_config.max_k {
            return Err(ivm::host::ERR_K);
        }
        Ok(())
    }

    fn load_vk_record(
        &self,
        env: &iroha_zkp_halo2::OpenVerifyEnvelope,
        vk_commitment: [u8; 32],
        schema_hash: [u8; 32],
        namespace: &str,
    ) -> Result<(VerifyingKeyRecord, String), u64> {
        let vk_rec = self
            .verifying_keys
            .values()
            .find(|r| r.commitment == vk_commitment)
            .cloned()
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        let curve_label = match iroha_zkp_halo2::ZkCurveId::from_u16(env.params.curve_id) {
            iroha_zkp_halo2::ZkCurveId::Pallas => "pallas",
            iroha_zkp_halo2::ZkCurveId::Pasta => "pasta",
            iroha_zkp_halo2::ZkCurveId::Goldilocks => "goldilocks",
            iroha_zkp_halo2::ZkCurveId::Bn254 => "bn254",
            _ => return Err(ivm::host::ERR_CURVE),
        };
        if !vk_rec.curve.eq_ignore_ascii_case(curve_label) {
            return Err(ivm::host::ERR_CURVE);
        }
        if vk_rec.backend != BackendTag::Halo2IpaPasta {
            return Err(ivm::host::ERR_BACKEND);
        }
        if !vk_rec.is_active() {
            return Err(ivm::host::ERR_VK_INACTIVE);
        }
        if vk_rec.public_inputs_schema_hash != schema_hash || vk_rec.commitment != vk_commitment {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        if vk_rec.namespace != namespace {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        let backend_label = Self::backend_label_for_record(&vk_rec);
        Ok((vk_rec, backend_label))
    }

    fn validate_proof_len(
        &self,
        vk_rec: &VerifyingKeyRecord,
        proof_len_bytes: usize,
    ) -> Result<(), u64> {
        let proof_limit = u32::try_from(proof_len_bytes).unwrap_or(u32::MAX);
        if vk_rec.max_proof_bytes > 0 && proof_limit > vk_rec.max_proof_bytes {
            return Err(ivm::host::ERR_PROOF_LEN);
        }
        if proof_len_bytes > self.halo2_config.max_proof_bytes {
            return Err(ivm::host::ERR_PROOF_LEN);
        }
        Ok(())
    }

    fn enforce_zk_envelope(
        &mut self,
        payload: &[u8],
        expected_label: &str,
        namespace: &str,
    ) -> Result<(), u64> {
        let env: iroha_zkp_halo2::OpenVerifyEnvelope =
            norito::decode_from_bytes(payload).map_err(|_| ivm::host::ERR_DECODE)?;
        self.validate_envelope_header(&env, payload.len(), expected_label)?;
        let vk_commitment = env.vk_commitment.ok_or(ivm::host::ERR_VK_MISSING)?;
        let schema_hash = env
            .public_inputs_schema_hash
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        let (vk_rec, backend_label) =
            self.load_vk_record(&env, vk_commitment, schema_hash, namespace)?;
        let vk_box = vk_rec.key.as_ref().ok_or(ivm::host::ERR_VK_MISSING)?;
        let inline_commit = Self::hash_vk_bytes(&backend_label, &vk_box.bytes);
        if inline_commit != vk_rec.commitment {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        let current_manifest = self.current_manifest_id.as_deref().unwrap_or("core");
        if vk_rec.owner_manifest_id.as_deref().unwrap_or("core") != current_manifest {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        if vk_rec.namespace != namespace {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        let proof_len_bytes: usize = norito::to_bytes(&env.proof)
            .map_err(|_| ivm::host::ERR_DECODE)?
            .len();
        self.validate_proof_len(&vk_rec, proof_len_bytes)?;
        let domain_inputs = DomainHashInputs {
            backend: &backend_label,
            curve: &vk_rec.curve,
            vk_commitment,
            schema_hash,
            syscall_label: expected_label,
            manifest: current_manifest,
            namespace,
        };
        let domain = self.compute_domain_hash(&domain_inputs);
        if env.domain_tag != Some(domain) {
            return Err(ivm::host::ERR_DOMAIN_TAG);
        }
        self.default
            .set_external_vk_bytes(vk_box.backend.clone(), vk_box.bytes.clone());
        Ok(())
    }

    /// Install a read-only snapshot of elections (finalized flag and tally) for state-read syscalls.
    pub fn set_zk_elections_snapshot(&mut self, map: BTreeMap<String, (bool, Vec<u64>)>) {
        self.zk_elections = map;
    }

    /// Configure the cap for recent shielded Merkle roots returned by `ZK_ROOTS_GET`.
    pub fn set_zk_root_history_cap(&mut self, cap: usize) {
        self.zk_root_history_cap = cap.max(1);
    }

    /// Optionally include an explicit empty‑tree root for assets with no commitments.
    /// Disabled by default. When enabled and an asset has zero recorded roots, the
    /// `ZK_ROOTS_GET` response will contain `latest = R_depth` and `roots = [R_depth]`
    /// where `R_depth` is computed as repeated `Hash(r||r)` starting from the
    /// domain‑tagged zero leaf at depth 0.
    pub fn set_zk_empty_root_policy(&mut self, include: bool, depth: u8) {
        self.zk_include_empty_root_when_empty = include;
        self.zk_empty_root_depth = depth;
    }

    /// Drain queued ISIs collected during the last VM run.
    pub fn drain_instructions(&mut self) -> Vec<InstructionBox> {
        self.flush_pending_fastpq_batch();
        mem::take(&mut self.queued)
    }

    /// Test helper: seed the transfer verification latch with a known envelope hash.
    #[cfg(test)]
    pub fn __test_seed_transfer_latch(&mut self, hash: [u8; 32]) {
        self.zk_verified_transfer.push_back(hash);
        self.zk_last_env_hash_transfer.push_back(hash);
    }

    /// Test helper: seed the unshield verification latch with a known envelope hash.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn __test_seed_unshield_latch(&mut self, hash: [u8; 32]) {
        self.zk_verified_unshield.push_back(hash);
        self.zk_last_env_hash_unshield.push_back(hash);
    }

    /// Test helper: seed the ballot verification latch with a known envelope hash.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn __test_seed_ballot_latch(&mut self, hash: [u8; 32]) {
        self.zk_verified_ballot.push_back(hash);
        self.zk_last_env_hash_ballot.push_back(hash);
    }

    /// Test helper: seed the tally verification latch with a known envelope hash.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn __test_seed_tally_latch(&mut self, hash: [u8; 32]) {
        self.zk_verified_tally.push_back(hash);
        self.zk_last_env_hash_tally.push_back(hash);
    }

    /// Test-only: replace the pending transfer envelope hash queue with a single entry.
    #[cfg(test)]
    pub fn __test_set_last_env_hash_transfer(&mut self, h: [u8; 32]) {
        self.zk_last_env_hash_transfer.clear();
        self.zk_last_env_hash_transfer.push_back(h);
    }
    /// Test helper: replace the pending unshield envelope hash queue with a single entry.
    pub fn __test_set_last_env_hash_unshield(&mut self, h: [u8; 32]) {
        self.zk_last_env_hash_unshield.clear();
        self.zk_last_env_hash_unshield.push_back(h);
    }
    /// Test-only: replace the pending ballot envelope hash queue with a single entry.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn __test_set_last_env_hash_ballot(&mut self, h: [u8; 32]) {
        self.zk_last_env_hash_ballot.clear();
        self.zk_last_env_hash_ballot.push_back(h);
    }
    /// Test-only: replace the pending tally envelope hash queue with a single entry.
    #[cfg(test)]
    pub fn __test_set_last_env_hash_tally(&mut self, h: [u8; 32]) {
        self.zk_last_env_hash_tally.clear();
        self.zk_last_env_hash_tally.push_back(h);
    }

    /// Apply queued ISIs via the executor, returning the executed instructions.
    ///
    /// # Errors
    /// Returns a `ValidationFail` if an instruction fails permission checks or execution.
    pub fn apply_queued(
        &mut self,
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        let queued = self.drain_instructions();
        let executor = tx.world.executor.clone();
        for instr in &queued {
            let instr = instr.clone();
            // Gate ZK ISIs on prior successful ZK_VERIFY
            let any: &dyn iroha_data_model::isi::Instruction = &*instr;
            let any_ref = any.as_any();
            if let Some(z) = any_ref.downcast_ref::<DMZk::ZkTransfer>() {
                let Some(expected_hash) = self.zk_verified_transfer.pop_front() else {
                    return Err(ValidationFail::NotPermitted(
                        "missing ZK_VERIFY_TRANSFER prior to ZkTransfer".to_owned(),
                    ));
                };
                let actual_hash = z.proof.envelope_hash.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "ZkTransfer missing envelope_hash after verification".to_owned(),
                    )
                })?;
                if actual_hash != expected_hash {
                    return Err(ValidationFail::NotPermitted(
                        "ZkTransfer envelope hash mismatch; requires fresh verification".to_owned(),
                    ));
                }
            }
            if let Some(u) = any_ref.downcast_ref::<DMZk::Unshield>() {
                let Some(expected_hash) = self.zk_verified_unshield.pop_front() else {
                    return Err(ValidationFail::NotPermitted(
                        "missing ZK_VERIFY_UNSHIELD prior to Unshield".to_owned(),
                    ));
                };
                let actual_hash = u.proof.envelope_hash.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "Unshield missing envelope_hash after verification".to_owned(),
                    )
                })?;
                if actual_hash != expected_hash {
                    return Err(ValidationFail::NotPermitted(
                        "Unshield envelope hash mismatch; requires fresh verification".to_owned(),
                    ));
                }
            }
            if let Some(sb) = any_ref.downcast_ref::<DMZk::SubmitBallot>() {
                let Some(expected_hash) = self.zk_verified_ballot.pop_front() else {
                    return Err(ValidationFail::NotPermitted(
                        "missing ZK_VOTE_VERIFY_BALLOT prior to SubmitBallot".to_owned(),
                    ));
                };
                let actual_hash = sb.ballot_proof.envelope_hash.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "SubmitBallot missing envelope_hash after verification".to_owned(),
                    )
                })?;
                if actual_hash != expected_hash {
                    return Err(ValidationFail::NotPermitted(
                        "SubmitBallot envelope hash mismatch; requires fresh verification"
                            .to_owned(),
                    ));
                }
            }
            if let Some(ft) = any_ref.downcast_ref::<DMZk::FinalizeElection>() {
                let Some(expected_hash) = self.zk_verified_tally.pop_front() else {
                    return Err(ValidationFail::NotPermitted(
                        "missing ZK_VOTE_VERIFY_TALLY prior to FinalizeElection".to_owned(),
                    ));
                };
                let actual_hash = ft.tally_proof.envelope_hash.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "FinalizeElection missing envelope_hash after verification".to_owned(),
                    )
                })?;
                if actual_hash != expected_hash {
                    return Err(ValidationFail::NotPermitted(
                        "FinalizeElection envelope hash mismatch; requires fresh verification"
                            .to_owned(),
                    ));
                }
            }
            executor.execute_instruction(tx, authority, instr)?;
        }
        if !queued.is_empty() {
            let delta = queued
                .iter()
                .map(crate::gas::confidential_gas_cost)
                .sum::<u64>();
            if delta > 0 {
                tx.record_confidential_gas_delta(delta);
            }
        }
        self.flush_completed_axt(tx);
        self.flush_durable_state(tx);
        Ok(queued)
    }

    fn log_state_read_key(&mut self, key: &str) {
        if !self.access_log_enabled {
            return;
        }
        self.state_access_log.read_keys.insert(key.to_string());
    }

    fn log_state_write_key(&mut self, key: &str) {
        if !self.access_log_enabled {
            return;
        }
        self.state_access_log.write_keys.insert(key.to_string());
        self.state_access_log
            .state_writes
            .push(ivm::parallel::StateUpdate {
                key: key.to_string(),
                value: 1,
            });
    }

    fn decode_name_payload(payload: &[u8]) -> Result<Name, ivm::VMError> {
        let mut reader = Cursor::new(payload);
        if let Ok(name) = Name::decode(&mut reader) {
            return Ok(name);
        }
        let s = core::str::from_utf8(payload).map_err(|_| ivm::VMError::NoritoInvalid)?;
        Name::from_str(s).map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn load_state_value(vm: &mut IVM, stored: &[u8]) -> Result<(), ivm::VMError> {
        let mut env = stored.to_vec();
        if let Ok(inner) = ivm::pointer_abi::validate_tlv_bytes(&env) {
            if inner.type_id != PointerType::NoritoBytes {
                return Err(ivm::VMError::NoritoInvalid);
            }
        } else {
            let mut out = Vec::with_capacity(7 + env.len() + Hash::LENGTH);
            out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
            out.push(1);
            let env_len = u32::try_from(env.len()).map_err(|_| ivm::VMError::NoritoInvalid)?;
            out.extend_from_slice(&env_len.to_be_bytes());
            out.extend_from_slice(&env);
            let h: [u8; Hash::LENGTH] = Hash::new(&env).into();
            out.extend_from_slice(&h);
            env = out;
        }
        let p = vm.alloc_input_tlv(&env)?;
        vm.set_register(10, p);
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn read_norito_payload<'a>(&self, vm: &'a IVM, ptr: u64) -> Result<&'a [u8], ivm::VMError> {
        // Try TLV envelope first: [type_id:u16][version:u8][len:be u32][payload][hash:32]
        // If TLV appears present but is invalid, do NOT silently fall back.
        match vm.memory.validate_tlv(ptr) {
            Ok(tlv) => {
                if !is_type_allowed_for_policy(vm.syscall_policy(), tlv.type_id) {
                    return Err(ivm::VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                return Ok(tlv.payload);
            }
            Err(ivm::VMError::NoritoInvalid | ivm::VMError::DecodeError) => {
                return Err(ivm::VMError::NoritoInvalid);
            }
            Err(_) => {
                // Not a valid TLV envelope; fall back to u32 length-prefixed blob.
            }
        }
        let len = u64::from(vm.memory.load_u32(ptr)?);
        vm.memory.load_region(ptr + 4, len)
    }

    fn decode_at<T: NoritoDecode>(&self, vm: &IVM, ptr: u64) -> Result<T, ivm::VMError> {
        let bytes = self.read_norito_payload(vm, ptr)?;
        T::decode(&mut &*bytes).map_err(|_| ivm::VMError::DecodeError)
    }

    /// Decode a typed TLV envelope from the VM INPUT region enforcing the pointer‑ABI type.
    ///
    /// - `ptr` points to the start of the TLV envelope.
    /// - `expected` is the pointer‑ABI type id that must match the envelope's `type_id`.
    /// - Also enforces that the `expected` type is allowed under the program's syscall policy.
    ///
    /// Returns the decoded value on success or a structured VM error on failure.
    ///
    /// # Errors
    /// Returns an error if the pointer type does not match, the type is not allowed
    /// by the current syscall policy, or the Norito payload cannot be decoded.
    pub fn decode_tlv_typed<T: NoritoDecode>(
        vm: &IVM,
        ptr: u64,
        expected: PointerType,
    ) -> Result<T, ivm::VMError> {
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != expected {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if !is_type_allowed_for_policy(vm.syscall_policy(), expected) {
            return Err(ivm::VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: expected as u16,
            });
        }
        let mut payload = tlv.payload;
        decode_from_bytes(payload)
            .or_else(|_| T::decode(&mut payload))
            .map_err(|_| ivm::VMError::DecodeError)
    }

    /// Decode a JSON pointer-ABI TLV. Accepts both Norito-encoded payloads and
    /// raw UTF-8 JSON strings to match Kotodama literal emission.
    ///
    /// # Errors
    /// Returns an error if the pointer is invalid for the active ABI policy or the payload fails to decode.
    pub fn decode_tlv_json(vm: &IVM, ptr: u64) -> Result<Json, ivm::VMError> {
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != PointerType::Json {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if !is_type_allowed_for_policy(vm.syscall_policy(), PointerType::Json) {
            return Err(ivm::VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: PointerType::Json as u16,
            });
        }

        if let Ok(value) = norito::decode_from_bytes(tlv.payload) {
            return Ok(value);
        }
        let s = core::str::from_utf8(tlv.payload).map_err(|_| ivm::VMError::DecodeError)?;
        s.parse::<Json>().map_err(|_| ivm::VMError::DecodeError)
    }

    fn decode_header_or_bare<T: NoritoDecode>(payload: &[u8]) -> Result<T, ivm::VMError> {
        decode_from_bytes(payload)
            .or_else(|_| {
                let mut bytes = payload;
                T::decode(&mut bytes)
            })
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn expect_tlv(vm: &IVM, ptr: u64, expected: PointerType) -> Result<ivm::Tlv<'_>, ivm::VMError> {
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != expected {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if !is_type_allowed_for_policy(vm.syscall_policy(), expected) {
            return Err(ivm::VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: expected as u16,
            });
        }
        Ok(tlv)
    }

    #[cfg(feature = "telemetry")]
    fn note_axt_proof_cache_event(&self, event: &'static str) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.note_axt_proof_cache_event(event);
        }
    }

    #[cfg(feature = "telemetry")]
    fn note_axt_proof_cache_state(&self, dsid: DataSpaceId, entry: &CachedProofEntry) {
        let manifest_root = entry.manifest_root.unwrap_or([0; 32]);
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.set_axt_proof_cache_state(
                dsid,
                entry.status,
                manifest_root,
                entry.verified_slot,
                entry.expiry_slot,
            );
        }
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::unused_self)]
    fn note_axt_proof_cache_state(&self, _dsid: DataSpaceId, _entry: &CachedProofEntry) {}

    #[cfg(feature = "telemetry")]
    fn clear_axt_proof_cache_state(&self, dsid: DataSpaceId, entry: &CachedProofEntry) {
        let manifest_root = entry.manifest_root.unwrap_or([0; 32]);
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.clear_axt_proof_cache_state(
                dsid,
                entry.status,
                manifest_root,
                entry.verified_slot,
            );
        }
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::unused_self)]
    fn clear_axt_proof_cache_state(&self, _dsid: DataSpaceId, _entry: &CachedProofEntry) {}

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::unused_self)]
    fn note_axt_proof_cache_event(&self, _event: &'static str) {}

    fn policy_current_slot(snapshot: &AxtPolicySnapshot) -> Option<u64> {
        snapshot
            .entries
            .iter()
            .map(|binding| binding.policy.current_slot)
            .filter(|slot| *slot > 0)
            .max()
    }

    fn axt_expiry_slot_with_skew(&self, expiry_slot: u64, override_ms: Option<u32>) -> u64 {
        ivm::axt::expiry_slot_with_skew(
            expiry_slot,
            self.axt_timing.slot_length_ms,
            self.axt_timing.max_clock_skew_ms,
            override_ms,
        )
    }

    fn reset_axt_proof_cache_for_slot(&mut self, slot: Option<u64>) {
        let slot = slot.filter(|value| *value > 0);
        if let Some(current_slot) = slot {
            let ttl_window = self
                .axt_timing
                .proof_cache_ttl_slots
                .get()
                .saturating_sub(1);
            let min_valid_slot = current_slot.saturating_sub(ttl_window);
            let mut expired = Vec::new();
            for (dsid, entry) in &self.axt_proof_cache {
                let expired_for_slot =
                    entry.verified_slot > 0 && entry.verified_slot < min_valid_slot;
                let expired_for_expiry = entry
                    .expiry_slot
                    .is_some_and(|expiry| expiry > 0 && current_slot > expiry);
                if expired_for_slot || expired_for_expiry {
                    expired.push(*dsid);
                }
            }
            if !expired.is_empty() {
                for dsid in expired {
                    if let Some(entry) = self.axt_proof_cache.remove(&dsid) {
                        self.clear_axt_proof_cache_state(dsid, &entry);
                    }
                }
                self.note_axt_proof_cache_event(AXT_PROOF_CACHE_EXPIRED);
            }
            self.axt_proof_cache_slot = Some(current_slot);
        } else {
            self.axt_proof_cache_slot = slot.or(self.axt_proof_cache_slot);
        }
    }

    fn policy_entry_for(&self, dsid: DataSpaceId) -> Option<AxtPolicyEntry> {
        self.axt_policy_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.entries.iter().find(|entry| entry.dsid == dsid))
            .map(|binding| binding.policy)
    }

    fn current_axt_policy_version(&self) -> u64 {
        self.axt_policy_snapshot.as_ref().map_or(0, |snapshot| {
            if snapshot.version != 0 {
                snapshot.version
            } else {
                AxtPolicySnapshot::compute_version(&snapshot.entries)
            }
        })
    }

    fn prune_axt_replay_ledger(&mut self, current_slot: u64) {
        let retention_slots = self.axt_timing.replay_retention_slots.get();
        let stale: Vec<_> = self
            .axt_replay_ledger
            .iter()
            .filter(|(_, entry)| entry.is_expired(current_slot, retention_slots))
            .map(|(key, _)| *key)
            .collect();
        for key in stale {
            self.axt_replay_ledger.remove(&key);
        }
    }

    fn replay_retain_until_slot(&self, handle: &AssetHandle, current_slot: u64) -> u64 {
        let expiry_slot =
            self.axt_expiry_slot_with_skew(handle.expiry_slot, handle.max_clock_skew_ms);
        let retention_cap =
            current_slot.saturating_add(self.axt_timing.replay_retention_slots.get());
        expiry_slot.max(retention_cap)
    }

    pub(crate) fn hydrate_axt_replay_ledger(&mut self, state: &impl StateReadOnly) {
        let current_slot = current_axt_slot_for_state(state);
        let retention_slots = self.axt_timing.replay_retention_slots.get();
        self.axt_replay_ledger.clear();
        for (key, entry) in state.world().axt_replay_ledger().iter() {
            if !entry.is_expired(current_slot, retention_slots) {
                self.axt_replay_ledger.insert(*key, *entry);
            }
        }
        self.prune_axt_replay_ledger(current_slot);
    }

    #[allow(clippy::too_many_lines)]
    fn validate_axt_proof(
        &mut self,
        dsid: DataSpaceId,
        proof: &ProofBlob,
        policy: AxtPolicyEntry,
    ) -> Result<(), ivm::VMError> {
        self.clear_axt_reject();
        let current_slot = Some(policy.current_slot);
        let expected_manifest = Some(policy.manifest_root);
        self.reset_axt_proof_cache_for_slot(current_slot);

        if policy.manifest_root.iter().all(|byte| *byte == 0) {
            self.record_axt_reject(
                AxtRejectReason::Manifest,
                Some(dsid),
                Some(policy.target_lane),
                "policy manifest root is zeroed",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if proof.payload.is_empty() {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "empty proof payload",
            );
            return Err(ivm::VMError::NoritoInvalid);
        }
        if proof.expiry_slot == Some(0) {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "proof expiry slot is zero",
            );
            return Err(ivm::VMError::NoritoInvalid);
        }
        let expiry_with_skew = proof
            .expiry_slot
            .map(|slot| self.axt_expiry_slot_with_skew(slot, None));
        let proof_with_skew = ProofBlob {
            payload: proof.payload.clone(),
            expiry_slot: expiry_with_skew,
        };

        let digest: [u8; 32] = Hash::new(&proof.payload).into();
        let mut cache_snapshot: Option<CachedProofEntry> = None;
        let mut cache_event: Option<&'static str> = None;
        let mut cache_hit_valid = false;
        let mut cache_hit_invalid = false;
        if let Some(entry) = self.axt_proof_cache.get_mut(&dsid) {
            if entry.digest == digest
                && entry.is_applicable_for_slot(
                    current_slot,
                    expected_manifest,
                    self.axt_timing.proof_cache_ttl_slots,
                )
            {
                entry.status = if entry.valid {
                    AXT_PROOF_CACHE_HIT
                } else {
                    AXT_PROOF_CACHE_REJECT
                };
                cache_snapshot = Some(*entry);
                cache_event = Some(entry.status);
                cache_hit_valid = entry.valid;
                cache_hit_invalid = !entry.valid;
            } else if !entry.is_applicable_for_slot(
                current_slot,
                expected_manifest,
                self.axt_timing.proof_cache_ttl_slots,
            ) {
                entry.status = AXT_PROOF_CACHE_EXPIRED;
                cache_snapshot = Some(*entry);
                cache_event = Some(AXT_PROOF_CACHE_EXPIRED);
            }
        }

        if let Some(entry) = cache_snapshot.as_ref() {
            self.note_axt_proof_cache_state(dsid, entry);
        }
        if let Some(event) = cache_event {
            self.note_axt_proof_cache_event(event);
        }
        if cache_hit_valid {
            let state = self.axt_state.as_mut().expect("axt_state checked above");
            state.record_proof(dsid, Some(proof_with_skew.clone()), current_slot)?;
            return Ok(());
        }
        if cache_hit_invalid {
            self.record_axt_reject(
                AxtRejectReason::ReplayCache,
                Some(dsid),
                Some(policy.target_lane),
                format!(
                    "proof cache rejected payload (manifest_root={})",
                    hex::encode(policy.manifest_root)
                ),
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        let proof_view = iroha_data_model::nexus::ProofBlob {
            payload: proof.payload.clone(),
            expiry_slot: proof.expiry_slot,
        };
        if !proof_matches_manifest(&proof_view, dsid, policy.manifest_root) {
            self.record_axt_reject(
                AxtRejectReason::Manifest,
                Some(dsid),
                Some(policy.target_lane),
                format!(
                    "proof does not match policy manifest_root={}",
                    hex::encode(policy.manifest_root)
                ),
            );
            self.note_axt_proof_cache_event(AXT_PROOF_CACHE_REJECT);
            return Err(ivm::VMError::PermissionDenied);
        }

        if let Some(expiry_slot) = expiry_with_skew {
            if policy.current_slot > 0 && policy.current_slot > expiry_slot {
                if let Some(entry) = self.axt_proof_cache.remove(&dsid) {
                    self.clear_axt_proof_cache_state(dsid, &entry);
                }
                self.note_axt_proof_cache_event(AXT_PROOF_CACHE_EXPIRED);
                self.record_axt_reject(
                    AxtRejectReason::Expiry,
                    Some(dsid),
                    Some(policy.target_lane),
                    format!(
                        "proof expired (policy_slot={}, expiry_slot={expiry_slot})",
                        policy.current_slot
                    ),
                );
                return Err(ivm::VMError::PermissionDenied);
            }
        }

        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_proof(dsid, Some(proof_with_skew), current_slot)?;
        self.cache_proof_entry(
            dsid,
            digest,
            expiry_with_skew,
            current_slot,
            expected_manifest,
            true,
            AXT_PROOF_CACHE_MISS,
        );
        self.note_axt_proof_cache_event(AXT_PROOF_CACHE_MISS);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn cache_proof_entry(
        &mut self,
        dsid: DataSpaceId,
        digest: [u8; 32],
        expiry_deadline_slot: Option<u64>,
        current_slot: Option<u64>,
        manifest_root: Option<[u8; 32]>,
        valid: bool,
        status: &'static str,
    ) {
        if let Some(existing) = self.axt_proof_cache.get(&dsid) {
            self.clear_axt_proof_cache_state(dsid, existing);
        }
        self.axt_proof_cache.insert(
            dsid,
            CachedProofEntry {
                digest,
                expiry_slot: expiry_deadline_slot,
                verified_slot: current_slot.unwrap_or(0),
                manifest_root,
                valid,
                status,
            },
        );
        if let Some(slot) = current_slot.filter(|value| *value > 0) {
            self.axt_proof_cache_slot = Some(slot);
        }
        if let Some(entry) = self.axt_proof_cache.get(&dsid) {
            self.note_axt_proof_cache_state(dsid, entry);
        }
    }

    #[inline]
    #[cfg_attr(not(feature = "zk-halo2-ipa"), allow(clippy::unnecessary_wraps))]
    fn compute_envelope_hash(payload: &[u8]) -> Result<[u8; 32], ivm::VMError> {
        #[cfg(feature = "zk-halo2-ipa")]
        {
            if norito::decode_from_bytes::<iroha_zkp_halo2::OpenVerifyEnvelope>(payload).is_err() {
                return Err(ivm::VMError::NoritoInvalid);
            }
        }
        Ok(iroha_crypto::Hash::new(payload).into())
    }

    fn len_to_u32(len: usize) -> Result<u32, ivm::VMError> {
        u32::try_from(len).map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn enqueue_fastpq_batch(&mut self, entries: Vec<TransferAssetBatchEntry>) {
        self.queued
            .push(InstructionBox::from(TransferAssetBatch::new(entries)));
    }

    fn flush_pending_fastpq_batch(&mut self) {
        if let Some(entries) = self.fastpq_batch_entries.take() {
            if entries.is_empty() {
                return;
            }
            self.enqueue_fastpq_batch(entries);
        }
    }

    fn begin_fastpq_batch(&mut self) -> Result<u64, ivm::VMError> {
        if self.fastpq_batch_entries.is_some() {
            return Err(ivm::VMError::PermissionDenied);
        }
        self.fastpq_batch_entries = Some(Vec::new());
        Ok(0)
    }

    fn push_fastpq_batch_entry(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        let Some(entries) = self.fastpq_batch_entries.as_mut() else {
            return Err(ivm::VMError::PermissionDenied);
        };
        let from_ptr = vm.register(10);
        let to_ptr = vm.register(11);
        let asset_def_ptr = vm.register(12);
        let amount = vm.register(13);
        let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
        let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
        let asset_def: AssetDefinitionId =
            Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
        entries.push(TransferAssetBatchEntry::new(from, to, asset_def, amount));
        Ok(0)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, ivm::VMError> {
        let Some(entries) = self.fastpq_batch_entries.take() else {
            return Err(ivm::VMError::PermissionDenied);
        };
        if entries.is_empty() {
            return Err(ivm::VMError::DecodeError);
        }
        self.enqueue_fastpq_batch(entries);
        Ok(0)
    }

    fn apply_fastpq_batch_from_tlv(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        if self.fastpq_batch_entries.is_some() {
            return Err(ivm::VMError::PermissionDenied);
        }
        let tlv = Self::expect_tlv(vm, vm.register(10), PointerType::NoritoBytes)?;
        let batch: TransferAssetBatch =
            decode_from_bytes(tlv.payload).map_err(|_| ivm::VMError::DecodeError)?;
        if batch.entries().is_empty() {
            return Err(ivm::VMError::DecodeError);
        }
        self.enqueue_fastpq_batch(batch.entries().clone());
        Ok(0)
    }

    fn handle_axt_begin(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        let ptr = vm.register(10);
        let descriptor: axt::AxtDescriptor =
            Self::decode_tlv_typed(vm, ptr, PointerType::AxtDescriptor)?;
        if let Err(err) = axt::validate_descriptor(&descriptor) {
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                None,
                None,
                format!("descriptor failed validation: {err}"),
                None,
                None,
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        let binding = axt::compute_binding(&descriptor).map_err(|_| {
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                None,
                None,
                "descriptor binding computation failed",
                None,
                None,
            );
            ivm::VMError::NoritoInvalid
        })?;
        let policy_slot = self
            .axt_policy_snapshot
            .as_ref()
            .and_then(Self::policy_current_slot);
        self.reset_axt_proof_cache_for_slot(policy_slot);
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        Ok(0)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        if self.axt_state.is_none() {
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_TOUCH invoked before AXT_BEGIN",
                None,
                None,
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        let ds_ptr = vm.register(10);
        let dsid: DataSpaceId = Self::decode_tlv_typed(vm, ds_ptr, PointerType::DataSpaceId)?;
        let manifest_ptr = vm.register(11);
        let manifest = if manifest_ptr == 0 {
            TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }
        } else {
            let manifest_tlv = Self::expect_tlv(vm, manifest_ptr, PointerType::NoritoBytes)?;
            Self::decode_header_or_bare(manifest_tlv.payload)?
        };
        if let Err(err) = self.axt_policy.allow_touch(dsid, &manifest) {
            let lane = self.policy_entry_for(dsid).map(|policy| policy.target_lane);
            self.record_axt_reject_detail(
                AxtRejectReason::PolicyDenied,
                Some(dsid),
                lane,
                "touch manifest denied by policy",
                None,
                None,
            );
            return Err(err);
        }
        let record_result = {
            let state = self.axt_state.as_mut().expect("axt_state checked above");
            state.record_touch(dsid, manifest)
        };
        if let Err(err) = record_result {
            let lane = self.policy_entry_for(dsid).map(|policy| policy.target_lane);
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                Some(dsid),
                lane,
                "touch manifest rejected by descriptor",
                None,
                None,
            );
            return Err(err);
        }
        Ok(0)
    }

    fn handle_axt_verify_ds_proof(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        if self.axt_state.is_none() {
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_VERIFY_DS_PROOF invoked before AXT_BEGIN",
                None,
                None,
            );
            return Err(ivm::VMError::PermissionDenied);
        };
        let ds_ptr = vm.register(10);
        let dsid: DataSpaceId = Self::decode_tlv_typed(vm, ds_ptr, PointerType::DataSpaceId)
            .map_err(|err| {
                #[cfg(test)]
                eprintln!("decode dsid failed: {err:?}");
                let _ = err;
                err
            })?;
        let expected = {
            let state_view = self.axt_state.as_ref().expect("axt_state checked above");
            state_view.expected_dsids().contains(&dsid)
        };
        if !expected {
            self.record_axt_reject_detail(
                AxtRejectReason::Descriptor,
                Some(dsid),
                None,
                "proof references undeclared dataspace",
                None,
                None,
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        let policy = self.policy_entry_for(dsid).ok_or_else(|| {
            self.record_axt_reject_detail(
                AxtRejectReason::MissingPolicy,
                Some(dsid),
                None,
                "no policy entry for dataspace",
                None,
                None,
            );
            ivm::VMError::PermissionDenied
        })?;
        self.reset_axt_proof_cache_for_slot(Some(policy.current_slot));
        let proof_ptr = vm.register(11);
        if proof_ptr == 0 {
            let state = self.axt_state.as_mut().expect("axt_state checked above");
            state.record_proof(dsid, None, Some(policy.current_slot))?;
            if let Some(entry) = self.axt_proof_cache.remove(&dsid) {
                self.clear_axt_proof_cache_state(dsid, &entry);
            }
            self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
            return Ok(0);
        }
        let proof_tlv = Self::expect_tlv(vm, proof_ptr, PointerType::ProofBlob)?;
        let mut proof_payload = proof_tlv.payload;
        #[cfg(test)]
        eprintln!("proof payload len {}", proof_payload.len());
        let proof: ProofBlob = decode_from_bytes(proof_payload)
            .or_else(|_| ProofBlob::decode(&mut proof_payload))
            .map_err(|err| {
                #[cfg(test)]
                eprintln!("decode proof failed: {err:?}");
                let _ = err;
                self.record_axt_reject_detail(
                    AxtRejectReason::Proof,
                    Some(dsid),
                    Some(policy.target_lane),
                    "proof payload failed to decode",
                    None,
                    None,
                );
                ivm::VMError::NoritoInvalid
            })?;
        self.validate_axt_proof(dsid, &proof, policy)?;
        Ok(0)
    }

    #[allow(clippy::too_many_lines)]
    fn enforce_axt_policy(&mut self, usage: &axt::HandleUsage) -> Result<(), ivm::VMError> {
        let dsid = usage.intent.asset_dsid;
        let model_usage = AxtHandleFragment::try_from(usage)?;
        let key = AxtHandleReplayKey::from_handle(&model_usage.handle);
        let mut policy_bounds: Option<(u64, u64)> = None;
        let mut policy_lane: Option<LaneId> = None;
        let mut record_slot: u64 = 0;
        if let Some(snapshot) = &self.axt_policy_snapshot {
            let binding = snapshot.entries.iter().find(|entry| entry.dsid == dsid);
            let Some(binding) = binding else {
                self.record_axt_reject_detail(
                    AxtRejectReason::MissingPolicy,
                    Some(dsid),
                    Some(usage.handle.target_lane),
                    "no policy entry for dataspace",
                    None,
                    None,
                );
                return Err(ivm::VMError::PermissionDenied);
            };
            let policy = &binding.policy;
            policy_bounds = Some((policy.min_handle_era, policy.min_sub_nonce));
            policy_lane = Some(policy.target_lane);
            record_slot = policy.current_slot;
            let policy_root_zeroed = policy.manifest_root.iter().all(|byte| *byte == 0);
            let handle_root_zeroed = usage
                .handle
                .manifest_view_root
                .iter()
                .all(|byte| *byte == 0);

            let mut rejection: Option<(AxtRejectReason, String, Option<u64>, Option<u64>)> = None;
            if policy_root_zeroed || handle_root_zeroed {
                rejection = Some((
                    AxtRejectReason::Manifest,
                    "policy or handle manifest root is zeroed".to_owned(),
                    Some(policy.min_handle_era),
                    Some(policy.min_sub_nonce),
                ));
            } else if policy.target_lane != usage.handle.target_lane {
                rejection = Some((
                    AxtRejectReason::Lane,
                    format!(
                        "handle lane {} does not match policy lane {}",
                        usage.handle.target_lane.as_u32(),
                        policy.target_lane.as_u32()
                    ),
                    Some(policy.min_handle_era),
                    Some(policy.min_sub_nonce),
                ));
            } else if policy.manifest_root.as_slice() != usage.handle.manifest_view_root.as_slice()
            {
                rejection = Some((
                    AxtRejectReason::Manifest,
                    format!(
                        "handle manifest root {} does not match policy {}",
                        hex::encode(&usage.handle.manifest_view_root),
                        hex::encode(policy.manifest_root)
                    ),
                    Some(policy.min_handle_era),
                    Some(policy.min_sub_nonce),
                ));
            } else if usage.handle.handle_era < policy.min_handle_era {
                rejection = Some((
                    AxtRejectReason::HandleEra,
                    format!(
                        "handle era {} below policy minimum {}",
                        usage.handle.handle_era, policy.min_handle_era
                    ),
                    Some(policy.min_handle_era),
                    Some(policy.min_sub_nonce),
                ));
            } else if usage.handle.sub_nonce < policy.min_sub_nonce {
                rejection = Some((
                    AxtRejectReason::SubNonce,
                    format!(
                        "handle sub-nonce {} below policy minimum {}",
                        usage.handle.sub_nonce, policy.min_sub_nonce
                    ),
                    Some(policy.min_handle_era),
                    Some(policy.min_sub_nonce),
                ));
            } else {
                let requested_skew_ms = usage
                    .handle
                    .max_clock_skew_ms
                    .map_or(self.axt_timing.max_clock_skew_ms, u64::from);
                if requested_skew_ms > self.axt_timing.max_clock_skew_ms {
                    rejection = Some((
                        AxtRejectReason::Expiry,
                        format!(
                            "handle requested max_clock_skew_ms={} exceeding configured bound {}",
                            requested_skew_ms, self.axt_timing.max_clock_skew_ms
                        ),
                        Some(policy.min_handle_era),
                        Some(policy.min_sub_nonce),
                    ));
                } else {
                    let expiry_slot = self.axt_expiry_slot_with_skew(
                        usage.handle.expiry_slot,
                        usage.handle.max_clock_skew_ms,
                    );
                    if policy.current_slot > 0 && policy.current_slot > expiry_slot {
                        rejection = Some((
                            AxtRejectReason::Expiry,
                            format!(
                                "handle expired for current policy slot={} (expiry_slot={expiry_slot})",
                                policy.current_slot
                            ),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }
                }
            }

            if let Some((reason, detail, next_handle_era, next_sub_nonce)) = rejection {
                self.record_axt_reject_detail(
                    reason,
                    Some(dsid),
                    Some(policy.target_lane),
                    detail,
                    next_handle_era,
                    next_sub_nonce,
                );
                return Err(ivm::VMError::PermissionDenied);
            }
        }

        if record_slot == 0 {
            record_slot = self
                .axt_policy_snapshot
                .as_ref()
                .and_then(Self::policy_current_slot)
                .unwrap_or(0);
        }
        let retention_slots = self.axt_timing.replay_retention_slots.get();
        self.prune_axt_replay_ledger(record_slot);
        if let Some(entry) = self.axt_replay_ledger.get(&key)
            && !entry.is_expired(record_slot, retention_slots)
        {
            let (next_handle_era, next_sub_nonce) =
                policy_bounds.map_or((None, None), |(era, sub)| (Some(era), Some(sub)));
            let lane = policy_lane.or(Some(usage.handle.target_lane));
            self.record_axt_reject_detail(
                AxtRejectReason::ReplayCache,
                Some(dsid),
                lane,
                format!(
                    "handle replay detected (used_slot={}, retain_until_slot={})",
                    entry.used_slot, entry.retain_until_slot
                ),
                next_handle_era,
                next_sub_nonce,
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        let result = self.axt_policy.allow_handle(usage);
        if result.is_err() {
            let (next_handle_era, next_sub_nonce) =
                policy_bounds.map_or((None, None), |(era, sub)| (Some(era), Some(sub)));
            let lane = policy_lane.or(Some(usage.handle.target_lane));
            self.record_axt_reject_detail(
                AxtRejectReason::PolicyDenied,
                Some(dsid),
                lane,
                "policy hook denied handle usage",
                next_handle_era,
                next_sub_nonce,
            );
        }
        if result.is_ok() {
            let retain_until_slot = self.replay_retain_until_slot(&usage.handle, record_slot);
            self.axt_replay_ledger.insert(
                key,
                AxtReplayRecord {
                    dataspace: dsid,
                    used_slot: record_slot,
                    retain_until_slot,
                },
            );
        }
        result
    }

    #[allow(clippy::too_many_lines)]
    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        if self.axt_state.is_none() {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_USE_ASSET_HANDLE invoked before AXT_BEGIN",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        let handle_ptr = vm.register(10);
        let handle_tlv = Self::expect_tlv(vm, handle_ptr, PointerType::AssetHandle)?;
        let handle: AssetHandle = Self::decode_header_or_bare(handle_tlv.payload)?;
        let binding = handle.binding_array().ok_or_else(|| {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                Some(handle.target_lane),
                "handle binding is not 32 bytes",
            );
            ivm::VMError::NoritoInvalid
        })?;
        if handle.manifest_view_root.len() != 32 {
            self.record_axt_reject(
                AxtRejectReason::Manifest,
                None,
                Some(handle.target_lane),
                "handle manifest root must be 32 bytes",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        let (state_binding, dsid_expected, has_touch) = {
            let state_ref = self.axt_state.as_ref().expect("axt_state checked above");
            (
                state_ref.binding(),
                state_ref.expected_dsids().contains(&intent.asset_dsid),
                state_ref.has_touch(&intent.asset_dsid),
            )
        };
        if binding != state_binding {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                Some(handle.target_lane),
                "handle binding does not match descriptor",
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        let intent_ptr = vm.register(11);
        let intent_tlv = Self::expect_tlv(vm, intent_ptr, PointerType::NoritoBytes)?;
        let intent: RemoteSpendIntent = Self::decode_header_or_bare(intent_tlv.payload)?;
        if !dsid_expected {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "intent references undeclared dataspace",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if !has_touch {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "missing AXT_TOUCH for dataspace",
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        if handle.handle_era == 0 {
            self.record_axt_reject(
                AxtRejectReason::HandleEra,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle era is zero",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle.sub_nonce == 0 {
            self.record_axt_reject(
                AxtRejectReason::SubNonce,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle sub-nonce is zero",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle.expiry_slot == 0 {
            self.record_axt_reject(
                AxtRejectReason::Expiry,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle expiry slot is zero",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle.scope.is_empty() {
            self.record_axt_reject(
                AxtRejectReason::PolicyDenied,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle scope is empty",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle
            .scope
            .iter()
            .all(|scope| scope != &intent.op.kind)
        {
            self.record_axt_reject(
                AxtRejectReason::PolicyDenied,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle scope does not permit intent kind",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle.subject.account != intent.op.from {
            self.record_axt_reject(
                AxtRejectReason::PolicyDenied,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle subject does not match intent sender",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if handle.group_binding.composability_group_id.is_empty() {
            self.record_axt_reject(
                AxtRejectReason::PolicyDenied,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle composability group id is empty",
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        let amount = intent
            .op
            .amount
            .parse::<u128>()
            .map_err(|_| {
                self.record_axt_reject(
                    AxtRejectReason::Budget,
                    Some(intent.asset_dsid),
                    Some(handle.target_lane),
                    "intent amount is not a valid u128",
                );
                ivm::VMError::NoritoInvalid
            })?;
        if amount == 0 {
            self.record_axt_reject(
                AxtRejectReason::Budget,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                "handle amount must be non-zero",
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        if amount > handle.budget.remaining {
            self.record_axt_reject(
                AxtRejectReason::Budget,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                format!(
                    "handle budget exceeded (requested={}, remaining={})",
                    amount, handle.budget.remaining
                ),
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        if let Some(per_use) = handle.budget.per_use
            && amount > per_use
        {
            self.record_axt_reject(
                AxtRejectReason::Budget,
                Some(intent.asset_dsid),
                Some(handle.target_lane),
                format!("per-use budget exceeded (requested={amount}, per_use={per_use})"),
            );
            return Err(ivm::VMError::PermissionDenied);
        }

        let proof_ptr = vm.register(12);
        let proof = if proof_ptr == 0 {
            None
        } else {
            let proof_tlv = Self::expect_tlv(vm, proof_ptr, PointerType::ProofBlob)?;
            let blob: ProofBlob = Self::decode_header_or_bare(proof_tlv.payload).map_err(|err| {
                self.record_axt_reject(
                    AxtRejectReason::Proof,
                    Some(intent.asset_dsid),
                    Some(handle.target_lane),
                    "proof payload failed to decode",
                );
                err
            })?;
            Some(blob)
        };
        if let Some(blob) = proof.as_ref() {
            let policy = self.policy_entry_for(intent.asset_dsid).ok_or_else(|| {
                self.record_axt_reject(
                    AxtRejectReason::MissingPolicy,
                    Some(intent.asset_dsid),
                    Some(handle.target_lane),
                    "no policy entry for dataspace",
                );
                ivm::VMError::PermissionDenied
            })?;
            self.validate_axt_proof(intent.asset_dsid, blob, policy)?;
        }

        let usage = axt::HandleUsage {
            handle,
            intent,
            proof,
            amount,
        };
        self.enforce_axt_policy(&usage)?;
        let state = self.axt_state.as_mut().expect("axt_state checked above");
        let usage_for_logging = usage.clone();
        if let Err(err) = state.record_handle(usage) {
            self.record_axt_reject(
                AxtRejectReason::Budget,
                Some(usage_for_logging.intent.asset_dsid),
                Some(usage_for_logging.handle.target_lane),
                format!("handle recording failed: {err:?}"),
            );
            return Err(err);
        }
        Ok(0)
    }

    fn handle_axt_commit(&mut self) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        let state = self.axt_state.take().ok_or_else(|| {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_COMMIT invoked before AXT_BEGIN",
            );
            ivm::VMError::PermissionDenied
        })?;
        if let Some(analysis) = &self.amx_analysis {
            if let Some(ds_count) = NonZeroUsize::new(state.expected_dsids().len()) {
                if let Err(err) = analysis::enforce_amx_budget(analysis, ds_count, &self.amx_limits)
                {
                    let dataspace = state
                        .expected_dsids()
                        .first()
                        .copied()
                        .unwrap_or_else(|| DataSpaceId::new(0));
                    let (elapsed_ns, budget_ns) = match err {
                        analysis::AmxBudgetError::PerDataspaceBudgetExceeded {
                            estimated_ns,
                            limit_ns,
                        }
                        | analysis::AmxBudgetError::GroupBudgetExceeded {
                            estimated_ns,
                            limit_ns,
                        } => (estimated_ns, limit_ns),
                    };
                    self.amx_budget_violation = Some(AmxBudgetViolation {
                        dataspace,
                        stage: AmxStage::Prepare,
                        elapsed_ms: nanoseconds_to_millis(elapsed_ns),
                        budget_ms: nanoseconds_to_millis(budget_ns),
                    });
                    #[cfg(feature = "telemetry")]
                    if let Some(telemetry) = self.telemetry.as_ref() {
                        let lane = state
                            .handles()
                            .first()
                            .map_or_else(|| LaneId::new(0), |h| h.handle.target_lane);
                        telemetry.inc_amx_abort(lane, "budget");
                    }
                    let dataspace = state
                        .expected_dsids()
                        .iter()
                        .copied()
                        .next()
                        .unwrap_or(DataSpaceId::GLOBAL);
                    let (elapsed_ms_ceil, budget_ms_ceil) = match err {
                        analysis::AmxBudgetError::PerDataspaceBudgetExceeded {
                            estimated_ns,
                            limit_ns,
                        }
                        | analysis::AmxBudgetError::GroupBudgetExceeded {
                            estimated_ns,
                            limit_ns,
                        } => {
                            let ms = |ns: u64| ns.saturating_add(999_999) / 1_000_000;
                            (ms(estimated_ns), ms(limit_ns))
                        }
                    };
                    self.axt_state = Some(state);
                    return Err(ivm::VMError::AmxBudgetExceeded {
                        dataspace,
                        stage: iroha_data_model::errors::AmxStage::Commit,
                        elapsed_ms: elapsed_ms_ceil,
                        budget_ms: budget_ms_ceil,
                    });
                }
            }
        }
        match state.validate_commit() {
            Ok(()) => {
                self.completed_axt.push(state);
                Ok(0)
            }
            Err(err) => {
                self.axt_state = Some(state);
                Err(err)
            }
        }
    }

    fn materialize_axt_record(
        state: &axt::HostAxtState,
        lane: LaneId,
        commit_height: u64,
    ) -> AxtEnvelopeRecord {
        let descriptor = ModelAxtDescriptor {
            dsids: state.descriptor().dsids.clone(),
            touches: state
                .descriptor()
                .touches
                .iter()
                .map(|touch| ModelAxtTouchSpec {
                    dsid: touch.dsid,
                    read: touch.read.clone(),
                    write: touch.write.clone(),
                })
                .collect(),
        };
        let binding = AxtBinding::new(state.binding());

        let mut touches: Vec<AxtTouchFragment> = state
            .touches()
            .iter()
            .map(|(dsid, manifest)| AxtTouchFragment {
                dsid: *dsid,
                manifest: ModelTouchManifest {
                    read: manifest.read.clone(),
                    write: manifest.write.clone(),
                },
            })
            .collect();
        touches.sort_by_key(|fragment| fragment.dsid);

        let mut proofs: Vec<AxtProofFragment> = state
            .proofs()
            .iter()
            .map(|(dsid, proof)| AxtProofFragment {
                dsid: *dsid,
                proof: ModelProofBlob {
                    payload: proof.payload.clone(),
                    expiry_slot: proof.expiry_slot,
                },
            })
            .collect();
        proofs.sort_by_key(|fragment| fragment.dsid);

        let mut handles: Vec<AxtHandleFragment> = state.handle_fragments().to_vec();
        handles.sort_by_key(|fragment| {
            (
                fragment.handle.axt_binding.as_bytes().to_owned(),
                fragment.handle.handle_era,
                fragment.handle.sub_nonce,
                fragment.intent.asset_dsid,
                fragment.amount,
            )
        });

        AxtEnvelopeRecord {
            binding,
            lane,
            descriptor,
            touches,
            proofs,
            handles,
            commit_height: Some(commit_height),
        }
    }

    fn flush_durable_state(&mut self, tx: &mut StateTransaction<'_, '_>) {
        if self.durable_state_overlay.is_empty() {
            return;
        }
        for (path, value) in &self.durable_state_overlay {
            if let Some(stored) = value {
                tx.world
                    .smart_contract_state
                    .insert(path.clone(), stored.clone());
                self.durable_state_base.insert(path.clone(), stored.clone());
            } else {
                tx.world.smart_contract_state.remove(path.clone());
                self.durable_state_base.remove(path);
            }
        }
        self.durable_state_overlay.clear();
    }

    fn flush_completed_axt(&mut self, tx: &mut StateTransaction<'_, '_>) {
        self.amx_budget_violation = None;
        if self.completed_axt.is_empty() {
            return;
        }
        let lane = tx.current_lane_id.unwrap_or_else(|| LaneId::new(0));
        let commit_height = tx.block_height();

        // Materialize and persist each completed envelope into the block-level accumulator.
        let envelopes: Vec<_> = self
            .completed_axt
            .drain(..)
            .map(|state| Self::materialize_axt_record(&state, lane, commit_height))
            .collect();
        for envelope in envelopes {
            tx.record_axt_envelope(envelope);
        }
    }

    /// Execute a closure with a mutable reference to the [`CoreHost`] attached to `vm`.
    ///
    /// Used in tests to access the host state without juggling raw pointers.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn with_host<R>(vm: &mut IVM, f: impl FnOnce(&mut CoreHost) -> R) -> R {
        let host_any = vm.host_mut_any().expect("CoreHost not attached to VM");
        let host = host_any
            .downcast_mut::<CoreHost>()
            .expect("host type mismatch: expected CoreHost");
        f(host)
    }
}

impl IVMHost for CoreHost {
    #[allow(clippy::too_many_lines)]
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        // Enforce central syscall policy (ABI-versioned) uniformly across hosts.
        if !ivm::syscalls::is_syscall_allowed(vm.syscall_policy(), number) {
            return Err(ivm::VMError::UnknownSyscall(number));
        }
        match number {
            // ----------------- Domain ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_REGISTER_DOMAIN => {
                let ptr = vm.register(10);
                let id: DomainId = Self::decode_tlv_typed(vm, ptr, PointerType::DomainId)?;
                let isi = Register::domain(Domain::new(id));
                self.queued
                    .push(InstructionBox::from(RegisterBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_UNREGISTER_DOMAIN => {
                let ptr = vm.register(10);
                let id: DomainId = Self::decode_tlv_typed(vm, ptr, PointerType::DomainId)?;
                let isi = Unregister::domain(id);
                self.queued
                    .push(InstructionBox::from(UnregisterBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_TRANSFER_DOMAIN => {
                // r10=&DomainId, r11=&AccountId(to)
                let dptr = vm.register(10);
                let tptr = vm.register(11);
                let id: DomainId = Self::decode_tlv_typed(vm, dptr, PointerType::DomainId)?;
                let to: AccountId = Self::decode_tlv_typed(vm, tptr, PointerType::AccountId)?;
                let from = self.authority.clone();
                let isi = Transfer::domain(from, id, to);
                self.queued
                    .push(InstructionBox::from(TransferBox::from(isi)));
                Ok(0)
            }
            // ----------------- Asset (Numeric) ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_MINT_ASSET => {
                let account_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let mut amount = vm.register(12);

                // Pointer-ABI: if the pointer lies in the INPUT region use it,
                // otherwise interpret sentinel values for minimal test samples.
                let account: AccountId = if account_ptr >= ivm::Memory::INPUT_START {
                    // Strict typed TLV (AccountId)
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?
                } else {
                    // Sentinel 0 => current authority
                    if account_ptr == 0 {
                        self.authority.clone()
                    } else {
                        return Err(ivm::VMError::DecodeError);
                    }
                };
                let asset_def: AssetDefinitionId = if asset_def_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?
                } else {
                    // Sentinel 0 => predefined sample: "rose#wonderland"
                    if asset_def_ptr == 0 {
                        "rose#wonderland"
                            .parse()
                            .map_err(|_| ivm::VMError::DecodeError)?
                    } else {
                        return Err(ivm::VMError::DecodeError);
                    }
                };
                let asset_id = AssetId::of(asset_def, account);
                // If amount is a sentinel (0), try to take it from trigger args.
                if let Some(args) = &self.args
                    && let Ok(value) = args.try_into_any_norito::<norito::json::Value>()
                    && let Some(v) = value.get("val").and_then(norito::json::Value::as_u64)
                {
                    amount = v;
                }
                let isi = Mint::asset_numeric(amount, asset_id);
                self.queued.push(InstructionBox::from(MintBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_BURN_ASSET => {
                let account_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let amount = vm.register(12);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let asset_id = AssetId::of(asset_def, account);
                let isi = Burn::asset_numeric(amount, asset_id);
                self.queued.push(InstructionBox::from(BurnBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_TRANSFER_ASSET => {
                if self.fastpq_batch_entries.is_some() {
                    return self.push_fastpq_batch_entry(vm);
                }
                let from_ptr = vm.register(10);
                let to_ptr = vm.register(11);
                let asset_def_ptr = vm.register(12);
                let amount = vm.register(13);
                let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
                let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let asset_id = AssetId::of(asset_def, from);
                let isi = Transfer::asset_numeric(asset_id, amount, to);
                self.queued
                    .push(InstructionBox::from(TransferBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch_from_tlv(vm),
            // Account metadata (SetKeyValue<Account>)
            ivm::syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                let account_ptr = vm.register(10);
                let key_ptr = vm.register(11);
                let value_ptr = vm.register(12);

                let account: AccountId = if account_ptr >= ivm::Memory::INPUT_START {
                    self.decode_at(vm, account_ptr)?
                } else if account_ptr == 0 {
                    self.authority.clone()
                } else {
                    return Err(ivm::VMError::DecodeError);
                };

                let key: Name = if key_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, key_ptr, PointerType::Name)?
                } else if key_ptr == 0 {
                    "cursor".parse().map_err(|_| ivm::VMError::DecodeError)?
                } else {
                    return Err(ivm::VMError::DecodeError);
                };

                let value: Json = if value_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_json(vm, value_ptr)?
                } else if value_ptr == 0 {
                    // Minimal ForwardCursor value so client-side deserialization succeeds.
                    let fc = ForwardCursor {
                        query: "sc_dummy".to_string(),
                        cursor: nonzero_ext::nonzero!(1_u64),
                        gas_budget: None,
                    };
                    Json::new(fc)
                } else {
                    return Err(ivm::VMError::DecodeError);
                };

                let isi = SetKeyValue::account(account, key, value);
                self.queued
                    .push(InstructionBox::from(SetKeyValueBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE => {
                let ptr = vm.register(10);
                // Decode manifest registration request from Norito-encoded TLV bytes.
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let request: scode::RegisterSmartContractCode =
                    norito::decode_from_bytes(tlv.payload)
                        .map_err(|_| ivm::VMError::DecodeError)?;
                self.queued.push(InstructionBox::from(request));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES => {
                let ptr = vm.register(10);
                let request: scode::RegisterSmartContractBytes =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                self.queued.push(InstructionBox::from(request));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE => {
                let ptr = vm.register(10);
                let request: scode::ActivateContractInstance =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                self.queued.push(InstructionBox::from(request));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE => {
                let ptr = vm.register(10);
                let request: scode::DeactivateContractInstance =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                self.queued.push(InstructionBox::from(request));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES => {
                let ptr = vm.register(10);
                let request: scode::RemoveSmartContractBytes =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                self.queued.push(InstructionBox::from(request));
                Ok(0)
            }
            // ----------------- NFT (Non-fungible) ISIs -----------------
            ivm::syscalls::SYSCALL_NFT_MINT_ASSET => {
                let nft_id_ptr = vm.register(10);
                let owner_ptr = vm.register(11);
                let owner: AccountId = if owner_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, owner_ptr, PointerType::AccountId)?
                } else if owner_ptr == 0 {
                    self.authority.clone()
                } else {
                    return Err(ivm::VMError::DecodeError);
                };
                let nft_id: NftId = if nft_id_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?
                } else if nft_id_ptr == 0 {
                    // Auto-generate a unique NFT id for the current authority with a monotonic suffix
                    let name = format!("nft_number_{}_for_{}", self.nft_seq, owner.signatory())
                        .parse()
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    self.nft_seq = self.nft_seq.saturating_add(1);
                    NftId::of(owner.domain().clone(), name)
                } else {
                    return Err(ivm::VMError::DecodeError);
                };
                let nft = Nft::new(nft_id, Metadata::default());
                self.queued
                    .push(InstructionBox::from(RegisterBox::from(Register::nft(nft))));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                let from_ptr = vm.register(10);
                let nft_id_ptr = vm.register(11);
                let to_ptr = vm.register(12);
                let from: AccountId = if from_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?
                } else if from_ptr == 0 {
                    self.authority.clone()
                } else {
                    return Err(ivm::VMError::DecodeError);
                };
                let nft_id: NftId = if nft_id_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?
                } else {
                    return Err(ivm::VMError::DecodeError);
                };
                let to: AccountId = if to_ptr >= ivm::Memory::INPUT_START {
                    Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?
                } else {
                    return Err(ivm::VMError::DecodeError);
                };
                let isi = Transfer::nft(from, nft_id, to);
                self.queued
                    .push(InstructionBox::from(TransferBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_NFT_SET_METADATA => {
                let nft_id_ptr = vm.register(10);
                let key_ptr = vm.register(11);
                let value_ptr = vm.register(12);

                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let key: Name = Self::decode_tlv_typed(vm, key_ptr, PointerType::Name)?;
                let value: Json = Self::decode_tlv_json(vm, value_ptr)?;

                let isi = SetKeyValue::nft(nft_id, key, value);
                self.queued
                    .push(InstructionBox::from(SetKeyValueBox::from(isi)));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_NFT_BURN_ASSET => {
                let nft_id_ptr = vm.register(10);
                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let isi = Unregister::nft(nft_id);
                self.queued
                    .push(InstructionBox::from(UnregisterBox::from(isi)));
                Ok(0)
            }
            // Sample convenience: create one NFT per known account (from snapshot)
            ivm::syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS => {
                let mut i = self.nft_seq;
                let start = i;
                for account_id in self.accounts_snapshot.iter() {
                    let name_str = format!("nft_number_{}_for_{}", i, account_id.signatory());
                    if let Ok(name) = name_str.parse() {
                        let nft_id = NftId::of(account_id.domain().clone(), name);
                        let nft = Nft::new(nft_id.clone(), Metadata::default());
                        self.queued
                            .push(InstructionBox::from(RegisterBox::from(Register::nft(nft))));
                        self.queued
                            .push(InstructionBox::from(TransferBox::from(Transfer::nft(
                                self.authority.clone(),
                                nft_id,
                                account_id.clone(),
                            ))));
                        i = i.saturating_add(1);
                        if i.saturating_sub(start) >= 256 {
                            break;
                        }
                    }
                }
                self.nft_seq = i;
                Ok(0)
            }
            // Set SmartContract execution depth parameter to x10
            ivm::syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH => {
                let depth_raw = vm.register(10);
                // If zero, treat as no-op to avoid setting an invalid value
                if depth_raw == 0 {
                    return Ok(0);
                }
                // SmartContractParameter::ExecutionDepth expects a u8.
                let depth: u8 = u8::try_from(depth_raw).unwrap_or(u8::MAX);
                let param = Parameter::SmartContract(
                    iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(depth),
                );
                self.queued
                    .push(InstructionBox::from(SetParameter::new(param)));
                Ok(0)
            }
            // Reserved for future smart-contract helpers
            // Accept a Norito-encoded InstructionBox and enqueue it for later execution.
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION => {
                let p = vm.register(10);
                let tlv = vm.memory.validate_tlv(p)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let ib: iroha_data_model::isi::InstructionBox =
                    norito::decode_from_bytes(tlv.payload)
                        .map_err(|_| ivm::VMError::NoritoInvalid)?;
                // Only enqueue supported ZK ISIs via this vendor syscall for now.
                // If present, attach the last verify envelope hash to the proof attachment
                // to bind it to the transaction call_hash via emitted events/metadata.
                let any_ref = ib.as_any();
                if let Some(z) = any_ref.downcast_ref::<DMZk::ZkTransfer>() {
                    let mut pa = z.proof.clone();
                    if pa.envelope_hash.is_none()
                        && let Some(h) = self.zk_last_env_hash_transfer.pop_front()
                    {
                        pa.envelope_hash = Some(h);
                    }
                    let new = DMZk::ZkTransfer {
                        asset: z.asset.clone(),
                        inputs: z.inputs.clone(),
                        outputs: z.outputs.clone(),
                        proof: pa,
                        root_hint: z.root_hint,
                    };
                    self.queued.push(new.into());
                    Ok(0)
                } else if let Some(u) = any_ref.downcast_ref::<DMZk::Unshield>() {
                    let mut pa = u.proof.clone();
                    if pa.envelope_hash.is_none()
                        && let Some(h) = self.zk_last_env_hash_unshield.pop_front()
                    {
                        pa.envelope_hash = Some(h);
                    }
                    let new = DMZk::Unshield {
                        asset: u.asset.clone(),
                        to: u.to.clone(),
                        public_amount: *u.public_amount(),
                        inputs: u.inputs.clone(),
                        proof: pa,
                        root_hint: u.root_hint,
                    };
                    self.queued.push(new.into());
                    Ok(0)
                } else if let Some(sb) = any_ref.downcast_ref::<DMZk::SubmitBallot>() {
                    let mut pa = sb.ballot_proof.clone();
                    if pa.envelope_hash.is_none()
                        && let Some(h) = self.zk_last_env_hash_ballot.pop_front()
                    {
                        pa.envelope_hash = Some(h);
                    }
                    let new = DMZk::SubmitBallot {
                        election_id: sb.election_id.clone(),
                        ciphertext: sb.ciphertext.clone(),
                        ballot_proof: pa,
                        nullifier: sb.nullifier,
                    };
                    self.queued.push(new.into());
                    Ok(0)
                } else if let Some(ft) = any_ref.downcast_ref::<DMZk::FinalizeElection>() {
                    let mut pa = ft.tally_proof.clone();
                    if pa.envelope_hash.is_none()
                        && let Some(h) = self.zk_last_env_hash_tally.pop_front()
                    {
                        pa.envelope_hash = Some(h);
                    }
                    let new = DMZk::FinalizeElection {
                        election_id: ft.election_id.clone(),
                        tally: ft.tally.clone(),
                        tally_proof: pa,
                    };
                    self.queued.push(new.into());
                    Ok(0)
                } else if any_ref.downcast_ref::<DMZk::VerifyProof>().is_some() {
                    // Allow explicit VerifyProof if issued via vendor bridge; pass-through
                    self.queued.push(ib);
                    Ok(0)
                } else {
                    Err(ivm::VMError::PermissionDenied)
                }
            }
            // Helper syscalls forwarded to the default host implementation.
            // Intercept ZK_VERIFY syscalls to record success and gate ZK ISIs.
            ivm::syscalls::SYSCALL_ZK_VERIFY_TRANSFER => {
                // Capture TLV payload hash (envelope hash) before delegating to default host
                // and enforce VK/label/domain binding.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let env_hash = Self::compute_envelope_hash(tlv.payload)?;
                if let Err(code) =
                    self.enforce_zk_envelope(tlv.payload, "zk_verify_transfer/v1", "transfer")
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, code);
                    return Ok(0);
                }
                let _ = self.default.syscall(number, vm)?;
                if vm.register(10) != 0 {
                    self.zk_verified_transfer.push_back(env_hash);
                    self.zk_last_env_hash_transfer.push_back(env_hash);
                }
                Ok(0)
            }
            ivm::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let env_hash = Self::compute_envelope_hash(tlv.payload)?;
                if let Err(code) =
                    self.enforce_zk_envelope(tlv.payload, "zk_verify_unshield/v1", "unshield")
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, code);
                    return Ok(0);
                }
                let _ = self.default.syscall(number, vm)?;
                if vm.register(10) != 0 {
                    self.zk_verified_unshield.push_back(env_hash);
                    self.zk_last_env_hash_unshield.push_back(env_hash);
                }
                Ok(0)
            }
            ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let env_hash = Self::compute_envelope_hash(tlv.payload)?;
                if let Err(code) =
                    self.enforce_zk_envelope(tlv.payload, "zk_verify_ballot/v1", "ballot")
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, code);
                    return Ok(0);
                }
                let _ = self.default.syscall(number, vm)?;
                if vm.register(10) != 0 {
                    self.zk_verified_ballot.push_back(env_hash);
                    self.zk_last_env_hash_ballot.push_back(env_hash);
                }
                Ok(0)
            }
            ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let env_hash = Self::compute_envelope_hash(tlv.payload)?;
                if let Err(code) =
                    self.enforce_zk_envelope(tlv.payload, "zk_verify_tally/v1", "tally")
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, code);
                    return Ok(0);
                }
                let _ = self.default.syscall(number, vm)?;
                if vm.register(10) != 0 {
                    self.zk_verified_tally.push_back(env_hash);
                    self.zk_last_env_hash_tally.push_back(env_hash);
                }
                Ok(0)
            }
            // ZK roots read: build response from snapshot and return TLV pointer in r10
            ivm::syscalls::SYSCALL_ZK_ROOTS_GET => {
                let ptr = vm.register(10);
                let req: ivm::zk_verify::RootsGetRequest =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let ad: AssetDefinitionId = req
                    .asset_id
                    .parse()
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                let roots_all = self.zk_roots.get(&ad).cloned().unwrap_or_default();
                let height = Self::len_to_u32(roots_all.len())?;
                // Optionally synthesize an explicit empty-tree root when no commitments exist.
                let latest = if roots_all.is_empty() && self.zk_include_empty_root_when_empty {
                    iroha_crypto::MerkleTree::<[u8; 32]>::shielded_empty_root(
                        self.zk_empty_root_depth,
                    )
                } else {
                    roots_all.last().copied().unwrap_or([0u8; 32])
                };
                let want = if req.max == 0 {
                    self.zk_root_history_cap
                } else {
                    core::cmp::min(self.zk_root_history_cap, req.max as usize)
                };
                let roots = if roots_all.is_empty() && self.zk_include_empty_root_when_empty {
                    if want > 0 { vec![latest] } else { Vec::new() }
                } else if roots_all.len() <= want {
                    roots_all
                } else {
                    roots_all[roots_all.len() - want..].to_vec()
                };
                let resp = ivm::zk_verify::RootsGetResponse {
                    latest,
                    roots,
                    height,
                };
                let body = norito::to_bytes(&resp).map_err(|_| ivm::VMError::NoritoInvalid)?;
                let body_len = Self::len_to_u32(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&body_len.to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            // ZK vote tally read: respond from elections snapshot
            ivm::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                let ptr = vm.register(10);
                let req: ivm::zk_verify::VoteGetTallyRequest =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let (finalized, tally) = match self.zk_elections.get(&req.election_id) {
                    Some((f, t)) => (*f, t.clone()),
                    None => (false, Vec::new()),
                };
                let resp = ivm::zk_verify::VoteGetTallyResponse { finalized, tally };
                let body = norito::to_bytes(&resp).map_err(|_| ivm::VMError::NoritoInvalid)?;
                let body_len = Self::len_to_u32(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&body_len.to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            // SM helper syscalls are gated by crypto configuration and forwarded to DefaultHost.
            ivm::syscalls::SYSCALL_SM3_HASH
            | ivm::syscalls::SYSCALL_SM2_VERIFY
            | ivm::syscalls::SYSCALL_SM4_GCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_GCM_OPEN
            | ivm::syscalls::SYSCALL_SM4_CCM_SEAL
            | ivm::syscalls::SYSCALL_SM4_CCM_OPEN => {
                if !self.crypto.sm_helpers_enabled() {
                    return Err(ivm::VMError::PermissionDenied);
                }
                let result = self.default.syscall(number, vm);
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = self.telemetry.as_ref() {
                    Self::record_sm_syscall(telemetry, number, &result);
                }
                result
            }
            // Durable state syscalls backed by WSV.
            ivm::syscalls::SYSCALL_STATE_GET => {
                let path_ptr = vm.register(10);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_read_key(path.as_ref());
                if let Some(entry) = self.durable_state_overlay.get(&path) {
                    match entry {
                        Some(stored) => Self::load_state_value(vm, stored)?,
                        None => vm.set_register(10, 0),
                    }
                    return Ok(0);
                }
                if let Some(stored) = self.durable_state_base.get(&path) {
                    Self::load_state_value(vm, stored)?;
                } else {
                    vm.set_register(10, 0);
                }
                Ok(0)
            }
            ivm::syscalls::SYSCALL_STATE_SET => {
                let path_ptr = vm.register(10);
                let val_ptr = vm.register(11);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let val_tlv = Self::expect_tlv(vm, val_ptr, PointerType::NoritoBytes)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_write_key(path.as_ref());
                let mut stored = Vec::with_capacity(7 + val_tlv.payload.len() + Hash::LENGTH);
                stored.extend_from_slice(&(val_tlv.type_id as u16).to_be_bytes());
                stored.push(val_tlv.version);
                let payload_len = u32::try_from(val_tlv.payload.len())
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                stored.extend_from_slice(&payload_len.to_be_bytes());
                stored.extend_from_slice(val_tlv.payload);
                let h: [u8; Hash::LENGTH] = Hash::new(val_tlv.payload).into();
                stored.extend_from_slice(&h);
                self.durable_state_overlay.insert(path, Some(stored));
                Ok(0)
            }
            ivm::syscalls::SYSCALL_STATE_DEL => {
                let path_ptr = vm.register(10);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_write_key(path.as_ref());
                self.durable_state_overlay.insert(path, None);
                Ok(0)
            }
            // Norito serialization helpers delegate to the ivm core host shim.
            ivm::syscalls::SYSCALL_BUILD_PATH_MAP_KEY
            | ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | ivm::syscalls::SYSCALL_ENCODE_INT
            | ivm::syscalls::SYSCALL_DECODE_INT
            | ivm::syscalls::SYSCALL_JSON_ENCODE
            | ivm::syscalls::SYSCALL_JSON_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_ENCODE
            | ivm::syscalls::SYSCALL_SCHEMA_DECODE
            | ivm::syscalls::SYSCALL_POINTER_TO_NORITO
            | ivm::syscalls::SYSCALL_POINTER_FROM_NORITO
            | ivm::syscalls::SYSCALL_TLV_EQ
            | ivm::syscalls::SYSCALL_SCHEMA_INFO
            | ivm::syscalls::SYSCALL_NAME_DECODE => self.codec_host.syscall(number, vm),
            // Remaining Norito helpers are safe to forward to DefaultHost.
            ivm::syscalls::SYSCALL_ALLOC
            | ivm::syscalls::SYSCALL_GROW_HEAP
            | ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT
            | ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV
            | ivm::syscalls::SYSCALL_COMMIT_OUTPUT
            | ivm::syscalls::SYSCALL_PROVE_EXECUTION
            | ivm::syscalls::SYSCALL_VERIFY_PROOF
            | ivm::syscalls::SYSCALL_GET_MERKLE_PATH => self.default.syscall(number, vm),

            // Provide current authority AccountId pointer via INPUT region.
            // New format: TLV (type_id:u16, version:u8, len:be u32, payload, hash:32).
            // Historical len-prefixed payloads are no longer emitted.
            ivm::syscalls::SYSCALL_GET_AUTHORITY => {
                use norito::codec::Encode as NoritoEncode;
                // Encode authority payload
                let payload = self.authority.encode();
                let payload_len = Self::len_to_u32(payload.len())?;
                // Build TLV envelope (AccountId: type_id=1, version=1)
                let type_id: u16 = 1;
                let version: u8 = 1;
                let mut blob = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
                blob.extend_from_slice(&type_id.to_be_bytes());
                blob.push(version);
                blob.extend_from_slice(&payload_len.to_be_bytes());
                blob.extend_from_slice(&payload);
                // Append hash: single ABI requires valid digest
                let h = iroha_crypto::Hash::new(&payload);
                blob.extend_from_slice(h.as_ref());
                // Allocate using the VM input bump to avoid overwriting other INPUT TLVs.
                let ptr = vm.alloc_input_tlv(&blob)?;
                vm.set_register(10, ptr);
                Ok(0)
            }

            ivm::syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            ivm::syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            ivm::syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(),
            ivm::syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            ivm::syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),

            // All other stateful operations must be routed via ISIs (not yet implemented).
            _ => Err(ivm::VMError::UnknownSyscall(number)),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn begin_tx(&mut self, declared: &ivm::parallel::StateAccessSet) -> Result<(), ivm::VMError> {
        self.durable_state_overlay.clear();
        self.state_access_log = ivm::host::AccessLog::default();
        self.codec_host.begin_tx(declared)
    }

    fn finish_tx(&mut self) -> Result<ivm::host::AccessLog, ivm::VMError> {
        if !self.access_log_enabled {
            return Ok(ivm::host::AccessLog::default());
        }
        Ok(self.state_access_log.clone())
    }

    fn access_logging_supported(&self) -> bool {
        self.access_log_enabled
    }
}

#[cfg(test)]
mod pointer_abi_tests {
    use core::str::FromStr;

    use iroha_crypto::{Hash as IrohaHash, KeyPair};
    use iroha_data_model::smart_contract::manifest::ContractManifest;
    use iroha_primitives::json::Json;
    use ivm::{
        axt::{GroupBinding, HandleBudget, HandleSubject, SpendOp},
        syscalls as ivm_sys,
    };

    use super::{
        tests::{begin_axt_envelope, make_policy_snapshot, norito_blob, store_tlv},
        *,
    };
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    pub(super) fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
        v.extend_from_slice(&type_id.to_be_bytes());
        v.push(1u8); // version
        let payload_len =
            u32::try_from(payload.len()).expect("payload length must fit into u32 for TLV");
        v.extend_from_slice(&payload_len.to_be_bytes());
        v.extend_from_slice(payload);
        let h = IrohaHash::new(payload);
        v.extend_from_slice(h.as_ref());
        v
    }

    #[test]
    fn strict_policy_accepts_domain_in_abi_v1() {
        // Prepare VM with ABI v1 (baseline)
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let mut vm = ivm::IVM::new(1_000_000);
        vm.load_program(&meta.encode()).expect("load meta");
        // DomainId TLV payload
        let did: DomainId = "wonder".parse().unwrap();
        let payload = did.encode();
        let tlv = make_tlv(8u16, &payload); // PointerType::DomainId = 0x0008
        vm.memory.preload_input(0, &tlv).expect("preload input");
        let decoded = CoreHost::decode_tlv_typed::<DomainId>(
            &vm,
            ivm::Memory::INPUT_START,
            PointerType::DomainId,
        )
        .expect("abi v1 should accept DomainId pointer type");
        assert_eq!(decoded, did);
    }
    #[test]
    fn tlv_decode_valid_account_id() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        // Prepare a valid TLV for AccountId("alice@wonderland") at INPUT_START
        let acc: AccountId = "alice@wonderland".parse().expect("valid AccountId");
        let payload = acc.encode();
        let tlv = make_tlv(1u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload input");

        let host = CoreHost::new(acc.clone());
        // SAFETY: call private decode helper via submodule access
        let decoded: AccountId = host
            .decode_at(&vm, ivm::Memory::INPUT_START)
            .expect("decode TLV");
        assert_eq!(decoded, acc);
    }

    #[test]
    fn json_tlv_decodes_raw_payload() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        let payload = b"1";
        let tlv = make_tlv(PointerType::Json as u16, payload);
        vm.memory.preload_input(0, &tlv).expect("preload json tlv");

        let decoded = CoreHost::decode_tlv_json(&vm, ivm::Memory::INPUT_START)
            .expect("raw json payload should decode");
        assert_eq!(decoded, Json::new(1_u64));
    }

    #[test]
    fn tlv_decode_invalid_hash_rejected() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        let acc: AccountId = "alice@wonderland".parse().expect("valid AccountId");
        let payload = acc.encode();
        let mut tlv = make_tlv(1u16, &payload);
        // Corrupt one byte of the hash (last byte)
        let last = tlv.len() - 1;
        tlv[last] ^= 0xFF;
        vm.memory.preload_input(0, &tlv).expect("preload input");

        let host = CoreHost::new(acc.clone());
        let err = host
            .decode_at::<AccountId>(&vm, ivm::Memory::INPUT_START)
            .expect_err("invalid hash must be rejected");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
    }

    #[test]
    fn pointer_from_norito_wrong_type_is_rejected() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority);

        let name_payload = Name::from_str("rose").unwrap().encode();
        let inner = make_tlv(PointerType::Name as u16, &name_payload);
        let outer = make_tlv(PointerType::NoritoBytes as u16, &inner);
        vm.memory
            .preload_input(0, &outer)
            .expect("preload outer TLV");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, u64::from(PointerType::AccountId as u16));

        let res = host.syscall(ivm::syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm);
        assert!(
            matches!(res, Err(ivm::VMError::NoritoInvalid)),
            "expected NoritoInvalid for mismatched pointer type, got {res:?}"
        );
    }

    #[test]
    fn axt_verify_ds_proof_enforces_manifest_root() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(3);
        let manifest_root = [0xAB; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);

        let mismatched_proof = ProofBlob {
            payload: vec![0x01, 0x02, 0x03],
            expiry_slot: Some(20),
        };
        let decoded = ProofBlob::decode(&mut norito_blob(&mismatched_proof).as_slice())
            .expect("decode proof blob");
        assert_eq!(decoded.payload, mismatched_proof.payload);
        let proof_ptr = store_tlv(
            &mut vm,
            PointerType::ProofBlob,
            &norito_blob(&mismatched_proof),
        );
        let proof_tlv = vm
            .memory
            .validate_tlv(proof_ptr)
            .expect("proof TLV should decode");
        let mut tlv_payload = proof_tlv.payload;
        assert!(
            ProofBlob::decode(&mut tlv_payload).is_ok(),
            "proof payload must decode"
        );
        vm.set_register(11, proof_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm);
        assert!(
            matches!(res, Err(VMError::PermissionDenied)),
            "manifest root mismatch must be rejected, got {res:?}"
        );
        assert!(host.axt_proof_cache.is_empty(), "cache must stay empty");

        let aligned_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(20),
        };
        let proof_ptr = store_tlv(
            &mut vm,
            PointerType::ProofBlob,
            &norito_blob(&aligned_proof),
        );
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0)
        );
        let entry = host
            .axt_proof_cache
            .get(&dsid)
            .expect("cache populated on accept");
        let expected_digest: [u8; 32] = Hash::new(&aligned_proof.payload).into();
        assert_eq!(entry.digest, expected_digest);
        assert_eq!(entry.manifest_root, Some(manifest_root));
    }

    #[test]
    fn axt_verify_ds_proof_rejects_undeclared_dataspace() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(3);
        let other = DataSpaceId::new(4);
        let manifest_root = [0xAB; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&other));
        let proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(20),
        };
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&proof));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, proof_ptr);

        let err = host
            .syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
            .expect_err("undeclared dataspace should be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context recorded");
        assert_eq!(ctx.reason, AxtRejectReason::Descriptor);
        assert_eq!(ctx.dataspace, Some(other));
        assert!(ctx.lane.is_none());
    }

    #[test]
    fn axt_verify_ds_proof_cache_tracks_slot_and_manifest_root() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(7);
        let manifest_root = [0x11; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let mut current_slot = 9;
        let snapshot = make_policy_snapshot(dsid, manifest_root, current_slot);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);
        let proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(30),
        };
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&proof));
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0)
        );
        let entry = host.axt_proof_cache.get(&dsid).expect("cache populated");
        assert_eq!(entry.verified_slot, current_slot);
        assert_eq!(entry.manifest_root, Some(manifest_root));

        current_slot += 1;
        if let Some(snapshot) = host.axt_policy_snapshot.as_mut() {
            snapshot.entries[0].policy.current_slot = current_slot;
        }
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0),
            "slot change should re-verify and refresh cache"
        );
        let entry = host
            .axt_proof_cache
            .get(&dsid)
            .expect("cache refreshed after slot change");
        assert_eq!(entry.verified_slot, current_slot);

        let new_manifest_root = [0x22; 32];
        if let Some(snapshot) = host.axt_policy_snapshot.as_mut() {
            snapshot.entries[0].policy.manifest_root = new_manifest_root;
        }
        let new_proof = ProofBlob {
            payload: new_manifest_root.to_vec(),
            expiry_slot: Some(30),
        };
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&new_proof));
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0),
            "manifest rotation must invalidate prior cache entry"
        );
        let entry = host
            .axt_proof_cache
            .get(&dsid)
            .expect("cache updated after manifest change");
        assert_eq!(entry.manifest_root, Some(new_manifest_root));
        let expected_digest: [u8; 32] = Hash::new(&new_proof.payload).into();
        assert_eq!(entry.digest, expected_digest);
    }

    #[test]
    fn axt_touch_rejects_manifest_prefix_violation_with_context() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(5);
        let manifest_root = [0x22; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![axt::AxtTouchSpec {
                dsid,
                read: vec!["orders/".into()],
                write: vec!["ledger/".into()],
            }],
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest = TouchManifest {
            read: vec!["payments/1".into()],
            write: Vec::new(),
        };
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm)
            .expect_err("prefix violation must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context recorded");
        assert_eq!(ctx.reason, AxtRejectReason::Descriptor);
        assert_eq!(ctx.dataspace, Some(dsid));
        assert_eq!(ctx.lane, Some(LaneId::new(1)));
    }

    #[test]
    fn axt_use_asset_handle_rejects_zero_handle_era() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(12);
        let manifest_root = [0x44; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let mut snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        snapshot.entries[0].policy.min_handle_era = 0;
        snapshot.entries[0].policy.min_sub_nonce = 0;
        snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0)
        );

        let binding = axt::compute_binding(&descriptor).expect("binding");
        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 0,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let handle_ptr = store_tlv(&mut vm, PointerType::AssetHandle, &norito_blob(&handle));
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);

        let err = host
            .syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("zero handle era must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context captured");
        assert_eq!(ctx.reason, AxtRejectReason::HandleEra);
        assert_eq!(ctx.dataspace, Some(dsid));
        assert_eq!(ctx.lane, Some(LaneId::new(1)));
    }

    #[test]
    fn axt_use_asset_handle_rejects_invalid_manifest_root_length() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(13);
        let manifest_root = [0x45; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0)
        );

        let binding = axt::compute_binding(&descriptor).expect("binding");
        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![0x11; 31],
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let handle_ptr = store_tlv(&mut vm, PointerType::AssetHandle, &norito_blob(&handle));
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);

        let err = host
            .syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("invalid manifest root length must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context captured");
        assert_eq!(ctx.reason, AxtRejectReason::Manifest);
        assert!(ctx.dataspace.is_none());
        assert_eq!(ctx.lane, Some(LaneId::new(1)));
    }

    #[test]
    fn axt_use_asset_handle_rejects_invalid_binding_length() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(14);
        let manifest_root = [0x46; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0)
        );

        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: vec![0xAA; 31],
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let handle_ptr = store_tlv(&mut vm, PointerType::AssetHandle, &norito_blob(&handle));
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);

        let err = host
            .syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("invalid binding length must be rejected");
        assert!(matches!(err, VMError::NoritoInvalid));
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context captured");
        assert_eq!(ctx.reason, AxtRejectReason::Descriptor);
        assert!(ctx.dataspace.is_none());
        assert_eq!(ctx.lane, Some(LaneId::new(1)));
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn axt_proof_cache_metrics_refresh_on_manifest_rotation() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(27);
        let manifest_root = [0x10; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let mut current_slot = 9;
        let snapshot = make_policy_snapshot(dsid, manifest_root, current_slot);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(iroha_config::parameters::actual::NexusAxt::default());
        host.set_telemetry(telemetry.clone());
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);
        let proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(30),
        };
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&proof));
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0)
        );
        let snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.status, AXT_PROOF_CACHE_MISS);
        assert_eq!(entry.verified_slot, current_slot);
        assert_eq!(entry.expiry_slot, Some(30));

        current_slot += 1;
        let rotated_root = [0x22; 32];
        let rotated_snapshot = make_policy_snapshot(dsid, rotated_root, current_slot);
        host.refresh_axt_policy_snapshot(&rotated_snapshot);
        let cleared_snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(cleared_snapshot.len(), 1);
        let cleared = &cleared_snapshot[0];
        assert_eq!(cleared.status, "cleared");
        assert_eq!(cleared.manifest_root, Some(manifest_root));
        assert_eq!(cleared.verified_slot, 9);

        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let rotated_proof = ProofBlob {
            payload: rotated_root.to_vec(),
            expiry_slot: Some(40),
        };
        let proof_ptr = store_tlv(
            &mut vm,
            PointerType::ProofBlob,
            &norito_blob(&rotated_proof),
        );
        vm.set_register(11, proof_ptr);
        vm.set_register(10, ds_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0)
        );
        let refreshed_snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(refreshed_snapshot.len(), 1);
        let entry = &refreshed_snapshot[0];
        assert_eq!(entry.status, AXT_PROOF_CACHE_MISS);
        assert_eq!(entry.manifest_root, Some(rotated_root));
        assert_eq!(entry.verified_slot, current_slot);
        assert_eq!(entry.expiry_slot, Some(40));

        let metrics_text = metrics.try_to_string().expect("encode metrics");
        assert!(
            metrics_text.contains("iroha_axt_proof_cache_state"),
            "axt cache metric should be exported"
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn axt_handle_reject_hints_exposed_via_telemetry() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(31);
        let manifest_root = [0xCC; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("binding");
        let mut snapshot = make_policy_snapshot(dsid, manifest_root, 12);
        snapshot.entries[0].policy.min_handle_era = 5;
        snapshot.entries[0].policy.min_sub_nonce = 3;
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        let mut host = CoreHost::new(authority.clone())
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(iroha_config::parameters::actual::NexusAxt::default());
        host.set_telemetry(telemetry.clone());
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        // Touch the dataspace so handle validation can proceed to policy checks.
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0),
            "touch should succeed before handle use"
        );

        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };

        let handle_ptr = store_tlv(&mut vm, PointerType::AssetHandle, &norito_blob(&handle));
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);
        let err = host
            .syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("handle with stale era must be rejected");
        assert!(matches!(err, VMError::PermissionDenied));

        let hints = telemetry.axt_reject_hints_snapshot();
        assert_eq!(hints.len(), 1);
        let hint = &hints[0];
        assert_eq!(hint.dataspace, dsid);
        assert_eq!(hint.target_lane, LaneId::new(1));
        assert_eq!(hint.next_min_handle_era, 5);
        assert_eq!(hint.next_min_sub_nonce, 3);
        assert_eq!(hint.reason, AxtRejectReason::HandleEra);

        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context captured");
        assert_eq!(ctx.reason, AxtRejectReason::HandleEra);
        assert_eq!(ctx.dataspace, Some(dsid));
        assert_eq!(ctx.lane, Some(LaneId::new(1)));
        assert_eq!(
            ctx.snapshot_version,
            AxtPolicySnapshot::compute_version(&snapshot.entries)
        );
        assert_eq!(ctx.next_min_handle_era, Some(5));
        assert_eq!(ctx.next_min_sub_nonce, Some(3));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn axt_reject_context_refreshes_minima_after_policy_update() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(37);
        let manifest_root = [0xDD; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("binding");
        let mut snapshot = make_policy_snapshot(dsid, manifest_root, 12);
        snapshot.entries[0].policy.min_handle_era = 5;
        snapshot.entries[0].policy.min_sub_nonce = 2;
        snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone())
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(iroha_config::parameters::actual::NexusAxt::default());

        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };

        let failing_handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 40,
            max_clock_skew_ms: Some(0),
        };

        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0),
            "touch should succeed before handle use"
        );
        let handle_ptr = store_tlv(
            &mut vm,
            PointerType::AssetHandle,
            &norito_blob(&failing_handle),
        );
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);
        host.syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("stale handle must be rejected");
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context captured");
        assert_eq!(ctx.next_min_handle_era, Some(5));
        assert_eq!(ctx.next_min_sub_nonce, Some(2));
        assert_eq!(ctx.snapshot_version, snapshot.version);

        let mut refreshed = snapshot.clone();
        refreshed.entries[0].policy.min_handle_era = 9;
        refreshed.entries[0].policy.min_sub_nonce = 4;
        refreshed.version = AxtPolicySnapshot::compute_version(&refreshed.entries);
        host.refresh_axt_policy_snapshot(&refreshed);

        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&manifest));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(0),
            "touch should succeed after policy refresh"
        );
        let handle_ptr = store_tlv(
            &mut vm,
            PointerType::AssetHandle,
            &norito_blob(&failing_handle),
        );
        let intent_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&intent));
        vm.set_register(10, handle_ptr);
        vm.set_register(11, intent_ptr);
        vm.set_register(12, 0);
        host.syscall(ivm_sys::SYSCALL_USE_ASSET_HANDLE, &mut vm)
            .expect_err("stale handle must still be rejected");
        let refreshed_ctx = host
            .take_axt_reject_for_tests()
            .expect("refreshed reject context captured");
        assert_eq!(refreshed_ctx.next_min_handle_era, Some(9));
        assert_eq!(refreshed_ctx.next_min_sub_nonce, Some(4));
        assert_eq!(refreshed_ctx.snapshot_version, refreshed.version);
    }

    #[test]
    fn axt_verify_ds_proof_rejects_expired_and_supports_clear() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(11);
        let manifest_root = [0x33; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 12);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);

        let expired_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(10),
        };
        let proof_ptr = store_tlv(
            &mut vm,
            PointerType::ProofBlob,
            &norito_blob(&expired_proof),
        );
        vm.set_register(11, proof_ptr);
        assert!(
            matches!(
                host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
                Err(VMError::PermissionDenied)
            ),
            "expired proof must be rejected"
        );
        assert!(host.axt_proof_cache.is_empty());

        let live_proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(50),
        };
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&live_proof));
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0)
        );
        assert!(host.axt_proof_cache.contains_key(&dsid));

        vm.set_register(11, 0);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(0),
            "proof clear should drop cache entry"
        );
        assert!(host.axt_proof_cache.is_empty());
    }

    #[test]
    fn axt_proof_cache_reused_across_envelopes_in_same_slot() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(12);
        let manifest_root = [0x44; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 20);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let digest: [u8; 32] = Hash::new(manifest_root).into();

        host.cache_proof_entry(
            dsid,
            digest,
            Some(50),
            Some(20),
            Some(manifest_root),
            true,
            AXT_PROOF_CACHE_MISS,
        );
        assert_eq!(host.axt_proof_cache_slot, Some(20));

        host.reset_axt_proof_cache_for_slot(Some(20));
        assert!(
            host.axt_proof_cache.contains_key(&dsid),
            "cache should persist across envelopes within the same slot"
        );
        host.reset_axt_proof_cache_for_slot(Some(20));
        assert_eq!(host.axt_proof_cache_slot, Some(20));
        assert!(host.axt_proof_cache.contains_key(&dsid));
    }

    #[test]
    fn axt_proof_cache_snapshot_exposes_entries() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(21);
        let manifest_root = [0x77; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 15);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("axt binding");
        host.axt_state = Some(axt::HostAxtState::new(descriptor, binding));

        let proof = ProofBlob {
            payload: manifest_root.to_vec(),
            expiry_slot: Some(30),
        };
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert_eq!(host.validate_axt_proof(dsid, &proof, policy), Ok(()));

        let snapshot = host.axt_proof_cache_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.dsid, dsid);
        assert_eq!(entry.manifest_root, Some(manifest_root));
        assert_eq!(entry.expiry_slot, Some(30));
        assert_eq!(entry.verified_slot, 15);
        assert!(entry.valid);
        assert_eq!(entry.status, AXT_PROOF_CACHE_MISS);
    }

    #[test]
    fn axt_proof_cache_evicted_when_slot_changes_between_envelopes() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(13);
        let manifest_root = [0x55; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 30);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let digest: [u8; 32] = Hash::new(manifest_root).into();

        host.cache_proof_entry(
            dsid,
            digest,
            Some(80),
            Some(30),
            Some(manifest_root),
            true,
            AXT_PROOF_CACHE_MISS,
        );
        assert_eq!(host.axt_proof_cache_slot, Some(30));
        assert!(host.axt_proof_cache.contains_key(&dsid));

        host.reset_axt_proof_cache_for_slot(Some(31));
        assert!(
            host.axt_proof_cache.is_empty(),
            "slot change should clear cached proofs"
        );
        assert_eq!(host.axt_proof_cache_slot, Some(31));
    }

    #[test]
    fn axt_proof_cache_respects_ttl_slots() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(14);
        let manifest_root = [0x66; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let timing = iroha_config::parameters::actual::NexusAxt {
            slot_length_ms: nonzero_ext::nonzero!(1_u64),
            max_clock_skew_ms: 0,
            proof_cache_ttl_slots: NonZeroU64::new(2).expect("non-zero ttl"),
            replay_retention_slots: NonZeroU64::new(
                iroha_config::parameters::defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
            )
            .expect("non-zero replay retention"),
        };
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(timing);
        let digest: [u8; 32] = Hash::new(manifest_root).into();

        host.cache_proof_entry(
            dsid,
            digest,
            Some(10),
            Some(5),
            Some(manifest_root),
            true,
            AXT_PROOF_CACHE_MISS,
        );
        host.reset_axt_proof_cache_for_slot(Some(6));
        assert!(
            host.axt_proof_cache.contains_key(&dsid),
            "cache should survive within the configured TTL window"
        );

        host.reset_axt_proof_cache_for_slot(Some(7));
        assert!(
            host.axt_proof_cache.is_empty(),
            "cache should expire once the TTL window elapses"
        );
    }

    #[test]
    fn axt_replay_ledger_from_state_rejects_reuse() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(21);
        let lane = LaneId::new(2);
        let manifest_root = [0x99; 32];
        let binding_bytes = [0xAB; 32];
        let binding = AxtBinding::new(binding_bytes);
        let policy = AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 2,
            min_sub_nonce: 3,
            current_slot: 10,
        };
        let replay_key = AxtHandleReplayKey {
            binding,
            handle_era: 2,
            sub_nonce: 5,
            target_lane: lane,
        };
        let replay_entry = AxtReplayRecord {
            dataspace: dsid,
            used_slot: 5,
            retain_until_slot: 50,
        };

        let world = World::new();
        {
            let mut block = world.block();
            block.axt_policies.insert(dsid, policy);
            block.axt_replay_ledger.insert(replay_key, replay_entry);
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::from_state(authority.clone(), &state);

        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 2,
            sub_nonce: 5,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.as_bytes().to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 60,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
        };

        let err = host
            .enforce_axt_policy(&usage)
            .expect_err("replay guard must reject reused handle");
        assert_eq!(err, VMError::PermissionDenied);
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context recorded");
        assert_eq!(ctx.reason, AxtRejectReason::ReplayCache);
    }

    #[test]
    fn axt_replay_ledger_records_retention_floor() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(23);
        let lane = LaneId::new(1);
        let manifest_root = [0x44; 32];
        let current_slot = 10;
        let retention_slots = 10;
        let timing = iroha_config::parameters::actual::NexusAxt {
            slot_length_ms: NonZeroU64::new(1).expect("slot length"),
            max_clock_skew_ms: 0,
            proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
            replay_retention_slots: NonZeroU64::new(retention_slots)
                .expect("non-zero retention window"),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, current_slot);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone())
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(timing);

        let binding = AxtBinding::new([0xAB; 32]);
        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.as_bytes().to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: current_slot + 2,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
        };

        host.enforce_axt_policy(&usage)
            .expect("policy should accept handle");
        let key = AxtHandleReplayKey {
            binding,
            handle_era: 1,
            sub_nonce: 1,
            target_lane: lane,
        };
        let entry = host
            .axt_replay_ledger
            .get(&key)
            .copied()
            .expect("replay entry recorded");
        let retention_cap = current_slot.saturating_add(retention_slots);
        assert_eq!(entry.retain_until_slot, retention_cap);
    }

    #[test]
    fn axt_replay_ledger_rejects_reuse_with_short_retain_until_slot() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(24);
        let lane = LaneId::new(1);
        let manifest_root = [0x33; 32];
        let current_slot = 5;
        let retention_slots = 10;
        let timing = iroha_config::parameters::actual::NexusAxt {
            slot_length_ms: NonZeroU64::new(1).expect("slot length"),
            max_clock_skew_ms: 0,
            proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
            replay_retention_slots: NonZeroU64::new(retention_slots)
                .expect("non-zero retention window"),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, current_slot);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone())
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(timing);

        let binding_bytes = [0xCD; 32];
        let replay_key = AxtHandleReplayKey {
            binding: AxtBinding::new(binding_bytes),
            handle_era: 1,
            sub_nonce: 1,
            target_lane: lane,
        };
        host.axt_replay_ledger.insert(
            replay_key,
            AxtReplayRecord {
                dataspace: dsid,
                used_slot: 1,
                retain_until_slot: 2,
            },
        );

        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding_bytes.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: current_slot + 20,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "bob@wonderland".into(),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
        };

        let err = host
            .enforce_axt_policy(&usage)
            .expect_err("replay should be rejected within retention window");
        assert_eq!(err, VMError::PermissionDenied);
        let ctx = host
            .take_axt_reject_for_tests()
            .expect("reject context recorded");
        assert_eq!(ctx.reason, AxtRejectReason::ReplayCache);
    }

    #[test]
    fn axt_replay_ledger_prunes_stale_entries_on_hydrate() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(22);
        let lane = LaneId::new(3);
        let manifest_root = [0x77; 32];
        let binding_bytes = [0xBC; 32];
        let binding = AxtBinding::new(binding_bytes);
        let policy = AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            min_handle_era: 1,
            min_sub_nonce: 1,
            current_slot: 2,
        };
        let stale_entry = AxtReplayRecord {
            dataspace: dsid,
            used_slot: 0,
            retain_until_slot: 0,
        };
        let world = World::new();
        {
            let mut block = world.block();
            block.axt_policies.insert(dsid, policy);
            block.axt_replay_ledger.insert(
                AxtHandleReplayKey {
                    binding,
                    handle_era: 1,
                    sub_nonce: 1,
                    target_lane: lane,
                },
                stale_entry,
            );
            block.commit();
        }
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(world, kura, query);
        let retention_slots = {
            state.nexus.get_mut().axt.replay_retention_slots =
                NonZeroU64::new(1).expect("non-zero retention window");
            state.nexus.get_mut().axt.replay_retention_slots.get()
        };
        state.prune_axt_replay_ledger_for_tests(retention_slots, retention_slots);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::from_state(authority.clone(), &state);

        let handle = AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: 10,
                per_use: Some(10),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.as_bytes().to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(0),
        };
        let intent = RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                kind: "transfer".into(),
                from: authority.to_string(),
                to: "charlie@wonderland".into(),
                amount: "3".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 3,
        };

        assert_eq!(
            host.enforce_axt_policy(&usage),
            Ok(()),
            "stale replay entries should be pruned on hydrate"
        );
        assert!(
            host.take_axt_reject_for_tests().is_none(),
            "no reject context should be recorded for pruned entries"
        );
    }

    #[test]
    fn register_contract_manifest_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let domain: DomainId = "wonderland".parse().unwrap();
        let authority = AccountId::of(domain, kp.public_key().clone());
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"contract-code");
        let abi_hash = IrohaHash::new(b"contract-abi");
        let request = scode::RegisterSmartContractCode {
            manifest: ContractManifest {
                code_hash: Some(code_hash),
                abi_hash: Some(abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                provenance: None,
            }
            .signed(&kp),
        };
        let payload = norito::to_bytes(&request).expect("encode request to Norito");
        let decoded: scode::RegisterSmartContractCode =
            norito::decode_from_bytes(&payload).expect("decode ok");
        assert_eq!(
            decoded.manifest, request.manifest,
            "roundtrip manifest must match"
        );
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE, &mut vm);
        assert_eq!(res, Ok(0));
        let expected = InstructionBox::from(request.clone());
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let domain: DomainId = "wonderland".parse().unwrap();
        let authority = AccountId::of(domain, public_key);
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"bytecode");
        let request = scode::RegisterSmartContractBytes {
            code_hash,
            code: vec![0xAA, 0xBB, 0xCC],
        };
        let payload = request.encode();
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            &mut vm,
        );
        assert_eq!(res, Ok(0));
        let expected = InstructionBox::from(request.clone());
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn activate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let domain: DomainId = "wonderland".parse().unwrap();
        let authority = AccountId::of(domain, public_key);
        let mut host = CoreHost::new(authority);

        let request = scode::ActivateContractInstance {
            namespace: "apps".into(),
            contract_id: "payments".into(),
            code_hash: IrohaHash::new(b"payments-code"),
        };
        let payload = request.encode();
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE, &mut vm);
        assert_eq!(res, Ok(0));
        let expected = InstructionBox::from(request.clone());
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn deactivate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let domain: DomainId = "wonderland".parse().unwrap();
        let authority = AccountId::of(domain, public_key);
        let mut host = CoreHost::new(authority);

        let request = scode::DeactivateContractInstance {
            namespace: "apps".to_owned(),
            contract_id: "settlement".to_owned(),
            reason: Some("compromised deployment".to_owned()),
        };
        let payload = request.encode();
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE, &mut vm);
        assert_eq!(res, Ok(0));
        let expected = InstructionBox::from(request.clone());
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn remove_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let domain: DomainId = "wonderland".parse().unwrap();
        let authority = AccountId::of(domain, public_key);
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"bytecode-image");
        let request = scode::RemoveSmartContractBytes {
            code_hash,
            reason: None,
        };
        let payload = request.encode();
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES, &mut vm);
        assert_eq!(res, Ok(0));
        let expected = InstructionBox::from(request.clone());
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn unknown_syscall_is_rejected_by_policy() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let mut host = CoreHost::new("alice@wonderland".parse().unwrap());
        // Pick a syscall number outside allowed/known ranges
        let res = host.syscall(0x99, &mut vm);
        assert!(matches!(res, Err(ivm::VMError::UnknownSyscall(0x99))));
    }

    #[test]
    fn tlv_wrong_type_id_is_rejected() {
        crate::test_alias::ensure();
        // Build a program that calls SYSCALL_NFT_SET_METADATA and pass a TLV with wrong type for key
        let mut vm = IVM::new(1_000);
        let owner: AccountId = "alice@wonderland".parse().unwrap();
        vm.set_host(CoreHost::with_accounts(
            owner.clone(),
            Arc::new(vec![owner.clone()]),
        ));
        // Minimal program: HALT, but we only exercise host.validate_tlv via manual call
        let code = [ivm::encoding::wide::encode_halt().to_le_bytes()].concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        // Prepare TLVs: nft_id (correct), key (WRONG: Json instead of Name), value (Json)
        let nft_id = NftId::of("wonderland".parse().unwrap(), "n1".parse().unwrap());
        let key: iroha_data_model::name::Name = "k".parse().unwrap();
        let value = iroha_primitives::json::Json::new("v");
        let nft_blob = nft_id.encode();
        let key_blob = key.encode();
        let val_blob = value.encode();
        // type ids: AccountId=1, AssetDefinitionId=2, Name=3, Json=4, NftId=5 (see pointer_abi)
        let tlv_nft = make_tlv(0x0005, &nft_blob);
        let tlv_key_wrong = make_tlv(0x0004, &key_blob); // should be Name (0x0003)
        let tlv_val = make_tlv(0x0004, &val_blob);
        let mut off = 0u64;
        // Preload TLVs and set registers x10..x12
        vm.memory
            .preload_input(off, &tlv_nft)
            .expect("preload input");
        vm.set_register(10, ivm::Memory::INPUT_START + off);
        off += tlv_nft.len() as u64 + 8; // ensure alignment gap
        vm.memory
            .preload_input(off, &tlv_key_wrong)
            .expect("preload input");
        vm.set_register(11, ivm::Memory::INPUT_START + off);
        off += tlv_key_wrong.len() as u64 + 8;
        vm.memory
            .preload_input(off, &tlv_val)
            .expect("preload input");
        vm.set_register(12, ivm::Memory::INPUT_START + off);
        // Build a tiny program invoking the syscall and run it; expect a failure
        let code = [
            ivm::encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                u8::try_from(ivm::syscalls::SYSCALL_NFT_SET_METADATA)
                    .expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
            ivm::encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        let res = vm.run();
        assert!(
            matches!(res, Err(ivm::VMError::NoritoInvalid)),
            "expected NoritoInvalid, got {res:?}"
        );
    }

    #[test]
    fn tlv_wrong_version_or_hash_is_rejected() {
        crate::test_alias::ensure();
        // Build a TLV with wrong version and with wrong hash and ensure validator rejects
        let mut vm = IVM::new(0);
        let owner: AccountId = "alice@wonderland".parse().unwrap();
        vm.set_host(CoreHost::with_accounts(
            owner.clone(),
            Arc::new(vec![owner.clone()]),
        ));
        let code = [ivm::encoding::wide::encode_halt().to_le_bytes()].concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        // payload is a Name
        let key: iroha_data_model::name::Name = "k".parse().unwrap();
        let key_blob = key.encode();
        // Wrong version
        let mut tlv_wrong_ver = Vec::new();
        tlv_wrong_ver.extend_from_slice(&0x0003u16.to_be_bytes());
        tlv_wrong_ver.push(2); // version=2 (should be 1)
        let key_len = u32::try_from(key_blob.len()).expect("encoded key length must fit into u32");
        tlv_wrong_ver.extend_from_slice(&key_len.to_be_bytes());
        tlv_wrong_ver.extend_from_slice(&key_blob);
        tlv_wrong_ver.extend_from_slice(&[0u8; 32]); // junk hash
        let mut off = 0u64;
        vm.memory
            .preload_input(off, &tlv_wrong_ver)
            .expect("preload input");
        let res = vm.memory.validate_tlv(ivm::Memory::INPUT_START + off);
        assert!(matches!(res, Err(ivm::VMError::NoritoInvalid)));

        // Wrong hash with correct header
        let mut tlv_wrong_hash = Vec::new();
        tlv_wrong_hash.extend_from_slice(&0x0003u16.to_be_bytes());
        tlv_wrong_hash.push(1); // version=1
        tlv_wrong_hash.extend_from_slice(&key_len.to_be_bytes());
        tlv_wrong_hash.extend_from_slice(&key_blob);
        let bad_hash = [0xAAu8; 32];
        tlv_wrong_hash.extend_from_slice(&bad_hash);
        off += 512;
        vm.memory
            .preload_input(off, &tlv_wrong_hash)
            .expect("preload input");
        let res2 = vm.memory.validate_tlv(ivm::Memory::INPUT_START + off);
        assert!(matches!(res2, Err(ivm::VMError::NoritoInvalid)));
    }
}

#[cfg(test)]
fn build_program(code: &[u8], vector_length: u8) -> Vec<u8> {
    let mut program = Vec::with_capacity(17 + code.len());
    program.extend_from_slice(b"IVM\0");
    program.push(1); // version_major
    program.push(0); // version_minor
    program.push(0); // mode
    program.push(vector_length);
    program.extend_from_slice(&0u64.to_le_bytes());
    // ABI v1 is the only supported surface; tests must encode the version byte accordingly.
    program.push(1); // abi_version
    program.extend_from_slice(code);
    program
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iroha_data_model::{
        proof::{VerifyingKeyBox, VerifyingKeyId},
        zk::BackendTag,
    };
    use ivm::{IVM, encoding, instruction, syscalls as ivm_sys};
    use norito::codec::Encode as NoritoEncode;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::ivm::host::pointer_abi_tests::make_tlv,
        state::{State, World},
    };

    #[cfg(feature = "zk-halo2-ipa")]
    fn sample_open_verify_envelope() -> iroha_zkp_halo2::OpenVerifyEnvelope {
        iroha_zkp_halo2::OpenVerifyEnvelope {
            params: iroha_zkp_halo2::IpaParams {
                version: 1,
                curve_id: 1,
                n: 1,
                g: vec![[0u8; 32]],
                h: vec![[0u8; 32]],
                u: [0u8; 32],
            },
            public: iroha_zkp_halo2::PolyOpenPublic {
                version: 1,
                curve_id: 1,
                n: 1,
                z: [0u8; 32],
                t: [0u8; 32],
                p_g: [0u8; 32],
            },
            proof: iroha_zkp_halo2::IpaProofData {
                version: 1,
                l: vec![[0u8; 32]],
                r: vec![[0u8; 32]],
                a_final: [0u8; 32],
                b_final: [0u8; 32],
            },
            transcript_label: "test".to_string(),
            vk_commitment: None,
            public_inputs_schema_hash: None,
            domain_tag: None,
        }
    }

    pub(super) fn norito_blob<T: NoritoEncode>(val: &T) -> Vec<u8> {
        val.encode()
    }

    pub(super) fn store_tlv(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
        let tlv = make_tlv(ty as u16, payload);
        vm.alloc_input_tlv(&tlv).expect("allocate TLV input")
    }

    pub(super) fn make_policy_snapshot(
        dsid: DataSpaceId,
        manifest_root: [u8; 32],
        current_slot: u64,
    ) -> AxtPolicySnapshot {
        let binding = AxtPolicyBinding {
            dsid,
            policy: AxtPolicyEntry {
                manifest_root,
                target_lane: LaneId::new(1),
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot,
            },
        };
        AxtPolicySnapshot {
            version: AxtPolicySnapshot::compute_version(&[binding]),
            entries: vec![binding],
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn compute_envelope_hash_rejects_invalid_payload() {
        let err = CoreHost::compute_envelope_hash(b"invalid envelope")
            .expect_err("invalid envelope should fail");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn compute_envelope_hash_accepts_norito_envelope() {
        let envelope = sample_open_verify_envelope();
        let payload = norito::to_bytes(&envelope).expect("encode envelope");
        let hash = CoreHost::compute_envelope_hash(&payload).expect("hash envelope");
        let expected: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
        assert_eq!(hash, expected);
    }

    #[cfg(not(feature = "zk-halo2-ipa"))]
    #[test]
    fn compute_envelope_hash_accepts_bytes_without_feature_gate() {
        let payload = b"opaque bytes";
        let hash = CoreHost::compute_envelope_hash(payload).expect("hash payload");
        let expected: [u8; 32] = iroha_crypto::Hash::new(payload).into();
        assert_eq!(hash, expected);
    }

    #[test]
    fn norito_blob_roundtrips_with_header_and_bare_decoders() {
        let proof = ProofBlob {
            payload: vec![1, 2, 3],
            expiry_slot: Some(42),
        };
        let bytes = norito_blob(&proof);
        let decoded_bare: ProofBlob =
            ProofBlob::decode(&mut bytes.as_slice()).expect("decode via bare adapter");
        assert_eq!(decoded_bare, proof);
        let framed = norito::core::frame_bare_with_header_flags::<ProofBlob>(
            &bytes,
            norito::core::default_encode_flags(),
        )
        .expect("frame header");
        let decoded_header: ProofBlob =
            norito::decode_from_bytes(&framed).expect("decode via header framing");
        assert_eq!(decoded_header, proof);
    }

    #[test]
    fn get_authority_allocates_in_input_bump() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load meta");

        host.syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, &mut vm)
            .expect("authority syscall");
        let authority_ptr = vm.register(10);

        let marker: Name = "marker".parse().unwrap();
        let _marker_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&marker));

        let decoded: AccountId = host
            .decode_at(&vm, authority_ptr)
            .expect("authority TLV should remain intact");
        assert_eq!(decoded, authority);
    }

    pub(super) fn begin_axt_envelope(
        host: &mut CoreHost,
        vm: &mut IVM,
        descriptor: &axt::AxtDescriptor,
    ) {
        let desc_ptr = store_tlv(vm, PointerType::AxtDescriptor, &norito_blob(descriptor));
        vm.set_register(10, desc_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_BEGIN, vm),
            Ok(0),
            "AXT_BEGIN should seed envelope state"
        );
    }

    #[test]
    fn backend_label_prefers_inline_vk_and_prefix() {
        crate::test_alias::ensure();
        let mut rec = VerifyingKeyRecord::new_with_owner(
            1,
            "halo2/ipa:test_circuit",
            None,
            "ns",
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0u8; 32],
            [1u8; 32],
        );
        // When no inline VK is present, use the circuit_id prefix before ':'
        assert_eq!(
            CoreHost::backend_label_for_record(&rec),
            "halo2/ipa".to_string()
        );
        // Inline VK backend must override the circuit prefix
        rec.key = Some(VerifyingKeyBox::new("custom/backend".into(), vec![1, 2, 3]));
        assert_eq!(
            CoreHost::backend_label_for_record(&rec),
            "custom/backend".to_string()
        );
    }

    #[test]
    fn state_syscall_works_without_access_logging() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &path.encode());
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, b"1");
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);

        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(0),
            "STATE_SET should succeed without access logging"
        );
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(0),
            "STATE_GET should succeed without access logging"
        );
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("state get tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(tlv.payload, b"1");
    }

    #[test]
    fn encode_decode_int_syscalls_roundtrip() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);

        vm.set_register(10, 42);
        assert_eq!(host.syscall(ivm_sys::SYSCALL_ENCODE_INT, &mut vm), Ok(0));
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr).expect("encode tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(tlv.payload, b"42");

        vm.set_register(10, ptr);
        assert_eq!(host.syscall(ivm_sys::SYSCALL_DECODE_INT, &mut vm), Ok(0));
        assert_eq!(vm.register(10), 42);
    }

    #[test]
    fn state_syscall_logs_access_when_enabled() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::new(authority).with_access_logging();
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &path.encode());
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, b"1");
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);

        host.begin_tx(&ivm::parallel::StateAccessSet::default())
            .expect("begin_tx should reset access log");
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(0),
            "STATE_SET should succeed when access logging is enabled"
        );
        let log = host.finish_tx().expect("finish_tx should return log");
        assert!(host.access_logging_supported());
        assert!(log.write_keys.contains("counter"));
    }

    #[test]
    fn state_syscall_reads_world_snapshot() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        world
            .smart_contract_state
            .insert(path.clone(), b"1".to_vec());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &path.encode());
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(0),
            "STATE_GET should read from the world snapshot"
        );
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("snapshot tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(tlv.payload, b"1");
    }

    #[test]
    fn state_syscall_flushes_to_wsv() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let header = iroha_data_model::block::BlockHeader::new(
            nonzero_ext::nonzero!(1_u64),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let mut host = CoreHost::new(authority.clone());
        host.set_durable_state_snapshot_from_world(&stx.world);

        let mut vm = IVM::new(10_000);
        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &path.encode());
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, b"1");
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(0),
            "STATE_SET should stage the overlay"
        );

        host.apply_queued(&mut stx, &authority)
            .expect("flush overlay into transaction");
        let stored = stx
            .world
            .smart_contract_state
            .get(&path)
            .expect("stored value");
        let tlv = ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(tlv.payload, b"1");
    }

    fn dummy_env(
        label: &str,
        vk_commitment: [u8; 32],
        schema_hash: [u8; 32],
        domain: [u8; 32],
    ) -> Vec<u8> {
        use iroha_zkp_halo2::{
            IpaParams, IpaProofData, OpenVerifyEnvelope, PolyOpenPublic, ZkCurveId,
        };
        let params = IpaParams {
            version: 1,
            curve_id: ZkCurveId::Pallas.as_u16(),
            n: 4,
            g: Vec::new(),
            h: Vec::new(),
            u: [0u8; 32],
        };
        let public = PolyOpenPublic {
            version: 1,
            curve_id: ZkCurveId::Pallas.as_u16(),
            n: 4,
            z: [0u8; 32],
            t: [0u8; 32],
            p_g: [0u8; 32],
        };
        let proof = IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: [0u8; 32],
            b_final: [0u8; 32],
        };
        let env = OpenVerifyEnvelope {
            params,
            public,
            proof,
            transcript_label: label.to_string(),
            vk_commitment: Some(vk_commitment),
            public_inputs_schema_hash: Some(schema_hash),
            domain_tag: Some(domain),
        };
        norito::to_bytes(&env).expect("serialize envelope")
    }

    fn active_vk_record(
        commitment: [u8; 32],
        schema_hash: [u8; 32],
        backend: &str,
    ) -> VerifyingKeyRecord {
        let mut rec = VerifyingKeyRecord::new_with_owner(
            1,
            format!("{backend}:circuit"),
            None,
            "transfer",
            BackendTag::Halo2IpaPasta,
            "pallas",
            schema_hash,
            commitment,
        );
        rec.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        rec.max_proof_bytes = 1024;
        rec.key = Some(VerifyingKeyBox::new(backend.into(), vec![9, 9, 9]));
        rec
    }

    #[test]
    fn enforce_zk_envelope_maps_errors_and_ok() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new("alice@wonderland".parse().unwrap());
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let vk_bytes = vec![9, 9, 9];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = [3u8; 32];
        let mut rec = active_vk_record(commitment, schema_hash, backend);
        rec.key = Some(VerifyingKeyBox::new(backend.into(), vk_bytes.clone()));
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec.clone());
        host.set_verifying_keys(map).expect("set registry");

        let domain_inputs = DomainHashInputs {
            backend,
            curve: "pallas",
            vk_commitment: commitment,
            schema_hash,
            syscall_label: "zk_verify_transfer/v1",
            manifest: "core",
            namespace: "transfer",
        };
        let domain = host.compute_domain_hash(&domain_inputs);
        let ok_env = dummy_env("zk_verify_transfer/v1", commitment, schema_hash, domain);
        assert_eq!(
            host.enforce_zk_envelope(&ok_env, "zk_verify_transfer/v1", "transfer"),
            Ok(())
        );

        let bad_label_env = dummy_env("wrong_label", commitment, schema_hash, domain);
        assert_eq!(
            host.enforce_zk_envelope(&bad_label_env, "zk_verify_transfer/v1", "transfer"),
            Err(ivm::host::ERR_TRANSCRIPT_LABEL)
        );
    }

    #[test]
    fn enforce_zk_envelope_rejects_namespace_and_manifest_replays() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new("alice@wonderland".parse().unwrap());
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let vk_bytes = vec![9, 9, 9];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = [7u8; 32];
        let mut rec = active_vk_record(commitment, schema_hash, backend);
        rec.namespace = "ballot".to_string();
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");

        let domain_inputs = DomainHashInputs {
            backend,
            curve: "pallas",
            vk_commitment: commitment,
            schema_hash,
            syscall_label: "zk_verify_transfer/v1",
            manifest: "core",
            namespace: "transfer",
        };
        let domain = host.compute_domain_hash(&domain_inputs);
        let env = dummy_env("zk_verify_transfer/v1", commitment, schema_hash, domain);
        assert_eq!(
            host.enforce_zk_envelope(&env, "zk_verify_transfer/v1", "transfer"),
            Err(ivm::host::ERR_NAMESPACE)
        );

        // Switching the caller manifest also trips the namespace binding.
        host.set_current_manifest_id(Some("other".to_string()));
        assert_eq!(
            host.enforce_zk_envelope(&env, "zk_verify_transfer/v1", "transfer"),
            Err(ivm::host::ERR_NAMESPACE)
        );
    }

    #[test]
    fn enforce_zk_envelope_rejects_vk_metadata_and_domain_mismatch() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new("alice@wonderland".parse().unwrap());
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let vk_bytes = vec![7, 7, 7];
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = [3u8; 32];
        let mut rec = active_vk_record(commitment, schema_hash, backend);
        rec.key = Some(VerifyingKeyBox::new(backend.into(), vk_bytes.clone()));
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec.clone());
        host.set_verifying_keys(map).expect("set registry");

        // Schema hash mismatch is rejected before domain-tag evaluation.
        let wrong_schema = [5u8; 32];
        let domain_inputs = DomainHashInputs {
            backend,
            curve: "pallas",
            vk_commitment: commitment,
            schema_hash: wrong_schema,
            syscall_label: "zk_verify_transfer/v1",
            manifest: "core",
            namespace: "transfer",
        };
        let domain = host.compute_domain_hash(&domain_inputs);
        let env = dummy_env("zk_verify_transfer/v1", commitment, wrong_schema, domain);
        assert_eq!(
            host.enforce_zk_envelope(&env, "zk_verify_transfer/v1", "transfer"),
            Err(ivm::host::ERR_VK_MISMATCH)
        );

        // Matching metadata but a tampered domain tag is rejected explicitly.
        let correct_domain_inputs = DomainHashInputs {
            backend,
            curve: "pallas",
            vk_commitment: commitment,
            schema_hash,
            syscall_label: "zk_verify_transfer/v1",
            manifest: "core",
            namespace: "transfer",
        };
        let correct_domain = host.compute_domain_hash(&correct_domain_inputs);
        let env_bad_domain =
            dummy_env("zk_verify_transfer/v1", commitment, schema_hash, [0xAA; 32]);
        assert_eq!(
            host.enforce_zk_envelope(&env_bad_domain, "zk_verify_transfer/v1", "transfer"),
            Err(ivm::host::ERR_DOMAIN_TAG)
        );

        // Happy-path with untampered domain tag still succeeds.
        let env_ok = dummy_env(
            "zk_verify_transfer/v1",
            commitment,
            schema_hash,
            correct_domain,
        );
        assert_eq!(
            host.enforce_zk_envelope(&env_ok, "zk_verify_transfer/v1", "transfer"),
            Ok(())
        );
    }

    #[test]
    fn input_publish_tlv_is_forwarded_to_default_host() {
        crate::test_alias::ensure();
        // Build a minimal program that calls INPUT_PUBLISH_TLV and then HALT
        let mut vm = IVM::new(10_000);
        let owner: AccountId = "alice@wonderland".parse().unwrap();
        vm.set_host(CoreHost::with_accounts(
            owner.clone(),
            Arc::new(vec![owner.clone()]),
        ));
        let code = [
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();

        // Prepare a TLV in INPUT region and set x10 to its pointer (source)
        let payload = b"hello".to_vec();
        let mut tlv = Vec::new();
        tlv.extend_from_slice(&0x0006u16.to_be_bytes()); // PointerType::Blob
        tlv.push(1); // version
        let payload_len = u32::try_from(payload.len()).expect("payload length must fit into u32");
        tlv.extend_from_slice(&payload_len.to_be_bytes());
        tlv.extend_from_slice(&payload);
        let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
        tlv.extend_from_slice(&h);
        let src_off = 256u64;
        vm.memory
            .preload_input(src_off, &tlv)
            .expect("preload input");
        let src_ptr = ivm::Memory::INPUT_START + src_off;
        vm.set_register(10, src_ptr);

        // Run: CoreHost should forward to DefaultHost and copy TLV, returning new INPUT pointer
        vm.run().expect("syscall ok");
        let dst_ptr = vm.register(10);
        assert!(dst_ptr >= ivm::Memory::INPUT_START);
        // Validate the copied TLV
        let v = vm
            .memory
            .validate_tlv(dst_ptr)
            .expect("validate copied tlv");
        assert_eq!(v.type_id, ivm::PointerType::Blob);
        assert_eq!(v.payload, payload.as_slice());
    }

    #[test]
    fn set_account_detail_rejects_tlv_with_bad_hash() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let key: Name = "cursor".parse().unwrap();

        let account_tlv = make_tlv(PointerType::AccountId as u16, &norito_blob(&authority));
        let key_tlv = make_tlv(PointerType::Name as u16, &norito_blob(&key));
        // Tamper with the trailing hash so TLV validation fails before decoding JSON.
        let mut value_tlv = make_tlv(PointerType::Json as u16, br#"{"note":"tampered"}"#);
        let last = value_tlv
            .last_mut()
            .expect("TLV must include a trailing hash");
        *last ^= 0xFF;

        let mut vm = IVM::new(10_000);
        vm.memory
            .preload_input(0, &account_tlv)
            .expect("preload account TLV");
        vm.memory
            .preload_input(256, &key_tlv)
            .expect("preload key TLV");
        vm.memory
            .preload_input(512, &value_tlv)
            .expect("preload value TLV");

        // SCALL SET_ACCOUNT_DETAIL; HALT
        let mut code = Vec::new();
        code.extend_from_slice(
            &encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
        let program = build_program(&code, 4);

        vm.set_host(CoreHost::with_accounts(
            authority.clone(),
            Arc::new(vec![authority]),
        ));
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.set_register(12, ivm::Memory::INPUT_START + 512);

        let err = vm
            .run()
            .expect_err("invalid TLV hash should reject the syscall");
        assert!(
            matches!(err, ivm::VMError::NoritoInvalid),
            "expected NoritoInvalid when TLV hash is tampered, got {err:?}"
        );
    }

    #[test]
    fn decode_tlv_typed_respects_pointer_policy_guard() {
        crate::test_alias::ensure();
        // Install a restrictive pointer policy so even valid types are rejected.
        let _guard =
            ivm::pointer_abi::PointerPolicyGuard::install(ivm::SyscallPolicy::Experimental(3), 9);

        let mut vm = ivm::IVM::new(1_000_000);
        let did: DomainId = "wonder".parse().unwrap();
        let payload = did.encode();
        let tlv = make_tlv(PointerType::DomainId as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload input");

        let err = CoreHost::decode_tlv_typed::<DomainId>(
            &vm,
            ivm::Memory::INPUT_START,
            PointerType::DomainId,
        )
        .expect_err("pointer policy guard should forbid DomainId");
        assert!(
            matches!(
                err,
                ivm::VMError::AbiTypeNotAllowed { abi, type_id }
                    if abi == 9 && type_id == PointerType::DomainId as u16
            ),
            "expected AbiTypeNotAllowed with annotated abi/type, got {err:?}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pointer_abi_transfer_asset_enqueues_isi() {
        // Prepare Norito-encoded inputs in INPUT region
        let from: AccountId =
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonder"
                .parse()
                .unwrap();
        let to: AccountId =
            "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonder"
                .parse()
                .unwrap();
        let asset_def: AssetDefinitionId = "coin#wonder".parse().unwrap();
        let from_bytes = norito_blob(&from);
        let to_bytes = norito_blob(&to);
        let asset_bytes = norito_blob(&asset_def);
        let from_tlv = pointer_abi_tests::make_tlv(ivm::PointerType::AccountId as u16, &from_bytes);
        let to_tlv = pointer_abi_tests::make_tlv(ivm::PointerType::AccountId as u16, &to_bytes);
        let asset_tlv =
            pointer_abi_tests::make_tlv(ivm::PointerType::AssetDefinitionId as u16, &asset_bytes);

        // Offsets in INPUT region
        let off_from = 0u64;
        let off_to = 256u64;
        let off_asset = 512u64;
        let ptr_from = ivm::Memory::INPUT_START + off_from;
        let ptr_to = ivm::Memory::INPUT_START + off_to;
        let ptr_asset = ivm::Memory::INPUT_START + off_asset;

        // Assemble: SCALL TRANSFER_ASSET; HALT (set arg registers from host side)
        let mut code = Vec::new();
        code.extend_from_slice(
            &encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_TRANSFER_ASSET).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

        // Build a program with metadata header (version 2.0, vector=4, max_cycles=0)
        let program = build_program(&code, 4);

        let mut vm = IVM::new(10_000);
        // Preload the INPUT region
        vm.memory
            .preload_input(off_from, &from_tlv)
            .expect("preload input");
        vm.memory
            .preload_input(off_to, &to_tlv)
            .expect("preload input");
        vm.memory
            .preload_input(off_asset, &asset_tlv)
            .expect("preload input");

        // Attach CoreHost and load program
        vm.set_host(CoreHost::new(from.clone()));
        vm.load_program(&program).unwrap();

        // Set arg registers to pointers and amount
        vm.set_register(10, ptr_from);
        vm.set_register(11, ptr_to);
        vm.set_register(12, ptr_asset);
        vm.set_register(13, 1234);

        // Run and inspect queued ISIs
        vm.run().unwrap();
        let host_any = vm.host_mut_any().unwrap();
        let host = host_any.downcast_mut::<CoreHost>().unwrap();
        let queued = host.drain_instructions();
        assert_eq!(queued.len(), 1);
        let instr = &queued[0];
        let any = instr.as_any();
        if let Some(tb) = any.downcast_ref::<TransferBox>() {
            match tb {
                TransferBox::Asset(inner) => {
                    assert_eq!(inner.destination, to);
                    assert_eq!(inner.source.account, from);
                    assert_eq!(inner.source.definition, asset_def);
                }
                _ => panic!("expected asset transfer"),
            }
        } else {
            panic!("expected TransferBox instruction");
        }
    }

    // NOTE: Additional CoreHost tests for NFT syscalls can be added once the VM instruction
    // builder helpers stabilize across metadata header formats.
    #[test]
    fn sentinel_nft_mint_enqueues_register() {
        let authority: AccountId =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                .parse()
                .unwrap();

        let mut code = Vec::new();
        code.extend_from_slice(
            &encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_NFT_MINT_ASSET).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

        let program = build_program(&code, 4);

        let mut vm = IVM::new(10_000);
        vm.set_host(CoreHost::new(authority));
        vm.load_program(&program).unwrap();
        // a0=a1=0 => sentinels
        vm.set_register(10, 0);
        vm.set_register(11, 0);
        vm.run().unwrap();

        let host_any = vm.host_mut_any().unwrap();
        let host = host_any.downcast_mut::<CoreHost>().unwrap();
        let queued = host.drain_instructions();
        assert_eq!(queued.len(), 1);
        assert!(matches!(
            queued[0].as_any().downcast_ref::<RegisterBox>(),
            Some(RegisterBox::Nft(_))
        ));
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn sm3_syscall_records_success_metric() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let message = b"telemetry";
        let tlv = pointer_abi_tests::make_tlv(ivm::PointerType::Blob as u16, message);

        let mut vm = IVM::new(10_000);
        vm.memory
            .preload_input(0, &tlv)
            .expect("preload SM3 input TLV");

        let mut code = Vec::new();
        code.extend_from_slice(
            &encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_SM3_HASH).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

        let program = build_program(&code, 4);

        let accounts = Arc::new(vec![authority.clone()]);
        let mut host = CoreHost::with_accounts(authority, accounts);
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        host.set_telemetry(StateTelemetry::new(Arc::clone(&metrics), true));
        vm.set_host(host);
        vm.load_program(&program).expect("load SM3 program");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.run().expect("SM3 syscall must succeed");

        assert_eq!(
            metrics
                .sm_syscall_total
                .with_label_values(&["hash", "-"])
                .get(),
            1
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn sm3_syscall_failure_records_failure_metric() {
        crate::test_alias::ensure();
        let authority: AccountId = "alice@wonderland".parse().unwrap();
        let message = b"not-a-blob";
        // Encode a TLV with the wrong pointer type to trigger a Norito validation error.
        let tlv = pointer_abi_tests::make_tlv(ivm::PointerType::AccountId as u16, message);

        let mut vm = IVM::new(10_000);
        vm.memory
            .preload_input(0, &tlv)
            .expect("preload SM3 input TLV");

        let mut code = Vec::new();
        code.extend_from_slice(
            &encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                u8::try_from(ivm_sys::SYSCALL_SM3_HASH).expect("syscall id fits in u8"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

        let program = build_program(&code, 4);

        let accounts = Arc::new(vec![authority.clone()]);
        let mut host = CoreHost::with_accounts(authority, accounts);
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        host.set_telemetry(StateTelemetry::new(Arc::clone(&metrics), true));
        vm.set_host(host);
        vm.load_program(&program).expect("load SM3 program");
        vm.set_register(10, ivm::Memory::INPUT_START);
        let err = vm
            .run()
            .expect_err("SM3 must be rejected when TLV carries the wrong type");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));

        assert_eq!(
            metrics
                .sm_syscall_failures_total
                .with_label_values(&["hash", "-", "norito_invalid"])
                .get(),
            1
        );
    }
}
