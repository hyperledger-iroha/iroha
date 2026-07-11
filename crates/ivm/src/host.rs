//! Host interface trait for handling syscalls, with default dummy implementations.
//!
//! The default host provides a minimal but functional set of syscalls used by
//! the tests. It supports heap allocation and retrieval of private inputs so
//! that zero‑knowledge programs can run without a custom environment.
//!
//! The host also exposes basic hardware feature discovery and proof generation
//! helpers used by some tests.
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, HashSet},
    num::NonZeroU16,
};

use iroha_crypto::{
    Sm2PublicKey, Sm2Signature, Sm3Digest, Sm4Key,
    blake2::{
        Blake2bVar,
        digest::{Update as Blake2Update, VariableOutput},
    },
};
use iroha_data_model::{
    isi::transfer::TransferAssetBatch,
    name::Name,
    nexus::{AxtPolicySnapshot, DataSpaceId},
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec},
};
use norito::decode_from_bytes;
use sha2::{Digest as Sha2Digest, Sha256};
use sha3_hash::{Digest as Sha3Digest, Keccak256, Sha3_256};

use crate::{
    SyscallPolicy,
    axt::{self, AssetHandle, ProofBlob, RemoteSpendIntent, TouchManifest},
    error::VMError,
    gas,
    ivm::IVM,
    memory::Memory,
    parallel::{StateAccessSet, StateKey, StateUpdate},
    pointer_abi::{self, PointerType},
    syscalls,
};

/// Runtime record of logical state touches performed by a host during a transaction.
#[derive(Clone, Default, Debug)]
pub struct AccessLog {
    pub read_keys: HashSet<StateKey>,
    pub write_keys: HashSet<StateKey>,
    /// Concrete durable-state paths observed by the host, including any
    /// contract-instance scope.
    pub durable_read_paths: HashSet<StateKey>,
    /// Whether `durable_read_paths` completely covers every durable read.
    ///
    /// The default is deliberately false so custom hosts and manually built
    /// logs fail closed until they explicitly provide this guarantee.
    pub durable_read_paths_complete: bool,
    pub reg_tags: HashSet<usize>,
    pub state_writes: Vec<StateUpdate>,
}

/// Minimal Halo2 verification config enforced by the default host.
#[derive(Clone, Copy, Debug)]
pub struct ZkHalo2Config {
    pub enabled: bool,
    pub curve: ZkCurve,
    pub backend: ZkHalo2Backend,
    pub max_k: u32,
    pub verifier_budget_ms: u64,
    pub verifier_max_batch: u32,
    pub max_envelope_bytes: usize,
    pub max_proof_bytes: usize,
    pub max_transcript_label_len: usize,
    pub enforce_transcript_label_ascii: bool,
}

impl Default for ZkHalo2Config {
    fn default() -> Self {
        Self {
            enabled: true,
            curve: ZkCurve::Pallas,
            backend: ZkHalo2Backend::Ipa,
            max_k: 18,
            verifier_budget_ms: 250,
            verifier_max_batch: 16,
            max_envelope_bytes: 256 * 1024,
            max_proof_bytes: 192 * 1024,
            max_transcript_label_len: 64,
            enforce_transcript_label_ascii: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkCurve {
    Pallas,
    Pasta,
    Goldilocks,
    Bn254,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkHalo2Backend {
    Ipa,
    Unsupported,
}

pub const ERR_DISABLED: u64 = 1;
pub const ERR_BACKEND: u64 = 2;
pub const ERR_CURVE: u64 = 3;
pub const ERR_K: u64 = 4;
pub const ERR_DECODE: u64 = 5;
pub const ERR_VERIFY: u64 = 6;
pub const ERR_BATCH: u64 = 7;
pub const ERR_ENVELOPE_SIZE: u64 = 8;
pub const ERR_TRANSCRIPT_LABEL: u64 = 9;
pub const ERR_PROOF_LEN: u64 = 10;
pub const ERR_VK_MISSING: u64 = 11;
pub const ERR_VK_MISMATCH: u64 = 12;
pub const ERR_VK_INACTIVE: u64 = 13;
pub const ERR_NAMESPACE: u64 = 14;
pub const ERR_DOMAIN_TAG: u64 = 15;

pub const LABEL_TRANSFER: &str = "zk_verify_transfer/v2";
pub const LABEL_UNSHIELD: &str = "zk_verify_unshield/v2";
pub const LABEL_VOTE_BALLOT: &str = "zk_verify_ballot/v2";
pub const LABEL_VOTE_TALLY: &str = "zk_verify_tally/v2";
pub const LABEL_BATCH: &str = "zk_verify_batch/v2";

const PUBLIC_INPUT_GAS_BASE: u64 = 16;
const PUBLIC_INPUT_GAS_PER_BYTE: u64 = 1;
const COMMIT_OUTPUT_GAS: u64 = 16;
const DEBUG_GAS: u64 = 16;
const GET_PRIVATE_INPUT_GAS: u64 = 16;
const GROW_HEAP_GAS_BASE: u64 = 16;
const GROW_HEAP_GAS_PER_PAGE: u64 = 16;
const GROW_HEAP_PAGE_BYTES: u64 = 4096;
const AXT_GAS_BASE: u64 = 16;
const AXT_GAS_PER_BYTE: u64 = 1;
const HASH_GAS_BASE: u64 = 16;
const HASH_GAS_PER_BYTE: u64 = 1;
const INPUT_PUBLISH_GAS_BASE: u64 = 16;
const INPUT_PUBLISH_GAS_PER_BYTE: u64 = 1;
const MERKLE_PATH_GAS_BASE: u64 = 16;
const MERKLE_PATH_GAS_PER_NODE: u64 = 1;
const MUTATION_GAS: u64 = 16;
const MUTATION_GAS_PER_BYTE: u64 = 1;
const NUMERIC_GAS: u64 = 16;
const NULLIFIER_GAS: u64 = 16;
const POINTER_GAS_BASE: u64 = 16;
const POINTER_GAS_PER_BYTE: u64 = 1;
const SIGNATURE_VERIFY_GAS_BASE: u64 = 64;
const SIGNATURE_VERIFY_GAS_PER_BYTE: u64 = 1;
const SM4_GAS_BASE: u64 = 64;
const SM4_GAS_PER_BYTE: u64 = 1;
/// Minimum gas charged before a host may inspect durable or ledger state.
///
/// Preparation rejects a smaller budget before the host is invoked. Stateful
/// scans then spend one additional gas unit per examined item and stop as soon
/// as the pre-debited reserve is exhausted.
pub const STATE_QUERY_GAS_BASE: u64 = 16;
const SYSVAR_GAS_BASE: u64 = 16;
const SYSVAR_GAS_PER_BYTE: u64 = 1;
const TLV_EQ_GAS_BASE: u64 = 16;
const TLV_EQ_GAS_PER_BYTE: u64 = 1;
const TLV_LEN_GAS_BASE: u64 = 16;
const TLV_LEN_GAS_PER_BYTE: u64 = 1;
const VERIFY_GAS_BASE: u64 = 64;
const VERIFY_GAS_PER_BYTE: u64 = 1;

pub(crate) fn debug_log_gas(payload_len: usize) -> u64 {
    DEBUG_GAS.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
}
const TLV_ENVELOPE_OVERHEAD: usize = 7 + iroha_crypto::Hash::LENGTH;

/// Build the injective V1 durable-map path for canonical Norito key bytes.
pub(crate) fn canonical_state_map_path(base: &Name, key: &[u8]) -> Result<Name, VMError> {
    if base.as_ref().len() > syscalls::STATE_MAP_MAX_BASE_BYTES
        || key.is_empty()
        || key.len() > syscalls::STATE_MAP_MAX_KEY_BYTES
    {
        return Err(VMError::NoritoInvalid);
    }
    let base = base.as_ref();
    let mut path = String::with_capacity(base.len() + 1 + key.len().saturating_mul(2));
    path.push_str(base);
    path.push('/');
    path.push_str(&hex::encode(key));
    path.parse().map_err(|_| VMError::NoritoInvalid)
}

/// Decode one canonical key from a `STATE_KEYS` page and validate its map path.
pub(crate) fn canonical_state_map_key_at(
    page_payload: &[u8],
    base: &Name,
    index: u64,
) -> Result<Option<Vec<u8>>, VMError> {
    if page_payload.len() > syscalls::STATE_MAP_MAX_PAGE_BYTES
        || base.as_ref().len() > syscalls::STATE_MAP_MAX_BASE_BYTES
    {
        return Err(VMError::NoritoInvalid);
    }
    let paths: Vec<Name> = decode_from_bytes(page_payload).map_err(|_| VMError::DecodeError)?;
    if paths.len() > usize::try_from(syscalls::STATE_KEYS_MAX_ITEMS).unwrap_or(usize::MAX) {
        return Err(VMError::NoritoInvalid);
    }
    let index = usize::try_from(index).unwrap_or(usize::MAX);
    let Some(path) = paths.get(index) else {
        return Ok(None);
    };
    let suffix = path
        .as_ref()
        .strip_prefix(base.as_ref())
        .and_then(|suffix| suffix.strip_prefix('/'))
        .ok_or(VMError::NoritoInvalid)?;
    if suffix.is_empty()
        || suffix.len() % 2 != 0
        || suffix.contains('/')
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(VMError::NoritoInvalid);
    }
    let key = hex::decode(suffix).map_err(|_| VMError::NoritoInvalid)?;
    if key.len() > syscalls::STATE_MAP_MAX_KEY_BYTES
        || canonical_state_map_path(base, &key)?.as_ref() != path.as_ref()
    {
        return Err(VMError::NoritoInvalid);
    }
    Ok(Some(key))
}

/// Validate the hard V1 page bound and return its platform-sized limit.
pub(crate) fn checked_state_keys_limit(limit: u64) -> Result<usize, VMError> {
    if limit > syscalls::STATE_KEYS_MAX_ITEMS {
        return Err(VMError::NoritoInvalid);
    }
    usize::try_from(limit).map_err(|_| VMError::NoritoInvalid)
}

/// Inspect a TLV header for gas quoting without decoding, hashing, or allocating.
///
/// # Errors
///
/// Returns an error when the envelope header, bounds, pointer type, or ABI policy is invalid.
pub fn quote_any_tlv_at(vm: &IVM, address: u64) -> Result<(PointerType, usize), VMError> {
    vm.ensure_owned_public_tlv_range(address, 7)?;
    let header = vm.memory.inspect_region(address, 7)?;
    let raw_type = u16::from_be_bytes([header[0], header[1]]);
    let pointer_type = PointerType::from_u16(raw_type).ok_or(VMError::NoritoInvalid)?;
    if header[2] != 1 {
        return Err(VMError::NoritoInvalid);
    }
    if !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), pointer_type) {
        return Err(VMError::AbiTypeNotAllowed {
            abi: vm.abi_version(),
            type_id: raw_type,
        });
    }
    let payload_len = u32::from_be_bytes([header[3], header[4], header[5], header[6]]) as usize;
    let total = 7usize
        .checked_add(payload_len)
        .and_then(|len| len.checked_add(iroha_crypto::Hash::LENGTH))
        .ok_or(VMError::NoritoInvalid)?;
    let total = u64::try_from(total).map_err(|_| VMError::NoritoInvalid)?;
    vm.ensure_owned_tlv_range(address, total)?;
    vm.memory.inspect_region(address, total)?;
    Ok((pointer_type, payload_len))
}

/// Inspect a typed TLV header for gas quoting without decoding, hashing, or allocating.
///
/// # Errors
///
/// Returns an error when the envelope is invalid or does not have `expected` pointer type.
pub fn quote_tlv_payload_len_at(
    vm: &IVM,
    address: u64,
    expected: PointerType,
) -> Result<usize, VMError> {
    let (actual, payload_len) = quote_any_tlv_at(vm, address)?;
    if actual != expected {
        return Err(VMError::NoritoInvalid);
    }
    Ok(payload_len)
}

/// Security-relevant host work class used by ABI-v1 syscall metering.
///
/// This is deliberately separate from a compiler builtin's source-level gas
/// class. It describes which runtime resource must be affordable before the
/// host performs work or exposes a side effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostSyscallGasClass {
    /// VM-local or immutable-context work with an input/output-derived bound.
    VmLocal,
    /// Guest-memory allocation whose size is known from public registers.
    Allocation,
    /// Contract-owned durable-state read.
    DurableStateRead,
    /// Contract-owned durable-state write.
    DurableStateWrite,
    /// Ledger/world-state query.
    LedgerRead,
    /// Ledger/world-state mutation or queued instruction.
    LedgerWrite,
    /// Nested, opaque, or externally routed work.
    Dynamic,
}

/// Default quote strategy required for a host syscall class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostSyscallQuoteStrategy {
    /// The host derives a deterministic upper bound from public VM inputs.
    InputOutputBounded,
    /// The allocation extent itself determines the exact quote.
    AllocationExtent,
    /// Escrow all currently available gas and enforce bounded post-debit work.
    ReserveAvailable,
}

/// Exhaustive ABI-v1 host metering metadata for one syscall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostSyscallMeteringSpec {
    /// Canonical syscall number.
    pub number: u32,
    /// Security-relevant runtime work class.
    pub gas_class: HostSyscallGasClass,
    /// Fail-closed default quote strategy.
    pub quote_strategy: HostSyscallQuoteStrategy,
    /// Minimum reserve required before host-state work may begin.
    pub minimum_gas: u64,
}

/// Return the registered metering metadata for an allowed syscall.
///
/// The access registry has no fallback. Consequently, adding a number to the
/// allowed ABI surface without classifying it makes preparation return
/// `UnknownSyscall` instead of silently selecting a generic quote.
#[must_use]
pub fn host_syscall_metering_spec(
    policy: SyscallPolicy,
    number: u32,
) -> Option<HostSyscallMeteringSpec> {
    if !syscalls::is_syscall_allowed(policy, number) {
        return None;
    }
    let access = syscalls::registered_syscall_access(number)?;
    let (gas_class, quote_strategy, minimum_gas) = if matches!(
        number,
        syscalls::SYSCALL_ALLOC | syscalls::SYSCALL_GROW_HEAP
    ) {
        (
            HostSyscallGasClass::Allocation,
            HostSyscallQuoteStrategy::AllocationExtent,
            1,
        )
    } else {
        match access {
            syscalls::SyscallAccess::None => (
                HostSyscallGasClass::VmLocal,
                HostSyscallQuoteStrategy::InputOutputBounded,
                0,
            ),
            syscalls::SyscallAccess::StateRead => (
                HostSyscallGasClass::DurableStateRead,
                HostSyscallQuoteStrategy::ReserveAvailable,
                STATE_QUERY_GAS_BASE,
            ),
            syscalls::SyscallAccess::StateWrite => (
                HostSyscallGasClass::DurableStateWrite,
                HostSyscallQuoteStrategy::InputOutputBounded,
                STATE_QUERY_GAS_BASE,
            ),
            syscalls::SyscallAccess::LedgerRead => (
                HostSyscallGasClass::LedgerRead,
                HostSyscallQuoteStrategy::ReserveAvailable,
                STATE_QUERY_GAS_BASE,
            ),
            syscalls::SyscallAccess::LedgerWrite => (
                HostSyscallGasClass::LedgerWrite,
                HostSyscallQuoteStrategy::ReserveAvailable,
                STATE_QUERY_GAS_BASE,
            ),
            syscalls::SyscallAccess::Dynamic => (
                HostSyscallGasClass::Dynamic,
                HostSyscallQuoteStrategy::ReserveAvailable,
                STATE_QUERY_GAS_BASE,
            ),
        }
    };
    Some(HostSyscallMeteringSpec {
        number,
        gas_class,
        quote_strategy,
        minimum_gas,
    })
}

/// Return the complete, sorted ABI-v1 host metering registry.
///
/// An allowed syscall missing explicit access metadata is intentionally absent.
/// Production preparation rejects that number, and the exhaustive release test
/// requires this registry's numbers to equal [`syscalls::abi_syscall_list`].
pub fn abi_v1_host_syscall_metering_registry() -> &'static [HostSyscallMeteringSpec] {
    static REGISTRY: std::sync::OnceLock<Box<[HostSyscallMeteringSpec]>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        syscalls::abi_syscall_list()
            .iter()
            .filter_map(|&number| host_syscall_metering_spec(SyscallPolicy::AbiV1, number))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    })
}

/// Require registered metering metadata for an allowed host syscall.
///
/// # Errors
///
/// Returns [`VMError::UnknownSyscall`] for a disallowed or unclassified number.
pub fn require_host_syscall_metering_spec(
    policy: SyscallPolicy,
    number: u32,
) -> Result<HostSyscallMeteringSpec, VMError> {
    host_syscall_metering_spec(policy, number).ok_or(VMError::UnknownSyscall(number))
}

/// Conservative deterministic gas bound for host implementations whose exact
/// response size is unavailable during syscall preparation.
///
/// The bound scales all pointer-ABI payloads in the syscall argument registers
/// and reserves the complete host-output regions. Hosts should prefer a tighter
/// syscall-specific quote whenever they can compute one without side effects.
#[must_use]
pub fn conservative_syscall_gas_quote(number: u32, vm: &IVM) -> u64 {
    const BASE: u64 = 4_096;
    const INPUT_MULTIPLIER: u64 = 64;
    const RESPONSE_MULTIPLIER: u64 = 4;

    let mut argument_bytes = 0_u64;
    for &register in crate::ivm::syscall_public_input_registers(number) {
        let pointer = vm.register(register);
        if pointer == 0 {
            continue;
        }
        let payload_len = quote_any_tlv_at(vm, pointer)
            .ok()
            .map_or(0, |(_, payload_len)| {
                u64::try_from(payload_len).unwrap_or(u64::MAX)
            });
        argument_bytes = argument_bytes.saturating_add(payload_len);
    }
    let response_bytes = Memory::INPUT_SIZE.saturating_add(Memory::OUTPUT_SIZE);
    BASE.saturating_add(INPUT_MULTIPLIER.saturating_mul(argument_bytes))
        .saturating_add(RESPONSE_MULTIPLIER.saturating_mul(response_bytes))
}

/// Reserve all gas currently available to a syscall whose exact cost depends on host state.
///
/// The host must call [`preflight_reserved_syscall_gas`] with the exact cost after reading its
/// state and before making any guest-visible allocation or mutation. Reserving a non-zero amount
/// also distinguishes VM-dispatched calls from direct host calls in tests and tooling.
pub fn reserve_available_syscall_gas(vm: &IVM) -> Result<u64, VMError> {
    reserve_available_syscall_gas_at_least(vm, 1)
}

/// Reserve all currently available syscall gas after enforcing a minimum.
///
/// The minimum is checked during side-effect-free preparation. This prevents a
/// caller from triggering even a single host-state lookup or scan with a
/// reserve smaller than the class's deterministic base charge.
pub fn reserve_available_syscall_gas_at_least(
    vm: &IVM,
    minimum: u64,
) -> Result<u64, VMError> {
    let available = vm.remaining_gas();
    if available < minimum {
        Err(VMError::OutOfGas)
    } else {
        Ok(available)
    }
}

/// Check an exact state-dependent syscall cost against the gas reserved before host execution.
///
/// Direct host calls have no reserve and remain supported for low-level tests. A VM-dispatched
/// call that cannot cover the exact cost consumes its full reserve and traps before the caller
/// performs guest-visible work.
pub fn preflight_reserved_syscall_gas(vm: &IVM, actual: u64) -> Result<(), VMError> {
    let reserved = vm.syscall_reserved_gas();
    if reserved != 0 && actual > reserved {
        return Err(VMError::metered(reserved, VMError::OutOfGas));
    }
    Ok(())
}

/// Charge the next state-scan item against the pre-debited syscall reserve.
///
/// `examined_before` is the number of items whose host work has already begun.
/// Callers must invoke this before reading or copying the next item. Direct
/// low-level host calls have no reserve and remain unbounded for test tooling;
/// every VM-dispatched call has a non-zero reserve established by preparation.
///
/// # Errors
///
/// Returns a metered [`VMError::OutOfGas`] before the next item is examined when
/// the base charge plus item count would exceed the reserved quote.
pub fn preflight_reserved_state_scan_item(
    vm: &IVM,
    examined_before: usize,
) -> Result<(), VMError> {
    let reserved = vm.syscall_reserved_gas();
    if reserved == 0 {
        return Ok(());
    }
    let examined_after = u64::try_from(examined_before)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let actual = STATE_QUERY_GAS_BASE.saturating_add(examined_after);
    preflight_reserved_syscall_gas(vm, actual)
}

/// Deterministic gas for one contiguous heap allocation.
///
/// Charging by eight-byte ABI words makes bounded collection capacity visible
/// to metering without depending on the host allocator or hardware page size.
#[must_use]
pub(crate) const fn allocation_gas(bytes: u64) -> u64 {
    1_u64.saturating_add(bytes.saturating_add(7) / 8)
}

/// Return whether a syscall belongs to the optional ShangMi family.
#[must_use]
pub(crate) const fn is_sm_syscall(number: u32) -> bool {
    matches!(
        number,
        syscalls::SYSCALL_SM3_HASH
            | syscalls::SYSCALL_SM2_VERIFY
            | syscalls::SYSCALL_SM4_GCM_SEAL
            | syscalls::SYSCALL_SM4_GCM_OPEN
            | syscalls::SYSCALL_SM4_CCM_SEAL
            | syscalls::SYSCALL_SM4_CCM_OPEN
    )
}

/// Compute exact, side-effect-free gas quotes for VM-local helper syscalls
/// shared by the standalone, core, and world-state hosts.
///
/// `None` means the syscall's gas depends on host-owned state or on an output
/// whose encoded size must be estimated by that host.
pub(crate) fn common_syscall_gas_quote(number: u32, vm: &IVM) -> Result<Option<u64>, VMError> {
    let number = syscalls::canonical_helper_syscall(number);
    if let Some(quote) = crate::amount::gas_quote(number, vm)? {
        return Ok(Some(quote));
    }
    let blob_len = |register: usize| -> Result<usize, VMError> {
        quote_tlv_payload_len_at(
            vm,
            DefaultHost::resolve_code_tlv_addr(vm, vm.register(register)),
            PointerType::Blob,
        )
    };

    let quote = match number {
        syscalls::SYSCALL_DEBUG_PRINT | syscalls::SYSCALL_EXIT | syscalls::SYSCALL_ABORT => {
            DEBUG_GAS
        }
        syscalls::SYSCALL_DEBUG_LOG => {
            let pointer = vm.register(10);
            if pointer == 0 {
                DEBUG_GAS
            } else {
                let (pointer_type, payload_len) =
                    quote_any_tlv_at(vm, DefaultHost::resolve_code_tlv_addr(vm, pointer))?;
                if !matches!(
                    pointer_type,
                    PointerType::Blob | PointerType::NoritoBytes | PointerType::Json
                ) {
                    return Err(VMError::NoritoInvalid);
                }
                debug_log_gas(payload_len)
            }
        }
        syscalls::SYSCALL_ALLOC => allocation_gas(vm.register(10)),
        syscalls::SYSCALL_POINTER_TO_NORITO => {
            let pointer = vm.register(10);
            if pointer == 0 {
                return Err(VMError::NoritoInvalid);
            }
            let (_, payload_len) =
                quote_any_tlv_at(vm, DefaultHost::resolve_code_tlv_addr(vm, pointer))?;
            DefaultHost::pointer_gas(TLV_ENVELOPE_OVERHEAD.saturating_add(payload_len))
        }
        syscalls::SYSCALL_POINTER_FROM_NORITO => {
            let pointer = vm.register(10);
            if pointer == 0 {
                DefaultHost::pointer_gas(0)
            } else {
                let (pointer_type, payload_len) =
                    quote_any_tlv_at(vm, DefaultHost::resolve_code_tlv_addr(vm, pointer))?;
                if !matches!(pointer_type, PointerType::NoritoBytes | PointerType::Blob) {
                    return Err(VMError::NoritoInvalid);
                }
                DefaultHost::pointer_gas(payload_len)
            }
        }
        syscalls::SYSCALL_TLV_EQ => {
            let left_pointer = vm.register(10);
            let right_pointer = vm.register(11);
            let left_len = if left_pointer == 0 {
                0
            } else {
                quote_any_tlv_at(vm, left_pointer)?.1
            };
            let right_len = if right_pointer == 0 || right_pointer == left_pointer {
                0
            } else {
                quote_any_tlv_at(vm, right_pointer)?.1
            };
            DefaultHost::tlv_eq_gas(left_len, right_len)
        }
        syscalls::SYSCALL_TLV_LEN => {
            let pointer = vm.register(10);
            if pointer == 0 {
                DefaultHost::tlv_len_gas(0)
            } else {
                let (_, payload_len) =
                    quote_any_tlv_at(vm, DefaultHost::resolve_code_tlv_addr(vm, pointer))?;
                DefaultHost::tlv_len_gas(payload_len)
            }
        }
        syscalls::SYSCALL_DECODE_ARGUMENT_RECORD => {
            crate::argument_record::decode_argument_record_gas_quote(vm)?
        }
        syscalls::SYSCALL_STATE_VALUE_ENCODE | syscalls::SYSCALL_STATE_VALUE_DECODE => {
            reserve_available_syscall_gas(vm)?
        }
        syscalls::SYSCALL_NUMERIC_FROM_INT => DefaultHost::numeric_gas(),
        syscalls::SYSCALL_NUMERIC_TO_INT | syscalls::SYSCALL_NUMERIC_NEG => {
            let payload_len = quote_tlv_payload_len_at(
                vm,
                DefaultHost::resolve_code_tlv_addr(vm, vm.register(10)),
                PointerType::NoritoBytes,
            )?;
            DefaultHost::numeric_gas()
                .saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
        }
        syscalls::SYSCALL_NUMERIC_ADD
        | syscalls::SYSCALL_NUMERIC_SUB
        | syscalls::SYSCALL_NUMERIC_MUL
        | syscalls::SYSCALL_NUMERIC_DIV
        | syscalls::SYSCALL_NUMERIC_REM
        | syscalls::SYSCALL_NUMERIC_EQ
        | syscalls::SYSCALL_NUMERIC_NE
        | syscalls::SYSCALL_NUMERIC_LT
        | syscalls::SYSCALL_NUMERIC_LE
        | syscalls::SYSCALL_NUMERIC_GT
        | syscalls::SYSCALL_NUMERIC_GE => {
            let left_len = quote_tlv_payload_len_at(
                vm,
                DefaultHost::resolve_code_tlv_addr(vm, vm.register(10)),
                PointerType::NoritoBytes,
            )?;
            let right_len = quote_tlv_payload_len_at(
                vm,
                DefaultHost::resolve_code_tlv_addr(vm, vm.register(11)),
                PointerType::NoritoBytes,
            )?;
            DefaultHost::numeric_gas().saturating_add(
                u64::try_from(left_len.saturating_add(right_len)).unwrap_or(u64::MAX),
            )
        }
        syscalls::SYSCALL_SM3_HASH
        | syscalls::SYSCALL_SHA256_HASH
        | syscalls::SYSCALL_SHA3_HASH
        | syscalls::SYSCALL_BLAKE2B256_HASH
        | syscalls::SYSCALL_KECCAK256_HASH
        | syscalls::SYSCALL_IROHA_HASH => DefaultHost::hash_syscall_gas(blob_len(10)?),
        syscalls::SYSCALL_SM2_VERIFY => {
            let message_len = blob_len(10)?;
            let signature_len = blob_len(11)?;
            let public_key_len = blob_len(12)?;
            let distid_len = if vm.register(13) == 0 {
                0
            } else {
                blob_len(13)?
            };
            DefaultHost::verify_gas(
                message_len
                    .saturating_add(signature_len)
                    .saturating_add(public_key_len)
                    .saturating_add(distid_len),
            )
        }
        syscalls::SYSCALL_SM4_GCM_SEAL
        | syscalls::SYSCALL_SM4_GCM_OPEN
        | syscalls::SYSCALL_SM4_CCM_SEAL
        | syscalls::SYSCALL_SM4_CCM_OPEN => {
            blob_len(10)?;
            blob_len(11)?;
            let aad_len = if vm.register(12) == 0 {
                0
            } else {
                blob_len(12)?
            };
            DefaultHost::sm4_gas(aad_len, blob_len(13)?)
        }
        syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
            let pointer = vm.register(10);
            if pointer == 0 {
                DefaultHost::input_publish_gas(0)
            } else {
                let (_, payload_len) =
                    quote_any_tlv_at(vm, DefaultHost::resolve_code_tlv_addr(vm, pointer))?;
                DefaultHost::input_publish_gas(TLV_ENVELOPE_OVERHEAD.saturating_add(payload_len))
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(quote))
}

/// Trait for IVM host environment to handle syscalls (SCALL).
pub trait IVMHost {
    /// Return a deterministic upper bound for the syscall's additional gas.
    ///
    /// Preparation must be side-effect-free. The VM debits this quote before
    /// invoking [`Self::syscall`] and refunds the unused portion afterwards.
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError>;

    /// Handle a syscall invoked by the VM. `number` is the syscall ID and the
    /// mutable reference to the VM gives access to registers and memory.
    ///
    /// The handler returns the actual additional gas cost. It must not exceed
    /// the bound returned by [`Self::prepare_syscall`]. Returning an error aborts
    /// execution. Metered errors report their actual cost through
    /// [`VMError::Metered`]; an unmetered error consumes the complete prepared
    /// quote so potentially completed host work is never refunded implicitly.
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError>;

    /// Whether this host accepts `number` for a program running under `policy`.
    ///
    /// This policy check must be side-effect-free because it runs before gas is
    /// reserved for the syscall.
    ///
    /// The default is the canonical public ABI surface. Tooling hosts may
    /// explicitly opt in to host-private syscalls without changing the ABI hash
    /// or weakening production admission.
    fn allows_syscall(&self, policy: SyscallPolicy, number: u32) -> bool {
        syscalls::is_syscall_allowed(policy, number)
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static;

    /// Whether this host is safe to share across worker threads during block execution.
    /// Hosts with internal mutable state should override and return `false` so the VM
    /// falls back to sequential execution.
    fn supports_concurrent_blocks(&self) -> bool {
        false
    }

    /// Hint that a transaction is about to start. Hosts can reset per-tx state here.
    /// Returning an error aborts the transaction before execution begins.
    fn begin_tx(&mut self, _declared: &StateAccessSet) -> Result<(), VMError> {
        Ok(())
    }

    /// Report the actual logical state accesses performed during the last transaction.
    /// Errors propagate to the caller and trigger a host rollback via `restore()`.
    fn finish_tx(&mut self) -> Result<AccessLog, VMError> {
        Ok(AccessLog::default())
    }

    /// Optional: inject external verifying key bytes for a backend label.
    /// Defaults to no-op for hosts that do not support VK injection.
    fn set_external_vk_bytes(&mut self, backend: String, bytes: Vec<u8>) {
        let _ = backend;
        let _ = bytes;
    }

    /// Optional transactional checkpoint. When provided, the VM will restore this snapshot
    /// if a transaction fails during block execution to avoid leaking side effects.
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        None
    }

    /// Attempt to restore from a previously taken checkpoint.
    fn restore(&mut self, _snapshot: &dyn Any) -> bool {
        false
    }

    /// Indicate whether this host reports logical state accesses via `finish_tx`.
    fn access_logging_supported(&self) -> bool {
        false
    }
}

// Compile-time signature guards keep the host lifecycle and syscall quote
// contracts uniform across downstream implementations.
type PrepareSyscallSignatureGuard =
    for<'a, 'b> fn(&'a dyn IVMHost, u32, &'b IVM) -> Result<u64, VMError>;
type AllowsSyscallSignatureGuard = for<'a> fn(&'a dyn IVMHost, SyscallPolicy, u32) -> bool;
type BeginTxSignatureGuard =
    for<'a, 'b> fn(&'a mut dyn IVMHost, &'b StateAccessSet) -> Result<(), VMError>;
type FinishTxSignatureGuard = for<'a> fn(&'a mut dyn IVMHost) -> Result<AccessLog, VMError>;
fn prepare_syscall_signature_guard(
    host: &dyn IVMHost,
    number: u32,
    vm: &IVM,
) -> Result<u64, VMError> {
    IVMHost::prepare_syscall(host, number, vm)
}

fn allows_syscall_signature_guard(host: &dyn IVMHost, policy: SyscallPolicy, number: u32) -> bool {
    IVMHost::allows_syscall(host, policy, number)
}

fn begin_tx_signature_guard(
    host: &mut dyn IVMHost,
    declared: &StateAccessSet,
) -> Result<(), VMError> {
    IVMHost::begin_tx(host, declared)
}

fn finish_tx_signature_guard(host: &mut dyn IVMHost) -> Result<AccessLog, VMError> {
    IVMHost::finish_tx(host)
}

const _: PrepareSyscallSignatureGuard = prepare_syscall_signature_guard;
const _: AllowsSyscallSignatureGuard = allows_syscall_signature_guard;
const _: BeginTxSignatureGuard = begin_tx_signature_guard;
const _: FinishTxSignatureGuard = finish_tx_signature_guard;

/// A basic host implementation used in tests. It supports heap allocation and
/// reading private inputs.
#[derive(Clone)]
pub struct DefaultHost {
    private_inputs: Vec<u64>,
    public_inputs: BTreeMap<Name, Vec<u8>>,
    state: BTreeMap<Name, Vec<u8>>,
    pub_output: Vec<u8>,
    nullifiers: HashSet<u64>,
    zk_cfg: ZkHalo2Config,
    chain_id: Option<Vec<u8>>,
    halo2_external_vks: std::collections::HashMap<String, Vec<u8>>,
    axt_state: Option<axt::HostAxtState>,
    axt_policy: std::sync::Arc<dyn axt::AxtPolicy>,
    fastpq_batch_active: bool,
    fastpq_batch_has_entries: bool,
    sm_enabled: bool,
    current_time_ms: u64,
    current_block_height: u64,
    access_log: AccessLog,
}

impl DefaultHost {
    pub fn new() -> Self {
        DefaultHost {
            private_inputs: Vec::new(),
            public_inputs: BTreeMap::new(),
            state: BTreeMap::new(),
            pub_output: Vec::new(),
            nullifiers: HashSet::new(),
            zk_cfg: ZkHalo2Config::default(),
            chain_id: None,
            halo2_external_vks: std::collections::HashMap::new(),
            axt_state: None,
            axt_policy: std::sync::Arc::new(axt::AllowAllAxtPolicy),
            fastpq_batch_active: false,
            fastpq_batch_has_entries: false,
            sm_enabled: false,
            current_time_ms: 0,
            current_block_height: 0,
            access_log: AccessLog::default(),
        }
    }

    /// Provide private inputs that can later be retrieved via `SYSCALL_GET_PRIVATE_INPUT`.
    pub fn with_private_inputs(inputs: Vec<u64>) -> Self {
        DefaultHost {
            private_inputs: inputs,
            public_inputs: BTreeMap::new(),
            state: BTreeMap::new(),
            pub_output: Vec::new(),
            nullifiers: HashSet::new(),
            zk_cfg: ZkHalo2Config::default(),
            chain_id: None,
            halo2_external_vks: std::collections::HashMap::new(),
            axt_state: None,
            axt_policy: std::sync::Arc::new(axt::AllowAllAxtPolicy),
            fastpq_batch_active: false,
            fastpq_batch_has_entries: false,
            sm_enabled: false,
            current_time_ms: 0,
            current_block_height: 0,
            access_log: AccessLog::default(),
        }
    }

    /// Configure Halo2 verification limits for this host.
    pub fn with_zk_halo2_config(mut self, cfg: ZkHalo2Config) -> Self {
        self.zk_cfg = cfg;
        self
    }

    /// Provide public inputs retrievable via `SYSCALL_GET_PUBLIC_INPUT`.
    pub fn with_public_inputs(mut self, inputs: BTreeMap<Name, Vec<u8>>) -> Self {
        self.public_inputs = inputs;
        self
    }

    /// Replace the public input map used by `SYSCALL_GET_PUBLIC_INPUT`.
    pub fn set_public_inputs(&mut self, inputs: BTreeMap<Name, Vec<u8>>) {
        self.public_inputs = inputs;
    }

    /// Expose the current Halo2 verifier config (for tests/introspection).
    pub fn zk_config(&self) -> ZkHalo2Config {
        self.zk_cfg
    }

    /// Install an AXT policy sourced from a data-model snapshot.
    pub fn with_axt_policy_from_snapshot(mut self, snapshot: &AxtPolicySnapshot) -> Self {
        self.axt_policy = std::sync::Arc::new(axt::SnapshotAxtPolicy::new(snapshot));
        self
    }

    /// Convenience: select ZK curve backend from a string.
    /// Accepts: "toy" | "pasta" | "goldilocks" | "bn254" (case-insensitive). Unknown values are ignored.
    pub fn with_zk_curve_str(mut self, curve: &str) -> Self {
        let c = match curve.to_ascii_lowercase().as_str() {
            "toy" | "toy_p61" | "toy-p61" => ZkCurve::Pallas,
            "pasta" => ZkCurve::Pasta,
            "goldilocks" => ZkCurve::Goldilocks,
            "bn254" | "bn-254" => ZkCurve::Bn254,
            _ => self.zk_cfg.curve,
        };
        self.zk_cfg.curve = c;
        self
    }

    /// Set chain_id used for VRF prehash binding. When set, VRF_VERIFY will
    /// enforce the envelope chain_id equals this value and use it for hashing.
    pub fn with_chain_id(mut self, chain_id: Vec<u8>) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Set maximum supported k (where n = 2^k) for Halo2 IPA verifier.
    pub fn with_max_k(mut self, max_k: u32) -> Self {
        self.zk_cfg.max_k = max_k;
        self
    }

    /// Mutably set chain id without moving the host.
    pub fn set_chain_id_bytes(&mut self, chain_id: Vec<u8>) {
        self.chain_id = Some(chain_id);
    }

    /// Enable or disable SM helper syscalls for this host.
    pub fn with_sm_enabled(mut self, enabled: bool) -> Self {
        self.sm_enabled = enabled;
        self
    }

    /// Toggle SM helper support at runtime.
    pub fn set_sm_enabled(&mut self, enabled: bool) {
        self.sm_enabled = enabled;
    }

    /// Set the deterministic block time returned by time syscalls.
    pub fn set_current_time_ms(&mut self, time_ms: u64) {
        self.current_time_ms = time_ms;
    }

    /// Set the deterministic block height returned by sysvar syscalls.
    pub fn set_current_block_height(&mut self, block_height: u64) {
        self.current_block_height = block_height;
    }

    fn expected_zk_verify_label(number: u32) -> Option<&'static str> {
        match number {
            syscalls::SYSCALL_ZK_VERIFY_TRANSFER => Some(LABEL_TRANSFER),
            syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => Some(LABEL_UNSHIELD),
            syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => Some(LABEL_VOTE_BALLOT),
            syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => Some(LABEL_VOTE_TALLY),
            syscalls::SYSCALL_ZK_VERIFY_BATCH => Some(LABEL_BATCH),
            _ => None,
        }
    }

    fn zk_curve_allowed(&self, curve: iroha_zkp_halo2::ZkCurveId) -> bool {
        match self.zk_cfg.curve {
            ZkCurve::Pallas | ZkCurve::Pasta => matches!(
                curve,
                iroha_zkp_halo2::ZkCurveId::Pallas | iroha_zkp_halo2::ZkCurveId::Pasta
            ),
            ZkCurve::Goldilocks => curve == iroha_zkp_halo2::ZkCurveId::Goldilocks,
            ZkCurve::Bn254 => curve == iroha_zkp_halo2::ZkCurveId::Bn254,
        }
    }

    fn map_zk_open_error(error: &iroha_zkp_halo2::Error) -> u64 {
        match error {
            iroha_zkp_halo2::Error::CurveMismatch { .. } => ERR_CURVE,
            iroha_zkp_halo2::Error::EnvelopeLimitExceeded { limit: "max_k", .. } => ERR_K,
            iroha_zkp_halo2::Error::EnvelopeLimitExceeded {
                limit: "transcript_label_len",
                ..
            } => ERR_TRANSCRIPT_LABEL,
            iroha_zkp_halo2::Error::UnsupportedBackend { .. } => ERR_BACKEND,
            iroha_zkp_halo2::Error::VerificationFailed => ERR_VERIFY,
            _ => ERR_DECODE,
        }
    }

    fn verify_zk_open_envelope(&self, number: u32, payload: &[u8]) -> Result<bool, u64> {
        use iroha_zkp_halo2::{
            OpenVerifyEnvelope, OpenVerifyLimits, Transcript,
            backend::{bn254, pallas},
            norito_helpers::{self as nh, DecodedEnvelope},
        };

        if payload.len() > self.zk_cfg.max_envelope_bytes {
            return Err(ERR_ENVELOPE_SIZE);
        }
        if !self.zk_cfg.enabled {
            return Err(ERR_DISABLED);
        }
        if self.zk_cfg.backend != ZkHalo2Backend::Ipa {
            return Err(ERR_BACKEND);
        }

        let env: OpenVerifyEnvelope = decode_from_bytes(payload).map_err(|_| ERR_DECODE)?;
        if env.transcript_label.len() > self.zk_cfg.max_transcript_label_len {
            return Err(ERR_TRANSCRIPT_LABEL);
        }
        if self.zk_cfg.enforce_transcript_label_ascii && !env.transcript_label.is_ascii() {
            return Err(ERR_TRANSCRIPT_LABEL);
        }
        let expected_label = Self::expected_zk_verify_label(number).ok_or(ERR_DECODE)?;
        if env.transcript_label != expected_label {
            return Err(ERR_TRANSCRIPT_LABEL);
        }

        let proof_bytes = norito::to_bytes(&env.proof).map_err(|_| ERR_DECODE)?;
        if proof_bytes.len() > self.zk_cfg.max_proof_bytes {
            return Err(ERR_PROOF_LEN);
        }

        let curve = iroha_zkp_halo2::ZkCurveId::from_u16(env.params.curve_id);
        if env.params.curve_id != env.public.curve_id || !self.zk_curve_allowed(curve) {
            return Err(ERR_CURVE);
        }

        let decoded = nh::decode_envelope_with_limits(
            &env,
            OpenVerifyLimits {
                max_k: Some(self.zk_cfg.max_k),
                max_transcript_label_len: Some(self.zk_cfg.max_transcript_label_len),
            },
        )
        .map_err(|error| Self::map_zk_open_error(&error))?;

        let mut transcript = Transcript::new(&env.transcript_label);
        let metadata = env.transcript_metadata();
        let result = match decoded {
            DecodedEnvelope::Pallas {
                params,
                proof,
                z,
                t,
                p_g,
            } => pallas::Polynomial::verify_open_with_metadata(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
                metadata,
            ),
            DecodedEnvelope::Bn254 {
                params,
                proof,
                z,
                t,
                p_g,
            } => bn254::Polynomial::verify_open_with_metadata(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
                metadata,
            ),
            #[cfg(feature = "goldilocks_backend")]
            DecodedEnvelope::Goldilocks {
                params,
                proof,
                z,
                t,
                p_g,
            } => iroha_zkp_halo2::backend::goldilocks::Polynomial::verify_open_with_metadata(
                params.as_ref(),
                &mut transcript,
                z,
                p_g,
                t,
                proof.as_ref(),
                metadata,
            ),
            #[cfg(not(feature = "goldilocks_backend"))]
            DecodedEnvelope::Goldilocks => {
                return Err(ERR_BACKEND);
            }
        };

        match result {
            Ok(()) => Ok(true),
            Err(iroha_zkp_halo2::Error::VerificationFailed) => Ok(false),
            Err(error) => Err(Self::map_zk_open_error(&error)),
        }
    }

    fn verify_zk_open_batch(&self, payload: &[u8]) -> Result<(Vec<u8>, Option<u64>), u64> {
        if !self.zk_cfg.enabled {
            return Err(ERR_DISABLED);
        }
        if self.zk_cfg.backend != ZkHalo2Backend::Ipa {
            return Err(ERR_BACKEND);
        }

        let envs: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            decode_from_bytes(payload).map_err(|_| ERR_DECODE)?;
        if envs.is_empty() {
            return Err(ERR_DECODE);
        }
        if u32::try_from(envs.len()).unwrap_or(u32::MAX) > self.zk_cfg.verifier_max_batch {
            return Err(ERR_BATCH);
        }

        let mut statuses = Vec::with_capacity(envs.len());
        let mut first_error = None;
        for env in envs {
            let env_payload = norito::to_bytes(&env).map_err(|_| ERR_DECODE)?;
            match self.verify_zk_open_envelope(syscalls::SYSCALL_ZK_VERIFY_BATCH, &env_payload) {
                Ok(true) => statuses.push(1),
                Ok(false) => {
                    first_error.get_or_insert(ERR_VERIFY);
                    statuses.push(0);
                }
                Err(status) => {
                    first_error.get_or_insert(status);
                    statuses.push(0);
                }
            }
        }

        Ok((statuses, first_error))
    }

    fn begin_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if self.fastpq_batch_active {
            return Err(VMError::metered(
                gas::G_FASTPQ_BATCH,
                VMError::PermissionDenied,
            ));
        }
        self.fastpq_batch_active = true;
        self.fastpq_batch_has_entries = false;
        Ok(gas::G_FASTPQ_BATCH)
    }

    fn push_fastpq_batch_entry(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        Self::expect_tlv(vm, 10, PointerType::AccountId)?;
        Self::expect_tlv(vm, 11, PointerType::AccountId)?;
        Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
        Self::expect_amount(vm, 13)?;
        let gas = Self::mutation_gas(0);
        preflight_reserved_syscall_gas(vm, gas)?;
        self.fastpq_batch_has_entries = true;
        Ok(gas)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::metered(
                gas::G_FASTPQ_BATCH,
                VMError::PermissionDenied,
            ));
        }
        self.fastpq_batch_active = false;
        if !self.fastpq_batch_has_entries {
            return Err(VMError::metered(gas::G_FASTPQ_BATCH, VMError::DecodeError));
        }
        self.fastpq_batch_has_entries = false;
        Ok(gas::G_FASTPQ_BATCH)
    }

    fn apply_fastpq_batch(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        let gas = Self::fastpq_batch_apply_gas_quote(vm)?;
        preflight_reserved_syscall_gas(vm, gas)?;
        Ok(gas)
    }

    fn fastpq_batch_apply_gas_quote(vm: &IVM) -> Result<u64, VMError> {
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        let batch: TransferAssetBatch =
            decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
        if batch.entries().is_empty() {
            return Err(VMError::DecodeError);
        }
        Ok(MUTATION_GAS.saturating_mul(u64::try_from(batch.entries().len()).unwrap_or(u64::MAX)))
    }

    fn unsupported_syscall_error(number: u32) -> VMError {
        if syscalls::abi_syscall_list().binary_search(&number).is_ok() {
            VMError::metered_not_implemented(MUTATION_GAS, number)
        } else {
            VMError::UnknownSyscall(number)
        }
    }

    /// Retrieve and clear the output committed by the last program run.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pub_output)
    }

    /// Check if a nullifier has been recorded.
    pub fn has_nullifier(&self, n: u64) -> bool {
        self.nullifiers.contains(&n)
    }

    /// Validate a TLV pointer in register `reg` has the expected `PointerType`.
    fn expect_tlv<'a>(
        vm: &'a IVM,
        reg: usize,
        ty: PointerType,
    ) -> Result<pointer_abi::Tlv<'a>, VMError> {
        let tlv = Self::decode_any_tlv(vm, vm.register(reg))?;
        if tlv.type_id as u16 != ty as u16 {
            return Err(VMError::NoritoInvalid);
        }
        let policy = vm.syscall_policy();
        if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id as u16,
            });
        }
        Ok(tlv)
    }

    fn expect_amount(vm: &IVM, reg: usize) -> Result<(), VMError> {
        let tlv = Self::expect_tlv(vm, reg, PointerType::Amount)?;
        let amount = decode_from_bytes::<Numeric>(tlv.payload).map_err(|_| VMError::DecodeError)?;
        if norito::to_bytes(&amount).map_err(|_| VMError::DecodeError)? != tlv.payload {
            return Err(VMError::DecodeError);
        }
        amount.validate_amount().map_err(|_| VMError::DecodeError)
    }

    fn resolve_literal_pointer(vm: &IVM, src: usize) -> Option<usize> {
        let address = u64::try_from(src).ok()?;
        vm.is_validated_literal_pointer(address).then_some(src)
    }

    fn resolve_code_tlv_addr(vm: &IVM, addr: u64) -> u64 {
        let input_lo = Memory::INPUT_START;
        let input_hi = Memory::INPUT_START + Memory::INPUT_SIZE;
        if addr >= input_lo && addr < input_hi {
            return addr;
        }
        Self::resolve_literal_pointer(vm, addr as usize)
            .map(|resolved| resolved as u64)
            .unwrap_or(addr)
    }

    fn decode_any_tlv<'a>(vm: &'a IVM, ptr: u64) -> Result<pointer_abi::Tlv<'a>, VMError> {
        let resolved = Self::resolve_code_tlv_addr(vm, ptr);
        if crate::dev_env::decode_trace_enabled() {
            eprintln!("[DefaultHost] decode_any_tlv ptr=0x{ptr:08x} resolved=0x{resolved:08x}");
        }
        vm.validate_tlv(resolved)
    }

    fn alloc_blob_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        use iroha_crypto::Hash;

        let mut out = Vec::with_capacity(7 + payload.len() + 32);
        out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
        out.push(1);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; 32] = Hash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_host_tlv(&out)
    }

    fn alloc_norito_bytes_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        use iroha_crypto::Hash;

        let mut out = Vec::with_capacity(7 + payload.len() + 32);
        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        out.push(1);
        let len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; 32] = Hash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_host_tlv(&out)
    }

    fn blake2b256(payload: &[u8]) -> [u8; 32] {
        let mut digest = [0u8; 32];
        let mut hasher =
            Blake2bVar::new(32).expect("32-byte Blake2b output size must be supported");
        hasher.update(payload);
        hasher
            .finalize_variable(&mut digest)
            .expect("fixed Blake2b output buffer has the requested length");
        digest
    }

    fn hash_syscall_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        HASH_GAS_BASE.saturating_add(HASH_GAS_PER_BYTE.saturating_mul(bytes))
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

    fn pointer_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        POINTER_GAS_BASE.saturating_add(POINTER_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn sm4_gas(aad_len: usize, data_len: usize) -> u64 {
        let bytes = u64::try_from(aad_len)
            .unwrap_or(u64::MAX)
            .saturating_add(u64::try_from(data_len).unwrap_or(u64::MAX));
        SM4_GAS_BASE.saturating_add(SM4_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn verify_gas(input_len: usize) -> u64 {
        let bytes = u64::try_from(input_len).unwrap_or(u64::MAX);
        VERIFY_GAS_BASE.saturating_add(VERIFY_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn sysvar_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        SYSVAR_GAS_BASE.saturating_add(SYSVAR_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn state_query_gas(payload_len: usize) -> u64 {
        STATE_QUERY_GAS_BASE.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
    }

    fn state_keys_gas(returned_count: usize, payload_len: usize) -> u64 {
        Self::state_query_gas(payload_len)
            .saturating_add(u64::try_from(returned_count).unwrap_or(u64::MAX))
    }

    fn state_count_gas(total_count: usize) -> u64 {
        STATE_QUERY_GAS_BASE.saturating_add(u64::try_from(total_count).unwrap_or(u64::MAX))
    }

    fn path_gas(input_len: usize, output_len: usize) -> u64 {
        STATE_QUERY_GAS_BASE
            .saturating_add(u64::try_from(input_len).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(output_len).unwrap_or(u64::MAX))
    }

    fn tlv_eq_gas(left_len: usize, right_len: usize) -> u64 {
        let bytes = u64::try_from(left_len)
            .unwrap_or(u64::MAX)
            .saturating_add(u64::try_from(right_len).unwrap_or(u64::MAX));
        TLV_EQ_GAS_BASE.saturating_add(TLV_EQ_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn tlv_len_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        TLV_LEN_GAS_BASE.saturating_add(TLV_LEN_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn numeric_gas() -> u64 {
        NUMERIC_GAS
    }

    fn input_publish_gas(envelope_len: usize) -> u64 {
        let bytes = u64::try_from(envelope_len).unwrap_or(u64::MAX);
        INPUT_PUBLISH_GAS_BASE.saturating_add(INPUT_PUBLISH_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn grow_heap_gas(additional: u64) -> u64 {
        let pages = if additional == 0 {
            0
        } else {
            additional
                .saturating_add(GROW_HEAP_PAGE_BYTES - 1)
                .saturating_div(GROW_HEAP_PAGE_BYTES)
        };
        GROW_HEAP_GAS_BASE.saturating_add(GROW_HEAP_GAS_PER_PAGE.saturating_mul(pages))
    }

    fn merkle_path_gas(depth: usize) -> u64 {
        let nodes = u64::try_from(depth).unwrap_or(u64::MAX);
        MERKLE_PATH_GAS_BASE.saturating_add(MERKLE_PATH_GAS_PER_NODE.saturating_mul(nodes))
    }

    fn complete_tree_depth(leaf_count: usize) -> usize {
        usize::try_from(usize::BITS - leaf_count.max(1).saturating_sub(1).leading_zeros())
            .unwrap_or(usize::MAX)
    }

    fn memory_merkle_path_depth(vm: &IVM) -> usize {
        let byte_len = vm.memory.stack_top().saturating_add(Memory::STACK_SLOP);
        let leaf_count = (usize::try_from(byte_len).unwrap_or(usize::MAX) / 32).max(1);
        Self::complete_tree_depth(leaf_count)
    }

    fn compact_merkle_depth(full_depth: usize, requested: u64) -> usize {
        if requested == 0 {
            full_depth
        } else {
            full_depth.min(usize::try_from(requested).unwrap_or(usize::MAX).min(32))
        }
    }

    fn execution_proof_gas_quote(vm: &IVM) -> Result<u64, VMError> {
        let payload_len = crate::execution_proof::ExecutionProof::encoded_len_v1()
            .map_err(|_| VMError::NoritoInvalid)?;
        Ok(128_u64
            .saturating_add(vm.execution_proof_event_count().saturating_mul(2))
            .saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX)))
    }

    fn signature_verify_gas(
        message_len: usize,
        signature_len: usize,
        public_key_len: usize,
    ) -> u64 {
        let bytes = u64::try_from(message_len)
            .unwrap_or(u64::MAX)
            .saturating_add(u64::try_from(signature_len).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(public_key_len).unwrap_or(u64::MAX));
        SIGNATURE_VERIFY_GAS_BASE
            .saturating_add(SIGNATURE_VERIFY_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn decode_signature_inputs(vm: &IVM) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), VMError> {
        let message_tlv = vm.memory.validate_tlv(vm.register(10))?;
        let message = match message_tlv.type_id {
            PointerType::Blob | PointerType::NoritoBytes => message_tlv.payload.to_vec(),
            PointerType::Json => {
                let json: Json =
                    decode_from_bytes(message_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let value =
                    norito::json::parse_value(json.get()).map_err(|_| VMError::DecodeError)?;
                norito::json::to_vec(&value).map_err(|_| VMError::DecodeError)?
            }
            _ => return Err(VMError::NoritoInvalid),
        };
        let decode_blob = |register: usize| -> Result<Vec<u8>, VMError> {
            let tlv = vm.memory.validate_tlv(vm.register(register))?;
            if tlv.type_id != PointerType::Blob {
                return Err(VMError::NoritoInvalid);
            }
            Ok(tlv.payload.to_vec())
        };
        Ok((message, decode_blob(11)?, decode_blob(12)?))
    }

    fn signature_verify_gas_quote(vm: &IVM) -> Result<u64, VMError> {
        let (message_type, message_len) = quote_any_tlv_at(vm, vm.register(10))?;
        if !matches!(
            message_type,
            PointerType::Blob | PointerType::NoritoBytes | PointerType::Json
        ) {
            return Err(VMError::NoritoInvalid);
        }
        let signature_len = quote_tlv_payload_len_at(vm, vm.register(11), PointerType::Blob)?;
        let public_key_len = quote_tlv_payload_len_at(vm, vm.register(12), PointerType::Blob)?;
        Ok(Self::signature_verify_gas(
            message_len,
            signature_len,
            public_key_len,
        ))
    }

    fn mutation_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        MUTATION_GAS.saturating_add(MUTATION_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn decode_name_tlv(vm: &IVM, reg: usize) -> Result<Name, VMError> {
        let tlv = Self::expect_tlv(vm, reg, PointerType::Name)?;
        decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)
    }

    fn state_key_matches_prefix(key: &Name, prefix: &Name) -> bool {
        let key = key.as_ref();
        let prefix = prefix.as_ref();
        key == prefix
            || key
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    fn state_keys_with_prefix(&self, prefix: &Name) -> Vec<Name> {
        self.state
            .keys()
            .filter(|key| Self::state_key_matches_prefix(key, prefix))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn paged_state_keys(keys: &[Name], offset: u64, limit: u64) -> Result<Vec<Name>, VMError> {
        let take = checked_state_keys_limit(limit)?;
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset >= keys.len() {
            return Ok(Vec::new());
        }
        Ok(keys.iter().skip(offset).take(take).cloned().collect())
    }

    /// Enforce the canonical integer domain used by V1 numeric arithmetic.
    ///
    /// `Numeric` can represent signed 512-bit decimals, but Kotodama `u128`
    /// arithmetic must never inherit that larger range merely because the
    /// pointer ABI uses `Numeric` as its transport representation.
    fn ensure_u128_integer(numeric: Numeric) -> Result<Numeric, VMError> {
        if numeric.scale() != 0 || numeric.try_mantissa_u128().is_none() {
            return Err(VMError::AssertionFailed);
        }
        Ok(numeric)
    }

    fn decode_numeric(vm: &IVM, ptr: u64) -> Result<Numeric, VMError> {
        let tlv = Self::decode_any_tlv(vm, ptr)?;
        if tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        let policy = vm.syscall_policy();
        if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id as u16,
            });
        }
        let numeric =
            decode_from_bytes::<Numeric>(tlv.payload).map_err(|_| VMError::DecodeError)?;
        if norito::to_bytes(&numeric).map_err(|_| VMError::DecodeError)? != tlv.payload {
            return Err(VMError::DecodeError);
        }
        Self::ensure_u128_integer(numeric)
    }

    /// Override the default allow-all AXT policy (test/dependency injection).
    pub fn with_axt_policy(mut self, policy: std::sync::Arc<dyn axt::AxtPolicy>) -> Self {
        self.axt_policy = policy;
        self
    }

    fn handle_axt_begin(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let ptr = vm.register(10);
        let tlv = vm.memory.validate_tlv(ptr)?;
        if tlv.type_id != PointerType::AxtDescriptor {
            return Err(VMError::NoritoInvalid);
        }
        let gas = Self::axt_gas(tlv.payload.len());
        let descriptor: axt::AxtDescriptor =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        axt::validate_descriptor(&descriptor)?;
        let binding = axt::compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        Ok(gas)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let ds_ptr = vm.register(10);
        let ds_tlv = vm.memory.validate_tlv(ds_ptr)?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        let mut gas_len = ds_tlv.payload.len();
        let dsid: DataSpaceId =
            norito::decode_from_bytes(ds_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state.expected_dsids().contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        let manifest_ptr = vm.register(11);
        let manifest = if manifest_ptr == 0 {
            TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }
        } else {
            let manifest_tlv = vm.memory.validate_tlv(manifest_ptr)?;
            if manifest_tlv.type_id != PointerType::NoritoBytes {
                return Err(VMError::NoritoInvalid);
            }
            gas_len = gas_len.saturating_add(manifest_tlv.payload.len());
            norito::decode_from_bytes(manifest_tlv.payload).map_err(|_| VMError::NoritoInvalid)?
        };
        let gas = Self::axt_gas(gas_len);
        preflight_reserved_syscall_gas(vm, gas)?;
        self.axt_policy.allow_touch(dsid, &manifest)?;
        state.record_touch(dsid, manifest)?;
        Ok(gas)
    }

    fn handle_axt_verify_ds_proof(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let ds_ptr = vm.register(10);
        let ds_tlv = vm.memory.validate_tlv(ds_ptr)?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        let dsid: DataSpaceId =
            norito::decode_from_bytes(ds_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state.expected_dsids().contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        let proof_ptr = vm.register(11);
        if proof_ptr == 0 {
            let gas = Self::verify_gas(0);
            preflight_reserved_syscall_gas(vm, gas)?;
            state.record_proof(dsid, None, None)?;
            return Ok(gas);
        }
        let proof_tlv = vm.memory.validate_tlv(proof_ptr)?;
        if proof_tlv.type_id != PointerType::ProofBlob {
            return Err(VMError::NoritoInvalid);
        }
        let proof: ProofBlob =
            norito::decode_from_bytes(proof_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        let envelope = norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof.payload)
            .map_err(|_| VMError::NoritoInvalid)?;
        axt::preflight_fastpq_v1_proof_envelope(&envelope, dsid)?;
        // The standalone host does not link the FastPQ verifier. Shape preflight
        // is diagnostic only, so proof-consuming AXT calls fail closed here.
        Err(VMError::PermissionDenied)
    }

    fn validate_axt_handle_proof_binding(
        handle: &AssetHandle,
        dsid: DataSpaceId,
        proof: &ProofBlob,
    ) -> Result<(), VMError> {
        if handle.manifest_view_root.len() != 32 {
            return Err(VMError::NoritoInvalid);
        }
        if handle.manifest_view_root.iter().all(|byte| *byte == 0) {
            return Err(VMError::PermissionDenied);
        }
        let envelope = norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof.payload)
            .map_err(|_| VMError::NoritoInvalid)?;
        axt::preflight_fastpq_v1_proof_envelope(&envelope, dsid)?;
        if handle.manifest_view_root.as_slice() != envelope.manifest_root.as_slice() {
            return Err(VMError::PermissionDenied);
        }
        // The standalone host cannot accept a FastPQ AXT proof without a real verifier.
        Err(VMError::PermissionDenied)
    }

    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let handle_ptr = vm.register(10);
        let handle_tlv = vm.memory.validate_tlv(handle_ptr)?;
        if handle_tlv.type_id != PointerType::AssetHandle {
            return Err(VMError::NoritoInvalid);
        }
        let mut gas_len = handle_tlv.payload.len();
        let handle: AssetHandle =
            norito::decode_from_bytes(handle_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        let Some(binding) = handle.binding_array() else {
            return Err(VMError::NoritoInvalid);
        };
        if binding != state.binding() {
            return Err(VMError::PermissionDenied);
        }

        let op_ptr = vm.register(11);
        let op_tlv = vm.memory.validate_tlv(op_ptr)?;
        if op_tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        gas_len = gas_len.saturating_add(op_tlv.payload.len());
        let intent: RemoteSpendIntent =
            norito::decode_from_bytes(op_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state.expected_dsids().contains(&intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }
        if !state.has_touch(&intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }

        let proof: Option<ProofBlob> = match vm.register(12) {
            0 => None,
            ptr => {
                let proof_tlv = vm.memory.validate_tlv(ptr)?;
                if proof_tlv.type_id != PointerType::ProofBlob {
                    return Err(VMError::NoritoInvalid);
                }
                gas_len = gas_len.saturating_add(proof_tlv.payload.len());
                Some(
                    norito::decode_from_bytes(proof_tlv.payload)
                        .map_err(|_| VMError::NoritoInvalid)?,
                )
            }
        };
        if let Some(proof_blob) = proof
            .as_ref()
            .or_else(|| state.proofs().get(&intent.asset_dsid))
        {
            Self::validate_axt_handle_proof_binding(&handle, intent.asset_dsid, proof_blob)?;
        }
        let resolved_amount = axt::resolve_handle_amount(&intent, proof.as_ref())
            .map_err(axt::HandleAmountResolutionError::to_vm_error)?;
        if resolved_amount.amount > handle.budget.remaining {
            return Err(VMError::PermissionDenied);
        }
        if let Some(per_use) = handle.budget.per_use
            && resolved_amount.amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }

        let usage = axt::HandleUsage {
            handle,
            intent,
            proof,
            amount: resolved_amount.amount,
            amount_commitment: resolved_amount.amount_commitment,
        };
        let gas = Self::axt_gas(gas_len);
        preflight_reserved_syscall_gas(vm, gas)?;
        self.axt_policy.allow_handle(&usage)?;
        state.record_handle(usage)?;
        Ok(gas)
    }

    fn handle_axt_commit(&mut self, vm: &IVM) -> Result<u64, VMError> {
        let gas = self
            .axt_state
            .as_ref()
            .map(Self::axt_commit_gas)
            .ok_or(VMError::PermissionDenied)?;
        preflight_reserved_syscall_gas(vm, gas)?;
        let state = self
            .axt_state
            .take()
            .expect("axt_state checked before gas preflight");
        match Self::validate_axt_commit(&state) {
            Ok(()) => Ok(gas),
            Err(err) => {
                self.axt_state = Some(state);
                Err(err)
            }
        }
    }

    fn validate_axt_commit(state: &axt::HostAxtState) -> Result<(), VMError> {
        state.validate_commit()?;
        for usage in state.handles() {
            let proof = usage
                .proof
                .as_ref()
                .or_else(|| state.proofs().get(&usage.intent.asset_dsid))
                .ok_or(VMError::PermissionDenied)?;
            Self::validate_axt_handle_proof_binding(&usage.handle, usage.intent.asset_dsid, proof)?;
        }
        Ok(())
    }
}

impl Default for DefaultHost {
    fn default() -> Self {
        Self::new()
    }
}

impl IVMHost for DefaultHost {
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError> {
        let number = crate::syscalls::canonical_helper_syscall(number);
        if is_sm_syscall(number) && !self.sm_enabled {
            // Disabled ShangMi helpers fail before doing metered work.
            return Ok(0);
        }
        if let Some(quote) = common_syscall_gas_quote(number, vm)? {
            return Ok(quote);
        }
        if crate::syscalls::is_json_getter_syscall(number) {
            let json = quote_tlv_payload_len_at(
                vm,
                Self::resolve_code_tlv_addr(vm, vm.register(10)),
                PointerType::Json,
            )?;
            let key = quote_tlv_payload_len_at(
                vm,
                Self::resolve_code_tlv_addr(vm, vm.register(11)),
                PointerType::Name,
            )?;
            let maximum_output = usize::try_from(Memory::INPUT_SIZE).unwrap_or(usize::MAX);
            let output = if number == crate::syscalls::SYSCALL_JSON_GET_I64 {
                core::mem::size_of::<i64>()
            } else {
                maximum_output
            }
            .saturating_add(16);
            return Ok(crate::json::typed_getter_gas(
                json.saturating_add(key),
                output,
            ));
        }
        let tlv_len = |register: usize| -> Result<usize, VMError> {
            let pointer = vm.register(register);
            quote_any_tlv_at(vm, pointer).map(|(_, payload_len)| payload_len)
        };
        let quote = match number {
            crate::syscalls::SYSCALL_STATE_MAP_KEY_AT => {
                let page_len =
                    quote_tlv_payload_len_at(vm, vm.register(10), PointerType::NoritoBytes)?;
                let base_len = quote_tlv_payload_len_at(vm, vm.register(11), PointerType::Name)?;
                if page_len > syscalls::STATE_MAP_MAX_PAGE_BYTES
                    || base_len > syscalls::STATE_MAP_MAX_BASE_BYTES
                {
                    return Err(VMError::NoritoInvalid);
                }
                Self::path_gas(
                    page_len.saturating_add(base_len),
                    syscalls::STATE_MAP_MAX_KEY_BYTES,
                )
            }
            crate::syscalls::SYSCALL_CURRENT_TIME_MS
            | crate::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS
            | crate::syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT
            | crate::syscalls::SYSCALL_SYSVAR_AUTHORITY
            | crate::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS
            | crate::syscalls::SYSCALL_SYSVAR_ENTRYPOINT => Self::sysvar_gas(0),
            crate::syscalls::SYSCALL_SYSVAR_CHAIN_ID => {
                Self::sysvar_gas(self.chain_id.as_ref().map_or(0, Vec::len))
            }
            crate::syscalls::SYSCALL_QUERY_EXECUTE_NORITO
            | crate::syscalls::SYSCALL_CORE_QUERY_GET
            | crate::syscalls::SYSCALL_CORE_QUERY_PAGE
            | crate::syscalls::SYSCALL_QUERY_GET_PARAMETER
            | crate::syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST
            | crate::syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE => STATE_QUERY_GAS_BASE,
            crate::syscalls::SYSCALL_JSON_BUILD => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_STATE_GET
            | crate::syscalls::SYSCALL_STATE_LEN
            | crate::syscalls::SYSCALL_STATE_KEYS
            | crate::syscalls::SYSCALL_STATE_COUNT => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_STATE_SET => Self::state_query_gas(tlv_len(11)?),
            crate::syscalls::SYSCALL_STATE_DEL | crate::syscalls::SYSCALL_STATE_HAS => {
                STATE_QUERY_GAS_BASE
            }
            crate::syscalls::SYSCALL_GROW_HEAP => Self::grow_heap_gas(vm.register(10)),
            crate::syscalls::SYSCALL_GET_PRIVATE_INPUT => GET_PRIVATE_INPUT_GAS,
            crate::syscalls::SYSCALL_GET_PUBLIC_INPUT => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_COMMIT_OUTPUT => COMMIT_OUTPUT_GAS,
            crate::syscalls::SYSCALL_USE_NULLIFIER => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_PROVE_EXECUTION => Self::execution_proof_gas_quote(vm)?,
            crate::syscalls::SYSCALL_VERIFY_PROOF => VERIFY_GAS_BASE,
            crate::syscalls::SYSCALL_VERIFY_SIGNATURE => Self::signature_verify_gas_quote(vm)?,
            crate::syscalls::SYSCALL_VRF_VERIFY
            | crate::syscalls::SYSCALL_VRF_VERIFY_BATCH
            | crate::syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | crate::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY
            | crate::syscalls::SYSCALL_ZK_VERIFY_BATCH => Self::verify_gas(tlv_len(10)?),
            crate::syscalls::SYSCALL_ADD_SIGNATORY
            | crate::syscalls::SYSCALL_REMOVE_SIGNATORY
            | crate::syscalls::SYSCALL_SET_ACCOUNT_QUORUM
            | crate::syscalls::SYSCALL_NFT_MINT_ASSET
            | crate::syscalls::SYSCALL_NFT_TRANSFER_ASSET
            | crate::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED
            | crate::syscalls::SYSCALL_NFT_SET_METADATA
            | crate::syscalls::SYSCALL_NFT_BURN_ASSET => Self::mutation_gas(0),
            crate::syscalls::SYSCALL_SET_ACCOUNT_DETAIL => Self::mutation_gas(tlv_len(12)?),
            crate::syscalls::SYSCALL_TRANSFER_V1 => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN
            | crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => gas::G_FASTPQ_BATCH,
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => reserve_available_syscall_gas(vm)?,
            crate::syscalls::SYSCALL_GET_MERKLE_PATH => {
                let address = vm.register(10);
                let max_address = vm.memory.stack_top().saturating_add(Memory::STACK_SLOP);
                if address >= max_address {
                    return Err(VMError::MemoryOutOfBounds);
                }
                Self::merkle_path_gas(Self::memory_merkle_path_depth(vm))
            }
            crate::syscalls::SYSCALL_GET_MERKLE_COMPACT => {
                let address = vm.register(10);
                let max_address = vm.memory.stack_top().saturating_add(Memory::STACK_SLOP);
                if address >= max_address {
                    return Err(VMError::MemoryOutOfBounds);
                }
                let depth =
                    Self::compact_merkle_depth(Self::memory_merkle_path_depth(vm), vm.register(12));
                Self::merkle_path_gas(depth)
            }
            crate::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT => {
                let index =
                    usize::try_from(vm.register(10)).map_err(|_| VMError::RegisterOutOfBounds)?;
                if index >= crate::parallel::REGISTER_COUNT {
                    return Err(VMError::RegisterOutOfBounds);
                }
                let full_depth = Self::complete_tree_depth(crate::parallel::REGISTER_COUNT);
                Self::merkle_path_gas(Self::compact_merkle_depth(full_depth, vm.register(12)))
            }
            crate::syscalls::SYSCALL_ZK_ROOTS_GET | crate::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                let request_len =
                    quote_tlv_payload_len_at(vm, vm.register(10), PointerType::NoritoBytes)?;
                let maximum_response = usize::try_from(Memory::INPUT_SIZE).unwrap_or(usize::MAX);
                Self::state_query_gas(request_len.saturating_add(maximum_response))
            }
            crate::syscalls::SYSCALL_AXT_BEGIN => {
                let payload_len =
                    quote_tlv_payload_len_at(vm, vm.register(10), PointerType::AxtDescriptor)?;
                Self::axt_gas(payload_len)
            }
            crate::syscalls::SYSCALL_AXT_TOUCH
            | crate::syscalls::SYSCALL_AXT_COMMIT
            | crate::syscalls::SYSCALL_VERIFY_DS_PROOF
            | crate::syscalls::SYSCALL_USE_ASSET_HANDLE => reserve_available_syscall_gas(vm)?,
            _ => {
                if syscalls::abi_syscall_list().binary_search(&number).is_ok() {
                    MUTATION_GAS
                } else {
                    return Err(VMError::UnknownSyscall(number));
                }
            }
        };
        Ok(quote)
    }

    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let requested_number = number;
        let number = crate::syscalls::canonical_helper_syscall(number);
        if crate::syscalls::is_amount_syscall(number) {
            return crate::amount::execute(number, vm);
        }
        if crate::syscalls::is_json_getter_syscall(number) {
            let cost =
                crate::json::typed_getter(vm, requested_number, Self::resolve_code_tlv_addr)?;
            return Ok(crate::json::typed_getter_gas(
                cost.input_bytes,
                cost.output_bytes,
            ));
        }
        if number == crate::syscalls::SYSCALL_JSON_BUILD {
            return crate::json::build_json(vm, Self::resolve_code_tlv_addr);
        }
        match number {
            crate::syscalls::SYSCALL_DEBUG_PRINT => {
                let value = vm.register(10);
                if cfg!(any(test, debug_assertions)) {
                    eprintln!("[IVM] debug_print r10={value}");
                }
                Ok(DEBUG_GAS)
            }
            crate::syscalls::SYSCALL_EXIT => {
                let status = vm.register(10);
                vm.request_exit();
                vm.set_register(10, status);
                Ok(DEBUG_GAS)
            }
            crate::syscalls::SYSCALL_ABORT => {
                // Preserve r10 so language-level `require` error codes remain
                // observable in deterministic execution diagnostics.
                vm.request_abort();
                Ok(DEBUG_GAS)
            }
            crate::syscalls::SYSCALL_CURRENT_TIME_MS
            | crate::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS => {
                vm.set_register(10, self.current_time_ms);
                Ok(Self::sysvar_gas(0))
            }
            crate::syscalls::SYSCALL_SYSVAR_CHAIN_ID => {
                let Some(chain_id) = self.chain_id.as_ref() else {
                    vm.set_register(10, 0);
                    return Ok(Self::sysvar_gas(0));
                };
                let ptr = Self::alloc_blob_tlv(vm, chain_id)?;
                vm.set_register(10, ptr);
                Ok(Self::sysvar_gas(chain_id.len()))
            }
            crate::syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT => {
                vm.set_register(10, self.current_block_height);
                Ok(Self::sysvar_gas(0))
            }
            crate::syscalls::SYSCALL_SYSVAR_AUTHORITY => {
                vm.set_register(10, 0);
                Ok(Self::sysvar_gas(0))
            }
            crate::syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS
            | crate::syscalls::SYSCALL_SYSVAR_ENTRYPOINT => {
                vm.set_register(10, 0);
                Ok(Self::sysvar_gas(0))
            }
            crate::syscalls::SYSCALL_QUERY_EXECUTE_NORITO
            | crate::syscalls::SYSCALL_CORE_QUERY_GET
            | crate::syscalls::SYSCALL_CORE_QUERY_PAGE
            | crate::syscalls::SYSCALL_QUERY_GET_PARAMETER
            | crate::syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST
            | crate::syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE => Err(
                VMError::metered_not_implemented(STATE_QUERY_GAS_BASE, number),
            ),
            crate::syscalls::SYSCALL_STATE_GET => {
                let path = Self::decode_name_tlv(vm, 10)?;
                self.access_log.read_keys.insert(path.as_ref().to_string());
                if let Some(value) = self.state.get(&path).cloned() {
                    let gas = Self::state_query_gas(value.len());
                    preflight_reserved_syscall_gas(vm, gas)?;
                    let ptr = Self::alloc_norito_bytes_tlv(vm, &value)?;
                    vm.set_register(10, ptr);
                    Ok(gas)
                } else {
                    preflight_reserved_syscall_gas(vm, STATE_QUERY_GAS_BASE)?;
                    vm.set_register(10, 0);
                    Ok(STATE_QUERY_GAS_BASE)
                }
            }
            crate::syscalls::SYSCALL_STATE_SET => {
                let path = Self::decode_name_tlv(vm, 10)?;
                let value = {
                    let tlv = Self::expect_tlv(vm, 11, PointerType::NoritoBytes)?;
                    tlv.payload.to_vec()
                };
                let value_len = value.len();
                self.access_log.write_keys.insert(path.as_ref().to_string());
                self.state.insert(path, value);
                Ok(Self::state_query_gas(value_len))
            }
            crate::syscalls::SYSCALL_STATE_DEL => {
                let path = Self::decode_name_tlv(vm, 10)?;
                self.access_log.write_keys.insert(path.as_ref().to_string());
                self.state.remove(&path);
                Ok(STATE_QUERY_GAS_BASE)
            }
            crate::syscalls::SYSCALL_STATE_KEYS => {
                let prefix = Self::decode_name_tlv(vm, 10)?;
                self.access_log
                    .read_keys
                    .insert(prefix.as_ref().to_string());
                let keys = self.state_keys_with_prefix(&prefix);
                let selected = Self::paged_state_keys(&keys, vm.register(11), vm.register(12))?;
                let payload = norito::to_bytes(&selected).map_err(|_| VMError::NoritoInvalid)?;
                let gas = Self::state_keys_gas(selected.len(), payload.len());
                preflight_reserved_syscall_gas(vm, gas)?;
                let ptr = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, ptr);
                vm.set_register(11, u64::try_from(keys.len()).unwrap_or(u64::MAX));
                vm.set_register(12, u64::try_from(selected.len()).unwrap_or(u64::MAX));
                Ok(gas)
            }
            crate::syscalls::SYSCALL_STATE_MAP_KEY_AT => {
                let page = Self::decode_any_tlv(vm, vm.register(10))?;
                let base_tlv = Self::decode_any_tlv(vm, vm.register(11))?;
                if page.type_id != PointerType::NoritoBytes || base_tlv.type_id != PointerType::Name
                {
                    return Err(VMError::NoritoInvalid);
                }
                let base: Name =
                    decode_from_bytes(base_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let key = canonical_state_map_key_at(page.payload, &base, vm.register(12))?;
                let gas = Self::path_gas(
                    page.payload.len().saturating_add(base_tlv.payload.len()),
                    key.as_ref().map_or(0, Vec::len),
                );
                if let Some(key) = key {
                    let pointer = Self::alloc_norito_bytes_tlv(vm, &key)?;
                    vm.set_register(10, pointer);
                } else {
                    vm.set_register(10, 0);
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_STATE_HAS => {
                let path = Self::decode_name_tlv(vm, 10)?;
                self.access_log.read_keys.insert(path.as_ref().to_string());
                vm.set_register(10, u64::from(self.state.contains_key(&path)));
                Ok(STATE_QUERY_GAS_BASE)
            }
            crate::syscalls::SYSCALL_STATE_LEN => {
                let path = Self::decode_name_tlv(vm, 10)?;
                self.access_log.read_keys.insert(path.as_ref().to_string());
                if let Some(value) = self.state.get(&path) {
                    let gas = Self::state_query_gas(value.len());
                    preflight_reserved_syscall_gas(vm, gas)?;
                    vm.set_register(10, u64::try_from(value.len()).unwrap_or(u64::MAX));
                    vm.set_register(11, 1);
                    Ok(gas)
                } else {
                    preflight_reserved_syscall_gas(vm, STATE_QUERY_GAS_BASE)?;
                    vm.set_register(10, 0);
                    vm.set_register(11, 0);
                    Ok(STATE_QUERY_GAS_BASE)
                }
            }
            crate::syscalls::SYSCALL_STATE_COUNT => {
                let prefix = Self::decode_name_tlv(vm, 10)?;
                self.access_log
                    .read_keys
                    .insert(prefix.as_ref().to_string());
                let total = self.state_keys_with_prefix(&prefix).len();
                let gas = Self::state_count_gas(total);
                preflight_reserved_syscall_gas(vm, gas)?;
                vm.set_register(10, u64::try_from(total).unwrap_or(u64::MAX));
                Ok(gas)
            }
            crate::syscalls::SYSCALL_POINTER_TO_NORITO => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = Self::decode_any_tlv(vm, ptr)?;
                let mut body =
                    Vec::with_capacity(2 + 1 + 4 + tlv.payload.len() + iroha_crypto::Hash::LENGTH);
                body.extend_from_slice(&tlv.type_id_raw().to_be_bytes());
                body.push(tlv.version);
                let len = u32::try_from(tlv.payload.len()).map_err(|_| VMError::NoritoInvalid)?;
                body.extend_from_slice(&len.to_be_bytes());
                body.extend_from_slice(tlv.payload);
                let hash: [u8; iroha_crypto::Hash::LENGTH] =
                    iroha_crypto::Hash::new(tlv.payload).into();
                body.extend_from_slice(&hash);
                let out = Self::alloc_norito_bytes_tlv(vm, &body)?;
                vm.set_register(10, out);
                Ok(Self::pointer_gas(body.len()))
            }
            crate::syscalls::SYSCALL_POINTER_FROM_NORITO => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::pointer_gas(0));
                }
                let tlv = Self::decode_any_tlv(vm, ptr)?;
                if !matches!(tlv.type_id, PointerType::NoritoBytes | PointerType::Blob) {
                    return Err(VMError::NoritoInvalid);
                }
                let encoded_len = tlv.payload.len();
                let (inner_type, inner_version, inner_payload) = {
                    let inner = pointer_abi::validate_tlv_bytes(tlv.payload)
                        .map_err(|_| VMError::NoritoInvalid)?;
                    (inner.type_id, inner.version, inner.payload.to_vec())
                };
                let expected = vm.register(11) as u16;
                if expected != 0 && expected != inner_type as u16 {
                    return Err(VMError::NoritoInvalid);
                }
                if !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), inner_type) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: inner_type as u16,
                    });
                }
                let mut out = Vec::with_capacity(
                    2 + 1 + 4 + inner_payload.len() + iroha_crypto::Hash::LENGTH,
                );
                out.extend_from_slice(&(inner_type as u16).to_be_bytes());
                out.push(inner_version);
                let len = u32::try_from(inner_payload.len()).map_err(|_| VMError::NoritoInvalid)?;
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(&inner_payload);
                let hash: [u8; iroha_crypto::Hash::LENGTH] =
                    iroha_crypto::Hash::new(&inner_payload).into();
                out.extend_from_slice(&hash);
                let out_ptr = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, out_ptr);
                Ok(Self::pointer_gas(encoded_len))
            }
            crate::syscalls::SYSCALL_TLV_EQ => {
                let ptr1 = vm.register(10);
                let ptr2 = vm.register(11);
                if ptr1 == 0 && ptr2 == 0 {
                    vm.set_register(10, 1);
                    return Ok(Self::tlv_eq_gas(0, 0));
                }
                if ptr1 == 0 {
                    let right_len = vm.validate_tlv(ptr2)?.payload.len();
                    vm.set_register(10, 0);
                    return Ok(Self::tlv_eq_gas(0, right_len));
                }
                let tlv1 = vm.validate_tlv(ptr1)?;
                let left_len = tlv1.payload.len();
                if ptr2 == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::tlv_eq_gas(left_len, 0));
                }
                if ptr1 == ptr2 {
                    vm.set_register(10, 1);
                    return Ok(Self::tlv_eq_gas(left_len, 0));
                }
                let tlv2 = vm.validate_tlv(ptr2)?;
                let right_len = tlv2.payload.len();
                let eq = tlv1.type_id == tlv2.type_id
                    && tlv1.version == tlv2.version
                    && tlv1.payload == tlv2.payload;
                vm.set_register(10, u64::from(eq));
                Ok(Self::tlv_eq_gas(left_len, right_len))
            }
            crate::syscalls::SYSCALL_TLV_LEN => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::tlv_len_gas(0));
                }
                let tlv = Self::decode_any_tlv(vm, ptr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let payload_len = tlv.payload.len();
                vm.set_register(10, u64::try_from(payload_len).unwrap_or(u64::MAX));
                Ok(Self::tlv_len_gas(payload_len))
            }
            crate::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD => {
                crate::argument_record::decode_argument_record(vm)
            }
            crate::syscalls::SYSCALL_STATE_VALUE_ENCODE => crate::state_value::encode_state_value(
                vm,
                crate::core_host::CoreHost::resolve_code_tlv_addr,
            ),
            crate::syscalls::SYSCALL_STATE_VALUE_DECODE => crate::state_value::decode_state_value(
                vm,
                crate::core_host::CoreHost::resolve_code_tlv_addr,
            ),
            crate::syscalls::SYSCALL_DEBUG_LOG => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Ok(DEBUG_GAS);
                }
                let tlv = Self::decode_any_tlv(vm, ptr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                match tlv.type_id {
                    PointerType::Blob | PointerType::NoritoBytes | PointerType::Json => {
                        if cfg!(any(test, debug_assertions)) {
                            let msg = if tlv.type_id == PointerType::Json {
                                decode_from_bytes::<Json>(tlv.payload)
                                    .map(|json| json.to_string())
                                    .unwrap_or_else(|_| {
                                        core::str::from_utf8(tlv.payload)
                                            .unwrap_or("<non-utf8>")
                                            .to_string()
                                    })
                            } else {
                                core::str::from_utf8(tlv.payload)
                                    .unwrap_or("<non-utf8>")
                                    .to_string()
                            };
                            eprintln!("[IVM] {msg}");
                        }
                        Ok(debug_log_gas(tlv.payload.len()))
                    }
                    _ => Err(VMError::NoritoInvalid),
                }
            }
            // Basic pointer‑ABI validations to mirror core host behavior in tests
            crate::syscalls::SYSCALL_ADD_SIGNATORY => {
                // r10=&AccountId, r11=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Json)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_REMOVE_SIGNATORY => {
                // r10=&AccountId, r11=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Json)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_SET_ACCOUNT_QUORUM => {
                // r10=&AccountId, r11=quorum:u64
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                let quorum_raw = vm.register(11);
                let quorum_u16 = u16::try_from(quorum_raw).map_err(|_| VMError::DecodeError)?;
                NonZeroU16::new(quorum_u16).ok_or(VMError::DecodeError)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                // r10=&AccountId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                let value = Self::expect_tlv(vm, 12, PointerType::Json)?;
                Ok(Self::mutation_gas(value.payload.len()))
            }
            crate::syscalls::SYSCALL_NFT_MINT_ASSET => {
                // r10=&NftId, r11=&AccountId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                // r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::NftId)?;
                Self::expect_tlv(vm, 12, PointerType::AccountId)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_TRANSFER_V1 => {
                if self.fastpq_batch_active {
                    self.push_fastpq_batch_entry(vm)
                } else {
                    Err(crate::VMError::PermissionDenied)
                }
            }
            crate::syscalls::SYSCALL_TRANSFER_ASSET_SCOPED => {
                // r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId,
                // r13=&Amount, r14=&DataSpaceId
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
                Self::expect_amount(vm, 13)?;
                Self::expect_tlv(vm, 14, PointerType::DataSpaceId)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch(vm),
            crate::syscalls::SYSCALL_NFT_SET_METADATA => {
                // r10=&NftId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                Self::expect_tlv(vm, 12, PointerType::Json)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_NFT_BURN_ASSET => {
                // r10=&NftId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Ok(Self::mutation_gas(0))
            }
            crate::syscalls::SYSCALL_NUMERIC_FROM_INT => {
                let val = vm.register(10) as i64;
                if val < 0 {
                    return Err(VMError::AssertionFailed);
                }
                let payload =
                    norito::to_bytes(&Numeric::new(val, 0)).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_TO_INT => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let numeric = Self::decode_numeric(vm, ptr)?;
                let value = numeric
                    .try_mantissa_i128()
                    .ok_or(VMError::AssertionFailed)?;
                if value > i64::MAX as i128 {
                    return Err(VMError::AssertionFailed);
                }
                vm.set_register(10, (value as i64) as u64);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_ADD => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let out = Self::ensure_u128_integer(
                    lhs.checked_add(rhs).ok_or(VMError::AssertionFailed)?,
                )?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_SUB => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let out = Self::ensure_u128_integer(
                    lhs.checked_sub(rhs).ok_or(VMError::AssertionFailed)?,
                )?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_MUL => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let out = Self::ensure_u128_integer(
                    lhs.checked_mul(rhs, NumericSpec::unconstrained())
                        .ok_or(VMError::AssertionFailed)?,
                )?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_DIV => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let out = Self::ensure_u128_integer(
                    lhs.checked_div(rhs, NumericSpec::unconstrained())
                        .ok_or(VMError::AssertionFailed)?,
                )?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_REM => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let out = Self::ensure_u128_integer(
                    lhs.checked_rem(rhs, NumericSpec::unconstrained())
                        .ok_or(VMError::AssertionFailed)?,
                )?;
                let payload = norito::to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_NEG => {
                let val = Self::decode_numeric(vm, vm.register(10))?;
                if !val.is_zero() {
                    return Err(VMError::AssertionFailed);
                }
                let payload = norito::to_bytes(&val).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_NUMERIC_EQ
            | crate::syscalls::SYSCALL_NUMERIC_NE
            | crate::syscalls::SYSCALL_NUMERIC_LT
            | crate::syscalls::SYSCALL_NUMERIC_LE
            | crate::syscalls::SYSCALL_NUMERIC_GT
            | crate::syscalls::SYSCALL_NUMERIC_GE => {
                let lhs = Self::decode_numeric(vm, vm.register(10))?;
                let rhs = Self::decode_numeric(vm, vm.register(11))?;
                let cmp = lhs.cmp(&rhs);
                let result = match number {
                    crate::syscalls::SYSCALL_NUMERIC_EQ => cmp == core::cmp::Ordering::Equal,
                    crate::syscalls::SYSCALL_NUMERIC_NE => cmp != core::cmp::Ordering::Equal,
                    crate::syscalls::SYSCALL_NUMERIC_LT => cmp == core::cmp::Ordering::Less,
                    crate::syscalls::SYSCALL_NUMERIC_LE => {
                        cmp == core::cmp::Ordering::Less || cmp == core::cmp::Ordering::Equal
                    }
                    crate::syscalls::SYSCALL_NUMERIC_GT => cmp == core::cmp::Ordering::Greater,
                    crate::syscalls::SYSCALL_NUMERIC_GE => {
                        cmp == core::cmp::Ordering::Greater || cmp == core::cmp::Ordering::Equal
                    }
                    _ => false,
                };
                vm.set_register(10, if result { 1 } else { 0 });
                Ok(Self::numeric_gas())
            }
            crate::syscalls::SYSCALL_ALLOC => {
                // Allocate `x10` bytes on the VM heap and return the pointer in `x10`.
                let size = vm.register(10);
                let addr = vm.alloc_heap(size)?;
                vm.set_register(10, addr);
                Ok(allocation_gas(size))
            }
            crate::syscalls::SYSCALL_VRF_VERIFY => {
                // Envelope-based syscall: r10 = &NoritoBytes(VrfVerifyRequest)
                // Return: r10 = &Blob(32 bytes) on success; r11 = status code (0=ok, >0 = error)
                use crate::vrf::VrfVerifyRequest;

                // Status codes specific to VRF_VERIFY
                const OK: u64 = 0;
                const ERR_TYPE: u64 = 1; // wrong TLV type
                const ERR_DECODE: u64 = 2; // Norito decode error
                const ERR_VARIANT: u64 = 3; // unknown variant
                const ERR_PK: u64 = 4; // bad pk encoding/length
                const ERR_PROOF: u64 = 5; // bad proof encoding/length
                const ERR_VERIFY: u64 = 6; // pairing check failed
                const ERR_OOM: u64 = 7; // allocation failure
                const ERR_CHAIN: u64 = 8; // chain_id mismatch

                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                let gas = Self::verify_gas(tlv.payload.len());
                if tlv.type_id != PointerType::NoritoBytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_TYPE);
                    return Ok(gas);
                }
                let req: VrfVerifyRequest = match norito::decode_from_bytes(tlv.payload) {
                    Ok(v) => v,
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        return Ok(gas);
                    }
                };

                // Prehash input with domain separation; enforce configured chain_id when present
                if let Some(cid) = &self.chain_id
                    && req.chain_id != *cid
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_CHAIN);
                    return Ok(gas);
                }
                let chain_bytes: &[u8] = if let Some(cid) = &self.chain_id {
                    cid
                } else {
                    &req.chain_id
                };
                let mut in_buf = Vec::with_capacity(
                    b"iroha:vrf:v1:input|".len() + chain_bytes.len() + 1 + req.input.len(),
                );
                in_buf.extend_from_slice(b"iroha:vrf:v1:input|");
                in_buf.extend_from_slice(chain_bytes);
                in_buf.push(b'|');
                in_buf.extend_from_slice(&req.input);
                let msg: [u8; 32] = iroha_crypto::Hash::new(&in_buf).into();

                // BLS helpers using blstrs
                use blstrs::{Bls12, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective};
                use group::{Curve, Group as _, prime::PrimeCurveAffine};
                use pairing::{MillerLoopResult as _, MultiMillerLoop as _};

                fn to_g1(bytes: &[u8]) -> Option<G1Affine> {
                    if bytes.len() != 48 {
                        return None;
                    }
                    if bytes.iter().all(|&byte| byte == 0) {
                        return None;
                    }
                    let mut arr = [0u8; 48];
                    arr.copy_from_slice(bytes);
                    let point = G1Affine::from_compressed(&arr).into_option()?;
                    if bool::from(point.is_identity()) {
                        return None;
                    }
                    if point.to_compressed() != arr {
                        None
                    } else {
                        Some(point)
                    }
                }
                fn to_g2(bytes: &[u8]) -> Option<G2Affine> {
                    if bytes.len() != 96 {
                        return None;
                    }
                    if bytes.iter().all(|&byte| byte == 0) {
                        return None;
                    }
                    let mut arr = [0u8; 96];
                    arr.copy_from_slice(bytes);
                    let point = G2Affine::from_compressed(&arr).into_option()?;
                    if bool::from(point.is_identity()) {
                        return None;
                    }
                    if point.to_compressed() != arr {
                        None
                    } else {
                        Some(point)
                    }
                }
                fn hash_to_g2(msg: &[u8]) -> G2Affine {
                    const DST: &[u8] = b"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
                    let mut buf = Vec::with_capacity(msg.len());
                    buf.extend_from_slice(msg);
                    G2Projective::hash_to_curve(&buf, DST, &[]).to_affine()
                }
                fn hash_to_g1(msg: &[u8]) -> G1Affine {
                    const DST: &[u8] = b"BLS12381G1_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
                    let mut buf = Vec::with_capacity(msg.len());
                    buf.extend_from_slice(msg);
                    G1Projective::hash_to_curve(&buf, DST, &[]).to_affine()
                }

                // Verify and produce y
                let ok: bool = match req.variant {
                    // 1 = SigInG2 (Normal): pk in G1 (48), proof in G2 (96)
                    1 => {
                        let Some(pk) = to_g1(&req.pk) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PK);
                            return Ok(gas);
                        };
                        let Some(sig) = to_g2(&req.proof) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PROOF);
                            return Ok(gas);
                        };
                        let h = hash_to_g2(&msg);
                        let terms: [(&G1Affine, &G2Prepared); 2] = [
                            (&G1Affine::generator(), &G2Prepared::from(sig)),
                            (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
                        ];
                        let gt = Bls12::multi_miller_loop(&terms).final_exponentiation();
                        gt.is_identity().into()
                    }
                    // 2 = SigInG1 (Small): pk in G2 (96), proof in G1 (48)
                    2 => {
                        let Some(pk) = to_g2(&req.pk) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PK);
                            return Ok(gas);
                        };
                        let Some(sig) = to_g1(&req.proof) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PROOF);
                            return Ok(gas);
                        };
                        let h = hash_to_g1(&msg);
                        let terms: [(&G1Affine, &G2Prepared); 2] = [
                            (&sig, &G2Prepared::from(G2Affine::generator())),
                            (&(-G1Projective::from(h)).to_affine(), &G2Prepared::from(pk)),
                        ];
                        let gt = Bls12::multi_miller_loop(&terms).final_exponentiation();
                        gt.is_identity().into()
                    }
                    _ => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_VARIANT);
                        return Ok(gas);
                    }
                };

                if !ok {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_VERIFY);
                    return Ok(gas);
                }

                // Derive output y = Hash(b"iroha:vrf:v1:output" || proof)
                let mut out_buf =
                    Vec::with_capacity(b"iroha:vrf:v1:output".len() + req.proof.len());
                out_buf.extend_from_slice(b"iroha:vrf:v1:output");
                out_buf.extend_from_slice(&req.proof);
                let y: [u8; 32] = iroha_crypto::Hash::new(&out_buf).into();

                // Build Blob TLV in INPUT and return pointer
                let mut tlv = Vec::with_capacity(7 + 32 + 32);
                tlv.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                tlv.push(1);
                tlv.extend_from_slice(&(32u32).to_be_bytes());
                tlv.extend_from_slice(&y);
                let h: [u8; 32] = iroha_crypto::Hash::new(y).into();
                tlv.extend_from_slice(&h);
                match vm.alloc_input_tlv(&tlv) {
                    Ok(p) => {
                        vm.set_register(10, p);
                        vm.set_register(11, OK);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_OOM);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_VRF_VERIFY_BATCH => {
                // r10 = &NoritoBytes(VrfVerifyBatchRequest { items: [VrfVerifyRequest] })
                use crate::vrf::VrfVerifyBatchRequest;
                const OK: u64 = 0;
                const ERR_TYPE: u64 = 1;
                const ERR_DECODE: u64 = 2;
                const ERR_VARIANT: u64 = 3;
                const ERR_PK: u64 = 4;
                const ERR_PROOF: u64 = 5;
                const ERR_VERIFY: u64 = 6;
                const ERR_CHAIN: u64 = 8;

                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                let gas = Self::verify_gas(tlv.payload.len());
                if tlv.type_id != PointerType::NoritoBytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_TYPE);
                    return Ok(gas);
                }
                let req: VrfVerifyBatchRequest = match norito::decode_from_bytes(tlv.payload) {
                    Ok(v) => v,
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        return Ok(gas);
                    }
                };
                if req.items.is_empty() {
                    // Return empty outputs vector
                    let body = norito::to_bytes(&Vec::<[u8; 32]>::new())
                        .map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    vm.set_register(11, OK);
                    return Ok(gas);
                }

                // Shared helpers
                use blstrs::{Bls12, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective};
                use group::{Curve, Group as _, prime::PrimeCurveAffine};
                use pairing::{MillerLoopResult as _, MultiMillerLoop as _};
                fn to_g1(bytes: &[u8]) -> Option<G1Affine> {
                    if bytes.len() != 48 {
                        return None;
                    }
                    if bytes.iter().all(|&byte| byte == 0) {
                        return None;
                    }
                    let mut arr = [0u8; 48];
                    arr.copy_from_slice(bytes);
                    let point = G1Affine::from_compressed(&arr).into_option()?;
                    if bool::from(point.is_identity()) {
                        return None;
                    }
                    if point.to_compressed() != arr {
                        None
                    } else {
                        Some(point)
                    }
                }
                fn to_g2(bytes: &[u8]) -> Option<G2Affine> {
                    if bytes.len() != 96 {
                        return None;
                    }
                    if bytes.iter().all(|&byte| byte == 0) {
                        return None;
                    }
                    let mut arr = [0u8; 96];
                    arr.copy_from_slice(bytes);
                    let point = G2Affine::from_compressed(&arr).into_option()?;
                    if bool::from(point.is_identity()) {
                        return None;
                    }
                    if point.to_compressed() != arr {
                        None
                    } else {
                        Some(point)
                    }
                }
                fn hash_to_g2(msg: &[u8]) -> G2Affine {
                    const DST: &[u8] = b"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
                    let mut buf = Vec::with_capacity(msg.len());
                    buf.extend_from_slice(msg);
                    G2Projective::hash_to_curve(&buf, DST, &[]).to_affine()
                }
                fn hash_to_g1(msg: &[u8]) -> G1Affine {
                    const DST: &[u8] = b"BLS12381G1_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";
                    let mut buf = Vec::with_capacity(msg.len());
                    buf.extend_from_slice(msg);
                    G1Projective::hash_to_curve(&buf, DST, &[]).to_affine()
                }

                let mut outputs: Vec<[u8; 32]> = Vec::with_capacity(req.items.len());
                for (idx, it) in req.items.iter().enumerate() {
                    if let Some(cid) = &self.chain_id
                        && it.chain_id != *cid
                    {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_CHAIN);
                        vm.set_register(12, idx as u64);
                        return Ok(gas);
                    }
                    // Prehash with configured chain id (if present)
                    let chain_bytes: &[u8] = if let Some(cid) = &self.chain_id {
                        cid
                    } else {
                        &it.chain_id
                    };
                    let mut in_buf = Vec::with_capacity(
                        b"iroha:vrf:v1:input|".len() + chain_bytes.len() + 1 + it.input.len(),
                    );
                    in_buf.extend_from_slice(b"iroha:vrf:v1:input|");
                    in_buf.extend_from_slice(chain_bytes);
                    in_buf.push(b'|');
                    in_buf.extend_from_slice(&it.input);
                    let msg: [u8; 32] = iroha_crypto::Hash::new(&in_buf).into();
                    let ok: bool = match it.variant {
                        1 => {
                            let Some(pk) = to_g1(&it.pk) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PK);
                                vm.set_register(12, idx as u64);
                                return Ok(gas);
                            };
                            let Some(sig) = to_g2(&it.proof) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PROOF);
                                vm.set_register(12, idx as u64);
                                return Ok(gas);
                            };
                            let h = hash_to_g2(&msg);
                            let terms: [(&G1Affine, &G2Prepared); 2] = [
                                (&G1Affine::generator(), &G2Prepared::from(sig)),
                                (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
                            ];
                            let gt = Bls12::multi_miller_loop(&terms).final_exponentiation();
                            gt.is_identity().into()
                        }
                        2 => {
                            let Some(pk) = to_g2(&it.pk) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PK);
                                vm.set_register(12, idx as u64);
                                return Ok(gas);
                            };
                            let Some(sig) = to_g1(&it.proof) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PROOF);
                                vm.set_register(12, idx as u64);
                                return Ok(gas);
                            };
                            let h = hash_to_g1(&msg);
                            let terms: [(&G1Affine, &G2Prepared); 2] = [
                                (&sig, &G2Prepared::from(G2Affine::generator())),
                                (&(-G1Projective::from(h)).to_affine(), &G2Prepared::from(pk)),
                            ];
                            let gt = Bls12::multi_miller_loop(&terms).final_exponentiation();
                            gt.is_identity().into()
                        }
                        _ => {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_VARIANT);
                            vm.set_register(12, idx as u64);
                            return Ok(gas);
                        }
                    };
                    if !ok {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_VERIFY);
                        vm.set_register(12, idx as u64);
                        return Ok(gas);
                    }
                    // Compute y
                    let mut out_buf =
                        Vec::with_capacity(b"iroha:vrf:v1:output".len() + it.proof.len());
                    out_buf.extend_from_slice(b"iroha:vrf:v1:output");
                    out_buf.extend_from_slice(&it.proof);
                    let y: [u8; 32] = iroha_crypto::Hash::new(&out_buf).into();
                    outputs.push(y);
                }

                // Encode outputs Vec<[u8;32]> as NoritoBytes and return pointer
                let body = norito::to_bytes(&outputs).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                vm.set_register(11, OK);
                Ok(gas)
            }
            crate::syscalls::SYSCALL_GROW_HEAP => {
                // Increase heap limit by `x10` bytes.
                let size = vm.register(10);
                let new_limit = vm.grow_heap(size)?;
                vm.set_register(10, new_limit);
                Ok(Self::grow_heap_gas(size))
            }
            crate::syscalls::SYSCALL_GET_PRIVATE_INPUT => {
                // Load a private input provided by the host. The index is in `x10`.
                let idx = vm.register(10) as usize;
                if let Some(&val) = self.private_inputs.get(idx) {
                    vm.set_register(10, val);
                    Ok(GET_PRIVATE_INPUT_GAS)
                } else {
                    Err(VMError::metered(
                        GET_PRIVATE_INPUT_GAS,
                        VMError::PermissionDenied,
                    ))
                }
            }
            crate::syscalls::SYSCALL_GET_PUBLIC_INPUT => {
                // Load a named public input provided by the host.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let name: Name =
                    norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                let Some(bytes) = self.public_inputs.get(&name) else {
                    return Err(VMError::PermissionDenied);
                };
                let tlv = pointer_abi::validate_tlv_bytes(bytes)?;
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                let gas = PUBLIC_INPUT_GAS_BASE
                    .saturating_add(PUBLIC_INPUT_GAS_PER_BYTE.saturating_mul(len));
                preflight_reserved_syscall_gas(vm, gas)?;
                let dst = vm.alloc_input_tlv(bytes)?;
                vm.set_register(10, dst);
                Ok(gas)
            }
            crate::syscalls::SYSCALL_COMMIT_OUTPUT => {
                // Make the VM's output buffer available to the host.
                self.pub_output = vm.read_output().to_vec();
                Ok(COMMIT_OUTPUT_GAS)
            }
            crate::syscalls::SYSCALL_USE_NULLIFIER => {
                // Record a nullifier and fail if it has already been used.
                let n = vm.register(10);
                if self.nullifiers.contains(&n) {
                    return Err(VMError::NullifierAlreadyUsed);
                }
                preflight_reserved_syscall_gas(vm, NULLIFIER_GAS)?;
                self.nullifiers.insert(n);
                Ok(NULLIFIER_GAS)
            }
            crate::syscalls::SYSCALL_PROVE_EXECUTION => {
                let proof = vm.execution_proof();
                let payload = norito::to_bytes(&proof).map_err(|_| VMError::NoritoInvalid)?;
                let ptr = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, ptr);
                vm.set_register(11, 0);
                let event_count = proof
                    .pc_trace_len
                    .saturating_add(proof.delta_trace_len)
                    .saturating_add(proof.register_trace_len)
                    .saturating_add(proof.constraint_len)
                    .saturating_add(proof.memory_log_len)
                    .saturating_add(proof.register_log_len)
                    .saturating_add(proof.step_log_len);
                let payload_len = u64::try_from(payload.len()).unwrap_or(u64::MAX);
                Ok(128_u64
                    .saturating_add(event_count.saturating_mul(2))
                    .saturating_add(payload_len))
            }
            crate::syscalls::SYSCALL_VERIFY_PROOF => {
                // Execution proof verification is implemented at the node layer (CoreHost),
                // not inside the standalone IVM host.
                Err(VMError::metered_not_implemented(VERIFY_GAS_BASE, number))
            }
            crate::syscalls::SYSCALL_VERIFY_SIGNATURE => {
                // r10 = &message TLV, r11 = &Blob signature TLV, r12 = &Blob public key TLV, r13 = scheme code
                let (msg, sig, pk) = Self::decode_signature_inputs(vm)?;
                let gas = Self::signature_verify_gas(msg.len(), sig.len(), pk.len());
                let scheme_code = vm.register(13) as u8;
                let scheme = match scheme_code {
                    1 => crate::signature::SignatureScheme::Ed25519,
                    2 => crate::signature::SignatureScheme::Secp256k1,
                    3 => crate::signature::SignatureScheme::MlDsa,
                    _ => {
                        vm.set_register(10, 0);
                        return Ok(gas);
                    }
                };
                let ok = crate::signature::verify_signature(scheme, &msg, &sig, &pk);
                vm.set_register(10, if ok { 1 } else { 0 });
                Ok(gas)
            }
            crate::syscalls::SYSCALL_SM3_HASH => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest = Sm3Digest::hash(tlv.payload);
                let bytes = digest.as_bytes();
                let addr = DefaultHost::alloc_blob_tlv(vm, bytes)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_SHA256_HASH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest = <Sha256 as Sha2Digest>::digest(tlv.payload);
                let addr = DefaultHost::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_SHA3_HASH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest = <Sha3_256 as Sha3Digest>::digest(tlv.payload);
                let addr = DefaultHost::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_BLAKE2B256_HASH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest = Self::blake2b256(tlv.payload);
                let addr = DefaultHost::alloc_blob_tlv(vm, &digest)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_KECCAK256_HASH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest = <Keccak256 as Sha3Digest>::digest(tlv.payload);
                let addr = DefaultHost::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_IROHA_HASH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::Blob {
                    return Err(VMError::NoritoInvalid);
                }
                let payload_len = tlv.payload.len();
                let digest: [u8; iroha_crypto::Hash::LENGTH] =
                    iroha_crypto::Hash::new(tlv.payload).into();
                let addr = DefaultHost::alloc_blob_tlv(vm, &digest)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            crate::syscalls::SYSCALL_SM2_VERIFY => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let msg_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let sig_tlv = vm.memory.validate_tlv(vm.register(11))?;
                let pk_tlv = vm.memory.validate_tlv(vm.register(12))?;

                if !matches!(
                    msg_tlv.type_id,
                    PointerType::Blob | PointerType::NoritoBytes
                ) || sig_tlv.type_id != PointerType::Blob
                    || pk_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }

                let distid_ptr = vm.register(13);
                let mut input_len = msg_tlv
                    .payload
                    .len()
                    .saturating_add(sig_tlv.payload.len())
                    .saturating_add(pk_tlv.payload.len());
                let distid = if distid_ptr != 0 {
                    let distid_tlv = vm.memory.validate_tlv(distid_ptr)?;
                    if distid_tlv.type_id != PointerType::Blob {
                        return Err(VMError::NoritoInvalid);
                    }
                    input_len = input_len.saturating_add(distid_tlv.payload.len());
                    std::str::from_utf8(distid_tlv.payload)
                        .map(|s| s.to_owned())
                        .map_err(|_| VMError::NoritoInvalid)?
                } else {
                    Sm2PublicKey::default_distid()
                };
                let gas = Self::verify_gas(input_len);

                let msg = msg_tlv.payload;
                let sig_bytes = sig_tlv.payload;
                if sig_bytes.len() != Sm2Signature::LENGTH {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut sig_buf = [0u8; Sm2Signature::LENGTH];
                sig_buf.copy_from_slice(sig_bytes);
                let signature = match Sm2Signature::from_bytes(&sig_buf) {
                    Ok(sig) => sig,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(gas);
                    }
                };

                let public_key = match Sm2PublicKey::from_sec1_bytes(&distid, pk_tlv.payload) {
                    Ok(pk) => pk,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(gas);
                    }
                };

                let ok = public_key.verify(msg, &signature).is_ok();
                vm.set_register(10, if ok { 1 } else { 0 });
                Ok(gas)
            }
            crate::syscalls::SYSCALL_SM4_GCM_SEAL => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let nonce_tlv = vm.memory.validate_tlv(vm.register(11))?;
                let aad_opt = if vm.register(12) == 0 {
                    None
                } else {
                    Some(vm.memory.validate_tlv(vm.register(12))?)
                };
                let pt_tlv = vm.memory.validate_tlv(vm.register(13))?;

                if key_tlv.type_id != PointerType::Blob
                    || nonce_tlv.type_id != PointerType::Blob
                    || pt_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                if let Some(ref aad_tlv) = aad_opt
                    && aad_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                let aad_len = aad_opt.as_ref().map(|tlv| tlv.payload.len()).unwrap_or(0);
                let gas = Self::sm4_gas(aad_len, pt_tlv.payload.len());

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                if nonce_tlv.payload.len() != 12 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(nonce_tlv.payload);
                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);

                match key.encrypt_gcm(&nonce, aad, pt_tlv.payload) {
                    Ok((cipher, tag)) => {
                        let mut payload = cipher;
                        payload.extend_from_slice(&tag);
                        let addr = DefaultHost::alloc_blob_tlv(vm, &payload)?;
                        vm.set_register(10, addr);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_SM4_GCM_OPEN => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let nonce_tlv = vm.memory.validate_tlv(vm.register(11))?;
                let aad_opt = if vm.register(12) == 0 {
                    None
                } else {
                    Some(vm.memory.validate_tlv(vm.register(12))?)
                };
                let ct_tlv = vm.memory.validate_tlv(vm.register(13))?;

                if key_tlv.type_id != PointerType::Blob
                    || nonce_tlv.type_id != PointerType::Blob
                    || ct_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                if let Some(ref aad_tlv) = aad_opt
                    && aad_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                let aad_len = aad_opt.as_ref().map(|tlv| tlv.payload.len()).unwrap_or(0);
                let gas = Self::sm4_gas(aad_len, ct_tlv.payload.len());

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                if nonce_tlv.payload.len() != 12 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(nonce_tlv.payload);
                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);

                if ct_tlv.payload.len() < 16 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let split = ct_tlv.payload.len() - 16;
                let (cipher_bytes, tag_bytes) = ct_tlv.payload.split_at(split);
                let mut tag = [0u8; 16];
                tag.copy_from_slice(tag_bytes);

                match key.decrypt_gcm(&nonce, aad, cipher_bytes, &tag) {
                    Ok(plaintext) => {
                        let addr = DefaultHost::alloc_blob_tlv(vm, &plaintext)?;
                        vm.set_register(10, addr);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_SM4_CCM_SEAL => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let nonce_tlv = vm.memory.validate_tlv(vm.register(11))?;
                let aad_opt = if vm.register(12) == 0 {
                    None
                } else {
                    Some(vm.memory.validate_tlv(vm.register(12))?)
                };
                let pt_tlv = vm.memory.validate_tlv(vm.register(13))?;
                let tag_len_raw = vm.register(14) as usize;

                if key_tlv.type_id != PointerType::Blob
                    || nonce_tlv.type_id != PointerType::Blob
                    || pt_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                if let Some(ref aad_tlv) = aad_opt
                    && aad_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                let aad_len = aad_opt.as_ref().map(|tlv| tlv.payload.len()).unwrap_or(0);
                let gas = Self::sm4_gas(aad_len, pt_tlv.payload.len());

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);
                let tag_len = if tag_len_raw == 0 { 16 } else { tag_len_raw };

                match key.encrypt_ccm(nonce_tlv.payload, aad, pt_tlv.payload, tag_len) {
                    Ok((mut cipher, tag)) => {
                        cipher.extend_from_slice(&tag);
                        let addr = DefaultHost::alloc_blob_tlv(vm, &cipher)?;
                        vm.set_register(10, addr);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_SM4_CCM_OPEN => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let nonce_tlv = vm.memory.validate_tlv(vm.register(11))?;
                let aad_opt = if vm.register(12) == 0 {
                    None
                } else {
                    Some(vm.memory.validate_tlv(vm.register(12))?)
                };
                let ct_tlv = vm.memory.validate_tlv(vm.register(13))?;
                let tag_len_raw = vm.register(14) as usize;

                if key_tlv.type_id != PointerType::Blob
                    || nonce_tlv.type_id != PointerType::Blob
                    || ct_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                if let Some(ref aad_tlv) = aad_opt
                    && aad_tlv.type_id != PointerType::Blob
                {
                    return Err(VMError::NoritoInvalid);
                }
                let aad_len = aad_opt.as_ref().map(|tlv| tlv.payload.len()).unwrap_or(0);
                let gas = Self::sm4_gas(aad_len, ct_tlv.payload.len());

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);
                let tag_len = if tag_len_raw == 0 { 16 } else { tag_len_raw };

                if ct_tlv.payload.len() < tag_len {
                    vm.set_register(10, 0);
                    return Ok(gas);
                }
                let split = ct_tlv.payload.len() - tag_len;
                let (cipher_bytes, tag_bytes) = ct_tlv.payload.split_at(split);

                match key.decrypt_ccm(nonce_tlv.payload, aad, cipher_bytes, tag_bytes) {
                    Ok(plaintext) => {
                        let addr = DefaultHost::alloc_blob_tlv(vm, &plaintext)?;
                        vm.set_register(10, addr);
                    }
                    Err(_) => {
                        vm.set_register(10, 0);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
                // Validate host-owned public TLVs in place. Immutable code literals are
                // materialized into the host arena so downstream pointer consumers can
                // retain the returned pointer after literal resolution.
                let src = vm.register(10);
                if src == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::input_publish_gas(0));
                }
                if src >= crate::memory::Memory::HEAP_START {
                    let tlv = vm.validate_tlv(src)?;
                    let envelope_len = 7usize.saturating_add(tlv.payload.len()).saturating_add(32);
                    return Ok(Self::input_publish_gas(envelope_len));
                }
                let resolved = Self::resolve_literal_pointer(vm, src as usize)
                    .ok_or(VMError::NoritoInvalid)? as u64;
                let bytes_vec = vm.clone_tlv(resolved)?;
                let total = bytes_vec.len();
                let dst = vm.alloc_host_tlv(&bytes_vec)?;
                vm.set_register(10, dst);
                Ok(Self::input_publish_gas(total))
            }
            crate::syscalls::SYSCALL_GET_MERKLE_PATH => {
                let addr = vm.register(10);
                let max_addr = vm
                    .memory
                    .stack_top()
                    .saturating_add(crate::Memory::STACK_SLOP);
                if addr >= max_addr {
                    return Err(VMError::MemoryOutOfBounds);
                }
                let dest = vm.register(11);
                let root_out = vm.register(12);
                let (root, path) = vm.memory.merkle_root_and_path(addr);
                for (i, node) in path.iter().enumerate() {
                    vm.memory.store_bytes(dest + (i as u64) * 32, node)?;
                }
                if root_out != 0 {
                    vm.memory.store_bytes(root_out, root.as_ref())?;
                }
                vm.set_register(10, path.len() as u64);
                Ok(Self::merkle_path_gas(path.len()))
            }
            crate::syscalls::SYSCALL_GET_MERKLE_COMPACT => {
                let addr = vm.register(10);
                let max_addr = vm
                    .memory
                    .stack_top()
                    .saturating_add(crate::Memory::STACK_SLOP);
                if addr >= max_addr {
                    return Err(VMError::MemoryOutOfBounds);
                }
                let dest = vm.register(11);
                let depth_cap_raw = vm.register(12) as usize;
                let depth_cap = if depth_cap_raw == 0 {
                    None
                } else {
                    Some(depth_cap_raw.min(32))
                };
                let root_out = vm.register(13);
                let (proof, root) = vm.memory.merkle_compact(addr, depth_cap);
                let depth = proof.depth() as usize;
                vm.memory.store_bytes(dest, &[proof.depth()])?;
                vm.memory
                    .store_bytes(dest + 1, &proof.dirs().to_le_bytes())?;
                let count = depth as u32;
                vm.memory.store_bytes(dest + 1 + 4, &count.to_le_bytes())?;
                let mut off = dest + 1 + 4 + 4;
                for sibling in proof.siblings() {
                    let bytes = sibling.map(|hash| *hash.as_ref()).unwrap_or([0u8; 32]);
                    vm.memory.store_bytes(off, &bytes)?;
                    off += 32;
                }
                if root_out != 0 {
                    vm.memory.store_bytes(root_out, root.as_ref())?;
                }
                vm.set_register(10, depth as u64);
                Ok(Self::merkle_path_gas(depth))
            }
            crate::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT => {
                let idx_raw = vm.register(10);
                let idx = usize::try_from(idx_raw).map_err(|_| VMError::RegisterOutOfBounds)?;
                if idx >= crate::parallel::REGISTER_COUNT {
                    return Err(VMError::RegisterOutOfBounds);
                }
                let dest = vm.register(11);
                let depth_cap_raw = vm.register(12) as usize;
                let depth_cap = if depth_cap_raw == 0 {
                    None
                } else {
                    Some(depth_cap_raw.min(32))
                };
                let root_out = vm.register(13);
                let (proof, root) = vm.registers.merkle_compact(idx, depth_cap);
                let depth = proof.depth() as usize;
                vm.memory.store_bytes(dest, &[proof.depth()])?;
                vm.memory
                    .store_bytes(dest + 1, &proof.dirs().to_le_bytes())?;
                let count = depth as u32;
                vm.memory.store_bytes(dest + 1 + 4, &count.to_le_bytes())?;
                let mut off = dest + 1 + 4 + 4;
                for sibling in proof.siblings() {
                    let bytes = sibling.map(|hash| *hash.as_ref()).unwrap_or([0u8; 32]);
                    vm.memory.store_bytes(off, &bytes)?;
                    off += 32;
                }
                if root_out != 0 {
                    vm.memory.store_bytes(root_out, root.as_ref())?;
                }
                vm.set_register(10, depth as u64);
                Ok(Self::merkle_path_gas(depth))
            }
            // --- ZK verify/state-read helpers ---
            crate::syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | crate::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                // The standalone IVM host supports direct Halo2 opening verification for
                // single-envelope syscalls so tests can exercise the real gating surface
                // without a full node host.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let gas = Self::verify_gas(tlv.payload.len());
                match self.verify_zk_open_envelope(number, tlv.payload) {
                    Ok(true) => {
                        vm.set_register(10, 1);
                        vm.set_register(11, 0);
                    }
                    Ok(false) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_VERIFY);
                    }
                    Err(status) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, status);
                    }
                }
                Ok(gas)
            }
            crate::syscalls::SYSCALL_ZK_ROOTS_GET | crate::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                // Expect a NoritoBytes TLV pointer in r10 (request). DefaultHost has
                // no ledger state, so it writes an empty deterministic response into
                // INPUT and returns a pointer.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let input_len = tlv.payload.len();
                if number == crate::syscalls::SYSCALL_ZK_ROOTS_GET {
                    // Decode request
                    let _req: crate::zk_verify::RootsGetRequest =
                        norito::decode_from_bytes(tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                    // DefaultHost response (empty roots)
                    let resp = crate::zk_verify::RootsGetResponse {
                        latest: [0u8; 32],
                        roots: Vec::new(),
                        height: 0,
                    };
                    let body = norito::to_bytes(&resp).map_err(|_| VMError::NoritoInvalid)?;
                    // Build TLV in INPUT and return its pointer
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(Self::state_query_gas(input_len.saturating_add(body.len())))
                } else {
                    // Vote tally read
                    let _req: crate::zk_verify::VoteGetTallyRequest =
                        norito::decode_from_bytes(tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                    let resp = crate::zk_verify::VoteGetTallyResponse {
                        finalized: false,
                        tally: Vec::new(),
                    };
                    let body = norito::to_bytes(&resp).map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(Self::state_query_gas(input_len.saturating_add(body.len())))
                }
            }
            crate::syscalls::SYSCALL_ZK_VERIFY_BATCH => {
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let gas = Self::verify_gas(tlv.payload.len());
                match self.verify_zk_open_batch(tlv.payload) {
                    Ok((statuses, first_error)) => {
                        let body =
                            norito::to_bytes(&statuses).map_err(|_| VMError::NoritoInvalid)?;
                        let ptr = Self::alloc_norito_bytes_tlv(vm, &body)?;
                        vm.set_register(10, ptr);
                        vm.set_register(11, first_error.unwrap_or(0));
                        if let Some((idx, _)) = statuses
                            .iter()
                            .enumerate()
                            .find(|(_, status)| **status == 0)
                        {
                            vm.set_register(12, idx as u64);
                        } else {
                            vm.set_register(12, u64::MAX);
                        }
                    }
                    Err(status) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, status);
                    }
                }
                Ok(gas)
            }
            syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(vm),
            syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),
            _ => Err(Self::unsupported_syscall_error(number)),
        }
    }

    /// Downcast support for hosts with extra methods/state.
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }

    fn supports_concurrent_blocks(&self) -> bool {
        false
    }

    fn begin_tx(&mut self, _declared: &StateAccessSet) -> Result<(), VMError> {
        self.access_log.read_keys.clear();
        self.access_log.write_keys.clear();
        self.access_log.reg_tags.clear();
        self.access_log.state_writes.clear();
        Ok(())
    }

    fn finish_tx(&mut self) -> Result<AccessLog, VMError> {
        Ok(self.access_log.clone())
    }

    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }

    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        if let Some(saved) = snapshot.downcast_ref::<DefaultHost>() {
            *self = saved.clone();
            true
        } else {
            false
        }
    }

    fn access_logging_supported(&self) -> bool {
        true
    }

    fn set_external_vk_bytes(&mut self, backend: String, bytes: Vec<u8>) {
        self.halo2_external_vks.insert(backend, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgramMetadata;
    use crate::pointer_abi::PointerType;

    fn test_tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
        out.extend_from_slice(&(kind as u16).to_be_bytes());
        out.push(1);
        out.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test payload length")
                .to_be_bytes(),
        );
        out.extend_from_slice(payload);
        let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
        out.extend_from_slice(&hash);
        out
    }

    #[test]
    fn downcast_default_host() {
        let mut host: Box<dyn IVMHost + Send + Sync> = Box::new(DefaultHost::new());
        assert!(host.as_any().downcast_mut::<DefaultHost>().is_some());
    }

    #[test]
    fn conservative_quote_ignores_registers_outside_the_syscall_signature() {
        let mut vm = IVM::new(u64::MAX);
        let pointer = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, &[0x5A; 256]))
            .expect("allocate quote fixture");
        let syscall = syscalls::SYSCALL_REGISTER_DOMAIN;
        let baseline = conservative_syscall_gas_quote(syscall, &vm);

        vm.set_register(15, pointer);
        vm.registers.set_tag(15, true);
        assert_eq!(
            conservative_syscall_gas_quote(syscall, &vm),
            baseline,
            "an unrelated secret register must not influence public gas"
        );

        vm.set_register(10, pointer);
        assert!(
            conservative_syscall_gas_quote(syscall, &vm) > baseline,
            "the documented r10 argument must remain part of the quote"
        );
    }

    #[test]
    fn debug_log_quote_and_execution_charge_payload_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let pointer = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, &[b'x'; 256]))
            .expect("allocate debug log fixture");
        vm.set_register(10, pointer);
        let mut host = DefaultHost::new();

        let quoted = host
            .prepare_syscall(syscalls::SYSCALL_DEBUG_LOG, &vm)
            .expect("quote debug log");
        let actual = host
            .syscall(syscalls::SYSCALL_DEBUG_LOG, &mut vm)
            .expect("execute debug log");
        assert_eq!(quoted, actual);
        assert_eq!(actual, debug_log_gas(256));
    }

    #[test]
    fn numeric_quote_is_payload_bounded_and_type_checked() {
        let mut vm = IVM::new(u64::MAX);
        let left = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &[0xA5; 256]))
            .expect("allocate left Numeric-shaped fixture");
        let right = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &[0x5A; 128]))
            .expect("allocate right Numeric-shaped fixture");
        vm.set_register(10, left);
        vm.set_register(11, right);
        let host = DefaultHost::new();

        assert_eq!(
            host.prepare_syscall(syscalls::SYSCALL_NUMERIC_ADD, &vm),
            Ok(DefaultHost::numeric_gas() + 256 + 128),
            "preparation must reserve declared operand bytes without decoding them"
        );

        let wrong_type = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, &[0x11; 32]))
            .expect("allocate wrong-type Numeric fixture");
        vm.set_register(11, wrong_type);
        assert!(
            host.prepare_syscall(syscalls::SYSCALL_NUMERIC_ADD, &vm)
                .is_err(),
            "preparation must reject a non-Numeric pointer type"
        );
    }

    #[test]
    fn zk_verify_label_mapping_covers_envelope_syscalls() {
        assert_eq!(
            DefaultHost::expected_zk_verify_label(syscalls::SYSCALL_ZK_VERIFY_TRANSFER),
            Some(LABEL_TRANSFER)
        );
        assert_eq!(
            DefaultHost::expected_zk_verify_label(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD),
            Some(LABEL_UNSHIELD)
        );
        assert_eq!(
            DefaultHost::expected_zk_verify_label(syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT),
            Some(LABEL_VOTE_BALLOT)
        );
        assert_eq!(
            DefaultHost::expected_zk_verify_label(syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY),
            Some(LABEL_VOTE_TALLY)
        );
        assert_eq!(
            DefaultHost::expected_zk_verify_label(syscalls::SYSCALL_ZK_VERIFY_BATCH),
            Some(LABEL_BATCH)
        );
    }

    #[test]
    fn zk_curve_allowlist_tracks_host_curve_family() {
        let pallas_host = DefaultHost::new();
        assert!(pallas_host.zk_curve_allowed(iroha_zkp_halo2::ZkCurveId::Pallas));
        assert!(pallas_host.zk_curve_allowed(iroha_zkp_halo2::ZkCurveId::Pasta));
        assert!(!pallas_host.zk_curve_allowed(iroha_zkp_halo2::ZkCurveId::Bn254));

        let bn254_host = DefaultHost::new().with_zk_curve_str("bn254");
        assert!(bn254_host.zk_curve_allowed(iroha_zkp_halo2::ZkCurveId::Bn254));
        assert!(!bn254_host.zk_curve_allowed(iroha_zkp_halo2::ZkCurveId::Pallas));
    }

    #[test]
    fn time_syscalls_use_configured_deterministic_value() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();

        assert_eq!(
            host.syscall(syscalls::SYSCALL_CURRENT_TIME_MS, &mut vm),
            Ok(DefaultHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 0);

        host.set_current_time_ms(42);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS, &mut vm),
            Ok(DefaultHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 42);
    }

    #[test]
    fn block_height_syscall_uses_configured_deterministic_value() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();

        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT, &mut vm),
            Ok(DefaultHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 0);

        host.set_current_block_height(77);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT, &mut vm),
            Ok(DefaultHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 77);
    }

    #[test]
    fn chain_id_sysvar_charges_returned_bytes() {
        crate::set_banner_enabled(false);
        let chain_id = b"chain-A".to_vec();
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new().with_chain_id(chain_id.clone());

        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_CHAIN_ID, &mut vm),
            Ok(DefaultHost::sysvar_gas(chain_id.len()))
        );
        let tlv = vm.memory.validate_tlv(vm.register(10)).expect("chain tlv");
        assert_eq!(tlv.type_id, PointerType::Blob);
        assert_eq!(tlv.payload, chain_id.as_slice());
    }

    #[test]
    fn default_host_vrf_verify_status_paths_charge_payload_bytes() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();

        let wrong_type_payload = b"{\"not\":\"vrf\"}";
        let wrong_type_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, wrong_type_payload))
            .expect("alloc wrong type");
        vm.set_register(10, wrong_type_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_VRF_VERIFY, &mut vm),
            Ok(DefaultHost::verify_gas(wrong_type_payload.len()))
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), 1);

        let malformed = [0xff];
        let malformed_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &malformed))
            .expect("alloc malformed request");
        vm.set_register(10, malformed_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_VRF_VERIFY, &mut vm),
            Ok(DefaultHost::verify_gas(malformed.len()))
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), 2);

        let bad_pk = crate::vrf::VrfVerifyRequest {
            variant: 1,
            pk: vec![1],
            proof: vec![2; 96],
            chain_id: b"test-chain".to_vec(),
            input: b"message".to_vec(),
        };
        let body = norito::to_bytes(&bad_pk).expect("encode vrf request");
        let bad_pk_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &body))
            .expect("alloc bad pk request");
        vm.set_register(10, bad_pk_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_VRF_VERIFY, &mut vm),
            Ok(DefaultHost::verify_gas(body.len()))
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), 4);
    }

    #[test]
    fn default_host_vrf_batch_empty_request_charges_payload_bytes() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let request = crate::vrf::VrfVerifyBatchRequest { items: Vec::new() };
        let body = norito::to_bytes(&request).expect("encode vrf batch request");
        let ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &body))
            .expect("alloc batch request");

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_VRF_VERIFY_BATCH, &mut vm),
            Ok(DefaultHost::verify_gas(body.len()))
        );
        assert_eq!(vm.register(11), 0);
        let out = vm.memory.validate_tlv(vm.register(10)).expect("output tlv");
        assert_eq!(out.type_id, PointerType::NoritoBytes);
        let outputs: Vec<[u8; 32]> = norito::decode_from_bytes(out.payload).expect("decode output");
        assert!(outputs.is_empty());
    }

    #[test]
    fn default_host_zk_verify_status_paths_charge_payload_bytes() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();

        let malformed = [0xff, 0x00, 0x01];
        let ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &malformed))
            .expect("alloc malformed envelope");
        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm),
            Ok(DefaultHost::verify_gas(malformed.len()))
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), ERR_DECODE);

        let batch_payload = b"batch-envelope";
        let batch_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, batch_payload))
            .expect("alloc batch envelope");
        vm.set_register(10, batch_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm),
            Ok(DefaultHost::verify_gas(batch_payload.len()))
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), ERR_DISABLED);
    }

    #[test]
    fn default_host_zk_read_helpers_charge_request_and_response_bytes() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();

        let roots_req = crate::zk_verify::RootsGetRequest {
            asset_id: "rose#garden".to_string(),
            max: 8,
        };
        let roots_payload = norito::to_bytes(&roots_req).expect("encode roots request");
        let roots_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &roots_payload))
            .expect("alloc roots request");
        vm.set_register(10, roots_ptr);
        let roots_quote = host
            .prepare_syscall(syscalls::SYSCALL_ZK_ROOTS_GET, &vm)
            .expect("quote roots get");
        let roots_gas = host
            .syscall(syscalls::SYSCALL_ZK_ROOTS_GET, &mut vm)
            .expect("roots get");
        assert!(roots_gas <= roots_quote);
        let roots_out = vm.memory.validate_tlv(vm.register(10)).expect("roots tlv");
        assert_eq!(roots_out.type_id, PointerType::NoritoBytes);
        assert_eq!(
            roots_gas,
            DefaultHost::state_query_gas(
                roots_payload.len().saturating_add(roots_out.payload.len())
            )
        );

        let tally_req = crate::zk_verify::VoteGetTallyRequest {
            election_id: "election".to_string(),
        };
        let tally_payload = norito::to_bytes(&tally_req).expect("encode tally request");
        let tally_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &tally_payload))
            .expect("alloc tally request");
        vm.set_register(10, tally_ptr);
        let tally_quote = host
            .prepare_syscall(syscalls::SYSCALL_ZK_VOTE_GET_TALLY, &vm)
            .expect("quote vote tally");
        let tally_gas = host
            .syscall(syscalls::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm)
            .expect("vote tally");
        assert!(tally_gas <= tally_quote);
        let tally_out = vm.memory.validate_tlv(vm.register(10)).expect("tally tlv");
        assert_eq!(tally_out.type_id, PointerType::NoritoBytes);
        assert_eq!(
            tally_gas,
            DefaultHost::state_query_gas(
                tally_payload.len().saturating_add(tally_out.payload.len())
            )
        );
    }

    #[test]
    fn default_host_state_has_len_and_keys_roundtrip() {
        fn tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(kind as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload length")
                    .to_be_bytes(),
            );
            out.extend_from_slice(payload);
            let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
            out.extend_from_slice(&hash);
            out
        }

        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let key: Name = "orders/1".parse().expect("state key");
        let prefix: Name = "orders".parse().expect("state prefix");
        let key_payload = norito::to_bytes(&key).expect("encode key");
        let prefix_payload = norito::to_bytes(&prefix).expect("encode prefix");
        let key_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &key_payload))
            .expect("alloc key");
        let prefix_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &prefix_payload))
            .expect("alloc prefix");
        let value_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::NoritoBytes, b"value"))
            .expect("alloc value");

        vm.set_register(10, key_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_STATE_SET, &mut vm),
            Ok(DefaultHost::state_query_gas(b"value".len()))
        );

        vm.set_register(10, key_ptr);
        host.syscall(syscalls::SYSCALL_STATE_HAS, &mut vm)
            .expect("STATE_HAS");
        assert_eq!(vm.register(10), 1);

        vm.set_register(10, key_ptr);
        host.syscall(syscalls::SYSCALL_STATE_LEN, &mut vm)
            .expect("STATE_LEN");
        assert_eq!(vm.register(10), 5);
        assert_eq!(vm.register(11), 1);

        vm.set_register(10, prefix_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_STATE_COUNT, &mut vm),
            Ok(DefaultHost::state_count_gas(1))
        );
        assert_eq!(vm.register(10), 1);

        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, syscalls::STATE_KEYS_MAX_ITEMS);
        host.syscall(syscalls::SYSCALL_STATE_KEYS, &mut vm)
            .expect("STATE_KEYS");
        assert_eq!(vm.register(11), 1);
        assert_eq!(vm.register(12), 1);
        let keys_tlv = vm.memory.validate_tlv(vm.register(10)).expect("keys tlv");
        let keys: Vec<Name> = norito::decode_from_bytes(keys_tlv.payload).expect("decode keys");
        assert_eq!(keys, vec![key.clone()]);

        vm.set_register(10, key_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_STATE_DEL, &mut vm),
            Ok(STATE_QUERY_GAS_BASE)
        );
        vm.set_register(10, key_ptr);
        host.syscall(syscalls::SYSCALL_STATE_HAS, &mut vm)
            .expect("STATE_HAS absent");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn host_state_dependent_prepare_reserves_without_observing_state() {
        let vm = IVM::new(321);
        let empty = DefaultHost::new();
        let mut populated = DefaultHost::new();
        populated
            .state
            .insert("orders/1".parse().expect("state key"), vec![0u8; 128]);
        populated.public_inputs.insert(
            "public".parse().expect("public input name"),
            test_tlv(PointerType::Blob, &[0u8; 128]),
        );
        populated.nullifiers.insert(9);
        populated.fastpq_batch_active = true;

        for syscall in [
            syscalls::SYSCALL_STATE_GET,
            syscalls::SYSCALL_STATE_LEN,
            syscalls::SYSCALL_STATE_KEYS,
            syscalls::SYSCALL_STATE_COUNT,
            syscalls::SYSCALL_GET_PUBLIC_INPUT,
            syscalls::SYSCALL_USE_NULLIFIER,
            syscalls::SYSCALL_TRANSFER_V1,
            syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY,
            syscalls::SYSCALL_AXT_TOUCH,
            syscalls::SYSCALL_VERIFY_DS_PROOF,
            syscalls::SYSCALL_USE_ASSET_HANDLE,
            syscalls::SYSCALL_AXT_COMMIT,
        ] {
            assert_eq!(empty.prepare_syscall(syscall, &vm), Ok(321));
            assert_eq!(populated.prepare_syscall(syscall, &vm), Ok(321));
        }

        assert_eq!(
            reserve_available_syscall_gas(&IVM::new(0)),
            Err(VMError::OutOfGas)
        );
    }

    #[test]
    fn default_host_pointer_helpers_roundtrip_and_charge_gas() {
        fn tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(kind as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload length")
                    .to_be_bytes(),
            );
            out.extend_from_slice(payload);
            let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
            out.extend_from_slice(&hash);
            out
        }

        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let payload = b"deterministic";
        let ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, payload))
            .expect("alloc blob");
        let envelope_len = 2 + 1 + 4 + payload.len() + iroha_crypto::Hash::LENGTH;

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_TO_NORITO, &mut vm),
            Ok(DefaultHost::pointer_gas(envelope_len))
        );
        let wrapped_ptr = vm.register(10);
        let wrapped = vm.memory.validate_tlv(wrapped_ptr).expect("wrapped tlv");
        assert_eq!(wrapped.type_id, PointerType::NoritoBytes);
        assert_eq!(wrapped.payload.len(), envelope_len);

        vm.set_register(10, wrapped_ptr);
        vm.set_register(11, PointerType::Blob as u64);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm),
            Ok(DefaultHost::pointer_gas(envelope_len))
        );
        let roundtrip = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("roundtrip tlv");
        assert_eq!(roundtrip.type_id, PointerType::Blob);
        assert_eq!(roundtrip.payload, payload);

        let blob_carrier = tlv(PointerType::Blob, &tlv(PointerType::Blob, payload));
        let blob_carrier_ptr = vm
            .alloc_input_tlv(&blob_carrier)
            .expect("alloc blob carrier");
        vm.set_register(10, blob_carrier_ptr);
        vm.set_register(11, PointerType::Blob as u64);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm),
            Ok(DefaultHost::pointer_gas(envelope_len))
        );
        let blob_roundtrip = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("blob-carrier roundtrip tlv");
        assert_eq!(blob_roundtrip.type_id, PointerType::Blob);
        assert_eq!(blob_roundtrip.payload, payload);
    }

    #[test]
    fn common_helper_quotes_are_exact_at_length_boundaries() {
        crate::set_banner_enabled(false);
        for payload_len in [0, 1, 127, 128, 255, 256, 4095] {
            let mut vm = IVM::new(u64::MAX);
            let mut host = DefaultHost::new();
            let payload = vec![0x5a; payload_len];
            let pointer = vm
                .alloc_input_tlv(&test_tlv(PointerType::Blob, &payload))
                .expect("allocate boundary TLV");
            vm.set_register(10, pointer);
            let registers_before = (vm.register(10), vm.register(11));
            vm.memory.clear_tracking();
            let quote = host
                .prepare_syscall(syscalls::SYSCALL_POINTER_TO_NORITO, &vm)
                .expect("quote pointer conversion");
            assert_eq!(
                (vm.register(10), vm.register(11)),
                registers_before,
                "preparation must not mutate registers"
            );
            assert!(
                vm.memory.read_set().is_empty(),
                "preparation must not mutate memory access tracking"
            );
            assert_eq!(
                host.syscall(syscalls::SYSCALL_POINTER_TO_NORITO, &mut vm)
                    .expect("convert pointer"),
                quote
            );
        }

        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let left = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, b"left"))
            .expect("allocate left TLV");
        vm.set_register(10, left);
        vm.set_register(11, Memory::OUTPUT_START);
        let error = host
            .prepare_syscall(syscalls::SYSCALL_TLV_EQ, &vm)
            .expect_err("invalid right pointer must fail strict pointer-ABI preparation");
        assert!(matches!(
            error,
            VMError::NoritoInvalid | VMError::MemoryAccessViolation { .. }
        ));
        assert!(host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm).is_err());

        vm.set_register(10, Memory::OUTPUT_START);
        vm.set_register(11, Memory::OUTPUT_START);
        assert!(
            host.prepare_syscall(syscalls::SYSCALL_TLV_EQ, &vm).is_err(),
            "equal raw addresses must not bypass pointer-ABI validation"
        );
        assert!(host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm).is_err());

        vm.set_register(10, 0);
        vm.set_register(11, Memory::OUTPUT_START);
        assert!(
            host.prepare_syscall(syscalls::SYSCALL_TLV_EQ, &vm).is_err(),
            "null comparison must still validate the non-null operand"
        );
        assert!(host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm).is_err());
    }

    #[test]
    fn crypto_quotes_use_processed_lengths_and_saturate() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let json = Json::from(norito::json!({
            "z": [true, false],
            "a": "canonicalized",
        }));
        let json_payload = norito::to_bytes(&json).expect("encode JSON envelope");
        let message = vm
            .alloc_input_tlv(&test_tlv(PointerType::Json, &json_payload))
            .expect("allocate message");
        let signature = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, b"signature"))
            .expect("allocate signature");
        let public_key = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, b"public-key"))
            .expect("allocate public key");
        vm.set_register(10, message);
        vm.set_register(11, signature);
        vm.set_register(12, public_key);
        vm.set_register(13, u64::MAX);
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_VERIFY_SIGNATURE, &vm)
            .expect("quote signature verification");
        let actual = host
            .syscall(syscalls::SYSCALL_VERIFY_SIGNATURE, &mut vm)
            .expect("unsupported scheme still reports processed gas");
        assert!(actual <= quote);

        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new().with_sm_enabled(true);
        for (register, payload) in [
            (10, Vec::new()),
            (11, Vec::new()),
            (12, vec![0xa5; 127]),
            (13, vec![0x5a; 128]),
        ] {
            let pointer = vm
                .alloc_input_tlv(&test_tlv(PointerType::Blob, &payload))
                .expect("allocate SM4 input");
            vm.set_register(register, pointer);
        }
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_SM4_GCM_SEAL, &vm)
            .expect("quote SM4");
        assert_eq!(quote, DefaultHost::sm4_gas(127, 128));
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SM4_GCM_SEAL, &mut vm)
                .expect("invalid key is a metered verification result"),
            quote
        );

        assert_eq!(DefaultHost::sm4_gas(usize::MAX, usize::MAX), u64::MAX);
        assert_eq!(
            DefaultHost::signature_verify_gas(usize::MAX, usize::MAX, usize::MAX),
            u64::MAX
        );
        assert_eq!(DefaultHost::tlv_eq_gas(usize::MAX, usize::MAX), u64::MAX);
    }

    #[test]
    fn unaffordable_signature_syscall_does_not_parse_the_message_during_prepare() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut code = Vec::new();
        code.extend_from_slice(
            &crate::encoding::wide::encode_sys(
                crate::instruction::wide::system::SCALL,
                u8::try_from(syscalls::SYSCALL_VERIFY_SIGNATURE).expect("syscall fits"),
            )
            .to_le_bytes(),
        );
        code.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        vm.load_code(&code).expect("load signature test program");
        let message = vm
            .alloc_input_tlv(&test_tlv(PointerType::Json, b"not valid Norito JSON"))
            .expect("allocate malformed JSON message");
        let signature = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, b"signature"))
            .expect("allocate signature");
        let public_key = vm
            .alloc_input_tlv(&test_tlv(PointerType::Blob, b"public-key"))
            .expect("allocate public key");
        vm.set_register(10, message);
        vm.set_register(11, signature);
        vm.set_register(12, public_key);
        let mut host = DefaultHost::new();
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_VERIFY_SIGNATURE, &vm)
            .expect("header-only preparation must not parse the message");
        vm.set_gas_limit(quote.saturating_add(4));

        let error = vm
            .run_with_host(&mut host)
            .expect_err("the quote is one gas beyond the post-SCALL budget");

        assert_eq!(error, VMError::OutOfGas);
        assert_eq!(vm.register(10), message);
        assert_eq!(vm.register(11), signature);
        assert_eq!(vm.register(12), public_key);
    }

    #[test]
    fn execution_proof_and_merkle_quotes_match_actual_costs() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let proof_quote = host
            .prepare_syscall(syscalls::SYSCALL_PROVE_EXECUTION, &vm)
            .expect("quote proof");
        assert_eq!(
            host.syscall(syscalls::SYSCALL_PROVE_EXECUTION, &mut vm)
                .expect("produce proof"),
            proof_quote
        );

        for depth_cap in [0, 1, 8, 32, u64::MAX] {
            let mut vm = IVM::new(u64::MAX);
            let mut host = DefaultHost::new();
            vm.memory
                .store_u32(Memory::HEAP_START, 0xfeed_beef)
                .expect("seed Merkle leaf");
            vm.set_register(10, Memory::HEAP_START);
            vm.set_register(11, Memory::OUTPUT_START);
            vm.set_register(12, depth_cap);
            vm.set_register(13, 0);
            let quote = host
                .prepare_syscall(syscalls::SYSCALL_GET_MERKLE_COMPACT, &vm)
                .expect("quote compact proof");
            assert_eq!(
                host.syscall(syscalls::SYSCALL_GET_MERKLE_COMPACT, &mut vm)
                    .expect("produce compact proof"),
                quote
            );
        }

        let mut vm = IVM::new(u64::MAX);
        let stack_top = vm.memory.stack_top();
        vm.set_register(10, stack_top);
        assert_eq!(
            DefaultHost::new().prepare_syscall(syscalls::SYSCALL_GET_MERKLE_PATH, &vm),
            Err(VMError::MemoryOutOfBounds)
        );
        vm.set_register(10, crate::parallel::REGISTER_COUNT as u64);
        assert_eq!(
            DefaultHost::new().prepare_syscall(syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT, &vm),
            Err(VMError::RegisterOutOfBounds)
        );
    }

    #[test]
    fn disabled_and_unknown_syscall_quotes_fail_closed_without_over_reserving() {
        let vm = IVM::new(u64::MAX);
        let host = DefaultHost::new();
        assert_eq!(host.prepare_syscall(syscalls::SYSCALL_SM3_HASH, &vm), Ok(0));
        assert_eq!(
            host.prepare_syscall(syscalls::SYSCALL_REGISTER_DOMAIN, &vm),
            Ok(MUTATION_GAS)
        );
        assert_eq!(
            host.prepare_syscall(0xffff_fffe, &vm),
            Err(VMError::UnknownSyscall(0xffff_fffe))
        );
    }

    #[test]
    fn default_host_sm4_gcm_charges_aad_and_data_bytes() {
        fn tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(kind as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload length")
                    .to_be_bytes(),
            );
            out.extend_from_slice(payload);
            let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
            out.extend_from_slice(&hash);
            out
        }

        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new().with_sm_enabled(true);
        let key = [0x11u8; 16];
        let nonce = [0x22u8; 12];
        let aad = b"aad";
        let plaintext = b"plaintext";
        let key_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, &key))
            .expect("alloc key");
        let nonce_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, &nonce))
            .expect("alloc nonce");
        let aad_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, aad))
            .expect("alloc aad");
        let plaintext_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, plaintext))
            .expect("alloc plaintext");

        vm.set_register(10, key_ptr);
        vm.set_register(11, nonce_ptr);
        vm.set_register(12, aad_ptr);
        vm.set_register(13, plaintext_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SM4_GCM_SEAL, &mut vm),
            Ok(DefaultHost::sm4_gas(aad.len(), plaintext.len()))
        );
        let sealed_ptr = vm.register(10);
        let sealed_len = {
            let sealed = vm.memory.validate_tlv(sealed_ptr).expect("sealed tlv");
            assert_eq!(sealed.type_id, PointerType::Blob);
            assert_eq!(sealed.payload.len(), plaintext.len() + 16);
            sealed.payload.len()
        };

        vm.set_register(10, key_ptr);
        vm.set_register(11, nonce_ptr);
        vm.set_register(12, aad_ptr);
        vm.set_register(13, sealed_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SM4_GCM_OPEN, &mut vm),
            Ok(DefaultHost::sm4_gas(aad.len(), sealed_len))
        );
        let opened = vm.memory.validate_tlv(vm.register(10)).expect("opened tlv");
        assert_eq!(opened.type_id, PointerType::Blob);
        assert_eq!(opened.payload, plaintext);
    }

    #[test]
    fn default_host_tlv_eq_charges_payload_bytes() {
        fn tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(kind as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload length")
                    .to_be_bytes(),
            );
            out.extend_from_slice(payload);
            let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
            out.extend_from_slice(&hash);
            out
        }

        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let left = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, b"same"))
            .expect("alloc left");
        let right = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, b"same"))
            .expect("alloc right");

        vm.set_register(10, left);
        vm.set_register(11, right);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm),
            Ok(DefaultHost::tlv_eq_gas(4, 4))
        );
        assert_eq!(vm.register(10), 1);
    }

    #[test]
    fn default_host_tlv_len_charges_payload_bytes() {
        fn tlv(kind: PointerType, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
            out.extend_from_slice(&(kind as u16).to_be_bytes());
            out.push(1);
            out.extend_from_slice(
                &u32::try_from(payload.len())
                    .expect("test payload length")
                    .to_be_bytes(),
            );
            out.extend_from_slice(payload);
            let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
            out.extend_from_slice(&hash);
            out
        }

        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut host = DefaultHost::new();
        let ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, b"length"))
            .expect("alloc tlv");

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_TLV_LEN, &mut vm),
            Ok(DefaultHost::tlv_len_gas(6))
        );
        assert_eq!(vm.register(10), 6);
    }

    #[test]
    fn input_publish_accepts_owned_public_heap_tlv_without_copying() {
        let mut host = DefaultHost::new();
        let mut vm = IVM::new(u64::MAX);
        let envelope = test_tlv(PointerType::Blob, b"heap result");
        let pointer = vm
            .alloc_heap(envelope.len() as u64)
            .expect("allocate owned heap range");
        vm.store_bytes(pointer, &envelope)
            .expect("store public heap TLV");
        vm.set_register(10, pointer);

        assert_eq!(
            host.prepare_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &vm),
            Ok(DefaultHost::input_publish_gas(envelope.len()))
        );
        assert_eq!(
            host.syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut vm),
            Ok(DefaultHost::input_publish_gas(envelope.len()))
        );
        assert_eq!(vm.register(10), pointer);
    }

    #[test]
    fn input_publish_rejects_malformed_and_unowned_heap_tlvs() {
        let mut host = DefaultHost::new();
        let mut vm = IVM::new(u64::MAX);

        let mut malformed = test_tlv(PointerType::Blob, b"bad hash");
        *malformed.last_mut().expect("hash byte") ^= 1;
        let malformed_pointer = vm
            .alloc_heap(malformed.len() as u64)
            .expect("allocate malformed envelope");
        vm.store_bytes(malformed_pointer, &malformed)
            .expect("store malformed envelope");
        vm.set_register(10, malformed_pointer);
        assert!(matches!(
            host.syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut vm),
            Err(VMError::NoritoInvalid)
        ));

        let unowned = test_tlv(PointerType::Blob, b"unowned");
        let unowned_pointer = Memory::HEAP_START + 0x10_000;
        vm.store_bytes(unowned_pointer, &unowned)
            .expect("memory permissions alone permit the adversarial write");
        vm.set_register(10, unowned_pointer);
        assert!(matches!(
            host.prepare_syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &vm),
            Err(VMError::NoritoInvalid)
        ));
        assert_eq!(vm.register(10), unowned_pointer);
    }

    #[test]
    fn default_host_runtime_helpers_charge_declared_gas() {
        crate::set_banner_enabled(false);

        let mut host = DefaultHost::with_private_inputs(vec![42]);
        let mut vm = IVM::new(u64::MAX);
        vm.set_register(10, 0);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_GET_PRIVATE_INPUT, &mut vm),
            Ok(GET_PRIVATE_INPUT_GAS)
        );
        assert_eq!(vm.register(10), 42);

        vm.set_register(10, 7);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_USE_NULLIFIER, &mut vm),
            Ok(NULLIFIER_GAS)
        );

        vm.memory
            .set_heap_limit(0x10_000)
            .expect("shrink heap limit for grow test");
        vm.set_register(10, 4097);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_GROW_HEAP, &mut vm),
            Ok(DefaultHost::grow_heap_gas(4097))
        );

        assert_eq!(
            host.syscall(syscalls::SYSCALL_COMMIT_OUTPUT, &mut vm),
            Ok(COMMIT_OUTPUT_GAS)
        );

        let tlv = test_tlv(PointerType::Blob, b"publish");
        let publish_ptr = vm.alloc_input_tlv(&tlv).expect("alloc publish tlv");
        vm.set_register(10, publish_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut vm),
            Ok(DefaultHost::input_publish_gas(tlv.len()))
        );

        let account_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::AccountId, b""))
            .expect("alloc account");
        let name_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::Name, b"k"))
            .expect("alloc name");
        let json_payload = b"{\"v\":1}";
        let json_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::Json, json_payload))
            .expect("alloc json");
        vm.set_register(10, account_ptr);
        vm.set_register(11, name_ptr);
        vm.set_register(12, json_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL, &mut vm),
            Ok(DefaultHost::mutation_gas(json_payload.len()))
        );

        let msg = test_tlv(PointerType::Blob, b"m");
        let sig = test_tlv(PointerType::Blob, b"sig");
        let pk = test_tlv(PointerType::Blob, b"pk");
        let msg_ptr = vm.alloc_input_tlv(&msg).expect("alloc msg");
        let sig_ptr = vm.alloc_input_tlv(&sig).expect("alloc sig");
        let pk_ptr = vm.alloc_input_tlv(&pk).expect("alloc pk");
        vm.set_register(10, msg_ptr);
        vm.set_register(11, sig_ptr);
        vm.set_register(12, pk_ptr);
        vm.set_register(13, u64::MAX);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_VERIFY_SIGNATURE, &mut vm),
            Ok(DefaultHost::signature_verify_gas(1, 3, 2))
        );
        assert_eq!(vm.register(10), 0);

        let addr = crate::Memory::HEAP_START;
        vm.memory.store_u32(addr, 0xABCD).expect("store heap");
        vm.memory.commit();
        let path_len = vm.memory.merkle_path(addr).len();
        vm.set_register(10, addr);
        vm.set_register(11, crate::Memory::OUTPUT_START);
        vm.set_register(12, 0);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_GET_MERKLE_PATH, &mut vm),
            Ok(DefaultHost::merkle_path_gas(path_len))
        );
        assert_eq!(
            vm.register(10),
            u64::try_from(path_len).expect("path len fits")
        );
    }

    #[test]
    fn expect_tlv_enforces_pointer_policy() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let program = ProgramMetadata::default_for(1, 0, 1).encode();
        vm.load_program(&program).expect("load program");
        // The first release only supports ABI v1; installing any other
        // annotated ABI version must fail closed during pointer validation.
        let _guard =
            crate::pointer_abi::PointerPolicyGuard::install(crate::SyscallPolicy::AbiV1, 2);
        let mut tlv = Vec::new();
        tlv.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
        tlv.push(1);
        tlv.extend_from_slice(&0u32.to_be_bytes());
        let hash: [u8; 32] = iroha_crypto::Hash::new([]).into();
        tlv.extend_from_slice(&hash);
        let ptr = vm.alloc_input_tlv(&tlv).expect("allocate TLV");
        vm.set_register(10, ptr);
        let err = DefaultHost::expect_tlv(&vm, 10, PointerType::AccountId).unwrap_err();
        assert!(matches!(
            err,
            VMError::AbiTypeNotAllowed { abi: 2, type_id } if type_id == PointerType::AccountId as u16
        ));
    }

    #[test]
    fn amount_arguments_require_canonical_amount_pointer() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let canonical = Numeric::new(125_u32, 2);
        let canonical_payload = norito::to_bytes(&canonical).expect("encode canonical Amount");
        let canonical_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::Amount, &canonical_payload))
            .expect("allocate canonical Amount");
        vm.set_register(13, canonical_ptr);
        assert_eq!(DefaultHost::expect_amount(&vm, 13), Ok(()));

        let legacy_ptr = vm
            .alloc_input_tlv(&test_tlv(PointerType::NoritoBytes, &canonical_payload))
            .expect("allocate legacy Numeric pointer");
        vm.set_register(13, legacy_ptr);
        assert_eq!(
            DefaultHost::expect_amount(&vm, 13),
            Err(VMError::NoritoInvalid)
        );

        let noncanonical = Numeric::new(1_250_u32, 3);
        let noncanonical_ptr = vm
            .alloc_input_tlv(&test_tlv(
                PointerType::Amount,
                &norito::to_bytes(&noncanonical).expect("encode noncanonical Amount"),
            ))
            .expect("allocate noncanonical Amount");
        vm.set_register(13, noncanonical_ptr);
        assert_eq!(
            DefaultHost::expect_amount(&vm, 13),
            Err(VMError::DecodeError)
        );
    }
}
