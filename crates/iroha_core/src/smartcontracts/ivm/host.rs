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
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem,
    num::{NonZeroU16, NonZeroU64, NonZeroUsize},
    str::FromStr,
    sync::Arc,
};

#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    executor::ContractRuntimeExecutionContext,
    smartcontracts::isi::{
        query::{IvmQueryValidator, QueryLimits, ValidQueryRequest},
        triggers::{TRIGGER_ENABLED_METADATA_KEY, set::SetReadOnly},
    },
    state::{
        StateBlock, StateQueryView, StateReadOnly, StateTransaction, StateView, WorldReadOnly,
        current_axt_slot_from_block,
    },
};
use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use iroha_data_model::{
    DataSpaceId, ValidationFail,
    account::rekey::AccountAlias,
    errors::{AmxStage, AmxTimeout, CanonicalErrorKind},
    escrow::EscrowId,
    events::time::Schedule,
    isi::{
        AddSignatory, Burn, BurnBox, InstructionBox, Mint, MintBox, Register, RegisterBox,
        RemoveSignatory, SetAccountQuorum, SetKeyValue, SetKeyValueBox, SetParameter, Transfer,
        TransferAssetBatch, TransferAssetBatchEntry, TransferBox, Unregister, UnregisterBox,
        escrow::{
            AcceptAnonymousAssetEscrow, AcceptAssetEscrow, CancelAnonymousAssetEscrow,
            CancelAssetEscrow, MarkAnonymousEscrowPaymentSent, MarkEscrowPaymentSent,
            OpenAnonymousAssetEscrow, OpenAnonymousEscrowDispute, OpenAssetEscrow,
            OpenEscrowDispute, ReleaseAnonymousAssetEscrow, ReleaseAssetEscrow,
            ResolveAnonymousEscrowDispute, ResolveEscrowDispute,
        },
        register::RegisterPeerWithPop,
        smart_contract_code as scode, zk as DMZk,
    },
    nexus::{
        AxtBinding, AxtDescriptor as ModelAxtDescriptor, AxtEnvelopeRecord, AxtHandleFragment,
        AxtHandleReplayKey, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot,
        AxtProofEnvelope as ModelAxtProofEnvelope, AxtProofFragment, AxtRejectContext,
        AxtRejectReason, AxtReplayRecord, AxtTouchFragment, AxtTouchSpec as ModelAxtTouchSpec,
        ProofBlob as ModelProofBlob, TouchManifest as ModelTouchManifest,
    },
    parameter::{Parameters, system::ivm_metadata},
    permission::Permissions,
    prelude::{AccountId, *},
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    query::{
        QueryRequest, QueryResponse, SingularQueryBox, SingularQueryOutputBox,
        asset::prelude::FindAssetById, error::QueryExecutionFail,
    },
    smart_contract::{ContractAddress, ContractAlias, ContractInstance},
    subscription::{
        ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY, AccountAliasAutoRenewMetadata,
        SUBSCRIPTION_INVOICE_METADATA_KEY, SUBSCRIPTION_METADATA_KEY,
        SUBSCRIPTION_PLAN_METADATA_KEY, SUBSCRIPTION_TRIGGER_REF_METADATA_KEY,
    },
    zk::BackendTag,
};
use iroha_primitives::calendar;
#[cfg(test)]
use ivm::VMError;
use ivm::{
    self, CoreHost as IvmCodecHost, IVM, PointerType,
    analysis::{self, AmxLimits, ProgramAnalysis},
    axt::{self, AssetHandle, ProofBlob, RemoteSpendIntent, TouchManifest},
    host::IVMHost,
    is_type_allowed_for_policy, pointer_abi,
};
use mv::storage::StorageReadOnly;
use norito::{
    NoritoDeserialize,
    core::{Archived, Header, NoritoSerialize},
    decode_from_bytes, json,
    streaming::CapabilityFlags,
};

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

const PUBLIC_INPUT_GAS_BASE_DEFAULT: u64 = 16;
const PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT: u64 = 1;
const CREATE_NFTS_ALL_GAS: u64 = 16;
const SMARTCONTRACT_DEPTH_GAS: u64 = 16;
const AXT_GAS_BASE: u64 = 16;
const AXT_GAS_PER_BYTE: u64 = 1;
const AXT_VERIFY_GAS_BASE: u64 = 64;
const AXT_VERIFY_GAS_PER_BYTE: u64 = 1;
const TRIGGER_EVENT_PUBLIC_INPUT_KEY: &str = "trigger_event_json";

#[derive(Debug, Clone, crate::json_macros::JsonDeserialize)]
struct PublicInputDescriptor {
    name: Name,
    type_id: u16,
    tlv_hex: String,
    #[norito(default)]
    gas_base: Option<u64>,
    #[norito(default)]
    gas_per_byte: Option<u64>,
}

#[derive(Debug, Clone)]
struct PublicInputRecord {
    type_id: PointerType,
    tlv: Vec<u8>,
    gas_base: u64,
    gas_per_byte: u64,
}

impl PublicInputRecord {
    fn from_tlv_bytes(bytes: Vec<u8>) -> Result<Self, ivm::VMError> {
        let tlv = pointer_abi::validate_tlv_bytes(&bytes)?;
        Ok(Self {
            type_id: tlv.type_id,
            tlv: bytes,
            gas_base: PUBLIC_INPUT_GAS_BASE_DEFAULT,
            gas_per_byte: PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT,
        })
    }

    fn from_descriptor(desc: PublicInputDescriptor) -> Result<(Name, Self), ivm::VMError> {
        let expected_type =
            PointerType::from_u16(desc.type_id).ok_or(ivm::VMError::NoritoInvalid)?;
        let hex_str = desc
            .tlv_hex
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        let bytes = hex::decode(hex_str).map_err(|_| ivm::VMError::NoritoInvalid)?;
        let tlv = pointer_abi::validate_tlv_bytes(&bytes)?;
        if tlv.type_id != expected_type {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let gas_base = desc.gas_base.unwrap_or(PUBLIC_INPUT_GAS_BASE_DEFAULT);
        let gas_per_byte = desc
            .gas_per_byte
            .unwrap_or(PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT);
        Ok((
            desc.name,
            Self {
                type_id: expected_type,
                tlv: bytes,
                gas_base,
                gas_per_byte,
            },
        ))
    }

    fn gas_cost(&self) -> u64 {
        let len = u64::try_from(self.tlv.len()).unwrap_or(u64::MAX);
        self.gas_base
            .saturating_add(self.gas_per_byte.saturating_mul(len))
    }
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
pub struct CoreHostImpl<QS> {
    authority: AccountId,
    current_contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    default: ivm::host::DefaultHost,
    codec_host: IvmCodecHost,
    access_log_enabled: bool,
    halo2_config: ivm::host::ZkHalo2Config,
    stark_config: iroha_config::parameters::actual::Stark,
    crypto: Arc<iroha_config::parameters::actual::Crypto>,
    queued: Vec<QueuedInstruction>,
    fastpq_batch_entries: Option<Vec<TransferAssetBatchEntry>>,
    // Snapshot of accounts available for simple iteration helpers used by samples.
    accounts_snapshot: Arc<Vec<AccountId>>,
    // Optional owned contract registry snapshot for c2c resolution when a live query view is unavailable.
    bound_contract_records_by_subject:
        BTreeMap<AccountId, crate::smartcontracts::code::BoundContractRecord>,
    // Live read-only state view used to execute queries during IVM runs.
    query_state: QS,
    // Simple counter for sample-generated NFT ids to guarantee uniqueness across calls.
    nft_seq: u64,
    // Optional trigger arguments for by-call entrypoints.
    args: Option<iroha_primitives::json::Json>,
    // Trigger identifier for the current execution (time/by-call triggers).
    current_trigger_id: Option<TriggerId>,
    // Block creation timestamp (UTC ms) for the current execution.
    current_block_time_ms: Option<u64>,
    // Current block height for the execution context.
    current_block_height: Option<u64>,
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
    vrf_epoch_seeds: BTreeMap<u64, [u8; 32]>,
    // Registry snapshot of verifying keys.
    verifying_keys: BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    // Registry snapshot of public inputs exposed via SYSCALL_GET_PUBLIC_INPUT.
    public_inputs: BTreeMap<Name, PublicInputRecord>,
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

/// Core host variant without query support (default for VM-attached hosts).
pub type CoreHost = CoreHostImpl<NoQueryState>;

/// Marker query slot for hosts that do not run queries.
#[derive(Default, Copy, Clone)]
pub struct NoQueryState;

/// Slot storing a live queryable state reference for a host run.
pub struct QueryStateSlot<QRef> {
    state: Option<QRef>,
}

impl<QRef> Default for QueryStateSlot<QRef> {
    fn default() -> Self {
        Self { state: None }
    }
}

/// Borrowed query-state reference used during IVM query syscalls.
#[derive(Copy, Clone)]
pub enum QueryStateRef<'block, 'tx, 'state> {
    /// Query against a state view snapshot.
    View(&'block StateView<'state>),
    /// Query against a lightweight state query snapshot.
    QueryView(&'block StateQueryView<'state>),
    /// Query against a state block snapshot.
    Block(&'block StateBlock<'state>),
    /// Query against a state transaction snapshot.
    Transaction(&'block StateTransaction<'tx, 'state>),
}

/// Adapter for state containers that can serve IVM queries.
pub trait QueryStateSource {
    /// Query-state reference type for a given borrow lifetime.
    type Ref<'a>: Copy + QueryStateExecute + QueryStateRefOps + 'a
    where
        Self: 'a;
    /// Borrow the state as a query-state reference.
    fn as_query_state_ref(&self) -> Self::Ref<'_>;

    /// Current block-height hint for sysvar exposure, when available.
    fn block_height_hint(&self) -> Option<u64> {
        None
    }
}

impl<'state> QueryStateSource for StateView<'state> {
    type Ref<'a>
        = QueryStateRef<'a, 'state, 'state>
    where
        Self: 'a;

    fn as_query_state_ref(&self) -> Self::Ref<'_> {
        QueryStateRef::View(self)
    }

    fn block_height_hint(&self) -> Option<u64> {
        Some(u64::try_from(self.height()).unwrap_or(u64::MAX))
    }
}

impl<'state> QueryStateSource for StateQueryView<'state> {
    type Ref<'a>
        = QueryStateRef<'a, 'state, 'state>
    where
        Self: 'a;

    fn as_query_state_ref(&self) -> Self::Ref<'_> {
        QueryStateRef::QueryView(self)
    }

    fn block_height_hint(&self) -> Option<u64> {
        Some(u64::try_from(self.height()).unwrap_or(u64::MAX))
    }
}

impl<'state> QueryStateSource for StateBlock<'state> {
    type Ref<'a>
        = QueryStateRef<'a, 'state, 'state>
    where
        Self: 'a;

    fn as_query_state_ref(&self) -> Self::Ref<'_> {
        QueryStateRef::Block(self)
    }

    fn block_height_hint(&self) -> Option<u64> {
        Some(u64::try_from(self.height()).unwrap_or(u64::MAX))
    }
}

impl<'block, 'state> QueryStateSource for StateTransaction<'block, 'state>
where
    'state: 'block,
{
    type Ref<'a>
        = QueryStateRef<'a, 'block, 'state>
    where
        Self: 'a;

    fn as_query_state_ref(&self) -> Self::Ref<'_> {
        QueryStateRef::Transaction(self)
    }

    fn block_height_hint(&self) -> Option<u64> {
        Some(self.block_height())
    }
}

/// Execute a query against a concrete state reference.
pub trait QueryStateExecute {
    /// Execute a query request for the provided authority.
    ///
    /// Returns the query response and processed item count or a VM error.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the query cannot be executed or fails validation.
    fn execute_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, ivm::VMError>
    where
        Self: Sized;

    /// Execute a query request with a budget for post-offset items.
    ///
    /// Implementations may ignore the budget if they do not support early aborts.
    /// Returns the query response and processed item count or a VM error.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the query cannot be executed or fails validation.
    fn execute_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget_items: Option<u64>,
    ) -> Result<QueryExecutionResult, ivm::VMError>
    where
        Self: Sized,
    {
        let _ = budget_items;
        self.execute_query(authority, request)
    }
}

impl QueryStateExecute for QueryStateRef<'_, '_, '_> {
    fn execute_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        QueryStateRef::execute_query(self, authority, request)
    }

    fn execute_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget_items: Option<u64>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        QueryStateRef::execute_query_with_budget(self, authority, request, budget_items)
    }
}

/// Query-state access shim for host types that may or may not carry a state reference.
pub trait QueryStateAccess {
    /// Query-state reference type for a given borrow lifetime.
    type Ref<'a>: QueryStateExecute + QueryStateRefOps
    where
        Self: 'a;

    /// Fetch the configured query-state reference, if any.
    fn get(&self) -> Option<Self::Ref<'_>>;
}

impl QueryStateAccess for NoQueryState {
    type Ref<'a>
        = QueryStateRef<'a, 'a, 'a>
    where
        Self: 'a;

    fn get(&self) -> Option<Self::Ref<'_>> {
        None
    }
}

impl<QRef> QueryStateAccess for QueryStateSlot<QRef>
where
    QRef: Copy + QueryStateExecute + QueryStateRefOps,
{
    type Ref<'a>
        = QRef
    where
        Self: 'a;

    fn get(&self) -> Option<Self::Ref<'_>> {
        self.state
    }
}

fn map_query_validation_error(error: &ValidationFail) -> ivm::VMError {
    match error {
        ValidationFail::NotPermitted(_) => ivm::VMError::PermissionDenied,
        _ => ivm::VMError::DecodeError,
    }
}

fn map_validation_fail(error: &ValidationFail) -> ivm::VMError {
    match error {
        ValidationFail::NotPermitted(_) => ivm::VMError::PermissionDenied,
        _ => ivm::VMError::DecodeError,
    }
}

fn map_query_execution_error(error: &QueryExecutionFail) -> ivm::VMError {
    match error {
        QueryExecutionFail::GasBudgetExceeded => ivm::VMError::OutOfGas,
        _ => ivm::VMError::DecodeError,
    }
}

/// Query response plus processed item count for gas accounting.
#[derive(Debug)]
pub struct QueryExecutionResult {
    /// Query response payload.
    pub response: QueryResponse,
    /// Number of items processed during query execution.
    pub processed_items: u64,
}

fn execute_query_on_state_with_budget<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget_items: Option<u64>,
) -> Result<QueryExecutionResult, ivm::VMError> {
    struct Validator<'a, R: StateReadOnly> {
        authority: &'a AccountId,
        state: &'a R,
    }

    impl<R: StateReadOnly> IvmQueryValidator for Validator<'_, R> {
        fn authority(&self) -> &AccountId {
            self.authority
        }

        fn validate_query(
            &mut self,
            authority: &AccountId,
            query: &QueryRequest,
        ) -> Result<(), ValidationFail> {
            self.state
                .world()
                .executor()
                .validate_query(self.state, authority, query)
        }
    }

    let mut validator = Validator { authority, state };
    let limits = QueryLimits::from_pipeline(state.pipeline());
    let validated = ValidQueryRequest::validate_for_ivm(request, &mut validator, limits)
        .map_err(|err| map_query_validation_error(&err))?;
    let (response, processed_items) = validated
        .execute_ephemeral_with_stats(state.query_handle(), state, authority, budget_items)
        .map_err(|err| map_query_execution_error(&err))?;
    Ok(QueryExecutionResult {
        response,
        processed_items,
    })
}

#[derive(Copy, Clone)]
struct QueryGasContext {
    base: u64,
    per_item: u64,
    offset_items: u64,
}

impl QueryGasContext {
    fn singular() -> Self {
        Self {
            base: CoreHostImpl::<NoQueryState>::QUERY_GAS_BASE_SINGULAR,
            per_item: CoreHostImpl::<NoQueryState>::QUERY_GAS_PER_ITEM,
            offset_items: 0,
        }
    }

    fn from_request(request: &QueryRequest) -> Self {
        let sort_requested = CoreHostImpl::<NoQueryState>::query_sort_requested(request);
        let base = match request {
            QueryRequest::Singular(_) => CoreHostImpl::<NoQueryState>::QUERY_GAS_BASE_SINGULAR,
            QueryRequest::Start(_) | QueryRequest::Continue(_) => {
                CoreHostImpl::<NoQueryState>::QUERY_GAS_BASE_ITERABLE
            }
        };
        let per_item = if sort_requested {
            CoreHostImpl::<NoQueryState>::QUERY_GAS_PER_ITEM
                .saturating_mul(CoreHostImpl::<NoQueryState>::QUERY_GAS_SORT_MULTIPLIER)
        } else {
            CoreHostImpl::<NoQueryState>::QUERY_GAS_PER_ITEM
        };
        let offset_items = if sort_requested {
            0
        } else {
            CoreHostImpl::<NoQueryState>::query_offset_items(request)
        };
        Self {
            base,
            per_item,
            offset_items,
        }
    }
}

enum ContractInstanceLookup {
    Address(ContractAddress),
    Alias(ContractAlias),
}

impl<'tx, 'state> QueryStateRef<'_, 'tx, 'state>
where
    'state: 'tx,
{
    fn execute_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        self.execute_query_with_budget(authority, request, None)
    }

    fn execute_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget_items: Option<u64>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        match self {
            QueryStateRef::View(view) => {
                execute_query_on_state_with_budget(view, authority, request, budget_items)
            }
            QueryStateRef::QueryView(view) => {
                execute_query_on_state_with_budget(view, authority, request, budget_items)
            }
            QueryStateRef::Block(block) => {
                execute_query_on_state_with_budget(block, authority, request, budget_items)
            }
            QueryStateRef::Transaction(tx) => {
                execute_query_on_state_with_budget(tx, authority, request, budget_items)
            }
        }
    }
}

/// Snapshot of subscription-related data resolved from the current state.
#[derive(Clone)]
enum SubscriptionBillingKind {
    Generic {
        plan: SubscriptionPlan,
        charge_asset_def: AssetDefinition,
    },
    AccountAliasAutoRenew {
        metadata: AccountAliasAutoRenewMetadata,
        charge_asset_def: AssetDefinition,
    },
}

/// Resolved subscription state and billing inputs for the trigger currently being billed.
pub struct SubscriptionContext {
    executable: Executable,
    authority: AccountId,
    trigger_metadata: Metadata,
    subscription_nft_id: NftId,
    subscription_state: SubscriptionState,
    billing: SubscriptionBillingKind,
    subscriber_balance: Numeric,
    nft_owner: AccountId,
}

/// Helpers for accessing subscription data through a query-state reference.
pub trait QueryStateRefOps {
    /// Parse and canonicalize an account alias literal using the current dataspace catalog.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the alias literal is invalid.
    fn parse_account_alias(&self, alias_literal: &str) -> Result<AccountAlias, ivm::VMError>;
    /// Resolve a stable account alias to the current backing account id.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the alias is missing or resolves ambiguously.
    fn resolve_account_alias(
        &self,
        authority: &AccountId,
        alias: &str,
    ) -> Result<AccountId, ivm::VMError>;
    /// Load an NFT by its canonical identifier.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the NFT is missing.
    fn nft_by_id(&self, nft_id: &NftId) -> Result<Nft, ivm::VMError>;
    /// Load a named parameter from the current parameter set.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the parameter name is unknown.
    fn parameter_by_name(&self, name: &Name) -> Result<Parameter, ivm::VMError>;
    /// Load a contract instance by canonical contract address.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the instance binding is missing.
    fn contract_instance_by_address(
        &self,
        contract_address: &ContractAddress,
    ) -> Result<ContractInstance, ivm::VMError>;
    /// Load a contract instance by stable contract alias.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the alias or instance binding is missing.
    fn contract_instance_by_alias(
        &self,
        alias: &ContractAlias,
    ) -> Result<ContractInstance, ivm::VMError>;
    /// Resolve subscription context for a trigger identifier.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the trigger or subscription metadata cannot be resolved.
    fn subscription_context_for_trigger(
        &self,
        trigger_id: &TriggerId,
    ) -> Result<SubscriptionContext, ivm::VMError>;
    /// Load subscription state from a subscription NFT.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the subscription state is missing or invalid.
    fn subscription_state_for_nft(&self, nft_id: &NftId)
    -> Result<SubscriptionState, ivm::VMError>;
    /// Load a subscription plan from its asset definition metadata.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the plan metadata cannot be decoded.
    fn subscription_plan(
        &self,
        plan_id: &AssetDefinitionId,
    ) -> Result<SubscriptionPlan, ivm::VMError>;
    /// Quote a SNS account-alias renewal using current state.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the alias is invalid or not renewable.
    fn quote_account_alias_renewal(
        &self,
        alias_literal: &str,
        term_years: u8,
        now_ms: u64,
    ) -> Result<crate::sns::LeaseQuote, ivm::VMError>;
    /// Resolve the fully bound contract record for a deterministic contract subject.
    fn bound_contract_record_by_subject(
        &self,
        contract_subject: &AccountId,
    ) -> Option<crate::smartcontracts::code::BoundContractRecord>;
    /// Resolve the fully bound contract record for a deployed contract address literal.
    fn bound_contract_record_by_address(
        &self,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
    ) -> Option<crate::smartcontracts::code::BoundContractRecord>;
    /// Load durable smart-contract state by canonical key.
    fn durable_state_get(&self, key: &Name) -> Option<Vec<u8>>;
    /// List durable smart-contract state keys.
    fn durable_state_keys(&self) -> Vec<Name>;
}

impl QueryStateRefOps for QueryStateRef<'_, '_, '_> {
    fn parse_account_alias(&self, alias_literal: &str) -> Result<AccountAlias, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::parse_account_alias(view, alias_literal)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::parse_account_alias(view, alias_literal)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::parse_account_alias(block, alias_literal)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::parse_account_alias(tx, alias_literal)
            }
        }
    }

    fn resolve_account_alias(
        &self,
        authority: &AccountId,
        alias: &str,
    ) -> Result<AccountId, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::resolve_account_alias(view, authority, alias)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::resolve_account_alias(view, authority, alias)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::resolve_account_alias(block, authority, alias)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::resolve_account_alias(tx, authority, alias)
            }
        }
    }

    fn nft_by_id(&self, nft_id: &NftId) -> Result<Nft, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => CoreHostImpl::<NoQueryState>::nft_by_id(view, nft_id),
            QueryStateRef::QueryView(view) => CoreHostImpl::<NoQueryState>::nft_by_id(view, nft_id),
            QueryStateRef::Block(block) => CoreHostImpl::<NoQueryState>::nft_by_id(block, nft_id),
            QueryStateRef::Transaction(tx) => CoreHostImpl::<NoQueryState>::nft_by_id(tx, nft_id),
        }
    }

    fn parameter_by_name(&self, name: &Name) -> Result<Parameter, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::parameter_by_name(view, name)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::parameter_by_name(view, name)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::parameter_by_name(block, name)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::parameter_by_name(tx, name)
            }
        }
    }

    fn contract_instance_by_address(
        &self,
        contract_address: &ContractAddress,
    ) -> Result<ContractInstance, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_address(view, contract_address)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_address(view, contract_address)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_address(block, contract_address)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_address(tx, contract_address)
            }
        }
    }

    fn contract_instance_by_alias(
        &self,
        alias: &ContractAlias,
    ) -> Result<ContractInstance, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_alias(view, alias)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_alias(view, alias)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_alias(block, alias)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::contract_instance_by_alias(tx, alias)
            }
        }
    }

    fn subscription_context_for_trigger(
        &self,
        trigger_id: &TriggerId,
    ) -> Result<SubscriptionContext, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::subscription_context_for_trigger(view, trigger_id)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::subscription_context_for_trigger(view, trigger_id)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::subscription_context_for_trigger(block, trigger_id)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::subscription_context_for_trigger(tx, trigger_id)
            }
        }
    }

    fn subscription_state_for_nft(
        &self,
        nft_id: &NftId,
    ) -> Result<SubscriptionState, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::subscription_state_for_nft(view, nft_id)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::subscription_state_for_nft(view, nft_id)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::subscription_state_for_nft(block, nft_id)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::subscription_state_for_nft(tx, nft_id)
            }
        }
    }

    fn subscription_plan(
        &self,
        plan_id: &AssetDefinitionId,
    ) -> Result<SubscriptionPlan, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::subscription_plan(view, plan_id)
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::subscription_plan(view, plan_id)
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::subscription_plan(block, plan_id)
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::subscription_plan(tx, plan_id)
            }
        }
    }

    fn quote_account_alias_renewal(
        &self,
        alias_literal: &str,
        term_years: u8,
        now_ms: u64,
    ) -> Result<crate::sns::LeaseQuote, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => CoreHostImpl::<NoQueryState>::quote_account_alias_renewal(
                view,
                alias_literal,
                term_years,
                now_ms,
            ),
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::quote_account_alias_renewal(
                    view,
                    alias_literal,
                    term_years,
                    now_ms,
                )
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::quote_account_alias_renewal(
                    block,
                    alias_literal,
                    term_years,
                    now_ms,
                )
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::quote_account_alias_renewal(
                    tx,
                    alias_literal,
                    term_years,
                    now_ms,
                )
            }
        }
    }

    fn bound_contract_record_by_subject(
        &self,
        contract_subject: &AccountId,
    ) -> Option<crate::smartcontracts::code::BoundContractRecord> {
        match *self {
            QueryStateRef::View(view) => {
                crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
                    view,
                    contract_subject,
                )
            }
            QueryStateRef::QueryView(view) => {
                crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
                    view,
                    contract_subject,
                )
            }
            QueryStateRef::Block(block) => {
                crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
                    block,
                    contract_subject,
                )
            }
            QueryStateRef::Transaction(tx) => {
                crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
                    tx,
                    contract_subject,
                )
            }
        }
    }

    fn bound_contract_record_by_address(
        &self,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
    ) -> Option<crate::smartcontracts::code::BoundContractRecord> {
        match *self {
            QueryStateRef::View(view) => {
                crate::smartcontracts::code::fetch_bound_contract_record(view, contract_address)
            }
            QueryStateRef::QueryView(view) => {
                crate::smartcontracts::code::fetch_bound_contract_record(view, contract_address)
            }
            QueryStateRef::Block(block) => {
                crate::smartcontracts::code::fetch_bound_contract_record(block, contract_address)
            }
            QueryStateRef::Transaction(tx) => {
                crate::smartcontracts::code::fetch_bound_contract_record(tx, contract_address)
            }
        }
    }

    fn durable_state_get(&self, key: &Name) -> Option<Vec<u8>> {
        match *self {
            QueryStateRef::View(view) => view.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::QueryView(view) => view.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::Block(block) => block.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::Transaction(tx) => tx.world().smart_contract_state().get(key).cloned(),
        }
    }

    fn durable_state_keys(&self) -> Vec<Name> {
        match *self {
            QueryStateRef::View(view) => view
                .world()
                .smart_contract_state()
                .iter()
                .map(|(key, _)| key.clone())
                .collect(),
            QueryStateRef::QueryView(view) => view
                .world()
                .smart_contract_state()
                .iter()
                .map(|(key, _)| key.clone())
                .collect(),
            QueryStateRef::Block(block) => block
                .world()
                .smart_contract_state()
                .iter()
                .map(|(key, _)| key.clone())
                .collect(),
            QueryStateRef::Transaction(tx) => tx
                .world()
                .smart_contract_state()
                .iter()
                .map(|(key, _)| key.clone())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NestedCallReturnSchema {
    Unit,
    Int,
    Bool,
    Numeric,
    Json,
    Name,
    AccountId,
    AssetDefinitionId,
    AssetId,
    DomainId,
    NftId,
    Blob,
    Bytes,
    DataSpaceId,
    AxtDescriptor,
    AssetHandle,
}

impl NestedCallReturnSchema {
    fn parse(raw: Option<&str>) -> Result<Self, ivm::VMError> {
        let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
            return Ok(Self::Unit);
        };
        match raw {
            "int" => Ok(Self::Int),
            "fixed_u128" | "Amount" | "Balance" => Ok(Self::Numeric),
            "bool" => Ok(Self::Bool),
            "Json" => Ok(Self::Json),
            "Name" => Ok(Self::Name),
            "AccountId" => Ok(Self::AccountId),
            "AssetDefinitionId" => Ok(Self::AssetDefinitionId),
            "AssetId" => Ok(Self::AssetId),
            "DomainId" => Ok(Self::DomainId),
            "NftId" => Ok(Self::NftId),
            "Blob" => Ok(Self::Blob),
            "bytes" => Ok(Self::Bytes),
            "DataSpaceId" => Ok(Self::DataSpaceId),
            "AxtDescriptor" => Ok(Self::AxtDescriptor),
            "AssetHandle" => Ok(Self::AssetHandle),
            _ => Err(ivm::VMError::DecodeError),
        }
    }
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

/// Execution artifacts extracted from a host run that can be applied to state later.
pub(crate) struct HostExecutionArtifacts {
    queued: Vec<QueuedInstruction>,
    confidential_gas_delta: u64,
    completed_axt: Vec<axt::HostAxtState>,
    durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
}

#[derive(Clone)]
pub(crate) struct QueuedInstruction {
    pub(crate) instruction: InstructionBox,
    pub(crate) authority: AccountId,
    pub(crate) contract_runtime_context: Option<ContractRuntimeExecutionContext>,
}

impl core::fmt::Debug for QueuedInstruction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QueuedInstruction")
            .field("instruction", &self.instruction)
            .field("authority", &self.authority)
            .field(
                "has_contract_runtime_context",
                &self.contract_runtime_context.is_some(),
            )
            .finish()
    }
}

impl PartialEq<InstructionBox> for QueuedInstruction {
    fn eq(&self, other: &InstructionBox) -> bool {
        self.contract_runtime_context.is_none() && self.instruction == *other
    }
}

#[derive(Clone)]
struct NestedContractCallHostSnapshot {
    authority: AccountId,
    current_contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    args: Option<iroha_primitives::json::Json>,
    fastpq_batch_entries: Option<Vec<TransferAssetBatchEntry>>,
    nft_seq: u64,
    queued: Vec<QueuedInstruction>,
    durable_state_overlay: BTreeMap<Name, Option<Vec<u8>>>,
    state_access_log: ivm::host::AccessLog,
    axt_state: Option<axt::HostAxtState>,
    completed_axt: Vec<axt::HostAxtState>,
    zk_verified_transfer: VecDeque<[u8; 32]>,
    zk_verified_unshield: VecDeque<[u8; 32]>,
    zk_verified_ballot: VecDeque<[u8; 32]>,
    zk_verified_tally: VecDeque<[u8; 32]>,
    zk_last_env_hash_transfer: VecDeque<[u8; 32]>,
    zk_last_env_hash_unshield: VecDeque<[u8; 32]>,
    zk_last_env_hash_ballot: VecDeque<[u8; 32]>,
    zk_last_env_hash_tally: VecDeque<[u8; 32]>,
    axt_replay_ledger: BTreeMap<AxtHandleReplayKey, AxtReplayRecord>,
    axt_proof_cache_slot: Option<u64>,
    axt_proof_cache: BTreeMap<DataSpaceId, CachedProofEntry>,
    last_axt_reject: Option<AxtRejectContext>,
    amx_budget_violation: Option<AmxBudgetViolation>,
}

impl HostExecutionArtifacts {
    pub(crate) fn queued_instructions(&self) -> Vec<InstructionBox> {
        self.queued
            .iter()
            .map(|queued| queued.instruction.clone())
            .collect()
    }

    fn seed_queued_call_hash_if_missing(
        tx: &mut StateTransaction<'_, '_>,
        queued: &[QueuedInstruction],
    ) -> Result<(), ValidationFail> {
        if tx.tx_call_hash.is_some() || queued.is_empty() {
            return Ok(());
        }
        let mut bytes = Vec::from(&b"iroha:ivm:queued-call-hash:v1|"[..]);
        for queued in queued {
            let authority = norito::to_bytes(&queued.authority).map_err(|error| {
                ValidationFail::InternalError(format!(
                    "failed to encode queued authority for call_hash seed: {error}"
                ))
            })?;
            let instruction = norito::to_bytes(&queued.instruction).map_err(|error| {
                ValidationFail::InternalError(format!(
                    "failed to encode queued instruction for call_hash seed: {error}"
                ))
            })?;
            bytes.extend_from_slice(
                &u64::try_from(authority.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&authority);
            bytes.extend_from_slice(
                &u64::try_from(instruction.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&instruction);
        }
        tx.tx_call_hash = Some(iroha_crypto::Hash::new(bytes));
        Ok(())
    }

    pub(crate) fn apply_to_transaction(
        self,
        tx: &mut StateTransaction<'_, '_>,
        _authority: &AccountId,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        Self::seed_queued_call_hash_if_missing(tx, &self.queued)?;
        let executor = tx.world.executor.clone();
        for queued in &self.queued {
            executor.execute_instruction_with_contract_runtime_context(
                tx,
                &queued.authority,
                queued.instruction.clone(),
                queued.contract_runtime_context.as_ref(),
            )?;
        }
        if self.confidential_gas_delta > 0 {
            tx.record_confidential_gas_delta(self.confidential_gas_delta);
        }
        if !self.completed_axt.is_empty() {
            let lane = tx.current_lane_id.unwrap_or_else(|| LaneId::new(0));
            let commit_height = tx.block_height();
            let envelopes: Vec<_> = self
                .completed_axt
                .into_iter()
                .map(|state| {
                    CoreHostImpl::<NoQueryState>::materialize_axt_record(
                        &state,
                        lane,
                        commit_height,
                    )
                })
                .collect();
            for envelope in envelopes {
                tx.record_axt_envelope(envelope);
            }
        }
        if !self.durable_state_overlay.is_empty() {
            for (path, value) in self.durable_state_overlay {
                if let Some(stored) = value {
                    tx.world.smart_contract_state.insert(path.clone(), stored);
                } else {
                    tx.world.smart_contract_state.remove(path.clone());
                }
            }
        }
        Ok(self
            .queued
            .into_iter()
            .map(|queued| queued.instruction)
            .collect())
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
impl<QS: Default + QueryStateAccess> CoreHostImpl<QS> {
    fn decode_escrow_id(vm: &IVM, ptr: u64) -> Result<EscrowId, ivm::VMError> {
        let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
        Ok(Self::escrow_id_from_name(&name))
    }

    fn decode_optional_evidence_hashes(vm: &IVM, ptr: u64) -> Result<Vec<Hash>, ivm::VMError> {
        if ptr == 0 {
            return Ok(Vec::new());
        }

        let tlv = Self::decode_pointer_tlv(vm, ptr, PointerType::NoritoBytes)
            .or_else(|_| Self::decode_pointer_tlv(vm, ptr, PointerType::Blob))?;
        decode_from_bytes(tlv.payload)
            .or_else(|_| {
                let owned = tlv.payload.to_vec();
                decode_from_bytes(&owned)
            })
            .map_err(|_| ivm::VMError::DecodeError)
    }

    /// Create a new host for the given authority.
    pub fn new(authority: AccountId) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let mut default = ivm::host::DefaultHost::new();
        default.set_sm_enabled(default_crypto.sm_helpers_enabled());
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            current_contract_runtime_context: None,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            stark_config: iroha_config::parameters::actual::Stark::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: Arc::new(Vec::new()),
            bound_contract_records_by_subject: BTreeMap::new(),
            query_state: QS::default(),
            nft_seq: 0,
            args: None,
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
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
            vrf_epoch_seeds: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
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

    /// Construct a host from a state snapshot, hydrating config, ZK snapshots, and AXT policy.
    pub fn from_state(authority: AccountId, state: &crate::state::State) -> Self {
        let view = state.view();
        let snapshot = view.axt_policy_snapshot();
        let mut host = Self::new(authority);
        host.set_crypto_config(Arc::clone(&view.crypto));
        host.set_zk_config(&view.zk);
        host.set_chain_id(&view.chain_id);
        host.set_block_height(u64::try_from(view.height()).unwrap_or(u64::MAX));
        host.set_axt_timing(view.nexus.axt);
        host.hydrate_axt_replay_ledger(&view);
        host.set_durable_state_snapshot_from_world(view.world());
        host.set_public_inputs_from_parameters(view.world().parameters());
        host.set_vrf_epoch_seeds_from_world(view.world());
        host.set_zk_snapshots_from_world(view.world(), &view.zk)
            .expect("valid ZK snapshot state");
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
            current_contract_runtime_context: None,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            stark_config: iroha_config::parameters::actual::Stark::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            bound_contract_records_by_subject: BTreeMap::new(),
            query_state: QS::default(),
            nft_seq: 0,
            args: None,
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
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
            vrf_epoch_seeds: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
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
            current_contract_runtime_context: None,
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            halo2_config: ivm::host::ZkHalo2Config::default(),
            stark_config: iroha_config::parameters::actual::Stark::default(),
            crypto,
            queued: Vec::new(),
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            bound_contract_records_by_subject: BTreeMap::new(),
            query_state: QS::default(),
            nft_seq: 0,
            args: Some(args),
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
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
            vrf_epoch_seeds: BTreeMap::new(),
            verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
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

    /// Preserve the currently executing contract identity for queued runtime instructions.
    pub(crate) fn set_contract_runtime_context(
        &mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) {
        self.current_contract_runtime_context = contract_runtime_context;
    }

    /// Bind the host to a deployed contract runtime identity for scoped state reads and writes.
    pub fn bind_contract_runtime_context(
        &mut self,
        contract_address: iroha_data_model::smart_contract::ContractAddress,
        contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
        entrypoint: String,
    ) {
        self.current_contract_runtime_context = Some(ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias,
            entrypoint,
        });
    }

    /// Seed deployed contract records for nested c2c resolution when a live query view is unavailable.
    pub(crate) fn set_bound_contract_records_by_subject_snapshot(
        &mut self,
        snapshot: BTreeMap<AccountId, crate::smartcontracts::code::BoundContractRecord>,
    ) {
        self.bound_contract_records_by_subject = snapshot;
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
                let digest = record.manifest_hash.as_ref();
                let is_zero_sentinel = digest[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
                    && digest[Hash::LENGTH - 1] == 1;
                if is_zero_sentinel {
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

    /// Configure STARK verification limits for forwarded `ZK_VERIFY` syscalls.
    pub fn set_stark_config(&mut self, cfg: &iroha_config::parameters::actual::Stark) {
        self.stark_config = *cfg;
    }

    /// Configure all ZK verification limits for forwarded `ZK_VERIFY` syscalls.
    pub fn set_zk_config(&mut self, cfg: &iroha_config::parameters::actual::Zk) {
        self.set_halo2_config(&cfg.halo2);
        self.set_stark_config(&cfg.stark);
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

    /// Provide public inputs retrievable via `SYSCALL_GET_PUBLIC_INPUT`.
    #[must_use]
    pub fn with_public_inputs(mut self, inputs: BTreeMap<Name, Vec<u8>>) -> Self {
        self.set_public_inputs(inputs);
        self
    }

    /// Replace the public input map used by `SYSCALL_GET_PUBLIC_INPUT`.
    pub fn set_public_inputs(&mut self, inputs: BTreeMap<Name, Vec<u8>>) {
        self.public_inputs = Self::public_inputs_from_tlvs(inputs);
    }

    /// Refresh the public input registry from on-chain parameters when present.
    pub fn set_public_inputs_from_parameters(&mut self, params: &Parameters) {
        if let Some(map) = Self::public_inputs_from_parameters(params) {
            self.public_inputs = map;
        }
    }

    fn public_inputs_from_tlvs(
        inputs: BTreeMap<Name, Vec<u8>>,
    ) -> BTreeMap<Name, PublicInputRecord> {
        let mut map = BTreeMap::new();
        for (name, bytes) in inputs {
            match PublicInputRecord::from_tlv_bytes(bytes) {
                Ok(record) => {
                    map.insert(name, record);
                }
                Err(err) => {
                    iroha_logger::warn!(
                        %name,
                        ?err,
                        "Skipping invalid public input TLV"
                    );
                }
            }
        }
        map
    }

    fn public_inputs_from_parameters(
        params: &Parameters,
    ) -> Option<BTreeMap<Name, PublicInputRecord>> {
        let id = ivm_metadata::public_inputs_id();
        let custom = params.custom().get(&id)?;
        let entries = match custom
            .payload()
            .try_into_any_norito::<Vec<PublicInputDescriptor>>()
        {
            Ok(entries) => entries,
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    "Failed to decode ivm_public_inputs custom parameter payload"
                );
                return Some(BTreeMap::new());
            }
        };
        let mut map = BTreeMap::new();
        for entry in entries {
            let name = entry.name.clone();
            match PublicInputRecord::from_descriptor(entry) {
                Ok((name, record)) => {
                    if map.insert(name.clone(), record).is_some() {
                        iroha_logger::warn!(
                            %name,
                            "Duplicate public input name in ivm_public_inputs registry"
                        );
                    }
                }
                Err(err) => {
                    iroha_logger::warn!(
                        %name,
                        ?err,
                        "Skipping invalid public input registry entry"
                    );
                }
            }
        }
        Some(map)
    }

    /// Install a read-only snapshot of ZK roots per asset for state-read syscalls.
    pub fn set_zk_roots_snapshot(&mut self, map: BTreeMap<AssetDefinitionId, Vec<[u8; 32]>>) {
        self.zk_roots = map;
    }

    /// Hydrate VRF epoch seed snapshots from world storage.
    pub fn set_vrf_epoch_seeds_from_world(&mut self, world: &impl WorldReadOnly) {
        self.vrf_epoch_seeds = world
            .vrf_epochs()
            .iter()
            .map(|(epoch, record)| (*epoch, record.seed))
            .collect();
    }

    /// Hydrate ZK snapshots (roots, elections, verifying keys) from a world view.
    ///
    /// # Errors
    /// Returns an error if the verifying key registry contains inconsistent entries.
    pub(crate) fn set_zk_snapshots_from_world(
        &mut self,
        world: &impl WorldReadOnly,
        zk_cfg: &iroha_config::parameters::actual::Zk,
    ) -> Result<(), ivm::VMError> {
        self.set_zk_root_history_cap(zk_cfg.root_history_cap);
        self.set_zk_empty_root_policy(zk_cfg.empty_root_on_empty, zk_cfg.merkle_depth);

        let mut roots = BTreeMap::new();
        for (ad, st) in world.zk_assets().iter() {
            roots.insert(ad.clone(), st.root_history.clone());
        }
        self.set_zk_roots_snapshot(roots);

        let mut elections = BTreeMap::new();
        for (id, st) in world.elections().iter() {
            elections.insert(id.clone(), (st.finalized, st.tally.clone()));
        }
        self.set_zk_elections_snapshot(elections);

        let mut vks = BTreeMap::new();
        for (id, rec) in world.verifying_keys().iter() {
            vks.insert(id.clone(), rec.clone());
        }
        self.set_verifying_keys(vks)?;
        Ok(())
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

    /// Replace the durable smart-contract state snapshot with a prebuilt offline fixture.
    pub fn set_durable_state_snapshot(&mut self, snapshot: BTreeMap<Name, Vec<u8>>) {
        self.durable_state_base = snapshot;
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
    /// Returns an error when a stored commitment does not match the stored verifying key bytes.
    pub fn set_verifying_keys(
        &mut self,
        map: BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    ) -> Result<(), ivm::VMError> {
        // Early sanity: ensure commitments match stored key bytes when present.
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

    #[cfg(test)]
    fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
        crate::zk::hash_vk_bytes(backend, bytes)
    }

    fn validate_envelope_header(
        &self,
        env: &iroha_data_model::zk::OpenVerifyEnvelope,
        payload_len: usize,
    ) -> Result<(), u64> {
        match env.backend {
            iroha_data_model::zk::BackendTag::Halo2IpaPasta => {
                if payload_len > self.halo2_config.max_envelope_bytes {
                    return Err(ivm::host::ERR_ENVELOPE_SIZE);
                }
                if !self.halo2_config.enabled {
                    return Err(ivm::host::ERR_DISABLED);
                }
                if self.halo2_config.backend != ivm::host::ZkHalo2Backend::Ipa {
                    return Err(ivm::host::ERR_BACKEND);
                }
            }
            iroha_data_model::zk::BackendTag::Stark => {
                if payload_len > self.stark_config.max_envelope_bytes {
                    return Err(ivm::host::ERR_ENVELOPE_SIZE);
                }
                if !self.stark_config.enabled {
                    return Err(ivm::host::ERR_DISABLED);
                }
            }
            _ => return Err(ivm::host::ERR_BACKEND),
        }
        Ok(())
    }

    fn normalize_halo2_circuit_id(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa/") {
            return (!rest.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/ipa::") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/ipa:") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = trimmed.strip_prefix("halo2/ipa/") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        Some(format!("halo2/pasta/ipa/{trimmed}"))
    }

    fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
        if backend == "halo2/ipa" {
            match (
                Self::normalize_halo2_circuit_id(record_id),
                Self::normalize_halo2_circuit_id(env_id),
            ) {
                (Some(rec), Some(env)) => rec == env,
                _ => record_id == env_id,
            }
        } else {
            record_id == env_id
        }
    }

    fn load_vk_record_any_namespace(
        &self,
        vk_commitment: [u8; 32],
    ) -> Result<(VerifyingKeyRecord, String), u64> {
        let vk_rec = self
            .verifying_keys
            .values()
            .find(|r| r.commitment == vk_commitment)
            .cloned()
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        if !matches!(
            vk_rec.backend,
            BackendTag::Halo2IpaPasta | BackendTag::Stark
        ) {
            return Err(ivm::host::ERR_BACKEND);
        }
        if !vk_rec.is_active() {
            return Err(ivm::host::ERR_VK_INACTIVE);
        }
        let backend_label = Self::backend_label_for_record(&vk_rec);
        Ok((vk_rec, backend_label))
    }

    fn validate_proof_len(
        &self,
        vk_rec: &VerifyingKeyRecord,
        backend_label: &str,
        env: &iroha_data_model::zk::OpenVerifyEnvelope,
    ) -> Result<(), u64> {
        let proof_len_bytes = env.proof_bytes.len();
        let proof_limit = u32::try_from(proof_len_bytes).unwrap_or(u32::MAX);
        if vk_rec.max_proof_bytes > 0 && proof_limit > vk_rec.max_proof_bytes {
            return Err(ivm::host::ERR_PROOF_LEN);
        }
        if crate::zk::is_stark_fri_v1_backend(backend_label) {
            if proof_len_bytes > self.stark_config.max_proof_bytes {
                return Err(ivm::host::ERR_PROOF_LEN);
            }
            if let Ok(open) = norito::decode_from_bytes::<iroha_data_model::zk::StarkFriOpenProofV1>(
                &env.proof_bytes,
            ) && open.envelope_bytes.len() > self.stark_config.max_proof_bytes
            {
                return Err(ivm::host::ERR_PROOF_LEN);
            }
        } else if proof_len_bytes > self.halo2_config.max_proof_bytes {
            return Err(ivm::host::ERR_PROOF_LEN);
        }
        Ok(())
    }

    fn parse_zk1_ipa_k(vk_bytes: &[u8]) -> Option<u32> {
        const MAGIC: &[u8; 4] = b"ZK1\0";
        if vk_bytes.len() < 4 || &vk_bytes[..4] != MAGIC {
            return None;
        }
        let mut cursor = 4usize;
        while cursor.checked_add(8)? <= vk_bytes.len() {
            let tag = &vk_bytes[cursor..cursor + 4];
            cursor += 4;
            let len = u32::from_le_bytes(vk_bytes[cursor..cursor + 4].try_into().ok()?) as usize;
            cursor += 4;
            let end = cursor.checked_add(len)?;
            if end > vk_bytes.len() {
                return None;
            }
            if tag == b"IPAK" && len == 4 {
                return Some(u32::from_le_bytes(vk_bytes[cursor..end].try_into().ok()?));
            }
            cursor = end;
        }
        None
    }

    fn curve_is_allowed(&self, backend_label: &str, curve_label: &str) -> bool {
        if crate::zk::is_stark_fri_v1_backend(backend_label) {
            return curve_label == "goldilocks";
        }
        match self.halo2_config.curve {
            ivm::host::ZkCurve::Pallas | ivm::host::ZkCurve::Pasta => {
                curve_label == "pallas" || curve_label == "pasta"
            }
            ivm::host::ZkCurve::Goldilocks => curve_label == "goldilocks",
            ivm::host::ZkCurve::Bn254 => curve_label == "bn254",
        }
    }

    fn enforce_zk_envelope_impl(
        &mut self,
        payload: &[u8],
        namespace: Option<&str>,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            VerifyingKeyBox,
            String,
        ),
        u64,
    > {
        let env: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(payload).map_err(|_| ivm::host::ERR_DECODE)?;
        self.validate_envelope_header(&env, payload.len())?;
        if env.vk_hash == [0u8; 32] {
            return Err(ivm::host::ERR_VK_MISSING);
        }
        let (vk_rec, backend_label) = self.load_vk_record_any_namespace(env.vk_hash)?;
        if let Some(expected_namespace) = namespace
            && vk_rec.namespace != expected_namespace
        {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        if !Self::circuit_id_matches(&backend_label, &vk_rec.circuit_id, &env.circuit_id) {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        let vk_box = vk_rec.key.clone().ok_or(ivm::host::ERR_VK_MISSING)?;
        let inline_commit = crate::zk::hash_vk(&vk_box);
        if inline_commit != vk_rec.commitment {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        let current_manifest = self.current_manifest_id.as_deref().unwrap_or("core");
        if vk_rec.owner_manifest_id.as_deref().unwrap_or("core") != current_manifest {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        if !self.curve_is_allowed(&backend_label, &vk_rec.curve) {
            return Err(ivm::host::ERR_CURVE);
        }
        if !crate::zk::is_stark_fri_v1_backend(&backend_label) {
            let Some(k) = Self::parse_zk1_ipa_k(&vk_box.bytes) else {
                return Err(ivm::host::ERR_DECODE);
            };
            if k > self.halo2_config.max_k {
                return Err(ivm::host::ERR_K);
            }
        }
        let schema_hash: [u8; 32] = Hash::new(&env.public_inputs).into();
        if schema_hash != vk_rec.public_inputs_schema_hash {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        self.validate_proof_len(&vk_rec, &backend_label, &env)?;
        Ok((env, vk_box, backend_label))
    }

    fn enforce_zk_envelope(
        &mut self,
        payload: &[u8],
        namespace: &str,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            VerifyingKeyBox,
            String,
        ),
        u64,
    > {
        self.enforce_zk_envelope_impl(payload, Some(namespace))
    }

    fn enforce_zk_envelope_any_namespace(
        &mut self,
        payload: &[u8],
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            VerifyingKeyBox,
            String,
        ),
        u64,
    > {
        self.enforce_zk_envelope_impl(payload, None)
    }

    fn zk_verify_guardrails(&self) -> crate::zk::ZkVerifyGuardrails {
        crate::zk::ZkVerifyGuardrails {
            halo2_enabled: self.halo2_config.enabled,
            halo2_max_envelope_bytes: self.halo2_config.max_envelope_bytes,
            halo2_max_proof_bytes: self.halo2_config.max_proof_bytes,
            stark_enabled: self.stark_config.enabled,
            stark_max_envelope_bytes: self.stark_config.max_envelope_bytes,
            stark_max_proof_bytes: self.stark_config.max_proof_bytes,
        }
    }

    fn verify_bound_envelope(&mut self, payload: &[u8], namespace: &str) -> Result<bool, u64> {
        let (_env, vk_box, backend_label) = self.enforce_zk_envelope(payload, namespace)?;
        let proof = ProofBox::new(backend_label.clone().into(), payload.to_vec());
        let guardrails = self.zk_verify_guardrails();
        Ok(crate::zk::verify_backend_with_timing_guardrails(
            &backend_label,
            &proof,
            Some(&vk_box),
            guardrails,
        )
        .ok)
    }

    fn verify_any_namespace_envelope(&mut self, payload: &[u8]) -> Result<bool, u64> {
        let (_env, vk_box, backend_label) = self.enforce_zk_envelope_any_namespace(payload)?;
        let proof = ProofBox::new(backend_label.clone().into(), payload.to_vec());
        let guardrails = self.zk_verify_guardrails();
        Ok(crate::zk::verify_backend_with_timing_guardrails(
            &backend_label,
            &proof,
            Some(&vk_box),
            guardrails,
        )
        .ok)
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
        self.drain_queued_instructions()
            .into_iter()
            .map(|queued| queued.instruction)
            .collect()
    }

    fn drain_queued_instructions(&mut self) -> Vec<QueuedInstruction> {
        self.flush_pending_fastpq_batch();
        mem::take(&mut self.queued)
    }

    fn drain_queued_instructions_with_fallback(
        &mut self,
        fallback: Option<ContractRuntimeExecutionContext>,
    ) -> Vec<QueuedInstruction> {
        let fallback = fallback.or_else(|| self.current_contract_runtime_context.clone());
        self.drain_queued_instructions()
            .into_iter()
            .map(|mut queued| {
                if queued.contract_runtime_context.is_none() {
                    queued.contract_runtime_context = fallback.clone();
                }
                queued
            })
            .collect()
    }

    pub(crate) fn drain_queued_instructions_with_contract_runtime_context(
        &mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) -> Vec<QueuedInstruction> {
        self.drain_queued_instructions_with_fallback(contract_runtime_context)
    }

    /// Drain durable contract-state writes collected during the last VM run.
    pub fn drain_durable_state_overlay(&mut self) -> BTreeMap<Name, Option<Vec<u8>>> {
        mem::take(&mut self.durable_state_overlay)
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

    fn validate_queued_for_zk(&mut self, queued: &[InstructionBox]) -> Result<(), ValidationFail> {
        for instr in queued {
            let any: &dyn iroha_data_model::isi::Instruction = &**instr;
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
        }
        Ok(())
    }

    pub(crate) fn into_execution_artifacts(
        mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) -> Result<HostExecutionArtifacts, ValidationFail> {
        let queued = self.drain_queued_instructions_with_fallback(contract_runtime_context);
        self.validate_queued_for_zk(
            &queued
                .iter()
                .map(|queued| queued.instruction.clone())
                .collect::<Vec<_>>(),
        )?;
        let confidential_gas_delta = queued
            .iter()
            .map(|queued| crate::gas::confidential_gas_cost(&queued.instruction))
            .sum::<u64>();
        let completed_axt = mem::take(&mut self.completed_axt);
        let durable_state_overlay = mem::take(&mut self.durable_state_overlay);
        Ok(HostExecutionArtifacts {
            queued,
            confidential_gas_delta,
            completed_axt,
            durable_state_overlay,
        })
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
        self.apply_queued_with_contract_runtime_context(tx, authority, None)
    }

    /// Apply queued ISIs via the executor while preserving an optional
    /// contract runtime context for nested contract execution.
    pub(crate) fn apply_queued_with_contract_runtime_context(
        &mut self,
        tx: &mut StateTransaction<'_, '_>,
        _authority: &AccountId,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        let queued =
            self.drain_queued_instructions_with_fallback(contract_runtime_context.cloned());
        self.validate_queued_for_zk(
            &queued
                .iter()
                .map(|queued| queued.instruction.clone())
                .collect::<Vec<_>>(),
        )?;
        HostExecutionArtifacts::seed_queued_call_hash_if_missing(tx, &queued)?;
        let executor = tx.world.executor.clone();
        for queued in &queued {
            executor.execute_instruction_with_contract_runtime_context(
                tx,
                &queued.authority,
                queued.instruction.clone(),
                queued
                    .contract_runtime_context
                    .as_ref()
                    .or(contract_runtime_context),
            )?;
        }
        if !queued.is_empty() {
            let delta = queued
                .iter()
                .map(|queued| crate::gas::confidential_gas_cost(&queued.instruction))
                .sum::<u64>();
            if delta > 0 {
                tx.record_confidential_gas_delta(delta);
            }
        }
        self.flush_completed_axt(tx);
        self.flush_durable_state(tx);
        Ok(queued
            .into_iter()
            .map(|queued| queued.instruction)
            .collect())
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

    fn scoped_durable_state_path(&self, path: &Name) -> Result<Option<Name>, ivm::VMError> {
        let Some(scope_prefix) = self.durable_state_scope_prefix() else {
            return Ok(None);
        };
        let scoped = format!("{scope_prefix}{}", path.as_ref());
        scoped
            .parse()
            .map(Some)
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn durable_state_scope_prefix(&self) -> Option<String> {
        let Some(context) = self.current_contract_runtime_context.as_ref() else {
            return None;
        };

        // Durable state belongs to one deployed contract instance. Alias bindings
        // can move across redeploys, so the immutable contract address is the
        // stable namespace key.
        let scope_id = context.contract_address.to_string();
        let digest = hex::encode(Hash::new(scope_id.as_bytes()).as_ref());
        Some(format!("sc/{digest}/"))
    }

    fn state_key_matches_prefix(key: &str, prefix: &str) -> bool {
        key == prefix
            || key
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    fn durable_state_key_present(&self, key: &Name) -> Result<bool, ivm::VMError> {
        if let Some(scoped_path) = self.scoped_durable_state_path(key)? {
            if let Some(entry) = self.durable_state_overlay.get(&scoped_path) {
                return Ok(entry.is_some());
            }
            if self.durable_state_base_or_live_get(&scoped_path).is_some() {
                return Ok(true);
            }
        }
        if let Some(entry) = self.durable_state_overlay.get(key) {
            return Ok(entry.is_some());
        }
        Ok(self.durable_state_base_or_live_get(key).is_some())
    }

    fn durable_state_value_payload_len(stored: &[u8]) -> Result<usize, ivm::VMError> {
        match ivm::pointer_abi::validate_tlv_bytes(stored) {
            Ok(tlv) if tlv.type_id == PointerType::NoritoBytes => Ok(tlv.payload.len()),
            Ok(_) => Err(ivm::VMError::NoritoInvalid),
            Err(_) => Ok(stored.len()),
        }
    }

    fn durable_state_value_len(&self, key: &Name) -> Result<Option<usize>, ivm::VMError> {
        if let Some(scoped_path) = self.scoped_durable_state_path(key)? {
            if let Some(entry) = self.durable_state_overlay.get(&scoped_path) {
                return entry
                    .as_deref()
                    .map(Self::durable_state_value_payload_len)
                    .transpose();
            }
            if let Some(stored) = self.durable_state_base_or_live_get(&scoped_path) {
                return Self::durable_state_value_payload_len(stored.as_ref()).map(Some);
            }
        }
        if let Some(entry) = self.durable_state_overlay.get(key) {
            return entry
                .as_deref()
                .map(Self::durable_state_value_payload_len)
                .transpose();
        }
        self.durable_state_base_or_live_get(key)
            .map_or(Ok(None), |stored| {
                Self::durable_state_value_payload_len(stored.as_ref()).map(Some)
            })
    }

    fn durable_state_base_or_live_get<'a>(&'a self, key: &Name) -> Option<Cow<'a, [u8]>> {
        if let Some(stored) = self.durable_state_base.get(key) {
            return Some(Cow::Borrowed(stored.as_slice()));
        }
        self.query_state
            .get()
            .and_then(|state| state.durable_state_get(key))
            .map(Cow::Owned)
    }

    fn state_query_gas(payload_len: usize) -> u64 {
        16_u64.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
    }

    fn state_keys_gas(returned_count: usize, payload_len: usize) -> u64 {
        Self::state_query_gas(payload_len)
            .saturating_add(u64::try_from(returned_count).unwrap_or(u64::MAX))
    }

    fn state_count_gas(total_count: usize) -> u64 {
        16_u64.saturating_add(u64::try_from(total_count).unwrap_or(u64::MAX))
    }

    fn sysvar_gas(payload_len: usize) -> u64 {
        16_u64.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
    }

    fn smartcontract_depth_gas() -> u64 {
        SMARTCONTRACT_DEPTH_GAS
    }

    fn create_nfts_for_all_gas() -> u64 {
        CREATE_NFTS_ALL_GAS
    }

    fn axt_verify_gas(payload_len: usize) -> u64 {
        AXT_VERIFY_GAS_BASE.saturating_add(
            AXT_VERIFY_GAS_PER_BYTE.saturating_mul(u64::try_from(payload_len).unwrap_or(u64::MAX)),
        )
    }

    fn axt_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        AXT_GAS_BASE.saturating_add(AXT_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn axt_commit_gas(state: &axt::HostAxtState) -> u64 {
        let entries = state
            .touches()
            .len()
            .saturating_add(state.proofs().len())
            .saturating_add(state.handles().len());
        Self::axt_gas(entries)
    }

    fn collect_durable_state_keys(&self, prefix: &Name) -> Result<Vec<Name>, ivm::VMError> {
        let prefix_str = prefix.as_ref();
        let scope_prefix = self.durable_state_scope_prefix();
        let mut candidates = BTreeSet::new();

        let mut consider_key = |key: &Name| -> Result<(), ivm::VMError> {
            let raw = key.as_ref();
            if let Some(scope_prefix) = scope_prefix.as_deref()
                && let Some(unscoped) = raw.strip_prefix(scope_prefix)
            {
                if Self::state_key_matches_prefix(unscoped, prefix_str) {
                    candidates.insert(
                        unscoped
                            .parse::<Name>()
                            .map_err(|_| ivm::VMError::NoritoInvalid)?,
                    );
                }
                return Ok(());
            }
            if Self::state_key_matches_prefix(raw, prefix_str) {
                candidates.insert(key.clone());
            }
            Ok(())
        };

        for key in self.durable_state_base.keys() {
            consider_key(key)?;
        }
        if let Some(state) = self.query_state.get() {
            for key in state.durable_state_keys() {
                consider_key(&key)?;
            }
        }
        for key in self.durable_state_overlay.keys() {
            consider_key(key)?;
        }

        candidates
            .into_iter()
            .filter_map(|key| match self.durable_state_key_present(&key) {
                Ok(true) => Some(Ok(key)),
                Ok(false) => None,
                Err(err) => Some(Err(err)),
            })
            .collect()
    }

    fn paged_state_keys(keys: &[Name], offset: u64, limit: u64) -> Vec<Name> {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset >= keys.len() {
            return Vec::new();
        }
        let take = if limit == 0 {
            keys.len().saturating_sub(offset)
        } else {
            usize::try_from(limit).unwrap_or(usize::MAX)
        };
        keys.iter().skip(offset).take(take).cloned().collect()
    }

    fn decode_name_payload(payload: &[u8]) -> Result<Name, ivm::VMError> {
        match decode_from_bytes(payload) {
            Ok(name) => Ok(name),
            Err(_) => {
                // Some Norito decoders depend on aligned/owned buffers; fall back to a copy.
                let owned = payload.to_vec();
                decode_from_bytes(&owned).map_err(|_| ivm::VMError::DecodeError)
            }
        }
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
        let p = vm.alloc_host_tlv(&env)?;
        vm.set_register(10, p);
        Ok(())
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
    fn decode_pointer_tlv<'a>(
        vm: &'a IVM,
        ptr: u64,
        expected: PointerType,
    ) -> Result<pointer_abi::Tlv<'a>, ivm::VMError> {
        let input_lo = ivm::Memory::INPUT_START;
        let input_hi = ivm::Memory::INPUT_START + ivm::Memory::INPUT_SIZE;
        let tlv = if ptr >= input_lo && ptr + 7 <= input_hi {
            let tlv = vm.memory.validate_tlv(ptr)?;
            if tlv.type_id != expected {
                return Err(ivm::VMError::NoritoInvalid);
            }
            tlv
        } else {
            let code_len = vm.memory.code_len();
            if ptr >= code_len || ptr + 7 > code_len {
                return Err(ivm::VMError::NoritoInvalid);
            }
            let mut hdr = [0u8; 7];
            vm.memory
                .load_bytes(ptr, &mut hdr)
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
            if type_id != expected as u16 || hdr[2] != 1 {
                return Err(ivm::VMError::NoritoInvalid);
            }
            let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
            let total = 7usize
                .checked_add(len)
                .and_then(|value| value.checked_add(Hash::LENGTH))
                .ok_or(ivm::VMError::NoritoInvalid)?;
            if usize::try_from(ptr)
                .ok()
                .and_then(|addr| addr.checked_add(total))
                .is_none_or(|end| end > code_len as usize)
            {
                return Err(ivm::VMError::NoritoInvalid);
            }
            let envelope = vm
                .memory
                .load_region(ptr, total as u64)
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            let tlv = pointer_abi::validate_tlv_bytes(envelope)?;
            if tlv.type_id != expected {
                return Err(ivm::VMError::NoritoInvalid);
            }
            tlv
        };
        if !is_type_allowed_for_policy(vm.syscall_policy(), expected) {
            return Err(ivm::VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: expected as u16,
            });
        }
        Ok(tlv)
    }

    /// Decode a typed pointer-ABI TLV into a Norito value.
    ///
    /// The decoder accepts pointers that already live in INPUT as well as
    /// literal TLVs returned directly from the contract code/literal section.
    ///
    /// # Errors
    /// Returns an error if the pointer type does not match, the type is not allowed
    /// by the current syscall policy, or the Norito payload cannot be decoded.
    pub fn decode_tlv_typed<T>(vm: &IVM, ptr: u64, expected: PointerType) -> Result<T, ivm::VMError>
    where
        T: for<'de> NoritoDeserialize<'de>,
    {
        let tlv = Self::decode_pointer_tlv(vm, ptr, expected)?;
        match decode_from_bytes(tlv.payload) {
            Ok(value) => Ok(value),
            Err(_) => {
                // Fall back to decoding from an owned buffer to avoid alignment quirks.
                let owned = tlv.payload.to_vec();
                decode_from_bytes(&owned).map_err(|_| ivm::VMError::DecodeError)
            }
        }
    }

    fn decode_query_key<T>(vm: &IVM, ptr: u64, expected: PointerType) -> Result<T, ivm::VMError>
    where
        T: for<'de> NoritoDeserialize<'de>,
    {
        let tlv = Self::decode_pointer_tlv(vm, ptr, expected)
            .or_else(|_| Self::decode_pointer_tlv(vm, ptr, PointerType::NoritoBytes))?;
        decode_from_bytes(tlv.payload)
            .or_else(|_| {
                let owned = tlv.payload.to_vec();
                decode_from_bytes(&owned)
            })
            .map_err(|_| ivm::VMError::DecodeError)
    }

    fn decode_contract_instance_lookup(
        vm: &IVM,
        ptr: u64,
    ) -> Result<ContractInstanceLookup, ivm::VMError> {
        if let Ok(address) =
            Self::decode_tlv_typed::<ContractAddress>(vm, ptr, PointerType::NoritoBytes)
        {
            return Ok(ContractInstanceLookup::Address(address));
        }
        let alias_name = Self::decode_tlv_typed::<Name>(vm, ptr, PointerType::Name)
            .or_else(|_| Self::decode_tlv_typed::<Name>(vm, ptr, PointerType::NoritoBytes))?;
        let alias = alias_name
            .as_ref()
            .parse::<ContractAlias>()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        Ok(ContractInstanceLookup::Alias(alias))
    }

    /// Decode a typed pointer-ABI TLV from any readable VM region.
    ///
    /// This variant accepts heap-backed envelopes in addition to INPUT and
    /// literal/code pointers, which is required for staged deployment helpers
    /// that reconstruct large lifecycle requests outside the 64 KiB INPUT
    /// window before queueing the final ISI.
    ///
    /// # Errors
    /// Returns an error if the pointer does not resolve to a TLV in a readable
    /// region, the type is not allowed by the active ABI policy, or the Norito
    /// payload cannot be decoded.
    pub fn decode_tlv_typed_any_region<T>(
        vm: &IVM,
        ptr: u64,
        expected: PointerType,
    ) -> Result<T, ivm::VMError>
    where
        T: for<'de> NoritoDeserialize<'de>,
    {
        let hdr = vm
            .memory
            .load_region(ptr, 7)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
        if type_id != expected as u16 || hdr[2] != 1 {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
        let total = 7usize
            .checked_add(len)
            .and_then(|size| size.checked_add(Hash::LENGTH))
            .ok_or(ivm::VMError::NoritoInvalid)?;
        let envelope = vm
            .memory
            .load_region(ptr, total as u64)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let tlv = pointer_abi::validate_tlv_bytes(envelope)?;
        if tlv.type_id != expected {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if !is_type_allowed_for_policy(vm.syscall_policy(), expected) {
            return Err(ivm::VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: expected as u16,
            });
        }
        match decode_from_bytes(tlv.payload) {
            Ok(value) => Ok(value),
            Err(_) => {
                let owned = tlv.payload.to_vec();
                decode_from_bytes(&owned).map_err(|_| ivm::VMError::DecodeError)
            }
        }
    }

    /// Decode a blob pointer-ABI TLV into owned bytes.
    ///
    /// # Errors
    /// Returns an error if the pointer is invalid for the active ABI policy.
    pub fn decode_tlv_blob(vm: &IVM, ptr: u64) -> Result<Vec<u8>, ivm::VMError> {
        let tlv = Self::decode_pointer_tlv(vm, ptr, PointerType::Blob)?;
        Ok(tlv.payload.to_vec())
    }

    /// Decode a JSON pointer-ABI TLV containing Norito-framed JSON.
    ///
    /// # Errors
    /// Returns an error if the pointer is invalid for the active ABI policy or the payload fails to decode.
    pub fn decode_tlv_json(vm: &IVM, ptr: u64) -> Result<Json, ivm::VMError> {
        let tlv = Self::decode_pointer_tlv(vm, ptr, PointerType::Json)?;

        match decode_from_bytes(tlv.payload) {
            Ok(value) => Ok(value),
            Err(_) => {
                // Some payloads decode successfully only after re-buffering.
                let owned = tlv.payload.to_vec();
                decode_from_bytes(&owned).map_err(|_| ivm::VMError::DecodeError)
            }
        }
    }

    fn permission_from_name(name: &Name) -> Permission {
        Permission::new(name.as_ref().to_string(), Json::new(()))
    }

    fn permission_from_value(value: &json::Value) -> Result<Permission, ivm::VMError> {
        if let Some(name) = value.as_str() {
            return Ok(Permission::new(name.to_string(), Json::new(())));
        }
        if let Some(map) = value.as_object() {
            if let Ok(permission) = json::from_value::<Permission>(value.clone()) {
                return Ok(permission);
            }
            let name = map
                .get("name")
                .and_then(json::Value::as_str)
                .ok_or(ivm::VMError::DecodeError)?;
            let payload = map.get("payload").cloned().unwrap_or(json::Value::Null);
            return Ok(Permission::new(name.to_string(), Json::from(payload)));
        }
        Err(ivm::VMError::DecodeError)
    }

    fn permission_from_json(value: &Json) -> Result<Permission, ivm::VMError> {
        let value: json::Value = value
            .try_into_any_norito::<json::Value>()
            .map_err(|_| ivm::VMError::DecodeError)?;
        Self::permission_from_value(&value)
    }

    fn permissions_from_value(value: &json::Value) -> Result<Permissions, ivm::VMError> {
        if let Ok(perms) = json::from_value::<Permissions>(value.clone()) {
            return Ok(perms);
        }
        let array = value.as_array().map_or_else(
            || {
                value.as_object().and_then(|map| {
                    map.get("permissions")
                        .or_else(|| map.get("perms"))
                        .or_else(|| map.get("permission"))
                        .and_then(json::Value::as_array)
                })
            },
            Some,
        );
        if let Some(items) = array {
            let mut perms = Permissions::new();
            for item in items {
                let perm = Self::permission_from_value(item)?;
                perms.insert(perm);
            }
            return Ok(perms);
        }
        let perm = Self::permission_from_value(value)?;
        let mut perms = Permissions::new();
        perms.insert(perm);
        Ok(perms)
    }

    fn permissions_from_json(value: &Json) -> Result<Permissions, ivm::VMError> {
        let value: json::Value = value
            .try_into_any_norito::<json::Value>()
            .map_err(|_| ivm::VMError::DecodeError)?;
        Self::permissions_from_value(&value)
    }

    fn public_key_from_value(value: &json::Value) -> Result<PublicKey, ivm::VMError> {
        if let Some(key_str) = value.as_str() {
            return key_str
                .parse::<PublicKey>()
                .map_err(|_| ivm::VMError::DecodeError);
        }
        if let Some(map) = value.as_object() {
            if let Some(key_value) = map
                .get("public_key")
                .or_else(|| map.get("publicKey"))
                .or_else(|| map.get("key"))
            {
                return Self::public_key_from_value(key_value);
            }
        }
        Err(ivm::VMError::DecodeError)
    }

    fn public_key_from_json(value: &Json) -> Result<PublicKey, ivm::VMError> {
        if let Ok(key) = value.try_into_any_norito::<PublicKey>() {
            return Ok(key);
        }
        let value: json::Value = value
            .try_into_any_norito::<json::Value>()
            .map_err(|_| ivm::VMError::DecodeError)?;
        Self::public_key_from_value(&value)
    }

    fn peer_id_from_value(value: &json::Value) -> Result<PeerId, ivm::VMError> {
        if let Some(peer_str) = value.as_str() {
            if let Ok(peer_id) = PeerId::from_str(peer_str) {
                return Ok(peer_id);
            }
            if let Ok(peer) = Peer::from_str(peer_str) {
                return Ok(peer.id().clone());
            }
            return Err(ivm::VMError::DecodeError);
        }
        if let Some(map) = value.as_object() {
            if let Some(peer_value) = map
                .get("peer")
                .or_else(|| map.get("peer_id"))
                .or_else(|| map.get("peerId"))
            {
                return Self::peer_id_from_value(peer_value);
            }
            if let Some(key_value) = map
                .get("public_key")
                .or_else(|| map.get("publicKey"))
                .or_else(|| map.get("key"))
            {
                let key = Self::public_key_from_value(key_value)?;
                return Ok(PeerId::from(key));
            }
        }
        Err(ivm::VMError::DecodeError)
    }

    fn register_peer_from_json(value: &Json) -> Result<RegisterPeerWithPop, ivm::VMError> {
        if let Ok(request) = value.try_into_any_norito::<RegisterPeerWithPop>() {
            return Ok(request);
        }
        let value: json::Value = value
            .try_into_any_norito::<json::Value>()
            .map_err(|_| ivm::VMError::DecodeError)?;
        Self::register_peer_from_value(&value)
    }

    fn register_peer_from_value(value: &json::Value) -> Result<RegisterPeerWithPop, ivm::VMError> {
        if let Ok(request) = json::from_value::<RegisterPeerWithPop>(value.clone()) {
            return Ok(request);
        }
        let map = value.as_object().ok_or(ivm::VMError::DecodeError)?;
        let peer_value = map
            .get("peer")
            .or_else(|| map.get("peer_id"))
            .or_else(|| map.get("peerId"))
            .or_else(|| map.get("public_key"))
            .or_else(|| map.get("publicKey"))
            .or_else(|| map.get("key"))
            .ok_or(ivm::VMError::DecodeError)?;
        let peer_id = Self::peer_id_from_value(peer_value)?;
        let pop_value = map.get("pop").ok_or(ivm::VMError::DecodeError)?;
        let pop: Vec<u8> =
            json::from_value(pop_value.clone()).map_err(|_| ivm::VMError::DecodeError)?;
        let mut request = RegisterPeerWithPop::new(peer_id, pop);
        if let Some(value) = map.get("activation_at") {
            request.activation_at =
                Some(json::from_value(value.clone()).map_err(|_| ivm::VMError::DecodeError)?);
        }
        if let Some(value) = map.get("expiry_at") {
            request.expiry_at =
                Some(json::from_value(value.clone()).map_err(|_| ivm::VMError::DecodeError)?);
        }
        if let Some(value) = map.get("hsm") {
            request.hsm =
                Some(json::from_value(value.clone()).map_err(|_| ivm::VMError::DecodeError)?);
        }
        Ok(request)
    }

    fn decode_permission(vm: &IVM, ptr: u64) -> Result<Permission, ivm::VMError> {
        let tlv = vm.memory.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Name => {
                let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
                Ok(Self::permission_from_name(&name))
            }
            PointerType::Json => {
                let payload = Self::decode_tlv_json(vm, ptr)?;
                Self::permission_from_json(&payload)
            }
            _ => Err(ivm::VMError::NoritoInvalid),
        }
    }

    fn decode_header<T>(payload: &[u8]) -> Result<T, ivm::VMError>
    where
        T: for<'de> NoritoDeserialize<'de>,
    {
        decode_from_bytes(payload).map_err(|_| ivm::VMError::NoritoInvalid)
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

    fn verify_fastpq_envelope_binding(
        &mut self,
        dsid: DataSpaceId,
        envelope: &ModelAxtProofEnvelope,
        policy: AxtPolicyEntry,
    ) -> Result<(), ivm::VMError> {
        let Some(binding) = envelope.fastpq_binding.as_ref() else {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "FASTPQ proof envelope is missing fastpq_binding",
            );
            return Err(ivm::VMError::PermissionDenied);
        };
        if binding.source_dsid != dsid.as_u64() {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "FASTPQ binding source_dsid mismatch",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        fastpq_prover::verify_axt_proof_envelope(envelope).map_err(|err| {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                format!("FASTPQ verification failed: {err}"),
            );
            ivm::VMError::PermissionDenied
        })?;
        Ok(())
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
        // Keep the recorded proof expiry as provided; skew is only applied to cache/slot checks.
        let proof_for_state = ProofBlob {
            payload: proof.payload.clone(),
            expiry_slot: proof.expiry_slot,
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
            state.record_proof(dsid, Some(proof_for_state.clone()), None)?;
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

        let envelope =
            norito::decode_from_bytes::<ModelAxtProofEnvelope>(&proof.payload).map_err(|err| {
                self.record_axt_reject(
                    AxtRejectReason::Proof,
                    Some(dsid),
                    Some(policy.target_lane),
                    format!("proof payload is not an AXT proof envelope: {err}"),
                );
                self.note_axt_proof_cache_event(AXT_PROOF_CACHE_REJECT);
                ivm::VMError::NoritoInvalid
            })?;
        if envelope.dsid != dsid || envelope.manifest_root != policy.manifest_root {
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

        self.verify_fastpq_envelope_binding(dsid, &envelope, policy)?;

        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_proof(dsid, Some(proof_for_state), None)?;
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
    #[cfg(test)]
    fn compute_envelope_hash(payload: &[u8]) -> Result<[u8; 32], ivm::VMError> {
        #[cfg(feature = "zk-halo2-ipa")]
        {
            if norito::decode_from_bytes::<iroha_data_model::zk::OpenVerifyEnvelope>(payload)
                .is_err()
            {
                return Err(ivm::VMError::NoritoInvalid);
            }
        }
        Ok(iroha_crypto::Hash::new(payload).into())
    }

    fn len_to_u32(len: usize) -> Result<u32, ivm::VMError> {
        u32::try_from(len).map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn alloc_tlv_payload(
        vm: &mut IVM,
        pointer_type: PointerType,
        payload: &[u8],
    ) -> Result<u64, ivm::VMError> {
        let payload_len = Self::len_to_u32(payload.len())?;
        let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
        out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
        out.push(1);
        out.extend_from_slice(&payload_len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; Hash::LENGTH] = Hash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_host_tlv(&out)
    }

    fn alloc_norito_bytes(vm: &mut IVM, payload: &[u8]) -> Result<u64, ivm::VMError> {
        Self::alloc_tlv_payload(vm, PointerType::NoritoBytes, payload)
    }

    fn snapshot_nested_contract_call(&self) -> NestedContractCallHostSnapshot {
        NestedContractCallHostSnapshot {
            authority: self.authority.clone(),
            current_contract_runtime_context: self.current_contract_runtime_context.clone(),
            args: self.args.clone(),
            fastpq_batch_entries: self.fastpq_batch_entries.clone(),
            nft_seq: self.nft_seq,
            queued: self.queued.clone(),
            durable_state_overlay: self.durable_state_overlay.clone(),
            state_access_log: self.state_access_log.clone(),
            axt_state: self.axt_state.clone(),
            completed_axt: self.completed_axt.clone(),
            zk_verified_transfer: self.zk_verified_transfer.clone(),
            zk_verified_unshield: self.zk_verified_unshield.clone(),
            zk_verified_ballot: self.zk_verified_ballot.clone(),
            zk_verified_tally: self.zk_verified_tally.clone(),
            zk_last_env_hash_transfer: self.zk_last_env_hash_transfer.clone(),
            zk_last_env_hash_unshield: self.zk_last_env_hash_unshield.clone(),
            zk_last_env_hash_ballot: self.zk_last_env_hash_ballot.clone(),
            zk_last_env_hash_tally: self.zk_last_env_hash_tally.clone(),
            axt_replay_ledger: self.axt_replay_ledger.clone(),
            axt_proof_cache_slot: self.axt_proof_cache_slot,
            axt_proof_cache: self.axt_proof_cache.clone(),
            last_axt_reject: self.last_axt_reject.clone(),
            amx_budget_violation: self.amx_budget_violation,
        }
    }

    fn restore_nested_contract_call_frame(&mut self, snapshot: &NestedContractCallHostSnapshot) {
        self.authority = snapshot.authority.clone();
        self.current_contract_runtime_context = snapshot.current_contract_runtime_context.clone();
        self.args = snapshot.args.clone();
        self.fastpq_batch_entries = snapshot.fastpq_batch_entries.clone();
    }

    fn rollback_nested_contract_call(&mut self, snapshot: &NestedContractCallHostSnapshot) {
        self.restore_nested_contract_call_frame(snapshot);
        self.nft_seq = snapshot.nft_seq;
        self.queued = snapshot.queued.clone();
        self.durable_state_overlay = snapshot.durable_state_overlay.clone();
        self.state_access_log = snapshot.state_access_log.clone();
        self.axt_state = snapshot.axt_state.clone();
        self.completed_axt = snapshot.completed_axt.clone();
        self.zk_verified_transfer = snapshot.zk_verified_transfer.clone();
        self.zk_verified_unshield = snapshot.zk_verified_unshield.clone();
        self.zk_verified_ballot = snapshot.zk_verified_ballot.clone();
        self.zk_verified_tally = snapshot.zk_verified_tally.clone();
        self.zk_last_env_hash_transfer = snapshot.zk_last_env_hash_transfer.clone();
        self.zk_last_env_hash_unshield = snapshot.zk_last_env_hash_unshield.clone();
        self.zk_last_env_hash_ballot = snapshot.zk_last_env_hash_ballot.clone();
        self.zk_last_env_hash_tally = snapshot.zk_last_env_hash_tally.clone();
        self.axt_replay_ledger = snapshot.axt_replay_ledger.clone();
        self.axt_proof_cache_slot = snapshot.axt_proof_cache_slot;
        self.axt_proof_cache = snapshot.axt_proof_cache.clone();
        self.last_axt_reject = snapshot.last_axt_reject.clone();
        self.amx_budget_violation = snapshot.amx_budget_violation;
    }

    fn encode_nested_contract_return(
        vm: &IVM,
        schema: &NestedCallReturnSchema,
    ) -> Result<Option<Vec<u8>>, ivm::VMError> {
        match schema {
            NestedCallReturnSchema::Unit => Ok(None),
            NestedCallReturnSchema::Int => {
                let value = vm.register(10) as i64;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::Bool => norito::to_bytes(&(vm.register(10) != 0))
                .map(Some)
                .map_err(|_| ivm::VMError::NoritoInvalid),
            NestedCallReturnSchema::Numeric => {
                let value: iroha_primitives::numeric::Numeric =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::NoritoBytes)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::Json => {
                let value = Self::decode_tlv_json(vm, vm.register(10))?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::Name => {
                let value: Name = Self::decode_tlv_typed(vm, vm.register(10), PointerType::Name)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::AccountId => {
                let value: AccountId =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::AccountId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::AssetDefinitionId => {
                let value: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::AssetDefinitionId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::AssetId => {
                let value: AssetId =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::AssetId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::DomainId => {
                let value: DomainId =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::DomainId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::NftId => {
                let value: NftId = Self::decode_tlv_typed(vm, vm.register(10), PointerType::NftId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::Blob | NestedCallReturnSchema::Bytes => {
                let value = Self::decode_tlv_blob(vm, vm.register(10))?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::DataSpaceId => {
                let value: DataSpaceId =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::DataSpaceId)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::AxtDescriptor => {
                let value: iroha_data_model::nexus::AxtDescriptor =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::AxtDescriptor)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
            NestedCallReturnSchema::AssetHandle => {
                let value: iroha_data_model::nexus::AssetHandle =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::AssetHandle)?;
                norito::to_bytes(&value)
                    .map(Some)
                    .map_err(|_| ivm::VMError::NoritoInvalid)
            }
        }
    }

    fn resolve_bound_contract_record_by_address(
        &self,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
    ) -> Option<crate::smartcontracts::code::BoundContractRecord> {
        if let Some(record) = self
            .bound_contract_records_by_subject
            .values()
            .find(|record| record.contract_address == *contract_address)
            .cloned()
        {
            return Some(record);
        }

        self.query_state
            .get()
            .and_then(|state_ref| state_ref.bound_contract_record_by_address(contract_address))
    }

    fn handle_call_contract(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        let mut request_bytes = 0usize;
        let caller_context = self
            .current_contract_runtime_context
            .clone()
            .ok_or_else(|| {
                ivm::VMError::metered(ivm::gas::G_CALL_CONTRACT, ivm::VMError::PermissionDenied)
            })?;
        let contract_literal_blob = Self::decode_tlv_blob(vm, vm.register(10))
            .map_err(|err| ivm::VMError::metered(ivm::gas::G_CALL_CONTRACT, err))?;
        request_bytes = request_bytes.saturating_add(contract_literal_blob.len());
        let contract_literal = String::from_utf8(contract_literal_blob).map_err(|_| {
            ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                ivm::VMError::DecodeError,
            )
        })?;
        let contract_address = contract_literal
            .parse::<iroha_data_model::smart_contract::ContractAddress>()
            .map_err(|_| {
                ivm::VMError::metered(
                    ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                    ivm::VMError::PermissionDenied,
                )
            })?;
        let entrypoint_blob = Self::decode_tlv_blob(vm, vm.register(11)).map_err(|err| {
            ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                err,
            )
        })?;
        request_bytes = request_bytes.saturating_add(entrypoint_blob.len());
        let entrypoint = String::from_utf8(entrypoint_blob).map_err(|_| {
            ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                ivm::VMError::DecodeError,
            )
        })?;
        if entrypoint.trim().is_empty() {
            return Err(ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                ivm::VMError::PermissionDenied,
            ));
        }
        let payload = Self::decode_tlv_json(vm, vm.register(12)).map_err(|err| {
            ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                err,
            )
        })?;
        let payload_bytes = norito::to_bytes(&payload).map_err(|_| {
            ivm::VMError::metered(
                ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0),
                ivm::VMError::NoritoInvalid,
            )
        })?;
        request_bytes = request_bytes.saturating_add(payload_bytes.len());
        let request_gas = ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, 0);
        let record = self
            .resolve_bound_contract_record_by_address(&contract_address)
            .ok_or_else(|| ivm::VMError::metered(request_gas, ivm::VMError::PermissionDenied))?;
        let invocation = iroha_data_model::transaction::executable::ContractInvocation {
            contract_address: record.contract_address.clone(),
            entrypoint,
            payload: Some(payload),
        };
        let entrypoint_name = invocation.entrypoint.clone();
        let call_context = crate::executor::parse_contract_invocation_execution_context(
            &invocation,
            record.code_bytes.as_ref(),
            record.contract_alias.clone(),
        )
        .map_err(|err| ivm::VMError::metered(request_gas, map_validation_fail(&err)))?;
        let callee_context = call_context
            .runtime_context()
            .ok_or_else(|| ivm::VMError::metered(request_gas, ivm::VMError::PermissionDenied))?;
        let return_type = record
            .manifest
            .entrypoints
            .as_ref()
            .and_then(|entrypoints| {
                entrypoints
                    .iter()
                    .find(|descriptor| descriptor.name == entrypoint_name)
            })
            .and_then(|descriptor| descriptor.return_type.as_deref());
        let return_schema = NestedCallReturnSchema::parse(return_type)
            .map_err(|err| ivm::VMError::metered(request_gas, err))?;
        if vm.remaining_gas() < request_gas {
            return Err(ivm::VMError::OutOfGas);
        }
        let child_gas_limit = vm.remaining_gas().saturating_sub(request_gas);
        let mut child_vm = IVM::new(child_gas_limit);
        child_vm
            .load_program(record.code_bytes.as_ref())
            .map_err(|_| ivm::VMError::metered(request_gas, ivm::VMError::DecodeError))?;
        if let Some(entrypoint_pc) = call_context.entrypoint_pc() {
            child_vm.set_register(1, child_vm.memory.code_len());
            child_vm
                .set_program_counter(entrypoint_pc)
                .map_err(|err| ivm::VMError::metered(request_gas, err))?;
        }
        child_vm.set_gas_limit(child_gas_limit);

        let snapshot = self.snapshot_nested_contract_call();
        self.authority = caller_context.contract_subject.clone();
        self.current_contract_runtime_context = Some(callee_context);
        self.args = Some(call_context.args().clone());
        self.fastpq_batch_entries = None;

        let run_result = child_vm.run_with_host(self);
        match run_result {
            Ok(()) => {
                let encoded_return = Self::encode_nested_contract_return(&child_vm, &return_schema)
                    .map_err(|err| ivm::VMError::metered(request_gas, err))?;
                vm.gas_remaining = child_vm.remaining_gas().saturating_add(request_gas);
                self.restore_nested_contract_call_frame(&snapshot);
                match encoded_return {
                    Some(encoded_return) => {
                        let return_bytes = encoded_return.len();
                        let ptr = Self::alloc_norito_bytes(vm, &encoded_return).map_err(|err| {
                            ivm::VMError::metered(
                                ivm::gas::syscall_byte_gas(
                                    ivm::gas::G_CALL_CONTRACT,
                                    request_bytes,
                                    return_bytes,
                                ),
                                err,
                            )
                        })?;
                        vm.set_register(10, ptr);
                        Ok(ivm::gas::syscall_byte_gas(
                            ivm::gas::G_CALL_CONTRACT,
                            request_bytes,
                            return_bytes,
                        ))
                    }
                    None => {
                        vm.set_register(10, 0);
                        Ok(request_gas)
                    }
                }
            }
            Err(err) => {
                vm.gas_remaining = child_vm.remaining_gas().saturating_add(request_gas);
                self.rollback_nested_contract_call(&snapshot);
                Err(ivm::VMError::metered(request_gas, err))
            }
        }
    }

    const QUERY_GAS_BASE_SINGULAR: u64 = 1_000;
    const QUERY_GAS_BASE_ITERABLE: u64 = 2_500;
    const QUERY_GAS_PER_ITEM: u64 = 250;
    const QUERY_GAS_SORT_MULTIPLIER: u64 = 4;
    const QUERY_GAS_PER_BYTE: u64 = 2;

    #[cfg(test)]
    fn query_total_items(response: &QueryResponse) -> u64 {
        match response {
            QueryResponse::Singular(_) => 1,
            QueryResponse::Iterable(output) => {
                let returned = output
                    .batch
                    .iter()
                    .map(|batch| u64::try_from(batch.len()).unwrap_or(u64::MAX))
                    .fold(0_u64, u64::saturating_add);
                returned.saturating_add(output.remaining_items_hint())
            }
        }
    }

    fn query_sort_requested(request: &QueryRequest) -> bool {
        match request {
            QueryRequest::Start(start) => start.params.sorting.sort_by_metadata_key.is_some(),
            QueryRequest::Singular(_) | QueryRequest::Continue(_) => false,
        }
    }

    fn query_offset_items(request: &QueryRequest) -> u64 {
        match request {
            QueryRequest::Start(start) => start.params.pagination.offset_value(),
            QueryRequest::Singular(_) | QueryRequest::Continue(_) => 0,
        }
    }

    fn query_gas_cost(ctx: &QueryGasContext, processed_items: u64, response_bytes: u64) -> u64 {
        ctx.base
            .saturating_add(ctx.per_item.saturating_mul(processed_items))
            .saturating_add(ctx.per_item.saturating_mul(ctx.offset_items))
            .saturating_add(Self::QUERY_GAS_PER_BYTE.saturating_mul(response_bytes))
    }

    fn subscription_bill_account_alias(
        &mut self,
        trigger_id: &TriggerId,
        context: SubscriptionContext,
        metadata: AccountAliasAutoRenewMetadata,
        charge_asset_def: AssetDefinition,
    ) -> Result<u64, ivm::VMError> {
        let mut subscription_state = context.subscription_state;
        if subscription_state.subscriber != context.nft_owner {
            return Err(ivm::VMError::NoritoInvalid);
        }
        subscription_state.billing_trigger_id = trigger_id.clone();

        let scheduled_at_ms = subscription_state.next_charge_ms;
        let now_ms = self.current_block_time_ms.unwrap_or(scheduled_at_ms);
        let attempted_at_ms = now_ms.max(scheduled_at_ms);
        let previous_period_end_ms = subscription_state.current_period_end_ms;
        let quote = {
            let Some(state_ref) = self.query_state.get() else {
                return Err(ivm::VMError::NotImplemented {
                    syscall: ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL,
                });
            };
            state_ref.quote_account_alias_renewal(
                &metadata.alias,
                metadata.term_years,
                attempted_at_ms,
            )
        };

        let mut gas = 0_u64;
        let invoice;
        match quote {
            Ok(quote) => {
                let charge_amount = crate::sns::quote_charge_amount_to_numeric(quote.charge_amount);
                let within_cap = charge_amount <= metadata.max_charge_amount.clone();
                let can_pay = within_cap && charge_amount <= context.subscriber_balance;
                invoice = SubscriptionInvoice {
                    subscription_nft_id: context.subscription_nft_id.clone(),
                    period_start_ms: previous_period_end_ms,
                    period_end_ms: quote.expires_at_ms,
                    attempted_at_ms,
                    amount: charge_amount.clone(),
                    asset_definition: charge_asset_def.id.clone(),
                    status: if can_pay {
                        SubscriptionInvoiceStatus::Paid
                    } else {
                        SubscriptionInvoiceStatus::Failed
                    },
                    tx_hash: None,
                };
                if can_pay {
                    let alias = {
                        let Some(state_ref) = self.query_state.get() else {
                            return Err(ivm::VMError::NotImplemented {
                                syscall: ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL,
                            });
                        };
                        state_ref.parse_account_alias(&metadata.alias)?
                    };
                    let renew =
                        iroha_data_model::isi::account_alias_lease::RenewAccountAliasLease::new(
                            alias,
                            subscription_state.subscriber.clone(),
                            metadata.term_years,
                        );
                    gas = gas.saturating_add(self.queue_instruction(InstructionBox::from(renew)));
                    subscription_state.failure_count = 0;
                    subscription_state.status = SubscriptionStatus::Active;
                    subscription_state.current_period_start_ms = previous_period_end_ms;
                    subscription_state.current_period_end_ms = quote.expires_at_ms;
                    subscription_state.next_charge_ms = quote.expires_at_ms;
                } else {
                    subscription_state.failure_count =
                        subscription_state.failure_count.saturating_add(1);
                    subscription_state.status =
                        if subscription_state.failure_count >= metadata.max_failures {
                            SubscriptionStatus::Suspended
                        } else {
                            SubscriptionStatus::PastDue
                        };
                    if subscription_state.status != SubscriptionStatus::Suspended {
                        subscription_state.next_charge_ms =
                            attempted_at_ms.saturating_add(metadata.retry_backoff_ms);
                    }
                }
            }
            Err(_) => {
                invoice = SubscriptionInvoice {
                    subscription_nft_id: context.subscription_nft_id.clone(),
                    period_start_ms: previous_period_end_ms,
                    period_end_ms: previous_period_end_ms,
                    attempted_at_ms,
                    amount: Numeric::zero(),
                    asset_definition: charge_asset_def.id.clone(),
                    status: SubscriptionInvoiceStatus::Failed,
                    tx_hash: None,
                };
                subscription_state.failure_count = metadata.max_failures.max(1);
                subscription_state.status = SubscriptionStatus::Suspended;
            }
        }

        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry.as_ref() {
            let outcome = match subscription_state.status {
                SubscriptionStatus::Active => "paid",
                SubscriptionStatus::Suspended => "suspended",
                _ => "failed",
            };
            telemetry.record_subscription_billing_outcome("sns_alias", outcome);
        }

        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let subscription_json = iroha_primitives::json::Json::new(subscription_state.clone());
        let subscription_isi = SetKeyValue::nft(
            context.subscription_nft_id.clone(),
            subscription_key,
            subscription_json,
        );
        gas = gas.saturating_add(
            self.queue_instruction(InstructionBox::from(SetKeyValueBox::from(subscription_isi))),
        );

        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let invoice_json = iroha_primitives::json::Json::new(invoice);
        let invoice_isi = SetKeyValue::nft(
            context.subscription_nft_id.clone(),
            invoice_key,
            invoice_json,
        );
        gas = gas.saturating_add(
            self.queue_instruction(InstructionBox::from(SetKeyValueBox::from(invoice_isi))),
        );

        if matches!(
            subscription_state.status,
            SubscriptionStatus::Suspended | SubscriptionStatus::Canceled
        ) {
            let unregister =
                InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
            gas = gas.saturating_add(self.queue_instruction(unregister));
            return Ok(gas);
        }

        let schedule = Schedule {
            start_ms: subscription_state.next_charge_ms,
            period_ms: None,
        };
        let mut next_trigger_metadata = context.trigger_metadata.clone();
        for key in ["__registered_block_height", "__registered_at_ms"] {
            let key = key
                .parse::<Name>()
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            next_trigger_metadata.remove(&key);
        }
        let action = iroha_data_model::trigger::action::Action::new(
            context.executable.clone(),
            Repeats::Exactly(1),
            context.authority.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        )
        .with_metadata(next_trigger_metadata);
        let trigger = Trigger::new(trigger_id.clone(), action);
        let unregister =
            InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
        gas = gas.saturating_add(self.queue_instruction(unregister));
        let register = InstructionBox::from(RegisterBox::from(Register::trigger(trigger)));
        gas = gas.saturating_add(self.queue_instruction(register));
        Ok(gas)
    }

    #[allow(clippy::too_many_lines)]
    fn subscription_bill(&mut self) -> Result<u64, ivm::VMError> {
        struct BillingWindow {
            start_ms: u64,
            end_ms: u64,
        }

        let trigger_id = self
            .current_trigger_id
            .clone()
            .ok_or(ivm::VMError::NotImplemented {
                syscall: ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL,
            })?;
        let context = {
            let Some(state_ref) = self.query_state.get() else {
                return Err(ivm::VMError::NotImplemented {
                    syscall: ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL,
                });
            };
            state_ref.subscription_context_for_trigger(&trigger_id)?
        };

        let mut subscription_state = context.subscription_state.clone();
        if subscription_state.subscriber != context.nft_owner {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let billing = context.billing.clone();
        if let SubscriptionBillingKind::Generic { plan, .. } = &billing
            && subscription_state.provider != plan.provider
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        subscription_state.billing_trigger_id = trigger_id.clone();

        #[cfg(feature = "telemetry")]
        let pricing_label = match &billing {
            SubscriptionBillingKind::Generic { plan, .. } => match &plan.pricing {
                SubscriptionPricing::Fixed(_) => "fixed",
                SubscriptionPricing::Usage(_) => "usage",
            },
            SubscriptionBillingKind::AccountAliasAutoRenew { .. } => "sns_alias",
        };
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.record_subscription_billing_attempt(pricing_label);
        }

        if matches!(
            subscription_state.status,
            SubscriptionStatus::Paused
                | SubscriptionStatus::Canceled
                | SubscriptionStatus::Suspended
        ) {
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry.as_ref() {
                telemetry.record_subscription_billing_outcome(pricing_label, "skipped");
            }
            let instr =
                InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
            return Ok(self.queue_instruction(instr));
        }

        if let SubscriptionBillingKind::AccountAliasAutoRenew {
            metadata,
            charge_asset_def,
        } = billing
        {
            return self.subscription_bill_account_alias(
                &trigger_id,
                context,
                metadata,
                charge_asset_def,
            );
        }

        let SubscriptionBillingKind::Generic {
            plan,
            charge_asset_def,
        } = billing
        else {
            unreachable!("subscription billing kind is exhaustive");
        };

        let billing = plan.billing;
        let scheduled_at_ms = subscription_state.next_charge_ms;
        let now_ms = self.current_block_time_ms.unwrap_or(scheduled_at_ms);
        let attempted_at_ms = now_ms.max(scheduled_at_ms);
        let bill_for = match billing.bill_for {
            SubscriptionBillFor::PreviousPeriod => calendar::BillingPeriod::Previous,
            SubscriptionBillFor::NextPeriod => calendar::BillingPeriod::Next,
        };

        let (charge_period, next_period, next_charge_ms) = match billing.cadence {
            SubscriptionCadence::MonthlyCalendar(cadence) => {
                let charge = calendar::monthly_billing_period(
                    scheduled_at_ms,
                    cadence.anchor_day,
                    cadence.anchor_time_ms,
                    bill_for,
                )
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
                let next_charge = calendar::monthly_anchor_after(
                    scheduled_at_ms,
                    cadence.anchor_day,
                    cadence.anchor_time_ms,
                )
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
                let next = calendar::monthly_billing_period(
                    scheduled_at_ms,
                    cadence.anchor_day,
                    cadence.anchor_time_ms,
                    calendar::BillingPeriod::Next,
                )
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
                (
                    BillingWindow {
                        start_ms: charge.start_ms,
                        end_ms: charge.end_ms,
                    },
                    BillingWindow {
                        start_ms: next.start_ms,
                        end_ms: next.end_ms,
                    },
                    next_charge,
                )
            }
            SubscriptionCadence::FixedPeriod(cadence) => {
                if cadence.period_ms == 0 {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let (start, end) = match billing.bill_for {
                    SubscriptionBillFor::PreviousPeriod => (
                        scheduled_at_ms
                            .checked_sub(cadence.period_ms)
                            .ok_or(ivm::VMError::NoritoInvalid)?,
                        scheduled_at_ms,
                    ),
                    SubscriptionBillFor::NextPeriod => (
                        scheduled_at_ms,
                        scheduled_at_ms
                            .checked_add(cadence.period_ms)
                            .ok_or(ivm::VMError::NoritoInvalid)?,
                    ),
                };
                let next_charge = scheduled_at_ms
                    .checked_add(cadence.period_ms)
                    .ok_or(ivm::VMError::NoritoInvalid)?;
                (
                    BillingWindow {
                        start_ms: start,
                        end_ms: end,
                    },
                    BillingWindow {
                        start_ms: scheduled_at_ms,
                        end_ms: next_charge,
                    },
                    next_charge,
                )
            }
        };

        let mut gas = 0_u64;
        if subscription_state.cancel_at_period_end {
            let cancel_at_ms = subscription_state
                .cancel_at_ms
                .unwrap_or(subscription_state.current_period_end_ms);
            if charge_period.start_ms >= cancel_at_ms {
                subscription_state.status = SubscriptionStatus::Canceled;
                subscription_state.cancel_at_period_end = false;
                subscription_state.cancel_at_ms = None;
                #[cfg(feature = "telemetry")]
                if let Some(telemetry) = self.telemetry.as_ref() {
                    telemetry.record_subscription_billing_outcome(pricing_label, "skipped");
                }
                let subscription_key: Name = SUBSCRIPTION_METADATA_KEY
                    .parse()
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                let subscription_json =
                    iroha_primitives::json::Json::new(subscription_state.clone());
                let isi = SetKeyValue::nft(
                    context.subscription_nft_id.clone(),
                    subscription_key,
                    subscription_json,
                );
                let instr = InstructionBox::from(SetKeyValueBox::from(isi));
                gas = gas.saturating_add(self.queue_instruction(instr));
                let unregister = InstructionBox::from(UnregisterBox::from(Unregister::trigger(
                    trigger_id.clone(),
                )));
                gas = gas.saturating_add(self.queue_instruction(unregister));
                return Ok(gas);
            }
        }

        let charge_spec = charge_asset_def.spec();
        let (amount, usage_key) = match &plan.pricing {
            SubscriptionPricing::Fixed(pricing) => {
                let fixed_amount = pricing.amount.clone();
                if fixed_amount < Numeric::zero() {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                if charge_spec.check(&fixed_amount).is_err() {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                (fixed_amount, None)
            }
            SubscriptionPricing::Usage(pricing) => {
                let usage = subscription_state
                    .usage_accumulated
                    .get(&pricing.unit_key)
                    .cloned()
                    .unwrap_or_else(Numeric::zero);
                let unit_price = pricing.unit_price.clone();
                if usage < Numeric::zero() || unit_price < Numeric::zero() {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let amount = usage
                    .checked_mul(unit_price, charge_spec)
                    .ok_or(ivm::VMError::NoritoInvalid)?;
                if charge_spec.check(&amount).is_err() {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                (amount, Some(pricing.unit_key.clone()))
            }
        };

        let can_pay = amount <= context.subscriber_balance;
        let invoice_status = if can_pay {
            SubscriptionInvoiceStatus::Paid
        } else {
            SubscriptionInvoiceStatus::Failed
        };
        let invoice = SubscriptionInvoice {
            subscription_nft_id: context.subscription_nft_id.clone(),
            period_start_ms: charge_period.start_ms,
            period_end_ms: charge_period.end_ms,
            attempted_at_ms,
            amount: amount.clone(),
            asset_definition: charge_asset_def.id.clone(),
            status: invoice_status,
            tx_hash: None,
        };
        if can_pay {
            if !amount.is_zero() {
                let asset_id = AssetId::of(
                    charge_asset_def.id.clone(),
                    subscription_state.subscriber.clone(),
                );
                let isi = Transfer::asset_numeric(asset_id, amount.clone(), plan.provider.clone());
                let instr = InstructionBox::from(TransferBox::from(isi));
                gas = gas.saturating_add(self.queue_instruction(instr));
            }
            subscription_state.failure_count = 0;
            subscription_state.status = SubscriptionStatus::Active;
            subscription_state.next_charge_ms = next_charge_ms;
            let (period_start, period_end) = match billing.bill_for {
                SubscriptionBillFor::PreviousPeriod => (next_period.start_ms, next_period.end_ms),
                SubscriptionBillFor::NextPeriod => (charge_period.start_ms, charge_period.end_ms),
            };
            subscription_state.current_period_start_ms = period_start;
            subscription_state.current_period_end_ms = period_end;
            if let Some(key) = usage_key {
                subscription_state.usage_accumulated.remove(&key);
            }
        } else {
            subscription_state.failure_count = subscription_state.failure_count.saturating_add(1);
            let grace_deadline = scheduled_at_ms.saturating_add(billing.grace_ms);
            let mut status = subscription_state.status;
            if subscription_state.failure_count >= billing.max_failures {
                status = SubscriptionStatus::Suspended;
            } else if attempted_at_ms >= grace_deadline {
                status = SubscriptionStatus::PastDue;
            }
            subscription_state.status = status;
            if status != SubscriptionStatus::Suspended {
                subscription_state.next_charge_ms =
                    attempted_at_ms.saturating_add(billing.retry_backoff_ms);
            }
        }

        if subscription_state.cancel_at_period_end {
            let cancel_at_ms = subscription_state
                .cancel_at_ms
                .unwrap_or(subscription_state.current_period_end_ms);
            if charge_period.end_ms >= cancel_at_ms {
                subscription_state.status = SubscriptionStatus::Canceled;
                subscription_state.cancel_at_period_end = false;
                subscription_state.cancel_at_ms = None;
            }
        }
        #[cfg(feature = "telemetry")]
        let outcome = if can_pay {
            "paid"
        } else if subscription_state.status == SubscriptionStatus::Suspended {
            "suspended"
        } else {
            "failed"
        };
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.record_subscription_billing_outcome(pricing_label, outcome);
        }

        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let subscription_json = iroha_primitives::json::Json::new(subscription_state.clone());
        let isi = SetKeyValue::nft(
            context.subscription_nft_id.clone(),
            subscription_key,
            subscription_json,
        );
        let instr = InstructionBox::from(SetKeyValueBox::from(isi));
        gas = gas.saturating_add(self.queue_instruction(instr));

        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let invoice_json = iroha_primitives::json::Json::new(invoice);
        let invoice_isi = SetKeyValue::nft(
            context.subscription_nft_id.clone(),
            invoice_key,
            invoice_json,
        );
        let invoice_instr = InstructionBox::from(SetKeyValueBox::from(invoice_isi));
        gas = gas.saturating_add(self.queue_instruction(invoice_instr));

        if matches!(
            subscription_state.status,
            SubscriptionStatus::Suspended | SubscriptionStatus::Canceled
        ) {
            let instr =
                InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
            gas = gas.saturating_add(self.queue_instruction(instr));
            return Ok(gas);
        }

        let schedule = Schedule {
            start_ms: subscription_state.next_charge_ms,
            period_ms: None,
        };
        let mut next_trigger_metadata = context.trigger_metadata.clone();
        for key in ["__registered_block_height", "__registered_at_ms"] {
            let key = key
                .parse::<Name>()
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            next_trigger_metadata.remove(&key);
        }
        let action = iroha_data_model::trigger::action::Action::new(
            context.executable.clone(),
            Repeats::Exactly(1),
            context.authority.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        )
        .with_metadata(next_trigger_metadata);
        let trigger = Trigger::new(trigger_id.clone(), action);
        let unregister =
            InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
        gas = gas.saturating_add(self.queue_instruction(unregister));
        let register = InstructionBox::from(RegisterBox::from(Register::trigger(trigger)));
        gas = gas.saturating_add(self.queue_instruction(register));

        Ok(gas)
    }

    fn subscription_record_usage(&mut self) -> Result<u64, ivm::VMError> {
        let args = self.args.as_ref().ok_or(ivm::VMError::NoritoInvalid)?;
        let delta: SubscriptionUsageDelta = args
            .try_into_any_norito()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        if delta.delta < Numeric::zero() {
            return Err(ivm::VMError::NoritoInvalid);
        }

        let (mut subscription_state, plan) = {
            let Some(state_ref) = self.query_state.get() else {
                return Err(ivm::VMError::NotImplemented {
                    syscall: ivm::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE,
                });
            };
            let subscription_state =
                state_ref.subscription_state_for_nft(&delta.subscription_nft_id)?;
            let plan = state_ref.subscription_plan(&subscription_state.plan_id)?;
            (subscription_state, plan)
        };

        if !matches!(
            subscription_state.status,
            SubscriptionStatus::Active | SubscriptionStatus::PastDue
        ) {
            return Err(ivm::VMError::PermissionDenied);
        }

        match &plan.pricing {
            SubscriptionPricing::Usage(pricing) => {
                if pricing.unit_key != delta.unit_key {
                    return Err(ivm::VMError::PermissionDenied);
                }
            }
            SubscriptionPricing::Fixed(_) => {
                return Err(ivm::VMError::PermissionDenied);
            }
        }

        let entry = subscription_state
            .usage_accumulated
            .entry(delta.unit_key.clone())
            .or_insert_with(Numeric::zero);
        let updated = entry
            .clone()
            .checked_add(delta.delta.clone())
            .ok_or(ivm::VMError::NoritoInvalid)?;
        *entry = updated;

        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let subscription_json = iroha_primitives::json::Json::new(subscription_state);
        let isi = SetKeyValue::nft(
            delta.subscription_nft_id,
            subscription_key,
            subscription_json,
        );
        let instr = InstructionBox::from(SetKeyValueBox::from(isi));
        Ok(self.queue_instruction(instr))
    }

    fn subscription_context_for_trigger<S: StateReadOnly>(
        state: &S,
        trigger_id: &TriggerId,
    ) -> Result<SubscriptionContext, ivm::VMError> {
        let triggers = state.world().triggers();
        let action = triggers
            .time_triggers()
            .get(trigger_id)
            .cloned()
            .ok_or(ivm::VMError::NoritoInvalid)?;
        let action = triggers
            .get_original_action(action)
            .ok_or(ivm::VMError::NoritoInvalid)?;
        let trigger_metadata = action.metadata.clone();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let trigger_ref_json = trigger_metadata
            .get(&trigger_ref_key)
            .ok_or(ivm::VMError::NoritoInvalid)?;
        let trigger_ref: SubscriptionTriggerRef = trigger_ref_json
            .try_into_any_norito()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;

        let subscription_nft_id = trigger_ref.subscription_nft_id;
        let (subscription_state, nft_owner) =
            Self::subscription_state_and_owner(state, &subscription_nft_id)?;
        let nft = state
            .world()
            .nft(&subscription_nft_id)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let alias_auto_renew_key: Name = ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let billing = if let Some(value) = nft.value().content.get(&alias_auto_renew_key) {
            let metadata = value
                .try_into_any_norito::<AccountAliasAutoRenewMetadata>()
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            let charge_asset_def = state
                .world()
                .asset_definition(&subscription_state.plan_id)
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
            SubscriptionBillingKind::AccountAliasAutoRenew {
                metadata,
                charge_asset_def,
            }
        } else {
            let plan = Self::subscription_plan(state, &subscription_state.plan_id)?;
            let charge_asset_def = {
                let charge_asset_id = match &plan.pricing {
                    SubscriptionPricing::Fixed(pricing) => &pricing.asset_definition,
                    SubscriptionPricing::Usage(pricing) => &pricing.asset_definition,
                };
                state
                    .world()
                    .asset_definition(charge_asset_id)
                    .map_err(|_| ivm::VMError::NoritoInvalid)?
            };
            SubscriptionBillingKind::Generic {
                plan,
                charge_asset_def,
            }
        };
        let charge_asset_definition_id = match &billing {
            SubscriptionBillingKind::Generic {
                charge_asset_def, ..
            }
            | SubscriptionBillingKind::AccountAliasAutoRenew {
                charge_asset_def, ..
            } => charge_asset_def.id.clone(),
        };

        let balance = {
            let asset_id = AssetId::of(
                charge_asset_definition_id,
                subscription_state.subscriber.clone(),
            );
            state.world().asset(&asset_id).map_or_else(
                |_| Numeric::zero(),
                |entry| entry.value().clone().into_inner(),
            )
        };

        Ok(SubscriptionContext {
            executable: action.executable.clone(),
            authority: action.authority.clone(),
            trigger_metadata,
            subscription_nft_id,
            subscription_state,
            billing,
            subscriber_balance: balance,
            nft_owner,
        })
    }

    fn subscription_state_for_nft<S: StateReadOnly>(
        state: &S,
        nft_id: &NftId,
    ) -> Result<SubscriptionState, ivm::VMError> {
        let (state, _) = Self::subscription_state_and_owner(state, nft_id)?;
        Ok(state)
    }

    fn subscription_state_and_owner<S: StateReadOnly>(
        state: &S,
        nft_id: &NftId,
    ) -> Result<(SubscriptionState, AccountId), ivm::VMError> {
        let entry = state
            .world()
            .nft(nft_id)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let value = entry
            .value()
            .content
            .get(&subscription_key)
            .ok_or(ivm::VMError::NoritoInvalid)?;
        let subscription_state = value
            .try_into_any_norito::<SubscriptionState>()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let owner = entry.value().owned_by.clone();
        Ok((subscription_state, owner))
    }

    fn subscription_plan<S: StateReadOnly>(
        state: &S,
        plan_id: &AssetDefinitionId,
    ) -> Result<SubscriptionPlan, ivm::VMError> {
        let asset_def = state
            .world()
            .asset_definition(plan_id)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let value = asset_def
            .metadata()
            .get(&plan_key)
            .ok_or(ivm::VMError::NoritoInvalid)?;
        value
            .try_into_any_norito::<SubscriptionPlan>()
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn parse_account_alias<S: StateReadOnly>(
        state: &S,
        alias_literal: &str,
    ) -> Result<AccountAlias, ivm::VMError> {
        AccountAlias::from_literal(alias_literal, &state.nexus().dataspace_catalog)
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn quote_account_alias_renewal<S: StateReadOnly>(
        state: &S,
        alias_literal: &str,
        term_years: u8,
        now_ms: u64,
    ) -> Result<crate::sns::LeaseQuote, ivm::VMError> {
        let alias = AccountAlias::from_literal(alias_literal, &state.nexus().dataspace_catalog)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        crate::sns::quote_account_alias_renewal(
            state.world(),
            &state.nexus().dataspace_catalog,
            &alias,
            term_years,
            now_ms,
        )
        .map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn nft_by_id<S: StateReadOnly>(state: &S, nft_id: &NftId) -> Result<Nft, ivm::VMError> {
        let entry = state
            .world()
            .nft(nft_id)
            .map_err(|_| ivm::VMError::DecodeError)?;
        let data = entry.value().clone().into_inner();
        Ok(Nft {
            id: entry.id().clone(),
            content: data.content,
            owned_by: data.owned_by,
        })
    }

    fn parameter_by_name<S: StateReadOnly>(
        state: &S,
        name: &Name,
    ) -> Result<Parameter, ivm::VMError> {
        use iroha_data_model::parameter::{
            BlockParameter, CustomParameterId, SmartContractParameter, SumeragiParameter,
            TransactionParameter,
        };

        let params = state.world().parameters();
        if let Some(custom) = params
            .custom()
            .get(&CustomParameterId::new(name.clone()))
            .cloned()
        {
            return Ok(Parameter::Custom(custom));
        }

        let parameter = match name.as_ref() {
            "sumeragi.block_time_ms" => Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(
                params.sumeragi().block_time_ms(),
            )),
            "sumeragi.commit_time_ms" => Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(
                params.sumeragi().commit_time_ms(),
            )),
            "sumeragi.min_finality_ms" => Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(
                params.sumeragi().min_finality_ms(),
            )),
            "sumeragi.pacing_factor_bps" => Parameter::Sumeragi(
                SumeragiParameter::PacingFactorBps(params.sumeragi().pacing_factor_bps()),
            ),
            "sumeragi.max_clock_drift_ms" => Parameter::Sumeragi(
                SumeragiParameter::MaxClockDriftMs(params.sumeragi().max_clock_drift_ms()),
            ),
            "sumeragi.collectors_k" => Parameter::Sumeragi(SumeragiParameter::CollectorsK(
                params.sumeragi().collectors_k(),
            )),
            "sumeragi.collectors_redundant_send_r" => Parameter::Sumeragi(
                SumeragiParameter::RedundantSendR(params.sumeragi().collectors_redundant_send_r()),
            ),
            "sumeragi.da_enabled" => {
                Parameter::Sumeragi(SumeragiParameter::DaEnabled(params.sumeragi().da_enabled()))
            }
            "block.max_transactions" => Parameter::Block(BlockParameter::MaxTransactions(
                params.block().max_transactions(),
            )),
            "transaction.max_signatures" => Parameter::Transaction(
                TransactionParameter::MaxSignatures(params.transaction().max_signatures()),
            ),
            "transaction.max_instructions" => Parameter::Transaction(
                TransactionParameter::MaxInstructions(params.transaction().max_instructions()),
            ),
            "transaction.ivm_bytecode_size" => Parameter::Transaction(
                TransactionParameter::IvmBytecodeSize(params.transaction().ivm_bytecode_size()),
            ),
            "transaction.max_tx_bytes" => Parameter::Transaction(TransactionParameter::MaxTxBytes(
                params.transaction().max_tx_bytes(),
            )),
            "transaction.max_decompressed_bytes" => {
                Parameter::Transaction(TransactionParameter::MaxDecompressedBytes(
                    params.transaction().max_decompressed_bytes(),
                ))
            }
            "transaction.max_metadata_depth" => Parameter::Transaction(
                TransactionParameter::MaxMetadataDepth(params.transaction().max_metadata_depth()),
            ),
            "transaction.require_height_ttl" => Parameter::Transaction(
                TransactionParameter::RequireHeightTtl(params.transaction().require_height_ttl()),
            ),
            "transaction.require_sequence" => Parameter::Transaction(
                TransactionParameter::RequireSequence(params.transaction().require_sequence()),
            ),
            "smart_contract.fuel" => Parameter::SmartContract(SmartContractParameter::Fuel(
                params.smart_contract().fuel(),
            )),
            "smart_contract.memory" => Parameter::SmartContract(SmartContractParameter::Memory(
                params.smart_contract().memory(),
            )),
            "smart_contract.execution_depth" => Parameter::SmartContract(
                SmartContractParameter::ExecutionDepth(params.smart_contract().execution_depth()),
            ),
            "executor.fuel" => {
                Parameter::Executor(SmartContractParameter::Fuel(params.executor().fuel()))
            }
            "executor.memory" => {
                Parameter::Executor(SmartContractParameter::Memory(params.executor().memory()))
            }
            "executor.execution_depth" => Parameter::Executor(
                SmartContractParameter::ExecutionDepth(params.executor().execution_depth()),
            ),
            _ => return Err(ivm::VMError::DecodeError),
        };
        Ok(parameter)
    }

    fn contract_instance_by_address<S: StateReadOnly>(
        state: &S,
        contract_address: &ContractAddress,
    ) -> Result<ContractInstance, ivm::VMError> {
        let code_hash = state
            .world()
            .contract_instances()
            .get(contract_address)
            .copied()
            .ok_or(ivm::VMError::DecodeError)?;
        let contract_alias = state
            .world()
            .contract_alias_bindings()
            .get(contract_address)
            .map(|binding| binding.alias.clone());
        Ok(ContractInstance {
            contract_address: contract_address.clone(),
            contract_alias,
            code_hash,
        })
    }

    fn contract_instance_by_alias<S: StateReadOnly>(
        state: &S,
        alias: &ContractAlias,
    ) -> Result<ContractInstance, ivm::VMError> {
        let contract_address = state
            .world()
            .contract_aliases()
            .get(alias)
            .cloned()
            .ok_or(ivm::VMError::DecodeError)?;
        let mut instance = Self::contract_instance_by_address(state, &contract_address)?;
        if instance.contract_alias.is_none() {
            instance.contract_alias = Some(alias.clone());
        }
        Ok(instance)
    }

    fn norito_encoded_len_exact<T: NoritoSerialize>(value: &T) -> Option<u64> {
        let payload_len = NoritoSerialize::encoded_len_exact(value)?;
        let header_len = Header::SIZE;
        let align = mem::align_of::<Archived<T>>();
        let padding = if align <= 1 {
            0
        } else {
            let rem = header_len % align;
            if rem == 0 { 0 } else { align - rem }
        };
        let total_len = header_len
            .saturating_add(padding)
            .saturating_add(payload_len);
        u64::try_from(total_len).ok()
    }

    fn query_items_budget(ctx: &QueryGasContext, gas_remaining: u64) -> Result<u64, ivm::VMError> {
        let base_cost = ctx
            .base
            .saturating_add(ctx.per_item.saturating_mul(ctx.offset_items));
        if gas_remaining < base_cost {
            return Err(ivm::VMError::OutOfGas);
        }
        let remaining = gas_remaining.saturating_sub(base_cost);
        if ctx.per_item == 0 {
            return Ok(u64::MAX);
        }
        Ok(remaining / ctx.per_item)
    }

    fn encode_norito_payload<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>, ivm::VMError> {
        norito::to_bytes(value).map_err(|_| ivm::VMError::NoritoInvalid)
    }

    fn execute_singular_query_payload(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        request: QueryRequest,
        payload_from_output: impl FnOnce(SingularQueryOutputBox) -> Result<Vec<u8>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError> {
        let gas_remaining = vm.remaining_gas();
        let gas_ctx = QueryGasContext::from_request(&request);
        let Some(state_ref) = self.query_state.get() else {
            return Err(ivm::VMError::NotImplemented { syscall });
        };
        let budget_items = Self::query_items_budget(&gas_ctx, gas_remaining)?;
        if budget_items == 0 {
            return Err(ivm::VMError::OutOfGas);
        }
        let query_result =
            state_ref.execute_query_with_budget(&self.authority, request, Some(budget_items))?;
        let output = match query_result.response {
            QueryResponse::Singular(output) => output,
            QueryResponse::Iterable(_) => return Err(ivm::VMError::DecodeError),
        };
        let payload = payload_from_output(output)?;
        let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        let gas = Self::query_gas_cost(&gas_ctx, query_result.processed_items, payload_len_u64);
        if gas > gas_remaining {
            return Err(ivm::VMError::OutOfGas);
        }
        let ptr = Self::alloc_norito_bytes(vm, &payload)?;
        vm.set_register(10, ptr);
        Ok(gas)
    }

    fn finish_direct_singular_query_payload(
        vm: &mut IVM,
        gas_remaining: u64,
        payload: Vec<u8>,
    ) -> Result<u64, ivm::VMError> {
        let gas_ctx = QueryGasContext::singular();
        let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        let gas = Self::query_gas_cost(&gas_ctx, 1, payload_len_u64);
        if gas > gas_remaining {
            return Err(ivm::VMError::OutOfGas);
        }
        let ptr = Self::alloc_norito_bytes(vm, &payload)?;
        vm.set_register(10, ptr);
        Ok(gas)
    }

    fn gas_for_zk_verify_payload(payload: &[u8]) -> u64 {
        // Reuse the confidential gas schedule by wrapping the envelope payload
        // in a VerifyProof instruction; this keeps ZK verify costs aligned with ISI gas.
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new(backend.clone(), payload.to_vec());
        let attachment = ProofAttachment::new_ref(
            backend.clone(),
            proof,
            VerifyingKeyId::new(backend.as_str(), "ivm_gas_meter"),
        );
        let instr = InstructionBox::from(DMZk::VerifyProof::new(attachment));
        crate::gas::meter_instruction(&instr)
    }

    fn queue_instruction(&mut self, instr: InstructionBox) -> u64 {
        let gas = crate::gas::meter_instruction(&instr);
        self.queued.push(QueuedInstruction {
            instruction: instr,
            authority: self.authority.clone(),
            contract_runtime_context: self.current_contract_runtime_context.clone(),
        });
        gas
    }

    fn asset_definition_name_for_syscall(id: &AssetDefinitionId) -> String {
        id.try_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| id.canonical_address())
    }

    #[cfg(test)]
    fn queue_instructions<I>(&mut self, instrs: I) -> u64
    where
        I: IntoIterator<Item = InstructionBox>,
    {
        let mut gas = 0_u64;
        for instr in instrs {
            gas = gas.saturating_add(crate::gas::meter_instruction(&instr));
            self.queued.push(QueuedInstruction {
                instruction: instr,
                authority: self.authority.clone(),
                contract_runtime_context: self.current_contract_runtime_context.clone(),
            });
        }
        gas
    }

    fn enqueue_fastpq_batch(&mut self, entries: Vec<TransferAssetBatchEntry>) {
        self.queued.push(QueuedInstruction {
            instruction: InstructionBox::from(TransferAssetBatch::new(entries)),
            authority: self.authority.clone(),
            contract_runtime_context: self.current_contract_runtime_context.clone(),
        });
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
            return Err(ivm::VMError::metered(
                ivm::gas::G_FASTPQ_BATCH,
                ivm::VMError::PermissionDenied,
            ));
        }
        self.fastpq_batch_entries = Some(Vec::new());
        Ok(ivm::gas::G_FASTPQ_BATCH)
    }

    fn push_fastpq_batch_entry(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        let Some(entries) = self.fastpq_batch_entries.as_mut() else {
            return Err(ivm::VMError::PermissionDenied);
        };
        let from_ptr = vm.register(10);
        let to_ptr = vm.register(11);
        let asset_def_ptr = vm.register(12);
        let amount_ptr = vm.register(13);
        let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
        let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
        let asset_def: AssetDefinitionId =
            Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
        let amount: Numeric = Self::decode_tlv_typed(vm, amount_ptr, PointerType::NoritoBytes)?;
        let asset_id = AssetId::of(asset_def.clone(), from.clone());
        let isi = Transfer::asset_numeric(asset_id, amount.clone(), to.clone());
        let gas = crate::gas::meter_instruction(&InstructionBox::from(TransferBox::from(isi)));
        entries.push(TransferAssetBatchEntry::new(from, to, asset_def, amount));
        Ok(gas)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, ivm::VMError> {
        let Some(entries) = self.fastpq_batch_entries.take() else {
            return Err(ivm::VMError::metered(
                ivm::gas::G_FASTPQ_BATCH,
                ivm::VMError::PermissionDenied,
            ));
        };
        if entries.is_empty() {
            return Err(ivm::VMError::metered(
                ivm::gas::G_FASTPQ_BATCH,
                ivm::VMError::DecodeError,
            ));
        }
        self.enqueue_fastpq_batch(entries);
        Ok(ivm::gas::G_FASTPQ_BATCH)
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
        let instr = InstructionBox::from(batch);
        Ok(self.queue_instruction(instr))
    }

    fn handle_axt_begin(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.clear_axt_reject();
        let ptr = vm.register(10);
        let descriptor_tlv = Self::expect_tlv(vm, ptr, PointerType::AxtDescriptor)?;
        let gas = Self::axt_gas(descriptor_tlv.payload.len());
        let descriptor: axt::AxtDescriptor = Self::decode_header(descriptor_tlv.payload)?;
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
        Ok(gas)
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
        let ds_tlv = Self::expect_tlv(vm, ds_ptr, PointerType::DataSpaceId)?;
        let mut gas_len = ds_tlv.payload.len();
        let dsid: DataSpaceId = Self::decode_header(ds_tlv.payload)?;
        let manifest_ptr = vm.register(11);
        let manifest = if manifest_ptr == 0 {
            TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }
        } else {
            let manifest_tlv = Self::expect_tlv(vm, manifest_ptr, PointerType::NoritoBytes)?;
            gas_len = gas_len.saturating_add(manifest_tlv.payload.len());
            Self::decode_header(manifest_tlv.payload)?
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
        Ok(Self::axt_gas(gas_len))
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
        }
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
            return Ok(Self::axt_verify_gas(0));
        }
        let proof_tlv = Self::expect_tlv(vm, proof_ptr, PointerType::ProofBlob)?;
        let gas = Self::axt_verify_gas(proof_tlv.payload.len());
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
        Ok(gas)
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
        let mut gas_len = handle_tlv.payload.len();
        let handle: AssetHandle = Self::decode_header(handle_tlv.payload)?;
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
        let intent_ptr = vm.register(11);
        let intent_tlv = Self::expect_tlv(vm, intent_ptr, PointerType::NoritoBytes)?;
        gas_len = gas_len.saturating_add(intent_tlv.payload.len());
        let intent: RemoteSpendIntent = Self::decode_header(intent_tlv.payload)?;
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
        if handle.scope.iter().all(|scope| scope != &intent.op.kind) {
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

        let proof_ptr = vm.register(12);
        let proof = if proof_ptr == 0 {
            None
        } else {
            let proof_tlv = Self::expect_tlv(vm, proof_ptr, PointerType::ProofBlob)?;
            gas_len = gas_len.saturating_add(proof_tlv.payload.len());
            let blob: ProofBlob = Self::decode_header(proof_tlv.payload).inspect_err(|_| {
                self.record_axt_reject(
                    AxtRejectReason::Proof,
                    Some(intent.asset_dsid),
                    Some(handle.target_lane),
                    "proof payload failed to decode",
                );
            })?;
            Some(blob)
        };
        let resolved_amount =
            axt::resolve_handle_amount(&intent, proof.as_ref()).map_err(|err| {
                let detail = match err {
                    axt::HandleAmountResolutionError::MissingAmount => {
                        "intent amount is hidden and proof has no committed amount"
                    }
                    axt::HandleAmountResolutionError::Mismatch => {
                        "intent amount does not match proof committed amount"
                    }
                    axt::HandleAmountResolutionError::ZeroAmount => {
                        "handle amount must be non-zero"
                    }
                };
                self.record_axt_reject(
                    AxtRejectReason::Budget,
                    Some(intent.asset_dsid),
                    Some(handle.target_lane),
                    detail,
                );
                err.to_vm_error()
            })?;
        let amount = resolved_amount.amount;

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
            amount_commitment: resolved_amount.amount_commitment,
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
        Ok(Self::axt_gas(gas_len))
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
        let gas = Self::axt_commit_gas(&state);
        if let Some(analysis) = &self.amx_analysis {
            if let Some(ds_count) = NonZeroUsize::new(state.expected_dsids().len()) {
                if let Err(err) = analysis::enforce_amx_budget(analysis, ds_count, &self.amx_limits)
                {
                    let dataspace = state
                        .expected_dsids()
                        .first()
                        .copied()
                        .unwrap_or(DataSpaceId::UNIVERSAL);
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
                        .unwrap_or(DataSpaceId::UNIVERSAL);
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
                Ok(gas)
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
    pub fn with_host<R>(vm: &mut IVM, f: impl FnOnce(&mut CoreHost) -> R) -> R {
        let host_any = vm.host_mut_any().expect("CoreHost not attached to VM");
        let host = host_any
            .downcast_mut::<CoreHost>()
            .expect("host type mismatch: expected CoreHost");
        f(host)
    }
}

impl<QRef> CoreHostImpl<QueryStateSlot<QRef>>
where
    QRef: Copy + QueryStateExecute,
{
    /// Attach a read-only state view for query execution during this VM run.
    pub fn set_query_state<'block, S>(&mut self, state: &'block S)
    where
        S: QueryStateSource<Ref<'block> = QRef>,
    {
        self.query_state.state = Some(state.as_query_state_ref());
        if let Some(block_height) = state.block_height_hint() {
            self.current_block_height = Some(block_height);
        }
    }
}

impl<QS> CoreHostImpl<QS> {
    fn resolve_account_alias<R: StateReadOnly>(
        state: &R,
        authority: &AccountId,
        alias: &str,
    ) -> Result<AccountId, ivm::VMError> {
        let alias_label = AccountAlias::from_literal(alias, &state.nexus().dataspace_catalog)
            .map_err(|_| ivm::VMError::DecodeError)?;
        if !crate::alias::authority_can_resolve_account_alias(
            state.world(),
            authority,
            &alias_label,
        ) {
            return Err(ivm::VMError::PermissionDenied);
        }
        if let Some(account_id) = state.world().account_aliases().get(&alias_label).cloned() {
            return Ok(account_id);
        }
        Err(ivm::VMError::DecodeError)
    }

    /// Set the trigger identifier for the current execution.
    pub(crate) fn set_trigger_id(&mut self, id: TriggerId) {
        self.current_trigger_id = Some(id);
    }

    /// Set the current block creation timestamp (UTC ms).
    pub(crate) fn set_block_time_ms(&mut self, time_ms: u64) {
        self.current_block_time_ms = Some(time_ms);
    }

    /// Set the current block height for sysvar exposure.
    pub(crate) fn set_block_height(&mut self, block_height: u64) {
        self.current_block_height = Some(block_height);
    }

    fn take_json_field<T: json::JsonDeserialize>(
        map: &mut BTreeMap<String, json::Value>,
        key: &str,
    ) -> Result<T, ivm::VMError> {
        let value = map.remove(key).ok_or(ivm::VMError::DecodeError)?;
        json::from_value(value).map_err(|_| ivm::VMError::DecodeError)
    }

    fn decode_trigger_filter(value: json::Value) -> Result<EventFilterBox, ivm::VMError> {
        use iroha_data_model::events::{
            data::DataEventFilter, execute_trigger::ExecuteTriggerEventFilter,
            pipeline::PipelineEventFilterBox, time::TimeEventFilter,
            trigger_completed::TriggerCompletedEventFilter,
        };

        if let Ok(filter) = json::from_value::<EventFilterBox>(value.clone()) {
            return Ok(filter);
        }
        if let Ok(filter) = json::from_value::<DataEventFilter>(value.clone()) {
            return Ok(EventFilterBox::Data(filter));
        }
        if let Ok(filter) = json::from_value::<PipelineEventFilterBox>(value.clone()) {
            return Ok(EventFilterBox::Pipeline(filter));
        }
        if let Ok(filter) = json::from_value::<TimeEventFilter>(value.clone()) {
            return Ok(EventFilterBox::Time(filter));
        }
        if let Ok(filter) = json::from_value::<ExecuteTriggerEventFilter>(value.clone()) {
            return Ok(EventFilterBox::ExecuteTrigger(filter));
        }
        if let Ok(filter) = json::from_value::<TriggerCompletedEventFilter>(value) {
            return Ok(EventFilterBox::TriggerCompleted(filter));
        }
        Err(ivm::VMError::DecodeError)
    }

    fn decode_trigger_action_spec(value: json::Value) -> Result<Action, ivm::VMError> {
        if let Ok(action) = json::from_value::<Action>(value.clone()) {
            return Ok(action);
        }

        let mut map = match value {
            json::Value::Object(map) => map,
            _ => return Err(ivm::VMError::DecodeError),
        };

        let executable: Executable = Self::take_json_field(&mut map, "executable")?;
        let repeats: Repeats = Self::take_json_field(&mut map, "repeats")?;
        let authority: AccountId = Self::take_json_field(&mut map, "authority")?;
        let filter_value = map.remove("filter").ok_or(ivm::VMError::DecodeError)?;
        let metadata = match map.remove("metadata") {
            Some(value) => json::from_value(value).map_err(|_| ivm::VMError::DecodeError)?,
            None => Metadata::default(),
        };

        let filter = Self::decode_trigger_filter(filter_value)?;
        if matches!(filter, EventFilterBox::TriggerCompleted(_)) {
            return Err(ivm::VMError::DecodeError);
        }

        Ok(Action::new(executable, repeats, authority, filter).with_metadata(metadata))
    }

    fn amount_from_trigger_args(&self) -> Option<Numeric> {
        fn parse_numeric(value: &json::Value) -> Option<Numeric> {
            match value {
                json::Value::String(raw) => raw.parse::<Numeric>().ok(),
                json::Value::Number(number) => match number {
                    json::native::Number::I64(v) => Some(Numeric::from(*v)),
                    json::native::Number::U64(v) => Some(Numeric::from(*v)),
                    json::native::Number::F64(v) => Numeric::try_from(*v).ok(),
                },
                _ => None,
            }
        }
        let args = self.args.as_ref()?;
        let value: json::Value = args.try_into_any_norito().ok()?;
        match value {
            json::Value::Object(map) => map.get("val").and_then(parse_numeric),
            other => parse_numeric(&other),
        }
    }

    fn escrow_id_from_name(name: &Name) -> EscrowId {
        EscrowId::from_kotodama_name(name)
    }
}

impl<QS: QueryStateAccess + Default> IVMHost for CoreHostImpl<QS> {
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
                let instr = InstructionBox::from(RegisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_UNREGISTER_DOMAIN => {
                let ptr = vm.register(10);
                let id: DomainId = Self::decode_tlv_typed(vm, ptr, PointerType::DomainId)?;
                let isi = Unregister::domain(id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_TRANSFER_DOMAIN => {
                // r10=&DomainId, r11=&AccountId(to)
                let dptr = vm.register(10);
                let tptr = vm.register(11);
                let id: DomainId = Self::decode_tlv_typed(vm, dptr, PointerType::DomainId)?;
                let to: AccountId = Self::decode_tlv_typed(vm, tptr, PointerType::AccountId)?;
                let from = self.authority.clone();
                let isi = Transfer::domain(from, id, to);
                let instr = InstructionBox::from(TransferBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            // ----------------- Peer ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_REGISTER_PEER => {
                let ptr = vm.register(10);
                let payload: Json = Self::decode_tlv_json(vm, ptr)?;
                let request = Self::register_peer_from_json(&payload)?;
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_UNREGISTER_PEER => {
                let ptr = vm.register(10);
                let payload: Json = Self::decode_tlv_json(vm, ptr)?;
                let value: json::Value = payload
                    .try_into_any_norito::<json::Value>()
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let peer_id = Self::peer_id_from_value(&value)?;
                let isi = Unregister::peer(peer_id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            // ----------------- Account ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_REGISTER_ACCOUNT => {
                let ptr = vm.register(10);
                let id: AccountId = Self::decode_tlv_typed(vm, ptr, PointerType::AccountId)?;
                let isi = Register::account(Account::new(id));
                let instr = InstructionBox::from(RegisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_UNREGISTER_ACCOUNT => {
                let ptr = vm.register(10);
                let id: AccountId = Self::decode_tlv_typed(vm, ptr, PointerType::AccountId)?;
                let isi = Unregister::account(id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_ADD_SIGNATORY => {
                let account_ptr = vm.register(10);
                let signatory_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let payload: Json = Self::decode_tlv_json(vm, signatory_ptr)?;
                let signatory = Self::public_key_from_json(&payload)?;
                let isi = AddSignatory::new(account, signatory);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REMOVE_SIGNATORY => {
                let account_ptr = vm.register(10);
                let signatory_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let payload: Json = Self::decode_tlv_json(vm, signatory_ptr)?;
                let signatory = Self::public_key_from_json(&payload)?;
                let isi = RemoveSignatory::new(account, signatory);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_SET_ACCOUNT_QUORUM => {
                let account_ptr = vm.register(10);
                let quorum_raw = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let quorum_u16 =
                    u16::try_from(quorum_raw).map_err(|_| ivm::VMError::DecodeError)?;
                let quorum = NonZeroU16::new(quorum_u16).ok_or(ivm::VMError::DecodeError)?;
                let isi = SetAccountQuorum::new(account, quorum);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            // ----------------- Asset (Numeric) ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_REGISTER_ASSET => {
                let ptr = vm.register(10);
                let id: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, ptr, PointerType::AssetDefinitionId)?;
                let name = Self::asset_definition_name_for_syscall(&id);
                let isi = Register::asset_definition({
                    let __asset_definition_id = id;
                    AssetDefinition::numeric(__asset_definition_id.clone()).with_name(name)
                });
                let instr = InstructionBox::from(RegisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_UNREGISTER_ASSET => {
                let ptr = vm.register(10);
                let id: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, ptr, PointerType::AssetDefinitionId)?;
                let isi = Unregister::asset_definition(id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_MINT_ASSET => {
                let account_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let amount_ptr = vm.register(12);

                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let amount = if amount_ptr == 0 {
                    self.amount_from_trigger_args()
                        .ok_or(ivm::VMError::DecodeError)?
                } else {
                    Self::decode_tlv_typed(vm, amount_ptr, PointerType::NoritoBytes)?
                };
                let asset_id = AssetId::of(asset_def, account);
                let isi = Mint::asset_numeric(amount, asset_id);
                let instr = InstructionBox::from(MintBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_BURN_ASSET => {
                let account_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let amount_ptr = vm.register(12);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let amount: Numeric =
                    Self::decode_tlv_typed(vm, amount_ptr, PointerType::NoritoBytes)?;
                let asset_id = AssetId::of(asset_def, account);
                let isi = Burn::asset_numeric(amount, asset_id);
                let instr = InstructionBox::from(BurnBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_TRANSFER_ASSET => {
                if self.fastpq_batch_entries.is_some() {
                    return self.push_fastpq_batch_entry(vm);
                }
                let from_ptr = vm.register(10);
                let to_ptr = vm.register(11);
                let asset_def_ptr = vm.register(12);
                let amount_ptr = vm.register(13);
                let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
                let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let amount: Numeric =
                    Self::decode_tlv_typed(vm, amount_ptr, PointerType::NoritoBytes)?;
                let asset_id = AssetId::of(asset_def, from);
                let isi = Transfer::asset_numeric(asset_id, amount, to);
                let instr = InstructionBox::from(TransferBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch_from_tlv(vm),
            // ----------------- Native anonymous asset escrow ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER => {
                let request: OpenAnonymousAssetEscrow =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::NoritoBytes)?;
                Ok(self.queue_instruction(InstructionBox::from(request)))
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_ACCEPT => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(
                    self.queue_instruction(InstructionBox::from(AcceptAnonymousAssetEscrow {
                        escrow_id,
                    })),
                )
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(
                    self.queue_instruction(InstructionBox::from(MarkAnonymousEscrowPaymentSent {
                        escrow_id,
                    })),
                )
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_RELEASE => {
                let request: ReleaseAnonymousAssetEscrow =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::NoritoBytes)?;
                Ok(self.queue_instruction(InstructionBox::from(request)))
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_CANCEL => {
                let request: CancelAnonymousAssetEscrow =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::NoritoBytes)?;
                Ok(self.queue_instruction(InstructionBox::from(request)))
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                let evidence_hashes = Self::decode_optional_evidence_hashes(vm, vm.register(11))?;
                Ok(
                    self.queue_instruction(InstructionBox::from(OpenAnonymousEscrowDispute {
                        escrow_id,
                        evidence_hashes,
                    })),
                )
            }
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE => {
                let request: ResolveAnonymousEscrowDispute =
                    Self::decode_tlv_typed(vm, vm.register(10), PointerType::NoritoBytes)?;
                Ok(self.queue_instruction(InstructionBox::from(request)))
            }
            // ----------------- Native asset escrow ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_ESCROW_OPEN_OFFER => {
                let escrow_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let amount_ptr = vm.register(12);
                let evidence_ptr = vm.register(13);
                let escrow_id = Self::decode_escrow_id(vm, escrow_ptr)?;
                let asset_definition: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let amount: Numeric =
                    Self::decode_tlv_typed(vm, amount_ptr, PointerType::NoritoBytes)?;
                let evidence_hashes = Self::decode_optional_evidence_hashes(vm, evidence_ptr)?;
                let instr = InstructionBox::from(OpenAssetEscrow {
                    escrow_id,
                    asset_definition,
                    amount,
                    evidence_hashes,
                });
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_ESCROW_ACCEPT => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(self.queue_instruction(InstructionBox::from(AcceptAssetEscrow { escrow_id })))
            }
            ivm::syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(self
                    .queue_instruction(InstructionBox::from(MarkEscrowPaymentSent { escrow_id })))
            }
            ivm::syscalls::SYSCALL_ESCROW_RELEASE => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(self.queue_instruction(InstructionBox::from(ReleaseAssetEscrow { escrow_id })))
            }
            ivm::syscalls::SYSCALL_ESCROW_CANCEL => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                Ok(self.queue_instruction(InstructionBox::from(CancelAssetEscrow { escrow_id })))
            }
            ivm::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE => {
                let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                let evidence_hashes = Self::decode_optional_evidence_hashes(vm, vm.register(11))?;
                Ok(
                    self.queue_instruction(InstructionBox::from(OpenEscrowDispute {
                        escrow_id,
                        evidence_hashes,
                    })),
                )
            }
            ivm::syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE => {
                let escrow_ptr = vm.register(10);
                let buyer_amount_ptr = vm.register(11);
                let seller_amount_ptr = vm.register(12);
                let evidence_ptr = vm.register(13);
                let escrow_id = Self::decode_escrow_id(vm, escrow_ptr)?;
                let buyer_amount: Numeric =
                    Self::decode_tlv_typed(vm, buyer_amount_ptr, PointerType::NoritoBytes)?;
                let seller_amount: Numeric =
                    Self::decode_tlv_typed(vm, seller_amount_ptr, PointerType::NoritoBytes)?;
                let evidence_hashes = Self::decode_optional_evidence_hashes(vm, evidence_ptr)?;
                Ok(
                    self.queue_instruction(InstructionBox::from(ResolveEscrowDispute {
                        escrow_id,
                        buyer_amount,
                        seller_amount,
                        evidence_hashes,
                    })),
                )
            }
            // Account metadata (SetKeyValue<Account>)
            ivm::syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                let account_ptr = vm.register(10);
                let key_ptr = vm.register(11);
                let value_ptr = vm.register(12);

                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let key: Name = Self::decode_tlv_typed(vm, key_ptr, PointerType::Name)?;
                let value: Json = Self::decode_tlv_json(vm, value_ptr)?;

                let isi = SetKeyValue::account(account, key, value);
                let instr = InstructionBox::from(SetKeyValueBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            // ----------------- Role and permission ISIs via pointer-ABI -----------------
            ivm::syscalls::SYSCALL_CREATE_ROLE => {
                let name_ptr = vm.register(10);
                let perms_ptr = vm.register(11);
                let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                let perms_json: Json = Self::decode_tlv_json(vm, perms_ptr)?;
                let permissions = Self::permissions_from_json(&perms_json)?;
                let role_id = RoleId::new(name);
                let mut role = Role::new(role_id, self.authority.clone());
                for perm in permissions {
                    role = role.add_permission(perm);
                }
                let isi = Register::role(role);
                let instr = InstructionBox::from(RegisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_DELETE_ROLE => {
                let name_ptr = vm.register(10);
                let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                let isi = Unregister::role(RoleId::new(name));
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_GRANT_ROLE => {
                let account_ptr = vm.register(10);
                let name_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                let isi = Grant::account_role(RoleId::new(name), account);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REVOKE_ROLE => {
                let account_ptr = vm.register(10);
                let name_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                let isi = Revoke::account_role(RoleId::new(name), account);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_GRANT_PERMISSION => {
                let account_ptr = vm.register(10);
                let perm_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let permission = Self::decode_permission(vm, perm_ptr)?;
                let isi = Grant::account_permission(permission, account);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REVOKE_PERMISSION => {
                let account_ptr = vm.register(10);
                let perm_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let permission = Self::decode_permission(vm, perm_ptr)?;
                let isi = Revoke::account_permission(permission, account);
                let instr = InstructionBox::from(isi);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_CREATE_TRIGGER => {
                let ptr = vm.register(10);
                let spec: Json = Self::decode_tlv_json(vm, ptr)?;
                let trigger = if let Ok(trigger) = spec.try_into_any_norito::<Trigger>() {
                    trigger
                } else {
                    let value: json::Value = spec
                        .try_into_any_norito::<json::Value>()
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    let mut map = match value {
                        json::Value::Object(map) => map,
                        _ => return Err(ivm::VMError::DecodeError),
                    };
                    let id_value = map.remove("id").ok_or(ivm::VMError::DecodeError)?;
                    let id_str = id_value.as_str().ok_or(ivm::VMError::DecodeError)?;
                    let id: TriggerId = id_str.parse().map_err(|_| ivm::VMError::DecodeError)?;
                    let action_value = map.remove("action").ok_or(ivm::VMError::DecodeError)?;
                    let mut action = Self::decode_trigger_action_spec(action_value)?;
                    if action.authority.subject_id() == self.authority.subject_id() {
                        action.authority = self.authority.clone();
                    }
                    Trigger::new(id, action)
                };
                let instr = InstructionBox::from(Register::trigger(trigger));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REMOVE_TRIGGER => {
                let ptr = vm.register(10);
                let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
                let id = TriggerId::new(name);
                let isi = Unregister::trigger(id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_SET_TRIGGER_ENABLED => {
                let ptr = vm.register(10);
                let enabled = vm.register(11) != 0;
                let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
                let id = TriggerId::new(name);
                let key: Name = TRIGGER_ENABLED_METADATA_KEY
                    .parse()
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let isi = SetKeyValue::trigger(id, key, Json::from(enabled));
                let instr = InstructionBox::from(SetKeyValueBox::from(isi));
                Ok(self.queue_instruction(instr))
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
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES => {
                let ptr = vm.register(10);
                let request: scode::RegisterSmartContractBytes =
                    Self::decode_tlv_typed_any_region(vm, ptr, PointerType::NoritoBytes)?;
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE => {
                let ptr = vm.register(10);
                let request: scode::ActivateContractInstance =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE => {
                let ptr = vm.register(10);
                let request: scode::DeactivateContractInstance =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES => {
                let ptr = vm.register(10);
                let request: scode::RemoveSmartContractBytes =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let instr = InstructionBox::from(request);
                Ok(self.queue_instruction(instr))
            }
            // ----------------- NFT (Non-fungible) ISIs -----------------
            ivm::syscalls::SYSCALL_NFT_MINT_ASSET => {
                let nft_id_ptr = vm.register(10);
                let owner_ptr = vm.register(11);
                let owner: AccountId =
                    Self::decode_tlv_typed(vm, owner_ptr, PointerType::AccountId)?;
                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let nft = Nft::new(nft_id.clone(), Metadata::default());
                let mut gas = self
                    .queue_instruction(InstructionBox::from(RegisterBox::from(Register::nft(nft))));
                if owner != self.authority {
                    let transfer = InstructionBox::from(TransferBox::from(Transfer::nft(
                        self.authority.clone(),
                        nft_id,
                        owner,
                    )));
                    gas = gas.saturating_add(self.queue_instruction(transfer));
                }
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                let from_ptr = vm.register(10);
                let nft_id_ptr = vm.register(11);
                let to_ptr = vm.register(12);
                let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
                let isi = Transfer::nft(from, nft_id, to);
                let instr = InstructionBox::from(TransferBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_NFT_SET_METADATA => {
                let nft_id_ptr = vm.register(10);
                let key_ptr = vm.register(11);
                let value_ptr = vm.register(12);

                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let key: Name = Self::decode_tlv_typed(vm, key_ptr, PointerType::Name)?;
                let value: Json = Self::decode_tlv_json(vm, value_ptr)?;

                let isi = SetKeyValue::nft(nft_id, key, value);
                let instr = InstructionBox::from(SetKeyValueBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            ivm::syscalls::SYSCALL_NFT_BURN_ASSET => {
                let nft_id_ptr = vm.register(10);
                let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                let isi = Unregister::nft(nft_id);
                let instr = InstructionBox::from(UnregisterBox::from(isi));
                Ok(self.queue_instruction(instr))
            }
            // Sample convenience: create one NFT per known account (from snapshot)
            ivm::syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS => {
                let mut gas = 0_u64;
                let mut i = self.nft_seq;
                let start = i;
                let accounts = Arc::clone(&self.accounts_snapshot);
                let authority = self.authority.clone();
                for account_id in accounts.iter() {
                    let name_str = format!("nft_number_{}_for_{}", i, account_id.signatory());
                    if let Ok(name) = name_str.parse() {
                        let nft_id = NftId::of(iroha_genesis::GENESIS_DOMAIN_ID.clone(), name);
                        let nft = Nft::new(nft_id.clone(), Metadata::default());
                        let reg = InstructionBox::from(RegisterBox::from(Register::nft(nft)));
                        gas = gas.saturating_add(self.queue_instruction(reg));
                        let xfer = InstructionBox::from(TransferBox::from(Transfer::nft(
                            authority.clone(),
                            nft_id,
                            account_id.clone(),
                        )));
                        gas = gas.saturating_add(self.queue_instruction(xfer));
                        i = i.saturating_add(1);
                        if i.saturating_sub(start) >= 256 {
                            break;
                        }
                    }
                }
                self.nft_seq = i;
                Ok(gas.max(Self::create_nfts_for_all_gas()))
            }
            // Set SmartContract execution depth parameter to x10
            ivm::syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH => {
                let depth_raw = vm.register(10);
                // If zero, treat as no-op to avoid setting an invalid value
                if depth_raw == 0 {
                    return Ok(Self::smartcontract_depth_gas());
                }
                // SmartContractParameter::ExecutionDepth expects a u8.
                let depth: u8 = u8::try_from(depth_raw).unwrap_or(u8::MAX);
                let param = Parameter::SmartContract(
                    iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(depth),
                );
                let instr = InstructionBox::from(SetParameter::new(param));
                Ok(self
                    .queue_instruction(instr)
                    .max(Self::smartcontract_depth_gas()))
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
                    let instr = InstructionBox::from(new);
                    Ok(self.queue_instruction(instr))
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
                        outputs: u.outputs.clone(),
                        proof: pa,
                        root_hint: u.root_hint,
                    };
                    let instr = InstructionBox::from(new);
                    Ok(self.queue_instruction(instr))
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
                    let instr = InstructionBox::from(new);
                    Ok(self.queue_instruction(instr))
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
                    let instr = InstructionBox::from(new);
                    Ok(self.queue_instruction(instr))
                } else if any_ref.downcast_ref::<DMZk::VerifyProof>().is_some() {
                    // Allow explicit VerifyProof if issued via vendor bridge; pass-through
                    Ok(self.queue_instruction(ib))
                } else {
                    Err(ivm::VMError::PermissionDenied)
                }
            }
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY
            | ivm::syscalls::SYSCALL_QUERY_EXECUTE_NORITO => {
                let ptr = vm.register(10);
                let request: QueryRequest =
                    Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                let gas_remaining = vm.remaining_gas();
                let gas_ctx = QueryGasContext::from_request(&request);
                let Some(state_ref) = self.query_state.get() else {
                    return Err(ivm::VMError::NotImplemented { syscall: number });
                };
                let budget_items = Self::query_items_budget(&gas_ctx, gas_remaining)?;
                if matches!(request, QueryRequest::Singular(_)) && budget_items == 0 {
                    return Err(ivm::VMError::OutOfGas);
                }
                let query_result = state_ref.execute_query_with_budget(
                    &self.authority,
                    request,
                    Some(budget_items),
                )?;
                let response = query_result.response;
                let processed_items = query_result.processed_items;
                if let Some(encoded_len) = Self::norito_encoded_len_exact(&response) {
                    let gas = Self::query_gas_cost(&gas_ctx, processed_items, encoded_len);
                    if gas > gas_remaining {
                        return Err(ivm::VMError::OutOfGas);
                    }
                }
                let response_bytes =
                    norito::to_bytes(&response).map_err(|_| ivm::VMError::DecodeError)?;
                let response_len_u64 = u64::try_from(response_bytes.len()).unwrap_or(u64::MAX);
                let gas = Self::query_gas_cost(&gas_ctx, processed_items, response_len_u64);
                if gas > gas_remaining {
                    return Err(ivm::VMError::OutOfGas);
                }
                let payload_len = Self::len_to_u32(response_bytes.len())?;
                let mut out = Vec::with_capacity(7 + response_bytes.len() + Hash::LENGTH);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&payload_len.to_be_bytes());
                out.extend_from_slice(&response_bytes);
                let h: [u8; Hash::LENGTH] = Hash::new(&response_bytes).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL => self.subscription_bill(),
            ivm::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE => self.subscription_record_usage(),
            ivm::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS => {
                let gas_remaining = vm.remaining_gas();
                let alias_ptr = vm.register(10);
                let alias_bytes = Self::decode_tlv_blob(vm, alias_ptr)?;
                let alias_literal =
                    String::from_utf8(alias_bytes).map_err(|_| ivm::VMError::DecodeError)?;
                let gas_ctx = QueryGasContext::singular();
                let Some(state_ref) = self.query_state.get() else {
                    return Err(ivm::VMError::NotImplemented { syscall: number });
                };
                if Self::query_items_budget(&gas_ctx, gas_remaining)? == 0 {
                    return Err(ivm::VMError::OutOfGas);
                }
                let account_id =
                    state_ref.resolve_account_alias(&self.authority, &alias_literal)?;
                let payload =
                    norito::to_bytes(&account_id).map_err(|_| ivm::VMError::NoritoInvalid)?;
                let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
                let gas = Self::query_gas_cost(&gas_ctx, 1, payload_len_u64);
                if gas > gas_remaining {
                    return Err(ivm::VMError::OutOfGas);
                }
                let payload_len = Self::len_to_u32(payload.len())?;
                let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
                out.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&payload_len.to_be_bytes());
                out.extend_from_slice(&payload);
                let h: [u8; Hash::LENGTH] = Hash::new(&payload).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_GET_ACCOUNT_BALANCE => {
                let account_ptr = vm.register(10);
                let asset_def_ptr = vm.register(11);
                let account: AccountId =
                    Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                let asset_def: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                let asset_id = AssetId::of(asset_def, account);
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindAssetById(FindAssetById {
                        id: asset_id,
                    }));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::Asset(asset) => {
                        Self::encode_norito_payload(asset.value())
                    }
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_GET_PUBLIC_INPUT => {
                let name_ptr = vm.register(10);
                let tlv = Self::expect_tlv(vm, name_ptr, PointerType::Name)?;
                let name = Self::decode_name_payload(tlv.payload)?;
                if name.as_ref() == TRIGGER_EVENT_PUBLIC_INPUT_KEY {
                    let Some(args) = self.args.as_ref() else {
                        return Err(ivm::VMError::PermissionDenied);
                    };
                    if !is_type_allowed_for_policy(vm.syscall_policy(), PointerType::Json) {
                        return Err(ivm::VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: PointerType::Json as u16,
                        });
                    }
                    let payload =
                        norito::to_bytes(args).map_err(|_| ivm::VMError::NoritoInvalid)?;
                    let payload_len = Self::len_to_u32(payload.len())?;
                    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&payload_len.to_be_bytes());
                    out.extend_from_slice(&payload);
                    let h: [u8; Hash::LENGTH] = Hash::new(&payload).into();
                    out.extend_from_slice(&h);
                    let ptr = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, ptr);
                    let bytes = u64::try_from(out.len()).unwrap_or(u64::MAX);
                    return Ok(PUBLIC_INPUT_GAS_BASE_DEFAULT
                        .saturating_add(PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT.saturating_mul(bytes)));
                }
                let Some(record) = self.public_inputs.get(&name) else {
                    return Err(ivm::VMError::PermissionDenied);
                };
                if !is_type_allowed_for_policy(vm.syscall_policy(), record.type_id) {
                    return Err(ivm::VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: record.type_id as u16,
                    });
                }
                let ptr = vm.alloc_input_tlv(&record.tlv)?;
                vm.set_register(10, ptr);
                Ok(record.gas_cost())
            }
            // ZK verify syscalls are handled here so CoreHost can enforce VK binding
            // and run full backend verification before arming ISI latches.
            ivm::syscalls::SYSCALL_ZK_VERIFY_TRANSFER => {
                // Capture TLV payload hash (envelope hash) and verify the bound proof.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                let ok = match self.verify_bound_envelope(tlv.payload, "transfer") {
                    Ok(ok) => ok,
                    Err(code) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, code);
                        return Ok(gas);
                    }
                };
                vm.set_register(10, u64::from(ok));
                vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                if ok {
                    self.zk_verified_transfer.push_back(env_hash);
                    self.zk_last_env_hash_transfer.push_back(env_hash);
                }
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                let ok = match self.verify_bound_envelope(tlv.payload, "unshield") {
                    Ok(ok) => ok,
                    Err(code) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, code);
                        return Ok(gas);
                    }
                };
                vm.set_register(10, u64::from(ok));
                vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                if ok {
                    self.zk_verified_unshield.push_back(env_hash);
                    self.zk_last_env_hash_unshield.push_back(env_hash);
                }
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                let ok = match self.verify_bound_envelope(tlv.payload, "ballot") {
                    Ok(ok) => ok,
                    Err(code) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, code);
                        return Ok(gas);
                    }
                };
                vm.set_register(10, u64::from(ok));
                vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                if ok {
                    self.zk_verified_ballot.push_back(env_hash);
                    self.zk_last_env_hash_ballot.push_back(env_hash);
                }
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                let ok = match self.verify_bound_envelope(tlv.payload, "tally") {
                    Ok(ok) => ok,
                    Err(code) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, code);
                        return Ok(gas);
                    }
                };
                vm.set_register(10, u64::from(ok));
                vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                if ok {
                    self.zk_verified_tally.push_back(env_hash);
                    self.zk_last_env_hash_tally.push_back(env_hash);
                }
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_VERIFY_PROOF => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let ok = match self.verify_any_namespace_envelope(tlv.payload) {
                    Ok(ok) => ok,
                    Err(code) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, code);
                        return Ok(gas);
                    }
                };
                vm.set_register(10, u64::from(ok));
                vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                Ok(gas)
            }
            ivm::syscalls::SYSCALL_ZK_VERIFY_BATCH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                let gas = Self::gas_for_zk_verify_payload(tlv.payload);
                let envs: Vec<iroha_data_model::zk::OpenVerifyEnvelope> =
                    if let Ok(v) = norito::decode_from_bytes(tlv.payload) {
                        v
                    } else {
                        vm.set_register(10, 0);
                        vm.set_register(11, ivm::host::ERR_DECODE);
                        return Ok(gas);
                    };
                if u32::try_from(envs.len()).unwrap_or(u32::MAX)
                    > self.halo2_config.verifier_max_batch
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, ivm::host::ERR_BATCH);
                    return Ok(gas);
                }
                let guardrails = self.zk_verify_guardrails();

                let mut statuses: Vec<u8> = Vec::with_capacity(envs.len());
                let mut first_error: Option<u64> = None;
                for env in &envs {
                    let mut status = 0u8;
                    let payload = if let Ok(bytes) = norito::to_bytes(env) {
                        bytes
                    } else {
                        first_error.get_or_insert(ivm::host::ERR_DECODE);
                        statuses.push(status);
                        continue;
                    };
                    let (_env, vk_box, backend_label) =
                        match self.enforce_zk_envelope_any_namespace(&payload) {
                            Ok(v) => v,
                            Err(code) => {
                                first_error.get_or_insert(code);
                                statuses.push(status);
                                continue;
                            }
                        };
                    let proof = ProofBox::new(backend_label.clone().into(), payload);
                    let report = crate::zk::verify_backend_with_timing_guardrails(
                        &backend_label,
                        &proof,
                        Some(&vk_box),
                        guardrails,
                    );
                    status = u8::from(report.ok);
                    if !report.ok {
                        first_error.get_or_insert(ivm::host::ERR_VERIFY);
                    }
                    statuses.push(status);
                }

                if statuses.is_empty() {
                    vm.set_register(10, 0);
                    vm.set_register(11, ivm::host::ERR_DECODE);
                    return Ok(gas);
                }

                let body = norito::to_bytes(&statuses).map_err(|_| ivm::VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                if let Some((idx, _)) = statuses.iter().enumerate().find(|(_, s)| **s == 0) {
                    vm.set_register(12, idx as u64);
                } else {
                    vm.set_register(12, u64::MAX);
                }
                if let Some(code) = first_error {
                    vm.set_register(11, code);
                } else {
                    vm.set_register(11, 0);
                }
                Ok(gas)
            }
            // ZK roots read: build response from snapshot and return TLV pointer in r10
            ivm::syscalls::SYSCALL_ZK_ROOTS_GET => {
                let ptr = vm.register(10);
                let req_tlv = Self::expect_tlv(vm, ptr, PointerType::NoritoBytes)?;
                let input_len = req_tlv.payload.len();
                let req: ivm::zk_verify::RootsGetRequest =
                    norito::decode_from_bytes(req_tlv.payload)
                        .map_err(|_| ivm::VMError::NoritoInvalid)?;
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
                Ok(Self::state_query_gas(input_len.saturating_add(body.len())))
            }
            // ZK vote tally read: respond from elections snapshot
            ivm::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                let ptr = vm.register(10);
                let req_tlv = Self::expect_tlv(vm, ptr, PointerType::NoritoBytes)?;
                let input_len = req_tlv.payload.len();
                let req: ivm::zk_verify::VoteGetTallyRequest =
                    norito::decode_from_bytes(req_tlv.payload)
                        .map_err(|_| ivm::VMError::NoritoInvalid)?;
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
                Ok(Self::state_query_gas(input_len.saturating_add(body.len())))
            }
            ivm::syscalls::SYSCALL_VRF_EPOCH_SEED => {
                use ivm::vrf::{VrfEpochSeedRequest, VrfEpochSeedResponse};

                const OK: u64 = 0;
                const ERR_TYPE: u64 = 1;
                const ERR_DECODE: u64 = 2;
                const ERR_OOM: u64 = 3;

                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                let input_len = tlv.payload.len();
                let gas = Self::state_query_gas(input_len);
                if tlv.type_id != PointerType::NoritoBytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_TYPE);
                    return Ok(gas);
                }
                let req: VrfEpochSeedRequest = match norito::decode_from_bytes(tlv.payload) {
                    Ok(req) => req,
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        return Ok(gas);
                    }
                };

                let resolved = self
                    .vrf_epoch_seeds
                    .get(&req.epoch)
                    .map(|seed| (req.epoch, *seed))
                    .or_else(|| {
                        if req.fallback_to_latest {
                            self.vrf_epoch_seeds
                                .iter()
                                .next_back()
                                .map(|(epoch, seed)| (*epoch, *seed))
                        } else {
                            None
                        }
                    });
                let (found, epoch, seed) = match resolved {
                    Some((epoch, seed)) => (true, epoch, seed),
                    None => (false, req.epoch, [0u8; 32]),
                };

                let resp = VrfEpochSeedResponse { found, epoch, seed };
                let body = norito::to_bytes(&resp).map_err(|_| ivm::VMError::NoritoInvalid)?;
                let body_len = Self::len_to_u32(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&body_len.to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                match vm.alloc_input_tlv(&out) {
                    Ok(p) => {
                        vm.set_register(10, p);
                        vm.set_register(11, OK);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_OOM);
                    }
                }
                Ok(Self::state_query_gas(input_len.saturating_add(body.len())))
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
                if let Some(scoped_path) = self.scoped_durable_state_path(&path)? {
                    if let Some(entry) = self.durable_state_overlay.get(&scoped_path) {
                        match entry {
                            Some(stored) => {
                                let len = Self::durable_state_value_payload_len(stored)?;
                                Self::load_state_value(vm, stored)?;
                                return Ok(Self::state_query_gas(len));
                            }
                            None => {
                                vm.set_register(10, 0);
                                return Ok(16);
                            }
                        }
                    }
                    if let Some(stored) = self.durable_state_base_or_live_get(&scoped_path) {
                        let len = Self::durable_state_value_payload_len(stored.as_ref())?;
                        Self::load_state_value(vm, stored.as_ref())?;
                        return Ok(Self::state_query_gas(len));
                    }
                }
                if let Some(entry) = self.durable_state_overlay.get(&path) {
                    match entry {
                        Some(stored) => {
                            let len = Self::durable_state_value_payload_len(stored)?;
                            Self::load_state_value(vm, stored)?;
                            return Ok(Self::state_query_gas(len));
                        }
                        None => {
                            vm.set_register(10, 0);
                            return Ok(16);
                        }
                    }
                }
                if let Some(stored) = self.durable_state_base_or_live_get(&path) {
                    let len = Self::durable_state_value_payload_len(stored.as_ref())?;
                    Self::load_state_value(vm, stored.as_ref())?;
                    Ok(Self::state_query_gas(len))
                } else {
                    vm.set_register(10, 0);
                    Ok(16)
                }
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
                let key = self
                    .scoped_durable_state_path(&path)?
                    .unwrap_or_else(|| path.clone());
                self.durable_state_overlay.insert(key, Some(stored));
                Ok(Self::state_query_gas(val_tlv.payload.len()))
            }
            ivm::syscalls::SYSCALL_STATE_DEL => {
                let path_ptr = vm.register(10);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_write_key(path.as_ref());
                if let Some(scoped_path) = self.scoped_durable_state_path(&path)? {
                    self.durable_state_overlay.insert(scoped_path, None);
                }
                self.durable_state_overlay.insert(path, None);
                Ok(16)
            }
            ivm::syscalls::SYSCALL_STATE_KEYS => {
                let prefix_ptr = vm.register(10);
                let prefix_tlv = Self::expect_tlv(vm, prefix_ptr, PointerType::Name)?;
                let prefix = Self::decode_name_payload(prefix_tlv.payload)?;
                self.log_state_read_key(prefix.as_ref());
                let keys = self.collect_durable_state_keys(&prefix)?;
                let selected = Self::paged_state_keys(&keys, vm.register(11), vm.register(12));
                let payload = Self::encode_norito_payload(&selected)?;
                let ptr = Self::alloc_norito_bytes(vm, &payload)?;
                vm.set_register(10, ptr);
                vm.set_register(11, u64::try_from(keys.len()).unwrap_or(u64::MAX));
                vm.set_register(12, u64::try_from(selected.len()).unwrap_or(u64::MAX));
                Ok(Self::state_keys_gas(selected.len(), payload.len()))
            }
            ivm::syscalls::SYSCALL_STATE_HAS => {
                let path_ptr = vm.register(10);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_read_key(path.as_ref());
                vm.set_register(10, u64::from(self.durable_state_key_present(&path)?));
                Ok(16)
            }
            ivm::syscalls::SYSCALL_STATE_LEN => {
                let path_ptr = vm.register(10);
                let path_tlv = Self::expect_tlv(vm, path_ptr, PointerType::Name)?;
                let path = Self::decode_name_payload(path_tlv.payload)?;
                self.log_state_read_key(path.as_ref());
                if let Some(len) = self.durable_state_value_len(&path)? {
                    vm.set_register(10, u64::try_from(len).unwrap_or(u64::MAX));
                    vm.set_register(11, 1);
                    Ok(Self::state_query_gas(len))
                } else {
                    vm.set_register(10, 0);
                    vm.set_register(11, 0);
                    Ok(16)
                }
            }
            ivm::syscalls::SYSCALL_STATE_COUNT => {
                let prefix_ptr = vm.register(10);
                let prefix_tlv = Self::expect_tlv(vm, prefix_ptr, PointerType::Name)?;
                let prefix = Self::decode_name_payload(prefix_tlv.payload)?;
                self.log_state_read_key(prefix.as_ref());
                let total = self.collect_durable_state_keys(&prefix)?.len();
                vm.set_register(10, u64::try_from(total).unwrap_or(u64::MAX));
                Ok(Self::state_count_gas(total))
            }
            // Norito serialization and numeric helpers delegate to the ivm core host shim.
            ivm::syscalls::SYSCALL_BUILD_PATH_MAP_KEY
            | ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT
            | ivm::syscalls::SYSCALL_ENCODE_INT
            | ivm::syscalls::SYSCALL_DECODE_INT
            | ivm::syscalls::SYSCALL_NUMERIC_FROM_INT
            | ivm::syscalls::SYSCALL_NUMERIC_TO_INT
            | ivm::syscalls::SYSCALL_NUMERIC_TO_INT_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_ADD
            | ivm::syscalls::SYSCALL_NUMERIC_ADD_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_SUB
            | ivm::syscalls::SYSCALL_NUMERIC_SUB_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_MUL
            | ivm::syscalls::SYSCALL_NUMERIC_MUL_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_DIV
            | ivm::syscalls::SYSCALL_NUMERIC_DIV_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_REM
            | ivm::syscalls::SYSCALL_NUMERIC_REM_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_NEG
            | ivm::syscalls::SYSCALL_NUMERIC_NEG_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_EQ
            | ivm::syscalls::SYSCALL_NUMERIC_EQ_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_NE
            | ivm::syscalls::SYSCALL_NUMERIC_NE_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_LT
            | ivm::syscalls::SYSCALL_NUMERIC_LT_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_LE
            | ivm::syscalls::SYSCALL_NUMERIC_LE_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_GT
            | ivm::syscalls::SYSCALL_NUMERIC_GT_DIRECT
            | ivm::syscalls::SYSCALL_NUMERIC_GE
            | ivm::syscalls::SYSCALL_NUMERIC_GE_DIRECT
            | ivm::syscalls::SYSCALL_JSON_ENCODE
            | ivm::syscalls::SYSCALL_JSON_DECODE
            | ivm::syscalls::SYSCALL_JSON_OBJECT
            | ivm::syscalls::SYSCALL_JSON_SET_I64
            | ivm::syscalls::SYSCALL_JSON_SET_I64_DIRECT
            | ivm::syscalls::SYSCALL_JSON_SET_ACCOUNT_ID
            | ivm::syscalls::SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT
            | ivm::syscalls::SYSCALL_TLV_LEN
            | ivm::syscalls::SYSCALL_JSON_GET_I64
            | ivm::syscalls::SYSCALL_JSON_GET_I64_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_JSON
            | ivm::syscalls::SYSCALL_JSON_GET_JSON_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_NAME
            | ivm::syscalls::SYSCALL_JSON_GET_NAME_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID
            | ivm::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_NFT_ID
            | ivm::syscalls::SYSCALL_JSON_GET_NFT_ID_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_BLOB_HEX
            | ivm::syscalls::SYSCALL_JSON_GET_BLOB_HEX_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID
            | ivm::syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID_DIRECT
            | ivm::syscalls::SYSCALL_JSON_GET_NUMERIC
            | ivm::syscalls::SYSCALL_JSON_GET_NUMERIC_DIRECT
            | ivm::syscalls::SYSCALL_SCHEMA_ENCODE
            | ivm::syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT
            | ivm::syscalls::SYSCALL_SCHEMA_DECODE
            | ivm::syscalls::SYSCALL_SCHEMA_DECODE_DIRECT
            | ivm::syscalls::SYSCALL_POINTER_TO_NORITO
            | ivm::syscalls::SYSCALL_POINTER_FROM_NORITO
            | ivm::syscalls::SYSCALL_TLV_EQ
            | ivm::syscalls::SYSCALL_SCHEMA_INFO
            | ivm::syscalls::SYSCALL_SCHEMA_INFO_DIRECT
            | ivm::syscalls::SYSCALL_NAME_DECODE => self.codec_host.syscall(number, vm),
            // Stateless helpers are safe to forward to DefaultHost.
            ivm::syscalls::SYSCALL_DEBUG_PRINT
            | ivm::syscalls::SYSCALL_EXIT
            | ivm::syscalls::SYSCALL_ABORT
            | ivm::syscalls::SYSCALL_DEBUG_LOG
            | ivm::syscalls::SYSCALL_ALLOC
            | ivm::syscalls::SYSCALL_GROW_HEAP
            | ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT
            | ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV
            | ivm::syscalls::SYSCALL_COMMIT_OUTPUT
            | ivm::syscalls::SYSCALL_PROVE_EXECUTION
            | ivm::syscalls::SYSCALL_VERIFY_SIGNATURE
            | ivm::syscalls::SYSCALL_SHA256_HASH
            | ivm::syscalls::SYSCALL_SHA3_HASH
            | ivm::syscalls::SYSCALL_BLAKE2B256_HASH
            | ivm::syscalls::SYSCALL_KECCAK256_HASH
            | ivm::syscalls::SYSCALL_IROHA_HASH
            | ivm::syscalls::SYSCALL_GET_MERKLE_PATH
            | ivm::syscalls::SYSCALL_GET_MERKLE_COMPACT
            | ivm::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT
            | ivm::syscalls::SYSCALL_USE_NULLIFIER
            | ivm::syscalls::SYSCALL_VRF_VERIFY
            | ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH => self.default.syscall(number, vm),

            // Provide current authority AccountId pointer via INPUT region.
            // New format: TLV (type_id:u16, version:u8, len:be u32, payload, hash:32).
            // Historical len-prefixed payloads are no longer emitted.
            ivm::syscalls::SYSCALL_GET_AUTHORITY | ivm::syscalls::SYSCALL_SYSVAR_AUTHORITY => {
                // Encode authority payload
                let payload =
                    norito::to_bytes(&self.authority).map_err(|_| ivm::VMError::NoritoInvalid)?;
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
                Ok(Self::sysvar_gas(payload.len()))
            }
            ivm::syscalls::SYSCALL_CURRENT_TIME_MS
            | ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS => {
                vm.set_register(10, self.current_block_time_ms.unwrap_or(0));
                Ok(Self::sysvar_gas(0))
            }
            ivm::syscalls::SYSCALL_SYSVAR_CHAIN_ID => {
                if self.chain_id_bytes.is_empty() {
                    vm.set_register(10, 0);
                    Ok(Self::sysvar_gas(0))
                } else {
                    let ptr = Self::alloc_tlv_payload(vm, PointerType::Blob, &self.chain_id_bytes)?;
                    vm.set_register(10, ptr);
                    Ok(Self::sysvar_gas(self.chain_id_bytes.len()))
                }
            }
            ivm::syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT => {
                vm.set_register(10, self.current_block_height.unwrap_or(0));
                Ok(Self::sysvar_gas(0))
            }
            ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS => {
                let Some(context) = self.current_contract_runtime_context.as_ref() else {
                    vm.set_register(10, 0);
                    return Ok(Self::sysvar_gas(0));
                };
                let payload = norito::to_bytes(&context.contract_address)
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                let ptr = Self::alloc_norito_bytes(vm, &payload)?;
                vm.set_register(10, ptr);
                Ok(Self::sysvar_gas(payload.len()))
            }
            ivm::syscalls::SYSCALL_SYSVAR_ENTRYPOINT => {
                let Some(context) = self.current_contract_runtime_context.as_ref() else {
                    vm.set_register(10, 0);
                    return Ok(Self::sysvar_gas(0));
                };
                let ptr =
                    Self::alloc_tlv_payload(vm, PointerType::Blob, context.entrypoint.as_bytes())?;
                vm.set_register(10, ptr);
                Ok(Self::sysvar_gas(context.entrypoint.len()))
            }
            ivm::syscalls::SYSCALL_QUERY_GET_ACCOUNT => {
                let id: AccountId =
                    Self::decode_query_key(vm, vm.register(10), PointerType::AccountId)?;
                let request = QueryRequest::Singular(SingularQueryBox::FindAccountById(
                    iroha_data_model::query::account::prelude::FindAccountById { id },
                ));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::Account(account) => {
                        Self::encode_norito_payload(&account)
                    }
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_QUERY_GET_ASSET => {
                let id: AssetId =
                    Self::decode_query_key(vm, vm.register(10), PointerType::AssetId)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindAssetById(FindAssetById { id }));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::Asset(asset) => Self::encode_norito_payload(&asset),
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_QUERY_GET_ASSET_DEFINITION => {
                let id: AssetDefinitionId =
                    Self::decode_query_key(vm, vm.register(10), PointerType::AssetDefinitionId)?;
                let request = QueryRequest::Singular(SingularQueryBox::FindAssetDefinitionById(
                    iroha_data_model::query::asset::prelude::FindAssetDefinitionById { id },
                ));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::AssetDefinition(definition) => {
                        Self::encode_norito_payload(&definition)
                    }
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_QUERY_GET_DOMAIN => {
                let id: DomainId =
                    Self::decode_query_key(vm, vm.register(10), PointerType::DomainId)?;
                let request = QueryRequest::Singular(SingularQueryBox::FindDomainById(
                    iroha_data_model::query::domain::prelude::FindDomainById { id },
                ));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::Domain(domain) => Self::encode_norito_payload(&domain),
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST => {
                let code_hash: Hash =
                    Self::decode_query_key(vm, vm.register(10), PointerType::NoritoBytes)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindContractManifestByCodeHash(
                        iroha_data_model::query::smart_contract::prelude::FindContractManifestByCodeHash {
                            code_hash,
                        },
                    ));
                self.execute_singular_query_payload(vm, number, request, |output| match output {
                    SingularQueryOutputBox::ContractManifest(manifest) => {
                        Self::encode_norito_payload(&manifest)
                    }
                    _ => Err(ivm::VMError::DecodeError),
                })
            }
            ivm::syscalls::SYSCALL_QUERY_GET_NFT => {
                let id: NftId = Self::decode_query_key(vm, vm.register(10), PointerType::NftId)?;
                let gas_remaining = vm.remaining_gas();
                let gas_ctx = QueryGasContext::singular();
                let Some(state_ref) = self.query_state.get() else {
                    return Err(ivm::VMError::NotImplemented { syscall: number });
                };
                if Self::query_items_budget(&gas_ctx, gas_remaining)? == 0 {
                    return Err(ivm::VMError::OutOfGas);
                }
                let nft = state_ref.nft_by_id(&id)?;
                let payload = Self::encode_norito_payload(&nft)?;
                Self::finish_direct_singular_query_payload(vm, gas_remaining, payload)
            }
            ivm::syscalls::SYSCALL_QUERY_GET_PARAMETER => {
                let name: Name = Self::decode_query_key(vm, vm.register(10), PointerType::Name)?;
                let gas_remaining = vm.remaining_gas();
                let gas_ctx = QueryGasContext::singular();
                let Some(state_ref) = self.query_state.get() else {
                    return Err(ivm::VMError::NotImplemented { syscall: number });
                };
                if Self::query_items_budget(&gas_ctx, gas_remaining)? == 0 {
                    return Err(ivm::VMError::OutOfGas);
                }
                let parameter = state_ref.parameter_by_name(&name)?;
                let payload = Self::encode_norito_payload(&parameter)?;
                Self::finish_direct_singular_query_payload(vm, gas_remaining, payload)
            }
            ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE => {
                let lookup = Self::decode_contract_instance_lookup(vm, vm.register(10))?;
                let gas_remaining = vm.remaining_gas();
                let gas_ctx = QueryGasContext::singular();
                let Some(state_ref) = self.query_state.get() else {
                    return Err(ivm::VMError::NotImplemented { syscall: number });
                };
                if Self::query_items_budget(&gas_ctx, gas_remaining)? == 0 {
                    return Err(ivm::VMError::OutOfGas);
                }
                let instance = match lookup {
                    ContractInstanceLookup::Address(address) => {
                        state_ref.contract_instance_by_address(&address)?
                    }
                    ContractInstanceLookup::Alias(alias) => {
                        state_ref.contract_instance_by_alias(&alias)?
                    }
                };
                let payload = Self::encode_norito_payload(&instance)?;
                Self::finish_direct_singular_query_payload(vm, gas_remaining, payload)
            }
            ivm::syscalls::SYSCALL_CALL_CONTRACT => self.handle_call_contract(vm),

            ivm::syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            ivm::syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            ivm::syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(),
            ivm::syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            ivm::syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),

            // All other allowed stateful operations must be routed via ISIs
            // before this host can execute them directly.
            _ => Err(ivm::VMError::metered_not_implemented(
                ivm::gas::G_CONTRACT_ADMIN,
                number,
            )),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
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
    use core::{num::NonZeroU16, str::FromStr};

    use iroha_crypto::{Algorithm, Hash as IrohaHash, KeyPair};
    use iroha_data_model::smart_contract::manifest::ContractManifest;
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use ivm::{
        axt::{GroupBinding, HandleBudget, HandleSubject, SpendOp},
        syscalls as ivm_sys,
    };

    use super::{
        tests::{begin_axt_envelope, make_policy_snapshot, norito_blob, proof_blob_for, store_tlv},
        *,
    };
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::triggers::specialized::SpecializedAction,
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

    fn fixture_account(label: &str) -> AccountId {
        match label {
            "alice" => ALICE_ID.clone(),
            "bob" => BOB_ID.clone(),
            "carol" | "charlie" => {
                let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
                let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
                AccountId::new(public_key)
            }
            other => panic!("unsupported fixture account label: {other}"),
        }
    }

    fn fixture_account_literal(label: &str) -> String {
        fixture_account(label).to_string()
    }

    #[test]
    fn strict_policy_accepts_domain_in_abi_current() {
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
        let did: DomainId = DomainId::try_new("wonder", "universal").unwrap();
        let payload = norito::to_bytes(&did).expect("encode domain id");
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
        // Prepare a valid TLV for a canonical encoded AccountId at INPUT_START.
        let acc: AccountId = fixture_account("alice");
        let payload = norito::to_bytes(&acc).expect("encode account id");
        let tlv = make_tlv(1u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload input");

        let decoded: AccountId =
            CoreHost::decode_tlv_typed(&vm, ivm::Memory::INPUT_START, PointerType::AccountId)
                .expect("decode TLV");
        assert_eq!(decoded, acc);
    }

    #[test]
    fn json_tlv_rejects_invalid_norito_payload() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        let mut payload = norito_blob(&Json::new("hello"));
        if let Some(first) = payload.first_mut() {
            *first ^= 0xFF;
        }
        let tlv = make_tlv(PointerType::Json as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload json tlv");

        let err = CoreHost::decode_tlv_json(&vm, ivm::Memory::INPUT_START)
            .expect_err("invalid norito json payload should be rejected");
        assert!(matches!(
            err,
            ivm::VMError::DecodeError | ivm::VMError::NoritoInvalid
        ));
    }

    #[test]
    fn json_tlv_decodes_norito_payload() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        let expected = Json::new("hello");
        let payload = norito_blob(&expected);
        let tlv = make_tlv(PointerType::Json as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload json tlv");

        let decoded = CoreHost::decode_tlv_json(&vm, ivm::Memory::INPUT_START)
            .expect("norito json payload should decode");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn tlv_decode_invalid_hash_rejected() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000_000);
        let acc: AccountId = fixture_account("alice");
        let payload = norito::to_bytes(&acc).expect("encode account id");
        let mut tlv = make_tlv(1u16, &payload);
        // Corrupt one byte of the hash (last byte)
        let last = tlv.len() - 1;
        tlv[last] ^= 0xFF;
        vm.memory.preload_input(0, &tlv).expect("preload input");

        let err = CoreHost::decode_tlv_typed::<AccountId>(
            &vm,
            ivm::Memory::INPUT_START,
            PointerType::AccountId,
        )
        .expect_err("invalid hash must be rejected");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
    }

    #[test]
    fn pointer_from_norito_wrong_type_is_rejected() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority);

        let name_payload = norito::to_bytes(&Name::from_str("rose").unwrap()).expect("encode name");
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
    fn load_state_value_roundtrips_wrapped_account_pointer_via_pointer_from_norito() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority);
        let account: AccountId = fixture_account("alice");
        let inner = make_tlv(
            PointerType::AccountId as u16,
            &norito::to_bytes(&account).expect("encode account"),
        );
        let stored = make_tlv(PointerType::NoritoBytes as u16, &inner);

        CoreHost::load_state_value(&mut vm, &stored).expect("load wrapped state value");
        vm.set_register(11, u64::from(PointerType::AccountId as u16));
        host.syscall(ivm::syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm)
            .expect("pointer_from_norito");

        let decoded: AccountId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
                .expect("decode account pointer from state");
        assert_eq!(decoded, account);
    }

    #[test]
    fn load_state_value_roundtrips_wrapped_asset_pointer_via_pointer_from_norito() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority);
        let asset: AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
            .parse()
            .expect("asset definition literal");
        let inner = make_tlv(
            PointerType::AssetDefinitionId as u16,
            &norito::to_bytes(&asset).expect("encode asset definition"),
        );
        let stored = make_tlv(PointerType::NoritoBytes as u16, &inner);

        CoreHost::load_state_value(&mut vm, &stored).expect("load wrapped state value");
        vm.set_register(11, u64::from(PointerType::AssetDefinitionId as u16));
        host.syscall(ivm::syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm)
            .expect("pointer_from_norito");

        let decoded: AssetDefinitionId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AssetDefinitionId)
                .expect("decode asset definition pointer from state");
        assert_eq!(decoded, asset);
    }

    #[test]
    fn load_state_value_rejects_wrapped_non_norito_bytes() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let stored = make_tlv(
            PointerType::Name as u16,
            &norito_blob(&Name::from_str("wrong").expect("valid name")),
        );

        let err =
            CoreHost::load_state_value(&mut vm, &stored).expect_err("wrapped non-NoritoBytes TLV");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
    }

    #[test]
    fn load_state_value_wraps_raw_norito_payload_into_norito_bytes_tlv() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let expected = vec![1_u8, 2, 3, 4, 5];
        let stored = norito::to_bytes(&expected).expect("encode raw state value");

        CoreHost::load_state_value(&mut vm, &stored).expect("load raw state value");
        let tlv = vm
            .validate_tlv(vm.register(10))
            .expect("wrapped raw state tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let decoded: Vec<u8> =
            norito::decode_from_bytes(tlv.payload).expect("decode wrapped raw state value");
        assert_eq!(decoded, expected);
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
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);

        let mismatched_envelope = axt::AxtProofEnvelope {
            dsid,
            manifest_root: [0xFE; 32],
            da_commitment: None,
            proof: vec![0x01, 0x02, 0x03],
            fastpq_binding: None,
            committed_amount: None,
            amount_commitment: None,
        };
        let mismatched_proof = ProofBlob {
            payload: norito::to_bytes(&mismatched_envelope).expect("encode mismatched envelope"),
            expiry_slot: Some(20),
        };
        let proof_bytes = norito_blob(&mismatched_proof);
        let decoded: ProofBlob =
            norito::decode_from_bytes(&proof_bytes).expect("decode proof blob");
        assert_eq!(decoded.payload, mismatched_proof.payload);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);
        let proof_tlv = vm
            .memory
            .validate_tlv(proof_ptr)
            .expect("proof TLV should decode");
        assert!(
            norito::decode_from_bytes::<ProofBlob>(proof_tlv.payload).is_ok(),
            "proof payload must decode"
        );
        vm.set_register(11, proof_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm);
        assert!(
            matches!(res, Err(VMError::PermissionDenied)),
            "manifest root mismatch must be rejected, got {res:?}"
        );
        assert!(host.axt_proof_cache.is_empty(), "cache must stay empty");

        let aligned_proof = proof_blob_for(dsid, manifest_root, b"aligned-proof", 20);
        let aligned_proof_bytes = norito_blob(&aligned_proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &aligned_proof_bytes);
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(aligned_proof_bytes.len()))
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
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&other));
        let proof = proof_blob_for(dsid, manifest_root, b"undeclared-dataspace", 20);
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
    fn axt_verify_ds_proof_records_raw_expiry() {
        crate::test_alias::ensure();
        let dsid = DataSpaceId::new(8);
        let manifest_root = [0x21; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 12);
        let timing = iroha_config::parameters::actual::NexusAxt {
            max_clock_skew_ms: 1,
            ..Default::default()
        };
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(timing);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let proof = proof_blob_for(dsid, manifest_root, b"raw-expiry", 11);
        let proof_bytes = norito_blob(&proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, proof_ptr);

        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(proof_bytes.len()))
        );
        let recorded = host
            .axt_state
            .as_ref()
            .expect("axt_state present")
            .proofs()
            .get(&dsid)
            .expect("proof recorded");
        assert_eq!(recorded.expiry_slot, Some(11));
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);
        let proof = proof_blob_for(dsid, manifest_root, b"cache-slot", 30);
        let proof_bytes = norito_blob(&proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(proof_bytes.len()))
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
            Ok(CoreHost::axt_verify_gas(proof_bytes.len())),
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
        let new_proof = proof_blob_for(dsid, new_manifest_root, b"cache-rotated-root", 30);
        let new_proof_bytes = norito_blob(&new_proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &new_proof_bytes);
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(new_proof_bytes.len())),
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
        let authority: AccountId = fixture_account("alice");
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            ))
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
                to: fixture_account_literal("bob"),
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            ))
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
                to: fixture_account_literal("bob"),
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone()).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            ))
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
                to: fixture_account_literal("bob"),
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
        let authority: AccountId = fixture_account("alice");
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
        let proof = proof_blob_for(dsid, manifest_root, b"telemetry-cache", 30);
        let proof_bytes = norito_blob(&proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(proof_bytes.len()))
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
        let rotated_proof = proof_blob_for(dsid, rotated_root, b"telemetry-cache-rotated", 40);
        let rotated_proof_bytes = norito_blob(&rotated_proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &rotated_proof_bytes);
        vm.set_register(11, proof_ptr);
        vm.set_register(10, ds_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(rotated_proof_bytes.len()))
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
        snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
        let authority: AccountId = fixture_account("alice");
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
        let mut host = CoreHost::new(authority.clone())
            .with_axt_policy_snapshot(&snapshot)
            .with_axt_timing(iroha_config::parameters::actual::NexusAxt::default());
        host.set_telemetry(telemetry.clone());
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        // Touch the dataspace so handle validation can proceed to policy checks.
        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest = TouchManifest {
            read: Vec::new(),
            write: Vec::new(),
        };
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            )),
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
                to: fixture_account_literal("bob"),
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
        let authority: AccountId = fixture_account("alice");
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
                to: fixture_account_literal("bob"),
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
        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            )),
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
        let ds_bytes = norito_blob(&dsid);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &ds_bytes);
        let manifest_bytes = norito_blob(&manifest);
        let manifest_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &manifest_bytes);
        vm.set_register(10, ds_ptr);
        vm.set_register(11, manifest_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_TOUCH, &mut vm),
            Ok(CoreHost::axt_gas(
                ds_bytes.len().saturating_add(manifest_bytes.len())
            )),
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);

        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);

        let expired_proof = proof_blob_for(dsid, manifest_root, b"expired-proof", 10);
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

        let live_proof = proof_blob_for(dsid, manifest_root, b"live-proof", 50);
        let live_proof_bytes = norito_blob(&live_proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &live_proof_bytes);
        vm.set_register(11, proof_ptr);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(live_proof_bytes.len()))
        );
        assert!(host.axt_proof_cache.contains_key(&dsid));

        vm.set_register(11, 0);
        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
            Ok(CoreHost::axt_verify_gas(0)),
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
        let authority: AccountId = fixture_account("alice");
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
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority).with_axt_policy_snapshot(&snapshot);
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("axt binding");
        host.axt_state = Some(axt::HostAxtState::new(descriptor, binding));

        let proof = proof_blob_for(dsid, manifest_root, b"cache-snapshot", 30);
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
        let authority: AccountId = fixture_account("alice");
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
        let authority: AccountId = fixture_account("alice");
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
        let authority: AccountId = fixture_account("alice");
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
                to: fixture_account_literal("bob"),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
            amount_commitment: None,
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
        let authority: AccountId = fixture_account("alice");
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
                to: fixture_account_literal("bob"),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
            amount_commitment: None,
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
        let authority: AccountId = fixture_account("alice");
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
                to: fixture_account_literal("bob"),
                amount: "5".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 5,
            amount_commitment: None,
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
        let authority: AccountId = fixture_account("alice");
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
                to: fixture_account_literal("charlie"),
                amount: "3".into(),
            },
        };
        let usage = axt::HandleUsage {
            handle,
            intent,
            proof: None,
            amount: 3,
            amount_commitment: None,
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
        let authority = AccountId::of(kp.public_key().clone());
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
                kotoba: None,
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
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"bytecode");
        let request = scode::RegisterSmartContractBytes {
            code_hash,
            code: vec![0xAA, 0xBB, 0xCC],
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            &mut vm,
        );
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_contract_bytes_syscall_accepts_heap_tlv() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"heap-bytecode");
        let request = scode::RegisterSmartContractBytes {
            code_hash,
            code: vec![0xAA, 0xBB, 0xCC],
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        let ptr = vm
            .alloc_heap(tlv.len() as u64)
            .expect("allocate heap pointer");
        vm.store_bytes(ptr, &tlv).expect("store heap tlv");
        let heap_bytes = vm
            .memory
            .load_region(ptr, tlv.len() as u64)
            .expect("read heap tlv bytes");
        let tlv_view =
            pointer_abi::validate_tlv_bytes(heap_bytes).expect("validate heap tlv bytes");
        assert_eq!(tlv_view.type_id, PointerType::NoritoBytes);
        let decoded: scode::RegisterSmartContractBytes =
            CoreHost::decode_tlv_typed_any_region(&vm, ptr, PointerType::NoritoBytes)
                .expect("decode heap register request");
        assert_eq!(decoded, request);
        vm.set_register(10, ptr);

        let res = host.syscall(
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            &mut vm,
        );
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn activate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = CoreHost::new(authority);

        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &host.authority,
            0,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let request = scode::ActivateContractInstance {
            contract_address,
            code_hash: IrohaHash::new(b"payments-code"),
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn deactivate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = CoreHost::new(authority);

        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &host.authority,
            1,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let request = scode::DeactivateContractInstance {
            contract_address,
            reason: Some("compromised deployment".to_owned()),
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn remove_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = KeyPair::random();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = CoreHost::new(authority);

        let code_hash = IrohaHash::new(b"bytecode-image");
        let request = scode::RemoveSmartContractBytes {
            code_hash,
            reason: None,
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_account_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let payload = norito_blob(&account);
        let expected_account: AccountId =
            norito::decode_from_bytes(&payload).expect("decode account");
        let ptr = store_tlv(&mut vm, PointerType::AccountId, &payload);
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_ACCOUNT, &mut vm);
        let expected = InstructionBox::from(Register::account(Account::new(expected_account)));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn unregister_account_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_UNREGISTER_ACCOUNT, &mut vm);
        let expected = InstructionBox::from(Unregister::account(account));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_asset_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let payload = norito_blob(&asset_def);
        let expected_asset_def: AssetDefinitionId =
            norito::decode_from_bytes(&payload).expect("decode canonical asset definition id");
        let ptr = store_tlv(&mut vm, PointerType::AssetDefinitionId, &payload);
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_ASSET, &mut vm);
        let expected_name = CoreHost::asset_definition_name_for_syscall(&expected_asset_def);
        let expected = InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(expected_asset_def.clone()).with_name(expected_name),
        ));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn unregister_asset_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_def),
        );
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_UNREGISTER_ASSET, &mut vm);
        let expected = InstructionBox::from(Unregister::asset_definition(asset_def));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn native_escrow_syscalls_queue_expected_instructions() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let escrow_name = Name::from_str("aitai_offer").expect("fixture escrow name");
        let escrow_id = CoreHost::escrow_id_from_name(&escrow_name);
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let amount = Numeric::from(42_u64);
        let buyer_amount = Numeric::from(25_u64);
        let seller_amount = Numeric::from(17_u64);
        let evidence_hashes = vec![Hash::new("receipt"), Hash::new("judgement")];

        let escrow_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&escrow_name));
        let asset_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_definition),
        );
        let amount_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&amount));
        let buyer_amount_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&buyer_amount),
        );
        let seller_amount_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&seller_amount),
        );
        let evidence_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&evidence_hashes),
        );

        let assert_queued = |host: &mut CoreHost, res, expected: InstructionBox| {
            let expected_gas = crate::gas::meter_instruction(&expected);
            assert_eq!(res, Ok(expected_gas));
            assert_eq!(host.queued, vec![expected]);
            host.queued.clear();
        };

        vm.set_register(10, escrow_ptr);
        vm.set_register(11, asset_ptr);
        vm.set_register(12, amount_ptr);
        vm.set_register(13, evidence_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_OPEN_OFFER, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(OpenAssetEscrow {
                escrow_id: escrow_id.clone(),
                asset_definition,
                amount,
                evidence_hashes: evidence_hashes.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_ACCEPT, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(AcceptAssetEscrow {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(MarkEscrowPaymentSent {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_RELEASE, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(ReleaseAssetEscrow {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_CANCEL, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(CancelAssetEscrow {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        vm.set_register(11, evidence_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(OpenEscrowDispute {
                escrow_id: escrow_id.clone(),
                evidence_hashes: evidence_hashes.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        vm.set_register(11, buyer_amount_ptr);
        vm.set_register(12, seller_amount_ptr);
        vm.set_register(13, evidence_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(ResolveEscrowDispute {
                escrow_id,
                buyer_amount,
                seller_amount,
                evidence_hashes,
            }),
        );
    }

    #[test]
    fn native_anonymous_escrow_syscalls_queue_expected_instructions() {
        fn proof_attachment(tag: &str) -> ProofAttachment {
            let backend: iroha_schema::Ident = "halo2/ipa/poly-open".into();
            ProofAttachment::new_ref(
                backend.clone(),
                ProofBox::new(backend.clone(), tag.as_bytes().to_vec()),
                VerifyingKeyId::new(backend.as_str(), tag),
            )
        }

        let mut vm = ivm::IVM::new(2_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let escrow_name = Name::from_str("anonymous_aitai_offer").expect("fixture escrow name");
        let escrow_id = CoreHost::escrow_id_from_name(&escrow_name);
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let evidence_hashes = vec![Hash::new("receipt"), Hash::new("judgement")];
        let escrow_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&escrow_name));
        let evidence_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&evidence_hashes),
        );

        let open = OpenAnonymousAssetEscrow::with_evidence_hashes(
            escrow_id.clone(),
            asset_definition,
            vec![[0x11; 32]],
            [0x22; 32],
            proof_attachment("open"),
            Some([0x33; 32]),
            evidence_hashes.clone(),
        );
        let release = ReleaseAnonymousAssetEscrow::new(
            escrow_id.clone(),
            vec![[0x44; 32]],
            vec![[0x55; 32]],
            proof_attachment("release"),
            Some([0x66; 32]),
        );
        let cancel = CancelAnonymousAssetEscrow::new(
            escrow_id.clone(),
            vec![[0x77; 32]],
            vec![[0x88; 32]],
            proof_attachment("cancel"),
            None,
        );
        let resolve = ResolveAnonymousEscrowDispute::with_evidence_hashes(
            escrow_id.clone(),
            vec![[0x99; 32]],
            vec![[0xAA; 32]],
            vec![[0xBB; 32]],
            proof_attachment("resolve"),
            Some([0xCC; 32]),
            evidence_hashes.clone(),
        );

        let open_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&open));
        let release_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&release));
        let cancel_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&cancel));
        let resolve_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&resolve));

        let assert_queued = |host: &mut CoreHost, res, expected: InstructionBox| {
            let expected_gas = crate::gas::meter_instruction(&expected);
            assert_eq!(res, Ok(expected_gas));
            assert_eq!(host.queued, vec![expected]);
            host.queued.clear();
        };

        vm.set_register(10, open_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_OFFER, &mut vm);
        assert_queued(&mut host, res, InstructionBox::from(open));

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_ACCEPT, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(AcceptAnonymousAssetEscrow {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, escrow_ptr);
        let res = host.syscall(
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_MARK_PAYMENT_SENT,
            &mut vm,
        );
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(MarkAnonymousEscrowPaymentSent {
                escrow_id: escrow_id.clone(),
            }),
        );

        vm.set_register(10, release_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_RELEASE, &mut vm);
        assert_queued(&mut host, res, InstructionBox::from(release));

        vm.set_register(10, cancel_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_CANCEL, &mut vm);
        assert_queued(&mut host, res, InstructionBox::from(cancel));

        vm.set_register(10, escrow_ptr);
        vm.set_register(11, evidence_ptr);
        let res = host.syscall(
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_OPEN_DISPUTE,
            &mut vm,
        );
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(OpenAnonymousEscrowDispute {
                escrow_id: escrow_id.clone(),
                evidence_hashes,
            }),
        );

        vm.set_register(10, resolve_ptr);
        let res = host.syscall(
            ivm::syscalls::SYSCALL_ANONYMOUS_ESCROW_RESOLVE_DISPUTE,
            &mut vm,
        );
        assert_queued(&mut host, res, InstructionBox::from(resolve));
    }

    #[test]
    fn register_peer_syscall_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let request = RegisterPeerWithPop::new(peer_id, vec![0xAB, 0xCD]);
        let json = Json::new(request.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(request);
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn register_peer_syscall_accepts_object_peer() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let mut peer_map = BTreeMap::new();
        peer_map.insert(
            "public_key".to_string(),
            norito::json::Value::String(peer_id.public_key().to_string()),
        );
        let mut request_map = BTreeMap::new();
        request_map.insert("peer".to_string(), norito::json::Value::Object(peer_map));
        request_map.insert(
            "pop".to_string(),
            norito::json::Value::Array(vec![
                norito::json::Value::from(0xAB_u64),
                norito::json::Value::from(0xCD_u64),
            ]),
        );
        let json = Json::from(&norito::json::Value::Object(request_map));
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(RegisterPeerWithPop::new(peer_id, vec![0xAB, 0xCD]));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn unregister_peer_syscall_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let peer_id = PeerId::new(KeyPair::random().public_key().clone());
        let json = Json::new(peer_id.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_UNREGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(Unregister::peer(peer_id));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    fn assert_create_role_syscall_queues_instruction(authority: AccountId) {
        let mut vm = ivm::IVM::new(1_000);
        let mut host = CoreHost::new(authority.clone());

        let role_name: Name = "auditor".parse().unwrap();
        let perms_json = Json::new(vec!["read_assets".to_string()]);

        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&role_name));
        let perms_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&perms_json));
        vm.set_register(10, name_ptr);
        vm.set_register(11, perms_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_CREATE_ROLE, &mut vm);
        let mut role = Role::new(RoleId::new(role_name), authority);
        role = role.add_permission(Permission::new("read_assets".to_string(), Json::new(())));
        let expected = InstructionBox::from(Register::role(role));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn create_role_syscall_queues_instruction_alias() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        assert_create_role_syscall_queues_instruction(authority);
    }

    #[test]
    fn create_role_syscall_queues_instruction_account_id() {
        let authority = ALICE_ID.clone();
        assert_create_role_syscall_queues_instruction(authority);
    }

    #[test]
    fn delete_role_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let role_name: Name = "auditor".parse().unwrap();
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&role_name));
        vm.set_register(10, name_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_DELETE_ROLE, &mut vm);
        let expected = InstructionBox::from(Unregister::role(RoleId::new(role_name)));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn grant_role_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let role_name: Name = "auditor".parse().unwrap();
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let role_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&role_name));
        vm.set_register(10, account_ptr);
        vm.set_register(11, role_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_GRANT_ROLE, &mut vm);
        let expected = InstructionBox::from(Grant::account_role(RoleId::new(role_name), account));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn revoke_role_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let role_name: Name = "auditor".parse().unwrap();
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let role_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&role_name));
        vm.set_register(10, account_ptr);
        vm.set_register(11, role_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REVOKE_ROLE, &mut vm);
        let expected = InstructionBox::from(Revoke::account_role(RoleId::new(role_name), account));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn grant_permission_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let perm_name: Name = "read_assets".parse().unwrap();
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let perm_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&perm_name));
        vm.set_register(10, account_ptr);
        vm.set_register(11, perm_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_GRANT_PERMISSION, &mut vm);
        let expected = InstructionBox::from(Grant::account_permission(
            Permission::new(perm_name.as_ref().to_string(), Json::new(())),
            account,
        ));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn revoke_permission_syscall_queues_instruction_from_json() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let permission = Permission::new("transfer_asset".to_string(), Json::new(()));
        let perm_json = Json::new(permission.clone());
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let perm_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&perm_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, perm_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REVOKE_PERMISSION, &mut vm);
        let expected = InstructionBox::from(Revoke::account_permission(permission, account));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn add_signatory_syscall_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let public_key = KeyPair::random().public_key().clone();
        let pk_json = Json::new(public_key.clone());
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let pk_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&pk_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, pk_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_ADD_SIGNATORY, &mut vm);
        let expected = InstructionBox::from(AddSignatory::new(account, public_key));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn remove_signatory_syscall_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let public_key = KeyPair::random().public_key().clone();
        let pk_json = Json::new(public_key.clone());
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let pk_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&pk_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, pk_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SIGNATORY, &mut vm);
        let expected = InstructionBox::from(RemoveSignatory::new(account, public_key));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn remove_signatory_syscall_accepts_object_public_key() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let public_key = KeyPair::random().public_key().clone();
        let mut key_map = BTreeMap::new();
        key_map.insert(
            "public_key".to_string(),
            norito::json::Value::String(public_key.to_string()),
        );
        let pk_json = Json::from(&norito::json::Value::Object(key_map));
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let pk_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&pk_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, pk_ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SIGNATORY, &mut vm);
        let expected = InstructionBox::from(RemoveSignatory::new(account, public_key));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn set_account_quorum_syscall_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let account: AccountId = fixture_account("bob");
        let quorum = NonZeroU16::new(2).expect("non-zero quorum");
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        vm.set_register(10, account_ptr);
        vm.set_register(11, quorum.get().into());

        let res = host.syscall(ivm::syscalls::SYSCALL_SET_ACCOUNT_QUORUM, &mut vm);
        let expected = InstructionBox::from(SetAccountQuorum::new(account, quorum));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn create_trigger_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority.clone());

        let trigger_id: TriggerId = "trigger_base64".parse().unwrap();
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "noop".to_owned(),
                ))],
                Repeats::Exactly(1),
                authority.clone(),
                DataEventFilter::Any,
            ),
        );
        let json = Json::new(trigger.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm);
        let expected = InstructionBox::from(Register::trigger(trigger));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn create_trigger_syscall_object_spec_queues_instruction() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority.clone());

        let trigger_id: TriggerId = "trigger_object".parse().unwrap();
        let action = SpecializedAction::new(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "object".to_owned(),
            ))],
            Repeats::Exactly(1),
            authority.clone(),
            EventFilterBox::Data(DataEventFilter::Any),
        );
        let action_value = norito::json::to_value(&action).expect("serialize specialized action");
        let mut map = BTreeMap::new();
        map.insert(
            "id".to_string(),
            norito::json::Value::String(trigger_id.to_string()),
        );
        map.insert("action".to_string(), action_value);
        let spec = Json::from(&norito::json::Value::Object(map));

        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&spec));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm);
        let expected_action = Action::new(
            action.executable.clone(),
            action.repeats,
            action.authority.clone(),
            action.filter.clone(),
        )
        .with_metadata(action.metadata.clone());
        let expected =
            InstructionBox::from(Register::trigger(Trigger::new(trigger_id, expected_action)));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn create_trigger_syscall_object_spec_defaults_metadata_and_filter() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority.clone());

        let trigger_id: TriggerId = "trigger_object_fallback".parse().unwrap();
        let action = SpecializedAction::new(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "object".to_owned(),
            ))],
            Repeats::Exactly(1),
            authority.clone(),
            EventFilterBox::Data(DataEventFilter::Any),
        );
        let mut action_value =
            norito::json::to_value(&action).expect("serialize specialized action");
        let filter_value =
            norito::json::to_value(&DataEventFilter::Any).expect("serialize data filter");
        match &mut action_value {
            norito::json::Value::Object(map) => {
                map.insert("filter".to_string(), filter_value);
                map.remove("metadata");
            }
            _ => panic!("expected action object"),
        }
        let mut map = BTreeMap::new();
        map.insert(
            "id".to_string(),
            norito::json::Value::String(trigger_id.to_string()),
        );
        map.insert("action".to_string(), action_value);
        let spec = Json::from(&norito::json::Value::Object(map));

        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&spec));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm);
        let expected_action = Action::new(
            action.executable.clone(),
            action.repeats,
            action.authority.clone(),
            action.filter.clone(),
        )
        .with_metadata(action.metadata.clone());
        let expected =
            InstructionBox::from(Register::trigger(Trigger::new(trigger_id, expected_action)));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn remove_trigger_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let name: Name = "drop_me".parse().unwrap();
        let ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, ptr);

        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_TRIGGER, &mut vm);
        let expected = InstructionBox::from(Unregister::trigger(TriggerId::new(name)));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn set_trigger_enabled_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);

        let name: Name = "toggle_me".parse().unwrap();
        let ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, ptr);
        vm.set_register(11, 1);

        let res = host.syscall(ivm::syscalls::SYSCALL_SET_TRIGGER_ENABLED, &mut vm);
        let key: Name = TRIGGER_ENABLED_METADATA_KEY
            .parse()
            .expect("valid metadata key");
        let expected = InstructionBox::from(SetKeyValue::trigger(
            TriggerId::new(name),
            key,
            Json::from(true),
        ));
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(res, Ok(expected_gas));
        assert_eq!(host.queued, vec![expected]);
    }

    #[test]
    fn mint_asset_syscall_returns_metered_gas() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load header");

        let account_payload = norito::to_bytes(&authority).expect("encode account");
        let account_tlv = make_tlv(PointerType::AccountId as u16, &account_payload);
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
        let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);
        let amount = Numeric::from(5u64);
        let amount_payload = norito::to_bytes(&amount).expect("encode amount");
        let amount_tlv = make_tlv(PointerType::NoritoBytes as u16, &amount_payload);

        vm.memory
            .preload_input(0, &account_tlv)
            .expect("preload account");
        vm.memory
            .preload_input(256, &asset_tlv)
            .expect("preload asset def");
        vm.memory
            .preload_input(512, &amount_tlv)
            .expect("preload amount");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.set_register(12, ivm::Memory::INPUT_START + 512);

        let gas = host
            .syscall(ivm::syscalls::SYSCALL_MINT_ASSET, &mut vm)
            .expect("mint syscall");
        let asset_id = AssetId::of(asset_def, authority);
        let isi = Mint::asset_numeric(5u64, asset_id);
        let expected = crate::gas::meter_instruction(&InstructionBox::from(MintBox::from(isi)));
        assert_eq!(gas, expected);
    }

    #[test]
    fn mint_asset_syscall_uses_trigger_args_for_zero_amount() {
        let mut vm = ivm::IVM::new(1_000);
        let authority = ALICE_ID.clone();
        let args = Json::new(norito::json!({"val": 42}));
        let mut host: CoreHost = CoreHostImpl::with_accounts_and_args(
            authority.clone(),
            Arc::new(vec![authority.clone()]),
            args,
        );
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load header");

        let account_payload = norito::to_bytes(&authority).expect("encode account");
        let account_tlv = make_tlv(PointerType::AccountId as u16, &account_payload);
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
        let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);

        vm.memory
            .preload_input(0, &account_tlv)
            .expect("preload account");
        vm.memory
            .preload_input(256, &asset_tlv)
            .expect("preload asset def");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.set_register(12, 0);

        let gas = host
            .syscall(ivm::syscalls::SYSCALL_MINT_ASSET, &mut vm)
            .expect("mint syscall");
        let asset_id = AssetId::of(asset_def, authority);
        let isi = Mint::asset_numeric(42u64, asset_id);
        let expected =
            crate::gas::meter_instruction(&InstructionBox::from(MintBox::from(isi.clone())));
        assert_eq!(gas, expected);
        assert_eq!(host.queued, vec![InstructionBox::from(MintBox::from(isi))]);
    }

    #[test]
    fn mint_asset_syscall_accepts_fractional_trigger_args() {
        let mut vm = ivm::IVM::new(1_000);
        let authority = ALICE_ID.clone();
        let args = Json::new(norito::json!({"val": 1.25}));
        let mut host: CoreHost = CoreHostImpl::with_accounts_and_args(
            authority.clone(),
            Arc::new(vec![authority.clone()]),
            args,
        );
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load header");

        let account_payload = norito::to_bytes(&authority).expect("encode account");
        let account_tlv = make_tlv(PointerType::AccountId as u16, &account_payload);
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
        let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);

        vm.memory
            .preload_input(0, &account_tlv)
            .expect("preload account");
        vm.memory
            .preload_input(256, &asset_tlv)
            .expect("preload asset def");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.set_register(12, 0);

        let gas = host
            .syscall(ivm::syscalls::SYSCALL_MINT_ASSET, &mut vm)
            .expect("mint syscall");
        let asset_id = AssetId::of(asset_def, authority);
        let amount = Numeric::new(125_u32, 2);
        let isi = Mint::asset_numeric(amount, asset_id);
        let expected =
            crate::gas::meter_instruction(&InstructionBox::from(MintBox::from(isi.clone())));
        assert_eq!(gas, expected);
        assert_eq!(host.queued, vec![InstructionBox::from(MintBox::from(isi))]);
    }

    #[test]
    fn unknown_syscall_is_rejected_by_policy() {
        crate::test_alias::ensure();
        let mut vm = ivm::IVM::new(10_000);
        let mut host = CoreHost::new(fixture_account("alice"));
        // Pick a syscall number outside allowed/known ranges.
        const UNKNOWN_SYSCALL: u32 = u32::MAX;
        let res = host.syscall(UNKNOWN_SYSCALL, &mut vm);
        assert!(matches!(
            res,
            Err(ivm::VMError::UnknownSyscall(UNKNOWN_SYSCALL))
        ));
    }

    #[test]
    fn tlv_wrong_type_id_is_rejected() {
        crate::test_alias::ensure();
        // Build a program that calls SYSCALL_NFT_SET_METADATA and pass a TLV with wrong type for key
        let mut vm = IVM::new(1_000);
        let owner: AccountId = fixture_account("alice");
        vm.set_host(CoreHost::with_accounts(
            owner.clone(),
            Arc::new(vec![owner.clone()]),
        ));
        // Minimal program: HALT, but we only exercise host.validate_tlv via manual call
        let code = [ivm::encoding::wide::encode_halt().to_le_bytes()].concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        // Prepare TLVs: nft_id (correct), key (WRONG: Json instead of Name), value (Json)
        let nft_id = NftId::of(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "n1".parse().unwrap(),
        );
        let key: iroha_data_model::name::Name = "k".parse().unwrap();
        let value = iroha_primitives::json::Json::new("v");
        let nft_blob = norito::to_bytes(&nft_id).expect("encode nft id");
        let key_blob = norito::to_bytes(&key).expect("encode name");
        let val_blob = norito::to_bytes(&value).expect("encode json");
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
        let owner: AccountId = fixture_account("alice");
        vm.set_host(CoreHost::with_accounts(
            owner.clone(),
            Arc::new(vec![owner.clone()]),
        ));
        let code = [ivm::encoding::wide::encode_halt().to_le_bytes()].concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        // payload is a Name
        let key: iroha_data_model::name::Name = "k".parse().unwrap();
        let key_blob = norito::to_bytes(&key).expect("encode key");
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
    use std::{collections::BTreeMap, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    #[cfg(not(feature = "fast_dsl"))]
    use iroha_data_model::query::account::prelude::FindAccounts;
    use iroha_data_model::{
        parameter::{CustomParameter, Parameter},
        proof::{VerifyingKeyBox, VerifyingKeyId},
        query::{QueryRequest, QueryResponse, SingularQueryBox, prelude::FindParameters},
        zk::BackendTag,
    };
    use iroha_data_model::{
        prelude::*,
        query::{
            QueryBox, QueryWithFilter, QueryWithParams,
            dsl::prelude::{CompoundPredicate, SelectorTuple},
            parameters::{FetchSize, ForwardCursor, Pagination, QueryParams, Sorting},
        },
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
    use ivm::{IVM, encoding, instruction, syscalls as ivm_sys};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
        smartcontracts::{
            Execute,
            isi::triggers::specialized::{SpecializedAction, SpecializedTrigger},
            ivm::host::pointer_abi_tests::make_tlv,
        },
        state::{ElectionState, State, World, ZkAssetState},
    };

    #[cfg(feature = "zk-halo2-ipa")]
    fn sample_open_verify_envelope() -> iroha_data_model::zk::OpenVerifyEnvelope {
        let fixture =
            crate::zk::test_utils::halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
        iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:tiny-add".to_string(),
            vk_hash: [1u8; 32],
            public_inputs: fixture.public_inputs,
            proof_bytes: fixture.proof_bytes,
            aux: Vec::new(),
        }
    }

    pub(super) fn norito_blob<T: NoritoSerialize>(val: &T) -> Vec<u8> {
        norito::to_bytes(val).expect("encode Norito payload")
    }

    pub(super) fn store_tlv(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
        let tlv = make_tlv(ty as u16, payload);
        vm.alloc_input_tlv(&tlv).expect("allocate TLV input")
    }

    fn seed_test_call_hash(tx: &mut StateTransaction<'_, '_>, byte: u8) {
        tx.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
    }

    fn fixture_domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("fixture domain id")
    }

    fn fixture_account(label: &str) -> AccountId {
        match label {
            "alice" => ALICE_ID.clone(),
            "bob" => BOB_ID.clone(),
            "carol" | "charlie" => {
                let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
                let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
                AccountId::new(public_key)
            }
            other => panic!("unsupported fixture account label: {other}"),
        }
    }

    fn fixture_account_in_domain(label: &str, domain_label: &str) -> AccountId {
        let seed: Vec<u8> = format!("{label}@{domain_label}")
            .as_bytes()
            .iter()
            .copied()
            .cycle()
            .take(32)
            .collect();
        let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn build_fixture_account(id: &AccountId, authority: &AccountId) -> Account {
        Account::new(id.clone()).build(authority)
    }

    fn retail_dataspace_catalog() -> (
        iroha_data_model::nexus::DataSpaceId,
        iroha_data_model::nexus::DataSpaceCatalog,
    ) {
        let paynet = iroha_data_model::nexus::DataSpaceId::new(12);
        let catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: paynet,
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("retail dataspace catalog");
        (paynet, catalog)
    }

    fn fixture_signing_keypair(authority: &AccountId) -> KeyPair {
        if authority == &*ALICE_ID {
            return (*ALICE_KEYPAIR).clone();
        }
        if authority == &*BOB_ID {
            return (*BOB_KEYPAIR).clone();
        }
        panic!("unsupported fixture signing authority: {authority}");
    }

    fn contract_test_state(authority: &AccountId) -> State {
        let domain = Domain::new(fixture_domain_id()).build(authority);
        let account = build_fixture_account(authority, authority);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(world, kura, query)
    }

    fn install_contract(
        state: &State,
        authority: &AccountId,
        source: &str,
        nonce: u64,
    ) -> ContractAddress {
        let compiler = ivm::KotodamaCompiler::new();
        let (code, mut manifest) = compiler
            .compile_source_with_manifest(source)
            .expect("compile contract with manifest");
        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height must fit in u64 and be non-zero");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        let code_hash =
            register_code_bytes(authority, code, &mut tx).expect("register contract bytecode");
        manifest.code_hash = Some(code_hash);
        manifest = manifest.signed(&fixture_signing_keypair(authority));
        register_manifest(authority, manifest, &mut tx).expect("register contract manifest");
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            authority,
            nonce,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        activate_instance(authority, contract_address.clone(), code_hash, &mut tx)
            .expect("activate contract");
        tx.apply();
        block.commit().expect("commit contract registration block");
        contract_address
    }

    fn execute_contract_call_transaction_result(
        state: &State,
        authority: &AccountId,
        keypair: &KeyPair,
        invocation: iroha_data_model::transaction::executable::ContractInvocation,
        ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
    ) -> Result<(), iroha_data_model::ValidationFail> {
        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height must fit in u64 and be non-zero");
        let mut metadata = Metadata::default();
        metadata.insert(
            "gas_limit".parse().expect("static metadata key"),
            Json::new(1_000_000_u64),
        );
        let tx = TransactionBuilder::new(ChainId::from("test-chain"), authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::ContractCall(invocation))
            .sign(keypair.private_key());
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut stx = block.transaction();
        let result = crate::executor::Executor::Initial
            .execute_transaction(&mut stx, authority, tx, ivm_cache);
        if result.is_ok() {
            stx.apply();
            block
                .commit()
                .expect("commit contract call transaction block");
        }
        result
    }

    fn execute_contract_call_transaction(
        state: &State,
        authority: &AccountId,
        keypair: &KeyPair,
        invocation: iroha_data_model::transaction::executable::ContractInvocation,
        ivm_cache: &mut crate::smartcontracts::ivm::cache::IvmCache,
    ) {
        execute_contract_call_transaction_result(state, authority, keypair, invocation, ivm_cache)
            .expect("contract call transaction should execute");
    }

    fn install_alias_payout_contract(
        state: &State,
        authority: &AccountId,
        recipient_expr: &str,
        nonce: u64,
    ) -> ContractAddress {
        let source = format!(
            r#"
seiyaku AliasPayout {{
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(settlement_asset: AssetDefinitionId) {{
    SettlementAsset = settlement_asset;
  }}

  #[access(read="*", write="*")]
  kotoage fn pay(amount: int) -> int permission(AssetOps) {{
    let merchant = {recipient_expr};
    transfer_asset(authority(), merchant, SettlementAsset, amount);
    return amount;
  }}
}}
"#
        );
        install_contract(state, authority, &source, nonce)
    }

    fn call_contract_syscall(
        state: &State,
        outer_authority: &AccountId,
        caller_contract: &ContractAddress,
        callee_contract: &ContractAddress,
        entrypoint: &str,
        payload: Json,
    ) -> (
        Result<u64, ivm::VMError>,
        IVM,
        BTreeMap<Name, Option<Vec<u8>>>,
    ) {
        let view = state.view();
        let mut host = CoreHostImpl::new(outer_authority.clone());
        host.set_query_state(&view);
        host.set_durable_state_snapshot_from_world(view.world());
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_address: caller_contract.clone(),
            contract_subject: caller_contract.subject_id(),
            contract_alias: None,
            entrypoint: "invoke".to_owned(),
        }));
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load metadata-only program");
        let target_ptr = store_tlv(
            &mut vm,
            PointerType::Blob,
            callee_contract.as_ref().as_bytes(),
        );
        let entrypoint_ptr = store_tlv(&mut vm, PointerType::Blob, entrypoint.as_bytes());
        let payload_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&payload));
        vm.set_register(10, target_ptr);
        vm.set_register(11, entrypoint_ptr);
        vm.set_register(12, payload_ptr);
        let result = host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm);
        let durable_state_overlay = host.drain_durable_state_overlay();
        (result, vm, durable_state_overlay)
    }

    #[test]
    fn execute_query_syscall_returns_norito_response_and_gas() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let request = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect("query syscall");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let response: QueryResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode response");
        assert!(matches!(response, QueryResponse::Singular(_)));

        let response_len = u64::try_from(tlv.payload.len()).unwrap_or(u64::MAX);
        let gas_ctx = QueryGasContext::from_request(&request);
        let processed_items = CoreHost::query_total_items(&response);
        let expected = CoreHost::query_gas_cost(&gas_ctx, processed_items, response_len);
        assert_eq!(gas, expected);
    }

    #[test]
    fn get_account_balance_syscall_reads_numeric_asset() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let account = build_fixture_account(&authority, &authority);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let asset = Asset::new(asset_id, Numeric::new(42_u32, 0));
        let world = World::with_assets([domain], [account], [asset_def], [asset], []);

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(10_000);

        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&authority));
        let asset_def_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_def_id),
        );
        vm.set_register(10, account_ptr);
        vm.set_register(11, asset_def_ptr);

        let balance_payload = norito::to_bytes(&Numeric::new(42_u32, 0)).expect("encode balance");
        let gas = host
            .syscall(ivm_sys::SYSCALL_GET_ACCOUNT_BALANCE, &mut vm)
            .expect("get balance");
        assert_eq!(
            gas,
            CoreHost::QUERY_GAS_BASE_SINGULAR
                .saturating_add(CoreHost::QUERY_GAS_PER_ITEM)
                .saturating_add(
                    CoreHost::QUERY_GAS_PER_BYTE
                        .saturating_mul(u64::try_from(balance_payload.len()).expect("gas length"))
                )
        );
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("balance tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: Numeric =
            norito::decode_from_bytes(tlv.payload).expect("decode numeric balance");
        assert_eq!(value, Numeric::new(42_u32, 0));
    }

    #[test]
    fn dedicated_query_syscalls_return_norito_payloads() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = build_fixture_account(&authority, &authority);
        let asset_def_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let asset = Asset::new(asset_id.clone(), Numeric::new(7_u32, 0));
        let nft_id: NftId = "ticket$wonderland.universal".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&authority);
        let world = World::with_assets([domain], [account], [asset_def], [asset], [nft]);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(world, kura, query);
        let contract_address = ContractAddress::derive(0, &authority, 0, DataSpaceId::UNIVERSAL)
            .expect("contract address");
        let code_hash = Hash::new(b"dedicated-query-contract");
        crate::query::insert_contract_instance_for_test(
            &mut state,
            contract_address.clone(),
            code_hash,
        );
        let alias: ContractAlias = "router::universal".parse().expect("contract alias");
        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        iroha_data_model::isi::SetContractAlias::bind(
            contract_address.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind contract alias");
        tx.apply();
        block.commit().expect("commit contract alias block");
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&authority));
        vm.set_register(10, account_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_ACCOUNT, &mut vm)
            .expect("get account");
        let account_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("account output");
        assert_eq!(account_tlv.type_id, PointerType::NoritoBytes);
        let account_out: Account =
            norito::decode_from_bytes(account_tlv.payload).expect("decode account");
        assert_eq!(account_out.id(), &authority);

        let asset_ptr = store_tlv(&mut vm, PointerType::AssetId, &norito_blob(&asset_id));
        vm.set_register(10, asset_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_ASSET, &mut vm)
            .expect("get asset");
        let asset_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("asset output");
        let asset_out: Asset = norito::decode_from_bytes(asset_tlv.payload).expect("decode asset");
        assert_eq!(asset_out.id(), &asset_id);

        let asset_def_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_def_id),
        );
        vm.set_register(10, asset_def_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_ASSET_DEFINITION, &mut vm)
            .expect("get asset definition");
        let asset_def_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("asset definition output");
        let asset_def_out: AssetDefinition =
            norito::decode_from_bytes(asset_def_tlv.payload).expect("decode asset definition");
        assert_eq!(asset_def_out.id, asset_def_id);

        let domain_ptr = store_tlv(&mut vm, PointerType::DomainId, &norito_blob(&domain_id));
        vm.set_register(10, domain_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_DOMAIN, &mut vm)
            .expect("get domain");
        let domain_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("domain output");
        let domain_out: Domain =
            norito::decode_from_bytes(domain_tlv.payload).expect("decode domain");
        assert_eq!(domain_out.id(), &domain_id);

        let nft_ptr = store_tlv(&mut vm, PointerType::NftId, &norito_blob(&nft_id));
        vm.set_register(10, nft_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_NFT, &mut vm)
            .expect("get nft");
        let nft_tlv = vm.memory.validate_tlv(vm.register(10)).expect("nft output");
        let nft_out: Nft = norito::decode_from_bytes(nft_tlv.payload).expect("decode nft");
        assert_eq!(nft_out.id(), &nft_id);
        assert_eq!(nft_out.owned_by(), &authority);

        let parameter_name: Name = "block.max_transactions".parse().expect("parameter name");
        let parameter_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&parameter_name));
        vm.set_register(10, parameter_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_PARAMETER, &mut vm)
            .expect("get parameter");
        let parameter_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("parameter output");
        let parameter_out: Parameter =
            norito::decode_from_bytes(parameter_tlv.payload).expect("decode parameter");
        assert!(matches!(parameter_out, Parameter::Block(_)));

        let contract_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&contract_address),
        );
        vm.set_register(10, contract_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_CONTRACT_INSTANCE, &mut vm)
            .expect("get contract instance");
        let contract_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("contract instance output");
        let instance_out: ContractInstance =
            norito::decode_from_bytes(contract_tlv.payload).expect("decode contract instance");
        assert_eq!(instance_out.contract_address, contract_address);
        assert_eq!(instance_out.code_hash, code_hash);
        assert_eq!(instance_out.contract_alias, Some(alias.clone()));

        let alias_name: Name = alias.as_ref().parse().expect("alias name pointer");
        let alias_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&alias_name));
        vm.set_register(10, alias_ptr);
        host.syscall(ivm_sys::SYSCALL_QUERY_GET_CONTRACT_INSTANCE, &mut vm)
            .expect("get contract instance by alias");
        let alias_contract_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("contract alias output");
        let alias_instance_out: ContractInstance =
            norito::decode_from_bytes(alias_contract_tlv.payload)
                .expect("decode contract alias instance");
        assert_eq!(alias_instance_out.contract_address, contract_address);
        assert_eq!(alias_instance_out.code_hash, code_hash);
        assert_eq!(alias_instance_out.contract_alias, Some(alias));
    }

    #[test]
    fn block_height_sysvar_uses_attached_transaction_context() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let header = BlockHeader::new(nonzero!(9_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let tx = block.transaction();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&tx);
        let mut vm = IVM::new(10_000);

        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_SYSVAR_BLOCK_HEIGHT, &mut vm),
            Ok(CoreHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 9);
    }

    #[test]
    fn execute_query_syscall_charges_sorted_queries_by_scanned_items() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let accounts = vec![
            build_fixture_account(&authority, &authority),
            build_fixture_account(&fixture_account("bob"), &authority),
            build_fixture_account(&fixture_account("carol"), &authority),
        ];
        let world = World::with([domain], accounts, []);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let sort_key: Name = "rank".parse().unwrap();
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(1_u64)), 0),
            sorting: Sorting::by_metadata_key(sort_key),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };
        let query_box = {
            #[cfg(not(feature = "fast_dsl"))]
            {
                let query = QueryWithFilter::<Account>::new_with_query(
                    Box::new(FindAccounts),
                    CompoundPredicate::PASS,
                    SelectorTuple::default(),
                );
                QueryBox::from(query)
            }
            #[cfg(feature = "fast_dsl")]
            {
                let query = QueryWithFilter::<Account>::new_with_query(
                    (),
                    CompoundPredicate::PASS,
                    SelectorTuple::default(),
                );
                QueryBox::from(query)
            }
        };
        let request = QueryRequest::Start({
            #[cfg(feature = "fast_dsl")]
            {
                QueryWithParams::new(&query_box, params)
            }
            #[cfg(not(feature = "fast_dsl"))]
            {
                QueryWithParams::new(query_box, params)
            }
        });
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect("query syscall");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        let response: QueryResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode response");
        let QueryResponse::Iterable(output) = response else {
            panic!("expected iterable query response");
        };
        assert_eq!(output.batch.len(), 1);
        assert_eq!(output.remaining_items, Some(0));

        let response_len = u64::try_from(tlv.payload.len()).unwrap_or(u64::MAX);
        let gas_ctx = QueryGasContext::from_request(&request);
        let expected = CoreHost::query_gas_cost(&gas_ctx, 3, response_len);
        assert_eq!(gas, expected);
    }

    #[test]
    fn execute_query_syscall_sorted_offset_ignores_offset_penalty() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let domain: Domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let accounts = vec![
            build_fixture_account(&authority, &authority),
            build_fixture_account(&fixture_account("bob"), &authority),
            build_fixture_account(&fixture_account("carol"), &authority),
        ];
        let world = World::with([domain], accounts, []);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let sort_key: Name = "rank".parse().unwrap();
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(1_u64)), 2),
            sorting: Sorting::by_metadata_key(sort_key),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };
        let query_box = {
            #[cfg(not(feature = "fast_dsl"))]
            {
                let query = QueryWithFilter::<Account>::new_with_query(
                    Box::new(FindAccounts),
                    CompoundPredicate::PASS,
                    SelectorTuple::default(),
                );
                QueryBox::from(query)
            }
            #[cfg(feature = "fast_dsl")]
            {
                let query = QueryWithFilter::<Account>::new_with_query(
                    (),
                    CompoundPredicate::PASS,
                    SelectorTuple::default(),
                );
                QueryBox::from(query)
            }
        };
        let request = QueryRequest::Start({
            #[cfg(feature = "fast_dsl")]
            {
                QueryWithParams::new(&query_box, params)
            }
            #[cfg(not(feature = "fast_dsl"))]
            {
                QueryWithParams::new(query_box, params)
            }
        });
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect("query syscall");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");

        let response_len = u64::try_from(tlv.payload.len()).unwrap_or(u64::MAX);
        let gas_ctx = QueryGasContext::from_request(&request);
        let expected = gas_ctx
            .base
            .saturating_add(gas_ctx.per_item.saturating_mul(3))
            .saturating_add(CoreHost::QUERY_GAS_PER_BYTE.saturating_mul(response_len));
        assert_eq!(gas, expected);
    }

    #[test]
    fn execute_query_syscall_out_of_gas_when_budget_exhausted() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(CoreHost::QUERY_GAS_BASE_SINGULAR - 1);

        let request = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect_err("query should run out of gas");
        assert!(matches!(err, ivm::VMError::OutOfGas));
    }

    #[test]
    fn execute_query_syscall_out_of_gas_when_response_bytes_exceed_budget() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(CoreHost::QUERY_GAS_BASE_SINGULAR + CoreHost::QUERY_GAS_PER_ITEM);

        let request = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect_err("query should run out of gas on response bytes");
        assert!(matches!(err, ivm::VMError::OutOfGas));
    }

    #[test]
    fn execute_query_syscall_rejects_continue_request() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let cursor = ForwardCursor {
            query: "ivm-cursor".to_string(),
            cursor: nonzero!(1_u64),
            gas_budget: None,
        };
        let request = QueryRequest::Continue(cursor);
        let request_bytes = norito::to_bytes(&request).expect("encode query request");
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &request_bytes);
        vm.set_register(10, ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
            .expect_err("continue should be rejected");
        assert!(matches!(err, ivm::VMError::PermissionDenied));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_bill_fixed_plan_transfers_and_reschedules() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let plan_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
        let charge_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
        let period_ms = 1_000_u64;
        let scheduled_at_ms = 10_000_u64;
        let trigger_id: TriggerId = "sub-bill".parse().unwrap();
        let amount = Numeric::new(120_u32, 0);

        let plan = SubscriptionPlan {
            provider: provider.clone(),
            billing: SubscriptionBilling {
                cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                    period_ms,
                }),
                bill_for: SubscriptionBillFor::PreviousPeriod,
                retry_backoff_ms: 500,
                max_failures: 3,
                grace_ms: 5_000,
            },
            pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                amount: amount.clone(),
                asset_definition: charge_asset_id.clone(),
            }),
        };

        let mut plan_def =
            AssetDefinition::new(plan_id.clone(), NumericSpec::integer()).build(&provider);
        let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
        plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
        let charge_def =
            AssetDefinition::new(charge_asset_id.clone(), NumericSpec::integer()).build(&provider);
        let asset_id = AssetId::of(charge_asset_id.clone(), subscriber.clone());
        let asset = Asset::new(asset_id.clone(), Numeric::new(500_u32, 0));

        let subscription_state = SubscriptionState {
            plan_id: plan_id.clone(),
            provider: provider.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: scheduled_at_ms - period_ms,
            current_period_end_ms: scheduled_at_ms,
            next_charge_ms: scheduled_at_ms,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 0,
            usage_accumulated: BTreeMap::new(),
            billing_trigger_id: trigger_id.clone(),
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let nft_id: NftId = "sub-0$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let domains = vec![
            Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let world = World::with_assets(domains, accounts, [plan_def, charge_def], [asset], [nft]);

        let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
        let mut trigger_metadata = Metadata::default();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
        trigger_metadata.insert(
            trigger_ref_key,
            Json::new(SubscriptionTriggerRef {
                subscription_nft_id: nft_id.clone(),
            }),
        );
        // Runtime registration metadata is present on real triggers and must be stripped
        // before re-registering, otherwise registration rejects reserved keys.
        trigger_metadata.insert(
            "__registered_block_height".parse().unwrap(),
            Json::from(42_u64),
        );
        trigger_metadata.insert(
            "__registered_at_ms".parse().unwrap(),
            Json::from(12_345_u64),
        );
        let schedule = Schedule {
            start_ms: scheduled_at_ms,
            period_ms: None,
        };
        let mut action = SpecializedAction::new(
            Executable::Ivm(bytecode.clone()),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        );
        action.metadata = trigger_metadata.clone();
        let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            tx.add_time_trigger(trigger).expect("add trigger");
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(subscriber.clone());
        #[cfg(feature = "telemetry")]
        let metrics = Arc::new(iroha_telemetry::metrics::Metrics::default());
        #[cfg(feature = "telemetry")]
        host.set_telemetry(StateTelemetry::new(Arc::clone(&metrics), true));
        host.set_query_state(&view);
        host.set_trigger_id(trigger_id.clone());
        host.set_block_time_ms(scheduled_at_ms);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
            .expect("billing");

        let mut expected_state = subscription_state.clone();
        expected_state.current_period_start_ms = scheduled_at_ms;
        expected_state.current_period_end_ms = scheduled_at_ms + period_ms;
        expected_state.next_charge_ms = scheduled_at_ms + period_ms;
        expected_state.failure_count = 0;
        expected_state.status = SubscriptionStatus::Active;

        let expected_transfer = InstructionBox::from(Transfer::asset_numeric(
            asset_id,
            amount.clone(),
            provider.clone(),
        ));
        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            subscription_key,
            Json::new(expected_state),
        ));
        let expected_invoice = SubscriptionInvoice {
            subscription_nft_id: nft_id.clone(),
            period_start_ms: scheduled_at_ms - period_ms,
            period_end_ms: scheduled_at_ms,
            attempted_at_ms: scheduled_at_ms,
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
            status: SubscriptionInvoiceStatus::Paid,
            tx_hash: None,
        };
        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
        let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            invoice_key,
            Json::new(expected_invoice),
        ));
        let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id.clone()));
        let expected_schedule = Schedule {
            start_ms: scheduled_at_ms + period_ms,
            period_ms: None,
        };
        let expected_action = iroha_data_model::trigger::action::Action::new(
            Executable::Ivm(bytecode),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(expected_schedule)),
        )
        .with_metadata({
            let mut metadata = trigger_metadata;
            let registered_height_key: Name = "__registered_block_height".parse().unwrap();
            metadata.remove(&registered_height_key);
            let registered_time_key: Name = "__registered_at_ms".parse().unwrap();
            metadata.remove(&registered_time_key);
            metadata
        });
        let expected_trigger = Trigger::new(trigger_id.clone(), expected_action);
        let expected_register = InstructionBox::from(Register::trigger(expected_trigger));

        assert_eq!(
            host.queued,
            vec![
                expected_transfer.clone(),
                expected_set.clone(),
                expected_invoice_set.clone(),
                expected_unregister.clone(),
                expected_register.clone()
            ]
        );
        let expected_gas = crate::gas::meter_instruction(&expected_transfer)
            .saturating_add(crate::gas::meter_instruction(&expected_set))
            .saturating_add(crate::gas::meter_instruction(&expected_invoice_set))
            .saturating_add(crate::gas::meter_instruction(&expected_unregister))
            .saturating_add(crate::gas::meter_instruction(&expected_register));
        assert_eq!(gas, expected_gas);

        #[cfg(feature = "telemetry")]
        {
            assert_eq!(
                metrics
                    .subscription_billing_attempts_total
                    .with_label_values(&["fixed"])
                    .get(),
                1
            );
            assert_eq!(
                metrics
                    .subscription_billing_outcomes_total
                    .with_label_values(&["fixed", "paid"])
                    .get(),
                1
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_record_usage_updates_metadata() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let plan_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("commerce", "universal").unwrap(),
            "usage_plan".parse().unwrap(),
        );
        let charge_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
        let unit_key: Name = "compute_ms".parse().unwrap();
        let trigger_id: TriggerId = "sub-usage".parse().unwrap();

        let plan = SubscriptionPlan {
            provider: provider.clone(),
            billing: SubscriptionBilling {
                cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                    period_ms: 1_000,
                }),
                bill_for: SubscriptionBillFor::PreviousPeriod,
                retry_backoff_ms: 100,
                max_failures: 3,
                grace_ms: 500,
            },
            pricing: SubscriptionPricing::Usage(SubscriptionUsagePricing {
                unit_price: Numeric::new(2_u32, 0),
                unit_key: unit_key.clone(),
                asset_definition: charge_asset_id.clone(),
            }),
        };

        let mut plan_def =
            AssetDefinition::new(plan_id.clone(), NumericSpec::integer()).build(&provider);
        let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
        plan_def.metadata.insert(plan_key, Json::new(plan));
        let charge_def =
            AssetDefinition::new(charge_asset_id, NumericSpec::integer()).build(&provider);

        let mut usage_accumulated = BTreeMap::new();
        usage_accumulated.insert(unit_key.clone(), Numeric::new(10_u32, 0));
        let subscription_state = SubscriptionState {
            plan_id,
            provider: provider.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: 0,
            current_period_end_ms: 1,
            next_charge_ms: 1,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 0,
            usage_accumulated,
            billing_trigger_id: trigger_id,
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let nft_id: NftId = "sub-usage$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let domains = vec![
            Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let world = World::with_assets(
            domains,
            accounts,
            [plan_def, charge_def],
            Vec::<Asset>::new(),
            [nft],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();

        let delta = SubscriptionUsageDelta {
            subscription_nft_id: nft_id.clone(),
            unit_key: unit_key.clone(),
            delta: Numeric::new(5_u32, 0),
        };
        let args = Json::new(delta);
        let mut host = CoreHostImpl::with_accounts_and_args(
            subscriber.clone(),
            Arc::new(vec![provider, subscriber]),
            args,
        );
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_RECORD_USAGE, &mut vm)
            .expect("usage record");

        let mut expected_state = subscription_state;
        expected_state
            .usage_accumulated
            .insert(unit_key, Numeric::new(15_u32, 0));
        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id,
            subscription_key,
            Json::new(expected_state),
        ));
        assert_eq!(host.queued, vec![expected_set.clone()]);
        assert_eq!(gas, crate::gas::meter_instruction(&expected_set));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_bill_failed_reschedules_and_records_invoice() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let plan_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
        let charge_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
        let period_ms = 1_000_u64;
        let scheduled_at_ms = 10_000_u64;
        let trigger_id: TriggerId = "sub-bill-fail".parse().unwrap();
        let amount = Numeric::new(120_u32, 0);
        let retry_backoff_ms = 500_u64;

        let plan = SubscriptionPlan {
            provider: provider.clone(),
            billing: SubscriptionBilling {
                cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                    period_ms,
                }),
                bill_for: SubscriptionBillFor::PreviousPeriod,
                retry_backoff_ms,
                max_failures: 3,
                grace_ms: 0,
            },
            pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                amount: amount.clone(),
                asset_definition: charge_asset_id.clone(),
            }),
        };

        let mut plan_def =
            AssetDefinition::new(plan_id.clone(), NumericSpec::integer()).build(&provider);
        let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
        plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
        let charge_def =
            AssetDefinition::new(charge_asset_id.clone(), NumericSpec::integer()).build(&provider);
        let asset_id = AssetId::of(charge_asset_id.clone(), subscriber.clone());
        let asset = Asset::new(asset_id.clone(), Numeric::new(50_u32, 0));

        let subscription_state = SubscriptionState {
            plan_id: plan_id.clone(),
            provider: provider.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: scheduled_at_ms - period_ms,
            current_period_end_ms: scheduled_at_ms,
            next_charge_ms: scheduled_at_ms,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 0,
            usage_accumulated: BTreeMap::new(),
            billing_trigger_id: trigger_id.clone(),
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let nft_id: NftId = "sub-fail$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let domains = vec![
            Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let world = World::with_assets(domains, accounts, [plan_def, charge_def], [asset], [nft]);

        let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
        let mut trigger_metadata = Metadata::default();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
        trigger_metadata.insert(
            trigger_ref_key,
            Json::new(SubscriptionTriggerRef {
                subscription_nft_id: nft_id.clone(),
            }),
        );
        let schedule = Schedule {
            start_ms: scheduled_at_ms,
            period_ms: None,
        };
        let mut action = SpecializedAction::new(
            Executable::Ivm(bytecode.clone()),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        );
        action.metadata = trigger_metadata.clone();
        let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            tx.add_time_trigger(trigger).expect("add trigger");
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(subscriber.clone());
        host.set_query_state(&view);
        host.set_trigger_id(trigger_id.clone());
        host.set_block_time_ms(scheduled_at_ms);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
            .expect("billing");

        let mut expected_state = subscription_state.clone();
        expected_state.failure_count = 1;
        expected_state.status = SubscriptionStatus::PastDue;
        expected_state.next_charge_ms = scheduled_at_ms + retry_backoff_ms;

        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            subscription_key,
            Json::new(expected_state),
        ));
        let expected_invoice = SubscriptionInvoice {
            subscription_nft_id: nft_id.clone(),
            period_start_ms: scheduled_at_ms - period_ms,
            period_end_ms: scheduled_at_ms,
            attempted_at_ms: scheduled_at_ms,
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
            status: SubscriptionInvoiceStatus::Failed,
            tx_hash: None,
        };
        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
        let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            invoice_key,
            Json::new(expected_invoice),
        ));
        let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id.clone()));
        let expected_schedule = Schedule {
            start_ms: scheduled_at_ms + retry_backoff_ms,
            period_ms: None,
        };
        let expected_action = iroha_data_model::trigger::action::Action::new(
            Executable::Ivm(bytecode),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(expected_schedule)),
        )
        .with_metadata(trigger_metadata);
        let expected_trigger = Trigger::new(trigger_id.clone(), expected_action);
        let expected_register = InstructionBox::from(Register::trigger(expected_trigger));

        assert_eq!(
            host.queued,
            vec![
                expected_set.clone(),
                expected_invoice_set.clone(),
                expected_unregister.clone(),
                expected_register.clone()
            ]
        );
        let expected_gas = crate::gas::meter_instruction(&expected_set)
            .saturating_add(crate::gas::meter_instruction(&expected_invoice_set))
            .saturating_add(crate::gas::meter_instruction(&expected_unregister))
            .saturating_add(crate::gas::meter_instruction(&expected_register));
        assert_eq!(gas, expected_gas);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_bill_suspends_after_max_failures() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let plan_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("commerce", "universal").unwrap(),
            "fixed_plan".parse().unwrap(),
        );
        let charge_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("pay", "universal").unwrap(),
            "usd".parse().unwrap(),
        );
        let period_ms = 1_000_u64;
        let scheduled_at_ms = 10_000_u64;
        let trigger_id: TriggerId = "sub-bill-suspend".parse().unwrap();
        let amount = Numeric::new(120_u32, 0);

        let plan = SubscriptionPlan {
            provider: provider.clone(),
            billing: SubscriptionBilling {
                cadence: SubscriptionCadence::FixedPeriod(SubscriptionFixedPeriodCadence {
                    period_ms,
                }),
                bill_for: SubscriptionBillFor::PreviousPeriod,
                retry_backoff_ms: 500,
                max_failures: 2,
                grace_ms: 0,
            },
            pricing: SubscriptionPricing::Fixed(SubscriptionFixedPricing {
                amount: amount.clone(),
                asset_definition: charge_asset_id.clone(),
            }),
        };

        let mut plan_def =
            AssetDefinition::new(plan_id.clone(), NumericSpec::integer()).build(&provider);
        let plan_key: Name = SUBSCRIPTION_PLAN_METADATA_KEY.parse().unwrap();
        plan_def.metadata.insert(plan_key, Json::new(plan.clone()));
        let charge_def =
            AssetDefinition::new(charge_asset_id.clone(), NumericSpec::integer()).build(&provider);

        let subscription_state = SubscriptionState {
            plan_id: plan_id.clone(),
            provider: provider.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: scheduled_at_ms - period_ms,
            current_period_end_ms: scheduled_at_ms,
            next_charge_ms: scheduled_at_ms,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 2,
            usage_accumulated: BTreeMap::new(),
            billing_trigger_id: trigger_id.clone(),
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let nft_id: NftId = "sub-suspend$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let domains = vec![
            Domain::new(DomainId::try_new("commerce", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("users", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("pay", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let world = World::with_assets(
            domains,
            accounts,
            [plan_def, charge_def],
            Vec::<Asset>::new(),
            [nft],
        );

        let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
        let mut trigger_metadata = Metadata::default();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
        trigger_metadata.insert(
            trigger_ref_key,
            Json::new(SubscriptionTriggerRef {
                subscription_nft_id: nft_id.clone(),
            }),
        );
        let schedule = Schedule {
            start_ms: scheduled_at_ms,
            period_ms: None,
        };
        let mut action = SpecializedAction::new(
            Executable::Ivm(bytecode.clone()),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        );
        action.metadata = trigger_metadata.clone();
        let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            tx.add_time_trigger(trigger).expect("add trigger");
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(subscriber.clone());
        host.set_query_state(&view);
        host.set_trigger_id(trigger_id.clone());
        host.set_block_time_ms(scheduled_at_ms);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
            .expect("billing");

        let mut expected_state = subscription_state.clone();
        expected_state.failure_count = 3;
        expected_state.status = SubscriptionStatus::Suspended;

        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            subscription_key,
            Json::new(expected_state),
        ));
        let expected_invoice = SubscriptionInvoice {
            subscription_nft_id: nft_id.clone(),
            period_start_ms: scheduled_at_ms - period_ms,
            period_end_ms: scheduled_at_ms,
            attempted_at_ms: scheduled_at_ms,
            amount: amount.clone(),
            asset_definition: charge_asset_id.clone(),
            status: SubscriptionInvoiceStatus::Failed,
            tx_hash: None,
        };
        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
        let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            invoice_key,
            Json::new(expected_invoice),
        ));
        let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id));

        assert_eq!(
            host.queued,
            vec![
                expected_set.clone(),
                expected_invoice_set.clone(),
                expected_unregister.clone(),
            ]
        );
        let expected_gas = crate::gas::meter_instruction(&expected_set)
            .saturating_add(crate::gas::meter_instruction(&expected_invoice_set))
            .saturating_add(crate::gas::meter_instruction(&expected_unregister));
        assert_eq!(gas, expected_gas);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let charge_asset_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let scheduled_at_ms = 10_000_u64;
        let trigger_id: TriggerId = "alias-renew".parse().unwrap();
        let alias = iroha_data_model::account::rekey::AccountAlias::domainless(
            "member".parse().unwrap(),
            DataSpaceId::UNIVERSAL,
        );
        let selector = crate::sns::selector_for_account_alias(
            &alias,
            &iroha_data_model::nexus::DataSpaceCatalog::default(),
        )
        .expect("selector");
        let alias_address = iroha_data_model::account::AccountAddress::from_account_id(&subscriber)
            .expect("account address");
        let alias_record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            subscriber.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(
                &alias_address,
            )],
            0,
            1,
            scheduled_at_ms,
            scheduled_at_ms + (30 * 86_400_000),
            scheduled_at_ms + (90 * 86_400_000),
            Metadata::default(),
        );

        let subscription_state = SubscriptionState {
            plan_id: charge_asset_id.clone(),
            provider: subscriber.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: 0,
            current_period_end_ms: scheduled_at_ms,
            next_charge_ms: scheduled_at_ms,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 0,
            usage_accumulated: BTreeMap::new(),
            billing_trigger_id: trigger_id.clone(),
        };
        let auto_renew = AccountAliasAutoRenewMetadata {
            alias: "member@universal".to_owned(),
            term_years: 1,
            max_charge_amount: Numeric::new(200_u32, 0),
            retry_backoff_ms: 500,
            max_failures: 3,
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let auto_renew_key: Name = ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY.parse().unwrap();
        nft_meta.insert(auto_renew_key, Json::new(auto_renew.clone()));
        let nft_id: NftId = "alias-renew$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let charge_def = AssetDefinition::numeric(charge_asset_id.clone())
            .with_name("xor".to_owned())
            .build(&provider);
        let asset = Asset::new(
            AssetId::of(charge_asset_id.clone(), subscriber.clone()),
            Numeric::new(500_u32, 0),
        );

        let domains = vec![
            Domain::new(DomainId::try_new("genesis", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let mut world = World::with_assets(domains, accounts, [charge_def], [asset], [nft]);
        crate::sns::seed_default_namespace_policies(&mut world);
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&alias_record),
        );

        let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
        let mut trigger_metadata = Metadata::default();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
        trigger_metadata.insert(
            trigger_ref_key,
            Json::new(SubscriptionTriggerRef {
                subscription_nft_id: nft_id.clone(),
            }),
        );
        trigger_metadata.insert(
            "__registered_block_height".parse().unwrap(),
            Json::from(42_u64),
        );
        trigger_metadata.insert(
            "__registered_at_ms".parse().unwrap(),
            Json::from(12_345_u64),
        );
        let schedule = Schedule {
            start_ms: scheduled_at_ms,
            period_ms: None,
        };
        let mut action = SpecializedAction::new(
            Executable::Ivm(bytecode.clone()),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        );
        action.metadata = trigger_metadata.clone();
        let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            tx.add_time_trigger(trigger).expect("add trigger");
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let quote = crate::sns::quote_account_alias_renewal(
            view.world(),
            &view.nexus.dataspace_catalog,
            &alias,
            auto_renew.term_years,
            scheduled_at_ms,
        )
        .expect("renewal quote");
        let mut host = CoreHostImpl::new(subscriber.clone());
        host.set_query_state(&view);
        host.set_trigger_id(trigger_id.clone());
        host.set_block_time_ms(scheduled_at_ms);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
            .expect("billing");

        let mut expected_state = subscription_state.clone();
        expected_state.current_period_start_ms = scheduled_at_ms;
        expected_state.current_period_end_ms = quote.expires_at_ms;
        expected_state.next_charge_ms = quote.expires_at_ms;
        expected_state.failure_count = 0;
        expected_state.status = SubscriptionStatus::Active;

        let expected_renew = InstructionBox::from(
            iroha_data_model::isi::account_alias_lease::RenewAccountAliasLease::new(
                alias.clone(),
                subscriber.clone(),
                auto_renew.term_years,
            ),
        );
        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            subscription_key,
            Json::new(expected_state),
        ));
        let expected_invoice = SubscriptionInvoice {
            subscription_nft_id: nft_id.clone(),
            period_start_ms: scheduled_at_ms,
            period_end_ms: quote.expires_at_ms,
            attempted_at_ms: scheduled_at_ms,
            amount: crate::sns::quote_charge_amount_to_numeric(quote.charge_amount),
            asset_definition: charge_asset_id.clone(),
            status: SubscriptionInvoiceStatus::Paid,
            tx_hash: None,
        };
        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
        let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            invoice_key,
            Json::new(expected_invoice),
        ));
        let expected_schedule = Schedule {
            start_ms: quote.expires_at_ms,
            period_ms: None,
        };
        let expected_action = iroha_data_model::trigger::action::Action::new(
            Executable::Ivm(bytecode),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(expected_schedule)),
        )
        .with_metadata({
            let mut metadata = trigger_metadata;
            let registered_height_key: Name = "__registered_block_height".parse().unwrap();
            metadata.remove(&registered_height_key);
            let registered_time_key: Name = "__registered_at_ms".parse().unwrap();
            metadata.remove(&registered_time_key);
            metadata
        });
        let expected_trigger = Trigger::new(trigger_id.clone(), expected_action);
        let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id.clone()));
        let expected_register = InstructionBox::from(Register::trigger(expected_trigger));

        assert_eq!(
            host.queued,
            vec![
                expected_renew.clone(),
                expected_set.clone(),
                expected_invoice_set.clone(),
                expected_unregister.clone(),
                expected_register.clone(),
            ]
        );
        let expected_gas = crate::gas::meter_instruction(&expected_renew)
            .saturating_add(crate::gas::meter_instruction(&expected_set))
            .saturating_add(crate::gas::meter_instruction(&expected_invoice_set))
            .saturating_add(crate::gas::meter_instruction(&expected_unregister))
            .saturating_add(crate::gas::meter_instruction(&expected_register));
        assert_eq!(gas, expected_gas);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn subscription_bill_account_alias_auto_renew_suspends_when_alias_is_missing() {
        crate::test_alias::ensure();
        let provider = fixture_account_in_domain("acme", "commerce");
        let subscriber = fixture_account_in_domain("alice", "users");
        let charge_asset_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let scheduled_at_ms = 10_000_u64;
        let trigger_id: TriggerId = "alias-renew-missing".parse().unwrap();

        let subscription_state = SubscriptionState {
            plan_id: charge_asset_id.clone(),
            provider: subscriber.clone(),
            subscriber: subscriber.clone(),
            status: SubscriptionStatus::Active,
            current_period_start_ms: 0,
            current_period_end_ms: scheduled_at_ms,
            next_charge_ms: scheduled_at_ms,
            cancel_at_period_end: false,
            cancel_at_ms: None,
            failure_count: 0,
            usage_accumulated: BTreeMap::new(),
            billing_trigger_id: trigger_id.clone(),
        };
        let auto_renew = AccountAliasAutoRenewMetadata {
            alias: "ghost@universal".to_owned(),
            term_years: 1,
            max_charge_amount: Numeric::new(200_u32, 0),
            retry_backoff_ms: 500,
            max_failures: 3,
        };
        let mut nft_meta = Metadata::default();
        let subscription_key: Name = SUBSCRIPTION_METADATA_KEY.parse().unwrap();
        nft_meta.insert(
            subscription_key.clone(),
            Json::new(subscription_state.clone()),
        );
        let auto_renew_key: Name = ACCOUNT_ALIAS_AUTO_RENEW_METADATA_KEY.parse().unwrap();
        nft_meta.insert(auto_renew_key, Json::new(auto_renew.clone()));
        let nft_id: NftId = "alias-missing$subscriptions.universal".parse().unwrap();
        let nft = Nft::new(nft_id.clone(), nft_meta).build(&subscriber);

        let charge_def = AssetDefinition::numeric(charge_asset_id.clone())
            .with_name("xor".to_owned())
            .build(&provider);
        let asset = Asset::new(
            AssetId::of(charge_asset_id.clone(), subscriber.clone()),
            Numeric::new(500_u32, 0),
        );

        let domains = vec![
            Domain::new(DomainId::try_new("genesis", "universal").unwrap()).build(&provider),
            Domain::new(DomainId::try_new("subscriptions", "universal").unwrap()).build(&provider),
        ];
        let accounts = vec![
            build_fixture_account(&provider, &provider),
            build_fixture_account(&subscriber, &provider),
        ];
        let mut world = World::with_assets(domains, accounts, [charge_def], [asset], [nft]);
        crate::sns::seed_default_namespace_policies(&mut world);

        let bytecode = IvmBytecode::from_compiled(ivm::ProgramMetadata::default().encode());
        let mut trigger_metadata = Metadata::default();
        let trigger_ref_key: Name = SUBSCRIPTION_TRIGGER_REF_METADATA_KEY.parse().unwrap();
        trigger_metadata.insert(
            trigger_ref_key,
            Json::new(SubscriptionTriggerRef {
                subscription_nft_id: nft_id.clone(),
            }),
        );
        let schedule = Schedule {
            start_ms: scheduled_at_ms,
            period_ms: None,
        };
        let mut action = SpecializedAction::new(
            Executable::Ivm(bytecode.clone()),
            Repeats::Exactly(1),
            subscriber.clone(),
            TimeEventFilter(ExecutionTime::Schedule(schedule)),
        );
        action.metadata = trigger_metadata.clone();
        let trigger = SpecializedTrigger::new(trigger_id.clone(), action);
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            tx.add_time_trigger(trigger).expect("add trigger");
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let mut host = CoreHostImpl::new(subscriber.clone());
        host.set_query_state(&view);
        host.set_trigger_id(trigger_id.clone());
        host.set_block_time_ms(scheduled_at_ms);
        let mut vm = IVM::new(1_000_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SUBSCRIPTION_BILL, &mut vm)
            .expect("billing");

        let mut expected_state = subscription_state.clone();
        expected_state.failure_count = auto_renew.max_failures;
        expected_state.status = SubscriptionStatus::Suspended;

        let expected_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            subscription_key,
            Json::new(expected_state),
        ));
        let expected_invoice = SubscriptionInvoice {
            subscription_nft_id: nft_id.clone(),
            period_start_ms: scheduled_at_ms,
            period_end_ms: scheduled_at_ms,
            attempted_at_ms: scheduled_at_ms,
            amount: Numeric::zero(),
            asset_definition: charge_asset_id,
            status: SubscriptionInvoiceStatus::Failed,
            tx_hash: None,
        };
        let invoice_key: Name = SUBSCRIPTION_INVOICE_METADATA_KEY.parse().unwrap();
        let expected_invoice_set = InstructionBox::from(SetKeyValue::nft(
            nft_id.clone(),
            invoice_key,
            Json::new(expected_invoice),
        ));
        let expected_unregister = InstructionBox::from(Unregister::trigger(trigger_id));

        assert_eq!(
            host.queued,
            vec![
                expected_set.clone(),
                expected_invoice_set.clone(),
                expected_unregister.clone(),
            ]
        );
        let expected_gas = crate::gas::meter_instruction(&expected_set)
            .saturating_add(crate::gas::meter_instruction(&expected_invoice_set))
            .saturating_add(crate::gas::meter_instruction(&expected_unregister));
        assert_eq!(gas, expected_gas);
    }

    #[test]
    fn queue_instructions_accumulates_gas_and_enqueues() {
        let authority = (*ALICE_ID).clone();
        let mut host = CoreHost::new(authority);
        let instr_one =
            InstructionBox::from(Log::new(iroha_logger::Level::INFO, "one".to_string()));
        let instr_two =
            InstructionBox::from(Log::new(iroha_logger::Level::WARN, "two".to_string()));
        let expected = crate::gas::meter_instruction(&instr_one)
            .saturating_add(crate::gas::meter_instruction(&instr_two));

        let gas = host.queue_instructions(vec![instr_one.clone(), instr_two.clone()]);

        assert_eq!(gas, expected);
        assert_eq!(host.queued, vec![instr_one, instr_two]);
    }

    #[test]
    fn create_nfts_for_all_noop_syscall_charges_declared_gas() {
        let authority = (*ALICE_ID).clone();
        let mut host = CoreHost::with_accounts(authority, Arc::new(Vec::new()));
        let mut vm = ivm::IVM::new(1_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
            .expect("create nfts for all");

        assert_eq!(gas, CoreHost::create_nfts_for_all_gas());
        assert!(host.queued.is_empty());
    }

    #[test]
    fn create_nfts_for_all_syscall_charges_at_least_declared_gas() {
        let authority = (*ALICE_ID).clone();
        let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority]));
        let mut vm = ivm::IVM::new(1_000);

        let gas = host
            .syscall(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS, &mut vm)
            .expect("create nfts for all");

        assert!(gas >= CoreHost::create_nfts_for_all_gas());
        assert_eq!(host.queued.len(), 2);
    }

    #[test]
    fn smartcontract_depth_noop_syscall_charges_declared_gas() {
        let authority = (*ALICE_ID).clone();
        let mut host = CoreHost::new(authority);
        let mut vm = ivm::IVM::new(1_000);
        vm.set_register(10, 0);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &mut vm)
            .expect("depth syscall");

        assert_eq!(gas, CoreHost::smartcontract_depth_gas());
        assert!(host.queued.is_empty());
    }

    #[test]
    fn smartcontract_depth_update_syscall_charges_at_least_declared_gas() {
        let authority = (*ALICE_ID).clone();
        let mut host = CoreHost::new(authority);
        let mut vm = ivm::IVM::new(1_000);
        vm.set_register(10, 3);

        let gas = host
            .syscall(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH, &mut vm)
            .expect("depth syscall");

        assert!(gas >= CoreHost::smartcontract_depth_gas());
        assert_eq!(host.queued.len(), 1);
    }

    #[test]
    fn fastpq_batch_entry_syscall_returns_transfer_gas() {
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(1_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load meta");

        host.syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm)
            .expect("begin batch");

        let from = authority.clone();
        let to: AccountId = fixture_account("bob");
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let amount = 7_u64;

        let from_payload = norito::to_bytes(&from).expect("encode from account");
        let to_payload = norito::to_bytes(&to).expect("encode to account");
        let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
        let from_tlv = make_tlv(PointerType::AccountId as u16, &from_payload);
        let to_tlv = make_tlv(PointerType::AccountId as u16, &to_payload);
        let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);
        let amount_payload = norito::to_bytes(&Numeric::from(amount)).expect("encode amount");
        let amount_tlv = make_tlv(PointerType::NoritoBytes as u16, &amount_payload);
        let amount_offset = 768u64;
        vm.memory.preload_input(0, &from_tlv).expect("preload from");
        vm.memory.preload_input(256, &to_tlv).expect("preload to");
        vm.memory
            .preload_input(512, &asset_tlv)
            .expect("preload asset");
        vm.memory
            .preload_input(amount_offset, &amount_tlv)
            .expect("preload amount");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.set_register(12, ivm::Memory::INPUT_START + 512);
        vm.set_register(13, ivm::Memory::INPUT_START + amount_offset);

        let gas = host
            .syscall(ivm_sys::SYSCALL_TRANSFER_ASSET, &mut vm)
            .expect("batch entry");
        let asset_id = AssetId::of(asset_def, from.clone());
        let isi = Transfer::asset_numeric(asset_id, amount, to);
        let expected = crate::gas::meter_instruction(&InstructionBox::from(TransferBox::from(isi)));
        assert_eq!(gas, expected);
        assert!(host.queued.is_empty());
        assert_eq!(host.fastpq_batch_entries.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn fastpq_batch_apply_syscall_returns_batch_gas() {
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(1_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load meta");

        let from = authority;
        let to: AccountId = fixture_account("bob");
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let entries = vec![
            TransferAssetBatchEntry::new(from.clone(), to.clone(), asset_def.clone(), 1_u64),
            TransferAssetBatchEntry::new(from.clone(), to.clone(), asset_def.clone(), 2_u64),
        ];
        let batch = TransferAssetBatch::new(entries);
        let payload = norito::to_bytes(&batch).expect("encode batch");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload batch");
        vm.set_register(10, ivm::Memory::INPUT_START);

        let gas = host
            .syscall(ivm_sys::SYSCALL_TRANSFER_V1_BATCH_APPLY, &mut vm)
            .expect("apply batch");
        let expected = InstructionBox::from(batch.clone());
        let expected_gas = crate::gas::meter_instruction(&expected);
        assert_eq!(gas, expected_gas);
        assert_eq!(host.queued, vec![expected]);
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

    pub(super) fn proof_blob_for(
        dsid: DataSpaceId,
        manifest_root: [u8; 32],
        proof_seed: &[u8],
        expiry_slot: u64,
    ) -> ProofBlob {
        let source_tx_commitment = test_digest(b"axt-host-test:source-tx", &[proof_seed]);
        let claim_digest = test_digest(b"axt-host-test:claim", &[proof_seed]);
        let witness_commitment = test_digest(b"axt-host-test:witness", &[proof_seed]);
        let policy_commitment = test_digest(b"axt-host-test:policy", &[&manifest_root[..]]);
        let binding = iroha_data_model::nexus::AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: dsid.as_u64(),
            source_dataspace: format!("test-dataspace-{}", dsid.as_u64()),
            source_receipt_id: format!("receipt-{}", hex::encode(source_tx_commitment.as_ref())),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: "test_effect".to_owned(),
            corridor: "test-corridor".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
        };

        let mut dsid_bytes = [0_u8; 16];
        dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
        let mut batch = fastpq_prover::TransitionBatch::new(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::PublicInputs {
                dsid: dsid_bytes,
                slot: expiry_slot,
                old_root: test_digest(b"axt-host-test:old-root", &[proof_seed]).into(),
                new_root: manifest_root,
                perm_root: test_digest(b"axt-host-test:perm-root", &[proof_seed]).into(),
                tx_set_hash: test_digest(b"axt-host-test:tx-set", &[proof_seed]).into(),
            },
        );
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/host/proof".to_vec(),
            proof_seed.to_vec(),
            manifest_root.to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
        batch.sort();
        batch.metadata.insert(
            "entry_hash".to_owned(),
            source_tx_commitment.as_ref().to_vec(),
        );
        fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT test batch");
        let proof = fastpq_prover::Prover::canonical(fastpq_prover::AXT_DEFAULT_PARAMETER)
            .expect("FASTPQ prover")
            .prove(&batch)
            .expect("FASTPQ proof");
        let fastpq_payload =
            fastpq_prover::encode_axt_fastpq_payload(&batch, proof).expect("AXT FASTPQ payload");
        let envelope = axt::AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: fastpq_payload,
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
            expiry_slot: Some(expiry_slot),
        }
    }

    fn test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
        let mut payload = Vec::new();
        payload.extend_from_slice(domain);
        for part in parts {
            payload.extend_from_slice(part);
        }
        iroha_crypto::Hash::new(payload)
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
    fn norito_blob_roundtrips_with_header_decoder() {
        let proof = ProofBlob {
            payload: vec![1, 2, 3],
            expiry_slot: Some(42),
        };
        let bytes = norito_blob(&proof);
        let decoded: ProofBlob =
            norito::decode_from_bytes(&bytes).expect("decode via header framing");
        assert_eq!(decoded, proof);
    }

    #[test]
    fn get_authority_allocates_in_input_bump() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load meta");

        host.syscall(ivm::syscalls::SYSCALL_GET_AUTHORITY, &mut vm)
            .expect("authority syscall");
        let authority_ptr = vm.register(10);

        let marker: Name = "marker".parse().unwrap();
        let _marker_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&marker));

        let decoded: AccountId =
            CoreHost::decode_tlv_typed(&vm, authority_ptr, PointerType::AccountId)
                .expect("authority TLV should remain intact");
        assert_eq!(decoded, authority);
    }

    #[test]
    fn current_time_syscall_uses_block_time() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        host.set_block_time_ms(1_717_171_717_000);
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load meta");

        assert_eq!(
            host.syscall(ivm::syscalls::SYSCALL_CURRENT_TIME_MS, &mut vm),
            Ok(CoreHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 1_717_171_717_000);
    }

    #[test]
    fn resolve_account_alias_syscall_reads_current_alias_binding() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let replacement_account_id: AccountId = fixture_account("carol");
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let replacement_account = Account::new(replacement_account_id.clone()).build(&authority);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with(
            [domain],
            [authority_account, merchant_account, replacement_account],
            [],
        );
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias_literal = "merchant@paynet";
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xA1);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account_id.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind alias");
        tx.apply();
        block.commit().expect("commit permissions");
        let first_view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&first_view);
        let mut vm = IVM::new(1_000_000);

        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect("resolve alias syscall");
        assert!(gas > 0);

        let resolved: AccountId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
                .expect("resolved account id");
        assert_eq!(resolved, merchant_account_id);

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xA2);
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account_id.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let rebound_view = state.view();
        host.set_query_state(&rebound_view);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect("resolve rebound alias syscall");
        assert!(gas > 0);
        let rebound: AccountId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
                .expect("rebound account id");
        assert_eq!(rebound, replacement_account_id);
    }

    #[test]
    fn resolve_account_alias_syscall_rejects_invalid_alias_literal_without_trimming() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with([domain], [authority_account], []);
        let state = State::new_for_testing(world, kura, query);
        let (_paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xA3);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.apply();
        block.commit().expect("commit catalog");

        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, b" merchant@paynet ");
        vm.set_register(10, alias_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect_err("invalid alias literal should fail at runtime");
        assert!(matches!(err, ivm::VMError::DecodeError));
    }

    #[test]
    fn resolve_account_alias_syscall_requires_authoritative_alias_binding() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with([domain], [authority_account, merchant_account], []);
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xA4);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing()
            .account_mut(&merchant_account_id)
            .expect("merchant account exists")
            .set_label(Some(alias));
        tx.apply();
        block.commit().expect("commit label-only alias state");

        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, b"merchant@paynet");
        vm.set_register(10, alias_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect_err("primary labels alone must not resolve without a binding");
        assert!(matches!(err, ivm::VMError::DecodeError));
    }

    #[test]
    fn resolve_account_alias_syscall_requires_dataspace_permission() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with([domain], [authority_account, merchant_account], []);
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias_literal = "merchant@paynet";
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB1);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account_id,
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect_err("alias resolution should require dataspace permission");
        assert!(matches!(err, ivm::VMError::PermissionDenied));
    }

    #[test]
    fn resolve_account_alias_syscall_reads_current_domain_qualified_binding() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let replacement_account_id: AccountId = fixture_account("carol");
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let replacement_account = Account::new(replacement_account_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_account, replacement_account],
            [payment_definition],
            [payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias_literal = "merchant@bank.paynet";
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        seed_test_call_hash(&mut tx, 0xF1);
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account_id.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
        tx.apply();
        block.commit().expect("commit permissions");
        let first_view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&first_view);
        let mut vm = IVM::new(1_000_000);

        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);

        host.syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect("resolve domain-qualified alias syscall");

        let resolved: AccountId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
                .expect("resolved account id");
        assert_eq!(resolved, merchant_account_id);

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account_id.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind domain-qualified alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let rebound_view = state.view();
        host.set_query_state(&rebound_view);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);
        host.syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect("resolve rebound domain-qualified alias syscall");
        let rebound: AccountId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
                .expect("rebound account id");
        assert_eq!(rebound, replacement_account_id);
    }

    #[test]
    fn resolve_account_alias_syscall_requires_domain_permission_for_domain_qualified_alias() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_account],
            [payment_definition],
            [payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias_literal = "merchant@bank.paynet";
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        seed_test_call_hash(&mut tx, 0xF2);
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account_id,
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect_err("domain-qualified alias resolution should require domain permission");
        assert!(matches!(err, ivm::VMError::PermissionDenied));
    }

    #[test]
    fn resolve_account_alias_syscall_requires_dataspace_permission_for_domain_qualified_alias() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account_id: AccountId = fixture_account("bob");
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let merchant_account = Account::new(merchant_account_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_account],
            [payment_definition],
            [payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(world, kura, query);
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;
        let alias_literal = "merchant@bank.paynet";
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        seed_test_call_hash(&mut tx, 0xF3);
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world_mut_for_testing().add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account_id,
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind domain-qualified alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let view = state.view();
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(1_000_000);
        let alias_ptr = store_tlv(&mut vm, PointerType::Blob, alias_literal.as_bytes());
        vm.set_register(10, alias_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
            .expect_err("domain-qualified alias resolution should require dataspace permission");
        assert!(matches!(err, ivm::VMError::PermissionDenied));
    }

    #[test]
    fn call_contract_syscall_returns_norito_encoded_int() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  kotoage fn main() -> int { return 0; }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Callee {
  kotoage fn value() -> int {
    return 42;
  }
}
"#,
            1,
        );

        let (result, vm, durable_state_overlay) = call_contract_syscall(
            &state,
            &authority,
            &caller_contract,
            &callee_contract,
            "value",
            Json::new(()),
        );
        let gas = result.expect("call_contract syscall should succeed");
        assert!(gas > 0);
        assert!(durable_state_overlay.is_empty());
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("returned NoritoBytes tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let expected_gas = ivm::gas::syscall_byte_gas(
            ivm::gas::G_CALL_CONTRACT,
            callee_contract.as_ref().as_bytes().len()
                + "value".len()
                + norito_blob(&Json::new(())).len(),
            tlv.payload.len(),
        );
        assert_eq!(gas, expected_gas);
        let value: i64 = norito::decode_from_bytes(tlv.payload).expect("decode int return");
        assert_eq!(value, 42);
    }

    #[test]
    fn call_contract_syscall_sets_nested_authority_to_caller_contract_subject() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  kotoage fn main() -> int { return 0; }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Callee {
  kotoage fn who_called() -> AccountId {
    return authority();
  }
}
"#,
            1,
        );

        let (result, vm, _) = call_contract_syscall(
            &state,
            &authority,
            &caller_contract,
            &callee_contract,
            "who_called",
            Json::new(()),
        );
        result.expect("call_contract syscall should succeed");
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("returned NoritoBytes tlv");
        let returned: AccountId =
            norito::decode_from_bytes(tlv.payload).expect("decode AccountId return");
        assert_eq!(returned, caller_contract.subject_id());
    }

    #[test]
    fn contract_view_of_persisted_account_state_returns_account_tlv() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let state = contract_test_state(&authority);
        let source = r#"
seiyaku StoredAccountView {
  meta { abi_version: 1; }

  state AccountId Stored;

  #[access(read="*", write="*")]
  kotoage fn bind(account_id: AccountId) {
    Stored = account_id;
  }

  view fn stored() -> AccountId {
    return Stored;
  }
}
"#;
        let contract = install_contract(&state, &authority, source, 0);
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload = Json::from_str_norito(&format!(r#"{{"account_id":"{}"}}"#, authority))
            .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let (code, _manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(source)
            .expect("compile source with manifest");
        let parsed = ivm::ProgramMetadata::parse(&code).expect("parse program metadata");
        let prefix_len = parsed.prefix_len() as u64;
        let descriptor = parsed
            .contract_interface
            .as_ref()
            .expect("contract interface")
            .entrypoints
            .iter()
            .find(|candidate| candidate.name == "stored")
            .expect("stored view descriptor");
        assert!(matches!(
            descriptor.kind,
            iroha_data_model::smart_contract::manifest::EntryPointKind::View
        ));
        let entry_pc = prefix_len + descriptor.entry_pc;

        let view = state.view();
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        host.set_durable_state_snapshot_from_world(view.world());
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: None,
            entrypoint: "stored".to_owned(),
        }));
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&code).expect("load program");
        vm.set_register(1, vm.memory.code_len());
        vm.set_program_counter(entry_pc).expect("seek stored view");
        vm.run_with_host(&mut host).expect("run stored view");

        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("stored view return tlv");
        assert_eq!(tlv.type_id, PointerType::AccountId);
        let returned: AccountId =
            norito::decode_from_bytes(tlv.payload).expect("decode stored account");
        assert_eq!(returned, authority);
    }

    #[test]
    fn call_contract_syscall_unit_return_survives_unaligned_requested_stack_limit() {
        struct StackLimitGuard {
            guest_limit: u64,
            budget_limit: u64,
        }

        impl Drop for StackLimitGuard {
            fn drop(&mut self) {
                ivm::set_guest_stack_limit(self.guest_limit);
                ivm::Memory::set_stack_budget_limit(self.budget_limit);
            }
        }

        crate::test_alias::ensure();
        let _stack_guard = StackLimitGuard {
            guest_limit: ivm::guest_stack_limit(),
            budget_limit: ivm::Memory::stack_budget_limit(),
        };
        ivm::set_guest_stack_limit(0x60a04);
        ivm::Memory::set_stack_budget_limit(0x60a04);

        let authority: AccountId = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  kotoage fn main() -> int { return 0; }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Callee {
  state int backlog;
  state int safe_mode;

  kotoage fn report(backlog_value: int, safe_mode_value: int) {
    backlog = backlog_value;
    safe_mode = safe_mode_value;
  }
}
"#,
            1,
        );

        let payload = Json::from(norito::json!({
            "backlog_value": 3,
            "safe_mode_value": 1
        }));
        let (result, vm, durable_state_overlay) = call_contract_syscall(
            &state,
            &authority,
            &caller_contract,
            &callee_contract,
            "report",
            payload,
        );

        result.expect("unit nested contract call should succeed");
        assert_eq!(vm.register(10), 0, "unit return should clear r10");
        assert!(
            durable_state_overlay
                .keys()
                .any(|key| key.as_ref().ends_with("/backlog")),
            "state overlay should include nested unit-return writes",
        );
        assert!(
            durable_state_overlay
                .keys()
                .any(|key| key.as_ref().ends_with("/safe_mode")),
            "state overlay should include all nested unit-return writes",
        );
    }

    #[test]
    fn call_contract_syscall_rolls_back_failed_nested_state_mutation() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let state = contract_test_state(&authority);
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  kotoage fn main() -> int { return 0; }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Callee {
  state int counter;

  kotoage fn fail_after_write() -> int {
    counter = 9;
    assert(false);
    return 0;
  }
}
"#,
            1,
        );

        let (result, _vm, durable_state_overlay) = call_contract_syscall(
            &state,
            &authority,
            &caller_contract,
            &callee_contract,
            "fail_after_write",
            Json::new(()),
        );
        assert!(result.is_err(), "nested contract call should fail");
        assert!(
            durable_state_overlay.is_empty(),
            "failed nested call must not leak contract-state writes",
        );
    }

    #[test]
    fn call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets([domain], [account], [asset_def], [source_asset], []);
        let state = State::new_for_testing(world, kura, query);
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  kotoage fn main() -> int { return 0; }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Callee {
  kotoage fn pull_into_vault(target: AccountId,
                             asset: AssetDefinitionId,
                             amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), target, asset, amount);
    return amount;
  }
}
"#,
            1,
        );

        let view = state.view();
        let caller_context = ContractRuntimeExecutionContext {
            contract_address: caller_contract.clone(),
            contract_subject: caller_contract.subject_id(),
            contract_alias: None,
            entrypoint: "open_position".to_owned(),
        };
        let mut host = CoreHostImpl::new(authority.clone());
        host.set_query_state(&view);
        host.set_durable_state_snapshot_from_world(view.world());
        host.set_contract_runtime_context(Some(caller_context.clone()));
        let mut vm = IVM::new(1_000_000);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load metadata-only program");

        let amount = Numeric::new(3_u32, 0);
        let from_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&authority));
        let to_ptr = store_tlv(
            &mut vm,
            PointerType::AccountId,
            &norito_blob(&caller_contract.subject_id()),
        );
        let asset_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_def_id),
        );
        let amount_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&amount).expect("encode amount"),
        );
        vm.set_register(10, from_ptr);
        vm.set_register(11, to_ptr);
        vm.set_register(12, asset_ptr);
        vm.set_register(13, amount_ptr);
        host.syscall(ivm_sys::SYSCALL_TRANSFER_ASSET, &mut vm)
            .expect("root transfer should enqueue");

        let nested_target = callee_contract.to_string();
        let nested_asset = asset_def_id.to_string();
        let nested_payload = Json::from(norito::json!({
            "target": nested_target,
            "asset": nested_asset,
            "amount": 3
        }));
        let target_ptr = store_tlv(
            &mut vm,
            PointerType::Blob,
            callee_contract.as_ref().as_bytes(),
        );
        let entrypoint_ptr = store_tlv(&mut vm, PointerType::Blob, b"pull_into_vault");
        let payload_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&nested_payload));
        vm.set_register(10, target_ptr);
        vm.set_register(11, entrypoint_ptr);
        vm.set_register(12, payload_ptr);
        host.syscall(ivm_sys::SYSCALL_CALL_CONTRACT, &mut vm)
            .expect("nested call should succeed");

        let artifacts = host
            .into_execution_artifacts(Some(caller_context))
            .expect("extract execution artifacts");

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut stx = block.transaction();
        artifacts
            .apply_to_transaction(&mut stx, &authority)
            .expect("root and nested transfers should apply");

        let authority_balance = stx
            .world
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let caller_asset_id = AssetId::of(asset_def_id.clone(), caller_contract.subject_id());
        let callee_asset_id = AssetId::of(asset_def_id, callee_contract.subject_id());
        let callee_balance = stx
            .world
            .asset(&callee_asset_id)
            .expect("callee contract asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            stx.world.asset(&caller_asset_id).is_err(),
            "fully drained caller balance should remove the asset entry",
        );
        assert_eq!(callee_balance.as_ref(), &Numeric::new(3_u32, 0));
    }

    #[test]
    fn contract_call_transaction_preserves_root_and_nested_transfer_authorities() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets([domain], [account], [asset_def], [source_asset], []);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  state AccountId CallerAccount;
  state bytes VaultContract;
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(caller_account: AccountId,
                  vault_contract: bytes,
                  settlement_asset: AssetDefinitionId) {
    CallerAccount = caller_account;
    VaultContract = vault_contract;
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn open(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), CallerAccount, SettlementAsset, amount);
    let payload = json_object();
    let payload = json_set_int(payload, name("amount"), amount);
    return decode_int(call_contract(VaultContract, "deposit", payload));
  }
}
"#,
            0,
        );
        let callee_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Vault {
  state AccountId VaultAccount;
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(vault_account: AccountId,
                  settlement_asset: AssetDefinitionId) {
    VaultAccount = vault_account;
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn deposit(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), VaultAccount, SettlementAsset, amount);
    return amount;
  }
}
"#,
            1,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let callee_bind_payload = Json::from_str_norito(&format!(
            r#"{{"vault_account":"{}","settlement_asset":"{}"}}"#,
            callee_contract.subject_id(),
            asset_def_id
        ))
        .expect("callee bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: callee_contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(callee_bind_payload),
            },
            &mut ivm_cache,
        );
        let caller_bind_payload = Json::from_str_norito(&format!(
            r#"{{"caller_account":"{}","vault_contract":"{}","settlement_asset":"{}"}}"#,
            caller_contract.subject_id(),
            format!("0x{}", hex::encode(callee_contract.as_ref())),
            asset_def_id
        ))
        .expect("caller bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: caller_contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(caller_bind_payload),
            },
            &mut ivm_cache,
        );
        let open_payload = Json::from_str_norito(r#"{"amount":3}"#).expect("open payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: caller_contract.clone(),
                entrypoint: "open".to_owned(),
                payload: Some(open_payload),
            },
            &mut ivm_cache,
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let caller_asset_id = AssetId::of(asset_def_id.clone(), caller_contract.subject_id());
        let callee_asset_id = AssetId::of(asset_def_id, callee_contract.subject_id());
        let callee_balance = view
            .world()
            .asset(&callee_asset_id)
            .expect("vault asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            view.world().asset(&caller_asset_id).is_err(),
            "fully drained caller balance should remove the asset entry",
        );
        assert_eq!(callee_balance.as_ref(), &Numeric::new(3_u32, 0));
    }

    #[test]
    fn contract_call_transaction_account_id_alias_shorthand_uses_current_binding() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let replacement_account = fixture_account("carol");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let replacement_fixture = build_fixture_account(&replacement_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account, merchant_fixture, replacement_fixture],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku AliasPayout {
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(settlement_asset: AssetDefinitionId) {
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn pay(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), account_id("merchant@paynet"), SettlementAsset, amount);
    return amount;
  }
}
"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );
        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let after_first = state.view();
        let merchant_asset_id = AssetId::of(asset_def_id.clone(), merchant_account.clone());
        let replacement_asset_id = AssetId::of(asset_def_id.clone(), replacement_account.clone());
        let merchant_balance = after_first
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset exists after first payout")
            .value()
            .clone();
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            after_first.world().asset(&replacement_asset_id).is_err(),
            "replacement account should not receive funds before the alias is rebound",
        );

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB2);
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind merchant alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let pay_payload = Json::from_str_norito(r#"{"amount":1}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let final_view = state.view();
        let authority_balance = final_view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_balance = final_view
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset remains")
            .value()
            .clone();
        let replacement_balance = final_view
            .world()
            .asset(&replacement_asset_id)
            .expect("replacement asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(replacement_balance.as_ref(), &Numeric::new(1_u32, 0));
    }

    #[test]
    fn contract_call_transaction_resolve_account_alias_builtin_uses_current_binding() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let replacement_account = fixture_account("carol");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let replacement_fixture = build_fixture_account(&replacement_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account, merchant_fixture, replacement_fixture],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB3);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku AliasPayout {
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(settlement_asset: AssetDefinitionId) {
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn pay(amount: int) -> int permission(AssetOps) {
    let merchant = resolve_account_alias("merchant@paynet");
    transfer_asset(authority(), merchant, SettlementAsset, amount);
    return amount;
  }
}
"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );
        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let after_first = state.view();
        let merchant_asset_id = AssetId::of(asset_def_id.clone(), merchant_account.clone());
        let replacement_asset_id = AssetId::of(asset_def_id.clone(), replacement_account.clone());
        let merchant_balance = after_first
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset exists after first payout")
            .value()
            .clone();
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            after_first.world().asset(&replacement_asset_id).is_err(),
            "replacement account should not receive funds before the alias is rebound",
        );

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind merchant alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let pay_payload = Json::from_str_norito(r#"{"amount":1}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let final_view = state.view();
        let authority_balance = final_view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_balance = final_view
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset remains")
            .value()
            .clone();
        let replacement_balance = final_view
            .world()
            .asset(&replacement_asset_id)
            .expect("replacement asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(replacement_balance.as_ref(), &Numeric::new(1_u32, 0));
    }

    #[test]
    fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_uses_current_binding()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let replacement_account = fixture_account("carol");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let replacement_fixture = build_fixture_account(&replacement_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture, replacement_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xB4);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF4; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );
        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let after_first = state.view();
        let merchant_asset_id = AssetId::of(asset_def_id.clone(), merchant_account.clone());
        let replacement_asset_id = AssetId::of(asset_def_id.clone(), replacement_account.clone());
        let merchant_balance = after_first
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset exists after first payout")
            .value()
            .clone();
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            after_first.world().asset(&replacement_asset_id).is_err(),
            "replacement account should not receive funds before the alias is rebound",
        );

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind merchant alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let pay_payload = Json::from_str_norito(r#"{"amount":1}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let final_view = state.view();
        let authority_balance = final_view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_balance = final_view
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset remains")
            .value()
            .clone();
        let replacement_balance = final_view
            .world()
            .asset(&replacement_asset_id)
            .expect("replacement asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(replacement_balance.as_ref(), &Numeric::new(1_u32, 0));
    }

    #[test]
    fn contract_call_transaction_resolve_account_alias_builtin_without_permission_is_rejected() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account, merchant_fixture],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::domainless("merchant".parse().expect("alias label"), paynet);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku AliasPayout {
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(settlement_asset: AssetDefinitionId) {
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn pay(amount: int) -> int permission(AssetOps) {
    let merchant = resolve_account_alias("merchant@paynet");
    transfer_asset(authority(), merchant, SettlementAsset, amount);
    return amount;
  }
}
"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            matches!(
                result,
                Err(iroha_data_model::ValidationFail::NotPermitted(_))
            ),
            "alias resolution without permission should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "failed alias resolution must not mint or transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_resolve_account_alias_builtin_without_binding_is_rejected() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account, merchant_fixture],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.apply();
        block.commit().expect("commit alias permissions");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "missing alias binding should reject builtin alias resolution"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "missing binding must not transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_resolve_account_alias_builtin_invalid_literal_is_rejected() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (_paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.apply();
        block.commit().expect("commit catalog");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "malformed builtin alias literal should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
    }

    #[test]
    fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_domain_permission_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD1);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF5; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            matches!(
                result,
                Err(iroha_data_model::ValidationFail::NotPermitted(_))
            ),
            "domain-qualified builtin alias resolution should require domain permission"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "failed alias resolution must not mint or transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_account_id_alias_shorthand_without_binding_is_rejected() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account, merchant_fixture],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.apply();
        block.commit().expect("commit alias permissions");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"account_id("merchant@paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "missing alias binding should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "missing binding must not transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_account_id_alias_shorthand_invalid_literal_is_rejected() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (_paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.apply();
        block.commit().expect("commit catalog");

        let contract =
            install_alias_payout_contract(&state, &authority, r#"account_id("merchant@")"#, 0);

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "malformed alias literal should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
    }

    #[test]
    fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_domain_permission_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD2);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF6; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"account_id("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            matches!(
                result,
                Err(iroha_data_model::ValidationFail::NotPermitted(_))
            ),
            "domain-qualified shorthand alias resolution should require domain permission"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "failed alias resolution must not mint or transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_dataspace_permission_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD3);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF7; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"account_id("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            matches!(
                result,
                Err(iroha_data_model::ValidationFail::NotPermitted(_))
            ),
            "domain-qualified shorthand alias resolution should require dataspace permission"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "failed alias resolution must not mint or transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_without_binding_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD4);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF8; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias,
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.apply();
        block.commit().expect("commit alias permissions");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"account_id("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "missing domain-qualified alias binding should reject shorthand alias resolution"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "missing binding must not transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_binding_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD5);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xF9; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias,
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.apply();
        block.commit().expect("commit alias permissions");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "missing domain-qualified alias binding should reject builtin alias resolution"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "missing binding must not transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_without_dataspace_permission_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD6);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xFA; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@bank.paynet")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            matches!(
                result,
                Err(iroha_data_model::ValidationFail::NotPermitted(_))
            ),
            "domain-qualified builtin alias resolution should require dataspace permission"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_asset_id = AssetId::of(asset_def_id, merchant_account);
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
        assert!(
            view.world().asset(&merchant_asset_id).is_err(),
            "failed alias resolution must not mint or transfer to the merchant account",
        );
    }

    #[test]
    fn contract_call_transaction_domain_qualified_resolve_account_alias_builtin_invalid_literal_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (_paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.apply();
        block.commit().expect("commit catalog");

        let contract = install_alias_payout_contract(
            &state,
            &authority,
            r#"resolve_account_alias("merchant@bank.")"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "malformed domain-qualified builtin alias literal should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
    }

    #[test]
    fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_invalid_literal_is_rejected()
     {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets(
            [domain],
            [authority_account],
            [asset_def],
            [source_asset],
            [],
        );
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (_paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.apply();
        block.commit().expect("commit catalog");

        let contract =
            install_alias_payout_contract(&state, &authority, r#"account_id("merchant@bank.")"#, 0);

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );

        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        let result = execute_contract_call_transaction_result(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );
        assert!(
            result.is_err(),
            "malformed domain-qualified shorthand alias literal should reject the transaction"
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(5_u32, 0));
    }

    #[test]
    fn contract_call_transaction_domain_qualified_account_id_alias_shorthand_uses_current_binding()
    {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let merchant_account = fixture_account("bob");
        let replacement_account = fixture_account("carol");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let retail_domain_id = DomainId::try_new("bank", "paynet").expect("retail domain id");
        let base_domain = Domain::new(fixture_domain_id()).build(&authority);
        let retail_domain = Domain::new(retail_domain_id.clone()).build(&authority);
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = build_fixture_account(&authority, &authority);
        let merchant_fixture = build_fixture_account(&merchant_account, &authority);
        let replacement_fixture = build_fixture_account(&replacement_account, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let payment_asset = Asset::new(
            AssetId::of(payment_asset_definition_id, authority.clone()),
            Numeric::new(1_000_u32, 0),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut world = World::with_assets(
            [base_domain, retail_domain, genesis_domain],
            [authority_account, merchant_fixture, replacement_fixture],
            [asset_def, payment_definition],
            [source_asset, payment_asset],
            [],
        );
        crate::sns::seed_default_namespace_policies(&mut world);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let (paynet, catalog) = retail_dataspace_catalog();
        state.nexus.write().dataspace_catalog = catalog;

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0xD7);
        tx.nexus.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        tx.world.dataspace_catalog = state.nexus.read().dataspace_catalog.clone();
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "bank".parse().expect("alias domain name"),
            )),
            paynet,
        );
        tx.tx_call_hash = Some(Hash::prehashed([0xFB; Hash::LENGTH]));
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease::new(
            alias.clone(),
            authority.clone(),
            authority.clone(),
            1,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("acquire domain-qualified alias lease");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanResolveAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(retail_domain_id.clone()),
            }),
        );
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            merchant_account.clone(),
            alias.clone(),
            None,
        )
        .execute(&authority, &mut tx)
        .expect("bind merchant alias");
        tx.apply();
        block.commit().expect("commit alias setup");

        let contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku AliasPayout {
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(settlement_asset: AssetDefinitionId) {
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn pay(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), account_id("merchant@bank.paynet"), SettlementAsset, amount);
    return amount;
  }
}
"#,
            0,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let bind_payload =
            Json::from_str_norito(&format!(r#"{{"settlement_asset":"{}"}}"#, asset_def_id))
                .expect("bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(bind_payload),
            },
            &mut ivm_cache,
        );
        let pay_payload = Json::from_str_norito(r#"{"amount":2}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract.clone(),
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let after_first = state.view();
        let merchant_asset_id = AssetId::of(asset_def_id.clone(), merchant_account.clone());
        let replacement_asset_id = AssetId::of(asset_def_id.clone(), replacement_account.clone());
        let merchant_balance = after_first
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset exists after first payout")
            .value()
            .clone();
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            after_first.world().asset(&replacement_asset_id).is_err(),
            "replacement account should not receive funds before the alias is rebound",
        );

        let next_height = u64::try_from(state.view().height() + 1)
            .ok()
            .and_then(core::num::NonZeroU64::new)
            .expect("next block height");
        let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
        let mut tx = block.transaction();
        iroha_data_model::isi::domain_link::SetAccountAliasBinding::bind(
            replacement_account.clone(),
            alias,
            None,
        )
        .execute(&authority, &mut tx)
        .expect("rebind merchant alias");
        tx.apply();
        block.commit().expect("commit alias rebind");

        let pay_payload = Json::from_str_norito(r#"{"amount":1}"#).expect("pay payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: contract,
                entrypoint: "pay".to_owned(),
                payload: Some(pay_payload),
            },
            &mut ivm_cache,
        );

        let final_view = state.view();
        let authority_balance = final_view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let merchant_balance = final_view
            .world()
            .asset(&merchant_asset_id)
            .expect("merchant asset remains")
            .value()
            .clone();
        let replacement_balance = final_view
            .world()
            .asset(&replacement_asset_id)
            .expect("replacement asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(merchant_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert_eq!(replacement_balance.as_ref(), &Numeric::new(1_u32, 0));
    }

    #[test]
    fn contract_call_transaction_preserves_three_hop_transfer_authorities() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let domain = Domain::new(fixture_domain_id()).build(&authority);
        let account = build_fixture_account(&authority, &authority);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let world = World::with_assets([domain], [account], [asset_def], [source_asset], []);
        let state = State::new_with_chain(world, kura, query, ChainId::from("test-chain"));
        let caller_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Caller {
  state AccountId CallerAccount;
  state bytes ForwarderContract;
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(caller_account: AccountId,
                  forwarder_contract: bytes,
                  settlement_asset: AssetDefinitionId) {
    CallerAccount = caller_account;
    ForwarderContract = forwarder_contract;
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn open(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), CallerAccount, SettlementAsset, amount);
    let payload = json_object();
    let payload = json_set_int(payload, name("amount"), amount);
    return decode_int(call_contract(ForwarderContract, "forward", payload));
  }
}
"#,
            0,
        );
        let forwarder_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Forwarder {
  state AccountId ForwarderAccount;
  state bytes VaultContract;
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(forwarder_account: AccountId,
                  vault_contract: bytes,
                  settlement_asset: AssetDefinitionId) {
    ForwarderAccount = forwarder_account;
    VaultContract = vault_contract;
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn forward(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), ForwarderAccount, SettlementAsset, amount);
    let payload = json_object();
    let payload = json_set_int(payload, name("amount"), amount);
    return decode_int(call_contract(VaultContract, "deposit", payload));
  }
}
"#,
            1,
        );
        let vault_contract = install_contract(
            &state,
            &authority,
            r#"
seiyaku Vault {
  state AccountId VaultAccount;
  state AssetDefinitionId SettlementAsset;

  #[access(read="*", write="*")]
  kotoage fn bind(vault_account: AccountId,
                  settlement_asset: AssetDefinitionId) {
    VaultAccount = vault_account;
    SettlementAsset = settlement_asset;
  }

  #[access(read="*", write="*")]
  kotoage fn deposit(amount: int) -> int permission(AssetOps) {
    transfer_asset(authority(), VaultAccount, SettlementAsset, amount);
    return amount;
  }
}
"#,
            2,
        );

        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let vault_bind_payload = Json::from_str_norito(&format!(
            r#"{{"vault_account":"{}","settlement_asset":"{}"}}"#,
            vault_contract.subject_id(),
            asset_def_id
        ))
        .expect("vault bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: vault_contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(vault_bind_payload),
            },
            &mut ivm_cache,
        );
        let forwarder_bind_payload = Json::from_str_norito(&format!(
            r#"{{"forwarder_account":"{}","vault_contract":"{}","settlement_asset":"{}"}}"#,
            forwarder_contract.subject_id(),
            format!("0x{}", hex::encode(vault_contract.as_ref())),
            asset_def_id
        ))
        .expect("forwarder bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: forwarder_contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(forwarder_bind_payload),
            },
            &mut ivm_cache,
        );
        let caller_bind_payload = Json::from_str_norito(&format!(
            r#"{{"caller_account":"{}","forwarder_contract":"{}","settlement_asset":"{}"}}"#,
            caller_contract.subject_id(),
            format!("0x{}", hex::encode(forwarder_contract.as_ref())),
            asset_def_id
        ))
        .expect("caller bind payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: caller_contract.clone(),
                entrypoint: "bind".to_owned(),
                payload: Some(caller_bind_payload),
            },
            &mut ivm_cache,
        );
        let open_payload = Json::from_str_norito(r#"{"amount":3}"#).expect("open payload");
        execute_contract_call_transaction(
            &state,
            &authority,
            &fixture_signing_keypair(&authority),
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: caller_contract.clone(),
                entrypoint: "open".to_owned(),
                payload: Some(open_payload),
            },
            &mut ivm_cache,
        );

        let view = state.view();
        let authority_balance = view
            .world()
            .asset(&source_asset_id)
            .expect("authority asset remains")
            .value()
            .clone();
        let caller_asset_id = AssetId::of(asset_def_id.clone(), caller_contract.subject_id());
        let forwarder_asset_id = AssetId::of(asset_def_id.clone(), forwarder_contract.subject_id());
        let vault_asset_id = AssetId::of(asset_def_id, vault_contract.subject_id());
        let vault_balance = view
            .world()
            .asset(&vault_asset_id)
            .expect("vault asset exists")
            .value()
            .clone();
        assert_eq!(authority_balance.as_ref(), &Numeric::new(2_u32, 0));
        assert!(
            view.world().asset(&caller_asset_id).is_err(),
            "fully drained caller balance should remove the caller asset entry",
        );
        assert!(
            view.world().asset(&forwarder_asset_id).is_err(),
            "fully drained forwarder balance should remove the forwarder asset entry",
        );
        assert_eq!(vault_balance.as_ref(), &Numeric::new(3_u32, 0));
    }

    #[test]
    fn host_execution_artifacts_apply_queued_uses_queued_authority_snapshot() {
        crate::test_alias::ensure();
        let outer_authority = fixture_account("alice");
        let nested_authority = fixture_account("bob");
        let recipient = fixture_account("carol");
        let domain = Domain::new(fixture_domain_id()).build(&outer_authority);
        let outer_account = build_fixture_account(&outer_authority, &outer_authority);
        let nested_account = build_fixture_account(&nested_authority, &outer_authority);
        let recipient_account = build_fixture_account(&recipient, &outer_authority);
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(fixture_domain_id(), "rose".parse().expect("asset name"));
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&outer_authority);
        let source_asset_id = AssetId::of(asset_def_id.clone(), nested_authority.clone());
        let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(5_u32, 0));
        let world = World::with_assets(
            [domain],
            [outer_account, nested_account, recipient_account],
            [asset_def],
            [source_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let genesis_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        state
            .block(genesis_header)
            .commit()
            .expect("commit bootstrap block");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let callee_contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &outer_authority,
            99,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: callee_contract.subject_id(),
            contract_address: callee_contract,
            contract_alias: Some("risk::vault".parse().expect("contract alias")),
            entrypoint: "deposit".to_owned(),
        };
        let artifacts = HostExecutionArtifacts {
            queued: vec![QueuedInstruction {
                instruction: InstructionBox::from(Transfer::asset_numeric(
                    source_asset_id.clone(),
                    1_u32,
                    recipient.clone(),
                )),
                authority: nested_authority.clone(),
                contract_runtime_context: Some(context),
            }],
            confidential_gas_delta: 0,
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::new(),
        };

        artifacts
            .apply_to_transaction(&mut stx, &outer_authority)
            .expect("queued instruction should execute under queued authority");

        let source_balance = stx
            .world
            .asset(&source_asset_id)
            .expect("source asset remains")
            .value()
            .clone();
        let recipient_asset_id = AssetId::of(asset_def_id, recipient);
        let recipient_balance = stx
            .world
            .asset(&recipient_asset_id)
            .expect("recipient asset created")
            .value()
            .clone();
        assert_eq!(source_balance.as_ref(), &Numeric::new(4_u32, 0));
        assert_eq!(recipient_balance.as_ref(), &Numeric::new(1_u32, 0));
    }

    pub(super) fn begin_axt_envelope(
        host: &mut CoreHost,
        vm: &mut IVM,
        descriptor: &axt::AxtDescriptor,
    ) {
        let descriptor_bytes = norito_blob(descriptor);
        let desc_ptr = store_tlv(vm, PointerType::AxtDescriptor, &descriptor_bytes);
        vm.set_register(10, desc_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_AXT_BEGIN, vm),
            Ok(CoreHost::axt_gas(descriptor_bytes.len())),
            "AXT_BEGIN should seed envelope state"
        );
    }

    #[test]
    fn backend_label_prefers_stored_vk_backend_and_prefix() {
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
        // When no stored VK bytes are present, use the circuit_id prefix before ':'
        assert_eq!(
            CoreHost::backend_label_for_record(&rec),
            "halo2/ipa".to_string()
        );
        // Stored VK backend must override the circuit prefix.
        rec.key = Some(VerifyingKeyBox::new("custom/backend".into(), vec![1, 2, 3]));
        assert_eq!(
            CoreHost::backend_label_for_record(&rec),
            "custom/backend".to_string()
        );
    }

    #[test]
    fn state_syscall_works_without_access_logging() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value_bytes);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);

        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
            "STATE_SET should succeed without access logging"
        );
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
            "STATE_GET should succeed without access logging"
        );
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("state get tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, 1);
    }

    #[test]
    fn encode_decode_int_syscalls_roundtrip() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);

        vm.set_register(10, 42);
        let encode_gas = host
            .syscall(ivm_sys::SYSCALL_ENCODE_INT, &mut vm)
            .expect("encode int");
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr).expect("encode tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let encoded_len = tlv.payload.len();
        assert_eq!(encode_gas, 16 + encoded_len as u64);
        let encoded: i64 = norito::decode_from_bytes(tlv.payload).expect("decode int payload");
        assert_eq!(encoded, 42);

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_DECODE_INT, &mut vm),
            Ok(16 + encoded_len as u64)
        );
        assert_eq!(vm.register(10), 42);
    }

    #[test]
    fn numeric_helper_syscalls_roundtrip_through_codec_host() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);

        vm.set_register(10, 42);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_NUMERIC_FROM_INT, &mut vm),
            Ok(16)
        );
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr).expect("numeric tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let encoded: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode numeric");
        assert_eq!(encoded, Numeric::from(42_u32));

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_NUMERIC_TO_INT, &mut vm),
            Ok(16)
        );
        assert_eq!(vm.register(10), 42);
    }

    #[test]
    fn state_syscall_logs_access_when_enabled() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority).with_access_logging();
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value_bytes);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);

        host.begin_tx(&ivm::parallel::StateAccessSet::default())
            .expect("begin_tx should reset access log");
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
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
        let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
        world
            .smart_contract_state
            .insert(path.clone(), value_bytes.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
            "STATE_GET should read from the world snapshot"
        );
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("snapshot tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, 1);
    }

    #[test]
    fn state_syscall_reads_live_query_state_without_eager_snapshot() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        let value_bytes = norito::to_bytes(&7_u64).expect("encode state value");
        world
            .smart_contract_state
            .insert(path.clone(), value_bytes.clone());
        world.smart_contract_state.insert(
            "unrelated/key".parse().unwrap(),
            norito::to_bytes(&9_u64).expect("encode unrelated value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHostImpl::new(authority);
        host.set_query_state(&view);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
            "STATE_GET should read from live query state without cloning the durable map"
        );
        assert!(
            host.durable_state_base.is_empty(),
            "live contract-call hosts should not eagerly clone smart_contract_state"
        );
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("live state tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, 7);

        let prefix: Name = "counter".parse().unwrap();
        let prefix_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&prefix));
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 0);
        host.syscall(ivm_sys::SYSCALL_STATE_KEYS, &mut vm)
            .expect("STATE_KEYS should read live query-state keys");
        assert_eq!(vm.register(11), 1);
        assert_eq!(vm.register(12), 1);
    }

    #[test]
    fn state_syscall_reads_world_snapshot_spills_to_heap_when_input_fills() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        let expected = vec![0xA5; 96];
        let value_bytes = norito::to_bytes(&expected).expect("encode state value");
        world.smart_contract_state.insert(path.clone(), value_bytes);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let filler = make_tlv(PointerType::Blob as u16, b"");
        while vm.alloc_input_tlv(&filler).is_ok() {}

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&expected)
                    .expect("encode state value")
                    .len()
            )),
            "STATE_GET should spill to heap when input space is exhausted"
        );
        let out_ptr = vm.register(10);
        assert!(
            (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
            "contract runtime state_get should return a heap pointer once input is full"
        );
        let tlv = vm.validate_tlv(out_ptr).expect("spilled snapshot tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, expected);
    }

    #[test]
    fn state_syscall_unscoped_overlay_overrides_base_value_without_context() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        world.smart_contract_state.insert(
            path.clone(),
            norito::to_bytes(&7_u64).expect("encode base state value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let value_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&22_u64).expect("encode overlay state value"),
        );

        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode overlay state value")
                    .len()
            )),
            "STATE_SET should stage an unscoped overlay value without runtime context"
        );

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode overlay state value")
                    .len()
            )),
            "unscoped overlay should override the persisted base value"
        );
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("overlay state tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode overlay state");
        assert_eq!(value, 22);

        let overlay = host.drain_durable_state_overlay();
        let stored = overlay
            .get(&path)
            .and_then(Option::as_ref)
            .expect("unscoped overlay entry");
        let stored_tlv =
            ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored unscoped overlay tlv");
        assert_eq!(stored_tlv.type_id, PointerType::NoritoBytes);
        let stored_value: u64 =
            norito::decode_from_bytes(stored_tlv.payload).expect("decode stored overlay state");
        assert_eq!(stored_value, 22);
    }

    #[test]
    fn state_syscall_unscoped_delete_shadows_base_value_without_context() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        world.smart_contract_state.insert(
            path.clone(),
            norito::to_bytes(&7_u64).expect("encode base state value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm),
            Ok(16),
            "STATE_DEL should stage an unscoped tombstone without runtime context"
        );

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(16),
            "unscoped tombstone should shadow the persisted base value"
        );
        assert_eq!(vm.register(10), 0);

        let overlay = host.drain_durable_state_overlay();
        assert_eq!(
            overlay.len(),
            1,
            "unscoped delete should only record one tombstone"
        );
        assert_eq!(overlay.get(&path), Some(&None));
    }

    #[test]
    fn state_syscall_reads_scoped_overlay_spills_to_heap_when_input_fills() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let expected = vec![0x3C; 80];
        let value_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&expected).expect("encode state value"),
        );
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            44,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("spill::overlay".parse().expect("contract alias")),
            entrypoint: "read".to_owned(),
        }));

        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&expected)
                    .expect("encode state value")
                    .len()
            ))
        );

        let filler = make_tlv(PointerType::Blob as u16, b"");
        while vm.alloc_input_tlv(&filler).is_ok() {}

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&expected)
                    .expect("encode state value")
                    .len()
            )),
            "scoped overlay STATE_GET should spill to heap when input space is exhausted"
        );
        let out_ptr = vm.register(10);
        assert!(
            (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
            "scoped overlay state_get should return a heap pointer once input is full"
        );
        let tlv = vm.validate_tlv(out_ptr).expect("spilled overlay tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, expected);
    }

    #[test]
    fn state_syscall_prefers_scoped_base_value_over_legacy_fallback() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let path: Name = "counter".parse().unwrap();
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            46,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("scoped::base".parse().expect("contract alias")),
            entrypoint: "read".to_owned(),
        };
        let mut scope_host = CoreHost::new(authority.clone());
        scope_host.set_contract_runtime_context(Some(context.clone()));
        let scoped_path = scope_host
            .scoped_durable_state_path(&path)
            .expect("build scoped path")
            .expect("scoped path should exist");

        let mut world = World::new();
        world.smart_contract_state.insert(
            path.clone(),
            norito::to_bytes(&7_u64).expect("encode legacy state value"),
        );
        world.smart_contract_state.insert(
            scoped_path,
            norito::to_bytes(&11_u64).expect("encode scoped state value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let mut host = CoreHost::from_state(authority, &state);
        host.set_contract_runtime_context(Some(context));

        let mut vm = IVM::new(10_000);
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let filler = make_tlv(PointerType::Blob as u16, b"");
        while vm.alloc_input_tlv(&filler).is_ok() {}

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&11_u64)
                    .expect("encode scoped state value")
                    .len()
            )),
            "STATE_GET should prefer the scoped persisted value over the legacy fallback"
        );
        let out_ptr = vm.register(10);
        assert!(
            (ivm::Memory::HEAP_START..ivm::Memory::INPUT_START).contains(&out_ptr),
            "scoped base STATE_GET should spill to heap once input is exhausted"
        );
        let tlv = vm.validate_tlv(out_ptr).expect("scoped base tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode scoped state");
        assert_eq!(value, 11);
    }

    #[test]
    fn state_keys_syscall_strips_scope_and_applies_tombstones() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            146,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("state::keys".parse().expect("contract alias")),
            entrypoint: "list".to_owned(),
        };
        let scoped_key: Name = "orders/2".parse().unwrap();
        let mut scope_host = CoreHost::new(authority.clone());
        scope_host.set_contract_runtime_context(Some(context.clone()));
        let scoped_path = scope_host
            .scoped_durable_state_path(&scoped_key)
            .expect("build scoped path")
            .expect("scoped path should exist");

        let mut world = World::new();
        world.smart_contract_state.insert(
            "orders/1".parse().unwrap(),
            norito::to_bytes(&1_u64).expect("encode legacy state value"),
        );
        world.smart_contract_state.insert(
            scoped_path,
            norito::to_bytes(&2_u64).expect("encode scoped state value"),
        );
        world.smart_contract_state.insert(
            "accounts/1".parse().unwrap(),
            norito::to_bytes(&3_u64).expect("encode unrelated state value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let mut host = CoreHost::from_state(authority, &state);
        host.set_contract_runtime_context(Some(context));
        let mut vm = IVM::new(10_000);

        let legacy_key: Name = "orders/1".parse().unwrap();
        let legacy_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&legacy_key));
        vm.set_register(10, legacy_ptr);
        assert_eq!(host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm), Ok(16));

        let prefix: Name = "orders".parse().unwrap();
        let prefix_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&prefix));
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 0);
        let gas = host
            .syscall(ivm_sys::SYSCALL_STATE_KEYS, &mut vm)
            .expect("STATE_KEYS should list scoped keys");

        assert!(gas > 0);
        assert_eq!(vm.register(11), 1);
        assert_eq!(vm.register(12), 1);
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("state keys tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let keys: Vec<Name> = norito::decode_from_bytes(tlv.payload).expect("decode key list");
        assert_eq!(keys, vec![scoped_key.clone()]);

        vm.set_register(10, prefix_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_COUNT, &mut vm),
            Ok(CoreHost::state_count_gas(1))
        );
        assert_eq!(vm.register(10), 1);

        let scoped_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&scoped_key));
        vm.set_register(10, scoped_ptr);
        assert_eq!(host.syscall(ivm_sys::SYSCALL_STATE_HAS, &mut vm), Ok(16));
        assert_eq!(vm.register(10), 1);

        vm.set_register(10, scoped_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_LEN, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&2_u64)
                    .expect("encode scoped state value")
                    .len()
            ))
        );
        assert_eq!(
            vm.register(10),
            u64::try_from(
                norito::to_bytes(&2_u64)
                    .expect("encode scoped state value")
                    .len()
            )
            .expect("length fits")
        );
        assert_eq!(vm.register(11), 1);
    }

    #[test]
    fn state_syscall_scoped_overlay_overrides_scoped_and_legacy_base_values() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let path: Name = "counter".parse().unwrap();
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            47,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("scoped::overlay".parse().expect("contract alias")),
            entrypoint: "write".to_owned(),
        };
        let mut scope_host = CoreHost::new(authority.clone());
        scope_host.set_contract_runtime_context(Some(context.clone()));
        let scoped_path = scope_host
            .scoped_durable_state_path(&path)
            .expect("build scoped path")
            .expect("scoped path should exist");

        let mut world = World::new();
        world.smart_contract_state.insert(
            path.clone(),
            norito::to_bytes(&7_u64).expect("encode legacy state value"),
        );
        world.smart_contract_state.insert(
            scoped_path.clone(),
            norito::to_bytes(&11_u64).expect("encode scoped state value"),
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let mut host = CoreHost::from_state(authority, &state);
        host.set_contract_runtime_context(Some(context));

        let mut vm = IVM::new(10_000);
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let value_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&22_u64).expect("encode overlay state value"),
        );

        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode overlay state value")
                    .len()
            ))
        );

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode overlay state value")
                    .len()
            )),
            "scoped overlay should win over both scoped and legacy persisted state"
        );
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("overlay state tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode overlay state");
        assert_eq!(value, 22);

        let overlay = host.drain_durable_state_overlay();
        assert!(
            !overlay.contains_key(&path),
            "scoped overlay should not write through the legacy unscoped path"
        );
        let stored = overlay
            .get(&scoped_path)
            .and_then(Option::as_ref)
            .expect("scoped overlay entry");
        let stored_tlv =
            ivm::pointer_abi::validate_tlv_bytes(stored).expect("stored scoped overlay tlv");
        assert_eq!(stored_tlv.type_id, PointerType::NoritoBytes);
        let stored_value: u64 =
            norito::decode_from_bytes(stored_tlv.payload).expect("decode stored overlay state");
        assert_eq!(stored_value, 22);
    }

    #[test]
    fn state_syscall_scoped_delete_shadows_legacy_raw_base_value() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        let value_bytes = norito::to_bytes(&7_u64).expect("encode state value");
        world.smart_contract_state.insert(path.clone(), value_bytes);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority.clone(), &state);
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            45,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("legacy::shadow".parse().expect("contract alias")),
            entrypoint: "write".to_owned(),
        }));

        let mut vm = IVM::new(10_000);
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        vm.set_register(10, path_ptr);
        assert_eq!(host.syscall(ivm_sys::SYSCALL_STATE_DEL, &mut vm), Ok(16));

        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(16),
            "scoped tombstone should shadow legacy unscoped base value"
        );
        assert_eq!(vm.register(10), 0);

        let overlay = host.drain_durable_state_overlay();
        assert_eq!(overlay.get(&path), Some(&None));
    }

    #[test]
    fn state_syscall_isolates_paths_by_contract_runtime_context() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let mut vm = IVM::new(10_000);

        let path: Name = "counter".parse().unwrap();
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));

        let contract_a = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            41,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract A");
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract_a.subject_id(),
            contract_address: contract_a.clone(),
            contract_alias: Some("fixture::shared".parse().expect("contract alias A")),
            entrypoint: "write".to_owned(),
        }));
        let value_a_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&11_u64).expect("encode state value A"),
        );
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_a_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&11_u64)
                    .expect("encode state value A")
                    .len()
            ))
        );

        let contract_b = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            42,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract B");
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract_b.subject_id(),
            contract_address: contract_b.clone(),
            contract_alias: Some("fixture::shared".parse().expect("contract alias B")),
            entrypoint: "write".to_owned(),
        }));
        let value_b_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito::to_bytes(&22_u64).expect("encode state value B"),
        );
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_b_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode state value B")
                    .len()
            ))
        );

        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract_a.subject_id(),
            contract_address: contract_a,
            contract_alias: Some("fixture::shared".parse().expect("contract alias A read")),
            entrypoint: "read".to_owned(),
        }));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&11_u64)
                    .expect("encode state value A")
                    .len()
            ))
        );
        let value_a_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("state get tlv A");
        let value_a: u64 =
            norito::decode_from_bytes(value_a_tlv.payload).expect("decode state value A");
        assert_eq!(value_a, 11);

        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract_b.subject_id(),
            contract_address: contract_b,
            contract_alias: Some("fixture::shared".parse().expect("contract alias B read")),
            entrypoint: "read".to_owned(),
        }));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(
                norito::to_bytes(&22_u64)
                    .expect("encode state value B")
                    .len()
            ))
        );
        let value_b_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("state get tlv B");
        let value_b: u64 =
            norito::decode_from_bytes(value_b_tlv.payload).expect("decode state value B");
        assert_eq!(value_b, 22);
    }

    #[test]
    fn state_syscall_reads_legacy_raw_key_when_scoped_key_is_missing() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let path: Name = "counter".parse().unwrap();
        let value_bytes = norito::to_bytes(&7_u64).expect("encode state value");
        world
            .smart_contract_state
            .insert(path.clone(), value_bytes.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority.clone(), &state);
        let contract = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            43,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract");
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: Some("legacy::reader".parse().expect("contract alias")),
            entrypoint: "read".to_owned(),
        }));
        let mut vm = IVM::new(10_000);
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        vm.set_register(10, path_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len()))
        );
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("legacy state get tlv");
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode legacy state");
        assert_eq!(value, 7);
    }

    #[test]
    fn vrf_epoch_seed_syscall_reads_world_snapshot() {
        crate::test_alias::ensure();
        let mut world = World::new();
        world.vrf_epochs.insert(
            7,
            iroha_data_model::consensus::VrfEpochRecord {
                epoch: 7,
                seed: [0x11; 32],
                epoch_length: 5,
                commit_deadline_offset: 1,
                reveal_deadline_offset: 2,
                roster_len: 0,
                finalized: true,
                updated_at_height: 100,
                participants: Vec::new(),
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            },
        );
        world.vrf_epochs.insert(
            9,
            iroha_data_model::consensus::VrfEpochRecord {
                epoch: 9,
                seed: [0x22; 32],
                epoch_length: 5,
                commit_deadline_offset: 1,
                reveal_deadline_offset: 2,
                roster_len: 0,
                finalized: true,
                updated_at_height: 110,
                participants: Vec::new(),
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            },
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let req = ivm::vrf::VrfEpochSeedRequest {
            epoch: 7,
            fallback_to_latest: false,
        };
        let req_bytes = norito_blob(&req);
        let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
        vm.set_register(10, req_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &mut vm)
            .expect("vrf epoch seed");
        assert_eq!(vm.register(11), 0);
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(
            gas,
            CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
        );
        let resp: ivm::vrf::VrfEpochSeedResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode response");
        assert!(resp.found);
        assert_eq!(resp.epoch, 7);
        assert_eq!(resp.seed, [0x11; 32]);

        let fallback_req = ivm::vrf::VrfEpochSeedRequest {
            epoch: 99,
            fallback_to_latest: true,
        };
        let fallback_bytes = norito_blob(&fallback_req);
        let fallback_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &fallback_bytes);
        vm.set_register(10, fallback_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &mut vm)
            .expect("vrf epoch seed fallback");
        assert_eq!(vm.register(11), 0);
        let out_ptr = vm.register(10);
        let tlv = vm
            .memory
            .validate_tlv(out_ptr)
            .expect("fallback output tlv");
        assert_eq!(
            gas,
            CoreHost::state_query_gas(fallback_bytes.len().saturating_add(tlv.payload.len()))
        );
        let resp: ivm::vrf::VrfEpochSeedResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode fallback response");
        assert!(resp.found);
        assert_eq!(resp.epoch, 9);
        assert_eq!(resp.seed, [0x22; 32]);
    }

    #[test]
    fn vrf_epoch_seed_syscall_reports_missing_without_fallback() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);
        let mut vm = IVM::new(10_000);

        let req = ivm::vrf::VrfEpochSeedRequest {
            epoch: 42,
            fallback_to_latest: false,
        };
        let req_bytes = norito_blob(&req);
        let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
        vm.set_register(10, req_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_VRF_EPOCH_SEED, &mut vm)
            .expect("vrf epoch seed missing");
        assert_eq!(vm.register(11), 0);
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(
            gas,
            CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
        );
        let resp: ivm::vrf::VrfEpochSeedResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode response");
        assert!(!resp.found);
        assert_eq!(resp.epoch, 42);
        assert_eq!(resp.seed, [0u8; 32]);
    }

    #[test]
    fn zk_vote_tally_syscall_reads_world_snapshot() {
        crate::test_alias::ensure();
        let mut world = World::new();
        let mut election = ElectionState::default();
        election.finalized = true;
        election.tally = vec![2, 1, 0];
        world.elections.insert("election-1".to_string(), election);

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:vote-tally";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = [5u8; 32];
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes,
        );
        let vk_id = VerifyingKeyId::new(backend, "vk");
        world.verifying_keys.insert(vk_id.clone(), rec);

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let view = state.view();
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        host.set_zk_config(&view.zk);
        host.set_chain_id(&view.chain_id);
        host.set_zk_snapshots_from_world(view.world(), &view.zk)
            .expect("hydrate zk snapshots");
        assert!(host.verifying_keys.contains_key(&vk_id));

        let mut vm = IVM::new(10_000);
        let req = ivm::zk_verify::VoteGetTallyRequest {
            election_id: "election-1".to_string(),
        };
        let req_bytes = norito::to_bytes(&req).expect("encode request");
        let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
        vm.set_register(10, req_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm)
            .expect("vote tally");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(
            gas,
            CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
        );
        let resp: ivm::zk_verify::VoteGetTallyResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode response");
        assert!(resp.finalized);
        assert_eq!(resp.tally, vec![2, 1, 0]);
    }

    #[test]
    fn from_state_hydrates_zk_snapshots() {
        crate::test_alias::ensure();
        let mut world = World::new();

        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("zkd", "universal").unwrap(),
            "zcoin".parse().unwrap(),
        );
        let mut zk_state = ZkAssetState::default();
        zk_state.root_history = vec![[1u8; 32], [2u8; 32]];
        world.zk_assets.insert(asset_def_id.clone(), zk_state);

        let mut election = ElectionState::default();
        election.finalized = true;
        election.tally = vec![1, 2];
        world.elections.insert("election-1".to_string(), election);

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:state-hydrate";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let schema_hash = [7u8; 32];
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes,
        );
        let vk_id = VerifyingKeyId::new(backend, "vk");
        world.verifying_keys.insert(vk_id.clone(), rec);

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::from_state(authority, &state);

        assert_eq!(
            host.zk_roots
                .get(&asset_def_id)
                .cloned()
                .unwrap_or_default(),
            vec![[1u8; 32], [2u8; 32]]
        );
        let (finalized, tally) = host
            .zk_elections
            .get("election-1")
            .cloned()
            .unwrap_or((false, Vec::new()));
        assert!(finalized);
        assert_eq!(tally, vec![1, 2]);
        assert!(host.verifying_keys.contains_key(&vk_id));

        let mut vm = IVM::new(10_000);
        let req = ivm::zk_verify::RootsGetRequest {
            asset_id: asset_def_id.to_string(),
            max: 2,
        };
        let req_bytes = norito::to_bytes(&req).expect("encode request");
        let req_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &req_bytes);
        vm.set_register(10, req_ptr);
        let gas = host
            .syscall(ivm_sys::SYSCALL_ZK_ROOTS_GET, &mut vm)
            .expect("roots get");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("roots output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        assert_eq!(
            gas,
            CoreHost::state_query_gas(req_bytes.len().saturating_add(tlv.payload.len()))
        );
        let resp: ivm::zk_verify::RootsGetResponse =
            norito::decode_from_bytes(tlv.payload).expect("decode roots response");
        assert_eq!(resp.latest, [2u8; 32]);
        assert_eq!(resp.roots, vec![[1u8; 32], [2u8; 32]]);
        assert_eq!(resp.height, 2);
    }

    #[test]
    fn state_syscall_flushes_to_wsv() {
        crate::test_alias::ensure();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let authority: AccountId = fixture_account("alice");
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
        let path_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&path));
        let value_bytes = norito::to_bytes(&1_u64).expect("encode state value");
        let value_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &value_bytes);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(ivm_sys::SYSCALL_STATE_SET, &mut vm),
            Ok(CoreHost::state_query_gas(value_bytes.len())),
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
        let value: u64 = norito::decode_from_bytes(tlv.payload).expect("decode state value");
        assert_eq!(value, 1);
    }

    fn dummy_env(
        circuit_id: &str,
        vk_hash: [u8; 32],
        public_inputs: Vec<u8>,
        proof_bytes: Vec<u8>,
    ) -> Vec<u8> {
        let env = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: circuit_id.to_string(),
            vk_hash,
            public_inputs,
            proof_bytes,
            aux: Vec::new(),
        };
        norito::to_bytes(&env).expect("serialize envelope")
    }

    fn minimal_zk1_vk_bytes(k: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ZK1\0");
        bytes.extend_from_slice(b"IPAK");
        bytes.extend_from_slice(&(4u32).to_le_bytes());
        bytes.extend_from_slice(&k.to_le_bytes());
        bytes
    }

    fn schema_hash(public_inputs: &[u8]) -> [u8; 32] {
        Hash::new(public_inputs).into()
    }

    fn active_vk_record(
        commitment: [u8; 32],
        schema_hash: [u8; 32],
        backend: &str,
        circuit_id: &str,
        namespace: &str,
        vk_bytes: Vec<u8>,
    ) -> VerifyingKeyRecord {
        let (backend_tag, curve) = if crate::zk::is_stark_fri_v1_backend(backend) {
            (BackendTag::Stark, "goldilocks")
        } else {
            (BackendTag::Halo2IpaPasta, "pallas")
        };
        let mut rec = VerifyingKeyRecord::new_with_owner(
            1,
            circuit_id.to_string(),
            None,
            namespace,
            backend_tag,
            curve,
            schema_hash,
            commitment,
        );
        rec.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        rec.max_proof_bytes = 1024 * 1024;
        rec.key = Some(VerifyingKeyBox::new(backend.into(), vk_bytes));
        rec
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn enable_halo2_batch_verifier(host: &mut CoreHost, verifier_max_batch: u32, max_k: u32) {
        let cfg = ivm::host::ZkHalo2Config {
            enabled: true,
            curve: ivm::host::ZkCurve::Pallas,
            backend: ivm::host::ZkHalo2Backend::Ipa,
            max_k,
            verifier_budget_ms: 200,
            verifier_max_batch,
            max_proof_bytes: usize::MAX,
            ..ivm::host::ZkHalo2Config::default()
        };
        host.halo2_config = cfg;
        host.default = ivm::host::DefaultHost::new().with_zk_halo2_config(cfg);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn registered_halo2_batch_fixture(
        host: &mut CoreHost,
        circuit_id: &str,
        namespace: &str,
    ) -> iroha_data_model::zk::OpenVerifyEnvelope {
        let backend = "halo2/ipa";
        let fixture_seed = crate::zk::test_utils::halo2_fixture_envelope(circuit_id, [0u8; 32]);
        let vk_bytes = fixture_seed.vk_bytes.expect("fixture vk bytes");
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let fixture = crate::zk::test_utils::halo2_fixture_envelope(circuit_id, commitment);
        let rec = active_vk_record(
            commitment,
            fixture.schema_hash,
            backend,
            circuit_id,
            namespace,
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");
        norito::decode_from_bytes(&fixture.proof_bytes).expect("decode fixture envelope")
    }

    #[test]
    fn enforce_zk_envelope_maps_errors_and_ok() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:transfer-check";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let public_inputs = vec![1u8, 2, 3, 4];
        let schema_hash = schema_hash(&public_inputs);
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes.clone(),
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");

        let ok_env = dummy_env(
            circuit_id,
            commitment,
            public_inputs.clone(),
            vec![0xAA; 16],
        );
        assert!(host.enforce_zk_envelope(&ok_env, "transfer").is_ok());

        let bad_circuit_env = dummy_env(
            "halo2/ipa:wrong-circuit",
            commitment,
            public_inputs,
            vec![0xAA; 16],
        );
        assert_eq!(
            host.enforce_zk_envelope(&bad_circuit_env, "transfer"),
            Err(ivm::host::ERR_VK_MISMATCH)
        );
    }

    #[test]
    fn enforce_zk_envelope_rejects_namespace_and_manifest_replays() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:transfer-check";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let public_inputs = vec![9u8, 8, 7, 6];
        let schema_hash = schema_hash(&public_inputs);
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "ballot",
            vk_bytes.clone(),
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");

        let env = dummy_env(
            circuit_id,
            commitment,
            public_inputs.clone(),
            vec![0xAA; 16],
        );
        assert_eq!(
            host.enforce_zk_envelope(&env, "transfer"),
            Err(ivm::host::ERR_NAMESPACE)
        );

        // Switching the caller manifest also trips the manifest binding.
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");
        host.set_current_manifest_id(Some("other".to_string()));
        assert_eq!(
            host.enforce_zk_envelope(&env, "transfer"),
            Err(ivm::host::ERR_NAMESPACE)
        );
    }

    #[test]
    fn enforce_zk_envelope_rejects_vk_metadata_mismatch() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:transfer-check";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let public_inputs = vec![1u8, 2, 3, 4];
        let schema_hash = schema_hash(&public_inputs);
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes.clone(),
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");

        // Schema hash mismatch is rejected.
        let env = dummy_env(circuit_id, commitment, vec![5u8, 6, 7, 8], vec![0xAA; 16]);
        assert_eq!(
            host.enforce_zk_envelope(&env, "transfer"),
            Err(ivm::host::ERR_VK_MISMATCH)
        );

        // Unknown vk hash is rejected explicitly.
        let env_bad_vk = dummy_env(
            circuit_id,
            [0xAA; 32],
            public_inputs.clone(),
            vec![0xAA; 16],
        );
        assert_eq!(
            host.enforce_zk_envelope(&env_bad_vk, "transfer"),
            Err(ivm::host::ERR_VK_MISSING)
        );

        // Happy-path still succeeds.
        let env_ok = dummy_env(circuit_id, commitment, public_inputs, vec![0xAA; 16]);
        assert!(host.enforce_zk_envelope(&env_ok, "transfer").is_ok());
    }

    #[test]
    fn generic_verify_proof_syscall_reports_registry_precheck_errors() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let payload = dummy_env(
            "halo2/ipa:missing-vk",
            [1u8; 32],
            vec![1u8, 2, 3, 4],
            vec![0xAA; 16],
        );
        let mut vm = IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_VERIFY_PROOF, &mut vm)
            .expect("generic verify proof syscall");
        assert!(gas > 0, "verification prechecks still charge proof gas");
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), ivm::host::ERR_VK_MISSING);
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_verify_batch_accepts_stark_registry_bound_envelope() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));
        let mut stark_cfg = iroha_config::parameters::actual::Stark::default();
        stark_cfg.enabled = true;
        host.set_stark_config(&stark_cfg);

        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = "stark/fri/sha256-goldilocks:ivm-syscall";
        let vk_payload = crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_string(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: crate::zk_stark::STARK_HASH_SHA256_V1,
        };
        let vk_bytes = norito::to_bytes(&vk_payload).expect("encode STARK vk");
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_bytes.clone());
        let schema_descriptor = b"ivm-syscall-schema-v1";
        let proof = crate::zk::prove_stark_fri_open_verify_envelope(
            backend,
            circuit_id,
            &vk_box,
            schema_descriptor,
            vec![vec![[7u8; 32]]],
        )
        .expect("prove STARK envelope");
        let env: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode OpenVerifyEnvelope");

        let commitment = crate::zk::hash_vk(&vk_box);
        let rec = active_vk_record(
            commitment,
            schema_hash(schema_descriptor),
            backend,
            circuit_id,
            "transfer",
            vk_bytes,
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk_stark"), rec);
        host.set_verifying_keys(map).expect("set registry");

        let payload = norito::to_bytes(&vec![env]).expect("encode batch");
        let mut vm = IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);

        host.syscall(ivm_sys::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
            .expect("batch verify");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let statuses: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode statuses");
        assert_eq!(statuses, vec![1]);
        assert_eq!(vm.register(11), 0);
        assert_eq!(vm.register(12), u64::MAX);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn zk_verify_batch_returns_statuses_with_registry_binding() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));
        enable_halo2_batch_verifier(&mut host, 8, 18);

        let env_ok =
            registered_halo2_batch_fixture(&mut host, "halo2/ipa:tiny-add-public", "transfer");
        assert!(
            !env_ok.public_inputs.is_empty(),
            "fixture circuit must expose public inputs for schema mismatch coverage"
        );
        let mut env_bad = env_ok.clone();
        env_bad.public_inputs[0] ^= 0x01;
        let payload = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

        let mut vm = IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);

        host.syscall(ivm_sys::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
            .expect("batch verify");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let statuses: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode statuses");
        assert_eq!(statuses, vec![1, 0]);
        assert_eq!(vm.register(11), ivm::host::ERR_VK_MISMATCH);
        assert_eq!(vm.register(12), 1);
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn zk_verify_batch_reports_backend_verifier_failure_after_prechecks() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));
        enable_halo2_batch_verifier(&mut host, 8, 18);

        let env_ok = registered_halo2_batch_fixture(&mut host, "halo2/ipa:tiny-add", "transfer");
        let mut env_bad = env_ok.clone();
        let last = env_bad
            .proof_bytes
            .last_mut()
            .expect("fixture proof bytes must not be empty");
        *last ^= 0x01;
        let payload = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

        let mut vm = IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);

        host.syscall(ivm_sys::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
            .expect("batch verify");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let statuses: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode statuses");
        assert_eq!(statuses, vec![1, 0]);
        assert_eq!(vm.register(11), ivm::host::ERR_VERIFY);
        assert_eq!(vm.register(12), 1);
    }

    #[test]
    fn zk_verify_batch_reports_first_error_for_dummy_payloads() {
        crate::test_alias::ensure();
        let mut host = CoreHost::new(fixture_account("alice"));
        host.set_chain_id_bytes(b"chain".to_vec());
        host.set_current_manifest_id(Some("core".to_string()));

        let backend = "halo2/ipa";
        let circuit_id = "halo2/ipa:transfer-check";
        let vk_bytes = minimal_zk1_vk_bytes(6);
        let commitment = CoreHost::hash_vk_bytes(backend, &vk_bytes);
        let public_inputs = vec![3u8, 1, 4, 1, 5, 9];
        let schema_hash = schema_hash(&public_inputs);
        let rec = active_vk_record(
            commitment,
            schema_hash,
            backend,
            circuit_id,
            "transfer",
            vk_bytes.clone(),
        );
        let mut map = BTreeMap::new();
        map.insert(VerifyingKeyId::new(backend, "vk"), rec);
        host.set_verifying_keys(map).expect("set registry");

        let env_bytes = dummy_env(circuit_id, commitment, public_inputs, vec![0xAA; 16]);
        let env: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&env_bytes).expect("decode envelope");
        let payload = norito::to_bytes(&vec![env]).expect("encode batch");

        let mut vm = IVM::new(1_000_000);
        let ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
        vm.set_register(10, ptr);

        host.syscall(ivm_sys::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
            .expect("batch verify");
        let out_ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let statuses: Vec<u8> = norito::decode_from_bytes(tlv.payload).expect("decode statuses");
        assert_eq!(statuses, vec![0]);
        assert_eq!(vm.register(11), ivm::host::ERR_VERIFY);
        assert_eq!(vm.register(12), 0);
    }

    #[test]
    fn input_publish_tlv_is_forwarded_to_default_host() {
        crate::test_alias::ensure();
        // Build a minimal program that calls INPUT_PUBLISH_TLV and then HALT
        let mut vm = IVM::new(10_000);
        let owner: AccountId = fixture_account("alice");
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
    fn get_public_input_uses_wsv_registry_and_charges_gas() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let name: Name = "pub_key".parse().unwrap();
        let payload = b"hello".to_vec();
        let tlv = make_tlv(PointerType::Blob as u16, &payload);
        let entry = norito::json::object([
            ("name", norito::json::Value::from(name.as_ref())),
            (
                "type_id",
                norito::json::Value::from(u64::from(PointerType::Blob as u16)),
            ),
            ("tlv_hex", norito::json::Value::from(hex::encode(&tlv))),
        ])
        .expect("registry entry");
        let registry = norito::json::Value::Array(vec![entry]);
        let custom = CustomParameter::new(ivm_metadata::public_inputs_id(), Json::from(registry));
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Custom(custom));

        let mut host = CoreHost::new(authority);
        host.set_public_inputs_from_parameters(&params);
        let mut vm = IVM::new(10_000);
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, name_ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
            .expect("public input syscall");
        let out_ptr = vm.register(10);
        let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(out_tlv.type_id, PointerType::Blob);
        assert_eq!(out_tlv.payload, payload.as_slice());
        let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
            PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
                .saturating_mul(u64::try_from(tlv.len()).unwrap_or(u64::MAX)),
        );
        assert_eq!(gas, expected_gas);
    }

    #[test]
    fn get_public_input_uses_programmatic_setters() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let name: Name = "pub_key".parse().unwrap();
        let payload = b"hello".to_vec();
        let tlv = make_tlv(PointerType::Blob as u16, &payload);
        let mut inputs = BTreeMap::new();
        inputs.insert(name.clone(), tlv.clone());

        let mut host = CoreHost::new(authority).with_public_inputs(inputs);
        let mut vm = IVM::new(10_000);
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, name_ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
            .expect("public input syscall");
        let out_ptr = vm.register(10);
        let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(out_tlv.type_id, PointerType::Blob);
        assert_eq!(out_tlv.payload, payload.as_slice());
        let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
            PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
                .saturating_mul(u64::try_from(tlv.len()).unwrap_or(u64::MAX)),
        );
        assert_eq!(gas, expected_gas);
    }

    #[test]
    fn get_public_input_exposes_trigger_event_args() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let args = Json::from(norito::json!({
            "kind": "asset_change",
            "amount_i64": 10
        }));
        let mut host = CoreHost::with_accounts_and_args(
            authority.clone(),
            Arc::new(vec![authority]),
            args.clone(),
        );
        let mut vm = IVM::new(10_000);
        let name: Name = TRIGGER_EVENT_PUBLIC_INPUT_KEY.parse().unwrap();
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, name_ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
            .expect("public input syscall");
        let out_ptr = vm.register(10);
        let out_tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
        assert_eq!(out_tlv.type_id, PointerType::Json);
        let decoded: Json = norito::decode_from_bytes(out_tlv.payload).expect("decode json");
        assert_eq!(decoded, args);

        let expected_tlv = make_tlv(PointerType::Json as u16, &norito_blob(&decoded));
        let expected_gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
            PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT
                .saturating_mul(u64::try_from(expected_tlv.len()).unwrap_or(u64::MAX)),
        );
        assert_eq!(gas, expected_gas);
    }

    #[test]
    fn get_public_input_missing_name_is_error() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let name: Name = "missing".parse().unwrap();
        let mut host = CoreHost::new(authority);
        let mut vm = IVM::new(10_000);
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, name_ptr);

        let err = host
            .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
            .expect_err("missing name should error");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn get_public_input_rejects_registry_type_mismatch() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
        let name: Name = "pub_key".parse().unwrap();
        let payload = b"hello".to_vec();
        let tlv = make_tlv(PointerType::Blob as u16, &payload);
        let entry = norito::json::object([
            ("name", norito::json::Value::from(name.as_ref())),
            (
                "type_id",
                norito::json::Value::from(u64::from(PointerType::Name as u16)),
            ),
            ("tlv_hex", norito::json::Value::from(hex::encode(&tlv))),
        ])
        .expect("registry entry");
        let registry = norito::json::Value::Array(vec![entry]);
        let custom = CustomParameter::new(ivm_metadata::public_inputs_id(), Json::from(registry));
        let mut params = Parameters::default();
        params.set_parameter(Parameter::Custom(custom));

        let mut host = CoreHost::new(authority);
        host.set_public_inputs_from_parameters(&params);
        assert!(host.public_inputs.is_empty());

        let mut vm = IVM::new(10_000);
        let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
        vm.set_register(10, name_ptr);
        let err = host
            .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut vm)
            .expect_err("mismatched registry entry should error");
        assert!(matches!(err, VMError::PermissionDenied));
    }

    #[test]
    fn set_account_detail_rejects_tlv_with_bad_hash() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
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
        // Install a non-v1 ABI annotation so pointer validation fails closed.
        let _guard = ivm::pointer_abi::PointerPolicyGuard::install(ivm::SyscallPolicy::AbiV1, 9);

        let mut vm = ivm::IVM::new(1_000_000);
        let did: DomainId = DomainId::try_new("wonder", "universal").unwrap();
        let payload = norito::to_bytes(&did).expect("encode domain id");
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
    fn decode_tlv_blob_accepts_code_region_literal() {
        crate::test_alias::ensure();

        let mut vm = ivm::IVM::new(1_000_000);
        vm.load_program(&build_program(
            &encoding::wide::encode_halt().to_le_bytes(),
            0,
        ))
        .expect("load abi v1 program");

        let payload = b"risk".to_vec();
        let tlv = make_tlv(PointerType::Blob as u16, &payload);
        vm.memory.load_code(&tlv);

        let decoded = CoreHost::decode_tlv_blob(&vm, 0).expect("decode code literal");
        assert_eq!(decoded, payload);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pointer_abi_transfer_asset_enqueues_isi() {
        // Prepare Norito-encoded inputs in INPUT region
        let from: AccountId = fixture_account("alice");
        let to: AccountId = fixture_account("bob");
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonder", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let amount = Numeric::from(1234_u64);
        let from_bytes = norito_blob(&from);
        let to_bytes = norito_blob(&to);
        let asset_bytes = norito_blob(&asset_def);
        let from_tlv = pointer_abi_tests::make_tlv(ivm::PointerType::AccountId as u16, &from_bytes);
        let to_tlv = pointer_abi_tests::make_tlv(ivm::PointerType::AccountId as u16, &to_bytes);
        let asset_tlv =
            pointer_abi_tests::make_tlv(ivm::PointerType::AssetDefinitionId as u16, &asset_bytes);
        let amount_payload = norito::to_bytes(&amount).expect("encode amount");
        let amount_tlv =
            pointer_abi_tests::make_tlv(ivm::PointerType::NoritoBytes as u16, &amount_payload);

        // Offsets in INPUT region
        let off_from = 0u64;
        let off_to = 256u64;
        let off_asset = 512u64;
        let off_amount = 768u64;
        let ptr_from = ivm::Memory::INPUT_START + off_from;
        let ptr_to = ivm::Memory::INPUT_START + off_to;
        let ptr_asset = ivm::Memory::INPUT_START + off_asset;
        let ptr_amount = ivm::Memory::INPUT_START + off_amount;

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
        vm.memory
            .preload_input(off_amount, &amount_tlv)
            .expect("preload input");

        // Attach CoreHost and load program
        vm.set_host(CoreHost::new(from.clone()));
        vm.load_program(&program).unwrap();

        // Set arg registers to pointers and amount
        vm.set_register(10, ptr_from);
        vm.set_register(11, ptr_to);
        vm.set_register(12, ptr_asset);
        vm.set_register(13, ptr_amount);

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
                    assert_eq!(inner.object, amount);
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
    fn nft_mint_enqueues_register_and_transfer() {
        let authority: AccountId = fixture_account("alice");
        let authority_clone = authority.clone();
        let owner: AccountId = fixture_account("bob");
        let nft_id: NftId = "gold$wonderland.universal".parse().unwrap();
        let nft_tlv = make_tlv(PointerType::NftId as u16, &norito_blob(&nft_id));
        let owner_tlv = make_tlv(PointerType::AccountId as u16, &norito_blob(&owner));

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
        vm.set_host(CoreHost::new(authority_clone));
        vm.load_program(&program).unwrap();
        vm.memory
            .preload_input(0, &nft_tlv)
            .expect("preload nft tlv");
        vm.memory
            .preload_input(256, &owner_tlv)
            .expect("preload owner tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);
        vm.set_register(11, ivm::Memory::INPUT_START + 256);
        vm.run().unwrap();

        let host_any = vm.host_mut_any().unwrap();
        let host = host_any.downcast_mut::<CoreHost>().unwrap();
        let queued = host.drain_instructions();
        assert_eq!(queued.len(), 2);
        let reg = queued[0]
            .as_any()
            .downcast_ref::<RegisterBox>()
            .expect("register instruction");
        match reg {
            RegisterBox::Nft(inner) => assert_eq!(&inner.object.id, &nft_id),
            _ => panic!("expected NFT register"),
        }
        let xfer = queued[1]
            .as_any()
            .downcast_ref::<TransferBox>()
            .expect("transfer instruction");
        match xfer {
            TransferBox::Nft(inner) => {
                assert_eq!(&inner.source, &authority);
                assert_eq!(&inner.destination, &owner);
                assert_eq!(&inner.object, &nft_id);
            }
            _ => panic!("expected NFT transfer"),
        }
    }

    #[cfg(all(feature = "telemetry", feature = "sm"))]
    #[test]
    fn sm3_syscall_records_success_metric() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
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
        host.force_sm_enabled_for_tests(true);
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

    #[cfg(all(feature = "telemetry", feature = "sm"))]
    #[test]
    fn sm3_syscall_failure_records_failure_metric() {
        crate::test_alias::ensure();
        let authority: AccountId = fixture_account("alice");
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
        host.force_sm_enabled_for_tests(true);
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
