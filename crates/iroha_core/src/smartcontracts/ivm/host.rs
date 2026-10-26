//! Core IVM host adapter.
//!
//! This host is responsible for handling IVM `SCALL`s. It MUST NOT mutate the
//! world state directly. Instead, it translates stateful calls into built‑in
//! ISIs and queues them so that the caller can execute them via the standard
//! executor. This ensures permissions, invariants, and events are enforced
//! uniformly and prevents direct world state mutations from the VM.
//!
//! Helper syscalls that do not touch WSV are forwarded to the IVM default host.
/// Physical durable state writes staged by this host.
type DurableStateOverlay = BTreeMap<StatePath, Option<Vec<u8>>>;
/// Borrowed visitor that can stop bounded traversal of physical state positions.
type StatePositionVisitor<'a> =
    dyn FnMut(&StatePath, bool) -> Result<std::ops::ControlFlow<()>, ivm::VMError> + 'a;
/// Staged writes paired with the immutable authorization that admitted each write.
#[cfg(any(test, feature = "iroha-core-tests"))]
type AuthorizedStateOverlay = (
    DurableStateOverlay,
    BTreeMap<StatePath, Option<ContractEntrypointAuthorizationSnapshot>>,
);
/// Test syscall result together with the resulting VM and staged writes.
#[cfg(test)]
type SyscallOutcome = (Result<u64, ivm::VMError>, IVM, DurableStateOverlay);
/// Test contract result, resulting VM and writes, and consumed cycle count.
#[cfg(test)]
type ContractCallOutcome = (Result<(), ivm::VMError>, IVM, DurableStateOverlay, u64);

mod attachment_lifecycle;
mod contract_calls;
pub(crate) mod contract_event;
mod controlled_asset_locks;
mod controlled_waterfall;
mod fi_link_revision;
mod linked_fi;
mod native_events;
#[cfg(test)]
#[path = "host/native_fi_waterfall_tests.rs"]
mod native_fi_waterfall_tests;
mod prepared_verifying_key_memo;
mod same_uid;
mod tlv_transport;
mod uid_holding_quote;

use super::cache::PreparedContractCache;
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    executor::{ContractEntrypointAuthorizationSnapshot, ContractRuntimeExecutionContext},
    smartcontracts::isi::{
        query::{
            IvmQueryValidator, QueryCountMode, QueryExecutionBudget, QueryExecutionStats,
            QueryLimits, ValidQueryRequest,
        },
        triggers::{TRIGGER_ENABLED_METADATA_KEY, set::SetReadOnly},
    },
    state::{
        StateBlock, StateQueryView, StateReadOnly, StateTransaction, StateView, WorldReadOnly,
    },
};
use iroha_crypto::{Hash, HashOf, PublicKey, streaming::TransportCapabilityResolutionSnapshot};
#[cfg(test)]
use iroha_data_model::smart_contract::entrypoint::EntrypointValueAtomV1;
use iroha_data_model::smart_contract::manifest::{
    ContractPermissionScopeV1, EntrypointAuthorizationV1,
};
#[cfg(test)]
use iroha_data_model::soracloud::SORACLOUD_HOST_REQUEST_VERSION_V1;
use iroha_data_model::{
    NetworkId, ValidationFail,
    account::rekey::AccountAlias,
    asset::{AssetBalancePolicy, AssetBalanceScope},
    errors::{AmxStage, AmxTimeout, CanonicalErrorKind},
    escrow::EscrowId,
    events::time::Schedule,
    isi::{
        AddSignatory, Burn, BurnBox, InstructionBox, Mint, MintBox, Register, RegisterBox,
        RemoveSignatory, SetAccountQuorum, SetKeyValue, SetKeyValueBox, SetParameter, Transfer,
        TransferAssetBatch, TransferAssetBatchEntry, TransferBox, Unregister, UnregisterBox,
        escrow::{
            AcceptAssetEscrow, CancelAssetEscrow, MarkEscrowPaymentSent, OpenAssetEscrow,
            OpenEscrowDispute, ReleaseAssetEscrow, ResolveEscrowDispute,
        },
        register::RegisterPeerWithPop,
        smart_contract_code as scode, zk as DMZk,
    },
    nexus::{
        AxtAnchoredSpendV1, AxtAssetIncarnationV1, AxtBinding, AxtDescriptor as ModelAxtDescriptor,
        AxtEnvelopeRecord, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot,
        AxtPolicySnapshotValidationError, AxtProofEnvelope as ModelAxtProofEnvelope,
        AxtProofFragment, AxtRejectContext, AxtRejectReason, AxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, ProofBlob as ModelProofBlob,
        TouchManifest as ModelTouchManifest, UniversalAccountId,
        VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX,
        VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX,
        VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX,
    },
    parameter::{Parameters, SmartContractParameters, system::ivm_metadata},
    permission::Permissions,
    prelude::{AccountId, *},
    proof::{ProofBox, VerifyingKeyId, VerifyingKeyRecord},
    query::{
        QueryItemKind, QueryOutputBatchBox, QueryRequest, QueryResponse, QueryWithParams,
        SingularQueryBox, SingularQueryOutputBox,
        account::prelude::FindAccountById,
        asset::prelude::{FindAssetById, FindAssetDefinitionById},
        domain::prelude::FindDomainById,
        dsl::{CompoundPredicate, SelectorTuple},
        error::QueryExecutionFail,
        nft::prelude::FindNftById,
        parameters::{FetchSize, Pagination, QueryParams, Sorting},
    },
    smart_contract::{ContractAddress, ContractAlias, ContractInstance},
    soracloud::{
        SoracloudHostOperationV1, SoracloudHostRequestEnvelopeV1, SoracloudHostRequestPayloadV1,
    },
    subscription::{
        SUBSCRIPTION_INVOICE_METADATA_KEY, SUBSCRIPTION_METADATA_KEY,
        SUBSCRIPTION_PLAN_METADATA_KEY, SUBSCRIPTION_TRIGGER_REF_METADATA_KEY,
    },
    zk::{BackendTag, OpenVerifyEnvelopeBounds, OpenVerifyEnvelopeValidationError},
};
#[cfg(test)]
use iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint;
use iroha_executor_data_model::permission::smart_contract::CanUseContractPermission;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::DataSpaceId;
use iroha_model_base::topology::LaneId;
use iroha_model_base::{name::Name, state_path::StatePath};
use iroha_primitives::{
    bigint::BigInt,
    calendar,
    numeric::Quantity,
    numeric_abi::{IntValueV1, QuantityValueV1},
};
#[cfg(test)]
use ivm::VMError;
use ivm::codec::{decode_canonical_norito, encode_canonical_norito};
use ivm::{
    self, CoreHost as IvmCodecHost, IVM, PointerType,
    analysis::{self, AmxLimits, ProgramAnalysis},
    axt::{self, ProofBlob, TouchManifest},
    core_query::{
        AccountView, AssetDefinitionView, AssetView, CoreQueryEntityTagV1, DomainView, NftView,
        QUERY_PAGE_CAPACITY_V1, QuantityV1, QueryPageV1,
    },
    host::{IVMHost, quote_tlv_payload_len_at},
    is_type_allowed_for_policy, pointer_abi,
};
use mv::storage::StorageReadOnly;
use norito::{
    NoritoDeserialize,
    core::{DecodeFromSlice, Header, NoritoSerialize, SerializePayload},
    json,
    streaming::CapabilityFlags,
};
use std::{
    any::Any,
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem,
    num::{NonZeroU16, NonZeroU64, NonZeroUsize},
    ops::Bound,
    str::FromStr,
    sync::Arc,
};
const AXT_PROOF_CACHE_HIT: &str = "hit";
const AXT_PROOF_CACHE_MISS: &str = "miss";
const AXT_PROOF_CACHE_EXPIRED: &str = "expired";
const AXT_PROOF_CACHE_CLEARED: &str = "cleared";
const AXT_PROOF_CACHE_REJECT: &str = "reject";
include!("host/contract_state_namespace.rs");
// `World::smart_contract_state` also carries native consensus records.  They must
// never be reachable through the generic contract state syscalls: a forged or
// deleted merge marker can make globally applied execution appear absent, fee
// markers are replay protection, and sealed-transaction records participate in
// admission ordering.  `sc/` is owned by this host for per-contract isolation;
// contracts address the unscoped suffix and must not address another contract's
// physical namespace directly.
//
// Keep these lists in sync with the native key constructors in `state.rs`,
// `tx.rs`, `da/quota.rs`, and the native custody query owners. Entries name a
// canonical root or include its delimiter. Matching reserves only the exact
// root and `_`/`/` descendants so similarly named user keys remain available.
const OPAQUE_SYSTEM_CONTRACT_STATE_PREFIXES: &[&str] = &[
    iroha_data_model::escrow::scoped::SCOPE_ROOT,
    crate::smartcontracts::isi::escrow::controlled::STATE_ROOT,
    crate::state::retail_daily_limit_state::POLICY_ROOT,
    crate::state::asset_fi::FI_ROOT,
    crate::state::asset_fi::ATTESTATION_ROOT,
    crate::state::asset_fi::LINK_ROOT,
    crate::state::asset_uid_enrollment::ENROLLMENT_ROOT,
    crate::state::asset_uid_enrollment::SYSTEM_ROOT,
    crate::state::asset_uid_reader::READER_ROOT,
    crate::state::asset_uid::POLICY_ROOT,
    crate::state::asset_uid::BINDING_ROOT,
    crate::state::asset_uid::USAGE_ROOT,
    crate::state::retail_daily_limit_state::ACTIVATION_ROOT,
    crate::state::retail_daily_limit_state::IDENTITY_ROOT,
    crate::state::retail_daily_limit_state::USAGE_ROOT,
    crate::state::retail_daily_limit_state::MONETARY_OPERATION_ROOT,
    "sorafs_final_promotion_authority_v1",
    "sorafs_final_promotion_account_custody_v1",
    "sorafs_stream_token_custody_v1",
    "sorafs_stream_token_operation_v1",
    crate::query::stream_token_gateway::storage::STATE_ROOT,
    "sorafs/provider_admission",
    "sc/",
    "da_ingest_quota_v1/",
    "faucet_claim_consumed_v1/",
    "merge_execution_batch_applied_",
    "merge_execution_lane_applied_",
    "merge_lane_frontier_v1_",
    "queue_plan_admission_v2_",
    "queue_plan_pending_obligation_v1_",
    "queue_plan_pending_route_member_v1_",
    "queue_plan_pending_signed_alias_member_v1_",
    "queue_plan_signed_alias_terminal_v1_",
    "nexus_fee_receipt_settled_",
    "nexus_fee_settlement_settled_",
    "sealed_tx_commitment_",
];
// These records are native-authored consensus inputs, but their canonical keys
// and encodings are an intentional public contract surface.  Generic state
// reads/enumeration remain supported; mutation must go through the validating
// native instructions that own the records.
const READ_ONLY_SYSTEM_CONTRACT_STATE_PREFIXES: &[&str] = &[
    "pkdeploy_verified_lane_relay_",
    "pkdeploy_verified_nexus_fee_budget_",
    VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX,
    VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX,
    VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX,
    "VerifiedLaneRelays/",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContractStateNamespaceAccess {
    User,
    ReadOnlySystem,
    OpaqueSystem,
}
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum AxtProofAdmission {
    // TODO: Admit this path only after the exact source roots and transaction
    // set are authenticated by an authoritative finalized/QC-backed source-state anchor.
    StandaloneUnanchored,
    #[cfg(test)]
    AuthenticatedHandle,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct AxtIssuerKeyBinding {
    manifest_root: [u8; 32],
    issuer: UniversalAccountId,
    public_key: PublicKey,
}
const PUBLIC_INPUT_GAS_BASE_DEFAULT: u64 = ivm::gas::HOST_BYTE_GAS_BASE;
const PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT: u64 = ivm::gas::SYSCALL_GAS_PER_BYTE;
const CREATE_NFTS_ALL_GAS: u64 = ivm::gas::HOST_BYTE_GAS_BASE;
/// Maximum NFT recipients of one `CREATE_NFTS_FOR_ALL_USERS` call.
///
/// Every call walks the host account snapshot from its first entry and stops
/// after this many recipients, so no call can observe a later account.
pub const CREATE_NFTS_FOR_ALL_MAX_RECIPIENTS: usize = 256;
/// Snapshot the account identifiers `CREATE_NFTS_FOR_ALL_USERS` can observe.
///
/// The syscall reads at most [`CREATE_NFTS_FOR_ALL_MAX_RECIPIENTS`] accounts from
/// the start of the canonical account order. Copying exactly that prefix keeps
/// host setup independent of the number of accounts in the World while every
/// call produces the same recipients as a complete snapshot would.
pub(crate) fn create_nfts_for_all_recipient_snapshot(
    world: &impl WorldReadOnly,
) -> Arc<Vec<AccountId>> {
    Arc::new(
        world
            .accounts_iter()
            .take(CREATE_NFTS_FOR_ALL_MAX_RECIPIENTS)
            .map(|entry| entry.id().clone())
            .collect(),
    )
}
const SMARTCONTRACT_DEPTH_GAS: u64 = ivm::gas::HOST_BYTE_GAS_BASE;
const AXT_GAS_BASE: u64 = ivm::gas::HOST_BYTE_GAS_BASE;
const AXT_GAS_PER_BYTE: u64 = ivm::gas::SYSCALL_GAS_PER_BYTE;
const AXT_VERIFY_GAS_BASE: u64 = ivm::gas::HOST_VERIFY_GAS_BASE;
const AXT_VERIFY_GAS_PER_BYTE: u64 = ivm::gas::SYSCALL_GAS_PER_BYTE;
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
    /// Hash of the proof payload and its exact outer expiry mirror.
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
        if let Some(expiry) = self.expiry_slot
            && slot > 0
            && slot > expiry
        {
            return false;
        }
        if slot == 0 || self.verified_slot == 0 {
            return true;
        }
        let ttl = ttl_slots.get().saturating_sub(1);
        let max_slot = self.verified_slot.saturating_add(ttl);
        slot >= self.verified_slot && slot <= max_slot
    }
}
pub(crate) fn current_axt_slot_for_state(state: &(impl StateReadOnly + ?Sized)) -> Option<u64> {
    let slot_length = state.nexus().axt.slot_length_ms;
    state
        .authenticated_query_ledger_time_ms()
        .map(|ledger_time_ms| ledger_time_ms / slot_length.get())
}
/// Resolve the balance scope of a contract `TRANSFER_ASSET_SCOPED` call.
///
/// Global definitions always move the global balance and ignore a supplied
/// dataspace. A dataspace-restricted definition needs the caller's explicit
/// `dataspace:` argument; without one (`r14 = 0`) the call traps exactly as a
/// missing `DataSpaceId` pointer would, instead of falling back to an ambient
/// or universal dataspace.
///
/// # Errors
/// Returns `NoritoInvalid` for a restricted definition without a dataspace
/// pointer.
// TODO: report a dedicated `VMError` variant naming the missing dataspace scope
// once `ivm_abi::error::VMError` and the proof-coverage producer inventory gain one.
fn transfer_scope_for_policy(
    policy: AssetBalancePolicy,
    dataspace: Option<DataSpaceId>,
) -> Result<AssetBalanceScope, ivm::VMError> {
    match (policy, dataspace) {
        (AssetBalancePolicy::Global, _) => Ok(AssetBalanceScope::Global),
        (AssetBalancePolicy::DataspaceRestricted, Some(dataspace)) => {
            Ok(AssetBalanceScope::Dataspace(dataspace))
        }
        (AssetBalancePolicy::DataspaceRestricted, None) => Err(ivm::VMError::NoritoInvalid),
    }
}
/// Convert a registered definition policy and exact AXT intent dataspace into
/// the public balance scope that the remote proof is claiming.
#[cfg(test)]
pub(crate) const fn remote_spend_asset_scope_from_policy(
    policy: AssetBalancePolicy,
    asset_dsid: DataSpaceId,
) -> AssetBalanceScope {
    match policy {
        AssetBalancePolicy::Global => AssetBalanceScope::Global,
        AssetBalancePolicy::DataspaceRestricted => AssetBalanceScope::Dataspace(asset_dsid),
    }
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
/// Optional observations for one V1 typed query execution.
///
/// The host leaves collection disabled unless a benchmark explicitly enables
/// it, so normal consensus execution does not maintain diagnostic state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreQueryPageMetrics {
    /// Number of bounded host query-engine invocations.
    pub host_queries: u64,
    /// Number of typed projection payload decode attempts.
    pub projection_decodes: u64,
    /// Canonical Norito bytes in the most recently decoded projection page.
    pub projection_payload_bytes: u64,
    /// Pointer-ABI TLV bytes prepared for typed leaves in that page.
    pub leaf_tlv_bytes: u64,
}
enum PreparedCoreQueryWord {
    Tlv(Vec<u8>),
    OptionalTlv(Option<Vec<u8>>),
}
impl PreparedCoreQueryWord {
    fn encoded_bytes(&self) -> u64 {
        let len = match self {
            Self::Tlv(tlv) | Self::OptionalTlv(Some(tlv)) => tlv.len(),
            Self::OptionalTlv(None) => 0,
        };
        u64::try_from(len).unwrap_or(u64::MAX)
    }
    fn materialize(self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        match self {
            Self::Tlv(tlv) => vm.alloc_host_tlv(&tlv),
            Self::OptionalTlv(tlv) => {
                let layout =
                    ivm::sum::SumLayoutV1::option(1).map_err(|_| ivm::VMError::DecodeError)?;
                match tlv {
                    Some(tlv) => {
                        let pointer = vm.alloc_host_tlv(&tlv)?;
                        ivm::sum::allocate_words(vm, layout, 1, &[pointer])
                    }
                    None => ivm::sum::allocate_words(vm, layout, 0, &[]),
                }
            }
        }
    }
}
/// Consensus bounds for artifacts retained by one host execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostOutputLimits {
    /// Maximum queued instructions, pending `FastPQ` entries, durable writes, and completed AXT items.
    pub max_items: u64,
    /// Maximum aggregate encoded/retained bytes for those artifacts.
    pub max_bytes: u64,
}
impl HostOutputLimits {
    /// Construct an output budget.
    #[must_use]
    pub const fn new(max_items: u64, max_bytes: u64) -> Self {
        Self {
            max_items,
            max_bytes,
        }
    }
    /// Derive the deterministic host-output budget from on-chain parameters.
    #[must_use]
    pub const fn from_parameters(parameters: SmartContractParameters) -> Self {
        Self::new(
            parameters.max_output_items.get(),
            parameters.max_output_bytes.get(),
        )
    }
    /// Return the component-wise stricter of two budgets.
    #[must_use]
    pub const fn restrict(self, stricter: Self) -> Self {
        Self::new(
            if self.max_items < stricter.max_items {
                self.max_items
            } else {
                stricter.max_items
            },
            if self.max_bytes < stricter.max_bytes {
                self.max_bytes
            } else {
                stricter.max_bytes
            },
        )
    }
}
impl Default for HostOutputLimits {
    fn default() -> Self {
        Self::from_parameters(SmartContractParameters::default())
    }
}
/// Sticky reason a bounded host stopped retaining execution output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOutputBudgetViolation {
    /// Retained item count would exceed the configured limit.
    ItemCount {
        /// Item count that would have been retained.
        attempted: u64,
        /// Configured maximum item count.
        limit: u64,
    },
    /// Retained bytes would exceed the configured limit.
    EncodedBytes {
        /// Byte count that would have been retained.
        attempted: u64,
        /// Configured maximum retained bytes.
        limit: u64,
    },
}
impl HostOutputBudgetViolation {
    fn into_vm_error(self) -> ivm::VMError {
        match self {
            Self::ItemCount { attempted, limit } => ivm::VMError::HostOutputBudgetExceeded {
                resource: ivm::HostOutputResource::Items,
                attempted,
                limit,
            },
            Self::EncodedBytes { attempted, limit } => ivm::VMError::HostOutputBudgetExceeded {
                resource: ivm::HostOutputResource::Bytes,
                attempted,
                limit,
            },
        }
    }
}
struct BoundedCountingWriter {
    written: usize,
    max: usize,
}
impl std::io::Write for BoundedCountingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("encoded output length overflow"))?;
        if next > self.max {
            return Err(std::io::Error::other("encoded output exceeds host limit"));
        }
        self.written = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Verifying-key metadata prepared and cached for repeated host-side proof checks.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedVerifyingKey {
    record: Arc<VerifyingKeyRecord>,
    backend_label: Arc<str>,
    material: Option<crate::zk::PreparedVerifyingKeyMaterialV1>,
}
/// One finalized epoch-boundary pulse that passed every State check of the
/// `VRF_EPOCH_SEED` projection; only its final threshold signature is unchecked.
#[derive(Clone, Debug, PartialEq, Eq)]
struct VrfEpochSeedCandidate {
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
    session: crate::beacon::ValidatedGlobalThresholdBeaconSessionV1,
    boundary_height: u64,
}
/// Candidates of one successor epoch and the seed resolved from them once.
#[derive(Clone, Debug, PartialEq, Eq)]
struct VrfEpochSeedEntry {
    candidates: Vec<VrfEpochSeedCandidate>,
    seed: std::sync::OnceLock<Option<[u8; 32]>>,
}
impl VrfEpochSeedEntry {
    /// Verify this epoch's candidates on first use and return its seed.
    ///
    /// Exactly one candidate with a valid final signature projects a seed.
    /// Conflicting valid candidates leave the epoch absent, as does none.
    fn seed(&self, epoch: u64) -> Option<[u8; 32]> {
        *self.seed.get_or_init(|| {
            let mut verified = self.candidates.iter().filter(|candidate| {
                crate::beacon::verify_persisted_global_threshold_beacon_pulse_signature_v1(
                    &candidate.session,
                    &candidate.pulse,
                )
                .is_ok()
            });
            let candidate = verified.next()?;
            if verified.next().is_some() {
                return None;
            }
            Some(
                crate::beacon::global_threshold_beacon_npos_successor_seed_v1(
                    &candidate.pulse,
                    candidate.boundary_height,
                    epoch,
                ),
            )
        })
    }
}
/// `VRF_EPOCH_SEED` projection of finalized epoch-boundary beacon pulses.
///
/// Building it runs every State check against one immutable State. The final
/// threshold signatures of an epoch's candidates are verified once, on the
/// first lookup of that epoch: verification is a pure function of the
/// retained validated session and pulse, so a lookup returns exactly the seed
/// that eager verification over the same State projects, while invocations
/// that never read a seed perform no pairing check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VrfEpochSeedProjection {
    epochs: BTreeMap<u64, VrfEpochSeedEntry>,
}
impl VrfEpochSeedProjection {
    /// Scan the finalized pulses of `state` for epochs of `epoch_length` blocks.
    ///
    /// Only pulses one block before an exact epoch boundary, at most the
    /// current height and anchored to the canonical block one height earlier
    /// are candidates, after the complete persisted-row, slot, network and key
    /// lifecycle checks of
    /// [`persisted_global_threshold_beacon_pulse_session_v1`](crate::beacon::persisted_global_threshold_beacon_pulse_session_v1).
    fn scan(state: &impl StateReadOnly, epoch_length: NonZeroU64) -> Self {
        let epoch_length = epoch_length.get();
        let maximum_height = u64::try_from(state.height()).unwrap_or(u64::MAX);
        let mut epochs: BTreeMap<u64, Vec<VrfEpochSeedCandidate>> = BTreeMap::new();
        for (_, candidate) in state.world().global_beacon_pulses().iter() {
            let Some(boundary_height) = candidate.height.checked_add(1) else {
                continue;
            };
            if candidate.height > maximum_height
                || boundary_height % epoch_length != 0
                || candidate.finalized_chain_anchor.height.checked_add(1) != Some(candidate.height)
            {
                continue;
            }
            let Some(anchor_index) = candidate
                .finalized_chain_anchor
                .height
                .checked_sub(1)
                .and_then(|height| usize::try_from(height).ok())
            else {
                continue;
            };
            if state.block_hashes().get(anchor_index)
                != Some(&candidate.finalized_chain_anchor.block_hash)
            {
                continue;
            }
            let Ok(session) = crate::beacon::persisted_global_threshold_beacon_pulse_session_v1(
                state.world(),
                state.network_id(),
                candidate,
            ) else {
                continue;
            };
            epochs
                .entry(boundary_height / epoch_length)
                .or_default()
                .push(VrfEpochSeedCandidate {
                    pulse: *candidate,
                    session: session.clone(),
                    boundary_height,
                });
        }
        Self {
            epochs: epochs
                .into_iter()
                .map(|(epoch, candidates)| {
                    (
                        epoch,
                        VrfEpochSeedEntry {
                            candidates,
                            seed: std::sync::OnceLock::new(),
                        },
                    )
                })
                .collect(),
        }
    }
    /// Return the seed projected for `epoch`, verifying its candidates on first use.
    fn seed(&self, epoch: u64) -> Option<[u8; 32]> {
        self.epochs.get(&epoch)?.seed(epoch)
    }
    #[cfg(test)]
    /// Install an already resolved seed for an explicitly constructed test host.
    fn insert_seed(&mut self, epoch: u64, seed: [u8; 32]) {
        self.epochs.insert(
            epoch,
            VrfEpochSeedEntry {
                candidates: Vec::new(),
                seed: std::sync::OnceLock::from(Some(seed)),
            },
        );
    }
    #[cfg(test)]
    /// Resolve every epoch, yielding the map an eager projection would contain.
    fn resolved_seeds(&self) -> BTreeMap<u64, [u8; 32]> {
        self.epochs
            .keys()
            .filter_map(|epoch| self.seed(*epoch).map(|seed| (*epoch, seed)))
            .collect()
    }
}
/// Source of the `VRF_EPOCH_SEED` projection installed in a host.
#[derive(Clone, Debug, PartialEq, Eq)]
enum VrfEpochSeedSource {
    /// Pulses scanned from the State passed to hydration.
    Scanned(VrfEpochSeedProjection),
    /// Pulses scanned on first use from the immutable query state pinned by
    /// [`CoreHostImpl::set_query_state_with_vrf_epoch_seeds`].
    QueryState {
        epoch_length: NonZeroU64,
        projection: std::sync::OnceLock<VrfEpochSeedProjection>,
    },
}
impl Default for VrfEpochSeedSource {
    fn default() -> Self {
        Self::Scanned(VrfEpochSeedProjection::default())
    }
}
#[cfg(test)]
impl VrfEpochSeedSource {
    /// Install an already resolved seed, replacing any deferred source.
    fn insert(&mut self, epoch: u64, seed: [u8; 32]) {
        if let Self::QueryState { .. } = self {
            *self = Self::default();
        }
        if let Self::Scanned(projection) = self {
            projection.insert_seed(epoch, seed);
        }
    }
    /// Whether a scanned projection resolves no epoch.
    fn is_empty(&self) -> bool {
        matches!(self, Self::Scanned(projection) if projection.resolved_seeds().is_empty())
    }
}
/// Read the committed `NPoS` epoch length that `VRF_EPOCH_SEED` projects pulses against.
///
/// `None` means the State has no `NPoS` parameters, so no epoch has a seed.
///
/// # Errors
/// Returns the original execution-attempt error if committed `NPoS` parameters
/// cannot be read or validated.
pub fn vrf_epoch_length_from_state(
    state: &impl StateReadOnly,
) -> Result<Option<NonZeroU64>, crate::execution_attempt::ExecutionAttemptError<String>> {
    Ok(state
        .world()
        .sumeragi_npos_parameters()?
        .map(|params| params.epoch_length_blocks()))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostExecutionClass {
    Generic,
    StateFreeGeneric,
    Contract,
    View,
    LocalContractDebug,
    LocalViewDebug,
}
impl HostExecutionClass {
    fn ensure_syscall_allowed(self, number: u32) -> Result<(), ivm::VMError> {
        if matches!(self, Self::Generic | Self::StateFreeGeneric)
            && !crate::smartcontracts::ivm::cache::is_generic_syscall_allowed(number)
        {
            return Err(ivm::VMError::GenericSyscallNotAllowed { syscall: number });
        }
        if matches!(self, Self::StateFreeGeneric | Self::LocalContractDebug)
            && ivm::syscalls::is_axt_syscall(number)
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        if matches!(self, Self::View | Self::LocalViewDebug)
            // Typed calls enforce the admitted target's view class before execution.
            && number != ivm::syscalls::SYSCALL_CALL_CONTRACT
            && matches!(
                ivm::syscalls::syscall_access(number),
                ivm::syscalls::SyscallAccess::StateWrite
                    | ivm::syscalls::SyscallAccess::LedgerWrite
                    | ivm::syscalls::SyscallAccess::Dynamic
            )
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        Ok(())
    }
}
/// Core host adapter used by Iroha to run IVM bytecode.
///
/// Stateful operations must be translated into ISIs and executed via the
/// executor. Durable-state syscalls are only forwarded to an in-memory
/// overlay when access logging is enabled for prepass execution.
/// Signed deployed invocations additionally borrow the executor's original
/// attachment owner for the whole run. Its fixed scope and consumption state
/// survive business rollback; no proof bytes enter a guest or a host-owned copy.
pub struct CoreHostImpl<'run, 'tx, QS> {
    authority: AccountId,
    attachment_custody: Option<crate::executor::attachment_custody::ContractCustody<'run, 'tx>>,
    execution_class: HostExecutionClass,
    local_debug_artifacts: bool,
    current_contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    current_entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    nested_contract_call_depth: usize,
    // Fixed V1 call-stack capacity; identity checks include every active ancestor.
    nested_contract_ancestors: [Option<ContractAddress>; MAX_NESTED_CONTRACT_CALL_DEPTH],
    default: ivm::host::DefaultHost,
    codec_host: IvmCodecHost,
    access_log_enabled: bool,
    zk_verify_limits: ivm::host::ZkVerifyLimits,
    zk_gas_schedule: ivm::gas::ZkGasScheduleV1,
    /// Verification gas schedule of the block this host executes in, installed
    /// with the block's frozen ZK settings by [`Self::set_zk_config`]. It prices
    /// the proof-bearing instructions the host queues.
    confidential_gas: iroha_config::parameters::actual::ConfidentialGas,
    stark_config: iroha_config::parameters::actual::Stark,
    pipa_r_config: iroha_config::parameters::actual::PipaR,
    crypto: Arc<iroha_config::parameters::actual::Crypto>,
    queued: Vec<QueuedEffect>,
    instruction_queue_limits: HostOutputLimits,
    instruction_queue_count: u64,
    instruction_queue_encoded_bytes: u64,
    instruction_queue_violation: Option<HostOutputBudgetViolation>,
    fastpq_batch_entries: Option<Vec<TransferAssetBatchEntry>>,
    // Canonically ordered account ids walked by `CREATE_NFTS_FOR_ALL_USERS`; state
    // owners supply only the prefix it can observe
    // (`create_nfts_for_all_recipient_snapshot`).
    accounts_snapshot: Arc<Vec<AccountId>>,
    // Optional owned contract registry snapshot for nested resolution without a live query view.
    bound_contract_records_by_subject:
        BTreeMap<AccountId, crate::smartcontracts::code::BoundContractRecord>,
    // Bounded immutable artifact cache shared with the outer runtime pool.
    prepared_contract_cache: PreparedContractCache,
    // Live read-only state view used to execute queries during IVM runs.
    query_state: QS,
    // Optional counters enabled only by typed-query performance validation.
    core_query_page_metrics: Option<CoreQueryPageMetrics>,
    // Simple counter for sample-generated NFT ids to guarantee uniqueness across calls.
    nft_seq: u64,
    // Optional trigger arguments for by-call entrypoints.
    args: Option<iroha_primitives::json::Json>,
    // Canonical schema-bound record for a parameterized contract entrypoint.
    entrypoint_argument_record: Option<ivm::PreparedArgumentRecord>,
    // Trigger identifier for the current execution (time/by-call triggers).
    current_trigger_id: Option<TriggerId>,
    // Block creation timestamp (UTC ms) for the current execution.
    current_block_time_ms: Option<u64>,
    // Current block height for the execution context.
    current_block_height: Option<u64>,
    // Snapshot of durable smart-contract state persisted in WSV.
    durable_state_base: BTreeMap<StatePath, Vec<u8>>,
    // Overlay of durable state updates staged by the current VM execution.
    durable_state_overlay: DurableStateOverlay,
    // Immutable authorization captured separately for every staged durable write.
    durable_state_authorizations:
        BTreeMap<StatePath, Option<ContractEntrypointAuthorizationSnapshot>>,
    // Access log for durable-state reads/writes (prepass).
    state_access_log: ivm::host::AccessLog,
    // Per-call undo records. Each journal stores only keys first touched while
    // it is active, so nesting does not clone the complete durable overlay.
    nested_contract_call_journals: Vec<NestedContractCallJournal>,
    // Active atomic cross-transaction (AXT) envelope state.
    axt_state: Option<Arc<axt::HostAxtState>>,
    // Completed AXT envelopes awaiting export into WSV/block artifacts.
    completed_axt: Vec<axt::HostAxtState>,
    // Snapshots of streaming session capabilities advertised by the VM.
    transport_caps_snapshot: Option<TransportCapabilityResolutionSnapshot>,
    negotiated_caps_snapshot: Option<CapabilityFlags>,
    // ZK-proof verification state (gates ISI mutations)
    zk_verified_ballot: Arc<VecDeque<[u8; 32]>>,
    zk_verified_tally: Arc<VecDeque<[u8; 32]>>,
    // Snapshots for state-read syscalls
    zk_roots: Arc<crate::state::IvmZkRootsSnapshot>,
    zk_elections: Arc<crate::state::IvmElectionsSnapshot>,
    // `VRF_EPOCH_SEED` projection; signatures and deferred scans resolve on first use.
    vrf_epoch_seeds: VrfEpochSeedSource,
    // Validated verifying-key registry indexed by commitment for proof dispatch.
    prepared_verifying_keys: BTreeMap<[u8; 32], PreparedVerifyingKey>,
    // Registry snapshot of public inputs exposed via SYSCALL_GET_PUBLIC_INPUT.
    public_inputs: BTreeMap<Name, PublicInputRecord>,
    // Chain id bytes for domain-tag binding.
    chain_id_bytes: Vec<u8>,
    // Exact genesis-derived security domain for AXT issuer authentication.
    network_id: Option<NetworkId>,
    // Mandatory snapshot-derived policy for AXT validation.
    axt_policy: Arc<dyn ivm::axt::AxtPolicy>,
    // Optional supplemental policy hook. This can only restrict the mandatory
    // authenticated snapshot policy; either denial wins.
    axt_policy_hook: Option<Arc<dyn ivm::axt::AxtPolicy>>,
    // AXT timing configuration sourced from `iroha_config`.
    axt_timing: iroha_config::parameters::actual::NexusAxt,
    // Current manifest id for namespace binding (None -> core/built-in).
    current_manifest_id: Option<String>,
    // Non-zero retained-root bound for ZK root reads.
    zk_tree_roots_history_len: NonZeroUsize,
    // Last seen verify envelope hashes per ZK op type (from pointer‑ABI TLVs)
    zk_last_env_hash_ballot: Arc<VecDeque<[u8; 32]>>,
    zk_last_env_hash_tally: Arc<VecDeque<[u8; 32]>>,
    // Cached AXT policy snapshot (if available) for telemetry and richer errors.
    axt_policy_snapshot: Option<AxtPolicySnapshot>,
    // Issuer keys resolved only from committed Space Directory/UAID state.
    axt_issuer_keys: Arc<BTreeMap<DataSpaceId, AxtIssuerKeyBinding>>,
    // Slot for which cached AXT proofs were verified.
    axt_proof_cache_slot: Option<u64>,
    // Cache of per-dataspace proofs validated in the current slot.
    axt_proof_cache: Arc<BTreeMap<DataSpaceId, CachedProofEntry>>,
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
pub type CoreHost = CoreHostImpl<'static, 'static, NoQueryState>;
/// Errors returned while constructing a core host from a state snapshot.
#[derive(Debug, thiserror::Error)]
pub enum CoreHostStateError {
    /// Original signed `NPoS` policy was invalid or locally unfinished.
    #[error("NPoS policy snapshot: {0}")]
    NposPolicy(crate::execution_attempt::ExecutionAttemptError<String>),
    /// A deployed contract registry read was rejected or remains locally unfinished.
    #[error("contract registry snapshot: {0}")]
    ContractRegistry(crate::execution_attempt::ExecutionAttemptError<ValidationFail>),
    /// The world-state ZK registry is internally inconsistent.
    #[error("invalid ZK snapshot state: {0}")]
    ZkSnapshot(ivm::VMError),
    /// The world-state AXT policy snapshot is not canonical.
    #[error("invalid AXT policy snapshot: {0}")]
    AxtPolicySnapshot(AxtPolicySnapshotValidationError),
}
/// Marker query slot for hosts that do not run queries.
#[derive(Default, Copy, Clone)]
pub struct NoQueryState;
/// Deterministic maximum number of active `CALL_CONTRACT` child frames.
const MAX_NESTED_CONTRACT_CALL_DEPTH: usize = 32;
/// Slot storing a live queryable state reference for a host run.
pub struct QueryStateSlot<QRef> {
    state: Option<QRef>,
    // State pinned for the deferred `VRF_EPOCH_SEED` projection.
    vrf_epoch_seed_state: Option<QRef>,
}
impl<QRef> Default for QueryStateSlot<QRef> {
    fn default() -> Self {
        Self {
            state: None,
            vrf_epoch_seed_state: None,
        }
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
        Some(self._curr_block.height().get())
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
    /// Execute a query request with a deterministic item-and-byte budget.
    ///
    /// Returns the query response and processed item count or a VM error.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the query cannot be executed or fails validation.
    fn execute_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError>
    where
        Self: Sized;
    /// Execute the authorized global-balance query, representing only an absent
    /// holding of an existing account and global asset definition as zero.
    ///
    /// # Errors
    ///
    /// Preserves query authorization, scope, budget, and unrelated execution failures.
    fn execute_account_balance_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError>
    where
        Self: Sized;
    /// Execute one singular query while representing a missing entity as
    /// `None` instead of a VM failure.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] for permission failures, malformed queries,
    /// and execution failures other than an absent entity.
    fn execute_optional_singular_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(Option<SingularQueryOutputBox>, QueryExecutionStats), ivm::VMError>
    where
        Self: Sized;
    /// Execute a bounded iterable query that stops after its first page and a
    /// single lookahead item.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if validation or execution fails.
    fn execute_bounded_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError>
    where
        Self: Sized;
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
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        QueryStateRef::execute_query_with_budget(self, authority, request, budget)
    }
    fn execute_account_balance_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        QueryStateRef::execute_account_balance_query(self, authority, request, budget)
    }
    fn execute_optional_singular_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(Option<SingularQueryOutputBox>, QueryExecutionStats), ivm::VMError> {
        QueryStateRef::execute_optional_singular_query(self, authority, request, budget)
    }
    fn execute_bounded_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        QueryStateRef::execute_bounded_query_with_budget(self, authority, request, budget)
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
    /// Fetch the state pinned for a deferred `VRF_EPOCH_SEED` projection, if any.
    fn vrf_epoch_seed_state(&self) -> Option<Self::Ref<'_>> {
        None
    }
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
    fn vrf_epoch_seed_state(&self) -> Option<Self::Ref<'_>> {
        self.vrf_epoch_seed_state
    }
}
fn map_query_validation_error(
    error: crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
) -> ivm::VMError {
    let error = match error {
        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
            return reason.into_vm_error();
        }
        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
    };
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
fn map_query_execution_error(
    error: crate::execution_attempt::ExecutionAttemptError<QueryExecutionFail>,
) -> ivm::VMError {
    let error = match error {
        crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
            return reason.into_vm_error();
        }
        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
    };
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
    /// Encoded bytes traversed or returned during query execution.
    pub processed_bytes: u64,
}
struct HostQueryValidator<'a, R: StateReadOnly> {
    authority: &'a AccountId,
    state: &'a R,
}
impl<R: StateReadOnly> IvmQueryValidator for HostQueryValidator<'_, R> {
    fn authority(&self) -> &AccountId {
        self.authority
    }
    fn validate_query(
        &mut self,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        self.state
            .world()
            .executor()
            .validate_query(self.state, authority, query)
    }
}
fn is_missing_query_error(error: &QueryExecutionFail) -> bool {
    matches!(
        error,
        QueryExecutionFail::Find(_) | QueryExecutionFail::NotFound
    )
}
fn execute_query_on_state_with_count_mode<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget: Option<QueryExecutionBudget>,
    count_mode: QueryCountMode,
) -> Result<QueryExecutionResult, ivm::VMError> {
    let mut validator = HostQueryValidator { authority, state };
    let limits = match count_mode {
        QueryCountMode::Exact => QueryLimits::from_pipeline(state.pipeline()),
        QueryCountMode::Bounded => QueryLimits::new(QUERY_PAGE_CAPACITY_V1 as u64),
    }
    .with_count_mode(count_mode);
    let validated = ValidQueryRequest::validate_for_ivm(request, &mut validator, limits)
        .map_err(map_query_validation_error)?;
    let (response, execution_stats) = validated
        .execute_ephemeral_with_stats(state.query_handle(), state, authority, budget)
        .map_err(map_query_execution_error)?;
    Ok(QueryExecutionResult {
        response,
        processed_items: execution_stats.processed_items(),
        processed_bytes: execution_stats.processed_bytes(),
    })
}
fn execute_query_on_state_with_budget<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget: Option<QueryExecutionBudget>,
) -> Result<QueryExecutionResult, ivm::VMError> {
    execute_query_on_state_with_count_mode(state, authority, request, budget, QueryCountMode::Exact)
}
fn execute_bounded_query_on_state_with_budget<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget: Option<QueryExecutionBudget>,
) -> Result<QueryExecutionResult, ivm::VMError> {
    execute_query_on_state_with_count_mode(
        state,
        authority,
        request,
        budget,
        QueryCountMode::Bounded,
    )
}
// The balance syscall alone defines an absent holding as zero. Ordinary singular
// asset queries still report absence, and no validation failure is converted.
fn execute_account_balance_query_on_state<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget: Option<QueryExecutionBudget>,
) -> Result<QueryExecutionResult, ivm::VMError> {
    let QueryRequest::Singular(SingularQueryBox::FindAssetById(query)) = &request else {
        return Err(ivm::VMError::DecodeError);
    };
    let asset_id = query.asset_id().clone();
    let mut validator = HostQueryValidator { authority, state };
    let validated = ValidQueryRequest::validate_for_ivm(
        request,
        &mut validator,
        QueryLimits::from_pipeline(state.pipeline()),
    )
    .map_err(map_query_validation_error)?;
    let (response, execution_stats) = match validated.execute_ephemeral_with_stats(
        state.query_handle(),
        state,
        authority,
        budget,
    ) {
        Ok(result) => result,
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            QueryExecutionFail::Find(iroha_data_model::query::error::FindError::Asset(missing)),
        )) if *missing == asset_id
            && *asset_id.scope() == AssetBalanceScope::Global
            && state.world().accounts().get(asset_id.account()).is_some()
            && state.world().assets().get(&asset_id).is_none()
            && state
                .world()
                .asset_definitions()
                .get(asset_id.definition())
                .is_some_and(|definition| {
                    definition.balance_scope_policy() == AssetBalancePolicy::Global
                }) =>
        {
            // Charge the synthetic source item and framed response before publishing
            // a Quantity TLV, just as a real query response is budget-admitted.
            let output = SingularQueryOutputBox::Asset(Asset::new(asset_id, Quantity::zero()));
            let mut execution_stats = QueryExecutionStats::default();
            execution_stats
                .record_item(&output, budget)
                .map_err(|error| map_query_execution_error(error.into()))?;
            let response = QueryResponse::Singular(output);
            execution_stats
                .record_response(&response, budget)
                .map_err(|error| map_query_execution_error(error.into()))?;
            (response, execution_stats)
        }
        Err(error) => return Err(map_query_execution_error(error)),
    };
    Ok(QueryExecutionResult {
        response,
        processed_items: execution_stats.processed_items(),
        processed_bytes: execution_stats.processed_bytes(),
    })
}
fn execute_optional_singular_query_on_state<R: StateReadOnly>(
    state: &R,
    authority: &AccountId,
    request: QueryRequest,
    budget: Option<QueryExecutionBudget>,
) -> Result<(Option<SingularQueryOutputBox>, QueryExecutionStats), ivm::VMError> {
    if !matches!(request, QueryRequest::Singular(_)) {
        return Err(ivm::VMError::DecodeError);
    }
    let mut validator = HostQueryValidator { authority, state };
    let limits = QueryLimits::from_pipeline(state.pipeline());
    let validated = match ValidQueryRequest::validate_for_ivm(request, &mut validator, limits) {
        Ok(validated) => validated,
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            ValidationFail::QueryFailed(error),
        )) if is_missing_query_error(&error) => {
            return Ok((None, QueryExecutionStats::default()));
        }
        Err(error) => return Err(map_query_validation_error(error)),
    };
    match validated.execute_ephemeral_with_stats(state.query_handle(), state, authority, budget) {
        Ok((QueryResponse::Singular(output), execution_stats)) => {
            Ok((Some(output), execution_stats))
        }
        Ok((QueryResponse::Iterable(_), _)) => Err(ivm::VMError::DecodeError),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(error))
            if is_missing_query_error(&error) =>
        {
            Ok((None, QueryExecutionStats::default()))
        }
        Err(error) => Err(map_query_execution_error(error)),
    }
}
// The filter's original decoded graph is consumed by the synchronous ephemeral
// query while this owner remains alive. Its allocation allowance is never moved
// to a different pool or released before the query drops its input.
struct CoreQueryAccountFilter {
    account: Option<AccountId>,
    _reservation: Option<iroha_allocation::AllocationReservation>,
}
impl CoreQueryAccountFilter {
    fn decode(vm: &IVM, pointer: u64) -> Result<Self, ivm::VMError> {
        let tlv = vm.validate_tlv(pointer)?;
        if tlv.type_id != PointerType::AccountId {
            return Err(ivm::VMError::DecodeError);
        }
        let limits = norito::canonical_decode_limits(tlv.payload.len());
        let mut reservation =
            vm.allocation_budget()
                .map(|budget| {
                    budget.try_reserve_bytes(limits.max_total_allocated_bytes().saturating_add(
                        norito::core::DecodeBudgetContext::allocation_layout().size(),
                    ))
                })
                .transpose()
                .map_err(ivm::VMError::AllocationDeferred)?;
        let unavailable =
            || ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::AllocationUnavailable);
        let context = match reservation.as_mut() {
            Some(reservation) => norito::core::DecodeBudgetContext::from_reservation(
                limits,
                reservation,
            )
            .map_err(|error| match error {
                iroha_allocation::PrepaidSharedError::Reservation(_) => {
                    ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::LocalInvariantViolation)
                }
                iroha_allocation::PrepaidSharedError::Allocator { .. } => unavailable(),
            })?,
            None => norito::core::DecodeBudgetContext::new(limits),
        };
        let account = context
            .with(|| norito::decode_canonical_with_limits(tlv.payload, limits))
            .map_err(|error| match error {
                norito::Error::AllocationFailed { .. } => unavailable(),
                _ => ivm::VMError::DecodeError,
            })?;
        drop(context);
        Ok(Self {
            account: Some(account),
            _reservation: reservation,
        })
    }
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
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        match self {
            QueryStateRef::View(view) => {
                execute_query_on_state_with_budget(view, authority, request, budget)
            }
            QueryStateRef::QueryView(view) => {
                execute_query_on_state_with_budget(view, authority, request, budget)
            }
            QueryStateRef::Block(block) => {
                execute_query_on_state_with_budget(block, authority, request, budget)
            }
            QueryStateRef::Transaction(tx) => {
                execute_query_on_state_with_budget(tx, authority, request, budget)
            }
        }
    }
    fn execute_account_balance_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        match self {
            QueryStateRef::View(view) => {
                execute_account_balance_query_on_state(view, authority, request, budget)
            }
            QueryStateRef::QueryView(view) => {
                execute_account_balance_query_on_state(view, authority, request, budget)
            }
            QueryStateRef::Block(block) => {
                execute_account_balance_query_on_state(block, authority, request, budget)
            }
            QueryStateRef::Transaction(tx) => {
                execute_account_balance_query_on_state(tx, authority, request, budget)
            }
        }
    }
    fn execute_optional_singular_query(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(Option<SingularQueryOutputBox>, QueryExecutionStats), ivm::VMError> {
        match self {
            QueryStateRef::View(view) => {
                execute_optional_singular_query_on_state(view, authority, request, budget)
            }
            QueryStateRef::QueryView(view) => {
                execute_optional_singular_query_on_state(view, authority, request, budget)
            }
            QueryStateRef::Block(block) => {
                execute_optional_singular_query_on_state(block, authority, request, budget)
            }
            QueryStateRef::Transaction(tx) => {
                execute_optional_singular_query_on_state(tx, authority, request, budget)
            }
        }
    }
    fn execute_bounded_query_with_budget(
        self,
        authority: &AccountId,
        request: QueryRequest,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<QueryExecutionResult, ivm::VMError> {
        match self {
            QueryStateRef::View(view) => {
                execute_bounded_query_on_state_with_budget(view, authority, request, budget)
            }
            QueryStateRef::QueryView(view) => {
                execute_bounded_query_on_state_with_budget(view, authority, request, budget)
            }
            QueryStateRef::Block(block) => {
                execute_bounded_query_on_state_with_budget(block, authority, request, budget)
            }
            QueryStateRef::Transaction(tx) => {
                execute_bounded_query_on_state_with_budget(tx, authority, request, budget)
            }
        }
    }
}
/// Resolved subscription state and billing inputs for the trigger currently being billed.
pub struct SubscriptionContext {
    executable: Executable,
    authority: AccountId,
    trigger_metadata: Metadata,
    subscription_nft_id: NftId,
    subscription_state: SubscriptionState,
    plan: SubscriptionPlan,
    charge_asset_def: AssetDefinition,
    subscriber_balance: Quantity,
    nft_owner: AccountId,
}
/// Helpers for accessing subscription data through a query-state reference.
pub trait QueryStateRefOps {
    /// Return the governed smart-contract heap ceiling in bytes.
    fn smart_contract_heap_limit(&self) -> u64;
    /// Scan finalized epoch-boundary pulses into a `VRF_EPOCH_SEED` projection.
    ///
    /// Final threshold signatures are verified only when an epoch is first read.
    fn vrf_epoch_seed_projection(&self, epoch_length: NonZeroU64) -> VrfEpochSeedProjection;
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
    /// Load an asset definition's balance-scope policy.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the asset definition is missing.
    fn asset_balance_policy(
        &self,
        definition_id: &AssetDefinitionId,
    ) -> Result<AssetBalancePolicy, ivm::VMError>;
    /// Read the exact beneficial-holdings projection for an owner-granted controller.
    ///
    /// Every inspected UID member and escrow row consumes the same query budget.
    ///
    /// # Errors
    ///
    /// Returns a VM error when the active controller or owner grant is invalid,
    /// the native policy is absent, or the deterministic query budget is exhausted.
    /// Local allocation refusals retain their original deferred-execution owner.
    fn uid_holding_quote(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        account: &AccountId,
        incoming: &Quantity,
        budget: QueryExecutionBudget,
    ) -> Result<
        (
            iroha_data_model::asset::uid::AssetUidHoldingQuoteV1,
            QueryExecutionStats,
        ),
        ivm::VMError,
    >;
    /// Read an exact current owner-approved FI-link revision after expected-FI equality.
    ///
    /// # Errors
    /// Returns the original local deferral or deterministic authority, relationship,
    /// canonical-decoding or shared-budget refusal. This is not a transfer grant.
    fn fi_link_revision(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        account: &AccountId,
        expected_fi: &AccountId,
        budget: QueryExecutionBudget,
    ) -> Result<(u64, QueryExecutionStats), ivm::VMError>;
    /// Compare exact current native retail memberships under the owner's reader grant.
    ///
    /// # Errors
    /// Rejects absent/nonretail/invalid membership, foreign frame or revoked grant;
    /// preserves canonical decode and local resource refusal provenance.
    fn same_uid(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        source: &AccountId,
        destination: &AccountId,
        budget: QueryExecutionBudget,
    ) -> Result<(bool, QueryExecutionStats), ivm::VMError>;
    /// Load an asset definition's exact incarnation and require that it was
    /// already live at the beginning of this block.
    ///
    /// # Errors
    ///
    /// Returns an [`ivm::VMError`] if the definition is missing, was registered,
    /// unregistered, or re-registered in the current block, or its token is
    /// malformed.
    fn stable_axt_asset_incarnation(
        &self,
        definition_id: &AssetDefinitionId,
    ) -> Result<AxtAssetIncarnationV1, ivm::VMError>;
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
    /// Resolve an authenticated contract identity, runtime subject, and artifact length without
    /// copying its manifest or bytecode.
    ///
    /// # Errors
    /// Returns a VM error if committed scope authorization or contract identity
    /// lookup fails. Missing identity, subject or deployed artifact instead
    /// returns `Ok(None)`.
    fn bound_contract_dispatch_identity_by_address(
        &self,
        contract_address: &ContractAddress,
    ) -> Result<
        Option<(
            crate::smartcontracts::code::BoundContractIdentity,
            AccountId,
            usize,
        )>,
        ivm::VMError,
    >;
    /// Prepare a content-addressed cache miss while borrowing deployed bytecode in place.
    ///
    /// # Errors
    /// Returns a VM error if the deployed artifact is absent or cannot be read,
    /// authenticated or prepared within the cache owner's resources.
    fn prepare_contract_cache_miss(
        &self,
        cache: &PreparedContractCache,
        artifact: iroha_data_model::smart_contract::ContractArtifactId,
    ) -> Result<ivm::PreparedContract, ivm::VMError>;
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
    /// Enforce manifest entrypoint permission metadata against the current world snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationFail`] when the authority is missing or lacks the
    /// named entrypoint permission.
    fn enforce_named_contract_entrypoint_authorization(
        &self,
        authority: &AccountId,
        contract_address: &ContractAddress,
        entrypoint: &str,
        permission: &EntrypointAuthorizationV1,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>>;
    /// Validate that a nested target is executable and not awaiting a lifecycle hook.
    ///
    /// # Errors
    /// Returns [`ValidationFail`] while a Parliament hold is active, when the live binding
    /// changed, or when `hajimari`/`始まり` or `kaizen`/`改善` is still pending.
    fn ensure_contract_entrypoint_lifecycle(
        &self,
        contract_address: &ContractAddress,
        code_hash: Hash,
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
    ) -> Result<(), ValidationFail>;
    /// Load durable smart-contract state by canonical key.
    fn durable_state_get(&self, key: &StatePath) -> Option<Vec<u8>>;
    /// Check durable smart-contract state without cloning the stored value.
    fn durable_state_contains(&self, key: &StatePath) -> bool;
    /// Read the payload length of durable smart-contract state without cloning it.
    ///
    /// # Errors
    ///
    /// Returns [`ivm::VMError::NoritoInvalid`] when the stored TLV has an
    /// unexpected pointer type.
    fn durable_state_payload_len(&self, key: &StatePath) -> Result<Option<usize>, ivm::VMError>;
    /// Seek after a canonical physical position and stop immediately when the
    /// page visitor reaches its bound. Values are never fetched or cloned.
    ///
    /// # Errors
    ///
    /// Propagates an error returned by `visitor`.
    fn visit_durable_state_positions(
        &self,
        prefix: &str,
        after: Option<&str>,
        visitor: &mut dyn FnMut(&StatePath) -> Result<std::ops::ControlFlow<()>, ivm::VMError>,
    ) -> Result<std::ops::ControlFlow<()>, ivm::VMError>;
    /// Visit durable-state keys whose text begins with `prefix`, in canonical order.
    ///
    /// Implementations must start at the prefix's ordered lower bound and stop
    /// when keys cease to share it; callers perform the stricter path-segment
    /// check. Values are never visited or cloned.
    ///
    /// # Errors
    ///
    /// Propagates an error returned by `visitor`.
    fn visit_durable_state_keys_with_text_prefix(
        &self,
        prefix: &str,
        visitor: &mut dyn FnMut(&StatePath) -> Result<(), ivm::VMError>,
    ) -> Result<(), ivm::VMError>;
}
fn durable_state_payload_len(stored: &[u8]) -> Result<usize, ivm::VMError> {
    ivm::host::validate_state_value_payload_len(stored.len())?;
    Ok(stored.len())
}
struct LocalDurableStateMerge<'a> {
    base: std::iter::Peekable<std::collections::btree_map::Range<'a, StatePath, Vec<u8>>>,
    overlay:
        std::iter::Peekable<std::collections::btree_map::Range<'a, StatePath, Option<Vec<u8>>>>,
}
impl<'a> LocalDurableStateMerge<'a> {
    fn new(
        base: &'a BTreeMap<StatePath, Vec<u8>>,
        overlay: &'a DurableStateOverlay,
        prefix: &str,
    ) -> Self {
        Self {
            base: base
                .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
                .peekable(),
            overlay: overlay
                .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
                .peekable(),
        }
    }
    fn seek(
        base: &'a BTreeMap<StatePath, Vec<u8>>,
        overlay: &'a DurableStateOverlay,
        prefix: &str,
        after: Option<&str>,
    ) -> Self {
        let lower = after.map_or(Bound::Included(prefix), Bound::Excluded);
        Self {
            base: base.range::<str, _>((lower, Bound::Unbounded)).peekable(),
            overlay: overlay
                .range::<str, _>((lower, Bound::Unbounded))
                .peekable(),
        }
    }
    fn peek_key(&mut self) -> Option<&'a StatePath> {
        match (self.base.peek(), self.overlay.peek()) {
            (Some((base, _)), Some((overlay, _))) => {
                Some(if *base <= *overlay { *base } else { *overlay })
            }
            (Some((base, _)), None) => Some(*base),
            (None, Some((overlay, _))) => Some(*overlay),
            (None, None) => None,
        }
    }
    fn next(&mut self) -> Option<(&'a StatePath, bool)> {
        let ordering = match (self.base.peek(), self.overlay.peek()) {
            (Some((base, _)), Some((overlay, _))) => base.cmp(overlay),
            (Some(_), None) => core::cmp::Ordering::Less,
            (None, Some(_)) => core::cmp::Ordering::Greater,
            (None, None) => return None,
        };
        match ordering {
            core::cmp::Ordering::Less => {
                let (key, _) = self.base.next().expect("peeked base entry");
                Some((key, true))
            }
            core::cmp::Ordering::Greater => {
                let (key, value) = self.overlay.next().expect("peeked overlay entry");
                Some((key, value.is_some()))
            }
            core::cmp::Ordering::Equal => {
                let (key, _) = self.base.next().expect("peeked base entry");
                let (_, value) = self.overlay.next().expect("peeked overlay entry");
                Some((key, value.is_some()))
            }
        }
    }
    fn next_before(&mut self, upper: &StatePath) -> Option<(&'a StatePath, bool)> {
        (self.peek_key()? < upper).then(|| self.next()).flatten()
    }
    fn take_equal(&mut self, key: &StatePath) -> Option<bool> {
        (self.peek_key()? == key)
            .then(|| self.next().map(|(_, present)| present))
            .flatten()
    }
}
fn visit_merged_durable_state_keys(
    base: &BTreeMap<StatePath, Vec<u8>>,
    overlay: &DurableStateOverlay,
    physical_prefix: &str,
    visit_live: impl FnOnce(
        &mut dyn FnMut(&StatePath) -> Result<(), ivm::VMError>,
    ) -> Result<(), ivm::VMError>,
    emit: &mut dyn FnMut(&StatePath, bool) -> Result<(), ivm::VMError>,
) -> Result<(), ivm::VMError> {
    let mut local = LocalDurableStateMerge::new(base, overlay, physical_prefix);
    let mut previous_live = String::new();
    let mut saw_live = false;
    let mut merge_live = |live_key: &StatePath| -> Result<(), ivm::VMError> {
        if !live_key.as_ref().starts_with(physical_prefix) {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if saw_live && previous_live.as_str() >= live_key.as_ref() {
            return Err(ivm::VMError::NoritoInvalid);
        }
        while let Some((key, present)) = local.next_before(live_key) {
            if !key.as_ref().starts_with(physical_prefix) {
                return Err(ivm::VMError::NoritoInvalid);
            }
            emit(key, present)?;
        }
        let present = local.take_equal(live_key).unwrap_or(true);
        emit(live_key, present)?;
        previous_live.clear();
        previous_live.push_str(live_key.as_ref());
        saw_live = true;
        Ok(())
    };
    visit_live(&mut merge_live)?;
    while let Some((key, present)) = local.next() {
        if !key.as_ref().starts_with(physical_prefix) {
            break;
        }
        emit(key, present)?;
    }
    Ok(())
}
fn visit_bounded_durable_state_positions(
    base: &BTreeMap<StatePath, Vec<u8>>,
    overlay: &DurableStateOverlay,
    prefix: &str,
    after: Option<&str>,
    visit_live: impl FnOnce(
        &mut dyn FnMut(&StatePath) -> Result<std::ops::ControlFlow<()>, ivm::VMError>,
    ) -> Result<std::ops::ControlFlow<()>, ivm::VMError>,
    emit: &mut StatePositionVisitor<'_>,
) -> Result<(), ivm::VMError> {
    use std::ops::ControlFlow;
    let mut local = LocalDurableStateMerge::seek(base, overlay, prefix, after);
    let mut previous_live: Option<StatePath> = None;
    let mut merge_live = |live_key: &StatePath| -> Result<ControlFlow<()>, ivm::VMError> {
        if !live_key.as_ref().starts_with(prefix)
            || after.is_some_and(|after| live_key.as_ref() <= after)
            || previous_live
                .as_ref()
                .is_some_and(|previous| previous >= live_key)
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        while let Some((key, present)) = local.next_before(live_key) {
            if !key.as_ref().starts_with(prefix) {
                return Err(ivm::VMError::NoritoInvalid);
            }
            if emit(key, present)?.is_break() {
                return Ok(ControlFlow::Break(()));
            }
        }
        let present = local.take_equal(live_key).unwrap_or(true);
        if emit(live_key, present)?.is_break() {
            return Ok(ControlFlow::Break(()));
        }
        previous_live = Some(live_key.clone());
        Ok(ControlFlow::Continue(()))
    };
    let flow = visit_live(&mut merge_live)?;
    if flow.is_break() {
        return Ok(());
    }
    while let Some((key, present)) = local.next() {
        if !key.as_ref().starts_with(prefix) {
            break;
        }
        if emit(key, present)?.is_break() {
            break;
        }
    }
    Ok(())
}
#[cfg(test)]
mod durable_state_merge_tests {
    use super::*;
    use iroha_model_base::state_path::MAX_STATE_PATH_BYTES;
    use iroha_test_samples::ALICE_ID;
    #[test]
    fn bounded_merge_seeks_after_deleted_positions_and_stops_without_lookahead() {
        use std::ops::ControlFlow;
        let base = (0..160)
            .map(|index| (state_path(&format!("orders/{index:04}")), vec![1]))
            .collect::<BTreeMap<_, _>>();
        let overlay = base
            .keys()
            .take(80)
            .cloned()
            .map(|key| (key, None))
            .collect::<BTreeMap<_, Option<Vec<u8>>>>();
        let after = "orders/0015";
        let mut visited_live = 0;
        let mut observed = Vec::new();
        visit_bounded_durable_state_positions(
            &base,
            &overlay,
            "orders/",
            Some(after),
            |visitor| {
                for (key, _) in base.range::<str, _>((Bound::Excluded(after), Bound::Unbounded)) {
                    visited_live += 1;
                    if visitor(key)?.is_break() {
                        return Ok(ControlFlow::Break(()));
                    }
                }
                Ok(ControlFlow::Continue(()))
            },
            &mut |key, present| {
                observed.push((key.clone(), present));
                Ok(if observed.len() == 64 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            },
        )
        .unwrap();
        assert_eq!(visited_live, 64);
        assert_eq!(observed.len(), 64);
        assert_eq!(observed.first().unwrap().0.as_ref(), "orders/0016");
        assert_eq!(observed.last().unwrap().0.as_ref(), "orders/0079");
        assert!(observed.iter().all(|(_, present)| !present));
    }

    #[test]
    fn bounded_merge_reads_overlay_insertions_and_overrides_backing_presence() {
        use std::ops::ControlFlow;
        let base = BTreeMap::from([
            (state_path("orders/02"), vec![1]),
            (state_path("orders/04"), vec![1]),
        ]);
        let overlay = BTreeMap::from([
            (state_path("orders/01"), Some(vec![2])),
            (state_path("orders/02"), None),
            (state_path("orders/03"), Some(vec![3])),
        ]);
        let mut observed = Vec::new();
        visit_bounded_durable_state_positions(
            &base,
            &overlay,
            "orders/",
            None,
            |visitor| {
                for key in [state_path("orders/02"), state_path("orders/05")] {
                    if visitor(&key)?.is_break() {
                        return Ok(ControlFlow::Break(()));
                    }
                }
                Ok(ControlFlow::Continue(()))
            },
            &mut |key, present| {
                observed.push((key.as_ref().to_owned(), present));
                Ok(ControlFlow::Continue(()))
            },
        )
        .unwrap();
        assert_eq!(
            observed,
            vec![
                ("orders/01".into(), true),
                ("orders/02".into(), false),
                ("orders/03".into(), true),
                ("orders/04".into(), true),
                ("orders/05".into(), true)
            ]
        );
    }

    fn state_path(value: &str) -> StatePath {
        value.parse().expect("durable-state test path")
    }
    #[test]
    fn streaming_merge_applies_overlay_precedence_and_canonical_interleaving() {
        let base = BTreeMap::from([
            (state_path("state/b"), vec![1]),
            (state_path("state/d"), vec![1]),
            (state_path("state/f"), vec![1]),
        ]);
        let overlay = BTreeMap::from([
            (state_path("state/b"), Some(vec![2])),
            (state_path("state/c"), Some(vec![2])),
            (state_path("state/d"), None),
            (state_path("state/e"), Some(vec![2])),
        ]);
        let live = [
            state_path("state/a"),
            state_path("state/b"),
            state_path("state/d"),
            state_path("state/g"),
        ];
        let mut emitted = Vec::new();
        visit_merged_durable_state_keys(
            &base,
            &overlay,
            "state/",
            |visitor| live.iter().try_for_each(visitor),
            &mut |key, present| {
                emitted.push((key.to_string(), present));
                Ok(())
            },
        )
        .expect("merge ordered sources");
        assert_eq!(
            emitted,
            [
                ("state/a".to_owned(), true),
                ("state/b".to_owned(), true),
                ("state/c".to_owned(), true),
                ("state/d".to_owned(), false),
                ("state/e".to_owned(), true),
                ("state/f".to_owned(), true),
                ("state/g".to_owned(), true),
            ]
        );
    }
    #[test]
    fn streaming_merge_rejects_nonmonotonic_or_out_of_prefix_live_visitors() {
        let base = BTreeMap::new();
        let overlay = BTreeMap::new();
        for live in [
            vec![state_path("state/b"), state_path("state/a")],
            vec![state_path("state/a"), state_path("other/a")],
        ] {
            assert!(matches!(
                visit_merged_durable_state_keys(
                    &base,
                    &overlay,
                    "state/",
                    |visitor| live.iter().try_for_each(visitor),
                    &mut |_, _| Ok(()),
                ),
                Err(ivm::VMError::NoritoInvalid)
            ));
        }
    }
    #[test]
    fn production_count_merges_live_values_and_tombstones() {
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.durable_state_base = BTreeMap::from([
            (state_path("state/a"), vec![1]),
            (state_path("state/b"), vec![1]),
        ]);
        host.durable_state_overlay = BTreeMap::from([
            (state_path("state/b"), None),
            (state_path("state/c"), Some(vec![3])),
        ]);
        let prefix = state_path("state");
        let path_len = norito::to_bytes(&prefix).expect("encode prefix").len();
        let vm = IVM::new(u64::MAX);
        let (total, _) = host
            .count_durable_state_keys(&vm, &prefix, path_len)
            .expect("count without materializing keys");
        assert_eq!(total, 2);
    }
    #[test]
    fn exact_state_path_storage_bound_remains_scannable() {
        let mut host = CoreHost::new(ALICE_ID.clone());
        let stem = "state/";
        let maximum = state_path(&format!(
            "{stem}{}",
            "x".repeat(MAX_STATE_PATH_BYTES - stem.len())
        ));
        assert_eq!(maximum.as_ref().len(), MAX_STATE_PATH_BYTES);
        assert!(
            format!("{maximum}x").parse::<StatePath>().is_err(),
            "StatePath must reject a key beyond its UTF-8 byte ceiling"
        );
        host.durable_state_base.insert(maximum, vec![1]);
        let prefix = state_path("state");
        let path_len = norito::to_bytes(&prefix).expect("encode prefix").len();
        let vm = IVM::new(u64::MAX);
        let (total, _) = host
            .count_durable_state_keys(&vm, &prefix, path_len)
            .expect("the exact StatePath ceiling remains a valid scan candidate");
        assert_eq!(total, 1);
    }
    #[test]
    fn maximum_name_base_and_four_kibibyte_map_key_roundtrip_through_core_storage() {
        let base: Name = "b"
            .repeat(ivm::syscalls::STATE_MAP_MAX_BASE_BYTES)
            .parse()
            .expect("maximum Name base");
        let key = vec![0xa5; ivm::syscalls::STATE_MAP_MAX_KEY_BYTES];
        let path = ivm::host::canonical_state_map_path(&base, &key).expect("maximum StateMap path");
        assert_eq!(
            path.as_ref().len(),
            ivm::syscalls::STATE_MAP_MAX_BASE_BYTES
                + 1
                + 2 * ivm::syscalls::STATE_MAP_MAX_KEY_BYTES
        );
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.durable_state_base.insert(path.clone(), vec![1]);
        let prefix = StatePath::from(&base);
        let path_len = norito::to_bytes(&prefix).expect("encode map prefix").len();
        let vm = IVM::new(u64::MAX);
        let (total, _) = host
            .count_durable_state_keys(&vm, &prefix, path_len)
            .expect("scan maximum StateMap path");
        assert_eq!(total, 1);
    }
    #[test]
    fn scoped_path_and_scan_reject_physical_length_overflow_before_state_access() {
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &ALICE_ID,
            703,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive scoped-boundary contract");
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_subject: contract.subject_id(),
            contract_address: contract,
            contract_alias: None,
            entrypoint: "boundary".to_owned(),
        }));
        let scope_len = host
            .durable_state_scope_prefix()
            .expect("contract context has a scope")
            .len();
        let exact = state_path(&"x".repeat(MAX_STATE_PATH_BYTES - scope_len));
        let exact_physical = host
            .scoped_durable_state_path(&exact)
            .expect("exact physical boundary is valid")
            .expect("contract path is scoped");
        assert_eq!(exact_physical.as_ref().len(), MAX_STATE_PATH_BYTES);
        let oversized = state_path(&"x".repeat(MAX_STATE_PATH_BYTES - scope_len + 1));
        assert!(matches!(
            host.scoped_durable_state_path(&oversized),
            Err(ivm::VMError::NoritoInvalid)
        ));
        let path_len = norito::to_bytes(&oversized)
            .expect("encode logical prefix")
            .len();
        let vm = IVM::new(u64::MAX);
        assert!(
            matches!(
                host.count_durable_state_keys(&vm, &oversized, path_len),
                Err(ivm::VMError::NoritoInvalid)
            ),
            "scans must not turn an unrepresentable physical prefix into an empty success"
        );
    }
}
fn visit_storage_keys_with_text_prefix(
    storage: &impl StorageReadOnly<StatePath, Vec<u8>>,
    prefix: &str,
    visitor: &mut dyn FnMut(&StatePath) -> Result<(), ivm::VMError>,
) -> Result<(), ivm::VMError> {
    for (key, _) in storage.range::<str>((Bound::Included(prefix), Bound::Unbounded)) {
        if !key.as_ref().starts_with(prefix) {
            break;
        }
        visitor(key)?;
    }
    Ok(())
}
fn visit_storage_positions(
    storage: &impl StorageReadOnly<StatePath, Vec<u8>>,
    prefix: &str,
    after: Option<&str>,
    visitor: &mut dyn FnMut(&StatePath) -> Result<std::ops::ControlFlow<()>, ivm::VMError>,
) -> Result<std::ops::ControlFlow<()>, ivm::VMError> {
    let lower = after.map_or(Bound::Included(prefix), Bound::Excluded);
    for (key, _) in storage.range::<str>((lower, Bound::Unbounded)) {
        if !key.as_ref().starts_with(prefix) {
            break;
        }
        if visitor(key)?.is_break() {
            return Ok(std::ops::ControlFlow::Break(()));
        }
    }
    Ok(std::ops::ControlFlow::Continue(()))
}
impl QueryStateRefOps for QueryStateRef<'_, '_, '_> {
    fn vrf_epoch_seed_projection(&self, epoch_length: NonZeroU64) -> VrfEpochSeedProjection {
        match *self {
            QueryStateRef::View(view) => VrfEpochSeedProjection::scan(view, epoch_length),
            QueryStateRef::QueryView(view) => VrfEpochSeedProjection::scan(view, epoch_length),
            QueryStateRef::Block(block) => VrfEpochSeedProjection::scan(block, epoch_length),
            QueryStateRef::Transaction(transaction) => {
                VrfEpochSeedProjection::scan(transaction, epoch_length)
            }
        }
    }
    fn smart_contract_heap_limit(&self) -> u64 {
        match *self {
            QueryStateRef::View(view) => view.world().parameters().smart_contract().memory().get(),
            QueryStateRef::QueryView(view) => {
                view.world().parameters().smart_contract().memory().get()
            }
            QueryStateRef::Block(block) => {
                block.world().parameters().smart_contract().memory().get()
            }
            QueryStateRef::Transaction(transaction) => transaction
                .world()
                .parameters()
                .smart_contract()
                .memory()
                .get(),
        }
    }
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
    fn asset_balance_policy(
        &self,
        definition_id: &AssetDefinitionId,
    ) -> Result<AssetBalancePolicy, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => view
                .world()
                .asset_definition(definition_id)
                .map(|definition| definition.balance_scope_policy())
                .map_err(|_| ivm::VMError::DecodeError),
            QueryStateRef::QueryView(view) => view
                .world()
                .asset_definition(definition_id)
                .map(|definition| definition.balance_scope_policy())
                .map_err(|_| ivm::VMError::DecodeError),
            QueryStateRef::Block(block) => block
                .world()
                .asset_definition(definition_id)
                .map(|definition| definition.balance_scope_policy())
                .map_err(|_| ivm::VMError::DecodeError),
            QueryStateRef::Transaction(tx) => tx
                .world()
                .asset_definition(definition_id)
                .map(|definition| definition.balance_scope_policy())
                .map_err(|_| ivm::VMError::DecodeError),
        }
    }
    fn uid_holding_quote(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        account: &AccountId,
        incoming: &Quantity,
        budget: QueryExecutionBudget,
    ) -> Result<
        (
            iroha_data_model::asset::uid::AssetUidHoldingQuoteV1,
            QueryExecutionStats,
        ),
        ivm::VMError,
    > {
        self.read_uid_holding_quote(authority, context, definition, account, incoming, budget)
    }
    fn fi_link_revision(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        account: &AccountId,
        expected_fi: &AccountId,
        budget: QueryExecutionBudget,
    ) -> Result<(u64, QueryExecutionStats), ivm::VMError> {
        self.read_fi_link_revision(authority, context, definition, account, expected_fi, budget)
    }
    fn same_uid(
        &self,
        authority: &AccountId,
        context: &ContractRuntimeExecutionContext,
        definition: &AssetDefinitionId,
        source: &AccountId,
        destination: &AccountId,
        budget: QueryExecutionBudget,
    ) -> Result<(bool, QueryExecutionStats), ivm::VMError> {
        self.read_same_uid(authority, context, definition, source, destination, budget)
    }
    fn stable_axt_asset_incarnation(
        &self,
        definition_id: &AssetDefinitionId,
    ) -> Result<AxtAssetIncarnationV1, ivm::VMError> {
        let (current, before_block) = match *self {
            QueryStateRef::View(view) => {
                let current = view
                    .world()
                    .axt_asset_incarnations()
                    .get(definition_id)
                    .copied();
                (current, current)
            }
            QueryStateRef::QueryView(view) => {
                let current = view
                    .world()
                    .axt_asset_incarnations()
                    .get(definition_id)
                    .copied();
                (current, current)
            }
            QueryStateRef::Block(block) => (
                block
                    .world()
                    .axt_asset_incarnations()
                    .get(definition_id)
                    .copied(),
                block
                    .axt_asset_incarnation_at_block_start(definition_id)
                    .copied(),
            ),
            QueryStateRef::Transaction(tx) => (
                tx.world.axt_asset_incarnations.get(definition_id).copied(),
                tx.world
                    .axt_asset_incarnations
                    .get_before_block(definition_id)
                    .copied(),
            ),
        };
        let current = current.ok_or(ivm::VMError::PermissionDenied)?;
        current
            .validate()
            .map_err(|_| ivm::VMError::PermissionDenied)?;
        if before_block != Some(current) {
            return Err(ivm::VMError::PermissionDenied);
        }
        Ok(current)
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
    fn bound_contract_dispatch_identity_by_address(
        &self,
        contract_address: &ContractAddress,
    ) -> Result<
        Option<(
            crate::smartcontracts::code::BoundContractIdentity,
            AccountId,
            usize,
        )>,
        ivm::VMError,
    > {
        match *self {
            QueryStateRef::View(view) => {
                CoreHostImpl::<NoQueryState>::bound_contract_dispatch_identity(
                    view,
                    contract_address,
                )
            }
            QueryStateRef::QueryView(view) => {
                CoreHostImpl::<NoQueryState>::bound_contract_dispatch_identity(
                    view,
                    contract_address,
                )
            }
            QueryStateRef::Block(block) => {
                CoreHostImpl::<NoQueryState>::bound_contract_dispatch_identity(
                    block,
                    contract_address,
                )
            }
            QueryStateRef::Transaction(tx) => {
                CoreHostImpl::<NoQueryState>::bound_contract_dispatch_identity(tx, contract_address)
            }
        }
    }
    fn prepare_contract_cache_miss(
        &self,
        cache: &PreparedContractCache,
        artifact: iroha_data_model::smart_contract::ContractArtifactId,
    ) -> Result<ivm::PreparedContract, ivm::VMError> {
        let prepared = match *self {
            QueryStateRef::View(view) => {
                crate::smartcontracts::code::with_code_bytes(view, &artifact, |bytes| {
                    cache.get_or_prepare(artifact.code_hash, bytes)
                })
            }
            QueryStateRef::QueryView(view) => {
                crate::smartcontracts::code::with_code_bytes(view, &artifact, |bytes| {
                    cache.get_or_prepare(artifact.code_hash, bytes)
                })
            }
            QueryStateRef::Block(block) => {
                crate::smartcontracts::code::with_code_bytes(block, &artifact, |bytes| {
                    cache.get_or_prepare(artifact.code_hash, bytes)
                })
            }
            QueryStateRef::Transaction(tx) => {
                crate::smartcontracts::code::with_code_bytes(tx, &artifact, |bytes| {
                    cache.get_or_prepare(artifact.code_hash, bytes)
                })
            }
        };
        prepared
            .map_err(|error| error.into_vm_error(|_| ivm::VMError::PermissionDenied))?
            .ok_or(ivm::VMError::PermissionDenied)?
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
    fn enforce_named_contract_entrypoint_authorization(
        &self,
        authority: &AccountId,
        contract_address: &ContractAddress,
        entrypoint: &str,
        permission: &EntrypointAuthorizationV1,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        match *self {
            QueryStateRef::View(view) => {
                crate::executor::enforce_named_contract_entrypoint_authorization(
                    view.world(),
                    authority,
                    contract_address,
                    entrypoint,
                    permission,
                )
            }
            QueryStateRef::QueryView(view) => {
                crate::executor::enforce_named_contract_entrypoint_authorization(
                    view.world(),
                    authority,
                    contract_address,
                    entrypoint,
                    permission,
                )
            }
            QueryStateRef::Block(block) => {
                crate::executor::enforce_named_contract_entrypoint_authorization(
                    block.world(),
                    authority,
                    contract_address,
                    entrypoint,
                    permission,
                )
            }
            QueryStateRef::Transaction(tx) => {
                crate::executor::enforce_named_contract_entrypoint_authorization(
                    tx.world(),
                    authority,
                    contract_address,
                    entrypoint,
                    permission,
                )
            }
        }
    }
    fn ensure_contract_entrypoint_lifecycle(
        &self,
        contract_address: &ContractAddress,
        code_hash: Hash,
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
    ) -> Result<(), ValidationFail> {
        fn ensure_allowed_lifecycle(
            world: &impl WorldReadOnly,
            contract_address: &ContractAddress,
            code_hash: Hash,
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
            execution_height: u64,
        ) -> Result<(), ValidationFail> {
            crate::smartcontracts::code::ensure_contract_execution_allowed(
                world,
                contract_address,
                execution_height,
            )
            .map_err(ValidationFail::NotPermitted)?;
            crate::smartcontracts::code::ensure_contract_entrypoint_lifecycle(
                world,
                contract_address,
                code_hash,
                kind,
            )
        }
        match *self {
            QueryStateRef::View(view) => ensure_allowed_lifecycle(
                view.world(),
                contract_address,
                code_hash,
                kind,
                u64::try_from(view.height()).unwrap_or(u64::MAX),
            ),
            QueryStateRef::QueryView(view) => ensure_allowed_lifecycle(
                view.world(),
                contract_address,
                code_hash,
                kind,
                u64::try_from(view.height()).unwrap_or(u64::MAX),
            ),
            QueryStateRef::Block(block) => ensure_allowed_lifecycle(
                block.world(),
                contract_address,
                code_hash,
                kind,
                block._curr_block.height().get(),
            ),
            QueryStateRef::Transaction(tx) => ensure_allowed_lifecycle(
                tx.world(),
                contract_address,
                code_hash,
                kind,
                tx.block_height(),
            ),
        }
    }
    fn durable_state_get(&self, key: &StatePath) -> Option<Vec<u8>> {
        match *self {
            QueryStateRef::View(view) => view.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::QueryView(view) => view.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::Block(block) => block.world().smart_contract_state().get(key).cloned(),
            QueryStateRef::Transaction(tx) => tx.world().smart_contract_state().get(key).cloned(),
        }
    }
    fn durable_state_contains(&self, key: &StatePath) -> bool {
        match *self {
            QueryStateRef::View(view) => view.world().smart_contract_state().get(key).is_some(),
            QueryStateRef::QueryView(view) => {
                view.world().smart_contract_state().get(key).is_some()
            }
            QueryStateRef::Block(block) => block.world().smart_contract_state().get(key).is_some(),
            QueryStateRef::Transaction(tx) => tx.world().smart_contract_state().get(key).is_some(),
        }
    }
    fn durable_state_payload_len(&self, key: &StatePath) -> Result<Option<usize>, ivm::VMError> {
        let stored = match *self {
            QueryStateRef::View(view) => view.world().smart_contract_state().get(key),
            QueryStateRef::QueryView(view) => view.world().smart_contract_state().get(key),
            QueryStateRef::Block(block) => block.world().smart_contract_state().get(key),
            QueryStateRef::Transaction(tx) => tx.world().smart_contract_state().get(key),
        };
        stored
            .map(|stored| durable_state_payload_len(stored))
            .transpose()
    }
    fn visit_durable_state_positions(
        &self,
        prefix: &str,
        after: Option<&str>,
        visitor: &mut dyn FnMut(&StatePath) -> Result<std::ops::ControlFlow<()>, ivm::VMError>,
    ) -> Result<std::ops::ControlFlow<()>, ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => {
                visit_storage_positions(view.world().smart_contract_state(), prefix, after, visitor)
            }
            QueryStateRef::QueryView(view) => {
                visit_storage_positions(view.world().smart_contract_state(), prefix, after, visitor)
            }
            QueryStateRef::Block(block) => visit_storage_positions(
                block.world().smart_contract_state(),
                prefix,
                after,
                visitor,
            ),
            QueryStateRef::Transaction(tx) => {
                visit_storage_positions(tx.world().smart_contract_state(), prefix, after, visitor)
            }
        }
    }
    fn visit_durable_state_keys_with_text_prefix(
        &self,
        prefix: &str,
        visitor: &mut dyn FnMut(&StatePath) -> Result<(), ivm::VMError>,
    ) -> Result<(), ivm::VMError> {
        match *self {
            QueryStateRef::View(view) => visit_storage_keys_with_text_prefix(
                view.world().smart_contract_state(),
                prefix,
                visitor,
            ),
            QueryStateRef::QueryView(view) => visit_storage_keys_with_text_prefix(
                view.world().smart_contract_state(),
                prefix,
                visitor,
            ),
            QueryStateRef::Block(block) => visit_storage_keys_with_text_prefix(
                block.world().smart_contract_state(),
                prefix,
                visitor,
            ),
            QueryStateRef::Transaction(tx) => visit_storage_keys_with_text_prefix(
                tx.world().smart_contract_state(),
                prefix,
                visitor,
            ),
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
    queued: Vec<QueuedEffect>,
    entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    confidential_gas_delta: u64,
    completed_axt: Vec<axt::HostAxtState>,
    durable_state_overlay: DurableStateOverlay,
    durable_state_authorizations:
        BTreeMap<StatePath, Option<ContractEntrypointAuthorizationSnapshot>>,
}
/// One effect in the exact order produced by a contract and all nested calls.
pub(crate) struct QueuedEffect {
    pub(crate) payload: QueuedEffectPayload,
    pub(crate) authority: AccountId,
    pub(crate) contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    pub(crate) entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
}
/// Canonical effect owner. Emission backing and its original allocation charges
/// cannot be copied or separated while passing through execution and replay.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "The queued effect retains its original instruction or emission charges without an additional uncharged allocation"
)]
pub(crate) enum QueuedEffectPayload {
    Instruction(InstructionBox),
    Emission(contract_event::OwnedContractEmission),
}
impl QueuedEffect {
    pub(crate) fn instructions(effects: &[Self]) -> impl ExactSizeIterator<Item = &InstructionBox> {
        struct Instructions<'a> {
            effects: std::slice::Iter<'a, QueuedEffect>,
            remaining: usize,
        }
        impl<'a> Iterator for Instructions<'a> {
            type Item = &'a InstructionBox;
            fn next(&mut self) -> Option<Self::Item> {
                let instruction = self.effects.find_map(QueuedEffect::instruction)?;
                self.remaining -= 1;
                Some(instruction)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.remaining, Some(self.remaining))
            }
        }
        impl ExactSizeIterator for Instructions<'_> {}
        Instructions {
            effects: effects.iter(),
            remaining: effects
                .iter()
                .filter(|effect| effect.instruction().is_some())
                .count(),
        }
    }
    pub(crate) fn instruction(&self) -> Option<&InstructionBox> {
        match &self.payload {
            QueuedEffectPayload::Instruction(instruction) => Some(instruction),
            QueuedEffectPayload::Emission(_) => None,
        }
    }
    fn into_instruction(self) -> Option<InstructionBox> {
        match self.payload {
            QueuedEffectPayload::Instruction(instruction) => Some(instruction),
            QueuedEffectPayload::Emission(_) => None,
        }
    }
}
impl core::fmt::Debug for QueuedEffect {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QueuedEffect")
            .field("payload", &self.payload)
            .field("authority", &self.authority)
            .field(
                "has_contract_runtime_context",
                &self.contract_runtime_context.is_some(),
            )
            .field(
                "has_entrypoint_authorization",
                &self.entrypoint_authorization.is_some(),
            )
            .finish()
    }
}
impl PartialEq<InstructionBox> for QueuedEffect {
    fn eq(&self, other: &InstructionBox) -> bool {
        self.contract_runtime_context.is_none()
            && self.entrypoint_authorization.is_none()
            && self.instruction() == Some(other)
    }
}
#[derive(Clone)]
enum OriginalMapEntry<T> {
    Absent,
    Present(T),
}
#[derive(Clone)]
struct DurableStateOriginal {
    overlay: OriginalMapEntry<Option<Vec<u8>>>,
    authorization: OriginalMapEntry<Option<ContractEntrypointAuthorizationSnapshot>>,
}
#[derive(Default)]
struct NestedContractCallJournal {
    durable_state_originals: BTreeMap<StatePath, DurableStateOriginal>,
    new_read_keys: BTreeSet<String>,
    new_write_keys: BTreeSet<String>,
    new_durable_read_paths: BTreeSet<String>,
    state_writes_len: usize,
}
#[derive(Clone, Copy)]
enum NestedContractCallOutcome {
    Commit,
    Rollback,
    RollbackPreservingReads,
}
struct NestedContractCallHostSnapshot {
    authority: AccountId,
    execution_class: HostExecutionClass,
    local_debug_artifacts: bool,
    current_contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    current_entrypoint_authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    args: Option<iroha_primitives::json::Json>,
    entrypoint_argument_record: Option<ivm::PreparedArgumentRecord>,
    fastpq_batch_entries: Option<Vec<TransferAssetBatchEntry>>,
    default: ivm::host::DefaultHostForwardedCallCheckpoint,
    nft_seq: u64,
    queued_len: usize,
    journal_depth: usize,
    durable_read_paths_complete: bool,
    axt_state: Option<Arc<axt::HostAxtState>>,
    completed_axt_len: usize,
    zk_verified_ballot: Arc<VecDeque<[u8; 32]>>,
    zk_verified_tally: Arc<VecDeque<[u8; 32]>>,
    zk_last_env_hash_ballot: Arc<VecDeque<[u8; 32]>>,
    zk_last_env_hash_tally: Arc<VecDeque<[u8; 32]>>,
    axt_proof_cache_slot: Option<u64>,
    axt_proof_cache: Arc<BTreeMap<DataSpaceId, CachedProofEntry>>,
    last_axt_reject: Option<AxtRejectContext>,
    amx_budget_violation: Option<AmxBudgetViolation>,
    output_count_before: u64,
    output_bytes_before: u64,
    output_violation_before: Option<HostOutputBudgetViolation>,
}
impl HostExecutionArtifacts {
    fn validate_queued_authorization(
        world: &impl WorldReadOnly,
        queued: &QueuedEffect,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        Self::validate_effect_authorization(
            world,
            &queued.authority,
            queued.contract_runtime_context.as_ref(),
            queued.entrypoint_authorization.as_ref(),
        )
    }
    fn validate_effect_authorization(
        world: &impl WorldReadOnly,
        authority: &AccountId,
        runtime_context: Option<&ContractRuntimeExecutionContext>,
        authorization: Option<&ContractEntrypointAuthorizationSnapshot>,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        match (runtime_context, authorization) {
            (Some(context), Some(authorization)) => {
                let live_subject = crate::smartcontracts::code::bound_contract_subject_from_world(
                    world,
                    &context.contract_address,
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        context.contract_address
                    ))
                })?;
                if context.contract_subject != live_subject
                    || context.contract_address != authorization.contract_address
                    || context.contract_alias != authorization.contract_alias
                    || context.entrypoint != authorization.entrypoint
                    || *authority != context.contract_subject
                {
                    return Err(ValidationFail::NotPermitted(
                        "queued contract effect does not match its immutable authorization snapshot"
                            .to_owned(),
                    ).into());
                }
                authorization.validate(world)
            }
            (Some(_), None) => Err(ValidationFail::NotPermitted(
                "queued contract effect is missing its immutable authorization snapshot".to_owned(),
            )
            .into()),
            (None, Some(_)) => Err(ValidationFail::InternalError(
                "queued entrypoint authorization has no contract runtime context".to_owned(),
            )
            .into()),
            (None, None) => Ok(()),
        }
    }
    fn durable_path_requires_authorization(path: &StatePath) -> bool {
        let path = path.as_ref();
        path.starts_with("sc/")
            || path.starts_with(crate::smartcontracts::code::CONTRACT_LIFECYCLE_STATE_PREFIX)
                && path
                    .as_bytes()
                    .get(crate::smartcontracts::code::CONTRACT_LIFECYCLE_STATE_PREFIX.len())
                    == Some(&b'/')
    }
    fn validate_durable_authorizations(
        world: &impl WorldReadOnly,
        durable_state_overlay: &DurableStateOverlay,
        durable_state_authorizations: &BTreeMap<
            StatePath,
            Option<ContractEntrypointAuthorizationSnapshot>,
        >,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        if durable_state_overlay.len() != durable_state_authorizations.len()
            || !durable_state_overlay
                .keys()
                .eq(durable_state_authorizations.keys())
        {
            return Err(ValidationFail::InternalError(
                "durable state overlay authorization keys are structurally inconsistent".to_owned(),
            )
            .into());
        }
        for (path, authorization) in durable_state_authorizations {
            validate_reserve_durable_state_path(path)?;
            if Self::durable_path_requires_authorization(path) && authorization.is_none() {
                return Err(ValidationFail::NotPermitted(format!(
                    "scoped durable state path `{path}` is missing its contract authorization snapshot"
                )).into());
            }
            if let Some(authorization) = authorization {
                if !authorization.owns_durable_state_path(path) {
                    return Err(ValidationFail::NotPermitted(format!(
                        "durable state path `{path}` does not belong to its contract authorization snapshot"
                    )).into());
                }
                authorization.validate(world)?;
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn queued_instructions(&self) -> Vec<InstructionBox> {
        self.queued
            .iter()
            .filter_map(|queued| queued.instruction().cloned())
            .collect()
    }
    pub(crate) fn queued_instructions_by_authority(
        &self,
    ) -> BTreeMap<AccountId, Vec<InstructionBox>> {
        let mut grouped = BTreeMap::new();
        for queued in &self.queued {
            if let Some(instruction) = queued.instruction() {
                grouped
                    .entry(queued.authority.clone())
                    .or_insert_with(Vec::new)
                    .push(instruction.clone());
            }
        }
        grouped
    }
    /// Preserve the global queue order together with the authority snapshot for
    /// validation rules whose effect plan spans multiple source accounts.
    pub(crate) fn queued_instructions_with_authority(&self) -> Vec<(AccountId, InstructionBox)> {
        self.queued
            .iter()
            .filter_map(|queued| {
                queued
                    .instruction()
                    .map(|instruction| (queued.authority.clone(), instruction.clone()))
            })
            .collect()
    }
    fn seed_queued_call_hash_if_missing(
        tx: &mut StateTransaction<'_, '_>,
        queued: &[QueuedEffect],
    ) -> Result<(), ValidationFail> {
        if tx.tx_call_hash.is_some() || queued.is_empty() {
            return Ok(());
        }
        let mut bytes = Vec::from(&b"iroha:ivm:queued-call-hash:v1|"[..]);
        let execution_identity = tx.direct_execution_identity();
        bytes.extend_from_slice(execution_identity.as_ref());
        for queued in queued {
            let Some(instruction) = queued.instruction() else {
                return Err(ValidationFail::InternalError(
                    "contract emission is missing its invocation execution owner".into(),
                ));
            };
            let authority = encode_canonical_norito(&queued.authority).map_err(|error| {
                ValidationFail::InternalError(format!(
                    "failed to encode queued authority for call_hash seed: {error}"
                ))
            })?;
            let instruction = encode_canonical_norito(instruction).map_err(|error| {
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
    /// Materialize deterministically replayed AXT completions at the transaction's lane/height.
    ///
    /// Raw and proof-carrying IVM execution must use this same path so a verified replay cannot
    /// silently discard an AXT envelope or derive different persistence metadata.
    pub(crate) fn record_completed_axt_states(
        tx: &mut StateTransaction<'_, '_>,
        completed_axt: Vec<axt::HostAxtState>,
    ) -> Result<(), ValidationFail> {
        if completed_axt.is_empty() {
            return Ok(());
        }
        let lane = tx.current_lane_id.unwrap_or_else(|| LaneId::new(0));
        let commit_height = tx.block_height();
        for state in completed_axt {
            tx.record_axt_envelope(CoreHostImpl::<NoQueryState>::materialize_axt_record(
                &state,
                lane,
                commit_height,
            ))
            .map_err(ValidationFail::InstructionFailed)?;
        }
        Ok(())
    }
    pub(crate) fn apply_to_transaction(
        self,
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        self.apply_to_transaction_with_lifecycle(tx, authority, None)
    }
    pub(crate) fn apply_to_transaction_with_lifecycle(
        self,
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        lifecycle_completion: Option<(
            &ContractAddress,
            crate::smartcontracts::code::PendingContractLifecycle,
        )>,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        let has_contract_effect = lifecycle_completion.is_some()
            || self.queued.iter().any(|queued| {
                queued.contract_runtime_context.is_some()
                    || queued.entrypoint_authorization.is_some()
            })
            || self
                .durable_state_authorizations
                .values()
                .any(Option::is_some);
        let root_authorization = self.entrypoint_authorization.as_ref();
        if has_contract_effect && root_authorization.is_none() {
            return Err(ValidationFail::NotPermitted(
                "contract execution artifacts are missing their root authorization snapshot"
                    .to_owned(),
            ));
        }
        if let Some(root) = root_authorization {
            if !root.is_root() {
                return Err(ValidationFail::NotPermitted(
                    "contract execution root authorization contains a parent invocation".to_owned(),
                ));
            }
            root.validate_for_authority(&tx.world, authority)
                .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
            for authorization in self
                .queued
                .iter()
                .filter_map(|queued| queued.entrypoint_authorization.as_ref())
                .chain(
                    self.durable_state_authorizations
                        .values()
                        .filter_map(Option::as_ref),
                )
            {
                if !authorization.descends_from(root) {
                    return Err(ValidationFail::NotPermitted(
                        "contract effect authorization does not retain the root invocation chain"
                            .to_owned(),
                    ));
                }
            }
        }
        for queued in &self.queued {
            Self::validate_queued_authorization(&tx.world, queued)
                .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        }
        Self::validate_durable_authorizations(
            &tx.world,
            &self.durable_state_overlay,
            &self.durable_state_authorizations,
        )
        .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        crate::deferred_authority::reject_opaque_instruction_authority(
            self.queued.iter().filter_map(QueuedEffect::instruction),
            tx,
        )
        .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        // The actual consumed group must fit before its first call-hash,
        // confidential-work, instruction, AXT or durable-state effect is applied.
        if let Some((contract_address, pending)) = lifecycle_completion {
            crate::smartcontracts::code::validate_contract_lifecycle_state_completion(
                &tx.world,
                &tx.execution_budget(),
                contract_address,
                pending,
                &self.durable_state_overlay,
            )
            .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        }
        tx.admit_host_execution_effects(QueuedEffect::instructions(&self.queued))?;
        Self::seed_queued_call_hash_if_missing(tx, &self.queued)?;
        if self.confidential_gas_delta > 0 {
            tx.record_confidential_gas_delta(self.confidential_gas_delta);
        }
        let executor = tx.world.executor.clone();
        let mut executed = Vec::new();
        for queued in self.queued {
            let QueuedEffect {
                payload,
                authority: effect_authority,
                contract_runtime_context,
                entrypoint_authorization,
            } = queued;
            if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                authorization
                    .validate_for_authority(&tx.world, authority)
                    .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
            }
            Self::validate_effect_authorization(
                &tx.world,
                &effect_authority,
                contract_runtime_context.as_ref(),
                entrypoint_authorization.as_ref(),
            )
            .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
            match payload {
                QueuedEffectPayload::Instruction(instruction) => {
                    executor.execute_instruction_with_contract_runtime_context(
                        tx,
                        &effect_authority,
                        instruction.clone(),
                        contract_runtime_context.as_ref(),
                    )?;
                    executed.push(instruction);
                }
                QueuedEffectPayload::Emission(emission) => {
                    Self::validate_emission_provenance(
                        &tx.world,
                        &emission.value,
                        contract_runtime_context.as_ref(),
                        entrypoint_authorization.as_ref(),
                    )?;
                    tx.record_contract_emission(emission)?;
                }
            }
            // A leaf can revoke its own permission or alter its binding in its final effect. Check
            // the selected root and the just-executed leaf before advancing or committing any
            // other artifact.
            if let Some(authorization) = self.entrypoint_authorization.as_ref() {
                authorization
                    .validate_for_authority(&tx.world, authority)
                    .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
            }
            Self::validate_effect_authorization(
                &tx.world,
                &effect_authority,
                contract_runtime_context.as_ref(),
                entrypoint_authorization.as_ref(),
            )
            .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        }
        if let Some(authorization) = self.entrypoint_authorization.as_ref() {
            authorization
                .validate_for_authority(&tx.world, authority)
                .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        }
        // Revalidate once after all queued effects, but before the durable lifecycle tombstone is
        // committed. A helper contract can otherwise stage a deactivate/reactivate ABA while a
        // hajimari or kaizen hook is still running.
        if let Some((contract_address, pending)) = lifecycle_completion {
            crate::smartcontracts::code::validate_contract_lifecycle_state_completion(
                &tx.world,
                &tx.execution_budget(),
                contract_address,
                pending,
                &self.durable_state_overlay,
            )
            .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        }
        // Queued effects may revoke the selected permission or change a contract binding. Check
        // every durable snapshot as a set before recording any additional artifact or writing the
        // first key, so a stale nested snapshot cannot cause a partial durable commit.
        Self::validate_durable_authorizations(
            &tx.world,
            &self.durable_state_overlay,
            &self.durable_state_authorizations,
        )
        .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
        Self::record_completed_axt_states(tx, self.completed_axt)?;
        if !self.durable_state_overlay.is_empty() {
            for (path, value) in self.durable_state_overlay {
                if let Some(authorization) = self
                    .durable_state_authorizations
                    .get(&path)
                    .and_then(Option::as_ref)
                {
                    authorization
                        .validate(&tx.world)
                        .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
                }
                if let Some(stored) = value {
                    tx.world.smart_contract_state.insert(path.clone(), stored);
                } else {
                    tx.world.smart_contract_state.remove(path.clone());
                }
            }
        }
        Ok(executed)
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
impl<QS: Default + QueryStateAccess> CoreHostImpl<'_, '_, QS> {
    fn decode_escrow_id(vm: &IVM, ptr: u64) -> Result<EscrowId, ivm::VMError> {
        let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
        Ok(Self::escrow_id_from_name(&name))
    }
    fn decode_optional_evidence_hashes(vm: &IVM, ptr: u64) -> Result<Vec<Hash>, ivm::VMError> {
        if ptr == 0 {
            return Ok(Vec::new());
        }
        let tlv = Self::decode_pointer_tlv(vm, ptr, PointerType::NoritoBytes)?;
        decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)
    }
    fn soracloud_operation_for_syscall(number: u32) -> Option<SoracloudHostOperationV1> {
        match number {
            ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE => {
                Some(SoracloudHostOperationV1::ReadCommittedState)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION => {
                Some(SoracloudHostOperationV1::EmitStateMutation)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE => {
                Some(SoracloudHostOperationV1::EmitMailboxMessage)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_APPEND_JOURNAL => {
                Some(SoracloudHostOperationV1::AppendJournal)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT => {
                Some(SoracloudHostOperationV1::PublishCheckpoint)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG => {
                Some(SoracloudHostOperationV1::ReadConfig)
            }
            ivm::syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => {
                Some(SoracloudHostOperationV1::ReadSecretEnvelope)
            }
            _ => None,
        }
    }
    fn soracloud_payload_matches_operation(
        payload: &SoracloudHostRequestPayloadV1,
        operation: SoracloudHostOperationV1,
    ) -> bool {
        matches!(
            (operation, payload),
            (
                SoracloudHostOperationV1::ReadCommittedState,
                SoracloudHostRequestPayloadV1::ReadCommittedState(_)
            ) | (
                SoracloudHostOperationV1::EmitStateMutation,
                SoracloudHostRequestPayloadV1::EmitStateMutation(_)
            ) | (
                SoracloudHostOperationV1::EmitMailboxMessage,
                SoracloudHostRequestPayloadV1::EmitMailboxMessage(_)
            ) | (
                SoracloudHostOperationV1::AppendJournal,
                SoracloudHostRequestPayloadV1::AppendJournal(_)
            ) | (
                SoracloudHostOperationV1::PublishCheckpoint,
                SoracloudHostRequestPayloadV1::PublishCheckpoint(_)
            ) | (
                SoracloudHostOperationV1::ReadConfig,
                SoracloudHostRequestPayloadV1::ReadConfig(_)
            ) | (
                SoracloudHostOperationV1::ReadSecretEnvelope,
                SoracloudHostRequestPayloadV1::ReadSecretEnvelope(_)
            )
        )
    }
    fn reject_soracloud_syscall(vm: &IVM, number: u32) -> Result<u64, ivm::VMError> {
        let expected = Self::soracloud_operation_for_syscall(number)
            .ok_or(ivm::VMError::UnknownSyscall(number))?;
        let tlv = Self::decode_pointer_tlv(vm, vm.register(10), PointerType::SoracloudRequest)?;
        let request: SoracloudHostRequestEnvelopeV1 =
            decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)?;
        request.validate().map_err(|_| ivm::VMError::DecodeError)?;
        if request.operation != expected
            || !Self::soracloud_payload_matches_operation(&request.payload, expected)
        {
            return Err(ivm::VMError::DecodeError);
        }
        Err(ivm::VMError::metered_not_implemented(
            ivm::gas::syscall_byte_gas(ivm::gas::G_SORACLOUD, tlv.payload.len(), 0),
            number,
        ))
    }
    /// Create a new host for the given authority.
    pub fn new(authority: AccountId) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let default = ivm::host::DefaultHost::new();
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            execution_class: HostExecutionClass::Generic,
            local_debug_artifacts: false,
            current_contract_runtime_context: None,
            current_entrypoint_authorization: None,
            attachment_custody: None,
            nested_contract_call_depth: 0,
            nested_contract_ancestors: std::array::from_fn(|_| None),
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            zk_verify_limits: ivm::host::ZkVerifyLimits::default(),
            zk_gas_schedule: ivm::gas::ZkGasScheduleV1::default(),
            confidential_gas: crate::gas::DEFAULT_ZK_GAS,
            stark_config: iroha_config::parameters::actual::Stark::default(),
            pipa_r_config: iroha_config::parameters::actual::PipaR::default(),
            crypto,
            queued: Vec::new(),
            instruction_queue_limits: HostOutputLimits::default(),
            instruction_queue_count: 0,
            instruction_queue_encoded_bytes: 0,
            instruction_queue_violation: None,
            fastpq_batch_entries: None,
            accounts_snapshot: Arc::new(Vec::new()),
            bound_contract_records_by_subject: BTreeMap::new(),
            prepared_contract_cache: PreparedContractCache::default(),
            query_state: QS::default(),
            core_query_page_metrics: None,
            nft_seq: 0,
            args: None,
            entrypoint_argument_record: None,
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            nested_contract_call_journals: Vec::new(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_ballot: Arc::new(VecDeque::new()),
            zk_verified_tally: Arc::new(VecDeque::new()),
            zk_roots: Arc::default(),
            zk_elections: Arc::default(),
            vrf_epoch_seeds: VrfEpochSeedSource::default(),
            prepared_verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            network_id: None,
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_policy_hook: None,
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_tree_roots_history_len:
                iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            zk_last_env_hash_ballot: Arc::new(VecDeque::new()),
            zk_last_env_hash_tally: Arc::new(VecDeque::new()),
            axt_policy_snapshot: None,
            axt_issuer_keys: Arc::new(BTreeMap::new()),
            axt_proof_cache_slot: None,
            axt_proof_cache: Arc::new(BTreeMap::new()),
            last_axt_reject: None,
            amx_analysis: None,
            amx_limits: AmxLimits::default(),
            amx_budget_violation: None,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }
    /// Enable and reset typed-query performance counters.
    ///
    /// Counters are disabled by default and do not affect consensus execution.
    pub fn enable_core_query_page_metrics(&mut self) {
        self.core_query_page_metrics = Some(CoreQueryPageMetrics::default());
    }
    /// Reset enabled typed-query performance counters.
    pub fn reset_core_query_page_metrics(&mut self) {
        if let Some(metrics) = &mut self.core_query_page_metrics {
            *metrics = CoreQueryPageMetrics::default();
        }
    }
    /// Return enabled typed-query performance counters.
    #[must_use]
    pub const fn core_query_page_metrics(&self) -> Option<CoreQueryPageMetrics> {
        self.core_query_page_metrics
    }
    /// Construct a host from a state snapshot, hydrating config, ZK snapshots, and AXT policy.
    ///
    /// # Errors
    ///
    /// Returns [`CoreHostStateError`] when the state contains an inconsistent
    /// ZK registry, a non-canonical AXT policy snapshot, or an invalid contract registry.
    /// Local registry resource refusal retains its unfinished attempt in the error.
    pub fn from_state(
        authority: AccountId,
        state: &crate::state::State,
    ) -> Result<Self, CoreHostStateError> {
        let view = state.view();
        let mut host = Self::new(authority);
        host.set_crypto_config(Arc::clone(&view.crypto));
        host.set_zk_config(&view.zk);
        host.set_chain_id(&view.chain_id);
        host.set_block_height(u64::try_from(view.height()).unwrap_or(u64::MAX));
        host.hydrate_axt_state(&view)
            .map_err(CoreHostStateError::AxtPolicySnapshot)?;
        host.set_durable_state_snapshot_from_world(view.world());
        host.set_output_limits_from_parameters(view.world().parameters().smart_contract());
        host.set_public_inputs_from_parameters(view.world().parameters());
        host.set_vrf_epoch_seeds_from_state(&view)
            .map_err(CoreHostStateError::NposPolicy)?;
        host.set_bound_contract_records_by_subject_snapshot(
            crate::smartcontracts::code::snapshot_bound_contract_records_by_subject(&view)
                .map_err(CoreHostStateError::ContractRegistry)?,
        );
        host.set_zk_snapshots_from_state(&view)
            .map_err(CoreHostStateError::ZkSnapshot)?;
        host.set_amx_limits(Self::amx_limits_from_config(&view.pipeline));
        Ok(host)
    }
    /// Create host with an account snapshot for iteration helpers.
    pub fn with_accounts(authority: AccountId, accounts: Arc<Vec<AccountId>>) -> Self {
        let default_crypto = iroha_config::parameters::actual::Crypto::default();
        let default = ivm::host::DefaultHost::new();
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            execution_class: HostExecutionClass::Generic,
            local_debug_artifacts: false,
            current_contract_runtime_context: None,
            current_entrypoint_authorization: None,
            attachment_custody: None,
            nested_contract_call_depth: 0,
            nested_contract_ancestors: std::array::from_fn(|_| None),
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            zk_verify_limits: ivm::host::ZkVerifyLimits::default(),
            zk_gas_schedule: ivm::gas::ZkGasScheduleV1::default(),
            confidential_gas: crate::gas::DEFAULT_ZK_GAS,
            stark_config: iroha_config::parameters::actual::Stark::default(),
            pipa_r_config: iroha_config::parameters::actual::PipaR::default(),
            crypto,
            queued: Vec::new(),
            instruction_queue_limits: HostOutputLimits::default(),
            instruction_queue_count: 0,
            instruction_queue_encoded_bytes: 0,
            instruction_queue_violation: None,
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            bound_contract_records_by_subject: BTreeMap::new(),
            prepared_contract_cache: PreparedContractCache::default(),
            query_state: QS::default(),
            core_query_page_metrics: None,
            nft_seq: 0,
            args: None,
            entrypoint_argument_record: None,
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            nested_contract_call_journals: Vec::new(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_ballot: Arc::new(VecDeque::new()),
            zk_verified_tally: Arc::new(VecDeque::new()),
            zk_roots: Arc::default(),
            zk_elections: Arc::default(),
            vrf_epoch_seeds: VrfEpochSeedSource::default(),
            prepared_verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            network_id: None,
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_policy_hook: None,
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_tree_roots_history_len:
                iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            zk_last_env_hash_ballot: Arc::new(VecDeque::new()),
            zk_last_env_hash_tally: Arc::new(VecDeque::new()),
            axt_policy_snapshot: None,
            axt_issuer_keys: Arc::new(BTreeMap::new()),
            axt_proof_cache_slot: None,
            axt_proof_cache: Arc::new(BTreeMap::new()),
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
        let default = ivm::host::DefaultHost::new();
        apply_sm_openssl_preview(default_crypto.enable_sm_openssl_preview);
        let crypto = Arc::new(default_crypto);
        Self {
            authority,
            execution_class: HostExecutionClass::Generic,
            local_debug_artifacts: false,
            current_contract_runtime_context: None,
            current_entrypoint_authorization: None,
            attachment_custody: None,
            nested_contract_call_depth: 0,
            nested_contract_ancestors: std::array::from_fn(|_| None),
            default,
            codec_host: IvmCodecHost::new(),
            access_log_enabled: false,
            zk_verify_limits: ivm::host::ZkVerifyLimits::default(),
            zk_gas_schedule: ivm::gas::ZkGasScheduleV1::default(),
            confidential_gas: crate::gas::DEFAULT_ZK_GAS,
            stark_config: iroha_config::parameters::actual::Stark::default(),
            pipa_r_config: iroha_config::parameters::actual::PipaR::default(),
            crypto,
            queued: Vec::new(),
            instruction_queue_limits: HostOutputLimits::default(),
            instruction_queue_count: 0,
            instruction_queue_encoded_bytes: 0,
            instruction_queue_violation: None,
            fastpq_batch_entries: None,
            accounts_snapshot: accounts,
            bound_contract_records_by_subject: BTreeMap::new(),
            prepared_contract_cache: PreparedContractCache::default(),
            query_state: QS::default(),
            core_query_page_metrics: None,
            nft_seq: 0,
            args: Some(args),
            entrypoint_argument_record: None,
            current_trigger_id: None,
            current_block_time_ms: None,
            current_block_height: None,
            durable_state_base: BTreeMap::new(),
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            state_access_log: ivm::host::AccessLog::default(),
            nested_contract_call_journals: Vec::new(),
            axt_state: None,
            completed_axt: Vec::new(),
            transport_caps_snapshot: None,
            negotiated_caps_snapshot: None,
            zk_verified_ballot: Arc::new(VecDeque::new()),
            zk_verified_tally: Arc::new(VecDeque::new()),
            zk_roots: Arc::default(),
            zk_elections: Arc::default(),
            vrf_epoch_seeds: VrfEpochSeedSource::default(),
            prepared_verifying_keys: BTreeMap::new(),
            public_inputs: BTreeMap::new(),
            chain_id_bytes: Vec::new(),
            network_id: None,
            axt_policy: Arc::new(ivm::axt::AllowAllAxtPolicy),
            axt_policy_hook: None,
            axt_timing: iroha_config::parameters::actual::NexusAxt::default(),
            current_manifest_id: None,
            zk_tree_roots_history_len:
                iroha_config::parameters::defaults::confidential::TREE_ROOTS_HISTORY_LEN,
            zk_last_env_hash_ballot: Arc::new(VecDeque::new()),
            zk_last_env_hash_tally: Arc::new(VecDeque::new()),
            axt_policy_snapshot: None,
            axt_issuer_keys: Arc::new(BTreeMap::new()),
            axt_proof_cache_slot: None,
            axt_proof_cache: Arc::new(BTreeMap::new()),
            last_axt_reject: None,
            amx_analysis: None,
            amx_limits: AmxLimits::default(),
            amx_budget_violation: None,
            #[cfg(feature = "telemetry")]
            telemetry: None,
        }
    }
    /// Create a host for an optional, validated contract-entrypoint record.
    pub fn with_accounts_and_argument_record(
        authority: AccountId,
        accounts: Arc<Vec<AccountId>>,
        record: Option<ivm::PreparedArgumentRecord>,
    ) -> Self {
        let mut host = Self::with_accounts_and_args(
            authority,
            accounts,
            iroha_primitives::json::Json::default(),
        );
        host.args = None;
        host.entrypoint_argument_record = record;
        host
    }
    /// Replace the prepared root argument record without changing the firing event JSON.
    pub fn set_entrypoint_argument_record(&mut self, record: Option<ivm::PreparedArgumentRecord>) {
        self.entrypoint_argument_record = record;
    }
    /// Enable the contract syscall surface for an unauthenticated local debug run.
    ///
    /// This mode deliberately leaves deployed-contract identity and authorization
    /// unset. Callers may inspect the resulting in-memory queue and durable-state
    /// overlay, but must not treat those local artifacts as authorized for commit.
    pub fn set_local_contract_debug_execution(&mut self) {
        self.clear_contract_runtime_binding();
        self.execution_class = HostExecutionClass::LocalContractDebug;
        self.local_debug_artifacts = true;
    }
    /// Enable the read-only contract syscall surface for an unauthenticated local debug run.
    ///
    /// State and ledger writes remain rejected, and deployed-contract identity and
    /// authorization remain unset. This is intended only for inspecting local views.
    pub fn set_local_contract_debug_view_execution(&mut self) {
        self.clear_contract_runtime_binding();
        self.execution_class = HostExecutionClass::LocalViewDebug;
        self.local_debug_artifacts = true;
    }
    /// Preserve the currently executing contract identity for queued runtime instructions.
    pub(crate) fn set_contract_runtime_context(
        &mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) {
        self.execution_class = if contract_runtime_context.is_some() {
            HostExecutionClass::Contract
        } else {
            HostExecutionClass::Generic
        };
        self.current_contract_runtime_context = contract_runtime_context;
        self.current_entrypoint_authorization = None;
    }
    /// Bind this host to an admitted contract-less program.
    ///
    /// The explicit execution class is defense in depth for the static generic
    /// syscall profile; absence of a contract context alone is not a safe
    /// discriminator because hosts are also assembled before contract binding.
    pub(crate) fn set_generic_execution(&mut self) {
        // A host may be pooled or reused after a contract/view run. Generic
        // execution must not inherit the prior contract subject as its effect
        // authority, even in release builds where a debug assertion would not
        // protect the boundary.
        self.clear_contract_runtime_binding();
    }
    /// Bind this host to a generic program running without a live state view.
    ///
    /// The generic ABI surface remains unchanged, but AXT is rejected by this
    /// execution class because its policy, replay ledger, and completion
    /// persistence all require one coherent world-state snapshot.
    pub(crate) fn set_state_free_generic_execution(&mut self) {
        self.clear_contract_runtime_binding();
        self.execution_class = HostExecutionClass::StateFreeGeneric;
    }
    /// Preserve the immutable dispatch authorization for every effect emitted by this frame.
    pub(crate) fn set_contract_entrypoint_authorization(
        &mut self,
        authorization: Option<ContractEntrypointAuthorizationSnapshot>,
    ) {
        if let (Some(context), Some(authorization)) = (
            self.current_contract_runtime_context.as_ref(),
            authorization.as_ref(),
        ) {
            debug_assert_eq!(context.contract_address, authorization.contract_address);
            debug_assert_eq!(context.contract_alias, authorization.contract_alias);
            debug_assert_eq!(context.entrypoint, authorization.entrypoint);
        }
        self.current_entrypoint_authorization = authorization;
    }
    /// Seed deployed contract records for nested resolution when no live query view is available.
    pub(crate) fn set_bound_contract_records_by_subject_snapshot(
        &mut self,
        snapshot: BTreeMap<AccountId, crate::smartcontracts::code::BoundContractRecord>,
    ) {
        self.bound_contract_records_by_subject = snapshot;
    }
    /// Stage successful consumption of a consensus-validated lifecycle transition.
    ///
    /// The caller validates the exact pending record before VM execution and again before live
    /// apply. Recording the physical marker as a durable read makes block-start overlays rebuild
    /// if another transaction consumes or replaces the transition first. The tombstone is only
    /// exported when the VM run succeeds and its host artifacts are drained.
    pub(crate) fn set_contract_lifecycle_transition(
        &mut self,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
        _transition: crate::smartcontracts::code::PendingContractLifecycle,
    ) {
        debug_assert_eq!(
            self.current_contract_runtime_context
                .as_ref()
                .map(|context| &context.contract_address),
            Some(contract_address),
            "lifecycle transition must belong to the bound runtime contract"
        );
        let marker = crate::smartcontracts::code::contract_lifecycle_state_key(contract_address);
        if self.access_log_enabled {
            self.insert_access_read_key(&(format!("lifecycle:{contract_address}")));
            self.insert_access_write_key(&(format!("lifecycle:{contract_address}")));
            self.insert_access_durable_read_path(marker.as_ref());
        }
        self.stage_durable_state_update(marker, None);
    }
    /// Bind an authenticated deployed runtime subject and its selected authorization snapshot.
    pub(crate) fn bind_contract_runtime_context(
        &mut self,
        contract_subject: AccountId,
        authorization: ContractEntrypointAuthorizationSnapshot,
    ) {
        self.execution_class = HostExecutionClass::Contract;
        self.current_contract_runtime_context = Some(ContractRuntimeExecutionContext {
            contract_address: authorization.contract_address.clone(),
            contract_subject,
            contract_alias: authorization.contract_alias.clone(),
            entrypoint: authorization.entrypoint.clone(),
        });
        self.current_entrypoint_authorization = Some(authorization);
    }
    fn clear_contract_runtime_binding(&mut self) {
        self.execution_class = HostExecutionClass::Generic;
        self.current_contract_runtime_context = None;
        self.current_entrypoint_authorization = None;
    }
    fn verified_deployed_contract_runtime_identity(
        state: &impl StateReadOnly,
        contract_address: &ContractAddress,
        expected_alias: Option<&ContractAlias>,
        contract: &ivm::PreparedContract,
    ) -> Result<
        (
            crate::smartcontracts::code::BoundContractIdentity,
            AccountId,
        ),
        crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
    > {
        let identity =
            crate::smartcontracts::code::fetch_bound_contract_identity(state, contract_address)?
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{contract_address}` has no valid live identity binding"
                    ))
                })?;
        if identity.contract_alias.as_ref() != expected_alias {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{contract_address}` alias changed before runtime binding"
            ))
            .into());
        }
        if identity.code_hash != contract.code_hash() {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{contract_address}` code binding changed before runtime binding"
            ))
            .into());
        }
        let contract_subject =
            crate::smartcontracts::code::fetch_bound_contract_subject(state, contract_address)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{contract_address}` has no valid subject binding"
                    ))
                })?;
        Ok((identity, contract_subject))
    }
    /// Revalidate and bind a transaction-capable deployed contract runtime against one immutable
    /// state view.
    ///
    /// App-facing simulation paths use this boundary instead of constructing runtime provenance
    /// from caller-supplied address, alias, code hash, subject, or permission fields. Read-only
    /// selectors are deliberately rejected here and must use the view-specific boundary below.
    /// A rejected rebind clears any earlier provenance so callers cannot accidentally execute
    /// under a stale authorization snapshot.
    ///
    /// # Errors
    /// Returns an error if the live identity, alias, subject, or code binding is absent or changed;
    /// if the selector is not a transaction-capable entrypoint; if its lifecycle is unavailable;
    /// or if the host authority lacks its state-derived permission. Resource refusal
    /// remains a local deferred attempt and never authorizes a partial binding.
    pub fn bind_authorized_deployed_contract_runtime_context(
        &mut self,
        state: &impl StateReadOnly,
        contract_address: &ContractAddress,
        expected_alias: Option<&ContractAlias>,
        contract: &ivm::PreparedContract,
        selector: &str,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        self.clear_contract_runtime_binding();
        let (identity, contract_subject) = Self::verified_deployed_contract_runtime_identity(
            state,
            contract_address,
            expected_alias,
            contract,
        )?;
        let authorization = crate::executor::authorize_prepared_contract_selector(
            state.world(),
            &self.authority,
            contract,
            selector,
            &identity,
        )?;
        let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
            ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
        })?;
        crate::smartcontracts::code::ensure_contract_entrypoint_lifecycle(
            state.world(),
            contract_address,
            identity.code_hash,
            descriptor.kind,
        )?;
        self.bind_contract_runtime_context(contract_subject, authorization);
        Ok(())
    }
    /// Revalidate and bind a read-only deployed contract view against one immutable state view.
    ///
    /// This boundary captures the same live identity and subject provenance as transaction
    /// simulation while requiring the selected artifact entrypoint to be declared as `view`.
    /// Rejection clears any previously bound runtime provenance.
    ///
    /// # Errors
    /// Returns an error if the live identity, alias, subject, or code binding is absent or changed;
    /// if the selector is not a read-only view; if the lifecycle blocks views; or if the host
    /// authority lacks its state-derived permission. Resource refusal remains a local
    /// deferred attempt and never authorizes a partial binding.
    pub fn bind_authorized_deployed_contract_view_runtime_context(
        &mut self,
        state: &impl StateReadOnly,
        contract_address: &ContractAddress,
        expected_alias: Option<&ContractAlias>,
        contract: &ivm::PreparedContract,
        selector: &str,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<ValidationFail>> {
        self.clear_contract_runtime_binding();
        let (identity, contract_subject) = Self::verified_deployed_contract_runtime_identity(
            state,
            contract_address,
            expected_alias,
            contract,
        )?;
        let authorization = crate::executor::authorize_prepared_contract_view_selector(
            state.world(),
            &self.authority,
            contract,
            selector,
            &identity,
        )?;
        crate::smartcontracts::code::ensure_contract_ready_for_view(
            state.world(),
            contract_address,
            identity.code_hash,
        )?;
        self.bind_contract_runtime_context(contract_subject, authorization);
        self.execution_class = HostExecutionClass::View;
        Ok(())
    }
    /// Share the outer runtime's bounded immutable contract-artifact cache.
    pub fn set_prepared_contract_cache(&mut self, cache: PreparedContractCache) {
        self.prepared_contract_cache = cache;
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
    /// Return the next deterministic NFT sequence after this host's completed execution.
    #[must_use]
    pub(crate) fn next_nft_sequence(&self) -> u64 {
        self.nft_seq
    }
    /// Update local implementation selection and cryptography telemetry.
    pub fn set_crypto_config(&mut self, crypto: Arc<iroha_config::parameters::actual::Crypto>) {
        self.crypto = crypto;
        apply_sm_openssl_preview(self.crypto.enable_sm_openssl_preview);
        self.notify_telemetry_crypto_config();
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Override the configured AXT timing (slot length + skew tolerance).
    ///
    /// A successful timing replacement aborts any active AXT envelope. The
    /// guest must issue `AXT_BEGIN` again before recording proofs under the new
    /// timing.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] without changing the host
    /// when an installed snapshot is no longer canonical.
    pub fn set_axt_timing(
        &mut self,
        timing: iroha_config::parameters::actual::NexusAxt,
    ) -> Result<(), AxtPolicySnapshotValidationError> {
        let policy = self
            .axt_policy_snapshot
            .as_ref()
            .map(|snapshot| {
                ivm::axt::SnapshotAxtPolicy::new_with_timing(
                    snapshot,
                    timing.slot_length_ms,
                    timing.max_clock_skew_ms,
                )
            })
            .transpose()?;
        self.axt_timing = timing;
        if let (Some(snapshot), Some(policy)) = (self.axt_policy_snapshot.clone(), policy) {
            self.install_validated_axt_policy_snapshot(&snapshot, policy);
        } else {
            self.abort_active_axt_envelope_for_policy_change();
            self.clear_axt_proof_cache();
        }
        self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
        Ok(())
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Set the AXT timing configuration and return the updated host (builder style).
    ///
    /// A successful timing replacement aborts any active AXT envelope.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] when an installed snapshot
    /// is no longer canonical.
    pub fn with_axt_timing(
        mut self,
        timing: iroha_config::parameters::actual::NexusAxt,
    ) -> Result<Self, AxtPolicySnapshotValidationError> {
        self.set_axt_timing(timing)?;
        Ok(self)
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Install a supplemental restrictive AXT policy hook.
    ///
    /// The mandatory snapshot-derived policy, issuer authentication, and replay
    /// checks remain authoritative. The hook can only add denials; it cannot
    /// authorize an operation rejected by the mandatory policy. Installing a
    /// hook aborts any active AXT envelope.
    #[must_use]
    pub fn with_axt_policy(mut self, policy_hook: Arc<dyn ivm::axt::AxtPolicy>) -> Self {
        self.abort_active_axt_envelope_for_policy_change();
        self.axt_policy_hook = Some(policy_hook);
        self.clear_axt_proof_cache();
        self
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Install the mandatory AXT policy from a Space Directory snapshot.
    ///
    /// A successful snapshot replacement aborts any active AXT envelope and
    /// retains an installed supplemental restrictive policy hook.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] when the snapshot is not
    /// canonically ordered or its version does not bind its exact entries.
    pub fn with_axt_policy_snapshot(
        mut self,
        snapshot: &AxtPolicySnapshot,
    ) -> Result<Self, AxtPolicySnapshotValidationError> {
        let policy = ivm::axt::SnapshotAxtPolicy::new_with_timing(
            snapshot,
            self.axt_timing.slot_length_ms,
            self.axt_timing.max_clock_skew_ms,
        )?;
        self.install_validated_axt_policy_snapshot(snapshot, policy);
        Ok(self)
    }
    fn install_validated_axt_policy_snapshot(
        &mut self,
        snapshot: &AxtPolicySnapshot,
        policy: ivm::axt::SnapshotAxtPolicy,
    ) {
        self.abort_active_axt_envelope_for_policy_change();
        self.axt_policy = Arc::new(policy);
        self.axt_policy_snapshot = Some(snapshot.clone());
        self.axt_issuer_keys = Arc::new(BTreeMap::new());
        self.clear_axt_proof_cache();
        self.axt_proof_cache_slot = Self::policy_current_slot(snapshot);
    }
    /// Install an issuer key for an explicitly constructed test policy.
    ///
    /// Production hosts obtain this map only through [`Self::hydrate_axt_state`].
    #[cfg(any(test, feature = "iroha-core-tests"))]
    #[must_use]
    pub fn with_axt_issuer_key_for_tests(
        mut self,
        dataspace: DataSpaceId,
        manifest_root: [u8; 32],
        issuer: UniversalAccountId,
        public_key: PublicKey,
    ) -> Self {
        Arc::make_mut(&mut self.axt_issuer_keys).insert(
            dataspace,
            AxtIssuerKeyBinding {
                manifest_root,
                issuer,
                public_key,
            },
        );
        self
    }
    /// Install an issuer key into an already constructed test host.
    ///
    /// Production hosts obtain this map only through [`Self::hydrate_axt_state`].
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn set_axt_issuer_key_for_tests(
        &mut self,
        dataspace: DataSpaceId,
        manifest_root: [u8; 32],
        issuer: UniversalAccountId,
        public_key: PublicKey,
    ) {
        Arc::make_mut(&mut self.axt_issuer_keys).insert(
            dataspace,
            AxtIssuerKeyBinding {
                manifest_root,
                issuer,
                public_key,
            },
        );
    }
    fn clear_axt_proof_cache(&mut self) {
        for (dsid, entry) in core::mem::take(Arc::make_mut(&mut self.axt_proof_cache)) {
            self.clear_axt_proof_cache_state(dsid, &entry);
        }
        self.axt_proof_cache_slot = None;
    }
    /// Drop an in-progress envelope when its proof-validation assumptions change.
    ///
    /// Proofs already recorded in the envelope were accepted under the previous
    /// policy and timing. Keeping them across a replacement would allow a
    /// mixed-policy envelope to commit.
    fn abort_active_axt_envelope_for_policy_change(&mut self) {
        self.axt_state = None;
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Refresh the active AXT policy snapshot without rebuilding the host.
    ///
    /// A successful refresh clears proof caches, aborts any active AXT
    /// envelope, and retains an installed supplemental restrictive policy
    /// hook. The guest must issue `AXT_BEGIN` again before recording proofs
    /// under the refreshed policy.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] without changing the host
    /// when the supplied snapshot is not canonical.
    pub fn refresh_axt_policy_snapshot(
        &mut self,
        snapshot: &AxtPolicySnapshot,
    ) -> Result<(), AxtPolicySnapshotValidationError> {
        let policy = ivm::axt::SnapshotAxtPolicy::new_with_timing(
            snapshot,
            self.axt_timing.slot_length_ms,
            self.axt_timing.max_clock_skew_ms,
        )?;
        self.install_validated_axt_policy_snapshot(snapshot, policy);
        self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
        Ok(())
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
    /// epoch and the current slot comes only from an authenticated block-header
    /// or snapshot-anchor timestamp. A committed view without such a timestamp
    /// cannot produce an AXT policy. When bindings exist without an active
    /// manifest we still emit a zeroed manifest root to ensure lane gating
    /// remains active.
    #[must_use]
    pub fn derive_axt_policy_snapshot_from_directory(
        state: &(impl StateReadOnly + ?Sized),
    ) -> Option<AxtPolicySnapshot> {
        use std::collections::{BTreeMap, btree_map::Entry};
        let mut lane_for_dataspace = BTreeMap::new();
        let nexus = state.nexus();
        let state_height = u64::try_from(state.height()).unwrap_or(u64::MAX);
        for lane in nexus.lane_catalog.lanes() {
            let Some(dataspace_id) =
                crate::state::nexus_active_lane_dataspace_at_height(lane.id, nexus, state_height)
            else {
                continue;
            };
            lane_for_dataspace.entry(dataspace_id).or_insert(lane.id);
        }
        if lane_for_dataspace.is_empty() {
            return None;
        }
        let mut bindings: BTreeMap<DataSpaceId, (AxtPolicyEntry, Option<u64>)> = BTreeMap::new();
        let current_slot = current_axt_slot_for_state(state)?;
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
                    active_handle_era: record
                        .lifecycle
                        .activated_epoch
                        .unwrap_or(record.manifest.activation_epoch),
                    next_handle_counter: 1,
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
                            active_handle_era: 0,
                            next_handle_counter: 0,
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
        let mut snapshot = AxtPolicySnapshot { version, entries };
        crate::state::project_axt_handle_counters(state.world(), &mut snapshot);
        Some(snapshot)
    }
    /// Load an AXT policy snapshot from cached WSV entries when available.
    #[must_use]
    pub fn axt_policy_snapshot_from_state(state: &impl StateReadOnly) -> Option<AxtPolicySnapshot> {
        let current_slot = current_axt_slot_for_state(state)?;
        let (mut snapshot, cache_event) = {
            let policies = state.world().axt_policies();
            if policies.is_empty() {
                (
                    Self::derive_axt_policy_snapshot_from_directory(state),
                    AxtPolicyCacheEvent::CacheMiss,
                )
            } else {
                let nexus = state.nexus();
                let state_height = u64::try_from(state.height()).unwrap_or(u64::MAX);
                let mut entries: Vec<_> = policies
                    .iter()
                    .filter_map(|(dsid, policy)| {
                        let active_dataspace = crate::state::nexus_active_lane_dataspace_at_height(
                            policy.target_lane,
                            nexus,
                            state_height,
                        )?;
                        if active_dataspace != *dsid {
                            return None;
                        }
                        let mut policy = *policy;
                        policy.current_slot = current_slot;
                        Some(AxtPolicyBinding {
                            dsid: *dsid,
                            policy,
                        })
                    })
                    .collect();
                entries.sort_by_key(|entry| entry.dsid);
                if entries.is_empty() {
                    (
                        Self::derive_axt_policy_snapshot_from_directory(state),
                        AxtPolicyCacheEvent::CacheMiss,
                    )
                } else {
                    let version = AxtPolicySnapshot::compute_version(&entries);
                    (
                        Some(AxtPolicySnapshot { version, entries }),
                        AxtPolicyCacheEvent::CacheHit,
                    )
                }
            }
        };
        if let Some(snapshot) = snapshot.as_mut() {
            crate::state::project_axt_handle_counters(state.world(), snapshot);
        }
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
    /// Configure STARK verification limits for forwarded `ZK_VERIFY` syscalls.
    pub fn set_stark_config(&mut self, cfg: &iroha_config::parameters::actual::Stark) {
        self.stark_config = *cfg;
    }
    /// Configure all ZK verification limits for forwarded `ZK_VERIFY` syscalls.
    pub fn set_zk_config(&mut self, cfg: &iroha_config::parameters::actual::Zk) {
        let schedule = ivm::gas::ZkGasScheduleV1::from_rates(
            cfg.gas.proof_base,
            cfg.gas.per_public_input,
            cfg.gas.per_proof_byte,
        );
        self.zk_gas_schedule = schedule;
        self.confidential_gas = cfg.gas;
        self.default.set_zk_gas_schedule(schedule);
        self.zk_verify_limits = ivm::host::ZkVerifyLimits {
            max_verify_batch: cfg.max_verify_batch,
            max_envelope_bytes: cfg
                .pipa_r
                .max_envelope_bytes
                .max(cfg.stark.max_envelope_bytes),
            max_proof_bytes: cfg.pipa_r.max_proof_bytes.max(cfg.stark.max_proof_bytes),
        };
        self.default.set_zk_verify_limits(self.zk_verify_limits);
        self.set_stark_config(&cfg.stark);
        self.pipa_r_config = cfg.pipa_r;
    }
    /// Return the immutable ZK gas schedule snapshot selected for this host.
    #[must_use]
    pub const fn zk_gas_schedule(&self) -> ivm::gas::ZkGasScheduleV1 {
        self.zk_gas_schedule
    }
    #[cfg(feature = "telemetry")]
    /// Attach a telemetry sink and report the local SM implementation selection.
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
    fn align_telemetry_snapshot(&mut self) {
        if let Some(telemetry) = self.telemetry.as_ref() {
            telemetry.set_sm_openssl_preview(self.crypto.enable_sm_openssl_preview);
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
        active_handle_era: Option<u64>,
        next_handle_counter: Option<u64>,
    ) {
        let detail = detail.into();
        let ctx = AxtRejectContext {
            reason,
            dataspace,
            lane,
            snapshot_version: self.current_axt_policy_version(),
            detail,
            active_handle_era,
            next_handle_counter,
        };
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry.as_ref() {
            if let (Some(lane), Some(snapshot_version)) = (lane, ctx.snapshot_version) {
                telemetry.note_axt_policy_reject(lane, reason, snapshot_version);
            }
            if let (Some(dsid), Some(lane), Some(active_handle_era), Some(next_handle_counter)) =
                (dataspace, lane, active_handle_era, next_handle_counter)
            {
                telemetry.set_axt_reject_hint(
                    dsid,
                    lane,
                    active_handle_era,
                    next_handle_counter,
                    reason,
                );
            }
        }
        iroha_logger::warn!(
            target = "iroha::axt",
            reason = reason.label(),
            snapshot_version = ?ctx.snapshot_version,
            ?dataspace,
            ?lane,
            detail = %ctx.detail,
            active_handle_era = ?ctx.active_handle_era,
            next_handle_counter = ?ctx.next_handle_counter,
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
            MemoryAccessViolation { .. } | MemoryOutOfBounds => "memory_violation",
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
    /// Set the human-readable chain label returned by the chain-id sysvar.
    pub fn set_chain_id(&mut self, chain: &iroha_model_base::chain::ChainId) {
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
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Install validated confidential-tree snapshots for state-read syscalls.
    ///
    /// # Errors
    ///
    /// Returns [`ivm::VMError::NoritoInvalid`] without replacing the current
    /// snapshots when any tree state is inconsistent with its persisted profile.
    pub fn set_zk_roots_snapshot(
        &mut self,
        map: &BTreeMap<AssetDefinitionId, crate::state::ZkAssetState>,
    ) -> Result<(), ivm::VMError> {
        self.zk_roots =
            crate::state::ivm_zk_roots::project(map).map_err(|_| ivm::VMError::NoritoInvalid)?;
        Ok(())
    }
    /// Hydrate the ABI-V1 `VRF_EPOCH_SEED` projection from finalized global pulses.
    ///
    /// The syscall name and response layout remain stable, but consensus
    /// commit/reveal records are retired. Only pulses at the exact configured
    /// epoch boundary are projected, after full public-session/signature and
    /// canonical block-anchor verification. Missing or conflicting evidence
    /// leaves that epoch absent so the syscall fails closed with `found=false`.
    ///
    /// The State checks run here. The final threshold signatures of an epoch's
    /// candidates are verified on that epoch's first `VRF_EPOCH_SEED` use, which
    /// observes exactly the seed eager verification would have projected.
    /// Hosts whose query state stays attached for the run should prefer
    /// [`CoreHostImpl::set_query_state_with_vrf_epoch_seeds`], which also
    /// defers the pulse scan.
    ///
    /// # Errors
    /// Returns the original execution-attempt error if committed `NPoS` parameters
    /// cannot be read or validated. Invalid or conflicting beacon evidence is
    /// instead omitted from the successful epoch projection.
    pub fn set_vrf_epoch_seeds_from_state(
        &mut self,
        state: &impl StateReadOnly,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<String>> {
        let projection = vrf_epoch_length_from_state(state)?
            .map_or_else(VrfEpochSeedProjection::default, |epoch_length| {
                VrfEpochSeedProjection::scan(state, epoch_length)
            });
        self.vrf_epoch_seeds = VrfEpochSeedSource::Scanned(projection);
        Ok(())
    }
    /// Return the `VRF_EPOCH_SEED` value of `epoch`, resolving it on first use.
    fn vrf_epoch_seed(&self, epoch: u64) -> Option<[u8; 32]> {
        match &self.vrf_epoch_seeds {
            VrfEpochSeedSource::Scanned(projection) => projection.seed(epoch),
            VrfEpochSeedSource::QueryState {
                epoch_length,
                projection,
            } => projection
                .get_or_init(|| {
                    self.query_state
                        .vrf_epoch_seed_state()
                        .map_or_else(VrfEpochSeedProjection::default, |state| {
                            state.vrf_epoch_seed_projection(*epoch_length)
                        })
                })
                .seed(epoch),
        }
    }
    /// Hydrate ZK state reads, reusing projections only while their source is unchanged.
    ///
    /// The State owns immutable election and confidential-root projections for
    /// each read view and untouched block. Election or tree edits bypass the
    /// matching cache, including edits in a child transaction; rollback restores
    /// access to the original projection. Elections, confidential trees and
    /// verifying-key entries retain their validation order.
    ///
    /// # Errors
    /// Rejects malformed election, tree or verifying-key state before replacing
    /// any existing host snapshot.
    pub(crate) fn set_zk_snapshots_from_state(
        &mut self,
        state: &impl StateReadOnly,
    ) -> Result<(), ivm::VMError> {
        let elections = state
            .ivm_elections_snapshot()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let roots = state
            .ivm_zk_roots_snapshot()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        self.install_zk_snapshots(state.world(), state.zk(), roots, elections)
    }
    /// Hydrate a standalone World fixture without an enclosing State cache.
    #[cfg(test)]
    pub(crate) fn set_zk_snapshots_from_world(
        &mut self,
        world: &impl WorldReadOnly,
        zk_cfg: &iroha_config::parameters::actual::Zk,
    ) -> Result<(), ivm::VMError> {
        let elections = crate::state::ivm_elections::project(world.elections())
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let roots = crate::state::ivm_zk_roots::project(world.zk_assets().iter())
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        self.install_zk_snapshots(world, zk_cfg, roots, elections)
    }
    /// Validate the verifying-key registry, then install the validated projections.
    fn install_zk_snapshots(
        &mut self,
        world: &impl WorldReadOnly,
        zk_cfg: &iroha_config::parameters::actual::Zk,
        roots: Arc<crate::state::IvmZkRootsSnapshot>,
        elections: Arc<crate::state::IvmElectionsSnapshot>,
    ) -> Result<(), ivm::VMError> {
        let prepared_verifying_keys =
            Self::prepared_verifying_key_index(world.verifying_keys().iter())?;
        self.prepared_verifying_keys = prepared_verifying_keys;
        self.set_zk_tree_roots_history_len(zk_cfg.tree_roots_history_len);
        self.zk_roots = roots;
        self.zk_elections = elections;
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
        self.durable_state_authorizations.clear();
    }
    /// Replace the durable smart-contract state snapshot with a prebuilt offline fixture.
    pub fn set_durable_state_snapshot(&mut self, snapshot: BTreeMap<StatePath, Vec<u8>>) {
        self.durable_state_base = snapshot;
        self.durable_state_overlay.clear();
        self.durable_state_authorizations.clear();
    }
    /// Set the human-readable chain label returned by the chain-id sysvar.
    pub fn set_chain_id_bytes(&mut self, chain_id: Vec<u8>) {
        self.chain_id_bytes = chain_id;
        self.default.set_chain_id_bytes(self.chain_id_bytes.clone());
    }
    /// Set the exact genesis-derived security domain used by signatures and VRF proofs.
    pub fn set_network_id(&mut self, network_id: NetworkId) {
        self.default.set_network_id(network_id);
        self.network_id = Some(network_id);
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Set the current manifest id for namespace binding.
    pub fn set_current_manifest_id(&mut self, manifest: Option<String>) {
        self.current_manifest_id = manifest;
    }
    /// Apply the active VK registry snapshot for verification-time binding.
    ///
    /// # Errors
    /// Returns an error when registry metadata or bounded backend material is
    /// non-canonical, or when a stored commitment does not match the key bytes.
    pub fn set_verifying_keys(
        &mut self,
        map: &BTreeMap<VerifyingKeyId, VerifyingKeyRecord>,
    ) -> Result<(), ivm::VMError> {
        self.prepared_verifying_keys = Self::prepared_verifying_key_index(map)?;
        Ok(())
    }
    /// Validate a verifying-key registry and index its prepared keys by commitment.
    ///
    /// Entries are checked in registry order and the first invalid entry or
    /// duplicate commitment rejects the complete registry.
    fn prepared_verifying_key_index<'a>(
        entries: impl IntoIterator<Item = (&'a VerifyingKeyId, &'a VerifyingKeyRecord)>,
    ) -> Result<BTreeMap<[u8; 32], PreparedVerifyingKey>, ivm::VMError> {
        let mut prepared_by_commitment = BTreeMap::new();
        for (id, rec) in entries {
            let prepared = Self::prepare_verifying_key(id, rec)?;
            if prepared_by_commitment
                .insert(rec.commitment, prepared)
                .is_some()
            {
                return Err(ivm::VMError::NoritoInvalid);
            }
        }
        Ok(prepared_by_commitment)
    }
    /// Validate one registry entry against the production verifier policy and
    /// prepare its backend material.
    ///
    /// A registry pair equal to one this process already validated, including
    /// every key byte, reuses the memoized result: validation is a pure
    /// function of that pair, so only the repeated hashing and backend
    /// preparation are skipped. A pair whose id, metadata or key bytes changed
    /// is validated again.
    fn prepare_verifying_key(
        id: &VerifyingKeyId,
        rec: &VerifyingKeyRecord,
    ) -> Result<PreparedVerifyingKey, ivm::VMError> {
        if let Some(prepared) = prepared_verifying_key_memo::lookup(id, rec) {
            return Ok(prepared);
        }
        // Early sanity: ensure commitments and backend labels match the
        // production no-trusted-setup verifier policy.
        if !id.is_portable_registry_id() {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let Some(registry_backend_tag) =
            crate::zk::verifier_backend_registry_tag_v1(id.backend.as_str())
        else {
            return Err(ivm::VMError::NoritoInvalid);
        };
        if registry_backend_tag != rec.backend {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if rec.commitment.iter().all(|byte| *byte == 0)
            || rec.public_inputs_schema_hash.iter().all(|byte| *byte == 0)
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        if rec.circuit_id.len() > iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES
            || !iroha_data_model::zk::open_verify_circuit_id_is_portable(&rec.circuit_id)
            || iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_namespace_v1(
                &rec.circuit_id,
            )
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let backend_label = Self::backend_label_for_record(id, rec);
        if !Self::production_backend_label_matches_record(&backend_label, rec.backend) {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let material =
            crate::zk::validate_and_prepare_verifying_key_record_v1(id, rec).map_err(|error| {
                match error {
                    crate::zk::VerificationAttemptError::Rejected(_) => ivm::VMError::NoritoInvalid,
                    crate::zk::VerificationAttemptError::Deferred(original) => {
                        ivm::VMError::VerificationDeferred(original)
                    }
                }
            })?;
        let prepared = PreparedVerifyingKey {
            record: Arc::new(rec.clone()),
            backend_label: Arc::from(backend_label),
            material,
        };
        prepared_verifying_key_memo::remember(id, &prepared);
        Ok(prepared)
    }
    fn backend_label_for_record(id: &VerifyingKeyId, rec: &VerifyingKeyRecord) -> String {
        if let Some(ref vk) = rec.key {
            return vk.backend.clone();
        }
        let registry_backend = id.backend.as_str();
        if !registry_backend.is_empty() {
            return registry_backend.to_string();
        }
        if let Some(prefix) = rec.circuit_id.split(':').next()
            && !prefix.is_empty()
        {
            return prefix.to_string();
        }
        match rec.backend {
            BackendTag::Stark => "stark",
            BackendTag::NativePipaRPasta => "pipa-r/pasta",
        }
        .to_string()
    }
    fn production_backend_label_matches_record(
        backend_label: &str,
        backend_tag: BackendTag,
    ) -> bool {
        crate::zk::verifier_backend_registry_tag_v1(backend_label)
            .is_some_and(|expected| expected == backend_tag)
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
            iroha_data_model::zk::BackendTag::NativePipaRPasta => {
                if payload_len > self.pipa_r_config.max_envelope_bytes {
                    return Err(ivm::host::ERR_ENVELOPE_SIZE);
                }
                if !self.pipa_r_config.enabled {
                    return Err(ivm::host::ERR_DISABLED);
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
        }
        Ok(())
    }
    fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
        let is_admissible = |circuit_id: &str| {
            circuit_id.len() <= iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES
                && iroha_data_model::zk::open_verify_circuit_id_is_portable(circuit_id)
                && !iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_namespace_v1(
                    circuit_id,
                )
        };
        if !is_admissible(record_id) || !is_admissible(env_id) {
            return false;
        }
        if crate::zk::verifier_backend_registry_tag_v1(backend)
            == Some(BackendTag::NativePipaRPasta)
        {
            return record_id == env_id
                && crate::zk::pipa_r_open_verify_circuit_id_matches_backend(backend, record_id);
        }
        crate::zk::is_stark_fri_v1_backend(backend) && record_id == env_id
    }

    fn load_vk_record_any_namespace(
        &self,
        vk_commitment: [u8; 32],
    ) -> Result<PreparedVerifyingKey, u64> {
        let prepared = self
            .prepared_verifying_keys
            .get(&vk_commitment)
            .cloned()
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        let active = self.current_block_height.map_or_else(
            || prepared.record.is_active(),
            |height| prepared.record.is_active_at(height),
        );
        if !active {
            return Err(ivm::host::ERR_VK_INACTIVE);
        }
        if !Self::production_backend_label_matches_record(
            prepared.backend_label.as_ref(),
            prepared.record.backend,
        ) {
            return Err(ivm::host::ERR_BACKEND);
        }
        Ok(prepared)
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
            let open: iroha_data_model::zk::StarkFriOpenProofV1 =
                decode_canonical_norito(&env.proof_bytes).map_err(|_| ivm::host::ERR_DECODE)?;
            if open.envelope_bytes.len() > self.stark_config.max_proof_bytes {
                return Err(ivm::host::ERR_PROOF_LEN);
            }
        } else if env.backend == BackendTag::NativePipaRPasta {
            if proof_len_bytes > self.pipa_r_config.max_proof_bytes {
                return Err(ivm::host::ERR_PROOF_LEN);
            }
        } else {
            return Err(ivm::host::ERR_BACKEND);
        }
        Ok(())
    }
    fn map_open_verify_envelope_validation_error(err: OpenVerifyEnvelopeValidationError) -> u64 {
        ivm::host::map_open_verify_validation_error(err)
    }
    fn curve_is_allowed(backend_label: &str, curve_label: &str) -> bool {
        if crate::zk::is_stark_fri_v1_backend(backend_label) {
            return curve_label == "goldilocks";
        }
        if crate::zk::verifier_backend_registry_tag_v1(backend_label)
            == Some(BackendTag::NativePipaRPasta)
        {
            return curve_label == "vesta";
        }
        false
    }
    fn enforce_zk_envelope_value_impl(
        &self,
        env: iroha_data_model::zk::OpenVerifyEnvelope,
        payload_len: usize,
        namespace: Option<&str>,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            PreparedVerifyingKey,
        ),
        u64,
    > {
        self.validate_envelope_header(&env, payload_len)?;
        let max_proof_bytes = match env.backend {
            BackendTag::Stark => self.stark_config.max_proof_bytes,
            BackendTag::NativePipaRPasta => self.pipa_r_config.max_proof_bytes,
        };
        env.validate_with_bounds(OpenVerifyEnvelopeBounds {
            max_proof_bytes,
            ..OpenVerifyEnvelopeBounds::default()
        })
        .map_err(Self::map_open_verify_envelope_validation_error)?;
        let prepared = self.load_vk_record_any_namespace(env.vk_hash)?;
        let vk_rec = prepared.record.as_ref();
        let backend_label = prepared.backend_label.as_ref();
        let expected_env_backend = crate::zk::verifier_backend_registry_tag_v1(backend_label)
            .ok_or(ivm::host::ERR_BACKEND)?;
        if env.backend != expected_env_backend {
            return Err(ivm::host::ERR_BACKEND);
        }
        if let Some(expected_namespace) = namespace
            && vk_rec.namespace != expected_namespace
        {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        if !Self::circuit_id_matches(backend_label, &vk_rec.circuit_id, &env.circuit_id) {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        vk_rec.key.as_ref().ok_or(ivm::host::ERR_VK_MISSING)?;
        let current_manifest = self.current_manifest_id.as_deref().unwrap_or("core");
        if vk_rec.owner_manifest_id.as_deref().unwrap_or("core") != current_manifest {
            return Err(ivm::host::ERR_NAMESPACE);
        }
        if !Self::curve_is_allowed(backend_label, &vk_rec.curve) {
            return Err(ivm::host::ERR_CURVE);
        }
        if !crate::zk::is_stark_fri_v1_backend(backend_label) {
            let Some(_fixed_k) = prepared
                .material
                .as_ref()
                .and_then(crate::zk::PreparedVerifyingKeyMaterialV1::ipa_k)
            else {
                return Err(ivm::host::ERR_DECODE);
            };
        }
        let schema_hash: [u8; 32] = Hash::new(&env.public_inputs).into();
        if schema_hash != vk_rec.public_inputs_schema_hash {
            return Err(ivm::host::ERR_VK_MISMATCH);
        }
        self.validate_proof_len(vk_rec, backend_label, &env)?;
        Ok((env, prepared))
    }
    #[cfg(test)]
    fn enforce_zk_envelope(
        &self,
        payload: &[u8],
        namespace: &str,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            PreparedVerifyingKey,
        ),
        u64,
    > {
        let env = ivm::host::decode_canonical_zk_envelope(payload).map_err(|error| {
            error
                .into_status()
                .expect("test fixture decode must complete locally")
        })?;
        self.enforce_zk_envelope_value_impl(env, payload.len(), Some(namespace))
    }
    fn enforce_zk_envelope_value(
        &self,
        env: iroha_data_model::zk::OpenVerifyEnvelope,
        payload_len: usize,
        namespace: &str,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            PreparedVerifyingKey,
        ),
        u64,
    > {
        self.enforce_zk_envelope_value_impl(env, payload_len, Some(namespace))
    }
    fn enforce_zk_envelope_any_namespace_value(
        &self,
        env: iroha_data_model::zk::OpenVerifyEnvelope,
        payload_len: usize,
    ) -> Result<
        (
            iroha_data_model::zk::OpenVerifyEnvelope,
            PreparedVerifyingKey,
        ),
        u64,
    > {
        self.enforce_zk_envelope_value_impl(env, payload_len, None)
    }
    fn zk_verify_guardrails(&self) -> crate::zk::ZkVerifyGuardrails {
        crate::zk::ZkVerifyGuardrails {
            pipa_r_enabled: self.pipa_r_config.enabled,
            pipa_r_max_envelope_bytes: self.pipa_r_config.max_envelope_bytes,
            pipa_r_max_proof_bytes: self.pipa_r_config.max_proof_bytes,

            stark_enabled: self.stark_config.enabled,
            stark_max_envelope_bytes: self.stark_config.max_envelope_bytes,
            stark_max_proof_bytes: self.stark_config.max_proof_bytes,
        }
    }
    fn verify_bound_envelope(
        &self,
        env: iroha_data_model::zk::OpenVerifyEnvelope,
        payload: &[u8],
        namespace: &str,
    ) -> Result<bool, crate::zk::VerificationAttemptError<u64>> {
        let (_env, prepared) = self.enforce_zk_envelope_value(env, payload.len(), namespace)?;
        let vk_box = prepared
            .record
            .key
            .as_ref()
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        let backend_label = prepared.backend_label.as_ref();
        let proof = ProofBox::new(backend_label.into(), payload.to_vec());
        let guardrails = self.zk_verify_guardrails();
        Ok(crate::zk::verify_backend_with_timing_guardrails(
            backend_label,
            &proof,
            Some(vk_box),
            guardrails,
        )
        .map_err(crate::zk::VerificationAttemptError::Deferred)?
        .ok)
    }
    fn verify_any_namespace_envelope(
        &self,
        env: iroha_data_model::zk::OpenVerifyEnvelope,
        payload: &[u8],
    ) -> Result<bool, crate::zk::VerificationAttemptError<u64>> {
        let (_env, prepared) = self.enforce_zk_envelope_any_namespace_value(env, payload.len())?;
        let vk_box = prepared
            .record
            .key
            .as_ref()
            .ok_or(ivm::host::ERR_VK_MISSING)?;
        let backend_label = prepared.backend_label.as_ref();
        let proof = ProofBox::new(backend_label.into(), payload.to_vec());
        let guardrails = self.zk_verify_guardrails();
        Ok(crate::zk::verify_backend_with_timing_guardrails(
            backend_label,
            &proof,
            Some(vk_box),
            guardrails,
        )
        .map_err(crate::zk::VerificationAttemptError::Deferred)?
        .ok)
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Install a validated read-only snapshot of elections for state-read syscalls.
    ///
    /// # Errors
    ///
    /// Returns [`ivm::VMError::NoritoInvalid`] without replacing the current snapshot when an
    /// election has a non-canonical selector, an out-of-range option count, or a mismatched tally
    /// length.
    pub fn set_zk_elections_snapshot(
        &mut self,
        map: BTreeMap<String, (u32, bool, Vec<u128>)>,
    ) -> Result<(), ivm::VMError> {
        Self::validate_zk_elections_snapshot(&map)?;
        self.zk_elections = Arc::new(map);
        Ok(())
    }
    /// Production hydration validates elections in [`crate::state::ivm_elections::project`].
    #[cfg(any(test, feature = "iroha-core-tests"))]
    fn validate_zk_elections_snapshot(
        map: &BTreeMap<String, (u32, bool, Vec<u128>)>,
    ) -> Result<(), ivm::VMError> {
        for (election_id, (options, _, tally)) in map {
            if !iroha_data_model::governance::is_valid_governance_selector_v1(election_id) {
                return Err(ivm::VMError::NoritoInvalid);
            }
            Self::validate_zk_election_tally(*options, tally)?;
        }
        Ok(())
    }
    fn validate_zk_election_tally(options: u32, tally: &[u128]) -> Result<(), ivm::VMError> {
        crate::state::ivm_elections::validate_tally(options, tally)
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }
    /// Configure the non-zero retained-root bound returned by `ZK_ROOTS_GET`.
    pub fn set_zk_tree_roots_history_len(&mut self, len: NonZeroUsize) {
        self.zk_tree_roots_history_len = len;
    }
    /// Drain queued ISIs collected during the last VM run.
    pub fn drain_instructions(&mut self) -> Vec<InstructionBox> {
        self.drain_queued_effects()
            .into_iter()
            .filter_map(QueuedEffect::into_instruction)
            .collect()
    }
    /// Configure the aggregate output budget from consensus parameters.
    ///
    /// Call this before execution. Every constructor starts with the same
    /// on-chain defaults so a missed configuration site remains bounded.
    pub fn set_output_limits_from_parameters(&mut self, parameters: SmartContractParameters) {
        self.instruction_queue_limits = HostOutputLimits::from_parameters(parameters);
        self.instruction_queue_count = 0;
        self.instruction_queue_encoded_bytes = 0;
        self.instruction_queue_violation = None;
    }
    /// Apply an additional local/transport cap without widening consensus limits.
    ///
    /// This is intended for read-only tooling and must be called before
    /// execution. Consensus validity never depends on this stricter cap.
    pub fn restrict_output_limits(&mut self, limits: HostOutputLimits) {
        self.instruction_queue_limits = self.instruction_queue_limits.restrict(limits);
        self.instruction_queue_count = 0;
        self.instruction_queue_encoded_bytes = 0;
        self.instruction_queue_violation = None;
    }
    /// Return the first output-budget violation observed by this host.
    #[must_use]
    pub const fn output_budget_violation(&self) -> Option<HostOutputBudgetViolation> {
        self.instruction_queue_violation
    }
    /// Number of items currently accounted against a configured output budget.
    #[must_use]
    pub const fn retained_output_items(&self) -> u64 {
        self.instruction_queue_count
    }
    /// Encoded/retained bytes currently accounted against a configured output budget.
    #[must_use]
    pub const fn retained_output_bytes(&self) -> u64 {
        self.instruction_queue_encoded_bytes
    }
    fn try_reserve_output(&mut self, items: usize, bytes: usize) -> bool {
        let limits = self.instruction_queue_limits;
        if self.instruction_queue_violation.is_some() {
            return false;
        }
        let items = u64::try_from(items).unwrap_or(u64::MAX);
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let attempted_items = self.instruction_queue_count.saturating_add(items);
        if attempted_items > limits.max_items {
            self.instruction_queue_violation = Some(HostOutputBudgetViolation::ItemCount {
                attempted: attempted_items,
                limit: limits.max_items,
            });
            return false;
        }
        let attempted_bytes = self.instruction_queue_encoded_bytes.saturating_add(bytes);
        if attempted_bytes > limits.max_bytes {
            self.instruction_queue_violation = Some(HostOutputBudgetViolation::EncodedBytes {
                attempted: attempted_bytes,
                limit: limits.max_bytes,
            });
            return false;
        }
        self.instruction_queue_count = attempted_items;
        self.instruction_queue_encoded_bytes = attempted_bytes;
        true
    }
    fn try_reserve_serialized_output<T: NoritoSerialize>(
        &mut self,
        value: &T,
        items: usize,
    ) -> bool {
        const RETAINED_ITEM_OVERHEAD_BYTES: u64 = 64;
        let limits = self.instruction_queue_limits;
        if self.instruction_queue_violation.is_some() {
            return false;
        }
        let remaining = limits
            .max_bytes
            .saturating_sub(self.instruction_queue_encoded_bytes);
        // Count directly into a bounded sink, avoiding a temporary encoded
        // allocation while enforcing the exact aggregate consensus budget.
        let payload_limit = remaining.saturating_sub(RETAINED_ITEM_OVERHEAD_BYTES);
        let mut writer = BoundedCountingWriter {
            written: 0,
            max: usize::try_from(payload_limit).unwrap_or(usize::MAX),
        };
        let serialization = {
            let _canonical_flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::serialize_to_writer(value, &mut writer)
        };
        if serialization.is_err() {
            self.instruction_queue_violation = Some(HostOutputBudgetViolation::EncodedBytes {
                attempted: limits.max_bytes.saturating_add(1),
                limit: limits.max_bytes,
            });
            return false;
        }
        self.try_reserve_output(
            items,
            usize::try_from(
                u64::try_from(writer.written)
                    .unwrap_or(u64::MAX)
                    .saturating_add(RETAINED_ITEM_OVERHEAD_BYTES),
            )
            .unwrap_or(usize::MAX),
        )
    }
    fn try_reserve_durable_state_update(&mut self, key: &StatePath, value_len: usize) -> bool {
        const DURABLE_ENTRY_OVERHEAD_BYTES: u64 = 64;
        let key_len = u64::try_from(key.as_ref().len()).unwrap_or(u64::MAX);
        let value_len = u64::try_from(value_len).unwrap_or(u64::MAX);
        let new_bytes = key_len
            .saturating_add(value_len)
            .saturating_add(DURABLE_ENTRY_OVERHEAD_BYTES);
        let old_bytes = self.durable_state_overlay.get(key).map_or(0, |value| {
            key_len
                .saturating_add(
                    value
                        .as_ref()
                        .map_or(0, |bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
                )
                .saturating_add(DURABLE_ENTRY_OVERHEAD_BYTES)
        });
        let items = usize::from(old_bytes == 0);
        if new_bytes >= old_bytes {
            self.try_reserve_output(
                items,
                usize::try_from(new_bytes - old_bytes).unwrap_or(usize::MAX),
            )
        } else {
            self.instruction_queue_encoded_bytes = self
                .instruction_queue_encoded_bytes
                .saturating_sub(old_bytes - new_bytes);
            true
        }
    }
    fn drain_queued_effects(&mut self) -> Vec<QueuedEffect> {
        self.flush_pending_fastpq_batch();
        mem::take(&mut self.queued)
    }
    fn drain_queued_effects_with_fallback(
        &mut self,
        fallback: Option<ContractRuntimeExecutionContext>,
    ) -> Vec<QueuedEffect> {
        let fallback = fallback.or_else(|| self.current_contract_runtime_context.clone());
        let authorization_fallback = self.current_entrypoint_authorization.clone();
        self.drain_queued_effects()
            .into_iter()
            .map(|mut queued| {
                if queued.contract_runtime_context.is_none() {
                    queued.contract_runtime_context.clone_from(&fallback);
                }
                if queued.entrypoint_authorization.is_none() {
                    queued
                        .entrypoint_authorization
                        .clone_from(&authorization_fallback);
                }
                queued
            })
            .collect()
    }
    /// Extract queued effects for the test overlay builders, retaining their context.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn drain_queued_effects_with_contract_runtime_context(
        &mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) -> Vec<QueuedEffect> {
        self.drain_queued_effects_with_fallback(contract_runtime_context)
    }
    /// Drain durable contract-state writes collected during the last VM run.
    pub fn drain_durable_state_overlay(&mut self) -> DurableStateOverlay {
        self.durable_state_authorizations.clear();
        mem::take(&mut self.durable_state_overlay)
    }
    /// Extract test overlay writes with the immutable authorization for each write.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn drain_durable_state_overlay_with_authorizations(
        &mut self,
    ) -> AuthorizedStateOverlay {
        (
            mem::take(&mut self.durable_state_overlay),
            mem::take(&mut self.durable_state_authorizations),
        )
    }
    /// Extract completed AXT states for test overlays that compare proof replay with
    /// raw execution using the same lane and height materialization.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(crate) fn drain_completed_axt_states(&mut self) -> Vec<axt::HostAxtState> {
        mem::take(&mut self.completed_axt)
    }
    /// Test helper: seed the ballot verification latch with a known envelope hash.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub fn __test_seed_ballot_latch(&mut self, hash: [u8; 32]) {
        Arc::make_mut(&mut self.zk_verified_ballot).push_back(hash);
        Arc::make_mut(&mut self.zk_last_env_hash_ballot).push_back(hash);
    }
    /// Test-only: replace the pending tally envelope hash queue with a single entry.
    #[cfg(test)]
    pub fn __test_set_last_env_hash_tally(&mut self, h: [u8; 32]) {
        *Arc::make_mut(&mut self.zk_last_env_hash_tally) = VecDeque::from([h]);
    }
    fn validate_queued_for_zk(&mut self, queued: &[InstructionBox]) -> Result<(), ValidationFail> {
        for instr in queued {
            let any: &dyn iroha_data_model::isi::Instruction = &**instr;
            let any_ref = any.as_any();
            if let Some(sb) = any_ref.downcast_ref::<DMZk::SubmitBallot>() {
                let Some(expected_hash) = Arc::make_mut(&mut self.zk_verified_ballot).pop_front()
                else {
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
                let Some(expected_hash) = Arc::make_mut(&mut self.zk_verified_tally).pop_front()
                else {
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
    fn ensure_view_execution_has_no_effect_artifacts(&self) -> Result<(), ValidationFail> {
        if matches!(
            self.execution_class,
            HostExecutionClass::View | HostExecutionClass::LocalViewDebug
        ) && (!self.queued.is_empty()
            || self.fastpq_batch_entries.is_some()
            || !self.durable_state_overlay.is_empty()
            || !self.durable_state_authorizations.is_empty()
            || self.axt_state.is_some()
            || !self.completed_axt.is_empty()
            || self.instruction_queue_violation.is_some())
        {
            return Err(ValidationFail::NotPermitted(
                "read-only view execution retained mutable host artifacts".to_owned(),
            ));
        }
        Ok(())
    }
    fn ensure_execution_artifacts_are_committable(&self) -> Result<(), ValidationFail> {
        if self.local_debug_artifacts {
            return Err(ValidationFail::NotPermitted(
                "local debug execution artifacts cannot be committed".to_owned(),
            ));
        }
        Ok(())
    }
    fn ensure_output_budget(&self) -> Result<(), ivm::VMError> {
        self.instruction_queue_violation
            .map_or(Ok(()), |violation| Err(violation.into_vm_error()))
    }
    /// Discard a completed root invocation's effects when its outer Result is Err.
    ///
    /// The VM's protected return has already validated the complete signed callable
    /// schema. The result table and consumed gas remain available to the caller;
    /// read dependencies survive because they selected this recoverable outcome.
    ///
    /// # Errors
    /// Rejects an invalid result, unfinished nested frame, or exceeded output budget.
    pub fn finish_contract_result(&mut self, vm: &IVM) -> Result<bool, ivm::VMError> {
        self.ensure_output_budget()?;
        let Some(context) = self.current_contract_runtime_context.as_ref() else {
            return Ok(false);
        };
        if self.nested_contract_call_depth != 0 || !self.nested_contract_call_journals.is_empty() {
            return Err(ivm::VMError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation,
            ));
        }
        let descriptor = vm
            .contract_interface()
            .and_then(|interface| {
                interface
                    .entrypoints
                    .iter()
                    .find(|entrypoint| entrypoint.name == context.entrypoint)
            })
            .ok_or(ivm::VMError::InvalidMetadata)?;
        self.finish_result_schema(vm, descriptor.return_schema.as_ref())
    }
    /// Apply recoverable Result rollback to an explicitly selected local debug call.
    ///
    /// Local debugging has no deployed identity; the selector chooses its descriptor
    /// from the loaded artifact. This never makes local effects committable.
    ///
    /// # Errors
    /// Rejects non-debug hosts, unknown selectors, invalid results, or output overruns.
    pub fn finish_local_contract_result(
        &mut self,
        vm: &IVM,
        selector: &str,
    ) -> Result<bool, ivm::VMError> {
        self.ensure_output_budget()?;
        if !matches!(
            self.execution_class,
            HostExecutionClass::LocalContractDebug | HostExecutionClass::LocalViewDebug
        ) {
            return Err(ivm::VMError::PermissionDenied);
        }
        let descriptor = vm
            .contract_interface()
            .and_then(|interface| {
                interface
                    .entrypoints
                    .iter()
                    .find(|entrypoint| entrypoint.name == selector)
            })
            .ok_or(ivm::VMError::InvalidMetadata)?;
        self.finish_result_schema(vm, descriptor.return_schema.as_ref())
    }
    fn finish_result_schema(
        &mut self,
        vm: &IVM,
        schema: Option<&iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1>,
    ) -> Result<bool, ivm::VMError> {
        let schema = schema.ok_or(ivm::VMError::InvalidMetadata)?;
        if !ivm::sum::entrypoint_return_is_error(vm, schema)? {
            return Ok(false);
        }
        let attachment_rollback = self.rollback_attachment_root();
        self.queued.clear();
        self.durable_state_overlay.clear();
        self.durable_state_authorizations.clear();
        self.completed_axt.clear();
        self.fastpq_batch_entries = None;
        self.state_access_log.write_keys.clear();
        self.state_access_log.state_writes.clear();
        attachment_rollback?;
        Ok(true)
    }
    pub(crate) fn into_execution_artifacts(
        mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) -> Result<
        HostExecutionArtifacts,
        crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
    > {
        self.take_execution_artifacts(contract_runtime_context)
    }
    fn take_execution_artifacts(
        &mut self,
        contract_runtime_context: Option<ContractRuntimeExecutionContext>,
    ) -> Result<
        HostExecutionArtifacts,
        crate::execution_attempt::ExecutionAttemptError<ValidationFail>,
    > {
        self.ensure_output_budget().map_err(|error| {
            crate::execution_attempt::vm_attempt_error(error, |error| {
                ValidationFail::NotPermitted(error.to_string())
            })
        })?;
        self.ensure_execution_artifacts_are_committable()?;
        self.ensure_view_execution_has_no_effect_artifacts()?;
        let confidential_gas_delta = crate::gas::sum_confidential_gas_costs(
            self.queued.iter().filter_map(QueuedEffect::instruction),
            &self.confidential_gas,
        )
        .map_err(|original| {
            crate::execution_attempt::ExecutionAttemptError::Deferred(original.into())
        })?;
        // TransferAssetBatch contributes no confidential surcharge. Preserve an unfinished
        // batch intact until every fallible quote over the existing queue has completed.
        self.flush_pending_fastpq_batch();
        let queued = self.drain_queued_effects_with_fallback(contract_runtime_context);
        self.validate_queued_for_zk(
            &queued
                .iter()
                .filter_map(|queued| queued.instruction().cloned())
                .collect::<Vec<_>>(),
        )?;
        let completed_axt = mem::take(&mut self.completed_axt);
        let durable_state_overlay = mem::take(&mut self.durable_state_overlay);
        let durable_state_authorizations = mem::take(&mut self.durable_state_authorizations);
        self.finish_attachment_root()?;
        Ok(HostExecutionArtifacts {
            queued,
            entrypoint_authorization: self.current_entrypoint_authorization.clone(),
            confidential_gas_delta,
            completed_axt,
            durable_state_overlay,
            durable_state_authorizations,
        })
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
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
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Apply queued ISIs via the executor while preserving an optional
    /// contract runtime context for nested contract execution.
    pub(crate) fn apply_queued_with_contract_runtime_context(
        &mut self,
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<Vec<InstructionBox>, ValidationFail> {
        self.confidential_gas = tx.zk.gas;
        self.take_execution_artifacts(contract_runtime_context.cloned())
            .map_err(|error| tx.attempt_error_to_validation_fail(error))?
            .apply_to_transaction(tx, authority)
    }
    fn journal_durable_state_before_write(&mut self, key: &StatePath) {
        if self.nested_contract_call_journals.is_empty() {
            return;
        }
        let original = DurableStateOriginal {
            overlay: self
                .durable_state_overlay
                .get(key)
                .cloned()
                .map_or(OriginalMapEntry::Absent, OriginalMapEntry::Present),
            authorization: self
                .durable_state_authorizations
                .get(key)
                .cloned()
                .map_or(OriginalMapEntry::Absent, OriginalMapEntry::Present),
        };
        for journal in &mut self.nested_contract_call_journals {
            journal
                .durable_state_originals
                .entry(key.clone())
                .or_insert_with(|| original.clone());
        }
    }
    fn stage_durable_state_update(&mut self, key: StatePath, value: Option<Vec<u8>>) {
        self.journal_durable_state_before_write(&key);
        self.durable_state_overlay.insert(key.clone(), value);
        self.durable_state_authorizations
            .insert(key, self.current_entrypoint_authorization.clone());
    }
    fn insert_access_read_key(&mut self, key: &str) {
        if self.state_access_log.read_keys.insert(key.to_owned()) {
            for journal in &mut self.nested_contract_call_journals {
                journal.new_read_keys.insert(key.to_owned());
            }
        }
    }
    fn insert_access_write_key(&mut self, key: &str) {
        if self.state_access_log.write_keys.insert(key.to_owned()) {
            for journal in &mut self.nested_contract_call_journals {
                journal.new_write_keys.insert(key.to_owned());
            }
        }
    }
    fn insert_access_durable_read_path(&mut self, path: &str) {
        if self
            .state_access_log
            .durable_read_paths
            .insert(path.to_owned())
        {
            for journal in &mut self.nested_contract_call_journals {
                journal.new_durable_read_paths.insert(path.to_owned());
            }
        }
    }
    fn log_state_read_key(&mut self, key: &str) {
        if !self.access_log_enabled {
            return;
        }
        let physical_path = self.durable_state_scope_prefix().map_or_else(
            || key.to_string(),
            |scope_prefix| format!("{scope_prefix}{key}"),
        );
        let new_logical_key = !self.state_access_log.read_keys.contains(key);
        let new_physical_path = !self
            .state_access_log
            .durable_read_paths
            .contains(&physical_path);
        let items = usize::from(new_logical_key).saturating_add(usize::from(new_physical_path));
        let bytes = if new_logical_key {
            key.len().saturating_add(64)
        } else {
            0
        }
        .saturating_add(if new_physical_path {
            physical_path.len().saturating_add(64)
        } else {
            0
        });
        if !self.try_reserve_output(items, bytes) {
            self.state_access_log.durable_read_paths_complete = false;
            return;
        }
        self.insert_access_read_key(key);
        self.insert_access_durable_read_path(&(physical_path));
    }
    fn log_state_write_key(&mut self, key: &str) {
        if !self.access_log_enabled {
            return;
        }
        let unique_key = !self.state_access_log.write_keys.contains(key);
        let items = 1_usize.saturating_add(usize::from(unique_key));
        let bytes = key.len().saturating_add(64).saturating_add(if unique_key {
            key.len().saturating_add(32)
        } else {
            0
        });
        if !self.try_reserve_output(items, bytes) {
            return;
        }
        self.insert_access_write_key(key);
        self.state_access_log
            .state_writes
            .push(ivm::parallel::StateUpdate {
                key: key.to_string(),
                value: 1,
            });
    }
    fn scoped_durable_state_path(
        &self,
        path: &StatePath,
    ) -> Result<Option<StatePath>, ivm::VMError> {
        let Some(scope_prefix) = self.durable_state_scope_prefix() else {
            self.ensure_raw_durable_state_path_allowed(path)?;
            return Ok(None);
        };
        let scoped = format!("{scope_prefix}{}", path.as_ref());
        scoped
            .parse()
            .map(Some)
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }
    fn ensure_raw_durable_state_path_allowed(&self, path: &StatePath) -> Result<(), ivm::VMError> {
        if self.current_contract_runtime_context.is_none()
            && (Self::state_key_matches_prefix(path.as_ref(), "sc")
                || Self::state_key_matches_prefix(
                    path.as_ref(),
                    crate::smartcontracts::code::CONTRACT_LIFECYCLE_STATE_PREFIX,
                ))
        {
            // `sc/<contract-address-hash>/...` and `lc/<contract-address-hash>` are
            // runtime-owned namespaces. An unscoped raw IVM program must not be able to
            // read, enumerate, overwrite, or tombstone either by spelling its physical path.
            return Err(ivm::VMError::PermissionDenied);
        }
        Ok(())
    }
    fn durable_state_scope_prefix(&self) -> Option<String> {
        let context = self.current_contract_runtime_context.as_ref()?;
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
    fn durable_state_key_present(&self, key: &StatePath) -> Result<bool, ivm::VMError> {
        if let Some(scoped_path) = self.scoped_durable_state_path(key)? {
            if let Some(entry) = self.durable_state_overlay.get(&scoped_path) {
                return Ok(entry.is_some());
            }
            return Ok(self.durable_state_base_or_live_contains(&scoped_path));
        }
        if let Some(entry) = self.durable_state_overlay.get(key) {
            return Ok(entry.is_some());
        }
        Ok(self.durable_state_base_or_live_contains(key))
    }
    fn durable_state_value_payload_len(stored: &[u8]) -> Result<usize, ivm::VMError> {
        durable_state_payload_len(stored)
    }
    fn durable_state_value_len(&self, key: &StatePath) -> Result<Option<usize>, ivm::VMError> {
        if let Some(scoped_path) = self.scoped_durable_state_path(key)? {
            if let Some(entry) = self.durable_state_overlay.get(&scoped_path) {
                return entry
                    .as_deref()
                    .map(Self::durable_state_value_payload_len)
                    .transpose();
            }
            return self.durable_state_base_or_live_value_len(&scoped_path);
        }
        if let Some(entry) = self.durable_state_overlay.get(key) {
            return entry
                .as_deref()
                .map(Self::durable_state_value_payload_len)
                .transpose();
        }
        self.durable_state_base_or_live_value_len(key)
    }
    fn durable_state_base_or_live_contains(&self, key: &StatePath) -> bool {
        self.durable_state_base.contains_key(key)
            || self
                .query_state
                .get()
                .is_some_and(|state| state.durable_state_contains(key))
    }
    fn durable_state_base_or_live_value_len(
        &self,
        key: &StatePath,
    ) -> Result<Option<usize>, ivm::VMError> {
        if let Some(stored) = self.durable_state_base.get(key) {
            return Self::durable_state_value_payload_len(stored).map(Some);
        }
        self.query_state
            .get()
            .map_or(Ok(None), |state| state.durable_state_payload_len(key))
    }
    fn durable_state_base_or_live_get<'a>(&'a self, key: &StatePath) -> Option<Cow<'a, [u8]>> {
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
    fn roots_get_response(
        &self,
        request: &ivm::zk_verify::RootsGetRequest,
    ) -> Result<ivm::zk_verify::RootsGetResponse, ivm::VMError> {
        let asset: AssetDefinitionId = request
            .asset_id
            .parse()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let Some(snapshot) = self.zk_roots.get(&asset) else {
            return Ok(ivm::zk_verify::RootsGetResponse {
                latest: [0; 32],
                roots: Vec::new(),
                height: 0,
            });
        };
        let height = Self::len_to_u32(snapshot.commitment_count)?;
        let all_roots = if snapshot.commitment_count == 0 {
            vec![snapshot.tree_profile.empty_root()]
        } else {
            snapshot.root_history.clone()
        };
        let latest = all_roots.last().copied().unwrap_or([0; 32]);
        let wanted = if request.max == 0 {
            self.zk_tree_roots_history_len.get()
        } else {
            self.zk_tree_roots_history_len
                .get()
                .min(request.max as usize)
        };
        let roots = if all_roots.len() <= wanted {
            all_roots
        } else {
            all_roots[all_roots.len() - wanted..].to_vec()
        };
        Ok(ivm::zk_verify::RootsGetResponse {
            latest,
            roots,
            height,
        })
    }
    fn vote_get_tally_response(
        &self,
        request: &ivm::zk_verify::VoteGetTallyRequest,
    ) -> Result<ivm::zk_verify::VoteGetTallyResponse, ivm::VMError> {
        if !request.is_valid_v1() {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let Some((options, finalized, tally)) = self.zk_elections.get(&request.election_id) else {
            return Ok(ivm::zk_verify::VoteGetTallyResponse {
                finalized: false,
                tally: Vec::new(),
            });
        };
        Self::validate_zk_election_tally(*options, tally)?;
        Ok(ivm::zk_verify::VoteGetTallyResponse {
            finalized: *finalized,
            tally: tally.clone(),
        })
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
    fn create_nfts_for_all_preview(&self) -> (Vec<InstructionBox>, u64) {
        let mut instructions = Vec::with_capacity(
            self.accounts_snapshot
                .len()
                .min(CREATE_NFTS_FOR_ALL_MAX_RECIPIENTS)
                .saturating_mul(2),
        );
        let mut sequence = self.nft_seq;
        for account_id in self.accounts_snapshot.iter() {
            if instructions.len() / 2 >= CREATE_NFTS_FOR_ALL_MAX_RECIPIENTS {
                break;
            }
            let Some(next_sequence) = sequence.checked_add(1) else {
                // Reusing an exhausted sequence would create duplicate NFT IDs.
                break;
            };
            let account_digest = HashOf::new(account_id);
            let name = format!(
                "nft_number_{}_for_{}",
                sequence,
                hex::encode(account_digest.as_ref())
            );
            let Ok(name) = name.parse() else {
                continue;
            };
            let nft_id = NftId::of(iroha_genesis::GENESIS_DOMAIN_ID.clone(), name);
            let register = InstructionBox::from(RegisterBox::from(Register::nft(Nft::new(
                nft_id.clone(),
                Metadata::default(),
            ))));
            let transfer = InstructionBox::from(TransferBox::from(Transfer::nft(
                self.authority.clone(),
                nft_id,
                account_id.clone(),
            )));
            instructions.push(register);
            instructions.push(transfer);
            sequence = next_sequence;
        }
        (instructions, sequence)
    }
    fn create_nfts_for_all_instructions_gas(
        &self,
        instructions: &[InstructionBox],
    ) -> Result<u64, ivm::VMError> {
        let gas = crate::gas::meter_instructions(instructions, &self.confidential_gas)
            .map_err(ivm::VMError::VerificationDeferred)?;
        Ok(gas.max(Self::create_nfts_for_all_gas()))
    }
    #[cfg(test)]
    fn create_nfts_for_all_gas_quote(&self) -> u64 {
        let (instructions, _) = self.create_nfts_for_all_preview();
        self.create_nfts_for_all_instructions_gas(&instructions)
            .expect("completed NFT fixture gas quote")
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
            .saturating_add(state.spends().len());
        Self::axt_gas(entries)
    }
    fn relative_durable_state_key<'a>(
        key: &'a StatePath,
        scope_prefix: Option<&str>,
    ) -> Result<&'a str, ivm::VMError> {
        scope_prefix.map_or(Ok(key.as_ref()), |scope_prefix| {
            key.as_ref()
                .strip_prefix(scope_prefix)
                .ok_or(ivm::VMError::NoritoInvalid)
        })
    }
    fn scan_durable_state_page(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        let scope = self.durable_state_scope_prefix();
        let request =
            ivm::state_scan::StateScanRequest::decode(vm, scope.as_deref().unwrap_or("local"))?;
        Self::ensure_contract_state_read_allowed(request.map())?;
        if scope.is_none() {
            self.ensure_raw_durable_state_path_allowed(request.map())?;
        }
        let map = request.map().clone();
        let prefix = format!("{}{}/", scope.as_deref().unwrap_or(""), map.as_ref());
        let after = request
            .after()
            .map(|key| format!("{}{}", scope.as_deref().unwrap_or(""), key.as_ref()));
        prefix
            .trim_end_matches('/')
            .parse::<StatePath>()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        if let Some(after) = after.as_ref() {
            after
                .parse::<StatePath>()
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
        }
        let mut page = ivm::state_scan::StateScanPage::new(request);
        visit_bounded_durable_state_positions(
            &self.durable_state_base,
            &self.durable_state_overlay,
            &prefix,
            after.as_deref(),
            |visitor| {
                self.query_state.get().map_or_else(
                    || Ok(std::ops::ControlFlow::Continue(())),
                    |state| state.visit_durable_state_positions(&prefix, after.as_deref(), visitor),
                )
            },
            &mut |key, present| {
                let relative = Self::relative_durable_state_key(key, scope.as_deref())?;
                page.examine(vm, relative, key.as_ref().len(), present)
            },
        )?;
        let gas = page.publish(vm)?;
        self.log_state_read_key(map.as_ref());
        Ok(gas)
    }
    fn count_durable_state_keys(
        &self,
        vm: &IVM,
        prefix: &StatePath,
        path_len: usize,
    ) -> Result<(u64, u64), ivm::VMError> {
        let prefix_str = prefix.as_ref();
        let scope_prefix = self.durable_state_scope_prefix();
        if scope_prefix.is_none() {
            self.ensure_raw_durable_state_path_allowed(prefix)?;
        }
        let physical_prefix = scope_prefix.as_ref().map_or_else(
            || prefix_str.to_owned(),
            |scope| format!("{scope}{prefix_str}"),
        );
        if scope_prefix.is_some() {
            physical_prefix
                .parse::<StatePath>()
                .map_err(|_| ivm::VMError::NoritoInvalid)?;
        }
        let mut total = 0_u64;
        let mut scan_work_gas = u64::try_from(path_len).unwrap_or(u64::MAX);
        let mut emit = |key: &StatePath, present: bool| -> Result<(), ivm::VMError> {
            let relative = Self::relative_durable_state_key(key, scope_prefix.as_deref())?;
            ivm::host::validate_state_path_text(relative)?;
            if Self::contract_state_namespace_access(relative)
                == ContractStateNamespaceAccess::OpaqueSystem
            {
                return Ok(());
            }
            ivm::host::preflight_reserved_state_scan_work_with_tail(
                vm,
                scan_work_gas,
                key.as_ref().len(),
                0,
            )?;
            scan_work_gas = scan_work_gas
                .saturating_add(ivm::gas::STATE_SCAN_ITEM_GAS)
                .saturating_add(u64::try_from(key.as_ref().len()).unwrap_or(u64::MAX));
            if !present {
                return Ok(());
            }
            if !Self::state_key_matches_prefix(relative, prefix_str) {
                return Ok(());
            }
            total = total.saturating_add(1);
            Ok(())
        };
        visit_merged_durable_state_keys(
            &self.durable_state_base,
            &self.durable_state_overlay,
            &physical_prefix,
            |visitor| {
                self.query_state.get().map_or_else(
                    || Ok(()),
                    |state| {
                        state.visit_durable_state_keys_with_text_prefix(&physical_prefix, visitor)
                    },
                )
            },
            &mut emit,
        )?;
        Ok((total, scan_work_gas))
    }
    fn decode_name_payload(payload: &[u8]) -> Result<Name, ivm::VMError> {
        decode_canonical_norito(payload).map_err(|_| ivm::VMError::DecodeError)
    }
    fn decode_durable_state_path_payload(
        &self,
        vm: &IVM,
        pointer: u64,
    ) -> Result<(StatePath, usize), ivm::VMError> {
        // Authenticated execution must bind every state path to the loaded CNTR
        // schema. Local-debug overlays are deliberately uncommittable, so they
        // may inspect ad-hoc paths without fabricating a deployable interface.
        if self.current_contract_runtime_context.is_some()
            && vm.contract_interface().is_none()
            && !self.local_debug_artifacts
        {
            return Err(ivm::VMError::InvalidMetadata);
        }
        let tlv = Self::expect_tlv(vm, pointer, PointerType::NoritoBytes)?;
        let path_len = tlv.payload.len();
        if path_len > ivm::syscalls::STATE_MAX_PATH_FRAME_BYTES {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let path: StatePath =
            decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)?;
        ivm::host::validate_state_path(&path)?;
        Ok((path, path_len))
    }
    fn decode_durable_state_path(
        &self,
        vm: &IVM,
        pointer: u64,
    ) -> Result<(StatePath, usize), ivm::VMError> {
        let (path, path_len) = self.decode_durable_state_path_payload(vm, pointer)?;
        ivm::host::validate_declared_state_path(vm, &path)?;
        Ok((path, path_len))
    }
    fn decode_durable_state_scan_path(
        &self,
        vm: &IVM,
        pointer: u64,
    ) -> Result<(StatePath, usize), ivm::VMError> {
        let (path, path_len) = self.decode_durable_state_path_payload(vm, pointer)?;
        ivm::host::validate_declared_state_scan_path(vm, &path)?;
        Ok((path, path_len))
    }
    fn load_state_value(vm: &mut IVM, stored: &[u8]) -> Result<(), ivm::VMError> {
        ivm::host::validate_state_value_payload_len(stored.len())?;
        let mut env = Vec::with_capacity(7 + stored.len() + Hash::LENGTH);
        env.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        env.push(1);
        let env_len = u32::try_from(stored.len()).map_err(|_| ivm::VMError::NoritoInvalid)?;
        env.extend_from_slice(&env_len.to_be_bytes());
        env.extend_from_slice(stored);
        let h: [u8; Hash::LENGTH] = Hash::new(stored).into();
        env.extend_from_slice(&h);
        let p = vm.alloc_host_tlv(&env)?;
        vm.set_register(10, p);
        Ok(())
    }
    /// Decode a typed TLV envelope from a provenance-valid pointer-ABI region.
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
    fn decode_pointer_tlv(
        vm: &IVM,
        ptr: u64,
        expected: PointerType,
    ) -> Result<pointer_abi::Tlv<'_>, ivm::VMError> {
        let tlv = vm.validate_tlv(ptr)?;
        if tlv.type_id != expected {
            return Err(ivm::VMError::NoritoInvalid);
        }
        Ok(tlv)
    }
    /// Decode a typed pointer-ABI TLV into a Norito value.
    ///
    /// The decoder accepts pointers in INPUT or allocated HEAP, as well as
    /// exact loader-validated literal TLVs from the contract image.
    ///
    /// # Errors
    /// Returns an error if the pointer type does not match, the type is not allowed
    /// by the current syscall policy, or the Norito payload cannot be decoded.
    pub fn decode_tlv_typed<T>(vm: &IVM, ptr: u64, expected: PointerType) -> Result<T, ivm::VMError>
    where
        T: for<'__frame> norito::NoritoDeserialize<'__frame> + norito::NoritoSerialize,
    {
        let tlv = Self::decode_pointer_tlv(vm, ptr, expected)?;
        decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)
    }
    fn decode_quantity(vm: &IVM, ptr: u64) -> Result<Quantity, ivm::VMError> {
        let tlv = Self::decode_pointer_tlv(vm, ptr, PointerType::Quantity)?;
        QuantityValueV1::decode_frame(tlv.payload)
            .map(QuantityValueV1::into_quantity)
            .map_err(|_| ivm::VMError::DecodeError)
    }
    fn decode_optional_amount(vm: &IVM, ptr: u64) -> Result<Option<Quantity>, ivm::VMError> {
        let layout = ivm::sum::SumLayoutV1::option(1).map_err(|_| ivm::VMError::DecodeError)?;
        let (is_some, payload) = ivm::sum::read_words(vm, ptr, layout)?;
        if !is_some {
            return Ok(None);
        }
        let amount_ptr = payload.first().copied().ok_or(ivm::VMError::DecodeError)?;
        Self::decode_quantity(vm, amount_ptr).map(Some)
    }
    fn decode_optional_string(vm: &IVM, ptr: u64) -> Result<Option<String>, ivm::VMError> {
        let layout = ivm::sum::SumLayoutV1::option(1).map_err(|_| ivm::VMError::DecodeError)?;
        let (is_some, payload) = ivm::sum::read_words(vm, ptr, layout)?;
        if !is_some {
            return Ok(None);
        }
        let string_ptr = payload.first().copied().ok_or(ivm::VMError::DecodeError)?;
        let bytes = Self::decode_tlv_blob(vm, string_ptr)?;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| ivm::VMError::DecodeError)
    }
    fn decode_query_key<T>(vm: &IVM, ptr: u64, expected: PointerType) -> Result<T, ivm::VMError>
    where
        T: for<'__frame> norito::NoritoDeserialize<'__frame> + norito::NoritoSerialize,
    {
        let tlv = Self::decode_pointer_tlv(vm, ptr, expected)?;
        decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)
    }
    fn decode_contract_instance_lookup(
        vm: &IVM,
        ptr: u64,
    ) -> Result<ContractInstanceLookup, ivm::VMError> {
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::NoritoBytes => decode_canonical_norito::<ContractAddress>(tlv.payload)
                .map(ContractInstanceLookup::Address)
                .map_err(|_| ivm::VMError::NoritoInvalid),
            PointerType::Name => {
                let alias_name = decode_canonical_norito::<Name>(tlv.payload)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let alias = alias_name
                    .as_ref()
                    .parse::<ContractAlias>()
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                Ok(ContractInstanceLookup::Alias(alias))
            }
            _ => Err(ivm::VMError::NoritoInvalid),
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
        decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)
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
        if let Some(map) = value.as_object()
            && let Some(key_value) = map
                .get("public_key")
                .or_else(|| map.get("publicKey"))
                .or_else(|| map.get("key"))
        {
            return Self::public_key_from_value(key_value);
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
        if map.keys().any(|key| {
            !matches!(
                key.as_str(),
                "peer"
                    | "peer_id"
                    | "peerId"
                    | "public_key"
                    | "publicKey"
                    | "key"
                    | "pop"
                    | "activation_at"
                    | "expiry_at"
            )
        }) {
            return Err(ivm::VMError::DecodeError);
        }
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
        Ok(request)
    }
    fn decode_permission(vm: &IVM, ptr: u64) -> Result<Permission, ivm::VMError> {
        let tlv = vm.validate_tlv(ptr)?;
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
        T: for<'__frame> norito::NoritoDeserialize<'__frame> + norito::NoritoSerialize,
    {
        decode_canonical_norito(payload).map_err(|_| ivm::VMError::NoritoInvalid)
    }
    fn expect_tlv(vm: &IVM, ptr: u64, expected: PointerType) -> Result<ivm::Tlv<'_>, ivm::VMError> {
        let tlv = vm.validate_tlv(ptr)?;
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
    fn axt_proof_cache_digest(proof: &ProofBlob) -> [u8; 32] {
        let mut identity = Vec::with_capacity(proof.payload.len().saturating_add(41));
        identity.extend_from_slice(b"iroha:axt:proof-cache:v1");
        match proof.expiry_slot {
            None => identity.push(0),
            Some(expiry_slot) => {
                identity.push(1);
                identity.extend_from_slice(&expiry_slot.to_le_bytes());
            }
        }
        identity.extend_from_slice(&proof.payload);
        Hash::new(identity).into()
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
            for (dsid, entry) in self.axt_proof_cache.iter() {
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
                    if let Some(entry) = Arc::make_mut(&mut self.axt_proof_cache).remove(&dsid) {
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
    fn current_axt_policy_version(&self) -> Option<u64> {
        self.axt_policy_snapshot
            .as_ref()
            .map(|snapshot| snapshot.version)
    }
    /// Atomically hydrate AXT timing, policy and issuer keys.
    ///
    /// Policy construction and authorization projection complete before the
    /// host is mutated. A successful replacement aborts any active envelope,
    /// because its proofs were admitted under the previous policy or timing.
    /// Work is proportional to the policy snapshot, never to the World: asset
    /// balance policies and incarnations are read from the attached query
    /// state only by the syscalls that need them.
    ///
    /// # Errors
    ///
    /// Returns [`AxtPolicySnapshotValidationError`] without changing the host
    /// when the state projects a non-canonical AXT policy snapshot.
    pub fn hydrate_axt_state(
        &mut self,
        state: &impl StateReadOnly,
    ) -> Result<(), AxtPolicySnapshotValidationError> {
        let timing = state.nexus().axt;
        let snapshot = state
            .axt_execution_policy_snapshot()
            .unwrap_or_else(|| state.axt_policy_snapshot());
        let policy = ivm::axt::SnapshotAxtPolicy::new_with_timing(
            &snapshot,
            timing.slot_length_ms,
            timing.max_clock_skew_ms,
        )?;
        let _current_slot = current_axt_slot_for_state(state)
            .ok_or(AxtPolicySnapshotValidationError::AuthenticatedLedgerTimeUnavailable)?;
        let mut issuer_keys = BTreeMap::new();
        for binding in &snapshot.entries {
            if binding.policy.manifest_root.iter().all(|byte| *byte == 0) {
                continue;
            }
            let resolved = state.axt_block_start_snapshot().map_or_else(
                || {
                    crate::nexus::space_directory::resolve_axt_issuer_binding(
                        state.world(),
                        binding.dsid,
                        binding.policy.manifest_root,
                    )
                },
                |block_start| block_start.issuer_binding(binding.dsid).cloned(),
            );
            if let Ok(resolved) = resolved {
                issuer_keys.insert(
                    binding.dsid,
                    AxtIssuerKeyBinding {
                        manifest_root: binding.policy.manifest_root,
                        issuer: resolved.issuer,
                        public_key: resolved.public_key,
                    },
                );
            }
        }
        self.axt_timing = timing;
        self.set_network_id(*state.network_id());
        self.install_validated_axt_policy_snapshot(&snapshot, policy);
        self.axt_issuer_keys = Arc::new(issuer_keys);
        self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
        Ok(())
    }
    fn map_axt_fastpq_error(
        &mut self,
        error: &fastpq_prover::Error,
        context: &str,
        dsid: DataSpaceId,
        lane: LaneId,
    ) -> ivm::VMError {
        if matches!(
            error,
            fastpq_prover::Error::LocalAllocationUnavailable { .. }
        ) {
            return ivm::VMError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable,
            );
        }
        self.record_axt_reject(
            AxtRejectReason::Proof,
            Some(dsid),
            Some(lane),
            format!("{context}: {error}"),
        );
        ivm::VMError::PermissionDenied
    }
    fn verify_fastpq_envelope_binding(
        &mut self,
        dsid: DataSpaceId,
        envelope: &ModelAxtProofEnvelope,
        expiry_slot: Option<u64>,
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
        fastpq_prover::validate_axt_transfer_claim_binding(binding).map_err(|err| {
            self.map_axt_fastpq_error(
                &(err),
                "generic AXT proof admission requires a witnessed transfer claim",
                dsid,
                policy.target_lane,
            )
        })?;
        if binding.source_dsid != dsid.as_u64() {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "FASTPQ binding source_dsid mismatch",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        fastpq_prover::verify_axt_proof_envelope_with_outer_metadata(envelope, expiry_slot)
            .map_err(|err| {
                self.map_axt_fastpq_error(
                    &(err),
                    "FASTPQ verification failed",
                    dsid,
                    policy.target_lane,
                )
            })?;
        Ok(())
    }
    #[allow(clippy::too_many_lines)]
    fn validate_axt_proof(
        &mut self,
        dsid: DataSpaceId,
        proof: &ProofBlob,
        policy: AxtPolicyEntry,
        admission: AxtProofAdmission,
    ) -> Result<(), ivm::VMError> {
        self.clear_axt_reject();
        if admission == AxtProofAdmission::StandaloneUnanchored {
            self.note_axt_proof_cache_event(AXT_PROOF_CACHE_REJECT);
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "standalone FASTPQ proof admission is unavailable until AXT_VERIFY_DS_PROOF binds the source roots and transaction set to an authoritative finalized source-state anchor",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
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
        if crate::fastpq::axt_proof_payload_exceeds_decode_limit(&proof.payload) {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                format!(
                    "proof payload exceeds the {}-byte decode limit",
                    fastpq_prover::MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES,
                ),
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
        // The cache identity covers both the expensive proof payload and its
        // exact proof-bound expiry mirror. The early range check cheaply
        // rejects stale artifacts, while the verifier authenticates equality.
        if let Some(expiry_slot) = expiry_with_skew
            && policy.current_slot > 0
            && policy.current_slot > expiry_slot
        {
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
        if !self.try_reserve_output(1, proof.payload.len().saturating_add(128)) {
            return Err(ivm::VMError::PermissionDenied);
        }
        // Keep the recorded proof expiry as provided; skew is only applied to cache/slot checks.
        let proof_for_state = ProofBlob {
            payload: proof.payload.clone(),
            expiry_slot: proof.expiry_slot,
        };
        // Decode and enforce the generic-consumer semantic profile before the
        // proof cache lookup. Otherwise a proof cached by an older or more
        // specialized verification path could turn an opaque carrier into a
        // contract-observable authorization success.
        let envelope =
            decode_canonical_norito::<ModelAxtProofEnvelope>(&proof.payload).map_err(|err| {
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
        let Some(binding) = envelope.fastpq_binding.as_ref() else {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                "FASTPQ proof envelope is missing fastpq_binding",
            );
            return Err(ivm::VMError::PermissionDenied);
        };
        fastpq_prover::validate_axt_transfer_claim_binding(binding).map_err(|err| {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(policy.target_lane),
                format!("generic AXT proof admission requires a witnessed transfer claim: {err}"),
            );
            ivm::VMError::PermissionDenied
        })?;
        let digest = Self::axt_proof_cache_digest(proof);
        let mut cache_snapshot: Option<CachedProofEntry> = None;
        let mut cache_event: Option<&'static str> = None;
        let mut cache_hit_valid = false;
        let mut cache_hit_invalid = false;
        if let Some(entry) = Arc::make_mut(&mut self.axt_proof_cache).get_mut(&dsid) {
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
            let state = Arc::make_mut(self.axt_state.as_mut().expect("axt_state checked above"));
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
        self.verify_fastpq_envelope_binding(dsid, &envelope, proof.expiry_slot, policy)?;
        let state = Arc::make_mut(self.axt_state.as_mut().expect("axt_state checked above"));
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
        Arc::make_mut(&mut self.axt_proof_cache).insert(
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
    #[cfg(test)]
    fn compute_envelope_hash(payload: &[u8]) -> Result<[u8; 32], ivm::VMError> {
        norito::decode_canonical::<iroha_data_model::zk::OpenVerifyEnvelope>(payload)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        Ok(iroha_crypto::Hash::new(payload).into())
    }
    fn len_to_u32(len: usize) -> Result<u32, ivm::VMError> {
        u32::try_from(len).map_err(|_| ivm::VMError::NoritoInvalid)
    }
    fn snapshot_nested_contract_call(&mut self) -> NestedContractCallHostSnapshot {
        let output_count_before = self.instruction_queue_count;
        let output_bytes_before = self.instruction_queue_encoded_bytes;
        let output_violation_before = self.instruction_queue_violation;
        let journal_depth = self.nested_contract_call_journals.len();
        self.nested_contract_call_journals
            .push(NestedContractCallJournal {
                state_writes_len: self.state_access_log.state_writes.len(),
                ..NestedContractCallJournal::default()
            });
        NestedContractCallHostSnapshot {
            authority: self.authority.clone(),
            execution_class: self.execution_class,
            local_debug_artifacts: self.local_debug_artifacts,
            current_contract_runtime_context: self.current_contract_runtime_context.clone(),
            current_entrypoint_authorization: self.current_entrypoint_authorization.clone(),
            args: self.args.take(),
            entrypoint_argument_record: self.entrypoint_argument_record.take(),
            fastpq_batch_entries: self.fastpq_batch_entries.take(),
            default: self.default.begin_forwarded_call(),
            nft_seq: self.nft_seq,
            queued_len: self.queued.len(),
            journal_depth,
            durable_read_paths_complete: self.state_access_log.durable_read_paths_complete,
            axt_state: self.axt_state.clone(),
            completed_axt_len: self.completed_axt.len(),
            zk_verified_ballot: self.zk_verified_ballot.clone(),
            zk_verified_tally: self.zk_verified_tally.clone(),
            zk_last_env_hash_ballot: self.zk_last_env_hash_ballot.clone(),
            zk_last_env_hash_tally: self.zk_last_env_hash_tally.clone(),
            axt_proof_cache_slot: self.axt_proof_cache_slot,
            axt_proof_cache: self.axt_proof_cache.clone(),
            last_axt_reject: self.last_axt_reject.clone(),
            amx_budget_violation: self.amx_budget_violation,
            output_count_before,
            output_bytes_before,
            output_violation_before,
        }
    }
    fn finish_nested_contract_call(
        &mut self,
        snapshot: NestedContractCallHostSnapshot,
        outcome: NestedContractCallOutcome,
    ) -> Result<(), ivm::VMError> {
        let NestedContractCallHostSnapshot {
            authority,
            execution_class,
            local_debug_artifacts,
            current_contract_runtime_context,
            current_entrypoint_authorization,
            args,
            entrypoint_argument_record,
            fastpq_batch_entries,
            default,
            nft_seq,
            queued_len,
            journal_depth,
            durable_read_paths_complete,
            axt_state,
            completed_axt_len,
            zk_verified_ballot,
            zk_verified_tally,
            zk_last_env_hash_ballot,
            zk_last_env_hash_tally,
            axt_proof_cache_slot,
            axt_proof_cache,
            last_axt_reject,
            amx_budget_violation,
            output_count_before,
            output_bytes_before,
            output_violation_before,
        } = snapshot;
        if self.nested_contract_call_journals.len() != journal_depth.saturating_add(1) {
            return Err(ivm::VMError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation,
            ));
        }
        let journal =
            self.nested_contract_call_journals
                .pop()
                .ok_or(ivm::VMError::ExecutionDeferred(
                    ivm::error::ExecutionDeferral::LocalInvariantViolation,
                ))?;
        self.authority = authority;
        self.execution_class = execution_class;
        self.local_debug_artifacts |= local_debug_artifacts;
        self.current_contract_runtime_context = current_contract_runtime_context;
        self.current_entrypoint_authorization = current_entrypoint_authorization;
        self.args = args;
        self.entrypoint_argument_record = entrypoint_argument_record;
        self.fastpq_batch_entries = fastpq_batch_entries;
        if matches!(outcome, NestedContractCallOutcome::Commit) {
            if !self.default.commit_forwarded_call(default) {
                return Err(ivm::VMError::ExecutionDeferred(
                    ivm::error::ExecutionDeferral::LocalInvariantViolation,
                ));
            }
            return Ok(());
        }
        if !self.default.rollback_forwarded_call(default) {
            return Err(ivm::VMError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::LocalInvariantViolation,
            ));
        }
        let preserve_reads = matches!(outcome, NestedContractCallOutcome::RollbackPreservingReads);
        let preserved_read_items = if preserve_reads {
            journal
                .new_read_keys
                .len()
                .saturating_add(journal.new_durable_read_paths.len())
        } else {
            0
        };
        let preserved_read_bytes = if preserve_reads {
            journal
                .new_read_keys
                .iter()
                .chain(journal.new_durable_read_paths.iter())
                .fold(0_usize, |total, key| {
                    total.saturating_add(key.len().saturating_add(64))
                })
        } else {
            0
        };
        let child_output_violation = self.instruction_queue_violation;
        let preserve_child_read_violation = preserve_reads
            && !self.state_access_log.durable_read_paths_complete
            && child_output_violation.is_some();
        let NestedContractCallJournal {
            durable_state_originals,
            new_read_keys,
            new_write_keys,
            new_durable_read_paths,
            state_writes_len,
        } = journal;
        self.nft_seq = nft_seq;
        self.queued.truncate(queued_len);
        for (key, original) in durable_state_originals {
            match original.overlay {
                OriginalMapEntry::Present(value) => {
                    self.durable_state_overlay.insert(key.clone(), value);
                }
                OriginalMapEntry::Absent => {
                    self.durable_state_overlay.remove(&key);
                }
            }
            match original.authorization {
                OriginalMapEntry::Present(authorization) => {
                    self.durable_state_authorizations.insert(key, authorization);
                }
                OriginalMapEntry::Absent => {
                    self.durable_state_authorizations.remove(&key);
                }
            }
        }
        for key in new_write_keys {
            self.state_access_log.write_keys.remove(&key);
        }
        self.state_access_log
            .state_writes
            .truncate(state_writes_len);
        if !preserve_reads {
            for key in new_read_keys {
                self.state_access_log.read_keys.remove(&key);
            }
            for path in new_durable_read_paths {
                self.state_access_log.durable_read_paths.remove(&path);
            }
            self.state_access_log.durable_read_paths_complete = durable_read_paths_complete;
        }
        self.axt_state = axt_state;
        self.completed_axt.truncate(completed_axt_len);
        self.zk_verified_ballot = zk_verified_ballot;
        self.zk_verified_tally = zk_verified_tally;
        self.zk_last_env_hash_ballot = zk_last_env_hash_ballot;
        self.zk_last_env_hash_tally = zk_last_env_hash_tally;
        self.axt_proof_cache_slot = axt_proof_cache_slot;
        self.axt_proof_cache = axt_proof_cache;
        self.last_axt_reject = last_axt_reject;
        self.amx_budget_violation = amx_budget_violation;
        self.instruction_queue_count = output_count_before;
        self.instruction_queue_encoded_bytes = output_bytes_before;
        self.instruction_queue_violation = output_violation_before;
        if preserve_reads
            && output_violation_before.is_none()
            && !self.try_reserve_output(preserved_read_items, preserved_read_bytes)
        {
            self.state_access_log.durable_read_paths_complete = false;
        }
        if preserve_child_read_violation && self.instruction_queue_violation.is_none() {
            self.instruction_queue_violation = child_output_violation;
        }
        Ok(())
    }
    fn resolve_bound_contract_dispatch_identity_by_address(
        &self,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
    ) -> Result<
        Option<(
            crate::smartcontracts::code::BoundContractIdentity,
            AccountId,
            usize,
        )>,
        ivm::VMError,
    > {
        if let Some(state_ref) = self.query_state.get() {
            // A live query view is authoritative. Never fall back to a stale
            // construction-time snapshot when an instance has been removed.
            return state_ref.bound_contract_dispatch_identity_by_address(contract_address);
        }
        let Some(record) = self
            .bound_contract_records_by_subject
            .values()
            .find(|record| record.contract_address == *contract_address)
        else {
            return Ok(None);
        };
        Ok(Some((
            crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: record.lifecycle_revision,
                contract_address: record.contract_address.clone(),
                contract_alias: record.contract_alias.clone(),
                contract_alias_binding: record.contract_alias_binding.clone(),
                code_hash: record.code_hash,
            },
            record.contract_subject.clone(),
            record.code_bytes.len(),
        )))
    }
    fn prepare_nested_contract(
        &self,
        identity: &crate::smartcontracts::code::BoundContractIdentity,
    ) -> Result<ivm::PreparedContract, ivm::VMError> {
        if let Some(prepared) = self.prepared_contract_cache.get(identity.code_hash) {
            return Ok(prepared);
        }
        if let Some(state_ref) = self.query_state.get() {
            return state_ref.prepare_contract_cache_miss(
                &self.prepared_contract_cache,
                iroha_data_model::smart_contract::ContractArtifactId::new(
                    identity
                        .contract_address
                        .dataspace_id()
                        .map_err(|_| ivm::VMError::PermissionDenied)?,
                    identity.code_hash,
                ),
            );
        }
        let record = self
            .bound_contract_records_by_subject
            .values()
            .find(|record| {
                record.contract_address == identity.contract_address
                    && record.code_hash == identity.code_hash
            })
            .ok_or(ivm::VMError::PermissionDenied)?;
        self.prepared_contract_cache
            .get_or_prepare(identity.code_hash, &record.code_bytes)
    }
    fn nested_contract_host_gas(
        request_bytes: usize,
        artifact_bytes: usize,
        return_bytes: usize,
    ) -> u64 {
        ivm::gas::syscall_byte_gas(ivm::gas::G_CALL_CONTRACT, request_bytes, return_bytes)
            .saturating_add(
                ivm::gas::CONSERVATIVE_SYSCALL_INPUT_MULTIPLIER
                    .saturating_mul(u64::try_from(artifact_bytes).unwrap_or(u64::MAX)),
            )
    }
    fn contract_address_is_active(&self, address: &ContractAddress) -> bool {
        self.current_contract_runtime_context
            .as_ref()
            .is_some_and(|context| context.contract_address == *address)
            || self.nested_contract_ancestors[..self.nested_contract_call_depth]
                .iter()
                .flatten()
                .any(|ancestor| ancestor == address)
    }
    const QUERY_GAS_BASE_SINGULAR: u64 = ivm::gas::LEDGER_QUERY_GAS_BASE_SINGULAR;
    const QUERY_GAS_BASE_ITERABLE: u64 = ivm::gas::LEDGER_QUERY_GAS_BASE_ITERABLE;
    const QUERY_GAS_PER_ITEM: u64 = ivm::gas::LEDGER_QUERY_GAS_PER_ITEM;
    const QUERY_GAS_SORT_MULTIPLIER: u64 = ivm::gas::LEDGER_QUERY_GAS_SORT_MULTIPLIER;
    const QUERY_GAS_PER_BYTE: u64 = ivm::gas::LEDGER_QUERY_GAS_PER_BYTE;
    const ACCOUNT_VIEW_WORDS: u64 = 2;
    const ASSET_VIEW_WORDS: u64 = 2;
    const ASSET_DEFINITION_VIEW_WORDS: u64 = 7;
    const DOMAIN_VIEW_WORDS: u64 = 3;
    const NFT_VIEW_WORDS: u64 = 3;
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
    fn query_gas_cost(ctx: &QueryGasContext, processed_items: u64, processed_bytes: u64) -> u64 {
        ivm::gas::ledger_query_gas_v1(
            ctx.base,
            ctx.per_item,
            ctx.offset_items,
            processed_items,
            processed_bytes,
        )
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
        let plan = context.plan.clone();
        if subscription_state.provider != plan.provider {
            return Err(ivm::VMError::NoritoInvalid);
        }
        subscription_state.billing_trigger_id = trigger_id.clone();
        #[cfg(feature = "telemetry")]
        let pricing_label = match &plan.pricing {
            SubscriptionPricing::Fixed(_) => "fixed",
            SubscriptionPricing::Usage(_) => "usage",
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
            return self.queue_instruction(instr);
        }
        let charge_asset_def = context.charge_asset_def.clone();
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
                gas = gas.saturating_add(self.queue_instruction(instr)?);
                let unregister = InstructionBox::from(UnregisterBox::from(Unregister::trigger(
                    trigger_id.clone(),
                )));
                gas = gas.saturating_add(self.queue_instruction(unregister)?);
                return Ok(gas);
            }
        }
        let charge_spec = charge_asset_def.spec();
        let (amount, usage_key) = match &plan.pricing {
            SubscriptionPricing::Fixed(pricing) => {
                let fixed_amount = pricing.amount.clone();
                if charge_spec.check(fixed_amount.as_numeric()).is_err() {
                    return Err(ivm::VMError::NoritoInvalid);
                }
                (fixed_amount, None)
            }
            SubscriptionPricing::Usage(pricing) => {
                let usage = subscription_state
                    .usage_accumulated
                    .get(&pricing.unit_key)
                    .cloned()
                    .unwrap_or_else(Quantity::zero);
                let amount = usage
                    .try_mul_decimal(pricing.unit_price.as_numeric())
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                if charge_spec.check(amount.as_numeric()).is_err() {
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
                let isi = Transfer::asset_quantity(asset_id, amount.clone(), plan.provider.clone());
                let instr = InstructionBox::from(TransferBox::from(isi));
                gas = gas.saturating_add(self.queue_instruction(instr)?);
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
        gas = gas.saturating_add(self.queue_instruction(instr)?);
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
        gas = gas.saturating_add(self.queue_instruction(invoice_instr)?);
        if matches!(
            subscription_state.status,
            SubscriptionStatus::Suspended | SubscriptionStatus::Canceled
        ) {
            let instr =
                InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
            gas = gas.saturating_add(self.queue_instruction(instr)?);
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
        .map_err(|_| ivm::VMError::NoritoInvalid)?
        .with_metadata(next_trigger_metadata);
        let trigger = Trigger::new(trigger_id.clone(), action);
        let unregister =
            InstructionBox::from(UnregisterBox::from(Unregister::trigger(trigger_id.clone())));
        gas = gas.saturating_add(self.queue_instruction(unregister)?);
        let register = InstructionBox::from(RegisterBox::from(Register::trigger(trigger)));
        gas = gas.saturating_add(self.queue_instruction(register)?);
        Ok(gas)
    }
    fn subscription_record_usage(&mut self) -> Result<u64, ivm::VMError> {
        let args = self.args.as_ref().ok_or(ivm::VMError::NoritoInvalid)?;
        let delta: SubscriptionUsageDelta = args
            .try_into_any_norito()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
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
            .or_insert_with(Quantity::zero);
        let updated = entry
            .checked_add(&delta.delta)
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
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
        self.queue_instruction(instr)
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
        let balance = {
            let asset_id = AssetId::of(
                charge_asset_def.id.clone(),
                subscription_state.subscriber.clone(),
            );
            state
                .world()
                .asset(&asset_id)
                .map_or_else(|_| Quantity::zero(), |entry| entry.value().as_ref().clone())
        };
        Ok(SubscriptionContext {
            executable: action.executable.clone(),
            authority: action.authority.clone(),
            trigger_metadata,
            subscription_nft_id,
            subscription_state,
            plan,
            charge_asset_def,
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
            "sumeragi.max_clock_drift_ms" => Parameter::Sumeragi(
                SumeragiParameter::MaxClockDriftMs(params.sumeragi().max_clock_drift_ms()),
            ),
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
            "transaction.max_time_to_live_ms" => Parameter::Transaction(
                TransactionParameter::MaxTimeToLiveMs(params.transaction().max_time_to_live_ms()),
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
            "smart_contract.max_output_items" => Parameter::SmartContract(
                SmartContractParameter::MaxOutputItems(params.smart_contract().max_output_items()),
            ),
            "smart_contract.max_output_bytes" => Parameter::SmartContract(
                SmartContractParameter::MaxOutputBytes(params.smart_contract().max_output_bytes()),
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
            "executor.max_output_items" => Parameter::Executor(
                SmartContractParameter::MaxOutputItems(params.executor().max_output_items()),
            ),
            "executor.max_output_bytes" => Parameter::Executor(
                SmartContractParameter::MaxOutputBytes(params.executor().max_output_bytes()),
            ),
            _ => return Err(ivm::VMError::DecodeError),
        };
        Ok(parameter)
    }
    fn contract_instance_by_address<S: StateReadOnly>(
        state: &S,
        contract_address: &ContractAddress,
    ) -> Result<ContractInstance, ivm::VMError> {
        crate::executor::root_scope::ensure_committed_contract_scope(
            state.world(),
            contract_address,
        )
        .map_err(|error| error.into_vm_error(|_| ivm::VMError::PermissionDenied))?;
        let code_hash = state
            .world()
            .contract_instances()
            .get(contract_address)
            .copied()
            .ok_or(ivm::VMError::DecodeError)?;
        let artifact = iroha_data_model::smart_contract::ContractArtifactId::new(
            contract_address
                .dataspace_id()
                .map_err(|_| ivm::VMError::PermissionDenied)?,
            code_hash,
        );
        let manifest = state
            .world()
            .contract_manifests()
            .get(&artifact)
            .ok_or(ivm::VMError::DecodeError)?;
        if manifest.code_hash != Some(code_hash)
            || state.world().contract_code().get(&artifact).is_none()
        {
            return Err(ivm::VMError::DecodeError);
        }
        let contract_alias = state
            .world()
            .contract_alias_bindings()
            .get(contract_address)
            .map(|binding| binding.alias.clone());
        if let Some(alias) = contract_alias.as_ref()
            && state.world().contract_aliases().get(alias) != Some(contract_address)
        {
            return Err(ivm::VMError::DecodeError);
        }
        if state
            .world()
            .contract_aliases()
            .iter()
            .any(|(alias, address)| {
                address == contract_address && Some(alias) != contract_alias.as_ref()
            })
        {
            return Err(ivm::VMError::DecodeError);
        }
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
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let payload_len = SerializePayload::encoded_len_exact(value)?;
        let header_len = Header::SIZE;
        let align = norito::core::archived_payload_align::<T>();
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
    fn query_execution_budget(
        ctx: &QueryGasContext,
        gas_remaining: u64,
    ) -> Result<QueryExecutionBudget, ivm::VMError> {
        let base_cost = ctx
            .base
            .saturating_add(ctx.per_item.saturating_mul(ctx.offset_items));
        if gas_remaining < base_cost {
            return Err(ivm::VMError::OutOfGas);
        }
        let remaining = gas_remaining.saturating_sub(base_cost);
        Ok(QueryExecutionBudget::from_weighted_limit(
            remaining,
            ctx.per_item,
            Self::QUERY_GAS_PER_BYTE,
        ))
    }
    fn encode_norito_payload<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>, ivm::VMError> {
        encode_canonical_norito(value)
    }
    fn encode_amount_payload(value: &Quantity) -> Result<Vec<u8>, ivm::VMError> {
        QuantityValueV1::new(value.clone())
            .encode_frame()
            .map_err(|_| ivm::VMError::NoritoInvalid)
    }
    fn project_account(account: Account) -> AccountView {
        AccountView {
            id: account.id,
            metadata: Json::new(account.metadata),
        }
    }
    fn project_asset(asset: Asset) -> AssetView {
        AssetView {
            id: asset.id,
            amount: asset.value.into(),
        }
    }
    fn project_asset_definition(definition: AssetDefinition) -> AssetDefinitionView {
        AssetDefinitionView {
            id: definition.id,
            name: definition.name,
            description: definition.description,
            owned_by: definition.owned_by,
            total_quantity: definition.total_quantity.into(),
            numeric_scale: definition.spec.scale().map(i64::from),
            metadata: Json::new(definition.metadata),
        }
    }
    fn project_domain(domain: Domain) -> DomainView {
        DomainView {
            id: domain.id,
            owned_by: domain.owned_by,
            metadata: Json::new(domain.metadata),
        }
    }
    fn project_nft(nft: Nft) -> NftView {
        NftView {
            id: nft.id,
            owned_by: nft.owned_by,
            content: Json::new(nft.content),
        }
    }
    fn prepare_typed_query_leaf<T: NoritoSerialize>(
        pointer_type: PointerType,
        value: &T,
    ) -> Result<PreparedCoreQueryWord, ivm::VMError> {
        let payload = Self::encode_norito_payload(value)?;
        Ok(PreparedCoreQueryWord::Tlv(Self::encode_tlv_payload(
            pointer_type,
            &payload,
        )?))
    }
    fn prepare_quantity_query_leaf(
        value: &QuantityV1,
    ) -> Result<PreparedCoreQueryWord, ivm::VMError> {
        let payload = QuantityValueV1::new(value.as_quantity().clone())
            .encode_frame()
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        Ok(PreparedCoreQueryWord::Tlv(Self::encode_tlv_payload(
            PointerType::Quantity,
            &payload,
        )?))
    }
    fn prepare_optional_int_query_leaf(
        value: Option<i64>,
    ) -> Result<PreparedCoreQueryWord, ivm::VMError> {
        let tlv = value
            .map(|value| {
                let frame = IntValueV1::try_new(BigInt::from(value))
                    .and_then(|value| value.encode_frame())
                    .map_err(|_| ivm::VMError::NoritoInvalid)?;
                Self::encode_tlv_payload(PointerType::Int, &frame)
            })
            .transpose()?;
        Ok(PreparedCoreQueryWord::OptionalTlv(tlv))
    }
    fn prepare_query_blob(value: &[u8]) -> Result<PreparedCoreQueryWord, ivm::VMError> {
        Ok(PreparedCoreQueryWord::Tlv(Self::encode_tlv_payload(
            PointerType::Blob,
            value,
        )?))
    }
    fn prepare_optional_query_blob(
        value: Option<&str>,
    ) -> Result<PreparedCoreQueryWord, ivm::VMError> {
        let tlv = value
            .map(|value| Self::encode_tlv_payload(PointerType::Blob, value.as_bytes()))
            .transpose()?;
        Ok(PreparedCoreQueryWord::OptionalTlv(tlv))
    }
    fn prepare_account_view(
        view: &AccountView,
    ) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError> {
        Ok(vec![
            Self::prepare_typed_query_leaf(PointerType::AccountId, &view.id)?,
            Self::prepare_typed_query_leaf(PointerType::Json, &view.metadata)?,
        ])
    }
    fn prepare_asset_view(view: &AssetView) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError> {
        Ok(vec![
            Self::prepare_typed_query_leaf(PointerType::AssetId, &view.id)?,
            Self::prepare_quantity_query_leaf(&view.amount)?,
        ])
    }
    fn prepare_asset_definition_view(
        view: &AssetDefinitionView,
    ) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError> {
        Ok(vec![
            Self::prepare_typed_query_leaf(PointerType::AssetDefinitionId, &view.id)?,
            Self::prepare_query_blob(view.name.as_bytes())?,
            Self::prepare_optional_query_blob(view.description.as_deref())?,
            Self::prepare_typed_query_leaf(PointerType::AccountId, &view.owned_by)?,
            Self::prepare_quantity_query_leaf(&view.total_quantity)?,
            Self::prepare_optional_int_query_leaf(view.numeric_scale)?,
            Self::prepare_typed_query_leaf(PointerType::Json, &view.metadata)?,
        ])
    }
    fn prepare_domain_view(view: &DomainView) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError> {
        Ok(vec![
            Self::prepare_typed_query_leaf(PointerType::DomainId, &view.id)?,
            Self::prepare_typed_query_leaf(PointerType::AccountId, &view.owned_by)?,
            Self::prepare_typed_query_leaf(PointerType::Json, &view.metadata)?,
        ])
    }
    fn prepare_nft_view(view: &NftView) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError> {
        Ok(vec![
            Self::prepare_typed_query_leaf(PointerType::NftId, &view.id)?,
            Self::prepare_typed_query_leaf(PointerType::AccountId, &view.owned_by)?,
            Self::prepare_typed_query_leaf(PointerType::Json, &view.content)?,
        ])
    }
    fn prepared_query_leaf_bytes(elements: &[Vec<PreparedCoreQueryWord>]) -> u64 {
        elements.iter().flatten().fold(0_u64, |total, word| {
            total.saturating_add(word.encoded_bytes())
        })
    }
    fn materialize_prepared_query_words(
        vm: &mut IVM,
        words: Vec<PreparedCoreQueryWord>,
    ) -> Result<Vec<u64>, ivm::VMError> {
        words.into_iter().map(|word| word.materialize(vm)).collect()
    }
    fn finish_and_materialize_core_query_option<T>(
        &mut self,
        vm: &mut IVM,
        gas_remaining: u64,
        gas_ctx: &QueryGasContext,
        (processed_items, processed_bytes): (u64, u64),
        (payload, payload_words): (&[u8], u64),
        prepare: fn(T) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError>
    where
        T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
    {
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.projection_decodes = metrics.projection_decodes.saturating_add(1);
            metrics.projection_payload_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        }
        let decoded: Option<T> =
            decode_canonical_norito(payload).map_err(|_| ivm::VMError::DecodeError)?;
        let prepared = decoded.map(prepare).transpose()?;
        if prepared
            .as_ref()
            .is_some_and(|words| u64::try_from(words.len()).unwrap_or(u64::MAX) != payload_words)
        {
            return Err(ivm::VMError::DecodeError);
        }
        let leaf_tlv_bytes = prepared.as_ref().map_or(0, |words| {
            Self::prepared_query_leaf_bytes(std::slice::from_ref(words))
        });
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.leaf_tlv_bytes = leaf_tlv_bytes;
        }
        let encoded_bytes = u64::try_from(payload.len())
            .unwrap_or(u64::MAX)
            .saturating_add(leaf_tlv_bytes);
        let gas = Self::finish_core_query_encoded_bytes_gas(
            vm,
            gas_remaining,
            gas_ctx,
            processed_items,
            processed_bytes,
            encoded_bytes,
        )?;
        let layout =
            ivm::sum::SumLayoutV1::option(payload_words).map_err(|_| ivm::VMError::DecodeError)?;
        let handle = match prepared {
            Some(words) => {
                let words = Self::materialize_prepared_query_words(vm, words)?;
                ivm::sum::allocate_words(vm, layout, 1, &words)?
            }
            None => ivm::sum::allocate_words(vm, layout, 0, &[])?,
        };
        vm.set_register(10, handle);
        Ok(gas)
    }
    fn finish_and_materialize_core_query_page<T>(
        &mut self,
        vm: &mut IVM,
        gas_remaining: u64,
        gas_ctx: &QueryGasContext,
        (processed_items, processed_bytes): (u64, u64),
        (payload, element_words): (&[u8], u64),
        prepare: fn(T) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError>
    where
        T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
    {
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.projection_decodes = metrics.projection_decodes.saturating_add(1);
            metrics.projection_payload_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        }
        let decoded: QueryPageV1<T> =
            decode_canonical_norito(payload).map_err(|_| ivm::VMError::DecodeError)?;
        let (items, next_offset) = decoded.into_parts();
        let elements = items
            .into_iter()
            .map(prepare)
            .collect::<Result<Vec<_>, _>>()?;
        let next_offset = Self::prepare_optional_int_query_leaf(next_offset)?;
        if elements
            .iter()
            .any(|words| u64::try_from(words.len()).unwrap_or(u64::MAX) != element_words)
        {
            return Err(ivm::VMError::DecodeError);
        }
        let leaf_tlv_bytes =
            Self::prepared_query_leaf_bytes(&elements).saturating_add(next_offset.encoded_bytes());
        let encoded_bytes = u64::try_from(payload.len())
            .unwrap_or(u64::MAX)
            .saturating_add(leaf_tlv_bytes);
        let gas = Self::finish_core_query_encoded_bytes_gas(
            vm,
            gas_remaining,
            gas_ctx,
            processed_items,
            processed_bytes,
            encoded_bytes,
        )?;
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.leaf_tlv_bytes = leaf_tlv_bytes;
        }
        let elements = elements
            .into_iter()
            .map(|words| Self::materialize_prepared_query_words(vm, words))
            .collect::<Result<Vec<_>, _>>()?;
        let list_layout =
            ivm::list::ListLayoutV1::try_new(QUERY_PAGE_CAPACITY_V1 as u64, element_words)
                .map_err(|_| ivm::VMError::DecodeError)?;
        let list_handle = ivm::list::allocate_words(vm, list_layout, &elements)?;
        let offset_handle = next_offset.materialize(vm)?;
        vm.set_register(10, list_handle);
        vm.set_register(11, offset_handle);
        Ok(gas)
    }
    fn core_query_page_request(
        tag: CoreQueryEntityTagV1,
        offset: u64,
        limit: u64,
        account: Option<AccountId>,
    ) -> Result<QueryRequest, ivm::VMError> {
        if account.is_some() && tag != CoreQueryEntityTagV1::Asset {
            return Err(ivm::VMError::DecodeError);
        }
        let fetch_size = NonZeroU64::new(limit).ok_or(ivm::VMError::DecodeError)?;
        let window_size = NonZeroU64::new(limit.checked_add(1).ok_or(ivm::VMError::DecodeError)?)
            .ok_or(ivm::VMError::DecodeError)?;
        let params = QueryParams {
            pagination: Pagination::new(Some(window_size), offset),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(fetch_size)),
        };
        macro_rules! request_for {
            ($item:ty, $query:expr, $kind:ident, $params:ident) => {{
                QueryRequest::Start(QueryWithParams {
                    query: (),
                    query_payload: norito::codec::Encode::encode(&$query),
                    item: QueryItemKind::$kind,
                    predicate_bytes: norito::codec::Encode::encode(
                        &CompoundPredicate::<$item>::PASS,
                    ),
                    selector_bytes: norito::codec::Encode::encode(
                        &SelectorTuple::<$item>::default(),
                    ),
                    params: $params,
                })
            }};
        }
        Ok(match tag {
            CoreQueryEntityTagV1::Account => request_for!(
                Account,
                iroha_data_model::query::account::prelude::FindAccounts,
                Account,
                params
            ),
            CoreQueryEntityTagV1::Asset if account.is_some() => request_for!(
                Asset,
                iroha_data_model::query::asset::prelude::FindAssetsByAccountId::new(
                    account.ok_or(ivm::VMError::DecodeError)?
                ),
                Asset,
                params
            ),
            CoreQueryEntityTagV1::Asset => request_for!(
                Asset,
                iroha_data_model::query::asset::prelude::FindAssets,
                Asset,
                params
            ),
            CoreQueryEntityTagV1::AssetDefinition => request_for!(
                AssetDefinition,
                iroha_data_model::query::asset::prelude::FindAssetDefinitions,
                AssetDefinition,
                params
            ),
            CoreQueryEntityTagV1::Domain => request_for!(
                Domain,
                iroha_data_model::query::domain::prelude::FindDomains,
                Domain,
                params
            ),
            CoreQueryEntityTagV1::Nft => {
                request_for!(
                    Nft,
                    iroha_data_model::query::nft::prelude::FindNfts,
                    Nft,
                    params
                )
            }
        })
    }
    fn finish_core_query_encoded_bytes_gas(
        vm: &mut IVM,
        gas_remaining: u64,
        gas_ctx: &QueryGasContext,
        processed_items: u64,
        processed_bytes: u64,
        encoded_bytes: u64,
    ) -> Result<u64, ivm::VMError> {
        // `processed_bytes` covers source traversal and the generic query
        // response. `encoded_bytes` is the separate typed core-query projection
        // (including leaf TLVs), so adding them is intentional rather than
        // charging one serialization twice.
        let gas = Self::query_gas_cost(
            gas_ctx,
            processed_items,
            processed_bytes.saturating_add(encoded_bytes),
        );
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        if gas > gas_remaining {
            return Err(ivm::VMError::OutOfGas);
        }
        Ok(gas)
    }
    fn execute_optional_core_query<T>(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        request: QueryRequest,
        project: impl FnOnce(SingularQueryOutputBox) -> Result<T, ivm::VMError>,
        payload_words: u64,
        prepare: fn(T) -> Result<Vec<PreparedCoreQueryWord>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError>
    where
        T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
    {
        let gas_remaining = vm.remaining_gas();
        let gas_ctx = QueryGasContext::singular();
        let budget = Self::query_execution_budget(&gas_ctx, gas_remaining)?;
        if budget.max_items() == 0 {
            return Err(ivm::VMError::OutOfGas);
        }
        let Some(state_ref) = self.query_state.get() else {
            return Err(ivm::VMError::NotImplemented { syscall });
        };
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.host_queries = metrics.host_queries.saturating_add(1);
        }
        let (output, execution_stats) =
            state_ref.execute_optional_singular_query(&self.authority, request, Some(budget))?;
        let projected = output.map(project).transpose()?;
        let payload = Self::encode_norito_payload(&projected)?;
        self.finish_and_materialize_core_query_option(
            vm,
            gas_remaining,
            &gas_ctx,
            (
                execution_stats.processed_items(),
                execution_stats.processed_bytes(),
            ),
            (&payload, payload_words),
            prepare,
        )
    }
    fn execute_core_query_get(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        tag: CoreQueryEntityTagV1,
    ) -> Result<u64, ivm::VMError> {
        let id_pointer = vm.register(11);
        match tag {
            CoreQueryEntityTagV1::Account => {
                let id: AccountId = Self::decode_tlv_typed(vm, id_pointer, PointerType::AccountId)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindAccountById(FindAccountById {
                        id,
                    }));
                self.execute_optional_core_query(
                    vm,
                    syscall,
                    request,
                    |output| match output {
                        SingularQueryOutputBox::Account(account) => {
                            Ok(Self::project_account(account))
                        }
                        _ => Err(ivm::VMError::DecodeError),
                    },
                    Self::ACCOUNT_VIEW_WORDS,
                    |value| Self::prepare_account_view(&value),
                )
            }
            CoreQueryEntityTagV1::Asset => {
                let id: AssetId = Self::decode_tlv_typed(vm, id_pointer, PointerType::AssetId)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindAssetById(FindAssetById { id }));
                self.execute_optional_core_query(
                    vm,
                    syscall,
                    request,
                    |output| match output {
                        SingularQueryOutputBox::Asset(asset) => Ok(Self::project_asset(asset)),
                        _ => Err(ivm::VMError::DecodeError),
                    },
                    Self::ASSET_VIEW_WORDS,
                    |value| Self::prepare_asset_view(&value),
                )
            }
            CoreQueryEntityTagV1::AssetDefinition => {
                let id: AssetDefinitionId =
                    Self::decode_tlv_typed(vm, id_pointer, PointerType::AssetDefinitionId)?;
                let request = QueryRequest::Singular(SingularQueryBox::FindAssetDefinitionById(
                    FindAssetDefinitionById { id },
                ));
                self.execute_optional_core_query(
                    vm,
                    syscall,
                    request,
                    |output| match output {
                        SingularQueryOutputBox::AssetDefinition(definition) => {
                            Ok(Self::project_asset_definition(definition))
                        }
                        _ => Err(ivm::VMError::DecodeError),
                    },
                    Self::ASSET_DEFINITION_VIEW_WORDS,
                    |value| Self::prepare_asset_definition_view(&value),
                )
            }
            CoreQueryEntityTagV1::Domain => {
                let id: DomainId = Self::decode_tlv_typed(vm, id_pointer, PointerType::DomainId)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindDomainById(FindDomainById { id }));
                self.execute_optional_core_query(
                    vm,
                    syscall,
                    request,
                    |output| match output {
                        SingularQueryOutputBox::Domain(domain) => Ok(Self::project_domain(domain)),
                        _ => Err(ivm::VMError::DecodeError),
                    },
                    Self::DOMAIN_VIEW_WORDS,
                    |value| Self::prepare_domain_view(&value),
                )
            }
            CoreQueryEntityTagV1::Nft => {
                let id: NftId = Self::decode_tlv_typed(vm, id_pointer, PointerType::NftId)?;
                let request =
                    QueryRequest::Singular(SingularQueryBox::FindNftById(FindNftById::new(id)));
                self.execute_optional_core_query(
                    vm,
                    syscall,
                    request,
                    |output| match output {
                        SingularQueryOutputBox::Nft(nft) => Ok(Self::project_nft(nft)),
                        _ => Err(ivm::VMError::DecodeError),
                    },
                    Self::NFT_VIEW_WORDS,
                    |value| Self::prepare_nft_view(&value),
                )
            }
        }
    }
    fn execute_core_query_page(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        tag: CoreQueryEntityTagV1,
        offset: i64,
        limit: u64,
    ) -> Result<u64, ivm::VMError> {
        if offset < 0 || !(1..=QUERY_PAGE_CAPACITY_V1 as u64).contains(&limit) {
            return Err(ivm::VMError::DecodeError);
        }
        let limit_i64 = i64::try_from(limit).map_err(|_| ivm::VMError::DecodeError)?;
        offset
            .checked_add(limit_i64)
            .ok_or(ivm::VMError::DecodeError)?;
        let offset_u64 = u64::try_from(offset).map_err(|_| ivm::VMError::DecodeError)?;
        vm.ensure_public_register(13)?;
        let owner_pointer = vm.register(13);
        let (mut filter, filter_gas) = if owner_pointer == 0 {
            (None, 0)
        } else {
            if tag != CoreQueryEntityTagV1::Asset {
                return Err(ivm::VMError::DecodeError);
            }
            let payload_bytes =
                quote_tlv_payload_len_at(vm, owner_pointer, PointerType::AccountId)?;
            let gas = 32_u64.saturating_add(u64::try_from(payload_bytes).unwrap_or(u64::MAX));
            ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
            if gas > vm.remaining_gas() {
                return Err(ivm::VMError::OutOfGas);
            }
            let filter = CoreQueryAccountFilter::decode(vm, owner_pointer)
                .map_err(|error| ivm::VMError::metered(gas, error))?;
            (Some(filter), gas)
        };
        let account = filter.as_mut().and_then(|filter| filter.account.take());
        let request = Self::core_query_page_request(tag, offset_u64, limit, account)?;
        let gas_remaining = vm.remaining_gas();
        let mut gas_ctx = QueryGasContext::from_request(&request);
        gas_ctx.base = gas_ctx.base.saturating_add(filter_gas);
        let budget = Self::query_execution_budget(&gas_ctx, gas_remaining)?;
        if budget.max_items() == 0 {
            return Err(ivm::VMError::OutOfGas);
        }
        let Some(state_ref) = self.query_state.get() else {
            return Err(ivm::VMError::NotImplemented { syscall });
        };
        if let Some(metrics) = &mut self.core_query_page_metrics {
            metrics.host_queries = metrics.host_queries.saturating_add(1);
        }
        let query_result =
            state_ref.execute_bounded_query_with_budget(&self.authority, request, Some(budget))?;
        let QueryResponse::Iterable(output) = query_result.response else {
            return Err(ivm::VMError::DecodeError);
        };
        let has_more = output.has_more;
        let mut columns = output.batch.into_columns();
        if columns.len() != 1 {
            return Err(ivm::VMError::DecodeError);
        }
        let batch = columns.pop().ok_or(ivm::VMError::DecodeError)?;
        match (tag, batch) {
            (CoreQueryEntityTagV1::Account, QueryOutputBatchBox::Account(accounts)) => {
                let items = accounts.into_iter().map(Self::project_account).collect();
                let page = QueryPageV1::from_window(offset, items, has_more)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let payload = Self::encode_norito_payload(&page)?;
                self.finish_and_materialize_core_query_page::<AccountView>(
                    vm,
                    gas_remaining,
                    &gas_ctx,
                    (query_result.processed_items, query_result.processed_bytes),
                    (&payload, Self::ACCOUNT_VIEW_WORDS),
                    |value| Self::prepare_account_view(&value),
                )
            }
            (CoreQueryEntityTagV1::Asset, QueryOutputBatchBox::Asset(assets)) => {
                let items = assets
                    .into_iter()
                    .map(Self::project_asset)
                    .collect::<Vec<_>>();
                let page = QueryPageV1::from_window(offset, items, has_more)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let payload = Self::encode_norito_payload(&page)?;
                self.finish_and_materialize_core_query_page::<AssetView>(
                    vm,
                    gas_remaining,
                    &gas_ctx,
                    (query_result.processed_items, query_result.processed_bytes),
                    (&payload, Self::ASSET_VIEW_WORDS),
                    |value| Self::prepare_asset_view(&value),
                )
            }
            (
                CoreQueryEntityTagV1::AssetDefinition,
                QueryOutputBatchBox::AssetDefinition(definitions),
            ) => {
                let items = definitions
                    .into_iter()
                    .map(Self::project_asset_definition)
                    .collect::<Vec<_>>();
                let page = QueryPageV1::from_window(offset, items, has_more)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let payload = Self::encode_norito_payload(&page)?;
                self.finish_and_materialize_core_query_page::<AssetDefinitionView>(
                    vm,
                    gas_remaining,
                    &gas_ctx,
                    (query_result.processed_items, query_result.processed_bytes),
                    (&payload, Self::ASSET_DEFINITION_VIEW_WORDS),
                    |value| Self::prepare_asset_definition_view(&value),
                )
            }
            (CoreQueryEntityTagV1::Domain, QueryOutputBatchBox::Domain(domains)) => {
                let items = domains.into_iter().map(Self::project_domain).collect();
                let page = QueryPageV1::from_window(offset, items, has_more)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let payload = Self::encode_norito_payload(&page)?;
                self.finish_and_materialize_core_query_page::<DomainView>(
                    vm,
                    gas_remaining,
                    &gas_ctx,
                    (query_result.processed_items, query_result.processed_bytes),
                    (&payload, Self::DOMAIN_VIEW_WORDS),
                    |value| Self::prepare_domain_view(&value),
                )
            }
            (CoreQueryEntityTagV1::Nft, QueryOutputBatchBox::Nft(nfts)) => {
                let items = nfts.into_iter().map(Self::project_nft).collect();
                let page = QueryPageV1::from_window(offset, items, has_more)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                let payload = Self::encode_norito_payload(&page)?;
                self.finish_and_materialize_core_query_page::<NftView>(
                    vm,
                    gas_remaining,
                    &gas_ctx,
                    (query_result.processed_items, query_result.processed_bytes),
                    (&payload, Self::NFT_VIEW_WORDS),
                    |value| Self::prepare_nft_view(&value),
                )
            }
            _ => Err(ivm::VMError::DecodeError),
        }
    }
    fn execute_singular_query_payload(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        request: QueryRequest,
        payload_from_output: impl FnOnce(SingularQueryOutputBox) -> Result<Vec<u8>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError> {
        self.execute_singular_query_payload_as(
            vm,
            syscall,
            PointerType::NoritoBytes,
            request,
            payload_from_output,
        )
    }
    fn execute_singular_query_payload_as(
        &mut self,
        vm: &mut IVM,
        syscall: u32,
        output_type: PointerType,
        request: QueryRequest,
        payload_from_output: impl FnOnce(SingularQueryOutputBox) -> Result<Vec<u8>, ivm::VMError>,
    ) -> Result<u64, ivm::VMError> {
        let gas_remaining = vm.remaining_gas();
        let gas_ctx = QueryGasContext::from_request(&request);
        let Some(state_ref) = self.query_state.get() else {
            return Err(ivm::VMError::NotImplemented { syscall });
        };
        let budget = Self::query_execution_budget(&gas_ctx, gas_remaining)?;
        if budget.max_items() == 0 {
            return Err(ivm::VMError::OutOfGas);
        }
        let query_result = if syscall == ivm::syscalls::SYSCALL_GET_ACCOUNT_BALANCE {
            state_ref.execute_account_balance_query(&self.authority, request, Some(budget))?
        } else {
            state_ref.execute_query_with_budget(&self.authority, request, Some(budget))?
        };
        let output = match query_result.response {
            QueryResponse::Singular(output) => output,
            QueryResponse::Iterable(_) => return Err(ivm::VMError::DecodeError),
        };
        let payload = payload_from_output(output)?;
        let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        let gas = Self::query_gas_cost(
            &gas_ctx,
            query_result.processed_items,
            query_result.processed_bytes.saturating_add(payload_len_u64),
        );
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        if gas > gas_remaining {
            return Err(ivm::VMError::OutOfGas);
        }
        let ptr = Self::alloc_tlv_payload(vm, output_type, &payload)?;
        vm.set_register(10, ptr);
        Ok(gas)
    }
    fn finish_direct_singular_query_payload(
        vm: &mut IVM,
        gas_remaining: u64,
        payload: &[u8],
    ) -> Result<u64, ivm::VMError> {
        let gas_ctx = QueryGasContext::singular();
        let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        let gas = Self::query_gas_cost(&gas_ctx, 1, payload_len_u64);
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        if gas > gas_remaining {
            return Err(ivm::VMError::OutOfGas);
        }
        let ptr = Self::alloc_norito_bytes(vm, payload)?;
        vm.set_register(10, ptr);
        Ok(gas)
    }
    fn gas_for_zk_verify_payload(&self, payload: &[u8]) -> u64 {
        self.zk_gas_schedule.conservative_single_gas(payload.len())
    }
    fn preflight_zk_verify_payload(&self, vm: &IVM, payload: &[u8]) -> Result<u64, ivm::VMError> {
        if u64::try_from(payload.len()).unwrap_or(u64::MAX) > self.zk_gas_schedule.max_payload_bytes
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let gas = self.gas_for_zk_verify_payload(payload);
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        Ok(gas)
    }
    fn decode_metered_zk_envelope(
        &self,
        vm: &IVM,
        payload: &[u8],
    ) -> Result<(Result<iroha_data_model::zk::OpenVerifyEnvelope, u64>, u64), ivm::VMError> {
        let conservative = self.preflight_zk_verify_payload(vm, payload)?;
        let envelope = match ivm::host::decode_canonical_zk_envelope(payload) {
            Ok(envelope) => envelope,
            Err(error) => return Ok((Err(error.into_status()?), conservative)),
        };
        let actual = self
            .zk_gas_schedule
            .actual_single_gas(payload.len(), envelope.public_inputs.len());
        ivm::host::preflight_reserved_syscall_gas(vm, actual)?;
        Ok((Ok(envelope), actual))
    }
    fn lifecycle_hook_is_running(&self) -> bool {
        self.current_contract_runtime_context
            .as_ref()
            .is_some_and(|context| {
                matches!(
                    context.entrypoint.as_str(),
                    "hajimari" | "始まり" | "kaizen" | "改善"
                )
            })
    }
    fn ensure_instruction_queue_allowed(
        &self,
        instruction: &InstructionBox,
    ) -> Result<(), ivm::VMError> {
        crate::executor::ensure_lifecycle_hook_cannot_mutate_contract_binding(
            self.current_contract_runtime_context.as_ref(),
            instruction,
        )
        .map_err(|_| ivm::VMError::PermissionDenied)
    }
    fn decode_opaque_instruction(vm: &IVM) -> Result<InstructionBox, ivm::VMError> {
        let pointer = vm.register(10);
        let tlv = Self::decode_pointer_tlv(vm, pointer, PointerType::NoritoBytes)?;
        decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::NoritoInvalid)
    }
    /// Return the authority recorded on ledger effects emitted by the active frame.
    ///
    /// A contract or proved-contract frame must always retain
    /// `current_contract_runtime_context`; falling back to the transaction authority
    /// inside such a frame would silently re-author contract effects. Generic/view
    /// setup therefore clears the frame and its execution class together, while every
    /// admitted contract binder installs both together.
    fn effect_authority(&self) -> AccountId {
        self.current_contract_runtime_context.as_ref().map_or_else(
            || self.authority.clone(),
            |context| context.contract_subject.clone(),
        )
    }
    fn queue_instruction(&mut self, instr: InstructionBox) -> Result<u64, ivm::VMError> {
        let gas = crate::gas::meter_instruction(&instr, &self.confidential_gas)
            .map_err(ivm::VMError::VerificationDeferred)?;
        self.enqueue_priced_instruction(instr);
        Ok(gas)
    }
    /// Publish an effect only after its caller has obtained the complete gas quote.
    fn enqueue_priced_instruction(&mut self, instr: InstructionBox) {
        let queued = QueuedEffect {
            payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(instr),
            authority: self.effect_authority(),
            contract_runtime_context: self.current_contract_runtime_context.clone(),
            entrypoint_authorization: self.current_entrypoint_authorization.clone(),
        };
        if self.try_reserve_serialized_output(queued.instruction().expect("instruction effect"), 1)
        {
            self.queued.push(queued);
        }
    }
    fn queue_instruction_after_preflight(
        &mut self,
        vm: &IVM,
        instruction: InstructionBox,
    ) -> Result<u64, ivm::VMError> {
        self.ensure_instruction_queue_allowed(&instruction)?;
        let gas = crate::gas::meter_instruction(&instruction, &self.confidential_gas)
            .map_err(ivm::VMError::VerificationDeferred)?;
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        self.enqueue_priced_instruction(instruction);
        Ok(gas)
    }
    fn stage_queued_syscall(
        &mut self,
        vm: &IVM,
        action: impl FnOnce(&mut Self) -> Result<u64, ivm::VMError>,
    ) -> Result<u64, ivm::VMError> {
        let prior = std::mem::take(&mut self.queued);
        let outcome = action(self);
        let staged = std::mem::replace(&mut self.queued, prior);
        let gas = outcome?;
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        self.queued.extend(staged);
        Ok(gas)
    }
    #[cfg(test)]
    fn queue_instructions<I>(&mut self, instrs: I) -> u64
    where
        I: IntoIterator<Item = InstructionBox>,
    {
        let mut gas = 0_u64;
        for instr in instrs {
            gas = gas.saturating_add(
                self.queue_instruction(instr)
                    .expect("completed fixture gas quote"),
            );
        }
        gas
    }
    fn enqueue_fastpq_batch(&mut self, entries: Vec<TransferAssetBatchEntry>) {
        // Entries are accounted incrementally by `push_fastpq_batch_entry` so
        // a long unfinished batch cannot grow without bound.
        self.queued.push(QueuedEffect {
            payload: crate::smartcontracts::ivm::host::QueuedEffectPayload::Instruction(
                InstructionBox::from(TransferAssetBatch::new(entries)),
            ),
            authority: self.effect_authority(),
            contract_runtime_context: self.current_contract_runtime_context.clone(),
            entrypoint_authorization: self.current_entrypoint_authorization.clone(),
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
    fn decode_transfer_v1_args(
        vm: &IVM,
    ) -> Result<(AccountId, AccountId, AssetDefinitionId, Quantity), ivm::VMError> {
        let from_ptr = vm.register(10);
        let to_ptr = vm.register(11);
        let asset_def_ptr = vm.register(12);
        let amount_ptr = vm.register(13);
        let from: AccountId = Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
        let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
        let asset_def: AssetDefinitionId =
            Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
        let amount = Self::decode_quantity(vm, amount_ptr)?;
        Ok((from, to, asset_def, amount))
    }
    fn push_fastpq_batch_entry(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        if self.fastpq_batch_entries.is_none() {
            return Err(ivm::VMError::PermissionDenied);
        }
        let (from, to, asset_def, amount) = Self::decode_transfer_v1_args(vm)?;
        let asset_id = AssetId::of(asset_def.clone(), from.clone());
        let isi = Transfer::asset_quantity(asset_id, amount.clone(), to.clone());
        let instruction = InstructionBox::from(TransferBox::from(isi));
        let gas = crate::gas::meter_instruction(&instruction, &self.confidential_gas)
            .map_err(ivm::VMError::VerificationDeferred)?;
        if self.try_reserve_serialized_output(&instruction, 1) {
            self.fastpq_batch_entries
                .as_mut()
                .expect("batch checked above")
                .push(TransferAssetBatchEntry::new(from, to, asset_def, amount));
        }
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
            decode_canonical_norito(tlv.payload).map_err(|_| ivm::VMError::DecodeError)?;
        if batch.entries().is_empty() {
            return Err(ivm::VMError::DecodeError);
        }
        let instr = InstructionBox::from(batch);
        self.queue_instruction(instr)
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
        if !self.try_reserve_output(
            1,
            descriptor_tlv
                .payload
                .len()
                .saturating_mul(3)
                .saturating_add(256),
        ) {
            return Err(ivm::VMError::PermissionDenied);
        }
        self.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
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
        axt::validate_touch_manifest(&manifest)?;
        let gas = Self::axt_gas(gas_len);
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
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
        let hook_result = self.axt_policy_hook.as_ref().map_or(Ok(()), |policy_hook| {
            policy_hook.allow_touch(dsid, &manifest)
        });
        if let Err(err) = hook_result {
            let lane = self.policy_entry_for(dsid).map(|policy| policy.target_lane);
            self.record_axt_reject_detail(
                AxtRejectReason::PolicyDenied,
                Some(dsid),
                lane,
                "policy hook denied touch manifest",
                None,
                None,
            );
            return Err(err);
        }
        if !self.try_reserve_output(1, gas_len.saturating_mul(2).saturating_add(128)) {
            return Err(ivm::VMError::PermissionDenied);
        }
        let record_result = {
            let state = Arc::make_mut(self.axt_state.as_mut().expect("axt_state checked above"));
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
        Ok(gas)
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
        let dsid: DataSpaceId = Self::decode_tlv_typed(vm, ds_ptr, PointerType::DataSpaceId)?;
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
        let proof_ptr = vm.register(11);
        if proof_ptr == 0 {
            self.reset_axt_proof_cache_for_slot(Some(policy.current_slot));
            let gas = Self::axt_verify_gas(0);
            ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
            let state = Arc::make_mut(self.axt_state.as_mut().expect("axt_state checked above"));
            state.record_proof(dsid, None, Some(policy.current_slot))?;
            if let Some(entry) = Arc::make_mut(&mut self.axt_proof_cache).remove(&dsid) {
                self.clear_axt_proof_cache_state(dsid, &entry);
            }
            self.note_axt_proof_cache_event(AXT_PROOF_CACHE_CLEARED);
            return Ok(gas);
        }
        let proof_tlv = Self::expect_tlv(vm, proof_ptr, PointerType::ProofBlob)?;
        let gas = Self::axt_verify_gas(proof_tlv.payload.len());
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        let proof: ProofBlob = decode_canonical_norito(proof_tlv.payload).map_err(|_err| {
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
        axt::validate_proof_blob(&proof)?;
        self.validate_axt_proof(
            dsid,
            &proof,
            policy,
            AxtProofAdmission::StandaloneUnanchored,
        )?;
        Ok(gas)
    }
    fn handle_axt_stage_anchored_spend(&mut self, vm: &IVM) -> Result<u64, ivm::VMError> {
        const MAX_SPEND_FRAME_BYTES_V1: usize = 4 * 1024 * 1024;

        self.clear_axt_reject();
        let Some(state_view) = self.axt_state.as_ref() else {
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_STAGE_ANCHORED_SPEND invoked before AXT_BEGIN",
            );
            return Err(ivm::VMError::PermissionDenied);
        };
        let tlv = Self::expect_tlv(vm, vm.register(10), PointerType::AxtAnchoredSpendV1)?;
        // The canonical proof is capped at 2 MiB; this larger envelope ceiling
        // leaves room for signed source metadata while bounding decode work.
        if tlv.payload.len() > MAX_SPEND_FRAME_BYTES_V1 {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let gas = Self::axt_gas(tlv.payload.len());
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        let spend: AxtAnchoredSpendV1 = decode_canonical_norito(tlv.payload)?;
        let dsid = spend.draft.intent.asset_dsid;
        if !state_view.expected_dsids().contains(&dsid)
            || spend.draft.handle.axt_binding != AxtBinding::new(state_view.binding())
            || spend.issuer_payload_v1().is_err()
        {
            self.record_axt_reject(
                AxtRejectReason::Proof,
                Some(dsid),
                Some(spend.draft.handle.target_lane),
                "signed anchored spend has inconsistent public binding",
            );
            return Err(ivm::VMError::PermissionDenied);
        }
        // This is only a producer for the signed wire. State remains the sole
        // owner of finalized-source, issuer-key, nonce, budget and effect
        // admission, and currently rejects every nonempty spend before effects.
        if !self.try_reserve_output(1, tlv.payload.len().saturating_add(128)) {
            return Err(ivm::VMError::PermissionDenied);
        }
        Arc::make_mut(self.axt_state.as_mut().expect("active envelope checked"))
            .record_spend(spend)?;
        Ok(gas)
    }
    fn handle_axt_commit(&mut self, vm: &IVM) -> Result<u64, ivm::VMError> {
        let Some(state) = self.axt_state.as_ref() else {
            self.clear_axt_reject();
            self.record_axt_reject(
                AxtRejectReason::Descriptor,
                None,
                None,
                "AXT_COMMIT invoked before AXT_BEGIN",
            );
            return Err(ivm::VMError::PermissionDenied);
        };
        let gas = Self::axt_commit_gas(state);
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        self.clear_axt_reject();
        let state = self
            .axt_state
            .take()
            .expect("axt_state checked before gas preflight");
        if let Some(analysis) = &self.amx_analysis
            && let Some(ds_count) = NonZeroUsize::new(state.expected_dsids().len())
            && let Err(err) = analysis::enforce_amx_budget(analysis, ds_count, &self.amx_limits)
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
                telemetry.inc_amx_abort(LaneId::new(0), "budget");
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
        if let Err(error) = state.validate_commit() {
            self.axt_state = Some(state);
            return Err(error);
        }
        let state = Arc::try_unwrap(state).unwrap_or_else(|state| (*state).clone());
        self.completed_axt.push(state);
        Ok(gas)
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
        AxtEnvelopeRecord {
            binding,
            lane,
            descriptor,
            touches,
            proofs,
            // Preserve the exact signed wire and VM call order. State alone
            // authenticates source finality, issuer authority, and replay.
            spends: state.spends().to_vec(),
            commit_height,
        }
    }
    #[cfg(test)]
    fn flush_durable_state(
        &mut self,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), ValidationFail> {
        if self.durable_state_overlay.is_empty() {
            return Ok(());
        }
        for (path, value) in &self.durable_state_overlay {
            if let Some(authorization) = self
                .durable_state_authorizations
                .get(path)
                .and_then(Option::as_ref)
            {
                authorization
                    .validate(&tx.world)
                    .map_err(|error| tx.attempt_error_to_validation_fail(error))?;
            }
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
        self.durable_state_authorizations.clear();
        Ok(())
    }
    #[cfg(any(test, feature = "iroha-core-tests"))]
    /// Execute a closure with a mutable reference to the [`CoreHost`] attached to `vm`.
    ///
    /// Used in tests to access the host state without juggling raw pointers.
    pub fn with_host<R>(vm: &mut IVM, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: 'static,
    {
        let host_any = vm.host_mut_any().expect("CoreHost not attached to VM");
        let host = host_any
            .downcast_mut::<Self>()
            .expect("host type mismatch: expected CoreHostImpl variant");
        f(host)
    }
}
impl<QRef> CoreHostImpl<'_, '_, QueryStateSlot<QRef>>
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
    /// Attach the query state and bind `VRF_EPOCH_SEED` to it.
    ///
    /// `epoch_length` must be [`vrf_epoch_length_from_state`] of this same
    /// `state`, read immediately before. The state stays borrowed while the
    /// host lives, so it cannot change before the first `VRF_EPOCH_SEED` use.
    /// That use scans the pinned state and verifies only the requested epoch,
    /// observing exactly the seed [`CoreHostImpl::set_vrf_epoch_seeds_from_state`]
    /// would install for this state; invocations that never read a seed pay
    /// for neither the scan nor any signature check.
    pub fn set_query_state_with_vrf_epoch_seeds<'block, S>(
        &mut self,
        state: &'block S,
        epoch_length: Option<NonZeroU64>,
    ) where
        S: QueryStateSource<Ref<'block> = QRef>,
    {
        self.set_query_state(state);
        self.query_state.vrf_epoch_seed_state = Some(state.as_query_state_ref());
        self.vrf_epoch_seeds =
            epoch_length.map_or_else(VrfEpochSeedSource::default, |epoch_length| {
                VrfEpochSeedSource::QueryState {
                    epoch_length,
                    projection: std::sync::OnceLock::new(),
                }
            });
    }
}
impl<QS> CoreHostImpl<'_, '_, QS> {
    fn bound_contract_dispatch_identity<R: StateReadOnly>(
        state: &R,
        contract_address: &ContractAddress,
    ) -> Result<
        Option<(
            crate::smartcontracts::code::BoundContractIdentity,
            AccountId,
            usize,
        )>,
        ivm::VMError,
    > {
        crate::executor::root_scope::ensure_committed_contract_scope(
            state.world(),
            contract_address,
        )
        .map_err(|error| error.into_vm_error(|_| ivm::VMError::PermissionDenied))?;
        let Some(identity) =
            crate::smartcontracts::code::fetch_bound_contract_identity(state, contract_address)
                .map_err(|error| error.into_vm_error(|_| ivm::VMError::PermissionDenied))?
        else {
            return Ok(None);
        };
        let Some(subject) =
            crate::smartcontracts::code::fetch_bound_contract_subject(state, contract_address)
        else {
            return Ok(None);
        };
        let Ok(dataspace) = identity.contract_address.dataspace_id() else {
            return Ok(None);
        };
        let Some(bytes) = state.world().contract_code().get(
            &iroha_data_model::smart_contract::ContractArtifactId::new(
                dataspace,
                identity.code_hash,
            ),
        ) else {
            return Ok(None);
        };
        Ok(Some((identity, subject, bytes.len())))
    }
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
        )
        .map_err(|error| {
            error
                .into_attempt_error(|_| ivm::VMError::DecodeError)
                .into_vm_error(core::convert::identity)
        })? {
            return Err(ivm::VMError::PermissionDenied);
        }
        let now_ms = state
            .latest_block()
            .map_err(|error| error.into_vm_error(|_| ivm::VMError::DecodeError))?
            .map_or(0, |block| {
                u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
            });
        if let Some(account_id) = crate::sns::resolve_active_account_alias(
            state.world(),
            &state.nexus().dataspace_catalog,
            &alias_label,
            now_ms,
        )
        .map_err(|error| {
            error
                .into_attempt_error(|_| ivm::VMError::DecodeError)
                .into_vm_error(core::convert::identity)
        })? {
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
        Action::new(executable, repeats, authority, filter)
            .map(|action| action.with_metadata(metadata))
            .map_err(|_| ivm::VMError::DecodeError)
    }
    fn escrow_id_from_name(name: &Name) -> EscrowId {
        EscrowId::from_kotodama_name(name)
    }
    fn is_codec_forwarded_syscall(number: u32) -> bool {
        if ivm::syscalls::is_numeric_v1_syscall(number) {
            return true;
        }
        matches!(
            number,
            ivm::syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
                | ivm::syscalls::SYSCALL_STATE_PATH_FROM_NAME
                | ivm::syscalls::SYSCALL_STATE_MAP_KEY_AT
                | ivm::syscalls::SYSCALL_VALUE_ENCODE
                | ivm::syscalls::SYSCALL_BLOB_CONCAT
                | ivm::syscalls::SYSCALL_UTF8_VALIDATE
                | ivm::syscalls::SYSCALL_VALUE_TO_STRING
                | ivm::syscalls::SYSCALL_STATE_VALUE_ENCODE
                | ivm::syscalls::SYSCALL_STATE_VALUE_DECODE
                | ivm::syscalls::SYSCALL_NORMALIZE_NORITO_BYTES
                | ivm::syscalls::SYSCALL_JSON_ENCODE
                | ivm::syscalls::SYSCALL_JSON_DECODE
                | ivm::syscalls::SYSCALL_JSON_OBJECT
                | ivm::syscalls::SYSCALL_JSON_BUILD
                | ivm::syscalls::SYSCALL_JSON_SET_I64
                | ivm::syscalls::SYSCALL_JSON_SET_ACCOUNT_ID
                | ivm::syscalls::SYSCALL_TLV_LEN
                | ivm::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD
                | ivm::syscalls::SYSCALL_JSON_GET_JSON
                | ivm::syscalls::SYSCALL_JSON_GET_NAME
                | ivm::syscalls::SYSCALL_JSON_GET_ACCOUNT_ID
                | ivm::syscalls::SYSCALL_JSON_GET_NFT_ID
                | ivm::syscalls::SYSCALL_JSON_GET_BLOB_HEX
                | ivm::syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID
                | ivm::syscalls::SYSCALL_JSON_GET_INT
                | ivm::syscalls::SYSCALL_JSON_GET_DECIMAL
                | ivm::syscalls::SYSCALL_JSON_GET_QUANTITY
                | ivm::syscalls::SYSCALL_JSON_GET_STRING
                | ivm::syscalls::SYSCALL_JSON_GET_BOOL
                | ivm::syscalls::SYSCALL_SCHEMA_ENCODE
                | ivm::syscalls::SYSCALL_SCHEMA_DECODE
                | ivm::syscalls::SYSCALL_POINTER_TO_NORITO
                | ivm::syscalls::SYSCALL_POINTER_FROM_NORITO
                | ivm::syscalls::SYSCALL_TLV_EQ
                | ivm::syscalls::SYSCALL_SCHEMA_INFO
                | ivm::syscalls::SYSCALL_NAME_DECODE
        )
    }
    fn is_default_forwarded_syscall(number: u32) -> bool {
        matches!(
            number,
            ivm::syscalls::SYSCALL_DEBUG_PRINT
                | ivm::syscalls::SYSCALL_EXIT
                | ivm::syscalls::SYSCALL_ABORT
                | ivm::syscalls::SYSCALL_CONTRACT_ABORT
                | ivm::syscalls::SYSCALL_DEBUG_LOG
                | ivm::syscalls::SYSCALL_ALLOC
                | ivm::syscalls::SYSCALL_GROW_HEAP
                | ivm::syscalls::SYSCALL_PRIVATE_NUMERIC_VALCOM
                | ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV
                | ivm::syscalls::SYSCALL_COMMIT_OUTPUT
                | ivm::syscalls::SYSCALL_EXECUTION_SUMMARY
                | ivm::syscalls::SYSCALL_VERIFY_SIGNATURE
                | ivm::syscalls::SYSCALL_SHA256_HASH
                | ivm::syscalls::SYSCALL_SHA3_HASH
                | ivm::syscalls::SYSCALL_BLAKE2B256_HASH
                | ivm::syscalls::SYSCALL_KECCAK256_HASH
                | ivm::syscalls::SYSCALL_IROHA_HASH
                | ivm::syscalls::SYSCALL_GET_MERKLE_PATH
                | ivm::syscalls::SYSCALL_GET_MERKLE_COMPACT
                | ivm::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT
                | ivm::syscalls::SYSCALL_VRF_VERIFY
                | ivm::syscalls::SYSCALL_VRF_VERIFY_BATCH
        )
    }
}
impl<QS: QueryStateAccess + Default> IVMHost for CoreHostImpl<'_, '_, QS> {
    fn prepared_entrypoint_arguments(&self) -> Option<ivm::PreparedArgumentRecord> {
        self.entrypoint_argument_record.clone()
    }
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, ivm::VMError> {
        let metering = ivm::host::require_host_syscall_metering_spec(vm.syscall_policy(), number)?;
        self.execution_class.ensure_syscall_allowed(number)?;
        // Consensus hosts never receive raw private witnesses. Keep this
        // defense-in-depth boundary independent of entrypoint admission so a
        // future resolver regression cannot turn DefaultHost's prover/test
        // transport into a production witness oracle.
        if number == ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT {
            return Err(ivm::VMError::PermissionDenied);
        }
        if self.lifecycle_hook_is_running()
            && matches!(
                number,
                ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE
                    | ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE
            )
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        let state_quote = match number {
            ivm::syscalls::SYSCALL_STATE_LEN
            | ivm::syscalls::SYSCALL_STATE_DEL
            | ivm::syscalls::SYSCALL_STATE_HAS => {
                let path_len = ivm::host::quote_state_path_payload_len_at(vm, vm.register(10))?;
                Some(ivm::host::state_path_gas(path_len))
            }
            ivm::syscalls::SYSCALL_STATE_GET | ivm::syscalls::SYSCALL_STATE_COUNT => {
                let path_len = ivm::host::quote_state_path_payload_len_at(vm, vm.register(10))?;
                Some(ivm::host::reserve_available_syscall_gas_at_least(
                    vm,
                    ivm::host::state_path_gas(path_len),
                )?)
            }
            ivm::syscalls::SYSCALL_STATE_SCAN => {
                Some(ivm::host::reserve_available_syscall_gas_at_least(
                    vm,
                    ivm::state_scan::prepare_minimum(vm)?,
                )?)
            }
            ivm::syscalls::SYSCALL_STATE_SET => {
                let path_len = ivm::host::quote_state_path_payload_len_at(vm, vm.register(10))?;
                let value_len =
                    quote_tlv_payload_len_at(vm, vm.register(11), PointerType::NoritoBytes)?;
                ivm::host::validate_state_value_payload_len(value_len)?;
                Some(ivm::host::state_value_gas(path_len, value_len))
            }
            _ => None,
        };
        if let Some(quote) = state_quote {
            return Ok(quote);
        }
        if Self::is_codec_forwarded_syscall(number) {
            return self.codec_host.prepare_syscall(number, vm);
        }
        if Self::is_default_forwarded_syscall(number) {
            return self.default.prepare_syscall(number, vm);
        }
        if matches!(
            number,
            ivm::syscalls::SYSCALL_SM3_HASH
                | ivm::syscalls::SYSCALL_SM2_VERIFY
                | ivm::syscalls::SYSCALL_SM4_GCM_SEAL
                | ivm::syscalls::SYSCALL_SM4_GCM_OPEN
                | ivm::syscalls::SYSCALL_SM4_CCM_SEAL
                | ivm::syscalls::SYSCALL_SM4_CCM_OPEN
        ) {
            let quote = self.default.prepare_syscall(number, vm);
            // Invalid inputs rejected during quoting never reach syscall dispatch.
            #[cfg(feature = "telemetry")]
            if quote.is_err()
                && let Some(telemetry) = self.telemetry.as_ref()
            {
                Self::record_sm_syscall(telemetry, number, &quote);
            }
            return quote;
        }
        let response_quote = match number {
            ivm::syscalls::SYSCALL_GET_AUTHORITY | ivm::syscalls::SYSCALL_SYSVAR_AUTHORITY => {
                if let Some(payload_len) = Self::norito_encoded_len_exact(&self.authority)
                    .and_then(|len| usize::try_from(len).ok())
                {
                    Some(Self::sysvar_gas(payload_len))
                } else {
                    Some(ivm::host::reserve_available_syscall_gas(vm)?)
                }
            }
            ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_SUBJECT => {
                if let Some(context) = self.current_contract_runtime_context.as_ref() {
                    if let Some(payload_len) =
                        Self::norito_encoded_len_exact(&context.contract_subject)
                            .and_then(|len| usize::try_from(len).ok())
                    {
                        Some(Self::sysvar_gas(payload_len))
                    } else {
                        Some(ivm::host::reserve_available_syscall_gas(vm)?)
                    }
                } else {
                    return Err(ivm::VMError::PermissionDenied);
                }
            }
            ivm::syscalls::SYSCALL_CURRENT_TIME_MS
            | ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS
            | ivm::syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT => Some(Self::sysvar_gas(0)),
            ivm::syscalls::SYSCALL_SYSVAR_CHAIN_ID => {
                Some(Self::sysvar_gas(self.chain_id_bytes.len()))
            }
            ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS => {
                self.current_contract_runtime_context.as_ref().map_or_else(
                    || Some(Self::sysvar_gas(0)),
                    |context| Some(Self::sysvar_gas(context.contract_address.as_str().len())),
                )
            }
            ivm::syscalls::SYSCALL_SYSVAR_ENTRYPOINT => Some(Self::sysvar_gas(
                self.current_contract_runtime_context
                    .as_ref()
                    .map_or(0, |context| context.entrypoint.len()),
            )),
            ivm::syscalls::SYSCALL_GET_PUBLIC_INPUT
            | ivm::syscalls::SYSCALL_ZK_ROOTS_GET
            | ivm::syscalls::SYSCALL_ZK_VOTE_GET_TALLY
            | ivm::syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS
            | ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION
            | ivm::syscalls::SYSCALL_SET_ASSET_TRANSFER_AVAILABILITY
            | ivm::syscalls::SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT
            | ivm::syscalls::SYSCALL_SET_ASSET_HOLDING_LIMIT
            | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_PROPOSE
            | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_APPROVE
            | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_CANCEL
            | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_FINALIZE
            | ivm::syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH
            | ivm::syscalls::SYSCALL_AXT_TOUCH
            | ivm::syscalls::SYSCALL_VERIFY_DS_PROOF
            | ivm::syscalls::SYSCALL_AXT_STAGE_ANCHORED_SPEND
            | ivm::syscalls::SYSCALL_AXT_COMMIT => {
                Some(ivm::host::reserve_available_syscall_gas(vm)?)
            }
            ivm::syscalls::SYSCALL_EMIT_CONTRACT_EVENT => Some(self.quote_contract_event(vm)?),
            ivm::syscalls::SYSCALL_CALL_CONTRACT => {
                Self::quote_contract_call_descriptors(vm)?;
                Some(ivm::host::reserve_available_syscall_gas(vm)?)
            }
            ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY
            | ivm::syscalls::SYSCALL_VERIFY_PROOF => {
                Some(ivm::host::quote_zk_single_at(vm, vm.register(10), self.zk_gas_schedule)?.1)
            }
            ivm::syscalls::SYSCALL_ZK_VERIFY_BATCH => {
                Some(ivm::host::quote_zk_batch_at(vm, vm.register(10), self.zk_gas_schedule)?.gas)
            }
            ivm::syscalls::SYSCALL_VRF_EPOCH_SEED => {
                vm.ensure_public_register(10)?;
                Some(ivm::vrf::epoch_seed_gas(
                    self.vrf_epoch_seed(vm.register(10)).is_some(),
                ))
            }
            ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED => {
                if self.fastpq_batch_entries.is_some() {
                    return Err(ivm::VMError::PermissionDenied);
                }
                for (register, pointer_type) in [
                    (10, PointerType::AccountId),
                    (11, PointerType::AccountId),
                    (12, PointerType::AssetDefinitionId),
                    (13, PointerType::Quantity),
                    (14, PointerType::DataSpaceId),
                ] {
                    vm.ensure_public_register(register)?;
                    // r14 = 0 means the call named no dataspace scope.
                    if register == 14 && vm.register(register) == 0 {
                        continue;
                    }
                    quote_tlv_payload_len_at(vm, vm.register(register), pointer_type)?;
                }
                // Either balance policy queues one native asset transfer. It has
                // a fixed instruction charge and publishes no VM return bytes;
                // reserving the generic host-output envelope would reject calls
                // that can pay for their complete execution.
                Some(crate::gas::BASE_TRANSFER)
            }
            ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN
            | ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => Some(ivm::gas::G_FASTPQ_BATCH),
            ivm::syscalls::SYSCALL_AXT_BEGIN => {
                let payload_len =
                    quote_tlv_payload_len_at(vm, vm.register(10), PointerType::AxtDescriptor)?;
                Some(Self::axt_gas(payload_len))
            }
            ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE
            | ivm::syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION
            | ivm::syscalls::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE
            | ivm::syscalls::SYSCALL_SORACLOUD_APPEND_JOURNAL
            | ivm::syscalls::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT
            | ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG
            | ivm::syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => {
                let request_len =
                    quote_tlv_payload_len_at(vm, vm.register(10), PointerType::SoracloudRequest)?;
                Some(ivm::gas::syscall_byte_gas(
                    ivm::gas::G_SORACLOUD,
                    request_len,
                    0,
                ))
            }
            _ => None,
        };
        if let Some(quote) = response_quote {
            return Ok(quote);
        }
        if matches!(
            number,
            ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY
                | ivm::syscalls::SYSCALL_QUERY_EXECUTE_NORITO
                | ivm::syscalls::SYSCALL_CORE_QUERY_GET
                | ivm::syscalls::SYSCALL_CORE_QUERY_PAGE
                | ivm::syscalls::SYSCALL_QUERY_GET_PARAMETER
                | ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST
                | ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE
                | ivm::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS
                | ivm::syscalls::SYSCALL_GET_ACCOUNT_BALANCE
                | ivm::syscalls::SYSCALL_ASSET_UID_HOLDING_QUOTE
                | ivm::syscalls::SYSCALL_ASSET_FI_LINK_REVISION
                | ivm::syscalls::SYSCALL_ASSET_SAME_UID
                | ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL
                | ivm::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE
        ) {
            // Query execution derives its item budget from the caller's gas and
            // checks the encoded response cost before returning. Reserving that
            // entire budget is therefore a tight upper bound even when the
            // world-state cardinality is not available without running a query.
            return ivm::host::reserve_available_syscall_gas_at_least(vm, metering.minimum_gas);
        }
        match metering.quote_strategy {
            ivm::host::HostSyscallQuoteStrategy::ReserveAvailable => {
                ivm::host::reserve_available_syscall_gas_at_least(vm, metering.minimum_gas)
            }
            ivm::host::HostSyscallQuoteStrategy::InputOutputBounded
            | ivm::host::HostSyscallQuoteStrategy::AllocationExtent => {
                // Forwarded exact/allocation paths returned above. Remaining
                // instruction-producing calls use the deterministic envelope
                // bound and cannot silently inherit a reserve strategy.
                Ok(ivm::host::conservative_syscall_gas_quote(number, vm))
            }
        }
    }
    #[allow(clippy::too_many_lines)]
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        self.ensure_output_budget()?;
        let result = (|| {
            // Enforce both ABI policy and exhaustive metering classification for
            // direct host calls as well as VM-dispatched execution.
            ivm::host::require_host_syscall_metering_spec(vm.syscall_policy(), number)?;
            self.execution_class.ensure_syscall_allowed(number)?;
            // See `prepare_syscall`: production consensus execution has no raw
            // witness transport, even if a caller bypasses the staged VM path and
            // invokes the host method directly.
            if number == ivm::syscalls::SYSCALL_GET_PRIVATE_INPUT {
                return Err(ivm::VMError::PermissionDenied);
            }
            if self.lifecycle_hook_is_running()
                && matches!(
                    number,
                    ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE
                        | ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE
                )
            {
                return Err(ivm::VMError::PermissionDenied);
            }
            match number {
                // ----------------- Domain ISIs via pointer-ABI -----------------
                ivm::syscalls::SYSCALL_REGISTER_DOMAIN => {
                    let ptr = vm.register(10);
                    let id: DomainId = Self::decode_tlv_typed(vm, ptr, PointerType::DomainId)?;
                    let isi = Register::domain(Domain::new(id));
                    let instr = InstructionBox::from(RegisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_UNREGISTER_DOMAIN => {
                    let ptr = vm.register(10);
                    let id: DomainId = Self::decode_tlv_typed(vm, ptr, PointerType::DomainId)?;
                    let isi = Unregister::domain(id);
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                // ----------------- Peer ISIs via pointer-ABI -----------------
                ivm::syscalls::SYSCALL_REGISTER_PEER => {
                    let ptr = vm.register(10);
                    let payload: Json = Self::decode_tlv_json(vm, ptr)?;
                    let request = Self::register_peer_from_json(&payload)?;
                    let instr = InstructionBox::from(request);
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                // ----------------- Account ISIs via pointer-ABI -----------------
                ivm::syscalls::SYSCALL_REGISTER_ACCOUNT => {
                    let ptr = vm.register(10);
                    let id: AccountId = Self::decode_tlv_typed(vm, ptr, PointerType::AccountId)?;
                    let isi = Register::account(Account::new(id));
                    let instr = InstructionBox::from(RegisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_UNREGISTER_ACCOUNT => {
                    let ptr = vm.register(10);
                    let id: AccountId = Self::decode_tlv_typed(vm, ptr, PointerType::AccountId)?;
                    let isi = Unregister::account(id);
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                // ----------------- Asset quantity ISIs via pointer-ABI -----------------
                ivm::syscalls::SYSCALL_REGISTER_ASSET => {
                    // r10 = &AssetDefinitionId, r11 = &Blob UTF-8 display name,
                    // r12 = numeric-spec word, r13 = mintability word, r14 = &DomainId.
                    // The explicit immutable home is authorized by the executor. The data-model
                    // `ivm_registration::decode_registration` decoder is shared with
                    // every host.
                    let id: AssetDefinitionId = Self::decode_tlv_typed(
                        vm,
                        vm.register(10),
                        PointerType::AssetDefinitionId,
                    )?;
                    let registration =
                        iroha_data_model::asset::definition::ivm_registration::decode_registration(
                            &Self::decode_tlv_blob(vm, vm.register(11))?,
                            vm.register(12),
                            vm.register(13),
                        )
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    let owning_domain: DomainId =
                        Self::decode_tlv_typed(vm, vm.register(14), PointerType::DomainId)?;
                    let definition = AssetDefinition::new(
                        id,
                        registration.name,
                        registration.spec,
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        Some(owning_domain),
                    )
                    .with_mintable(registration.mintable);
                    let isi = Register::asset_definition(definition);
                    let instr = InstructionBox::from(RegisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_UNREGISTER_ASSET => {
                    let ptr = vm.register(10);
                    let id: AssetDefinitionId =
                        Self::decode_tlv_typed(vm, ptr, PointerType::AssetDefinitionId)?;
                    let isi = Unregister::asset_definition(id);
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_MINT_ASSET => {
                    let account_ptr = vm.register(10);
                    let asset_def_ptr = vm.register(11);
                    let amount_ptr = vm.register(12);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let asset_def: AssetDefinitionId =
                        Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                    let amount = Self::decode_quantity(vm, amount_ptr)?;
                    let asset_id = AssetId::of(asset_def, account);
                    let isi = Mint::asset_quantity(amount, asset_id);
                    let instr = InstructionBox::from(MintBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_BURN_ASSET => {
                    let account_ptr = vm.register(10);
                    let asset_def_ptr = vm.register(11);
                    let amount_ptr = vm.register(12);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let asset_def: AssetDefinitionId =
                        Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                    let amount = Self::decode_quantity(vm, amount_ptr)?;
                    let asset_id = AssetId::of(asset_def, account);
                    let isi = Burn::asset_quantity(amount, asset_id);
                    let instr = InstructionBox::from(BurnBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_TRANSFER_V1 => {
                    if self.fastpq_batch_entries.is_some() {
                        return self.push_fastpq_batch_entry(vm);
                    }
                    Err(ivm::VMError::PermissionDenied)
                }
                ivm::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED => {
                    if self.fastpq_batch_entries.is_some() {
                        return Err(ivm::VMError::PermissionDenied);
                    }
                    let from_ptr = vm.register(10);
                    let to_ptr = vm.register(11);
                    let asset_def_ptr = vm.register(12);
                    let amount_ptr = vm.register(13);
                    let dataspace_ptr = vm.register(14);
                    let from: AccountId =
                        Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
                    let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
                    let asset_def: AssetDefinitionId =
                        Self::decode_tlv_typed(vm, asset_def_ptr, PointerType::AssetDefinitionId)?;
                    let amount = Self::decode_quantity(vm, amount_ptr)?;
                    // r14 = 0 means the call named no `dataspace:`.
                    let dataspace: Option<DataSpaceId> = match dataspace_ptr {
                        0 => None,
                        _ => Some(Self::decode_tlv_typed(
                            vm,
                            dataspace_ptr,
                            PointerType::DataSpaceId,
                        )?),
                    };
                    let scope = {
                        let state_ref = self
                            .query_state
                            .get()
                            .ok_or(ivm::VMError::PermissionDenied)?;
                        transfer_scope_for_policy(
                            state_ref.asset_balance_policy(&asset_def)?,
                            dataspace,
                        )?
                    };
                    let asset_id = AssetId::with_scope(asset_def, from, scope);
                    let isi = Transfer::asset_quantity(asset_id, amount, to);
                    let instr = InstructionBox::from(TransferBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_SET_ASSET_TRANSFER_AVAILABILITY => {
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, vm.register(10), PointerType::AccountId)?;
                    let asset_definition: AssetDefinitionId = Self::decode_tlv_typed(
                        vm,
                        vm.register(11),
                        PointerType::AssetDefinitionId,
                    )?;
                    let expected_revision = vm.register(12);
                    let availability_flags = vm.register(13);
                    if availability_flags & !0b11 != 0 {
                        return Err(ivm::VMError::DecodeError);
                    }
                    let availability = |mask| {
                        if availability_flags & mask == 0 {
                            iroha_data_model::asset::AssetTransferAvailability::Disabled
                        } else {
                            iroha_data_model::asset::AssetTransferAvailability::Enabled
                        }
                    };
                    let incoming = availability(0b01);
                    let outgoing = availability(0b10);
                    let reason = Self::decode_optional_string(vm, vm.register(14))?;
                    let instruction = iroha_data_model::isi::SetAssetTransferAvailability::new(
                        account,
                        asset_definition,
                        expected_revision,
                        incoming,
                        outgoing,
                        reason,
                    );
                    self.queue_instruction_after_preflight(vm, InstructionBox::from(instruction))
                }
                ivm::syscalls::SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT => {
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, vm.register(10), PointerType::AccountId)?;
                    let asset_definition: AssetDefinitionId = Self::decode_tlv_typed(
                        vm,
                        vm.register(11),
                        PointerType::AssetDefinitionId,
                    )?;
                    let cap = Self::decode_optional_amount(vm, vm.register(12))?;
                    let instruction = iroha_data_model::isi::SetAssetTransferControl::new(
                        account,
                        asset_definition,
                        vec![iroha_data_model::asset::AssetTransferLimit {
                            window: iroha_data_model::asset::AssetTransferControlWindow::Day,
                            cap_amount: cap,
                        }],
                    );
                    self.queue_instruction_after_preflight(vm, InstructionBox::from(instruction))
                }
                ivm::syscalls::SYSCALL_SET_ASSET_HOLDING_LIMIT => {
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, vm.register(10), PointerType::AccountId)?;
                    let asset_definition: AssetDefinitionId = Self::decode_tlv_typed(
                        vm,
                        vm.register(11),
                        PointerType::AssetDefinitionId,
                    )?;
                    let limit = Self::decode_optional_amount(vm, vm.register(12))?;
                    let instruction = iroha_data_model::isi::SetAssetHoldingLimit::new(
                        account,
                        asset_definition,
                        limit,
                    );
                    self.queue_instruction_after_preflight(vm, InstructionBox::from(instruction))
                }
                ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_PROPOSE => {
                    let alias_bytes = Self::decode_tlv_blob(vm, vm.register(10))?;
                    let alias_literal =
                        std::str::from_utf8(&alias_bytes).map_err(|_| ivm::VMError::DecodeError)?;
                    let alias = self
                        .query_state
                        .get()
                        .ok_or(ivm::VMError::PermissionDenied)?
                        .parse_account_alias(alias_literal)?;
                    let replacement: AccountId =
                        Self::decode_tlv_typed(vm, vm.register(11), PointerType::AccountId)?;
                    let request_generation = std::num::NonZeroU64::new(vm.register(12))
                        .ok_or(ivm::VMError::DecodeError)?;
                    let instruction =
                        iroha_data_model::isi::account_recovery::ProposeAccountRecovery {
                            alias,
                            new_controller: replacement.controller().clone(),
                            request_generation,
                        };
                    self.queue_instruction_after_preflight(vm, InstructionBox::from(instruction))
                }
                ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_APPROVE
                | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_CANCEL
                | ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_FINALIZE => {
                    let alias_bytes = Self::decode_tlv_blob(vm, vm.register(10))?;
                    let alias_literal =
                        std::str::from_utf8(&alias_bytes).map_err(|_| ivm::VMError::DecodeError)?;
                    let alias = self
                        .query_state
                        .get()
                        .ok_or(ivm::VMError::PermissionDenied)?
                        .parse_account_alias(alias_literal)?;
                    let request_generation = std::num::NonZeroU64::new(vm.register(11))
                        .ok_or(ivm::VMError::DecodeError)?;
                    let instruction = match number {
                        ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_APPROVE => InstructionBox::from(
                            iroha_data_model::isi::account_recovery::ApproveAccountRecovery {
                                alias,
                                request_generation,
                            },
                        ),
                        ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_CANCEL => InstructionBox::from(
                            iroha_data_model::isi::account_recovery::CancelAccountRecovery {
                                alias,
                                request_generation,
                            },
                        ),
                        ivm::syscalls::SYSCALL_ACCOUNT_RECOVERY_FINALIZE => InstructionBox::from(
                            iroha_data_model::isi::account_recovery::FinalizeAccountRecovery {
                                alias,
                                request_generation,
                            },
                        ),
                        _ => unreachable!("matched account-recovery syscall"),
                    };
                    self.queue_instruction_after_preflight(vm, instruction)
                }
                ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
                ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
                ivm::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => {
                    self.apply_fastpq_batch_from_tlv(vm)
                }
                ivm::syscalls::SYSCALL_ASSET_UID_HOLDING_QUOTE => {
                    self.execute_uid_holding_quote(vm, number)
                }
                ivm::syscalls::SYSCALL_TRANSFER_TO_LINKED_FI => self.transfer_to_linked_fi(vm),
                ivm::syscalls::SYSCALL_ASSET_LOCK_DRAWDOWN_WATERFALL
                | ivm::syscalls::SYSCALL_ASSET_LOCK_CANCEL_WATERFALL
                | ivm::syscalls::SYSCALL_ASSET_LOCK_EXPIRE_WATERFALL => {
                    self.controlled_asset_lock_with_waterfall(vm, number)
                }
                ivm::syscalls::SYSCALL_ASSET_FI_LINK_REVISION => {
                    self.execute_fi_link_revision(vm, number)
                }
                ivm::syscalls::SYSCALL_ASSET_SAME_UID => self.execute_same_uid(vm, number),
                ivm::syscalls::SYSCALL_ASSET_LOCK_OPEN
                | ivm::syscalls::SYSCALL_ASSET_LOCK_DRAWDOWN
                | ivm::syscalls::SYSCALL_ASSET_LOCK_CANCEL
                | ivm::syscalls::SYSCALL_ASSET_LOCK_EXPIRE => {
                    self.controlled_asset_lock(vm, number)
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
                    let amount = Self::decode_quantity(vm, amount_ptr)?;
                    let evidence_hashes = Self::decode_optional_evidence_hashes(vm, evidence_ptr)?;
                    let instr = InstructionBox::from(OpenAssetEscrow {
                        escrow_id,
                        asset_definition,
                        amount,
                        evidence_hashes,
                    });
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_ESCROW_ACCEPT => {
                    let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                    Ok(self
                        .queue_instruction(InstructionBox::from(AcceptAssetEscrow { escrow_id }))?)
                }
                ivm::syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT => {
                    let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                    Ok(
                        self.queue_instruction(InstructionBox::from(MarkEscrowPaymentSent {
                            escrow_id,
                        }))?,
                    )
                }
                ivm::syscalls::SYSCALL_ESCROW_RELEASE => {
                    let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                    Ok(
                        self.queue_instruction(InstructionBox::from(ReleaseAssetEscrow {
                            escrow_id,
                        }))?,
                    )
                }
                ivm::syscalls::SYSCALL_ESCROW_CANCEL => {
                    let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                    Ok(self
                        .queue_instruction(InstructionBox::from(CancelAssetEscrow { escrow_id }))?)
                }
                ivm::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE => {
                    let escrow_id = Self::decode_escrow_id(vm, vm.register(10))?;
                    let evidence_hashes =
                        Self::decode_optional_evidence_hashes(vm, vm.register(11))?;
                    Ok(
                        self.queue_instruction(InstructionBox::from(OpenEscrowDispute {
                            escrow_id,
                            evidence_hashes,
                        }))?,
                    )
                }
                ivm::syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE => {
                    let escrow_ptr = vm.register(10);
                    let buyer_amount_ptr = vm.register(11);
                    let seller_amount_ptr = vm.register(12);
                    let evidence_ptr = vm.register(13);
                    let escrow_id = Self::decode_escrow_id(vm, escrow_ptr)?;
                    let buyer_amount = Self::decode_quantity(vm, buyer_amount_ptr)?;
                    let seller_amount = Self::decode_quantity(vm, seller_amount_ptr)?;
                    let evidence_hashes = Self::decode_optional_evidence_hashes(vm, evidence_ptr)?;
                    Ok(
                        self.queue_instruction(InstructionBox::from(ResolveEscrowDispute {
                            escrow_id,
                            buyer_amount,
                            seller_amount,
                            evidence_hashes,
                        }))?,
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
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_DELETE_ROLE => {
                    let name_ptr = vm.register(10);
                    let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                    let isi = Unregister::role(RoleId::new(name));
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_GRANT_ROLE => {
                    let account_ptr = vm.register(10);
                    let name_ptr = vm.register(11);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                    let isi = Grant::account_role(RoleId::new(name), account);
                    let instr = InstructionBox::from(isi);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REVOKE_ROLE => {
                    let account_ptr = vm.register(10);
                    let name_ptr = vm.register(11);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let name: Name = Self::decode_tlv_typed(vm, name_ptr, PointerType::Name)?;
                    let isi = Revoke::account_role(RoleId::new(name), account);
                    let instr = InstructionBox::from(isi);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_GRANT_PERMISSION => {
                    let account_ptr = vm.register(10);
                    let perm_ptr = vm.register(11);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let permission = Self::decode_permission(vm, perm_ptr)?;
                    let isi = Grant::account_permission(permission, account);
                    let instr = InstructionBox::from(isi);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REVOKE_PERMISSION => {
                    let account_ptr = vm.register(10);
                    let perm_ptr = vm.register(11);
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, account_ptr, PointerType::AccountId)?;
                    let permission = Self::decode_permission(vm, perm_ptr)?;
                    let isi = Revoke::account_permission(permission, account);
                    let instr = InstructionBox::from(isi);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_GRANT_CONTRACT_PERMISSION
                | ivm::syscalls::SYSCALL_REVOKE_CONTRACT_PERMISSION => {
                    let account: AccountId =
                        Self::decode_tlv_typed(vm, vm.register(10), PointerType::AccountId)?;
                    let declaration_name: iroha_model_base::name::Name =
                        Self::decode_tlv_typed(vm, vm.register(11), PointerType::Name)?;
                    let interface = vm
                        .contract_interface()
                        .ok_or(ivm::VMError::PermissionDenied)?;
                    if !interface.permissions.iter().any(|declaration| {
                        declaration.name == declaration_name
                            && matches!(declaration.scope, ContractPermissionScopeV1::Instance)
                    }) {
                        return Err(ivm::VMError::PermissionDenied);
                    }
                    let contract = self
                        .current_contract_runtime_context
                        .as_ref()
                        .map(|context| context.contract_address.clone())
                        .ok_or(ivm::VMError::PermissionDenied)?;
                    let permission = Permission::from(CanUseContractPermission {
                        contract,
                        permission: declaration_name,
                    });
                    let instruction = if number == ivm::syscalls::SYSCALL_GRANT_CONTRACT_PERMISSION
                    {
                        InstructionBox::from(Grant::account_permission(permission, account))
                    } else {
                        InstructionBox::from(Revoke::account_permission(permission, account))
                    };
                    Ok(self.queue_instruction(instruction)?)
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
                        let id: TriggerId =
                            id_str.parse().map_err(|_| ivm::VMError::DecodeError)?;
                        let action_value = map.remove("action").ok_or(ivm::VMError::DecodeError)?;
                        let mut action = Self::decode_trigger_action_spec(action_value)?;
                        if action.authority.subject_id() == self.authority.subject_id() {
                            action.authority = self.authority.clone();
                        }
                        Trigger::new(id, action)
                    };
                    // A contract-registered trigger is an opaque deferred executable: it may
                    // not carry the signed-only outbound SCCP record (specs/sccp.md §4.4).
                    // Applying the queue re-checks every effect, including multisig approvals.
                    if crate::deferred_authority::trigger_executable_derives_sccp_outbound_record(
                        trigger.action().executable(),
                    ) {
                        return Err(ivm::VMError::PermissionDenied);
                    }
                    let instr = InstructionBox::from(Register::trigger(trigger));
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REMOVE_TRIGGER => {
                    let ptr = vm.register(10);
                    let name: Name = Self::decode_tlv_typed(vm, ptr, PointerType::Name)?;
                    let id = TriggerId::new(name);
                    let isi = Unregister::trigger(id);
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE => {
                    let ptr = vm.register(10);
                    // Decode manifest registration request from Norito-encoded TLV bytes.
                    let tlv = vm.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::NoritoBytes {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    let request: scode::RegisterSmartContractCode =
                        decode_canonical_norito(tlv.payload)
                            .map_err(|_| ivm::VMError::DecodeError)?;
                    let instr = InstructionBox::from(request);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES => {
                    let ptr = vm.register(10);
                    let request: scode::RegisterSmartContractBytes =
                        Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                    let instr = InstructionBox::from(request);
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE => {
                    let ptr = vm.register(10);
                    let request: scode::ActivateContractInstance =
                        Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                    let instr = InstructionBox::from(request);
                    self.ensure_instruction_queue_allowed(&instr)?;
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE => {
                    let ptr = vm.register(10);
                    let request: scode::DeactivateContractInstance =
                        Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                    let instr = InstructionBox::from(request);
                    self.ensure_instruction_queue_allowed(&instr)?;
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES => {
                    let ptr = vm.register(10);
                    let request: scode::RemoveSmartContractBytes =
                        Self::decode_tlv_typed(vm, ptr, PointerType::NoritoBytes)?;
                    let instr = InstructionBox::from(request);
                    Ok(self.queue_instruction(instr)?)
                }
                // ----------------- NFT (Non-fungible) ISIs -----------------
                ivm::syscalls::SYSCALL_NFT_MINT_ASSET => {
                    let nft_id_ptr = vm.register(10);
                    let owner_ptr = vm.register(11);
                    let owner: AccountId =
                        Self::decode_tlv_typed(vm, owner_ptr, PointerType::AccountId)?;
                    let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                    let nft = Nft::new(nft_id.clone(), Metadata::default());
                    let issuer = self.effect_authority();
                    let mut gas = self.queue_instruction(InstructionBox::from(
                        RegisterBox::from(Register::nft(nft)),
                    ))?;
                    if owner != issuer {
                        let transfer = InstructionBox::from(TransferBox::from(Transfer::nft(
                            issuer, nft_id, owner,
                        )));
                        gas = gas.saturating_add(self.queue_instruction(transfer)?);
                    }
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                    let from_ptr = vm.register(10);
                    let nft_id_ptr = vm.register(11);
                    let to_ptr = vm.register(12);
                    let from: AccountId =
                        Self::decode_tlv_typed(vm, from_ptr, PointerType::AccountId)?;
                    let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                    let to: AccountId = Self::decode_tlv_typed(vm, to_ptr, PointerType::AccountId)?;
                    let isi = Transfer::nft(from, nft_id, to);
                    let instr = InstructionBox::from(TransferBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
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
                    Ok(self.queue_instruction(instr)?)
                }
                ivm::syscalls::SYSCALL_NFT_BURN_ASSET => {
                    let nft_id_ptr = vm.register(10);
                    let nft_id: NftId = Self::decode_tlv_typed(vm, nft_id_ptr, PointerType::NftId)?;
                    let isi = Unregister::nft(nft_id);
                    let instr = InstructionBox::from(UnregisterBox::from(isi));
                    Ok(self.queue_instruction(instr)?)
                }
                // Sample convenience: create one NFT per known account (from snapshot)
                ivm::syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS => {
                    let (instructions, next_sequence) = self.create_nfts_for_all_preview();
                    let exact_gas = self.create_nfts_for_all_instructions_gas(&instructions)?;
                    ivm::host::preflight_reserved_syscall_gas(vm, exact_gas)?;
                    for instruction in instructions {
                        self.enqueue_priced_instruction(instruction);
                    }
                    self.nft_seq = next_sequence;
                    Ok(exact_gas)
                }
                // Set SmartContract execution depth parameter to x10
                ivm::syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH => {
                    let depth_raw = vm.register(10);
                    // If zero, treat as no-op to avoid setting an invalid value
                    if depth_raw == 0 {
                        let gas = Self::smartcontract_depth_gas();
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        return Ok(gas);
                    }
                    // SmartContractParameter::ExecutionDepth expects a u8.
                    let depth: u8 = u8::try_from(depth_raw).unwrap_or(u8::MAX);
                    let param = Parameter::SmartContract(
                        iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(depth),
                    );
                    let instr = InstructionBox::from(SetParameter::new(param));
                    let gas = crate::gas::meter_instruction(&instr, &self.confidential_gas)
                        .map_err(ivm::VMError::VerificationDeferred)?
                        .max(Self::smartcontract_depth_gas());
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    self.enqueue_priced_instruction(instr);
                    Ok(gas)
                }
                // Accept one of the operation-tagged instruction bridges used by Kotodama.
                ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION => {
                    let operation_tag = vm.register(11);
                    let ib = Self::decode_opaque_instruction(vm)?;
                    self.ensure_instruction_queue_allowed(&ib)?;
                    let any_ref = ib.as_any();
                    match operation_tag {
                        ivm::syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT => {
                            let sb = any_ref
                                .downcast_ref::<DMZk::SubmitBallot>()
                                .ok_or(ivm::VMError::PermissionDenied)?;
                            let mut proof = sb.ballot_proof.clone();
                            let pending_hash = if proof.envelope_hash.is_none() {
                                self.zk_last_env_hash_ballot.front().copied()
                            } else {
                                None
                            };
                            if let Some(hash) = pending_hash {
                                proof.envelope_hash = Some(hash);
                            }
                            let instruction = InstructionBox::from(DMZk::SubmitBallot {
                                election_id: sb.election_id.clone(),
                                ciphertext: sb.ciphertext.clone(),
                                ballot_proof: proof,
                                nullifier: sb.nullifier,
                            });
                            let gas =
                                crate::gas::meter_instruction(&instruction, &self.confidential_gas)
                                    .map_err(ivm::VMError::VerificationDeferred)?;
                            ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                            if pending_hash.is_some() {
                                let consumed_hash =
                                    Arc::make_mut(&mut self.zk_last_env_hash_ballot).pop_front();
                                debug_assert_eq!(consumed_hash, pending_hash);
                            }
                            self.enqueue_priced_instruction(instruction);
                            Ok(gas)
                        }
                        // ABI v1 defines only `1=SubmitBallot`. Contracts record no SCCP
                        // messages (specs/sccp.md §4.4): an encoded `RecordSccpMessage` fails
                        // under every tag, like every other instruction type.
                        _ => Err(ivm::VMError::PermissionDenied),
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
                    let budget = Self::query_execution_budget(&gas_ctx, gas_remaining)?;
                    if matches!(request, QueryRequest::Singular(_)) && budget.max_items() == 0 {
                        return Err(ivm::VMError::OutOfGas);
                    }
                    let query_result = state_ref.execute_query_with_budget(
                        &self.authority,
                        request,
                        Some(budget),
                    )?;
                    let response = query_result.response;
                    let processed_items = query_result.processed_items;
                    let gas = Self::query_gas_cost(
                        &gas_ctx,
                        processed_items,
                        query_result.processed_bytes,
                    );
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    if gas > gas_remaining {
                        return Err(ivm::VMError::OutOfGas);
                    }
                    let response_bytes = Self::encode_norito_payload(&response)
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    let payload_len = Self::len_to_u32(response_bytes.len())?;
                    let mut out = Vec::with_capacity(7 + response_bytes.len() + Hash::LENGTH);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&payload_len.to_be_bytes());
                    out.extend_from_slice(&response_bytes);
                    let h: [u8; Hash::LENGTH] = Hash::new(&response_bytes).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_SUBSCRIPTION_BILL => {
                    self.stage_queued_syscall(vm, Self::subscription_bill)
                }
                ivm::syscalls::SYSCALL_SUBSCRIPTION_RECORD_USAGE => {
                    self.stage_queued_syscall(vm, Self::subscription_record_usage)
                }
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
                    if Self::query_execution_budget(&gas_ctx, gas_remaining)?.max_items() == 0 {
                        return Err(ivm::VMError::OutOfGas);
                    }
                    let account_id =
                        state_ref.resolve_account_alias(&self.authority, &alias_literal)?;
                    let payload = Self::encode_norito_payload(&account_id)?;
                    let payload_len_u64 = u64::try_from(payload.len()).unwrap_or(u64::MAX);
                    let gas = Self::query_gas_cost(&gas_ctx, 1, payload_len_u64);
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
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
                    let p = vm.alloc_host_tlv(&out)?;
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
                    self.execute_singular_query_payload_as(
                        vm,
                        number,
                        PointerType::Quantity,
                        request,
                        |output| match output {
                            SingularQueryOutputBox::Asset(asset) => {
                                Self::encode_amount_payload(asset.value())
                            }
                            _ => Err(ivm::VMError::DecodeError),
                        },
                    )
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
                        let payload = Self::encode_norito_payload(args)?;
                        let payload_len = Self::len_to_u32(payload.len())?;
                        let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&payload_len.to_be_bytes());
                        out.extend_from_slice(&payload);
                        let h: [u8; Hash::LENGTH] = Hash::new(&payload).into();
                        out.extend_from_slice(&h);
                        let bytes = u64::try_from(out.len()).unwrap_or(u64::MAX);
                        let gas = PUBLIC_INPUT_GAS_BASE_DEFAULT.saturating_add(
                            PUBLIC_INPUT_GAS_PER_BYTE_DEFAULT.saturating_mul(bytes),
                        );
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        let ptr = vm.alloc_host_tlv(&out)?;
                        vm.set_register(10, ptr);
                        return Ok(gas);
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
                    let gas = record.gas_cost();
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    let ptr = vm.alloc_host_tlv(&record.tlv)?;
                    vm.set_register(10, ptr);
                    Ok(gas)
                }
                // ZK verify syscalls are handled here so CoreHost can enforce VK binding
                // and run full backend verification before arming ISI latches.
                ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => {
                    let ptr = vm.register(10);
                    let tlv = vm.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::NoritoBytes {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    let (envelope, gas) = self.decode_metered_zk_envelope(vm, tlv.payload)?;
                    let envelope = match envelope {
                        Ok(envelope) => envelope,
                        Err(code) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, code);
                            return Ok(gas);
                        }
                    };
                    let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                    let ok = match self.verify_bound_envelope(envelope, tlv.payload, "ballot") {
                        Ok(ok) => ok,
                        Err(crate::zk::VerificationAttemptError::Deferred(original)) => {
                            return Err(ivm::VMError::VerificationDeferred(original));
                        }
                        Err(crate::zk::VerificationAttemptError::Rejected(code)) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, code);
                            return Ok(gas);
                        }
                    };
                    vm.set_register(10, u64::from(ok));
                    vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                    if ok {
                        Arc::make_mut(&mut self.zk_verified_ballot).push_back(env_hash);
                        Arc::make_mut(&mut self.zk_last_env_hash_ballot).push_back(env_hash);
                    }
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                    let ptr = vm.register(10);
                    let tlv = vm.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::NoritoBytes {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    let (envelope, gas) = self.decode_metered_zk_envelope(vm, tlv.payload)?;
                    let envelope = match envelope {
                        Ok(envelope) => envelope,
                        Err(code) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, code);
                            return Ok(gas);
                        }
                    };
                    let env_hash: [u8; 32] = Hash::new(tlv.payload).into();
                    let ok = match self.verify_bound_envelope(envelope, tlv.payload, "tally") {
                        Ok(ok) => ok,
                        Err(crate::zk::VerificationAttemptError::Deferred(original)) => {
                            return Err(ivm::VMError::VerificationDeferred(original));
                        }
                        Err(crate::zk::VerificationAttemptError::Rejected(code)) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, code);
                            return Ok(gas);
                        }
                    };
                    vm.set_register(10, u64::from(ok));
                    vm.set_register(11, if ok { 0 } else { ivm::host::ERR_VERIFY });
                    if ok {
                        Arc::make_mut(&mut self.zk_verified_tally).push_back(env_hash);
                        Arc::make_mut(&mut self.zk_last_env_hash_tally).push_back(env_hash);
                    }
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_VERIFY_PROOF => {
                    let ptr = vm.register(10);
                    let tlv = vm.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::NoritoBytes {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    let (envelope, gas) = self.decode_metered_zk_envelope(vm, tlv.payload)?;
                    let envelope = match envelope {
                        Ok(envelope) => envelope,
                        Err(code) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, code);
                            return Ok(gas);
                        }
                    };
                    let ok = match self.verify_any_namespace_envelope(envelope, tlv.payload) {
                        Ok(ok) => ok,
                        Err(crate::zk::VerificationAttemptError::Deferred(original)) => {
                            return Err(ivm::VMError::VerificationDeferred(original));
                        }
                        Err(crate::zk::VerificationAttemptError::Rejected(code)) => {
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
                    let tlv = vm.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::NoritoBytes {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    if u64::try_from(tlv.payload.len()).unwrap_or(u64::MAX)
                        > self.zk_gas_schedule.max_payload_bytes
                    {
                        return Err(ivm::VMError::NoritoInvalid);
                    }
                    let quote = ivm::host::quote_zk_batch_at(vm, ptr, self.zk_gas_schedule)?;
                    ivm::host::preflight_reserved_syscall_gas(vm, quote.gas)?;
                    let max_items = usize::try_from(self.zk_verify_limits.max_verify_batch)
                        .unwrap_or(usize::MAX)
                        .min(
                            usize::try_from(self.zk_gas_schedule.max_batch_proofs)
                                .unwrap_or(usize::MAX),
                        );
                    let envs = match ivm::host::decode_canonical_zk_batch(tlv.payload, max_items) {
                        Ok(envs) => envs,
                        Err(error) => {
                            let status = error.into_status()?;
                            vm.set_register(10, 0);
                            vm.set_register(11, status);
                            vm.set_register(12, u64::MAX);
                            return Ok(quote.gas);
                        }
                    };
                    let public_input_count = envs.iter().fold(0_u64, |total, env| {
                        total.saturating_add(
                            self.zk_gas_schedule
                                .public_input_count(env.public_inputs.len()),
                        )
                    });
                    let gas = self.zk_gas_schedule.actual_batch_gas(
                        envs.len(),
                        tlv.payload.len(),
                        public_input_count,
                    );
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    let guardrails = self.zk_verify_guardrails();
                    let mut statuses: Vec<u8> = Vec::with_capacity(envs.len());
                    let mut first_error: Option<u64> = None;
                    for env in envs {
                        let mut status = 0u8;
                        let payload = if let Ok(bytes) = Self::encode_norito_payload(&env) {
                            bytes
                        } else {
                            first_error.get_or_insert(ivm::host::ERR_DECODE);
                            statuses.push(status);
                            continue;
                        };
                        let (_env, prepared) = match self
                            .enforce_zk_envelope_any_namespace_value(env, payload.len())
                        {
                            Ok(v) => v,
                            Err(code) => {
                                first_error.get_or_insert(code);
                                statuses.push(status);
                                continue;
                            }
                        };
                        let Some(vk_box) = prepared.record.key.as_ref() else {
                            first_error.get_or_insert(ivm::host::ERR_VK_MISSING);
                            statuses.push(status);
                            continue;
                        };
                        let backend_label = prepared.backend_label.as_ref();
                        let proof = ProofBox::new(backend_label.into(), payload);
                        let report = crate::zk::verify_backend_with_timing_guardrails(
                            backend_label,
                            &proof,
                            Some(vk_box),
                            guardrails,
                        )
                        .map_err(ivm::VMError::VerificationDeferred)?;
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
                    let body = Self::encode_norito_payload(&statuses)
                        .map_err(|_| ivm::VMError::metered(gas, ivm::VMError::NoritoInvalid))?;
                    let output_bytes = 7_usize
                        .saturating_add(body.len())
                        .saturating_add(iroha_crypto::Hash::LENGTH);
                    if u64::try_from(output_bytes).unwrap_or(u64::MAX)
                        != self.zk_gas_schedule.batch_output_bytes(statuses.len())
                    {
                        return Err(ivm::VMError::metered(gas, ivm::VMError::NoritoInvalid));
                    }
                    let mut out = Vec::with_capacity(output_bytes);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(
                        &u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes(),
                    );
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
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
                        decode_canonical_norito(req_tlv.payload)
                            .map_err(|_| ivm::VMError::NoritoInvalid)?;
                    let resp = self.roots_get_response(&req)?;
                    let body = Self::encode_norito_payload(&resp)?;
                    let gas = Self::state_query_gas(input_len.saturating_add(body.len()));
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    let body_len = Self::len_to_u32(body.len())?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&body_len.to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(gas)
                }
                // ZK vote tally read: respond from elections snapshot
                ivm::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                    let ptr = vm.register(10);
                    let req_tlv = Self::expect_tlv(vm, ptr, PointerType::NoritoBytes)?;
                    let input_len = req_tlv.payload.len();
                    let req: ivm::zk_verify::VoteGetTallyRequest =
                        decode_canonical_norito(req_tlv.payload)
                            .map_err(|_| ivm::VMError::NoritoInvalid)?;
                    let resp = self.vote_get_tally_response(&req)?;
                    let body = Self::encode_norito_payload(&resp)?;
                    let gas = Self::state_query_gas(input_len.saturating_add(body.len()));
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    let body_len = Self::len_to_u32(body.len())?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&body_len.to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_VRF_EPOCH_SEED => {
                    let epoch = vm.register(10);
                    ivm::vrf::publish_epoch_seed(vm, self.vrf_epoch_seed(epoch).as_ref())
                }
                ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE
                | ivm::syscalls::SYSCALL_SORACLOUD_EMIT_STATE_MUTATION
                | ivm::syscalls::SYSCALL_SORACLOUD_EMIT_MAILBOX_MESSAGE
                | ivm::syscalls::SYSCALL_SORACLOUD_APPEND_JOURNAL
                | ivm::syscalls::SYSCALL_SORACLOUD_PUBLISH_CHECKPOINT
                | ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG
                | ivm::syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE => {
                    Self::reject_soracloud_syscall(vm, number)
                }
                // Mandatory V1 SM helpers share the portable DefaultHost implementation.
                ivm::syscalls::SYSCALL_SM3_HASH
                | ivm::syscalls::SYSCALL_SM2_VERIFY
                | ivm::syscalls::SYSCALL_SM4_GCM_SEAL
                | ivm::syscalls::SYSCALL_SM4_GCM_OPEN
                | ivm::syscalls::SYSCALL_SM4_CCM_SEAL
                | ivm::syscalls::SYSCALL_SM4_CCM_OPEN => {
                    let result = self.default.syscall(number, vm);
                    #[cfg(feature = "telemetry")]
                    if let Some(telemetry) = self.telemetry.as_ref() {
                        Self::record_sm_syscall(telemetry, number, &result);
                    }
                    result
                }
                // Durable state syscalls backed by WSV.
                ivm::syscalls::SYSCALL_STATE_GET => {
                    let (path, path_len) = self.decode_durable_state_path(vm, vm.register(10))?;
                    Self::ensure_contract_state_read_allowed(&path)?;
                    let key = self
                        .scoped_durable_state_path(&path)?
                        .unwrap_or_else(|| path.clone());
                    if let Some(entry) = self.durable_state_overlay.get(&key) {
                        if let Some(stored) = entry {
                            let len = Self::durable_state_value_payload_len(stored)?;
                            let gas = ivm::host::state_value_gas(path_len, len);
                            ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                            ivm::host::validate_declared_state_value_payload(vm, &path, stored)?;
                            let stored = stored.clone();
                            self.log_state_read_key(path.as_ref());
                            Self::load_state_value(vm, &stored)?;
                            return Ok(gas);
                        }
                        let gas = ivm::host::state_path_gas(path_len);
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        self.log_state_read_key(path.as_ref());
                        vm.set_register(10, 0);
                        return Ok(gas);
                    }
                    if let Some(len) = self.durable_state_base_or_live_value_len(&key)? {
                        let gas = ivm::host::state_value_gas(path_len, len);
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        let stored = self
                            .durable_state_base_or_live_get(&key)
                            .ok_or(ivm::VMError::DecodeError)?;
                        ivm::host::validate_declared_state_value_payload(
                            vm,
                            &path,
                            stored.as_ref(),
                        )?;
                        let stored = stored.into_owned();
                        self.log_state_read_key(path.as_ref());
                        Self::load_state_value(vm, &stored)?;
                        Ok(gas)
                    } else {
                        let gas = ivm::host::state_path_gas(path_len);
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        self.log_state_read_key(path.as_ref());
                        vm.set_register(10, 0);
                        Ok(gas)
                    }
                }
                ivm::syscalls::SYSCALL_STATE_SET => {
                    let path_ptr = vm.register(10);
                    let val_ptr = vm.register(11);
                    let (path, path_len) = self.decode_durable_state_path_payload(vm, path_ptr)?;
                    Self::ensure_contract_state_write_allowed(&path)?;
                    let key = self
                        .scoped_durable_state_path(&path)?
                        .unwrap_or_else(|| path.clone());
                    validate_reserve_durable_state_path(&key)
                        .map_err(|_| ivm::VMError::PermissionDenied)?;
                    if crate::validation_fee::is_consensus_fee_state_key(&key) {
                        return Err(ivm::VMError::PermissionDenied);
                    }
                    ivm::host::validate_declared_state_path(vm, &path)?;
                    let val_tlv = Self::expect_tlv(vm, val_ptr, PointerType::NoritoBytes)?;
                    ivm::host::validate_state_value_payload_len(val_tlv.payload.len())?;
                    ivm::host::validate_declared_state_value_payload(vm, &path, val_tlv.payload)?;
                    let gas = ivm::host::state_value_gas(path_len, val_tlv.payload.len());
                    if !self.try_reserve_durable_state_update(&key, val_tlv.payload.len()) {
                        return Ok(gas);
                    }
                    let stored = val_tlv.payload.to_vec();
                    self.stage_durable_state_update(key, Some(stored));
                    self.log_state_write_key(path.as_ref());
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_STATE_DEL => {
                    let (path, path_len) =
                        self.decode_durable_state_path_payload(vm, vm.register(10))?;
                    Self::ensure_contract_state_write_allowed(&path)?;
                    let scoped_path = self.scoped_durable_state_path(&path)?;
                    let key = scoped_path.unwrap_or_else(|| path.clone());
                    let effective_path = &key;
                    validate_reserve_durable_state_path(effective_path)
                        .map_err(|_| ivm::VMError::PermissionDenied)?;
                    if crate::validation_fee::is_consensus_fee_state_key(effective_path) {
                        return Err(ivm::VMError::PermissionDenied);
                    }
                    ivm::host::validate_declared_state_path(vm, &path)?;
                    let gas = ivm::host::state_path_gas(path_len);
                    if !self.try_reserve_durable_state_update(&key, 0) {
                        return Ok(gas);
                    }
                    self.stage_durable_state_update(key, None);
                    self.log_state_write_key(path.as_ref());
                    Ok(gas)
                }
                ivm::syscalls::SYSCALL_STATE_SCAN => self.scan_durable_state_page(vm),
                ivm::syscalls::SYSCALL_STATE_HAS => {
                    let (path, path_len) = self.decode_durable_state_path(vm, vm.register(10))?;
                    Self::ensure_contract_state_read_allowed(&path)?;
                    let present = self.durable_state_key_present(&path)?;
                    self.log_state_read_key(path.as_ref());
                    vm.set_register(10, u64::from(present));
                    Ok(ivm::host::state_path_gas(path_len))
                }
                ivm::syscalls::SYSCALL_STATE_LEN => {
                    let (path, path_len) = self.decode_durable_state_path(vm, vm.register(10))?;
                    Self::ensure_contract_state_read_allowed(&path)?;
                    let gas = ivm::host::state_path_gas(path_len);
                    if let Some(len) = self.durable_state_value_len(&path)? {
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        self.log_state_read_key(path.as_ref());
                        vm.set_register(10, u64::try_from(len).unwrap_or(u64::MAX));
                        vm.set_register(11, 1);
                        Ok(gas)
                    } else {
                        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                        self.log_state_read_key(path.as_ref());
                        vm.set_register(10, 0);
                        vm.set_register(11, 0);
                        Ok(gas)
                    }
                }
                ivm::syscalls::SYSCALL_STATE_COUNT => {
                    let (prefix, path_len) =
                        self.decode_durable_state_scan_path(vm, vm.register(10))?;
                    let (total, scan_work_gas) =
                        self.count_durable_state_keys(vm, &prefix, path_len)?;
                    let gas = ivm::host::STATE_QUERY_GAS_BASE.saturating_add(scan_work_gas);
                    ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
                    self.log_state_read_key(prefix.as_ref());
                    vm.set_register(10, total);
                    Ok(gas)
                }
                // Norito serialization and numeric helpers delegate to the IVM codec host.
                forwarded if Self::is_codec_forwarded_syscall(forwarded) => {
                    self.codec_host.syscall(forwarded, vm)
                }
                // Stateless helpers are safe to forward to DefaultHost.
                forwarded if Self::is_default_forwarded_syscall(forwarded) => {
                    self.default.syscall(forwarded, vm)
                }
                // Provide the current authority as a host-owned AccountId pointer.
                // New format: TLV (type_id:u16, version:u8, len:be u32, payload, hash:32).
                // Historical len-prefixed payloads are no longer emitted.
                ivm::syscalls::SYSCALL_GET_AUTHORITY | ivm::syscalls::SYSCALL_SYSVAR_AUTHORITY => {
                    // Encode authority payload
                    let payload = Self::encode_norito_payload(&self.authority)?;
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
                    // Prefer INPUT and spill to owned HEAP without overwriting earlier TLVs.
                    let ptr = vm.alloc_host_tlv(&blob)?;
                    vm.set_register(10, ptr);
                    Ok(Self::sysvar_gas(payload.len()))
                }
                ivm::syscalls::SYSCALL_SYSVAR_CONTRACT_SUBJECT => {
                    let subject = self
                        .current_contract_runtime_context
                        .as_ref()
                        .map(|context| &context.contract_subject)
                        .ok_or(ivm::VMError::PermissionDenied)?;
                    let payload = Self::encode_norito_payload(subject)?;
                    let ptr = Self::alloc_tlv_payload(vm, PointerType::AccountId, &payload)?;
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
                        let ptr =
                            Self::alloc_tlv_payload(vm, PointerType::Blob, &self.chain_id_bytes)?;
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
                    let payload = context.contract_address.as_str().as_bytes();
                    let ptr = Self::alloc_tlv_payload(vm, PointerType::Blob, payload)?;
                    vm.set_register(10, ptr);
                    Ok(Self::sysvar_gas(payload.len()))
                }
                ivm::syscalls::SYSCALL_SYSVAR_ENTRYPOINT => {
                    let Some(context) = self.current_contract_runtime_context.as_ref() else {
                        vm.set_register(10, 0);
                        return Ok(Self::sysvar_gas(0));
                    };
                    let ptr = Self::alloc_tlv_payload(
                        vm,
                        PointerType::Blob,
                        context.entrypoint.as_bytes(),
                    )?;
                    vm.set_register(10, ptr);
                    Ok(Self::sysvar_gas(context.entrypoint.len()))
                }
                ivm::syscalls::SYSCALL_CORE_QUERY_GET => {
                    let tag = CoreQueryEntityTagV1::try_from(vm.register(10))
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    self.execute_core_query_get(vm, number, tag)
                }
                ivm::syscalls::SYSCALL_CORE_QUERY_PAGE => {
                    let tag = CoreQueryEntityTagV1::try_from(vm.register(10))
                        .map_err(|_| ivm::VMError::DecodeError)?;
                    let offset = i64::from_ne_bytes(vm.register(11).to_ne_bytes());
                    let limit = vm.register(12);
                    self.execute_core_query_page(vm, number, tag, offset, limit)
                }
                ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST => {
                    let artifact_id: iroha_data_model::smart_contract::ContractArtifactId =
                        Self::decode_query_key(vm, vm.register(10), PointerType::NoritoBytes)?;
                    let request =
                    QueryRequest::Singular(SingularQueryBox::FindContractManifestByArtifactId(
                        iroha_data_model::query::smart_contract::prelude::FindContractManifestByArtifactId {
                            artifact_id,
                        },
                    ));
                    self.execute_singular_query_payload(
                        vm,
                        number,
                        request,
                        |output| match output {
                            SingularQueryOutputBox::ContractManifest(manifest) => {
                                Self::encode_norito_payload(&manifest)
                            }
                            _ => Err(ivm::VMError::DecodeError),
                        },
                    )
                }
                ivm::syscalls::SYSCALL_QUERY_GET_PARAMETER => {
                    let name: Name =
                        Self::decode_query_key(vm, vm.register(10), PointerType::Name)?;
                    let gas_remaining = vm.remaining_gas();
                    let gas_ctx = QueryGasContext::singular();
                    let Some(state_ref) = self.query_state.get() else {
                        return Err(ivm::VMError::NotImplemented { syscall: number });
                    };
                    if Self::query_execution_budget(&gas_ctx, gas_remaining)?.max_items() == 0 {
                        return Err(ivm::VMError::OutOfGas);
                    }
                    let parameter = state_ref.parameter_by_name(&name)?;
                    let payload = Self::encode_norito_payload(&parameter)?;
                    Self::finish_direct_singular_query_payload(vm, gas_remaining, &(payload))
                }
                ivm::syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE => {
                    let lookup = Self::decode_contract_instance_lookup(vm, vm.register(10))?;
                    let gas_remaining = vm.remaining_gas();
                    let gas_ctx = QueryGasContext::singular();
                    let Some(state_ref) = self.query_state.get() else {
                        return Err(ivm::VMError::NotImplemented { syscall: number });
                    };
                    if Self::query_execution_budget(&gas_ctx, gas_remaining)?.max_items() == 0 {
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
                    Self::finish_direct_singular_query_payload(vm, gas_remaining, &(payload))
                }
                ivm::syscalls::SYSCALL_EMIT_CONTRACT_EVENT => self.emit_contract_event(vm),
                ivm::syscalls::SYSCALL_CALL_CONTRACT => self.handle_call_contract(vm),
                ivm::syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
                ivm::syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
                ivm::syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(vm),
                ivm::syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
                ivm::syscalls::SYSCALL_AXT_STAGE_ANCHORED_SPEND => {
                    self.handle_axt_stage_anchored_spend(vm)
                }
                // All other allowed stateful operations must be routed via ISIs
                // before this host can execute them directly.
                _ => Err(ivm::VMError::metered_not_implemented(
                    ivm::gas::G_CONTRACT_ADMIN,
                    number,
                )),
            }
        })();
        self.ensure_output_budget()?;
        result
    }
    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }
    fn begin_tx(&mut self, declared: &ivm::parallel::StateAccessSet) -> Result<(), ivm::VMError> {
        if !self.nested_contract_call_journals.is_empty() {
            return Err(ivm::VMError::DecodeError);
        }
        self.rollback_attachment_root()?;
        self.durable_state_overlay.clear();
        self.durable_state_authorizations.clear();
        self.state_access_log = ivm::host::AccessLog {
            durable_read_paths_complete: true,
            ..ivm::host::AccessLog::default()
        };
        self.codec_host.begin_tx(declared)
    }
    fn finish_tx(&mut self) -> Result<ivm::host::AccessLog, ivm::VMError> {
        self.ensure_output_budget()?;
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
    use super::{
        tests::{
            begin_axt_envelope, bind_artifact_test_signed_root, contract_test_state,
            fixture_public_key_from_seed, grant_named_permission_to_account, install_contract,
            make_policy_snapshot, norito_blob, opaque_proof_blob_for, proof_blob_for,
            quantity_frame, store_quantity, store_tlv,
        },
        *,
    };
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::triggers::specialized::SpecializedAction,
        state::{State, World},
    };
    use core::{num::NonZeroU16, str::FromStr};
    use iroha_crypto::{Algorithm, Hash as IrohaHash, KeyPair, PublicKey};
    use iroha_data_model::{
        events::execute_trigger::ExecuteTriggerEventFilter,
        smart_contract::manifest::ContractManifest,
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use ivm::syscalls as ivm_sys;
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
    pub(super) fn establish_authenticated_axt_ledger_time(state: &State, slot: u64) {
        let timestamp_ms = slot
            .checked_mul(state.nexus.read().axt.slot_length_ms.get())
            .expect("AXT fixture timestamp fits u64");
        let signer = KeyPair::from_seed(vec![0xA7; 32], Algorithm::Ed25519);
        let block = iroha_data_model::block::builder::BlockBuilder::new(BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("first height is nonzero"),
            None,
            None,
            timestamp_ms,
            0,
        ))
        .build_with_signature(0, signer.private_key());
        state
            .kura()
            .store_block(crate::block::reserve_block_for_tests().initialize(block.clone()))
            .expect("store authenticated ledger-time fixture block");
        state.append_committed_block_header_for_tests(block.header());
        assert_eq!(
            state.latest_block_creation_time_ms_fast(),
            Some(timestamp_ms),
            "fixture ledger time comes from the committed block header"
        );
    }
    fn store_state_path_tlv(vm: &mut IVM, path: &impl AsRef<str>) -> u64 {
        let path: StatePath = path.as_ref().parse().expect("valid durable-state path");
        store_tlv(vm, PointerType::NoritoBytes, &norito_blob(&path))
    }
    fn fixture_account(label: &str) -> AccountId {
        match label {
            "alice" => ALICE_ID.clone(),
            "bob" => BOB_ID.clone(),
            "carol" | "charlie" => {
                let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
                AccountId::new(fixture_public_key_from_seed(seed))
            }
            other => panic!("unsupported fixture account label: {other}"),
        }
    }
    fn local_contract_host(authority: AccountId) -> CoreHost {
        let mut host = CoreHost::new(authority);
        host.set_local_contract_debug_execution();
        host
    }
    #[test]
    fn call_contract_rejects_at_deterministic_nesting_depth_before_frame_growth() {
        let authority = ALICE_ID.clone();
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            42,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "outer".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash: Hash::new(b"depth-guard caller contract"),
            },
        );
        let mut host = CoreHost::new(authority);
        host.bind_contract_runtime_context(contract.subject_id(), authorization);
        host.nested_contract_call_depth = MAX_NESTED_CONTRACT_CALL_DEPTH;
        let mut vm = IVM::new(1_000);
        let result = host.handle_call_contract(&mut vm);
        let (gas, original) = result
            .expect_err("call depth must be enforced")
            .split_metered();
        assert_eq!(gas, Some(ivm::gas::G_CALL_CONTRACT));
        assert!(matches!(original, ivm::VMError::CallDepthExceeded));
        assert_eq!(
            host.nested_contract_call_depth, MAX_NESTED_CONTRACT_CALL_DEPTH,
            "rejection must not mutate the active-frame counter"
        );
    }
    #[test]
    fn authorized_deployed_binders_capture_exact_state_backed_root_authorization() {
        let authority = ALICE_ID.clone();
        let state = contract_test_state(&authority);
        let contract_address = install_contract(
            &state,
            &authority,
            r"
seiyaku StateBackedBinding { permission CanExecuteContract; permission CanInspectContract;
  kotoage fn execute() authorize(CanExecuteContract) -> int {
    return 7;
  }

  view fn inspect() authorize(CanInspectContract) -> int {
    return 8;
  }
}
",
            43,
        );
        grant_named_permission_to_account(
            &state,
            &authority,
            &(authority.clone()),
            "CanExecuteContract",
        );
        grant_named_permission_to_account(
            &state,
            &authority,
            &(authority.clone()),
            "CanInspectContract",
        );
        let view = state.view();
        let record =
            crate::smartcontracts::code::fetch_bound_contract_record(&view, &contract_address)
                .expect("registry read completes")
                .expect("installed contract record");
        let prepared = ivm::prepare_contract(Arc::<[u8]>::from(record.code_bytes.clone()))
            .expect("prepare installed contract");
        let mut host = CoreHost::new(authority.clone());
        host.bind_authorized_deployed_contract_runtime_context(
            &view,
            &contract_address,
            None,
            &prepared,
            "execute",
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("bind authorized transaction entrypoint");
        assert_eq!(host.execution_class, HostExecutionClass::Contract);
        let context = host
            .current_contract_runtime_context
            .as_ref()
            .expect("runtime context");
        assert_eq!(context.contract_address, contract_address);
        assert_eq!(context.contract_subject, record.contract_subject);
        assert_eq!(context.contract_alias, record.contract_alias);
        assert_eq!(context.entrypoint, "execute");
        let authorization = host
            .current_entrypoint_authorization
            .as_ref()
            .expect("root authorization");
        assert!(authorization.is_root());
        assert_eq!(authorization.authority, authority);
        assert_eq!(authorization.contract_address, contract_address);
        assert_eq!(authorization.contract_alias, record.contract_alias);
        assert_eq!(authorization.code_hash, record.code_hash);
        assert_eq!(authorization.entrypoint, "execute");
        assert_eq!(
            authorization.authorization,
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission(
                "CanExecuteContract".parse().unwrap()
            )
        );
        host.bind_authorized_deployed_contract_view_runtime_context(
            &view,
            &contract_address,
            record.contract_alias.as_ref(),
            &prepared,
            "inspect",
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("bind authorized view entrypoint");
        assert_eq!(host.execution_class, HostExecutionClass::View);
        let context = host
            .current_contract_runtime_context
            .as_ref()
            .expect("view runtime context");
        assert_eq!(context.contract_address, contract_address);
        assert_eq!(context.contract_subject, record.contract_subject);
        assert_eq!(context.contract_alias, record.contract_alias);
        assert_eq!(context.entrypoint, "inspect");
        let authorization = host
            .current_entrypoint_authorization
            .as_ref()
            .expect("view root authorization");
        assert!(authorization.is_root());
        assert_eq!(authorization.authority, authority);
        assert_eq!(authorization.contract_address, contract_address);
        assert_eq!(authorization.contract_alias, record.contract_alias);
        assert_eq!(authorization.code_hash, record.code_hash);
        assert_eq!(authorization.entrypoint, "inspect");
        assert_eq!(
            authorization.authorization,
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission(
                "CanInspectContract".parse().unwrap()
            )
        );
    }
    #[test]
    fn authorized_view_binding_rejects_effect_syscalls_before_input_decode() {
        let authority = ALICE_ID.clone();
        let state = contract_test_state(&authority);
        let contract_address = install_contract(
            &state,
            &authority,
            r"
seiyaku ReadOnlyBinding { permission CanInspectContract;
  state int missing;

  hajimari() {
    missing = 0;
  }

  view fn inspect() authorize(CanInspectContract) -> int {
    return 8;
  }
}
",
            46,
        );
        grant_named_permission_to_account(
            &state,
            &authority,
            &(authority.clone()),
            "CanInspectContract",
        );
        let view = state.view();
        let record =
            crate::smartcontracts::code::fetch_bound_contract_record(&view, &contract_address)
                .expect("registry read completes")
                .expect("installed contract record");
        let prepared = ivm::prepare_contract(Arc::<[u8]>::from(record.code_bytes.clone()))
            .expect("prepare installed contract");
        let mut host = CoreHost::new(authority);
        host.bind_authorized_deployed_contract_view_runtime_context(
            &view,
            &contract_address,
            record.contract_alias.as_ref(),
            &prepared,
            "inspect",
        )
        .map_err(crate::execution_attempt::expect_completed_rejection)
        .expect("bind authorized view entrypoint");
        let effect_syscalls = ivm_sys::abi_syscall_list()
            .iter()
            .copied()
            .filter(|syscall| {
                *syscall != ivm_sys::SYSCALL_CALL_CONTRACT
                    && matches!(
                        ivm_sys::syscall_access(*syscall),
                        ivm_sys::SyscallAccess::StateWrite
                            | ivm_sys::SyscallAccess::LedgerWrite
                            | ivm_sys::SyscallAccess::Dynamic
                    )
            })
            .collect::<Vec<_>>();
        assert!(
            effect_syscalls.len() > 4,
            "the ABI fixture must exercise the complete effect surface"
        );
        for syscall in effect_syscalls {
            let vm = IVM::new(1_000_000);
            assert!(
                matches!(
                    host.prepare_syscall(syscall, &vm),
                    Err(ivm::VMError::PermissionDenied)
                ),
                "view preparation must reject effect syscall 0x{syscall:02x} before pointer decoding"
            );
            let mut vm = IVM::new(1_000_000);
            assert!(
                matches!(
                    host.syscall(syscall, &mut vm),
                    Err(ivm::VMError::PermissionDenied)
                ),
                "direct view dispatch must reject effect syscall 0x{syscall:02x} before pointer decoding"
            );
        }
        let mut vm = IVM::new(1_000_000);
        vm.load_prepared(&prepared)
            .expect("load the authenticated view contract into the VM");
        let missing_path: StatePath = "missing".parse().expect("valid state path");
        let path_ptr = store_state_path_tlv(&mut vm, &missing_path);
        vm.set_register(10, path_ptr);
        host.prepare_syscall(ivm_sys::SYSCALL_STATE_GET, &vm)
            .expect("view preparation permits durable-state reads");
        host.syscall(ivm_sys::SYSCALL_STATE_GET, &mut vm)
            .expect("view dispatch permits durable-state reads");
        assert_eq!(vm.register(10), 0, "missing state returns a null pointer");
        assert!(host.queued.is_empty());
        assert!(host.durable_state_overlay.is_empty());
    }
    #[test]
    fn reused_host_resets_view_and_contract_provenance_before_generic_execution() {
        let authority = ALICE_ID.clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            47,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "execute".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract_address.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash: Hash::new(b"reused host execution class"),
            },
        );
        let mut host = CoreHost::new(authority.clone());
        host.bind_contract_runtime_context(contract_address.subject_id(), authorization.clone());
        assert_eq!(host.effect_authority(), contract_address.subject_id());
        assert!(matches!(
            host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_SET),
            Ok(())
        ));
        host.execution_class = HostExecutionClass::View;
        host.set_generic_execution();
        assert_eq!(host.execution_class, HostExecutionClass::Generic);
        assert!(host.current_contract_runtime_context.is_none());
        assert!(host.current_entrypoint_authorization.is_none());
        assert_eq!(host.effect_authority(), authority);
        assert!(matches!((&(host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_SET)), &(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_SET,
            })), (Err(ivm::VMError::GenericSyscallNotAllowed { syscall: __actual_syscall }), ivm::VMError::GenericSyscallNotAllowed { syscall: __expected_syscall }) if __actual_syscall == __expected_syscall));
        host.bind_contract_runtime_context(contract_address.subject_id(), authorization);
        assert_eq!(host.execution_class, HostExecutionClass::Contract);
        assert!(
            matches!(
                host.execution_class
                    .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_SET),
                Ok(())
            ),
            "a prior view binding must not leave the host over-restricted"
        );
    }
    #[test]
    fn local_debug_execution_classes_are_unbound_and_preserve_view_restrictions() {
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority);
        host.set_local_contract_debug_execution();
        assert_eq!(host.execution_class, HostExecutionClass::LocalContractDebug);
        assert!(host.current_contract_runtime_context.is_none());
        assert!(host.current_entrypoint_authorization.is_none());
        assert!(host.ensure_execution_artifacts_are_committable().is_err());
        assert!(matches!(
            host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_SET),
            Ok(())
        ));
        assert!(matches!(
            host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_REGISTER_DOMAIN),
            Ok(())
        ));
        for syscall in [
            ivm_sys::SYSCALL_AXT_BEGIN,
            ivm_sys::SYSCALL_AXT_TOUCH,
            ivm_sys::SYSCALL_AXT_COMMIT,
            ivm_sys::SYSCALL_VERIFY_DS_PROOF,
        ] {
            assert!(
                matches!(
                    host.execution_class.ensure_syscall_allowed(syscall),
                    Err(ivm::VMError::PermissionDenied)
                ),
                "state-free local debug calls must reject AXT syscall 0x{syscall:02x}"
            );
        }
        host.set_local_contract_debug_view_execution();
        assert_eq!(host.execution_class, HostExecutionClass::LocalViewDebug);
        assert!(host.current_contract_runtime_context.is_none());
        assert!(host.current_entrypoint_authorization.is_none());
        assert!(host.ensure_execution_artifacts_are_committable().is_err());
        assert!(matches!(
            host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_GET),
            Ok(())
        ));
        for syscall in [ivm_sys::SYSCALL_STATE_SET, ivm_sys::SYSCALL_REGISTER_DOMAIN] {
            assert!(
                matches!(
                    host.execution_class.ensure_syscall_allowed(syscall),
                    Err(ivm::VMError::PermissionDenied)
                ),
                "local debug views must reject effect syscall 0x{syscall:02x}"
            );
        }
        host.durable_state_overlay.insert(
            "debug_view_effect"
                .parse()
                .expect("valid durable state path"),
            Some(vec![1]),
        );
        assert!(
            host.ensure_view_execution_has_no_effect_artifacts()
                .is_err(),
            "local debug views must fail closed if mutable artifacts survive dispatch"
        );
        host.set_generic_execution();
        assert_eq!(host.execution_class, HostExecutionClass::Generic);
        assert!(
            host.ensure_execution_artifacts_are_committable().is_err(),
            "changing execution class must not clear local-debug provenance"
        );
    }
    #[test]
    fn state_free_generic_execution_rejects_axt_without_narrowing_generic_v1() {
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.set_state_free_generic_execution();
        assert_eq!(host.execution_class, HostExecutionClass::StateFreeGeneric);
        assert!(matches!(
            host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_REGISTER_DOMAIN),
            Ok(())
        ));
        assert!(matches!((&(host.execution_class
                .ensure_syscall_allowed(ivm_sys::SYSCALL_STATE_GET)), &(ivm::VMError::GenericSyscallNotAllowed {
                syscall: ivm_sys::SYSCALL_STATE_GET,
            })), (Err(ivm::VMError::GenericSyscallNotAllowed { syscall: __actual_syscall }), ivm::VMError::GenericSyscallNotAllowed { syscall: __expected_syscall }) if __actual_syscall == __expected_syscall));
        for syscall in [
            ivm_sys::SYSCALL_AXT_BEGIN,
            ivm_sys::SYSCALL_AXT_TOUCH,
            ivm_sys::SYSCALL_AXT_COMMIT,
            ivm_sys::SYSCALL_VERIFY_DS_PROOF,
        ] {
            assert!(
                matches!(
                    host.execution_class.ensure_syscall_allowed(syscall),
                    Err(ivm::VMError::PermissionDenied)
                ),
                "state-free generic execution must reject AXT syscall 0x{syscall:02x}"
            );
            assert!(
                ivm::syscalls::is_generic_program_syscall_allowed(
                    ivm::SyscallPolicy::AbiV1,
                    syscall
                ),
                "live-state generic ABI V1 execution must retain AXT syscall 0x{syscall:02x}"
            );
        }
    }
    #[test]
    fn local_contract_debug_artifacts_survive_runtime_rebinding_and_fail_closed() {
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority.clone());
        host.set_local_contract_debug_execution();
        let attempted_domain =
            DomainId::try_new("local_debug_guard", "universal").expect("valid domain id");
        let attempted = InstructionBox::from(RegisterBox::from(Register::domain(Domain::new(
            attempted_domain.clone(),
        ))));
        host.queue_instruction(attempted.clone())
            .expect("completed fixture gas quote");
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            49,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive rebound contract address");
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "execute".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract_address.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash: Hash::new(b"local debug rebound guard"),
            },
        );
        host.bind_contract_runtime_context(contract_address.subject_id(), authorization);
        assert_eq!(host.execution_class, HostExecutionClass::Contract);
        assert!(host.local_debug_artifacts);
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let header = BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let error = host
            .apply_queued(&mut stx, &authority)
            .expect_err("local debug artifacts must never be applied");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message == "local debug execution artifacts cannot be committed"
        ));
        assert!(stx.world.domain(&attempted_domain).is_err());
        assert!(
            stx.tx_call_hash.is_none(),
            "the local debug guard must run before execution identity is seeded"
        );
        assert_eq!(host.queued.len(), 1, "the guard must run before draining");
        assert_eq!(host.queued[0].instruction(), Some(&attempted));
        let error = match host.into_execution_artifacts(None) {
            Err(error) => error,
            Ok(_) => panic!("local debug artifacts must never be exported"),
        };
        assert!(matches!(
            error,
            crate::execution_attempt::ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(message))
                if message == "local debug execution artifacts cannot be committed"
        ));
    }
    #[test]
    fn view_effect_artifacts_fail_closed_before_application_or_extraction() {
        let authority = ALICE_ID.clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            48,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "inspect".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract_address.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash: Hash::new(b"view application guard"),
            },
        );
        let mut host = CoreHost::new(authority.clone());
        host.bind_contract_runtime_context(contract_address.subject_id(), authorization);
        host.execution_class = HostExecutionClass::View;
        let attempted_domain =
            DomainId::try_new("view_effect_guard", "universal").expect("valid domain id");
        let attempted = InstructionBox::from(RegisterBox::from(Register::domain(Domain::new(
            attempted_domain.clone(),
        ))));
        host.queue_instruction(attempted.clone())
            .expect("completed fixture gas quote");
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        let header = BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let error = host
            .apply_queued(&mut stx, &authority)
            .expect_err("a view must never apply a retained effect");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message == "read-only view execution retained mutable host artifacts"
        ));
        assert!(stx.world.domain(&attempted_domain).is_err());
        assert!(
            stx.tx_call_hash.is_none(),
            "the view guard must run before execution identity is seeded"
        );
        assert_eq!(host.queued.len(), 1, "the guard must run before draining");
        assert_eq!(host.queued[0].instruction(), Some(&attempted));
        let error = match host.into_execution_artifacts(None) {
            Err(error) => error,
            Ok(_) => panic!("a view must never export a retained effect"),
        };
        assert!(matches!(
            error,
            crate::execution_attempt::ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(message))
                if message == "read-only view execution retained mutable host artifacts"
        ));
    }
    #[test]
    fn prevalidated_runtime_binding_captures_exact_root_authorization() {
        let authority = ALICE_ID.clone();
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            43,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let code_hash = Hash::new(b"Torii prevalidated runtime binding");
        let mut host = CoreHost::new(authority.clone());
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "inspect".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission(
                "CanInspectContract".parse().unwrap(),
            ),
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash,
            },
        );
        host.bind_contract_runtime_context(contract.subject_id(), authorization);
        let context = host
            .current_contract_runtime_context
            .as_ref()
            .expect("runtime context");
        assert_eq!(context.contract_address, contract);
        assert_eq!(context.contract_subject, contract.subject_id());
        assert_eq!(context.entrypoint, "inspect");
        let authorization = host
            .current_entrypoint_authorization
            .as_ref()
            .expect("root authorization");
        assert!(authorization.is_root());
        assert_eq!(authorization.authority, authority);
        assert_eq!(authorization.contract_address, contract);
        assert_eq!(authorization.code_hash, code_hash);
        assert_eq!(authorization.entrypoint, "inspect");
        assert_eq!(
            authorization.authorization,
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Permission(
                "CanInspectContract".parse().unwrap()
            )
        );
    }
    #[test]
    fn authorized_deployed_binders_reject_forgery_and_clear_stale_provenance() {
        let authority = ALICE_ID.clone();
        let state = contract_test_state(&authority);
        let legitimate_address = install_contract(
            &state,
            &authority,
            r"
seiyaku LegitimateBinding { permission CanExecuteContract; permission CanInspectContract;
  kotoage fn execute() authorize(CanExecuteContract) -> int {
    return 1;
  }

  view fn inspect() authorize(CanInspectContract) -> int {
    return 2;
  }
}
",
            44,
        );
        let privileged_address = install_contract(
            &state,
            &authority,
            r"
seiyaku PrivilegedBinding { permission CanAdministerContract;
  kotoage fn administer() authorize(CanAdministerContract) -> int {
    return 3;
  }
}
",
            45,
        );
        grant_named_permission_to_account(
            &state,
            &authority,
            &(authority.clone()),
            "CanExecuteContract",
        );
        grant_named_permission_to_account(
            &state,
            &authority,
            &(authority.clone()),
            "CanInspectContract",
        );
        let view = state.view();
        let legitimate =
            crate::smartcontracts::code::fetch_bound_contract_record(&view, &legitimate_address)
                .expect("registry read completes")
                .expect("legitimate contract record");
        let privileged =
            crate::smartcontracts::code::fetch_bound_contract_record(&view, &privileged_address)
                .expect("registry read completes")
                .expect("privileged contract record");
        let legitimate_prepared =
            ivm::prepare_contract(Arc::<[u8]>::from(legitimate.code_bytes.clone()))
                .expect("prepare legitimate contract");
        let privileged_prepared =
            ivm::prepare_contract(Arc::<[u8]>::from(privileged.code_bytes.clone()))
                .expect("prepare privileged contract");
        let mut host = CoreHost::new(authority.clone());
        let bind_legitimate = |host: &mut CoreHost| {
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &legitimate_address,
                legitimate.contract_alias.as_ref(),
                &legitimate_prepared,
                "execute",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection)
            .expect("bind legitimate entrypoint");
        };
        let assert_cleared = |host: &CoreHost| {
            assert!(
                host.current_contract_runtime_context.is_none(),
                "a rejected rebind must remove stale runtime identity"
            );
            assert!(
                host.current_entrypoint_authorization.is_none(),
                "a rejected rebind must remove stale authorization"
            );
        };
        bind_legitimate(&mut host);
        let forged_alias: ContractAlias = "forged::universal"
            .parse()
            .expect("syntactically valid forged alias");
        assert!(matches!(
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &legitimate_address,
                Some(&forged_alias),
                &legitimate_prepared,
                "execute",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
        bind_legitimate(&mut host);
        assert!(matches!(
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &legitimate_address,
                legitimate.contract_alias.as_ref(),
                &privileged_prepared,
                "administer",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
        bind_legitimate(&mut host);
        assert!(matches!(
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &privileged_address,
                privileged.contract_alias.as_ref(),
                &privileged_prepared,
                "administer",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
        bind_legitimate(&mut host);
        assert!(matches!(
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &legitimate_address,
                legitimate.contract_alias.as_ref(),
                &legitimate_prepared,
                "inspect",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
        bind_legitimate(&mut host);
        assert!(matches!(
            host.bind_authorized_deployed_contract_view_runtime_context(
                &view,
                &legitimate_address,
                legitimate.contract_alias.as_ref(),
                &legitimate_prepared,
                "execute",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
        bind_legitimate(&mut host);
        assert!(matches!(
            host.bind_authorized_deployed_contract_runtime_context(
                &view,
                &legitimate_address,
                legitimate.contract_alias.as_ref(),
                &legitimate_prepared,
                "forged_selector",
            )
            .map_err(crate::execution_attempt::expect_completed_rejection),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert_cleared(&host);
    }
    #[test]
    fn contract_runtime_state_rejects_manifest_free_vm_image() {
        let authority = ALICE_ID.clone();
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            44,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let mut host = CoreHost::new(authority.clone());
        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority,
            "inspect".to_owned(),
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            &crate::smartcontracts::code::BoundContractIdentity {
                lifecycle_revision: 1,
                contract_address: contract.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash: Hash::new(b"manifest-free runtime fixture"),
            },
        );
        host.bind_contract_runtime_context(contract.subject_id(), authorization);
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("generic image parses outside deployable admission");
        let path: StatePath = "state".parse().expect("state path");
        let path_payload = norito::to_bytes(&path).expect("encode state path");
        let path_pointer = vm
            .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes as u16, &path_payload))
            .expect("allocate state path");
        vm.set_register(10, path_pointer);
        assert!(
            matches!(
                host.syscall(ivm::syscalls::SYSCALL_STATE_GET, &mut vm),
                Err(ivm::VMError::InvalidMetadata)
            ),
            "a contract runtime context must never fall back to untyped durable state"
        );
    }
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("IVM host fixture key generation should succeed")
    }
    fn checked_public_key() -> PublicKey {
        checked_keypair().public_key().clone()
    }
    fn checked_peer_id() -> PeerId {
        PeerId::new(checked_public_key())
    }
    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }
    #[test]
    fn strict_policy_accepts_domain_in_abi_current() {
        // Prepare VM with ABI v1 (baseline)
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let mut vm = ivm::IVM::new(50_000_000);
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
        let mut vm = ivm::IVM::new(50_000_000);
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
        let mut vm = ivm::IVM::new(10_000);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority);
        let account: AccountId = fixture_account("alice");
        let inner = make_tlv(
            PointerType::AccountId as u16,
            &norito::to_bytes(&account).expect("encode account"),
        );
        CoreHost::load_state_value(&mut vm, &inner).expect("load raw state value");
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
        CoreHost::load_state_value(&mut vm, &inner).expect("load raw state value");
        vm.set_register(11, u64::from(PointerType::AssetDefinitionId as u16));
        host.syscall(ivm::syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm)
            .expect("pointer_from_norito");
        let decoded: AssetDefinitionId =
            CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AssetDefinitionId)
                .expect("decode asset definition pointer from state");
        assert_eq!(decoded, asset);
    }
    #[test]
    fn load_state_value_rejects_oversized_raw_payload() {
        let mut vm = ivm::IVM::new(10_000);
        let stored = vec![0u8; ivm::syscalls::STATE_MAX_VALUE_BYTES + 1];
        let err =
            CoreHost::load_state_value(&mut vm, &stored).expect_err("oversized raw state payload");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
    }
    #[test]
    fn load_state_value_wraps_raw_norito_payload_into_norito_bytes_tlv() {
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
    fn axt_stage_anchored_spend_preserves_exact_signed_wire_in_call_order() {
        let descriptor_fixture: norito::json::Value =
            norito::json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json"
            )))
            .expect("current descriptor fixture");
        let descriptor: axt::AxtDescriptor =
            norito::json::from_value(descriptor_fixture["descriptor"].clone())
                .expect("ABI descriptor fixture");
        let spend_fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../iroha_data_model/tests/fixtures/axt_envelope_multi_ds.json"
        )))
        .expect("current signed-spend fixture");
        let spends: Vec<AxtAnchoredSpendV1> = spend_fixture["spends"]["happy"]
            .as_array()
            .expect("signed-spend fixture array")
            .iter()
            .cloned()
            .map(|value| norito::json::from_value(value).expect("signed spend"))
            .collect();
        assert_eq!(spends.len(), 2);
        let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
        assert!(spends.iter().all(|spend| {
            spend.draft.handle.axt_binding == AxtBinding::new(binding)
                && spend.issuer_payload_v1().is_ok()
        }));
        let mut host = CoreHost::new(ALICE_ID.clone());
        let mut vm = IVM::new(1_000_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        for spend in &spends {
            let payload = norito_blob(spend);
            let ptr = store_tlv(&mut vm, PointerType::AxtAnchoredSpendV1, &payload);
            vm.set_register(10, ptr);
            assert_eq!(
                (host.syscall(ivm_sys::SYSCALL_AXT_STAGE_ANCHORED_SPEND, &mut vm))
                    .expect("expected successful fixture result"),
                CoreHost::axt_gas(payload.len())
            );
        }
        let active = host.axt_state.as_ref().expect("active AXT envelope");
        assert_eq!(active.spends(), spends);
        let materialized = CoreHost::materialize_axt_record(active, LaneId::new(0), 1);
        assert_eq!(materialized.spends, spends);
        assert!(host.completed_axt.is_empty());
    }
    #[test]
    fn axt_verify_ds_proof_rejects_unanchored_fastpq_without_state_or_cache_mutation() {
        let dsid = DataSpaceId::new(3);
        let manifest_root = [0xAB; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        host.cache_proof_entry(
            dsid,
            [0x44; 32],
            Some(50),
            Some(5),
            Some(manifest_root),
            true,
            AXT_PROOF_CACHE_HIT,
        );
        let proofs_before = host
            .axt_state
            .as_ref()
            .expect("active AXT envelope")
            .proofs()
            .clone();
        let cache_before = Arc::clone(&host.axt_proof_cache);
        let cache_slot_before = host.axt_proof_cache_slot;
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);
        let proof = proof_blob_for(dsid, manifest_root, b"unanchored-standalone", 20);
        let proof_bytes = norito_blob(&proof);
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &proof_bytes);
        vm.set_register(11, proof_ptr);
        assert!(
            matches!(
                host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
                Err(VMError::PermissionDenied)
            ),
            "a valid caller-carried FASTPQ proof must not become standalone authorization"
        );
        assert_eq!(
            host.axt_state
                .as_ref()
                .expect("active AXT envelope")
                .proofs(),
            &proofs_before,
            "an unanchored proof must not be recorded"
        );
        assert_eq!(
            host.axt_proof_cache.as_ref(),
            cache_before.as_ref(),
            "an unanchored proof must not alter the verified-proof cache"
        );
        assert_eq!(host.axt_proof_cache_slot, cache_slot_before);
        let reject = host.take_axt_reject_for_tests().expect("reject context");
        assert_eq!(reject.reason, AxtRejectReason::Proof);
        assert_eq!(reject.dataspace, Some(dsid));
        assert!(
            reject
                .detail
                .contains("authoritative finalized source-state anchor")
        );
    }

    #[test]
    fn axt_verify_ds_proof_rejects_opaque_carrier_before_valid_cache_hit() {
        let dsid = DataSpaceId::new(103);
        let manifest_root = [0xA3; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let mut host = CoreHost::new(ALICE_ID.clone())
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let opaque = opaque_proof_blob_for(
            dsid,
            manifest_root,
            b"opaque-authorization-cache-attack",
            20,
        );
        let digest = CoreHost::axt_proof_cache_digest(&opaque);
        host.cache_proof_entry(
            dsid,
            digest,
            Some(20),
            Some(5),
            Some(manifest_root),
            true,
            AXT_PROOF_CACHE_HIT,
        );
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        let proof_ptr = store_tlv(&mut vm, PointerType::ProofBlob, &norito_blob(&opaque));
        vm.set_register(10, ds_ptr);
        vm.set_register(11, proof_ptr);
        assert!(
            matches!(
                host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
                Err(VMError::PermissionDenied)
            ),
            "an old valid cache bit must not make an opaque carrier contract-observable"
        );
        let reject = host.take_axt_reject_for_tests().expect("reject context");
        assert_eq!(reject.reason, AxtRejectReason::Proof);
        assert!(
            reject
                .detail
                .contains("authoritative finalized source-state anchor")
        );
    }

    #[test]
    fn axt_proof_validation_rejects_oversized_payload_before_decode() {
        let dsid = DataSpaceId::new(104);
        let manifest_root = [0xA4; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let mut host = CoreHost::new(ALICE_ID.clone())
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        let proof = ProofBlob {
            payload: vec![0u8; fastpq_prover::MAX_AXT_PROOF_BLOB_PAYLOAD_BYTES + 1],
            expiry_slot: Some(20),
        };

        assert!(matches!(
            host.validate_axt_proof(dsid, &proof, policy, AxtProofAdmission::AuthenticatedHandle,),
            Err(VMError::NoritoInvalid)
        ));
        let reject = host.take_axt_reject_for_tests().expect("reject context");
        assert_eq!(reject.reason, AxtRejectReason::Proof);
        assert!(reject.detail.contains("decode limit"));
    }

    #[test]
    fn axt_verify_ds_proof_rejects_mutated_outer_expiry_on_cache_hit() {
        let dsid = DataSpaceId::new(31);
        let manifest_root = [0x31; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let live_proof = proof_blob_for(dsid, manifest_root, b"outer-expiry-alias", 20);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(
            host.validate_axt_proof(
                dsid,
                &live_proof,
                policy,
                AxtProofAdmission::AuthenticatedHandle,
            )
            .is_ok(),
            "an authenticated handle proof must populate the cache"
        );
        assert!(host.axt_proof_cache.contains_key(&dsid));

        let expired_alias = ProofBlob {
            payload: live_proof.payload.clone(),
            expiry_slot: Some(4),
        };
        assert!(
            matches!(
                host.validate_axt_proof(
                    dsid,
                    &expired_alias,
                    policy,
                    AxtProofAdmission::AuthenticatedHandle,
                ),
                Err(VMError::PermissionDenied)
            ),
            "an expired outer expiry must not alias a cached live payload"
        );
        let reject = host.take_axt_reject_for_tests().expect("reject context");
        assert_eq!(reject.reason, AxtRejectReason::Expiry);
        assert!(
            host.axt_proof_cache.contains_key(&dsid),
            "invalid outer metadata must not evict a valid cached payload"
        );

        let extended_alias = ProofBlob {
            payload: live_proof.payload.clone(),
            expiry_slot: Some(200),
        };
        assert!(
            matches!(
                host.validate_axt_proof(
                    dsid,
                    &extended_alias,
                    policy,
                    AxtProofAdmission::AuthenticatedHandle,
                ),
                Err(VMError::PermissionDenied)
            ),
            "a longer outer expiry must not alias a cached proof payload"
        );
        let reject = host.take_axt_reject_for_tests().expect("reject context");
        assert_eq!(reject.reason, AxtRejectReason::Proof);
        assert!(
            host.axt_proof_cache.contains_key(&dsid),
            "an unauthenticated longer expiry must not evict the valid cache entry"
        );
        assert!(
            host.validate_axt_proof(
                dsid,
                &live_proof,
                policy,
                AxtProofAdmission::AuthenticatedHandle,
            )
            .is_ok(),
            "the original live proof must remain reusable after alias rejection"
        );
        assert_eq!(
            host.axt_proof_cache
                .get(&dsid)
                .expect("live cache entry retained")
                .status,
            AXT_PROOF_CACHE_HIT
        );
    }
    #[test]
    fn axt_verify_ds_proof_rejects_undeclared_dataspace() {
        let dsid = DataSpaceId::new(3);
        let other = DataSpaceId::new(4);
        let manifest_root = [0xAB; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 5);
        let authority = ALICE_ID.clone();
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
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
            .and_then(|host| host.with_axt_timing(timing))
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let proof = proof_blob_for(dsid, manifest_root, b"raw-expiry", 11);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(matches!(
            host.validate_axt_proof(dsid, &proof, policy, AxtProofAdmission::AuthenticatedHandle,),
            Ok(())
        ));
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
        let dsid = DataSpaceId::new(7);
        let manifest_root = [0x11; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let mut current_slot = 9;
        let snapshot = make_policy_snapshot(dsid, manifest_root, current_slot);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let proof = proof_blob_for(dsid, manifest_root, b"cache-slot", 30);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(matches!(
            host.validate_axt_proof(dsid, &proof, policy, AxtProofAdmission::AuthenticatedHandle,),
            Ok(())
        ));
        let entry = host.axt_proof_cache.get(&dsid).expect("cache populated");
        assert_eq!(entry.verified_slot, current_slot);
        assert_eq!(entry.manifest_root, Some(manifest_root));
        current_slot += 1;
        if let Some(snapshot) = host.axt_policy_snapshot.as_mut() {
            snapshot.entries[0].policy.current_slot = current_slot;
        }
        let policy = host.policy_entry_for(dsid).expect("updated policy entry");
        assert!(
            matches!(
                host.validate_axt_proof(
                    dsid,
                    &proof,
                    policy,
                    AxtProofAdmission::AuthenticatedHandle,
                ),
                Ok(())
            ),
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
        let policy = host.policy_entry_for(dsid).expect("rotated policy entry");
        assert!(
            matches!(
                host.validate_axt_proof(
                    dsid,
                    &new_proof,
                    policy,
                    AxtProofAdmission::AuthenticatedHandle,
                ),
                Ok(())
            ),
            "manifest rotation must invalidate prior cache entry"
        );
        let entry = host
            .axt_proof_cache
            .get(&dsid)
            .expect("cache updated after manifest change");
        assert_eq!(entry.manifest_root, Some(new_manifest_root));
        let expected_digest = CoreHost::axt_proof_cache_digest(&new_proof);
        assert_eq!(entry.digest, expected_digest);
    }
    #[test]
    fn axt_touch_rejects_manifest_prefix_violation_with_context() {
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
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
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
    #[cfg(feature = "telemetry")]
    #[test]
    fn axt_proof_cache_metrics_refresh_on_manifest_rotation() {
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
            .and_then(|host| {
                host.with_axt_timing(iroha_config::parameters::actual::NexusAxt::default())
            })
            .expect("canonical policy snapshot");
        host.set_telemetry(telemetry.clone());
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let proof = proof_blob_for(dsid, manifest_root, b"telemetry-cache", 30);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(matches!(
            host.validate_axt_proof(dsid, &proof, policy, AxtProofAdmission::AuthenticatedHandle,),
            Ok(())
        ));
        let snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.status, AXT_PROOF_CACHE_MISS);
        assert_eq!(entry.verified_slot, current_slot);
        assert_eq!(entry.expiry_slot, Some(30));
        current_slot += 1;
        let rotated_root = [0x22; 32];
        let rotated_snapshot = make_policy_snapshot(dsid, rotated_root, current_slot);
        host.refresh_axt_policy_snapshot(&rotated_snapshot)
            .expect("canonical rotated snapshot");
        let cleared_snapshot = telemetry.axt_proof_cache_status_snapshot();
        assert_eq!(cleared_snapshot.len(), 1);
        let cleared = &cleared_snapshot[0];
        assert_eq!(cleared.status, "cleared");
        assert_eq!(cleared.manifest_root, Some(manifest_root));
        assert_eq!(cleared.verified_slot, 9);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let rotated_proof = proof_blob_for(dsid, rotated_root, b"telemetry-cache-rotated", 40);
        let policy = host.policy_entry_for(dsid).expect("rotated policy entry");
        assert!(matches!(
            host.validate_axt_proof(
                dsid,
                &rotated_proof,
                policy,
                AxtProofAdmission::AuthenticatedHandle,
            ),
            Ok(())
        ));
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
    #[test]
    fn axt_verify_ds_proof_rejects_expired_and_supports_clear() {
        let dsid = DataSpaceId::new(11);
        let manifest_root = [0x33; 32];
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let snapshot = make_policy_snapshot(dsid, manifest_root, 12);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let mut vm = IVM::new(10_000);
        begin_axt_envelope(&mut host, &mut vm, &descriptor);
        let ds_ptr = store_tlv(&mut vm, PointerType::DataSpaceId, &norito_blob(&dsid));
        vm.set_register(10, ds_ptr);
        let expired_proof = proof_blob_for(dsid, manifest_root, b"expired-proof", 10);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(
            matches!(
                host.validate_axt_proof(
                    dsid,
                    &expired_proof,
                    policy,
                    AxtProofAdmission::AuthenticatedHandle,
                ),
                Err(VMError::PermissionDenied)
            ),
            "expired proof must be rejected"
        );
        assert!(host.axt_proof_cache.is_empty());
        let live_proof = proof_blob_for(dsid, manifest_root, b"live-proof", 50);
        assert!(matches!(
            host.validate_axt_proof(
                dsid,
                &live_proof,
                policy,
                AxtProofAdmission::AuthenticatedHandle,
            ),
            Ok(())
        ));
        assert!(host.axt_proof_cache.contains_key(&dsid));
        vm.set_register(11, 0);
        assert_eq!(
            (host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm))
                .expect("expected successful fixture result"),
            CoreHost::axt_verify_gas(0),
            "proof clear should drop cache entry"
        );
        assert!(host.axt_proof_cache.is_empty());
    }
    #[test]
    fn axt_proof_cache_reused_across_envelopes_in_same_slot() {
        let dsid = DataSpaceId::new(12);
        let manifest_root = [0x44; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 20);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
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
    fn axt_timing_change_without_snapshot_clears_defensive_cache_state() {
        let dsid = DataSpaceId::new(22);
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("axt binding");
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        host.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
        host.cache_proof_entry(
            dsid,
            [0x22; 32],
            Some(30),
            Some(20),
            Some([0x44; 32]),
            true,
            AXT_PROOF_CACHE_MISS,
        );
        assert!(host.axt_policy_snapshot.is_none());
        assert_eq!(host.axt_proof_cache_slot, Some(20));
        let timing = iroha_config::parameters::actual::NexusAxt {
            max_clock_skew_ms: host.axt_timing.max_clock_skew_ms.saturating_add(1),
            ..host.axt_timing
        };
        host.set_axt_timing(timing)
            .expect("timing without a snapshot is valid");
        assert!(host.axt_state.is_none());
        assert!(host.axt_proof_cache.is_empty());
        assert!(host.axt_proof_cache_slot.is_none());
    }
    #[test]
    fn rejected_axt_timing_change_is_fully_non_mutating() {
        let dsid = DataSpaceId::new(23);
        let mut snapshot = make_policy_snapshot(dsid, [0x23; 32], 20);
        let expected_version = snapshot.version;
        snapshot.version = expected_version.wrapping_add(1);
        let advertised_version = snapshot.version;
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("axt binding");
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        host.axt_policy_snapshot = Some(snapshot);
        host.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
        host.cache_proof_entry(
            dsid,
            [0x23; 32],
            Some(30),
            Some(20),
            Some([0x23; 32]),
            true,
            AXT_PROOF_CACHE_MISS,
        );
        let timing_before = host.axt_timing;
        let state_before = Arc::clone(host.axt_state.as_ref().expect("active AXT envelope"));
        let cache_before = Arc::clone(&host.axt_proof_cache);
        let slot_before = host.axt_proof_cache_slot;
        let replacement = iroha_config::parameters::actual::NexusAxt {
            max_clock_skew_ms: timing_before.max_clock_skew_ms.saturating_add(1),
            ..timing_before
        };
        assert!(matches!(
            host.set_axt_timing(replacement),
            Err(AxtPolicySnapshotValidationError::VersionMismatch {
                expected,
                actual,
            }) if expected == expected_version && actual == advertised_version
        ));
        assert_eq!(host.axt_timing, timing_before);
        assert!(Arc::ptr_eq(
            host.axt_state.as_ref().expect("active envelope preserved"),
            &state_before
        ));
        assert!(Arc::ptr_eq(&host.axt_proof_cache, &cache_before));
        assert_eq!(host.axt_proof_cache_slot, slot_before);
    }
    #[test]
    fn axt_proof_cache_snapshot_exposes_entries() {
        let dsid = DataSpaceId::new(21);
        let manifest_root = [0x77; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 15);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
        let descriptor = axt::AxtDescriptor {
            dsids: vec![dsid],
            touches: Vec::new(),
        };
        let binding = axt::compute_binding(&descriptor).expect("axt binding");
        host.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
        let proof = proof_blob_for(dsid, manifest_root, b"cache-snapshot", 30);
        let policy = host.policy_entry_for(dsid).expect("policy entry");
        assert!(matches!(
            host.validate_axt_proof(dsid, &proof, policy, AxtProofAdmission::AuthenticatedHandle,),
            Ok(())
        ));
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
        let dsid = DataSpaceId::new(13);
        let manifest_root = [0x55; 32];
        let snapshot = make_policy_snapshot(dsid, manifest_root, 30);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority)
            .with_axt_policy_snapshot(&snapshot)
            .expect("canonical policy snapshot");
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
            .and_then(|host| host.with_axt_timing(timing))
            .expect("canonical policy snapshot");
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
    include!("host/axt_unanchored_admission_tests.rs");
    #[test]
    fn register_contract_manifest_syscall_queues_instruction() {
        let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let authority = AccountId::of(kp.public_key().clone());
        let mut host = local_contract_host(authority);
        let code_hash = IrohaHash::new(b"contract-code");
        let abi_hash = IrohaHash::new(b"contract-abi");
        let request = scode::RegisterSmartContractCode {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                DataSpaceId::UNIVERSAL,
                code_hash,
            ),
            manifest: ContractManifest {
                events: Vec::new(),
                enum_types: Vec::new(),
                permissions: Vec::new(),
                seiyaku_name: None,
                code_hash: Some(code_hash),
                abi_hash: Some(abi_hash),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_messages: None,
                error_types: None,
                provenance: None,
            }
            .try_signed(
                manifest_signing.context(),
                manifest_signing.max_frame_bytes(),
                &kp,
            )
            .expect("sign bounded fixture manifest"),
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = local_contract_host(authority);
        let code_hash = IrohaHash::new(b"bytecode");
        let request = scode::RegisterSmartContractBytes {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                DataSpaceId::UNIVERSAL,
                code_hash,
            ),
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_contract_bytes_syscall_accepts_heap_tlv() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = local_contract_host(authority);
        let code_hash = IrohaHash::new(b"heap-bytecode");
        let request = scode::RegisterSmartContractBytes {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                DataSpaceId::UNIVERSAL,
                code_hash,
            ),
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
            CoreHost::decode_tlv_typed(&vm, ptr, PointerType::NoritoBytes)
                .expect("decode heap register request");
        assert_eq!(decoded, request);
        vm.set_register(10, ptr);
        let res = host.syscall(
            ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
            &mut vm,
        );
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_contract_bytes_syscall_rejects_unowned_tlv_regions() {
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let request = scode::RegisterSmartContractBytes {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                DataSpaceId::UNIVERSAL,
                IrohaHash::new(b"unowned-bytecode"),
            ),
            code: vec![0xAA, 0xBB, 0xCC],
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        for (label, pointer) in [
            ("unallocated heap", ivm::Memory::HEAP_START),
            ("output", ivm::Memory::OUTPUT_START),
            ("stack", ivm::Memory::STACK_START),
        ] {
            let mut vm = ivm::IVM::new(1_000);
            vm.store_bytes(pointer, &tlv)
                .unwrap_or_else(|error| panic!("store {label} fixture: {error:?}"));
            vm.set_register(10, pointer);
            let mut host = local_contract_host(authority.clone());
            assert!(
                matches!(
                    host.syscall(
                        ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
                        &mut vm,
                    ),
                    Err(ivm::VMError::NoritoInvalid)
                ),
                "{label} must not be accepted as pointer-ABI provenance"
            );
            assert!(host.queued.is_empty());
        }
        let mut vm = ivm::IVM::new(1_000);
        let mut code = ivm::encoding::wide::encode_halt().to_le_bytes().to_vec();
        let pointer = u64::try_from(code.len()).expect("code offset fits u64");
        code.extend_from_slice(&tlv);
        vm.load_code(&code).expect("load arbitrary code bytes");
        vm.set_register(10, pointer);
        let mut host = local_contract_host(authority.clone());
        assert!(matches!(
            host.syscall(
                ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
                &mut vm,
            ),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert!(host.queued.is_empty());
        let mut partial_vm = ivm::IVM::new(1_000);
        let owned_tlv_bytes = tlv
            .len()
            .checked_sub(8)
            .expect("request envelope exceeds one HEAP alignment unit");
        let partial_pointer = partial_vm
            .alloc_heap(u64::try_from(owned_tlv_bytes).expect("partial TLV length fits u64"))
            .expect("allocate truncated heap ownership");
        partial_vm
            .store_bytes(partial_pointer, &tlv)
            .expect("store across unowned heap boundary");
        partial_vm.set_register(10, partial_pointer);
        let mut host = local_contract_host(authority.clone());
        assert!(matches!(
            host.syscall(
                ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
                &mut partial_vm,
            ),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert!(host.queued.is_empty());
        let mut corrupted = tlv.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        for (label, envelope, expected) in [
            (
                "wrong pointer type",
                make_tlv(PointerType::Blob as u16, &payload),
                ivm::VMError::NoritoInvalid,
            ),
            (
                "corrupted payload hash",
                corrupted,
                ivm::VMError::NoritoInvalid,
            ),
            (
                "malformed Norito payload",
                make_tlv(PointerType::NoritoBytes as u16, b"not a request"),
                ivm::VMError::DecodeError,
            ),
        ] {
            let mut vm = ivm::IVM::new(1_000);
            let pointer = vm
                .alloc_host_tlv(&envelope)
                .unwrap_or_else(|error| panic!("allocate {label} fixture: {error:?}"));
            vm.set_register(10, pointer);
            let mut host = local_contract_host(authority.clone());
            let actual = host.syscall(
                ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
                &mut vm,
            );
            assert!(
                matches!(
                    (&actual, &expected),
                    (
                        Err(ivm::VMError::NoritoInvalid),
                        ivm::VMError::NoritoInvalid
                    ) | (Err(ivm::VMError::DecodeError), ivm::VMError::DecodeError)
                ),
                "{label} must fail closed: {actual:?} != {expected:?}"
            );
            assert!(host.queued.is_empty());
        }
    }
    #[test]
    fn activate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = local_contract_host(authority);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &host.authority,
            0,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let request = scode::ActivateContractInstance {
            contract_address,
            expected_revision: 1,
            code_hash: IrohaHash::new(b"payments-code"),
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);
        let res = host.syscall(ivm::syscalls::SYSCALL_ACTIVATE_CONTRACT_INSTANCE, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn deactivate_contract_instance_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = local_contract_host(authority);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &host.authority,
            1,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let request = scode::DeactivateContractInstance {
            contract_address,
            expected_revision: 1,
            reason: Some("compromised deployment".to_owned()),
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);
        let res = host.syscall(ivm::syscalls::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn lifecycle_hooks_reject_direct_binding_syscalls_before_metering() {
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            211,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let activate = scode::ActivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            code_hash: IrohaHash::new(b"self-activation-code"),
        };
        let deactivate = scode::DeactivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            reason: Some("self-deactivation attempt".to_owned()),
        };
        let requests = [
            (
                ivm_sys::SYSCALL_ACTIVATE_CONTRACT_INSTANCE,
                norito::to_bytes(&activate).expect("encode activation"),
            ),
            (
                ivm_sys::SYSCALL_DEACTIVATE_CONTRACT_INSTANCE,
                norito::to_bytes(&deactivate).expect("encode deactivation"),
            ),
        ];
        for entrypoint in ["hajimari", "始まり", "kaizen", "改善"] {
            for (syscall, payload) in &requests {
                let mut host = CoreHost::new(authority.clone());
                host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
                    contract_address: contract_address.clone(),
                    contract_subject: contract_address.subject_id(),
                    contract_alias: None,
                    entrypoint: entrypoint.to_owned(),
                }));
                let mut vm = IVM::new(1_000);
                let pointer = store_tlv(&mut vm, PointerType::NoritoBytes, payload);
                vm.set_register(10, pointer);
                let gas_before = vm.remaining_gas();
                assert!(
                    matches!(
                        host.prepare_syscall(*syscall, &vm),
                        Err(ivm::VMError::PermissionDenied)
                    ),
                    "{entrypoint} must fail during syscall preparation"
                );
                assert_eq!(vm.remaining_gas(), gas_before);
                assert!(
                    matches!(
                        host.syscall(*syscall, &mut vm),
                        Err(ivm::VMError::PermissionDenied)
                    ),
                    "{entrypoint} must also fail closed when the host is invoked directly"
                );
                assert_eq!(vm.remaining_gas(), gas_before);
                assert!(host.queued.is_empty());
            }
        }
    }
    #[test]
    fn lifecycle_hooks_cannot_smuggle_binding_mutations_through_opaque_syscall() {
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            212,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let instruction = InstructionBox::from(scode::ActivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            code_hash: IrohaHash::new(b"opaque-self-activation-code"),
        });
        let payload = norito::to_bytes(&instruction).expect("encode opaque instruction");
        let mut exhausted_host = CoreHost::new(authority.clone());
        exhausted_host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_address: contract_address.clone(),
            contract_subject: contract_address.subject_id(),
            contract_alias: None,
            entrypoint: "hajimari".to_owned(),
        }));
        let mut exhausted_vm = IVM::new(0);
        exhausted_vm.set_register(10, u64::MAX);
        assert!(
            matches!(
                exhausted_host.prepare_syscall(
                    ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
                    &exhausted_vm,
                ),
                Err(ivm::VMError::OutOfGas)
            ),
            "an unaffordable opaque call must fail before its invalid pointer is decoded"
        );
        assert!(exhausted_host.queued.is_empty());
        for entrypoint in ["hajimari", "始まり", "kaizen", "改善"] {
            let mut host = CoreHost::new(authority.clone());
            host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
                contract_address: contract_address.clone(),
                contract_subject: contract_address.subject_id(),
                contract_alias: None,
                entrypoint: entrypoint.to_owned(),
            }));
            let mut vm = IVM::new(1_000);
            let pointer = store_tlv(&mut vm, PointerType::NoritoBytes, &payload);
            vm.set_register(10, pointer);
            vm.set_register(11, ivm_sys::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT);
            let gas_before = vm.remaining_gas();
            assert!(
                host.prepare_syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &vm)
                    .is_ok(),
                "opaque instruction quotation must not decode attacker-controlled payloads"
            );
            assert_eq!(vm.remaining_gas(), gas_before);
            assert!(matches!(
                host.syscall(ivm_sys::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
                Err(ivm::VMError::PermissionDenied)
            ));
            assert_eq!(vm.remaining_gas(), gas_before);
            assert!(host.queued.is_empty());
        }
    }
    #[test]
    fn remove_contract_bytes_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let kp = checked_keypair();
        let (public_key, _) = kp.into_parts();
        let authority = AccountId::of(public_key);
        let mut host = local_contract_host(authority);
        let code_hash = IrohaHash::new(b"bytecode-image");
        let request = scode::RemoveSmartContractBytes {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                DataSpaceId::UNIVERSAL,
                code_hash,
            ),
            reason: None,
        };
        let payload = norito::to_bytes(&request).expect("encode request");
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
        vm.memory.preload_input(0, &tlv).expect("preload tlv");
        vm.set_register(10, ivm::Memory::INPUT_START);
        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SMART_CONTRACT_BYTES, &mut vm);
        let expected = InstructionBox::from(request.clone());
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn live_nested_dispatch_rejects_foreign_private_root_addresses_before_lookup() {
        let own = DataSpaceId::new((1_u64 << 40) + 31);
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(IrohaHash::new(b"parent")),
        );
        let world = crate::sumeragi::lanes::routing::test_support::world(
            iroha_data_model::block::consensus::SumeragiRootScope::Dataspace {
                parent_network_id: network,
                dataspace_id: own,
            },
        );
        let state = crate::state::State::new_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let authority = fixture_account("alice");
        let view = state.view();
        for foreign in [DataSpaceId::UNIVERSAL, DataSpaceId::new(31)] {
            let address = ContractAddress::derive(&network, &authority, 0, foreign).unwrap();
            assert!(matches!(
                CoreHost::contract_instance_by_address(&view, &address),
                Err(ivm::VMError::PermissionDenied)
            ));
            assert!(matches!(
                CoreHost::bound_contract_dispatch_identity(&view, &address),
                Err(ivm::VMError::PermissionDenied)
            ));
        }
        let local = ContractAddress::derive(&network, &authority, 0, own).unwrap();
        assert!(
            matches!(
                CoreHost::contract_instance_by_address(&view, &local),
                Err(ivm::VMError::DecodeError)
            ),
            "exact local scope proceeds to the missing-binding lookup"
        );
    }
    #[test]
    fn vm_produced_instruction_cannot_acquire_unreviewed_private_root_authority() {
        let authority: AccountId = fixture_account("alice");
        let account: AccountId = fixture_account("bob");
        let own = DataSpaceId::new((1_u64 << 40) + 31);
        let scope = iroha_data_model::block::consensus::SumeragiRootScope::Dataspace {
            parent_network_id: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(IrohaHash::new(b"private-parent")),
            ),
            dataspace_id: own,
        };
        let world = crate::sumeragi::lanes::routing::test_support::world(scope);
        let state = crate::state::State::new_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        let mut block = state.block(iroha_data_model::block::BlockHeader::new(
            2.try_into().unwrap(),
            None,
            None,
            1_000,
            0,
        ));
        let mut tx = block.transaction();
        tx.current_dataspace_id = Some(own);
        tx.world.current_dataspace_id = Some(own);
        bind_artifact_test_signed_root(&mut tx, &authority);
        let mut host = CoreHost::new(authority.clone());
        let mut vm = ivm::IVM::new(1_000);
        let pointer = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        vm.set_register(10, pointer);
        host.syscall(ivm::syscalls::SYSCALL_REGISTER_ACCOUNT, &mut vm)
            .expect("VM produces one canonical native instruction");
        assert_eq!(host.queued.len(), 1);
        let error = host
            .apply_queued(&mut tx, &authority)
            .expect_err("generated effects require a reviewed private scope owner");
        assert!(
            error
                .to_string()
                .contains("reviewed private-root scope owner"),
            "{error:?}"
        );
        assert!(tx.world.accounts.get(&account).is_none());
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_asset_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let asset_def: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
        let payload = norito_blob(&asset_def);
        let expected_asset_def: AssetDefinitionId =
            norito::decode_from_bytes(&payload).expect("decode canonical asset definition id");
        let ptr = store_tlv(&mut vm, PointerType::AssetDefinitionId, &payload);
        vm.set_register(10, ptr);
        let name = store_tlv(&mut vm, PointerType::Blob, b"Rose");
        vm.set_register(11, name);
        let spec = iroha_primitives::numeric::NumericSpec::fractional(2);
        let mintable = iroha_data_model::asset::Mintable::limited_from_u32(3).unwrap();
        vm.set_register(
            12,
            iroha_data_model::asset::definition::ivm_registration::numeric_spec_word(spec),
        );
        vm.set_register(
            13,
            iroha_data_model::asset::definition::ivm_registration::mintable_word(mintable),
        );
        let owning_domain = DomainId::try_new("issuer", "universal").unwrap();
        let domain = store_tlv(&mut vm, PointerType::DomainId, &norito_blob(&owning_domain));
        vm.set_register(14, domain);
        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_ASSET, &mut vm);
        let expected = InstructionBox::from(Register::asset_definition(
            AssetDefinition::new(
                expected_asset_def.clone(),
                "Rose",
                spec,
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(owning_domain),
            )
            .with_mintable(mintable),
        ));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_asset_syscall_requires_the_owning_domain_pointer() {
        let mut vm = ivm::IVM::new(1_000);
        let mut host = CoreHost::new(fixture_account("alice"));
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("issuer", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let id = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset),
        );
        let name = store_tlv(&mut vm, PointerType::Blob, b"Coin");
        vm.set_register(10, id);
        vm.set_register(11, name);
        vm.set_register(12, 0);
        vm.set_register(13, 0);
        vm.set_register(14, 0);
        assert!(
            host.syscall(ivm::syscalls::SYSCALL_REGISTER_ASSET, &mut vm)
                .is_err()
        );
        assert!(host.queued.is_empty());
    }

    #[test]
    fn register_asset_syscall_rejects_malformed_registration_words() {
        let asset_def: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
        for (name, spec_word, mintable_word) in [
            // Scale 29 exceeds the numeric domain.
            (&b"Rose"[..], 30, 0),
            // `Infinitely` tag carrying stray payload bits.
            (&b"Rose"[..], 0, 0b100),
            // `Limited` tag with a zero token budget.
            (&b"Rose"[..], 0, 0b011),
            (&b""[..], 0, 0),
            (&b"ro#se"[..], 0, 0),
            // Display names must be UTF-8.
            (&b"\xff"[..], 0, 0),
        ] {
            let mut vm = ivm::IVM::new(1_000);
            let mut host = CoreHost::new(fixture_account("alice"));
            let ptr = store_tlv(
                &mut vm,
                PointerType::AssetDefinitionId,
                &norito_blob(&asset_def),
            );
            vm.set_register(10, ptr);
            let name = store_tlv(&mut vm, PointerType::Blob, name);
            vm.set_register(11, name);
            vm.set_register(12, spec_word);
            vm.set_register(13, mintable_word);
            assert!(matches!(
                host.syscall(ivm::syscalls::SYSCALL_REGISTER_ASSET, &mut vm),
                Err(ivm::VMError::DecodeError)
            ));
            assert!(host.queued.is_empty());
        }
    }
    #[test]
    fn transfer_scope_requires_dataspace_only_for_restricted_definitions() {
        let dataspace = DataSpaceId::new(7);
        assert_eq!(
            (transfer_scope_for_policy(AssetBalancePolicy::Global, None))
                .expect("expected successful fixture result"),
            AssetBalanceScope::Global
        );
        assert_eq!(
            (transfer_scope_for_policy(AssetBalancePolicy::Global, Some(dataspace)))
                .expect("expected successful fixture result"),
            AssetBalanceScope::Global
        );
        assert_eq!(
            (transfer_scope_for_policy(AssetBalancePolicy::DataspaceRestricted, Some(dataspace)))
                .expect("expected successful fixture result"),
            AssetBalanceScope::Dataspace(dataspace)
        );
        assert!(matches!(
            transfer_scope_for_policy(AssetBalancePolicy::DataspaceRestricted, None),
            Err(ivm::VMError::NoritoInvalid)
        ));
    }
    #[test]
    fn unregister_asset_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let asset_def: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn native_escrow_syscalls_queue_expected_instructions() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let escrow_name = Name::from_str("aitai_offer").expect("fixture escrow name");
        let escrow_id = CoreHost::escrow_id_from_name(&escrow_name);
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let amount = Quantity::from(42_u64);
        let buyer_amount = Quantity::from(25_u64);
        let seller_amount = Quantity::from(17_u64);
        let evidence_hashes = vec![Hash::new("receipt"), Hash::new("judgement")];
        let escrow_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&escrow_name));
        let asset_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito_blob(&asset_definition),
        );
        let amount_ptr = store_quantity(&mut vm, &amount);
        let buyer_amount_ptr = store_quantity(&mut vm, &buyer_amount);
        let seller_amount_ptr = store_quantity(&mut vm, &seller_amount);
        let evidence_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&evidence_hashes),
        );
        let retired_blob_evidence_ptr =
            store_tlv(&mut vm, PointerType::Blob, &norito_blob(&evidence_hashes));
        let assert_queued =
            |host: &mut CoreHost, res: Result<u64, ivm::VMError>, expected: InstructionBox| {
                let expected_gas =
                    crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
                        .expect("completed fixture gas quote");
                assert_eq!(
                    (res).expect("expected successful fixture result"),
                    expected_gas
                );
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
                escrow_id,
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
            InstructionBox::from(AcceptAssetEscrow { escrow_id }),
        );
        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_MARK_PAYMENT_SENT, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(MarkEscrowPaymentSent { escrow_id }),
        );
        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_RELEASE, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(ReleaseAssetEscrow { escrow_id }),
        );
        vm.set_register(10, escrow_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_CANCEL, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(CancelAssetEscrow { escrow_id }),
        );
        vm.set_register(10, escrow_ptr);
        vm.set_register(11, evidence_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE, &mut vm);
        assert_queued(
            &mut host,
            res,
            InstructionBox::from(OpenEscrowDispute {
                escrow_id,
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
        let legacy_amount_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&Numeric::from(42_u64)),
        );
        vm.set_register(10, escrow_ptr);
        vm.set_register(11, asset_ptr);
        vm.set_register(12, legacy_amount_ptr);
        vm.set_register(13, 0);
        assert!(matches!(
            host.syscall(ivm::syscalls::SYSCALL_ESCROW_OPEN_OFFER, &mut vm),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert!(host.queued.is_empty());
        vm.set_register(10, escrow_ptr);
        vm.set_register(11, legacy_amount_ptr);
        vm.set_register(12, seller_amount_ptr);
        vm.set_register(13, 0);
        assert!(matches!(
            host.syscall(ivm::syscalls::SYSCALL_ESCROW_RESOLVE_DISPUTE, &mut vm),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert!(host.queued.is_empty());
        vm.set_register(10, escrow_ptr);
        vm.set_register(11, retired_blob_evidence_ptr);
        assert!(matches!(
            host.syscall(ivm::syscalls::SYSCALL_ESCROW_OPEN_DISPUTE, &mut vm),
            Err(ivm::VMError::NoritoInvalid)
        ));
        assert_eq!(vm.register(11), retired_blob_evidence_ptr);
        assert!(host.queued.is_empty());
    }
    #[test]
    fn register_peer_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let peer_id = checked_peer_id();
        let request = RegisterPeerWithPop::new(peer_id, vec![0xAB, 0xCD]);
        let json = Json::new(request.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(request);
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn register_peer_syscall_accepts_object_peer() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let peer_id = checked_peer_id();
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
        let json = Json::from(&norito::json::Value::Object(request_map.clone()));
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_REGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(RegisterPeerWithPop::new(peer_id, vec![0xAB, 0xCD]));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
        request_map.insert("unknown".to_owned(), norito::json::Value::Null);
        let json = Json::from(&norito::json::Value::Object(request_map));
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);
        assert!(matches!(
            host.syscall(ivm::syscalls::SYSCALL_REGISTER_PEER, &mut vm),
            Err(ivm::VMError::DecodeError)
        ));
        assert_eq!(host.queued.len(), 1);
    }
    #[test]
    fn unregister_peer_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let peer_id = checked_peer_id();
        let json = Json::new(peer_id.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_UNREGISTER_PEER, &mut vm);
        let expected = InstructionBox::from(Unregister::peer(peer_id));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    fn soracloud_read_config_request() -> SoracloudHostRequestEnvelopeV1 {
        SoracloudHostRequestEnvelopeV1 {
            schema_version: SORACLOUD_HOST_REQUEST_VERSION_V1,
            operation: SoracloudHostOperationV1::ReadConfig,
            payload: SoracloudHostRequestPayloadV1::ReadConfig(
                iroha_data_model::soracloud::SoracloudReadConfigRequestV1 {
                    config_name: "runtime".to_owned(),
                },
            ),
        }
    }
    #[test]
    fn soracloud_syscalls_validate_request_then_reject_until_runtime_bound() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let request = soracloud_read_config_request();
        let request_bytes = norito_blob(&request);
        let request_ptr = store_tlv(&mut vm, PointerType::SoracloudRequest, &request_bytes);
        vm.set_register(10, request_ptr);
        let quote = host
            .prepare_syscall(ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG, &vm)
            .expect("quote Soracloud runtime rejection");
        let err = host
            .syscall(ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG, &mut vm)
            .expect_err("Soracloud runtime host is not bound in CoreHostImpl");
        assert!(matches!(
            err.as_unmetered(),
            ivm::VMError::NotImplemented { syscall }
                if *syscall == ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG
        ));
        assert_eq!(
            err.metered_gas(),
            Some(ivm::gas::syscall_byte_gas(
                ivm::gas::G_SORACLOUD,
                request_bytes.len(),
                0,
            ))
        );
        assert_eq!(err.metered_gas(), Some(quote));
        assert!(host.queued.is_empty());
    }
    #[test]
    fn soracloud_syscalls_reject_wrong_pointer_type() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let request = soracloud_read_config_request();
        let request_ptr = store_tlv(&mut vm, PointerType::NoritoBytes, &norito_blob(&request));
        vm.set_register(10, request_ptr);
        assert!(matches!(
            host.prepare_syscall(ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG, &vm),
            Err(ivm::VMError::NoritoInvalid)
        ));
        let err = host
            .syscall(ivm::syscalls::SYSCALL_SORACLOUD_READ_CONFIG, &mut vm)
            .expect_err("wrong pointer type must fail before runtime rejection");
        assert!(matches!(err, ivm::VMError::NoritoInvalid));
        assert!(host.queued.is_empty());
    }
    #[test]
    fn soracloud_syscalls_reject_operation_mismatch() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let request_ptr = store_tlv(
            &mut vm,
            PointerType::SoracloudRequest,
            &norito_blob(&soracloud_read_config_request()),
        );
        vm.set_register(10, request_ptr);
        let err = host
            .syscall(
                ivm::syscalls::SYSCALL_SORACLOUD_READ_SECRET_ENVELOPE,
                &mut vm,
            )
            .expect_err("operation mismatch must fail before runtime rejection");
        assert!(matches!(err, ivm::VMError::DecodeError));
        assert!(host.queued.is_empty());
    }
    #[test]
    fn retired_soracloud_raw_material_syscall_holes_are_unknown() {
        let mut vm = ivm::IVM::new(10_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        for number in 0xC5..=0xC7 {
            assert!(matches!(
                host.syscall(number, &mut vm),
                Err(ivm::VMError::UnknownSyscall(rejected)) if rejected == number
            ));
        }
        assert!(host.queued.is_empty());
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn create_role_syscall_queues_instruction_alias() {
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn add_signatory_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let account: AccountId = fixture_account("bob");
        let public_key = checked_public_key();
        let pk_json = Json::new(public_key.clone());
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let pk_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&pk_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, pk_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_ADD_SIGNATORY, &mut vm);
        let expected = InstructionBox::from(AddSignatory::new(account, public_key));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn remove_signatory_syscall_queues_instruction() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let account: AccountId = fixture_account("bob");
        let public_key = checked_public_key();
        let pk_json = Json::new(public_key.clone());
        let account_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&account));
        let pk_ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&pk_json));
        vm.set_register(10, account_ptr);
        vm.set_register(11, pk_ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_REMOVE_SIGNATORY, &mut vm);
        let expected = InstructionBox::from(RemoveSignatory::new(account, public_key));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn remove_signatory_syscall_accepts_object_public_key() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority);
        let account: AccountId = fixture_account("bob");
        let public_key = checked_public_key();
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn set_account_quorum_syscall_queues_instruction() {
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
            )
            .expect("trigger action fixture satisfies validation invariants"),
        );
        let json = Json::new(trigger.clone());
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
        vm.set_register(10, ptr);
        let res = host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm);
        let expected = InstructionBox::from(Register::trigger(trigger));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn create_trigger_syscall_object_spec_queues_instruction() {
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
        )
        .expect("test data-trigger action satisfies its authority invariant");
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
        .expect("test action reconstructed from a validated specialized action")
        .with_metadata(action.metadata.clone());
        let expected =
            InstructionBox::from(Register::trigger(Trigger::new(trigger_id, expected_action)));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn create_trigger_syscall_object_spec_defaults_metadata_and_filter() {
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
        )
        .expect("test data-trigger action satisfies its authority invariant");
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
        .expect("test action reconstructed from a validated specialized action")
        .with_metadata(action.metadata.clone());
        let expected =
            InstructionBox::from(Register::trigger(Trigger::new(trigger_id, expected_action)));
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn create_trigger_syscall_rejects_mismatched_filter_authority() {
        let mut vm = ivm::IVM::new(1_000);
        let action_authority = ALICE_ID.clone();
        let filter_authority = BOB_ID.clone();
        let mut host = CoreHost::new(action_authority.clone());
        let action = SpecializedAction::new(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "must not be queued".to_owned(),
            ))],
            Repeats::Exactly(1),
            filter_authority.clone(),
            EventFilterBox::ExecuteTrigger(
                ExecuteTriggerEventFilter::new().under_authority(filter_authority),
            ),
        )
        .expect("test by-call action initially has matching filter and action authorities");
        let mut action_value =
            norito::json::to_value(&action).expect("serialize specialized action");
        match &mut action_value {
            norito::json::Value::Object(map) => {
                map.insert(
                    "authority".to_owned(),
                    norito::json::to_value(&action_authority)
                        .expect("serialize mismatched action authority"),
                );
            }
            _ => panic!("expected action object"),
        }
        let trigger_id: TriggerId = "trigger_authority_mismatch".parse().unwrap();
        let mut map = BTreeMap::new();
        map.insert(
            "id".to_owned(),
            norito::json::Value::String(trigger_id.to_string()),
        );
        map.insert("action".to_owned(), action_value);
        let spec = Json::from(&norito::json::Value::Object(map));
        let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&spec));
        vm.set_register(10, ptr);
        let result = host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm);
        assert!(matches!(result, Err(ivm::VMError::DecodeError)));
        assert!(
            host.queued.is_empty(),
            "an invalid trigger action must not leave queued effects"
        );
    }
    #[test]
    fn create_trigger_syscall_rejects_sccp_outbound_record_before_queueing() {
        // specs/sccp.md §4.4: a contract-registered trigger is an opaque deferred
        // executable, so it can carry `RecordSccpMessage` neither directly nor nested in a
        // batch, a registered trigger or a multisig proposal.
        let authority = ALICE_ID.clone();
        let record = || {
            InstructionBox::from(
                crate::smartcontracts::isi::sccp::test_support::SampleInstructions::record(),
            )
        };
        let action = |executable: Executable| {
            Action::new(
                executable,
                Repeats::Exactly(1),
                authority.clone(),
                DataEventFilter::Any,
            )
            .expect("trigger action fixture satisfies validation invariants")
        };
        let inner = Trigger::new(
            "inner_record".parse().unwrap(),
            action(Executable::Instructions(vec![record()].into())),
        );
        let executables = [
            Executable::Instructions(vec![record()].into()),
            Executable::Batch(
                vec![
                    iroha_data_model::transaction::ExecutableBatchItem::Instruction(
                        InstructionBox::from(Log::new(Level::INFO, "first".to_owned())),
                    ),
                    iroha_data_model::transaction::ExecutableBatchItem::Instruction(record()),
                ]
                .into(),
            ),
            Executable::Instructions(vec![InstructionBox::from(Register::trigger(inner))].into()),
            Executable::Instructions(
                vec![InstructionBox::from(
                    iroha_executor_data_model::isi::multisig::MultisigPropose::new(
                        authority.clone(),
                        iroha_executor_data_model::isi::multisig::MultisigProposalNonce::new(
                            [0x61; 32],
                        )
                        .unwrap(),
                        vec![record()],
                        None,
                    ),
                )]
                .into(),
            ),
        ];
        for (index, executable) in executables.into_iter().enumerate() {
            let trigger = Trigger::new(
                format!("record_trigger_{index}").parse().unwrap(),
                action(executable),
            );
            let json = Json::new(trigger);
            for contract_frame in [false, true] {
                let mut host = if contract_frame {
                    local_contract_host(authority.clone())
                } else {
                    CoreHost::new(authority.clone())
                };
                let mut vm = ivm::IVM::new(1_000);
                let ptr = store_tlv(&mut vm, PointerType::Json, &norito_blob(&json));
                vm.set_register(10, ptr);
                assert!(
                    matches!(
                        host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm),
                        Err(ivm::VMError::PermissionDenied)
                    ),
                    "executable {index} (contract frame: {contract_frame})"
                );
                assert!(
                    host.queued.is_empty(),
                    "a refused trigger registration must not leave queued effects"
                );
            }
        }
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
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
        let expected_gas = crate::gas::meter_instruction(&expected, &crate::gas::DEFAULT_ZK_GAS)
            .expect("completed fixture gas quote");
        assert_eq!(
            (res).expect("expected successful fixture result"),
            expected_gas
        );
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn quantity_decoder_requires_nominal_canonical_payloads() {
        let mut vm = IVM::new(10_000);
        let canonical: Quantity = "1.25".parse().expect("canonical quantity");
        let canonical_ptr = store_quantity(&mut vm, &canonical);
        assert_eq!(
            CoreHost::decode_quantity(&vm, canonical_ptr).expect("decode canonical amount"),
            canonical
        );
        let wrong_type_ptr = store_tlv(
            &mut vm,
            PointerType::NoritoBytes,
            &norito_blob(&Numeric::new(125_u32, 2)),
        );
        assert!(matches!(
            CoreHost::decode_quantity(&vm, wrong_type_ptr),
            Err(ivm::VMError::NoritoInvalid)
        ));
        for (mantissa, scale, expected) in [
            (
                &[10_u8][..],
                1_u8,
                iroha_primitives::numeric_abi::NumericAbiError::NonCanonicalDecimal,
            ),
            (
                &[0xff_u8][..],
                0_u8,
                iroha_primitives::numeric_abi::NumericAbiError::NegativeQuantity,
            ),
        ] {
            let mut body = Vec::with_capacity(4 + mantissa.len() + 1);
            body.extend_from_slice(
                &u32::try_from(mantissa.len())
                    .expect("malformed fixture mantissa length fits u32")
                    .to_le_bytes(),
            );
            body.extend_from_slice(mantissa);
            body.push(scale);
            let frame = norito::core::frame_bare_with_header_flags::<QuantityValueV1>(&body, 0)
                .expect("schema-correct malformed quantity frame");
            assert_eq!(QuantityValueV1::decode_frame(&frame), Err(expected));
            let ptr = store_tlv(&mut vm, PointerType::Quantity, &frame);
            assert!(matches!(
                CoreHost::decode_quantity(&vm, ptr),
                Err(ivm::VMError::DecodeError)
            ));
        }
    }
    #[test]
    fn mint_asset_syscall_returns_metered_gas() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        vm.load_program(&ivm::ProgramMetadata::default().encode())
            .expect("load header");
        let account_payload = norito::to_bytes(&authority).expect("encode account");
        let account_tlv = make_tlv(PointerType::AccountId as u16, &account_payload);
        let asset_def: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
        let asset_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
        let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);
        let amount = Quantity::from(5u64);
        let amount_tlv = make_tlv(PointerType::Quantity as u16, &quantity_frame(&amount));
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
        let isi = Mint::asset_quantity(5u64, asset_id);
        let expected = crate::gas::meter_instruction(
            &InstructionBox::from(MintBox::from(isi)),
            &crate::gas::DEFAULT_ZK_GAS,
        )
        .expect("completed fixture gas quote");
        assert_eq!(gas, expected);
    }
    #[test]
    fn nft_mint_uses_contract_effect_authority_as_issuer_and_transfer_source() {
        let caller = fixture_account("alice");
        let owner = fixture_account("bob");
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &caller,
            91,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let contract_subject = contract_address.subject_id();
        assert_ne!(caller, contract_subject);
        let mut host = CoreHost::new(caller.clone());
        host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
            contract_address,
            contract_subject: contract_subject.clone(),
            contract_alias: None,
            entrypoint: "mint_for_owner".to_owned(),
        }));
        let mut vm = IVM::new(10_000);
        let nft_id = NftId::of(
            DomainId::try_new("wonderland", "universal").expect("NFT domain"),
            "contract_issued".parse().expect("NFT name"),
        );
        let nft_ptr = store_tlv(&mut vm, PointerType::NftId, &norito_blob(&nft_id));
        let owner_ptr = store_tlv(&mut vm, PointerType::AccountId, &norito_blob(&owner));
        vm.set_register(10, nft_ptr);
        vm.set_register(11, owner_ptr);
        host.syscall(ivm::syscalls::SYSCALL_NFT_MINT_ASSET, &mut vm)
            .expect("queue contract NFT mint");
        assert_eq!(host.queued.len(), 2);
        let expected_register = InstructionBox::from(RegisterBox::from(Register::nft(Nft::new(
            nft_id.clone(),
            Metadata::default(),
        ))));
        let expected_transfer = InstructionBox::from(TransferBox::from(Transfer::nft(
            contract_subject.clone(),
            nft_id,
            owner,
        )));
        assert_eq!(host.queued[0].instruction(), Some(&expected_register));
        assert_eq!(host.queued[0].authority, contract_subject);
        assert_eq!(host.queued[1].instruction(), Some(&expected_transfer));
        assert_eq!(host.queued[1].authority, host.queued[0].authority);
        assert_ne!(
            host.queued[1].authority, caller,
            "the transaction caller must not become the contract NFT issuer",
        );
    }
    #[test]
    fn mint_asset_syscall_accepts_allocated_heap_quantity_tlv() {
        let mut vm = ivm::IVM::new(1_000);
        let authority: AccountId = fixture_account("alice");
        let mut host = CoreHost::new(authority.clone());
        let account_ptr = store_tlv(
            &mut vm,
            PointerType::AccountId,
            &norito::to_bytes(&authority).expect("encode account"),
        );
        let asset_def = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        );
        let asset_ptr = store_tlv(
            &mut vm,
            PointerType::AssetDefinitionId,
            &norito::to_bytes(&asset_def).expect("encode asset definition"),
        );
        let amount = Quantity::from(5_u64);
        let amount_tlv = make_tlv(PointerType::Quantity as u16, &quantity_frame(&amount));
        let amount_ptr = vm
            .alloc_heap(u64::try_from(amount_tlv.len()).expect("amount TLV length fits u64"))
            .expect("allocate amount TLV in heap");
        vm.store_bytes(amount_ptr, &amount_tlv)
            .expect("store amount TLV in allocated heap");
        vm.set_register(10, account_ptr);
        vm.set_register(11, asset_ptr);
        vm.set_register(12, amount_ptr);
        host.syscall(ivm::syscalls::SYSCALL_MINT_ASSET, &mut vm)
            .expect("mint with heap-backed quantity");
        let asset_id = AssetId::of(asset_def, authority);
        let expected = InstructionBox::from(MintBox::from(Mint::asset_quantity(amount, asset_id)));
        assert_eq!(host.queued, vec![expected]);
    }
    #[test]
    fn mint_asset_syscall_rejects_null_amount_pointer_without_trigger_fallback() {
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
        let asset_def: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
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
        assert!(
            host.syscall(ivm::syscalls::SYSCALL_MINT_ASSET, &mut vm)
                .is_err(),
            "null must never select or decode an out-of-band JSON amount"
        );
        assert!(host.queued.is_empty());
    }
    #[test]
    fn unknown_syscall_is_rejected_by_policy() {
        const UNKNOWN_SYSCALL: u32 = u32::MAX;

        let mut vm = ivm::IVM::new(10_000);
        let mut host = CoreHost::new(fixture_account("alice"));
        // Pick a syscall number outside allowed/known ranges.
        let res = host.syscall(UNKNOWN_SYSCALL, &mut vm);
        assert!(matches!(
            res,
            Err(ivm::VMError::UnknownSyscall(UNKNOWN_SYSCALL))
        ));
    }
    #[test]
    fn tlv_wrong_type_id_is_rejected() {
        // Build a program that calls SYSCALL_NFT_SET_METADATA and pass a TLV with wrong type for key
        let mut vm = IVM::new(1_000_000);
        let owner: AccountId = fixture_account("alice");
        let mut host = CoreHost::with_accounts(owner.clone(), Arc::new(vec![owner.clone()]));
        host.set_local_contract_debug_execution();
        vm.set_host(host);
        // Minimal program: HALT, but we only exercise host.validate_tlv via manual call
        let code = [ivm::encoding::wide::encode_halt().to_le_bytes()].concat();
        let program = build_program(&code, 0);
        vm.load_program(&program).unwrap();
        // Prepare TLVs: nft_id (correct), key (WRONG: Json instead of Name), value (Json)
        let nft_id = NftId::of(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "n1".parse().unwrap(),
        );
        let key: iroha_model_base::name::Name = "k".parse().unwrap();
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
        let key: iroha_model_base::name::Name = "k".parse().unwrap();
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
    let mut program = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length,
        max_cycles: 1_000_000,
        abi_version: 1,
    }
    .encode();
    program.extend_from_slice(code);
    program
}
#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

#[cfg(test)]
mod return_resource_tests;

#[cfg(test)]
mod setup_snapshot_tests;

#[cfg(test)]
mod native_pipa_r_admission_tests {
    use super::*;
    use iroha_test_samples::ALICE_ID;

    #[test]
    fn native_policy_caps_curve_and_exact_circuit_are_independent_of_trace() {
        let mut host = CoreHost::new(ALICE_ID.clone());
        let mut config = crate::state::default_zk_config();
        config.trace.enabled = false;
        config.pipa_r.enabled = true;
        config.pipa_r.max_envelope_bytes = 100;
        config.pipa_r.max_proof_bytes = 2;
        host.set_zk_config(&config);
        let mut envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: BackendTag::NativePipaRPasta,
            circuit_id: "pipa-r/pasta/kaigi-usage-v1".into(),
            vk_hash: [1; 32],
            public_inputs: vec![1],
            proof_bytes: vec![1, 2],
            aux: Vec::new(),
        };
        assert!(matches!(
            host.validate_envelope_header(&envelope, 100),
            Ok(())
        ));
        assert_eq!(
            host.validate_envelope_header(&envelope, 101),
            Err(ivm::host::ERR_ENVELOPE_SIZE)
        );
        let record = VerifyingKeyRecord::new(
            1,
            &envelope.circuit_id,
            BackendTag::NativePipaRPasta,
            "vesta",
            [1; 32],
            [2; 32],
        );
        assert!(matches!(
            host.validate_proof_len(&record, "pipa-r/pasta", &envelope),
            Ok(())
        ));
        envelope.proof_bytes.push(3);
        assert_eq!(
            host.validate_proof_len(&record, "pipa-r/pasta", &envelope),
            Err(ivm::host::ERR_PROOF_LEN)
        );
        assert!(CoreHost::curve_is_allowed("pipa-r/pasta", "vesta"));
        for curve in ["pallas", "pasta", "Vesta", "vesta "] {
            assert!(!CoreHost::curve_is_allowed("pipa-r/pasta", curve));
        }
        assert!(CoreHost::circuit_id_matches(
            "pipa-r/pasta",
            &envelope.circuit_id,
            &envelope.circuit_id
        ));
        for alias in [
            "halo2/pasta/ipa/kaigi-usage-v1",
            "pipa-r/ipa/pasta/kaigi-usage-v1",
            "kaigi-usage-v1",
        ] {
            assert!(!CoreHost::circuit_id_matches("pipa-r/pasta", alias, alias));
        }
        config.pipa_r.enabled = false;
        host.set_zk_config(&config);
        assert_eq!(
            host.validate_envelope_header(&envelope, 100),
            Err(ivm::host::ERR_DISABLED)
        );
        assert!(!host.zk_verify_guardrails().pipa_r_enabled);
    }
}
