//! Minimal core host shim validating pointer-ABI TLVs for representative syscalls.
//!
//! This host does not execute real ISI. It only validates the pointer-ABI
//! arguments and returns success. It is intended for end-to-end tests that
//! exercise TLV validation from VM bytecode through host dispatch.

use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};

#[cfg(test)]
use std::str::FromStr;

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use iroha_crypto::{
    Hash as IrohaHash, Sm3Digest,
    blake2::{
        Blake2bVar,
        digest::{Update as Blake2Update, VariableOutput},
    },
};
use iroha_data_model::{
    account::AccountId,
    isi::transfer::TransferAssetBatch,
    nexus::{AxtPolicyEntry, AxtPolicySnapshot, DataSpaceId},
    prelude::Name,
};
#[cfg(test)]
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_primitives::{json::Json, numeric_abi::QuantityValueV1};
use norito::{decode_from_bytes, json as njson, to_bytes};
use sha2::{Digest as Sha2Digest, Sha256};
use sha3_hash::{Digest as Sha3Digest, Keccak256, Sha3_256};

use crate::{
    VMError,
    axt::{self, AxtPolicy},
    gas,
    host::{
        AccessLog, IVMHost, TLV_ENVELOPE_OVERHEAD, canonical_state_map_key_at,
        canonical_typed_state_map_path, checked_state_keys_limit, common_syscall_gas_quote,
        conservative_syscall_gas_quote, debug_log_gas, is_sm_syscall,
        preflight_reserved_state_keys_page, preflight_reserved_syscall_gas,
        quote_canonical_state_map_path_lengths, quote_tlv_payload_len_at,
        require_host_syscall_metering_spec, reserve_available_syscall_gas_at_least,
        validate_declared_state_map_base, validate_declared_state_map_key,
    },
    ivm::IVM,
    memory::Memory,
    mock_wsv::{MockWorldStateView, SpaceDirectoryAxtPolicy},
    parallel::StateUpdate,
    pointer_abi::{self, PointerType},
    schema_registry::{DefaultRegistry, SchemaRegistry},
    state_overlay::{DurableStateOverlay, DurableStateSnapshot},
    syscalls,
};

const HASH_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const HASH_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const AXT_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const AXT_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const DEBUG_GAS: u64 = gas::HOST_DEBUG_GAS_BASE;
const JSON_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const JSON_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const INPUT_PUBLISH_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const INPUT_PUBLISH_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const MUTATION_GAS: u64 = gas::HOST_BYTE_GAS_BASE;
const MUTATION_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const NAME_DECODE_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const NAME_DECODE_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const NUMERIC_GAS: u64 = gas::HOST_BYTE_GAS_BASE;
const PATH_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const PATH_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const POINTER_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const POINTER_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const SCHEMA_GAS_BASE: u64 = gas::HOST_SCHEMA_GAS_BASE;
const SCHEMA_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const STATE_QUERY_GAS_BASE: u64 = gas::STATE_QUERY_GAS_BASE;
const SYSVAR_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const SYSVAR_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const TLV_EQ_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const TLV_EQ_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const TLV_LEN_GAS_BASE: u64 = gas::HOST_BYTE_GAS_BASE;
const TLV_LEN_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;
const VERIFY_GAS_BASE: u64 = gas::HOST_VERIFY_GAS_BASE;
const VERIFY_GAS_PER_BYTE: u64 = gas::SYSCALL_GAS_PER_BYTE;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CachedProofEntry {
    digest: [u8; 32],
    expiry_slot: Option<u64>,
    verified_slot: u64,
    manifest_root: Option<[u8; 32]>,
    valid: bool,
}

#[allow(dead_code)]
impl CachedProofEntry {
    fn is_applicable_for_slot(&self, slot: Option<u64>, manifest_root: Option<[u8; 32]>) -> bool {
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
        slot == 0 || self.verified_slot == 0 || self.verified_slot == slot
    }
}

#[derive(Clone)]
pub struct CoreHost {
    // Simple in-memory state for STATE_{GET,SET,DEL} syscalls keyed by path.
    state: DurableStateOverlay,
    schema: Arc<dyn SchemaRegistry + Send + Sync>,
    axt_state: Option<axt::HostAxtState>,
    axt_policy: Arc<dyn AxtPolicy>,
    axt_policy_snapshot: Option<AxtPolicySnapshot>,
    axt_proof_cache: BTreeMap<DataSpaceId, CachedProofEntry>,
    axt_proof_cache_slot: Option<u64>,
    slot_length_ms: NonZeroU64,
    max_clock_skew_ms: u64,
    axt_active: bool,
    fastpq_batch_active: bool,
    fastpq_batch_has_entries: bool,
    sm_enabled: bool,
    current_time_ms: u64,
    access_log: AccessLog,
    #[cfg(test)]
    state_scan_examined: Arc<AtomicU64>,
}

#[derive(Clone)]
struct CoreHostSnapshot {
    state: DurableStateSnapshot,
    axt_state: Option<axt::HostAxtState>,
    axt_policy: Arc<dyn AxtPolicy>,
    axt_policy_snapshot: Option<AxtPolicySnapshot>,
    axt_proof_cache: BTreeMap<DataSpaceId, CachedProofEntry>,
    axt_proof_cache_slot: Option<u64>,
    slot_length_ms: NonZeroU64,
    max_clock_skew_ms: u64,
    axt_active: bool,
    fastpq_batch_active: bool,
    fastpq_batch_has_entries: bool,
    sm_enabled: bool,
    current_time_ms: u64,
    access_log: AccessLog,
}

impl CoreHost {
    pub fn new() -> Self {
        Self {
            state: DurableStateOverlay::in_memory(),
            schema: Arc::new(DefaultRegistry::new()),
            axt_state: None,
            axt_policy: Arc::new(axt::AllowAllAxtPolicy),
            axt_policy_snapshot: None,
            axt_proof_cache: BTreeMap::new(),
            axt_proof_cache_slot: None,
            slot_length_ms: NonZeroU64::new(1).expect("slot length must be non-zero"),
            max_clock_skew_ms: 0,
            axt_active: false,
            fastpq_batch_active: false,
            fastpq_batch_has_entries: false,
            sm_enabled: false,
            current_time_ms: 0,
            access_log: AccessLog::default(),
            #[cfg(test)]
            state_scan_examined: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Construct a CoreHost with a specific schema registry implementation.
    pub fn new_with_registry(reg: Box<dyn SchemaRegistry + Send + Sync>) -> Self {
        Self {
            state: DurableStateOverlay::in_memory(),
            schema: Arc::from(reg),
            axt_state: None,
            axt_policy: Arc::new(axt::AllowAllAxtPolicy),
            axt_policy_snapshot: None,
            axt_proof_cache: BTreeMap::new(),
            axt_proof_cache_slot: None,
            slot_length_ms: NonZeroU64::new(1).expect("slot length must be non-zero"),
            max_clock_skew_ms: 0,
            axt_active: false,
            fastpq_batch_active: false,
            fastpq_batch_has_entries: false,
            sm_enabled: false,
            current_time_ms: 0,
            access_log: AccessLog::default(),
            #[cfg(test)]
            state_scan_examined: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Override the AXT policy used by this host.
    pub fn with_axt_policy(mut self, policy: Arc<dyn AxtPolicy>) -> Self {
        self.axt_policy = policy;
        self.axt_policy_snapshot = None;
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = None;
        self
    }

    /// Override the slot length and clock-skew tolerance used for AXT expiry checks.
    pub fn with_axt_timing(mut self, slot_length_ms: NonZeroU64, max_clock_skew_ms: u64) -> Self {
        self.slot_length_ms = slot_length_ms;
        self.max_clock_skew_ms = max_clock_skew_ms;
        self
    }

    /// Install a Space Directory-backed AXT policy from a replicated snapshot.
    pub fn with_axt_policy_snapshot(mut self, snapshot: &AxtPolicySnapshot) -> Self {
        self.axt_policy = Arc::new(SpaceDirectoryAxtPolicy::from_policy_snapshot_with_timing(
            snapshot,
            self.slot_length_ms,
            self.max_clock_skew_ms,
        ));
        self.axt_policy_snapshot = Some(snapshot.clone());
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = None;
        self
    }

    /// Install an AXT policy derived from a [`MockWorldStateView`] snapshot and current slot.
    pub fn with_wsv_policy(mut self, wsv: &MockWorldStateView) -> Self {
        let slot_length_ms = wsv.slot_length_ms();
        let max_clock_skew_ms = wsv.max_clock_skew_ms();
        self.slot_length_ms = slot_length_ms;
        self.max_clock_skew_ms = max_clock_skew_ms;
        let snapshot = wsv.axt_policy_snapshot();
        let has_explicit_slot = snapshot.values().any(|policy| policy.current_slot != 0);
        let mut policy = SpaceDirectoryAxtPolicy::from_snapshot_with_timing(
            snapshot,
            slot_length_ms,
            max_clock_skew_ms,
        );
        if !has_explicit_slot {
            policy = policy.with_current_slot(wsv.current_slot());
        }
        self.axt_policy = Arc::new(policy);
        self.axt_policy_snapshot = Some(wsv.axt_policy_snapshot_model());
        self.axt_proof_cache.clear();
        self.axt_proof_cache_slot = None;
        self
    }

    pub fn axt_cached_proof_status(&self, dsid: DataSpaceId) -> Option<(bool, Option<[u8; 32]>)> {
        self.axt_proof_cache
            .get(&dsid)
            .map(|entry| (entry.valid, entry.manifest_root))
    }

    pub fn axt_recorded_proof_payload(&self, dsid: DataSpaceId) -> Option<Vec<u8>> {
        self.axt_state
            .as_ref()
            .and_then(|state| state.proofs().get(&dsid))
            .map(|blob| blob.payload.clone())
    }

    /// Insert a raw Norito payload into durable state. Intended for unit tests.
    pub fn insert_state_value<P: AsRef<str>, B: AsRef<[u8]>>(&mut self, path: P, value: B) {
        self.state
            .set(path.as_ref(), value.as_ref().to_vec())
            .expect("insert state value");
    }

    /// Return a copy of all durable state paths currently stored in the host.
    pub fn state_paths(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.state.keys().cloned().collect();
        keys.sort();
        keys
    }

    fn state_key_matches_prefix(key: &str, prefix: &str) -> bool {
        key == prefix
            || key
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    fn state_keys_page_with_prefix(
        &self,
        vm: &IVM,
        prefix: &Name,
        path_len: usize,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Name>, u64, u64), VMError> {
        #[cfg(test)]
        self.state_scan_examined.store(0, Ordering::Relaxed);
        let prefix = prefix.as_ref();
        let take = checked_state_keys_limit(limit)?;
        let mut selected = Vec::new();
        let mut selected_element_bytes = 0_usize;
        let mut total = 0_u64;
        let mut scan_work_gas = u64::try_from(path_len).unwrap_or(u64::MAX);
        let mut response_tail_gas = crate::host::state_keys_prepare_minimum(path_len, limit)?
            .saturating_sub(crate::host::state_path_gas(path_len));
        for key in self.state.keys_with_text_prefix(prefix) {
            if key.len() > syscalls::STATE_MAX_PATH_BYTES {
                return Err(VMError::NoritoInvalid);
            }
            crate::host::preflight_reserved_state_scan_work_with_tail(
                vm,
                scan_work_gas,
                key.len(),
                response_tail_gas,
            )?;
            #[cfg(test)]
            self.state_scan_examined.fetch_add(1, Ordering::Relaxed);
            scan_work_gas = scan_work_gas
                .saturating_add(1)
                .saturating_add(u64::try_from(key.len()).unwrap_or(u64::MAX));
            if Self::state_key_matches_prefix(key, prefix) {
                if total >= offset && selected.len() < take {
                    let (next_elements, next_response_tail) =
                        crate::host::state_keys_response_tail_after_item(
                            selected.len(),
                            selected_element_bytes,
                            key,
                        )?;
                    preflight_reserved_syscall_gas(
                        vm,
                        STATE_QUERY_GAS_BASE
                            .saturating_add(scan_work_gas)
                            .saturating_add(u64::try_from(next_response_tail).unwrap_or(u64::MAX)),
                    )?;
                    selected_element_bytes = next_elements;
                    response_tail_gas = u64::try_from(next_response_tail).unwrap_or(u64::MAX);
                    selected.push(key.parse().map_err(|_| VMError::NoritoInvalid)?);
                }
                total = total.saturating_add(1);
            }
        }
        Ok((selected, total, scan_work_gas))
    }

    /// Borrow the raw Norito payload stored under `path`, if present.
    pub fn state_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.state
            .value_payload_ref(path)
            .ok()
            .flatten()
            .map(<[u8]>::to_vec)
    }

    fn log_read_key(&mut self, key: &str) {
        self.access_log.read_keys.insert(key.to_string());
    }

    fn log_write_key(&mut self, key: &str) {
        self.access_log.write_keys.insert(key.to_string());
        self.access_log.state_writes.push(StateUpdate {
            key: key.to_string(),
            value: 1,
        });
    }

    fn decode_name_payload(&self, payload: &[u8]) -> Result<Name, VMError> {
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_state_path_tlv(&self, vm: &IVM, pointer: u64) -> Result<(Name, usize), VMError> {
        if pointer == 0 {
            return Err(VMError::NoritoInvalid);
        }
        let tlv = self.decode_tlv(vm, pointer, PointerType::Name)?;
        let path_len = tlv.payload.len();
        if path_len > syscalls::STATE_MAX_PATH_BYTES {
            return Err(VMError::NoritoInvalid);
        }
        let path = self.decode_name_payload(tlv.payload)?;
        crate::host::validate_state_path_name(&path)?;
        crate::host::validate_declared_state_path(vm, &path)?;
        Ok((path, path_len))
    }

    fn decode_state_scan_path_tlv(&self, vm: &IVM, pointer: u64) -> Result<(Name, usize), VMError> {
        if pointer == 0 {
            return Err(VMError::NoritoInvalid);
        }
        let tlv = self.decode_tlv(vm, pointer, PointerType::Name)?;
        let path_len = tlv.payload.len();
        if path_len > syscalls::STATE_MAX_PATH_BYTES {
            return Err(VMError::NoritoInvalid);
        }
        let path = self.decode_name_payload(tlv.payload)?;
        crate::host::validate_state_path_name(&path)?;
        crate::host::validate_declared_state_scan_path(vm, &path)?;
        Ok((path, path_len))
    }

    fn policy_entry_for(&self, dsid: DataSpaceId) -> Option<AxtPolicyEntry> {
        self.axt_policy_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.entries.iter().find(|entry| entry.dsid == dsid))
            .map(|binding| binding.policy)
    }

    fn reset_axt_proof_cache_for_slot(&mut self, slot: Option<u64>) {
        let slot = slot.filter(|value| *value > 0);
        let should_clear = match (self.axt_proof_cache_slot, slot) {
            (Some(prev), Some(next)) => prev != next,
            (Some(_), None) => true,
            _ => false,
        };
        if should_clear {
            self.axt_proof_cache.clear();
        }
        self.axt_proof_cache_slot = slot;
    }

    fn axt_expiry_slot_with_skew(&self, expiry_slot: u64, override_ms: Option<u32>) -> u64 {
        axt::expiry_slot_with_skew(
            expiry_slot,
            self.slot_length_ms,
            self.max_clock_skew_ms,
            override_ms,
        )
    }

    fn max_policy_slot(&self) -> Option<u64> {
        self.axt_policy_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .entries
                .iter()
                .map(|binding| binding.policy.current_slot)
                .filter(|slot| *slot > 0)
                .max()
        })
    }

    fn cache_proof_entry(
        &mut self,
        dsid: DataSpaceId,
        digest: [u8; 32],
        expiry_slot: Option<u64>,
        verified_slot: Option<u64>,
        manifest_root: Option<[u8; 32]>,
        valid: bool,
    ) {
        self.axt_proof_cache.insert(
            dsid,
            CachedProofEntry {
                digest,
                expiry_slot,
                verified_slot: verified_slot.unwrap_or(0),
                manifest_root,
                valid,
            },
        );
        if let Some(slot) = verified_slot.filter(|value| *value > 0) {
            self.axt_proof_cache_slot = Some(slot);
        }
    }

    fn handle_axt_begin(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let tlv = vm.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::AxtDescriptor {
            return Err(VMError::NoritoInvalid);
        }
        let gas = Self::axt_gas(tlv.payload.len());
        let descriptor: axt::AxtDescriptor =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        axt::validate_descriptor(&descriptor)?;
        let binding = axt::compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        self.reset_axt_proof_cache_for_slot(self.max_policy_slot());
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        self.axt_active = true;
        Ok(gas)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let ds_tlv = vm.validate_tlv(vm.register(10))?;
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
            axt::TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            }
        } else {
            let manifest_tlv = vm.validate_tlv(manifest_ptr)?;
            if manifest_tlv.type_id != PointerType::NoritoBytes {
                return Err(VMError::NoritoInvalid);
            }
            gas_len = gas_len.saturating_add(manifest_tlv.payload.len());
            norito::decode_from_bytes(manifest_tlv.payload).map_err(|_| VMError::NoritoInvalid)?
        };
        self.axt_policy.allow_touch(dsid, &manifest)?;
        state.record_touch(dsid, manifest)?;
        Ok(Self::axt_gas(gas_len))
    }

    fn handle_axt_verify_ds_proof(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let Some(state_view) = self.axt_state.as_ref() else {
            return Err(VMError::PermissionDenied);
        };
        let ds_tlv = vm.validate_tlv(vm.register(10))?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
        let dsid: DataSpaceId =
            norito::decode_from_bytes(ds_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state_view.expected_dsids().contains(&dsid) {
            return Err(VMError::PermissionDenied);
        }
        let policy = self
            .policy_entry_for(dsid)
            .ok_or(VMError::PermissionDenied)?;
        self.reset_axt_proof_cache_for_slot(Some(policy.current_slot));
        if policy.manifest_root.iter().all(|byte| *byte == 0) {
            self.axt_proof_cache.remove(&dsid);
            return Err(VMError::PermissionDenied);
        }

        let proof_ptr = vm.register(11);
        if proof_ptr == 0 {
            let state = self.axt_state.as_mut().expect("axt_state checked above");
            state.record_proof(dsid, None, None)?;
            self.axt_proof_cache.remove(&dsid);
            return Ok(Self::verify_gas(0));
        }

        let proof_tlv = vm.validate_tlv(proof_ptr)?;
        if proof_tlv.type_id != PointerType::ProofBlob {
            return Err(VMError::NoritoInvalid);
        }
        let proof: axt::ProofBlob =
            norito::decode_from_bytes(proof_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if proof.payload.is_empty() || proof.expiry_slot == Some(0) {
            return Err(VMError::NoritoInvalid);
        }
        let expiry_with_skew = proof
            .expiry_slot
            .map(|slot| self.axt_expiry_slot_with_skew(slot, None));

        let digest: [u8; 32] = IrohaHash::new(&proof.payload).into();
        if let Some(entry) = self.axt_proof_cache.get(&dsid)
            && entry.digest == digest
            && entry.is_applicable_for_slot(Some(policy.current_slot), Some(policy.manifest_root))
        {
            return Err(VMError::PermissionDenied);
        }

        let envelope = match norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof.payload) {
            Ok(envelope) => envelope,
            Err(_) => {
                self.cache_proof_entry(
                    dsid,
                    digest,
                    expiry_with_skew,
                    Some(policy.current_slot),
                    Some(policy.manifest_root),
                    false,
                );
                return Err(VMError::NoritoInvalid);
            }
        };
        if let Some(expiry_slot) = expiry_with_skew
            && policy.current_slot > 0
            && policy.current_slot > expiry_slot
        {
            self.cache_proof_entry(
                dsid,
                digest,
                Some(expiry_slot),
                Some(policy.current_slot),
                Some(policy.manifest_root),
                false,
            );
            return Err(VMError::PermissionDenied);
        }

        if let Err(err) = axt::preflight_fastpq_v1_proof_envelope_for_manifest(
            &envelope,
            dsid,
            policy.manifest_root,
        ) {
            self.cache_proof_entry(
                dsid,
                digest,
                expiry_with_skew,
                Some(policy.current_slot),
                Some(envelope.manifest_root),
                false,
            );
            return Err(err);
        }

        self.cache_proof_entry(
            dsid,
            digest,
            expiry_with_skew,
            Some(policy.current_slot),
            Some(envelope.manifest_root),
            false,
        );
        // This standalone shim can preflight AXT FastPQ metadata, but it has no
        // FastPQ verifier callback. Do not turn preflight into acceptance.
        Err(VMError::PermissionDenied)
    }

    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        if self.axt_state.is_none() {
            return Err(VMError::PermissionDenied);
        }
        let handle_tlv = vm.validate_tlv(vm.register(10))?;
        if handle_tlv.type_id != PointerType::AssetHandle {
            return Err(VMError::NoritoInvalid);
        }
        let mut gas_len = handle_tlv.payload.len();
        let handle: axt::AssetHandle =
            norito::decode_from_bytes(handle_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;

        let intent_tlv = vm.validate_tlv(vm.register(11))?;
        if intent_tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        gas_len = gas_len.saturating_add(intent_tlv.payload.len());
        let intent: axt::RemoteSpendIntent =
            norito::decode_from_bytes(intent_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;

        let proof: Option<axt::ProofBlob> = match vm.register(12) {
            0 => None,
            ptr => {
                let proof_tlv = vm.validate_tlv(ptr)?;
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
        let resolved_amount = axt::resolve_handle_amount(&intent, proof.as_ref())
            .map_err(axt::HandleAmountResolutionError::to_vm_error)?;
        if resolved_amount.amount > handle.budget.remaining {
            return Err(VMError::PermissionDenied);
        }
        if let Some(per_use) = handle.budget.per_use.as_ref()
            && &resolved_amount.amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }

        if let Some(proof_blob) = proof.as_ref() {
            let policy = self
                .policy_entry_for(intent.asset_dsid)
                .ok_or(VMError::PermissionDenied)?;
            if proof_blob.payload.is_empty() || proof_blob.expiry_slot == Some(0) {
                return Err(VMError::NoritoInvalid);
            }
            let expiry_with_skew = proof_blob
                .expiry_slot
                .map(|slot| self.axt_expiry_slot_with_skew(slot, None));
            if let Some(expiry_slot) = expiry_with_skew
                && policy.current_slot > 0
                && policy.current_slot > expiry_slot
            {
                return Err(VMError::PermissionDenied);
            }
            let envelope = norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof_blob.payload)
                .map_err(|_| VMError::NoritoInvalid)?;
            axt::preflight_fastpq_v1_proof_envelope_for_manifest(
                &envelope,
                intent.asset_dsid,
                policy.manifest_root,
            )?;
            // This standalone shim cannot verify FastPQ proof contents.
            return Err(VMError::PermissionDenied);
        }

        let usage = axt::HandleUsage {
            handle,
            intent,
            proof,
            amount: resolved_amount.amount,
            amount_commitment: resolved_amount.amount_commitment,
        };
        self.axt_policy.allow_handle(&usage)?;
        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_handle(usage)?;
        Ok(Self::axt_gas(gas_len))
    }

    fn handle_axt_commit(&mut self, vm: &IVM) -> Result<u64, VMError> {
        let gas = self
            .axt_state
            .as_ref()
            .map(Self::axt_commit_gas)
            .ok_or(VMError::PermissionDenied)?;
        preflight_reserved_syscall_gas(vm, gas)?;
        self.axt_state
            .take()
            .expect("axt_state checked before gas preflight");
        self.axt_active = false;
        Ok(gas)
    }

    /// Attach a schema registry implementation.
    pub fn with_schema_registry(mut self, reg: Box<dyn SchemaRegistry + Send + Sync>) -> Self {
        self.schema = Arc::from(reg);
        self
    }

    /// Enable or disable SM helper syscalls when constructing the host.
    pub fn with_sm_enabled(mut self, enabled: bool) -> Self {
        self.sm_enabled = enabled;
        self
    }

    /// Set the trusted host time returned by `current_time_ms()`.
    pub fn with_current_time_ms(mut self, current_time_ms: u64) -> Self {
        self.current_time_ms = current_time_ms;
        self
    }

    /// Enable or disable SM helper syscalls on this host.
    pub fn set_sm_enabled(&mut self, enabled: bool) {
        self.sm_enabled = enabled;
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
        self.fastpq_batch_has_entries = true;
        Ok(Self::mutation_gas(0))
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::metered(
                gas::G_FASTPQ_BATCH,
                VMError::PermissionDenied,
            ));
        }
        if !self.fastpq_batch_has_entries {
            self.fastpq_batch_active = false;
            return Err(VMError::metered(gas::G_FASTPQ_BATCH, VMError::DecodeError));
        }
        self.fastpq_batch_active = false;
        self.fastpq_batch_has_entries = false;
        Ok(gas::G_FASTPQ_BATCH)
    }

    fn apply_fastpq_batch(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        let ptr = vm.register(10);
        let tlv = vm.validate_tlv(ptr)?;
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

    fn resolve_literal_pointer(vm: &IVM, src: usize) -> Option<usize> {
        let address = u64::try_from(src).ok()?;
        vm.is_validated_literal_pointer(address).then_some(src)
    }

    fn expect_tlv(vm: &IVM, reg: usize, ty: PointerType) -> Result<(), VMError> {
        let addr = Self::resolve_code_tlv_addr(vm, vm.register(reg));
        let tlv = vm.validate_tlv(addr)?;
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[CoreHost] reg r{reg} expect={:?} got={:?} addr=0x{addr:08x}",
                ty, tlv.type_id
            );
        }
        // Enforce exact type match for the syscall argument position
        if tlv.type_id != ty {
            return Err(VMError::NoritoInvalid);
        }
        Ok(())
    }

    fn expect_tlv_payload_len(vm: &IVM, reg: usize, ty: PointerType) -> Result<usize, VMError> {
        let addr = Self::resolve_code_tlv_addr(vm, vm.register(reg));
        Self::expect_tlv(vm, reg, ty)?;
        Ok(vm.validate_tlv(addr)?.payload.len())
    }

    fn expect_amount(vm: &IVM, reg: usize) -> Result<(), VMError> {
        let addr = Self::resolve_code_tlv_addr(vm, vm.register(reg));
        let tlv = vm.validate_tlv(addr)?;
        if tlv.type_id != PointerType::Quantity {
            return Err(VMError::NoritoInvalid);
        }
        QuantityValueV1::decode_frame(tlv.payload)
            .map(drop)
            .map_err(|_| VMError::DecodeError)
    }

    pub(crate) fn resolve_code_tlv_addr(vm: &IVM, addr: u64) -> u64 {
        let input_lo = Memory::INPUT_START;
        let input_hi = Memory::INPUT_START + Memory::INPUT_SIZE;
        if addr >= input_lo && addr < input_hi {
            return addr;
        }
        Self::resolve_literal_pointer(vm, addr as usize)
            .map(|resolved| resolved as u64)
            .unwrap_or(addr)
    }

    fn decode_tlv<'a>(
        &self,
        vm: &'a IVM,
        addr: u64,
        expected: PointerType,
    ) -> Result<pointer_abi::Tlv<'a>, VMError> {
        let resolved = Self::resolve_code_tlv_addr(vm, addr);
        let tlv = vm.validate_tlv(resolved)?;
        if tlv.type_id != expected {
            return Err(VMError::NoritoInvalid);
        }
        Ok(tlv)
    }

    fn norito_bytes_tlv(payload: &[u8]) -> Result<Vec<u8>, VMError> {
        let mut out = Vec::with_capacity(7 + payload.len() + IrohaHash::LENGTH);
        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        out.push(1);
        let len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; 32] = IrohaHash::new(payload).into();
        out.extend_from_slice(&h);
        Ok(out)
    }

    fn alloc_norito_bytes_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        vm.alloc_host_tlv(&Self::norito_bytes_tlv(payload)?)
    }

    fn alloc_blob_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        let mut out = Vec::with_capacity(7 + payload.len() + IrohaHash::LENGTH);
        out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
        out.push(1);
        let len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; IrohaHash::LENGTH] = IrohaHash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_host_tlv(&out)
    }

    fn blake2b256(payload: &[u8]) -> [u8; IrohaHash::LENGTH] {
        let mut digest = [0u8; IrohaHash::LENGTH];
        let mut hasher =
            Blake2bVar::new(IrohaHash::LENGTH).expect("32-byte Blake2b output size is supported");
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

    fn verify_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        VERIFY_GAS_BASE.saturating_add(VERIFY_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn byte_gas(base: u64, per_byte: u64, input_len: usize, output_len: usize) -> u64 {
        let bytes = u64::try_from(input_len)
            .unwrap_or(u64::MAX)
            .saturating_add(u64::try_from(output_len).unwrap_or(u64::MAX));
        base.saturating_add(per_byte.saturating_mul(bytes))
    }

    fn json_gas(input_len: usize, output_len: usize) -> u64 {
        Self::byte_gas(JSON_GAS_BASE, JSON_GAS_PER_BYTE, input_len, output_len)
    }

    fn name_decode_gas(input_len: usize, output_len: usize) -> u64 {
        Self::byte_gas(
            NAME_DECODE_GAS_BASE,
            NAME_DECODE_GAS_PER_BYTE,
            input_len,
            output_len,
        )
    }

    fn numeric_payload_gas(input_len: usize, output_len: usize) -> u64 {
        Self::byte_gas(NUMERIC_GAS, 1, input_len, output_len)
    }

    fn path_gas(input_len: usize, output_len: usize) -> u64 {
        Self::byte_gas(PATH_GAS_BASE, PATH_GAS_PER_BYTE, input_len, output_len)
    }

    fn schema_gas(input_len: usize, output_len: usize) -> u64 {
        Self::byte_gas(SCHEMA_GAS_BASE, SCHEMA_GAS_PER_BYTE, input_len, output_len)
    }

    fn sysvar_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        SYSVAR_GAS_BASE.saturating_add(SYSVAR_GAS_PER_BYTE.saturating_mul(bytes))
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

    /// Read only a pointer-ABI header and its declared extent for preparation.
    ///
    /// This deliberately does not decode the payload, verify its digest, or
    /// allocate an owned envelope. Full validation remains in the post-debit
    /// syscall path. Literal pointers are resolved without copying code.
    pub(crate) fn quote_codec_tlv_payload_len(
        vm: &IVM,
        register: usize,
        expected: PointerType,
        nullable: bool,
    ) -> Result<usize, VMError> {
        Self::quote_codec_tlv_payload_len_with_limit(
            vm,
            register,
            expected,
            nullable,
            gas::HOST_CODEC_MAX_INPUT_BYTES,
        )
    }

    fn quote_codec_tlv_payload_len_with_limit(
        vm: &IVM,
        register: usize,
        expected: PointerType,
        nullable: bool,
        maximum_payload_len: usize,
    ) -> Result<usize, VMError> {
        let pointer = vm.register(register);
        if pointer == 0 {
            return if nullable {
                Ok(0)
            } else {
                Err(VMError::NoritoInvalid)
            };
        }
        let payload_len = match quote_tlv_payload_len_at(vm, pointer, expected) {
            Ok(payload_len) => payload_len,
            Err(raw_error) => {
                let resolved = Self::resolve_code_tlv_addr(vm, pointer);
                if resolved == pointer {
                    return Err(raw_error);
                } else {
                    quote_tlv_payload_len_at(vm, resolved, expected)?
                }
            }
        };
        if payload_len > maximum_payload_len {
            return Err(VMError::NoritoInvalid);
        }
        Ok(payload_len)
    }

    fn maximum_host_output_payload() -> usize {
        gas::HOST_CODEC_MAX_OUTPUT_BYTES
    }

    fn maximum_host_pointer_output_payload() -> usize {
        usize::try_from(Memory::HEAP_SIZE.max(Memory::INPUT_SIZE))
            .unwrap_or(usize::MAX)
            .saturating_sub(TLV_ENVELOPE_OVERHEAD)
    }

    fn validate_codec_output_payload_len(payload_len: usize) -> Result<(), VMError> {
        if payload_len > gas::HOST_CODEC_MAX_OUTPUT_BYTES {
            return Err(VMError::NoritoInvalid);
        }
        Ok(())
    }

    /// Return a payload-length upper bound for the response-producing codec
    /// helpers without running the helper against a cloned VM.
    pub(crate) fn codec_gas_quote(number: u32, vm: &IVM) -> Result<Option<u64>, VMError> {
        let canonical = syscalls::canonical_helper_syscall(number);
        let maximum_output = Self::maximum_host_output_payload();
        let quote = match canonical {
            syscalls::SYSCALL_DECODE_INT => {
                let input =
                    Self::quote_codec_tlv_payload_len(vm, 10, PointerType::NoritoBytes, true)?;
                Self::numeric_payload_gas(input, 0)
            }
            syscalls::SYSCALL_ENCODE_INT => Self::numeric_payload_gas(0, 64),
            syscalls::SYSCALL_JSON_ENCODE => {
                let input = Self::quote_codec_tlv_payload_len(vm, 10, PointerType::Json, false)?;
                Self::json_gas(input, maximum_output)
            }
            syscalls::SYSCALL_JSON_DECODE => {
                let input =
                    Self::quote_codec_tlv_payload_len(vm, 10, PointerType::NoritoBytes, true)?;
                Self::json_gas(input, maximum_output)
            }
            syscalls::SYSCALL_JSON_OBJECT => Self::json_gas(0, maximum_output),
            syscalls::SYSCALL_JSON_SET_I64 | syscalls::SYSCALL_JSON_SET_ACCOUNT_ID => {
                let json = Self::quote_codec_tlv_payload_len(vm, 10, PointerType::Json, false)?;
                let key = Self::quote_codec_tlv_payload_len(vm, 11, PointerType::Name, false)?;
                let value = if canonical == syscalls::SYSCALL_JSON_SET_ACCOUNT_ID {
                    Self::quote_codec_tlv_payload_len(vm, 12, PointerType::AccountId, false)?
                } else {
                    core::mem::size_of::<i64>()
                };
                Self::json_gas(
                    json.saturating_add(key).saturating_add(value),
                    maximum_output,
                )
            }
            syscalls::SYSCALL_JSON_GET_JSON
            | syscalls::SYSCALL_JSON_GET_NAME
            | syscalls::SYSCALL_JSON_GET_ACCOUNT_ID
            | syscalls::SYSCALL_JSON_GET_NFT_ID
            | syscalls::SYSCALL_JSON_GET_BLOB_HEX
            | syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID
            | syscalls::SYSCALL_JSON_GET_INT
            | syscalls::SYSCALL_JSON_GET_DECIMAL
            | syscalls::SYSCALL_JSON_GET_QUANTITY => {
                let output_bound = if canonical == syscalls::SYSCALL_JSON_GET_JSON {
                    Self::maximum_host_pointer_output_payload()
                } else {
                    maximum_output
                };
                let json = Self::quote_codec_tlv_payload_len_with_limit(
                    vm,
                    10,
                    PointerType::Json,
                    false,
                    if canonical == syscalls::SYSCALL_JSON_GET_JSON {
                        output_bound
                    } else {
                        gas::HOST_CODEC_MAX_INPUT_BYTES
                    },
                )?;
                let key = Self::quote_codec_tlv_payload_len(vm, 11, PointerType::Name, false)?;
                Self::json_gas(json.saturating_add(key), output_bound.saturating_add(16))
            }
            syscalls::SYSCALL_NAME_DECODE => {
                let input =
                    Self::quote_codec_tlv_payload_len(vm, 10, PointerType::NoritoBytes, true)?;
                Self::name_decode_gas(input, maximum_output)
            }
            _ => return Ok(None),
        };
        Ok(Some(quote))
    }

    pub(crate) fn schema_gas_quote(number: u32, vm: &IVM) -> Result<Option<u64>, VMError> {
        let (value_type, has_value) = match number {
            syscalls::SYSCALL_SCHEMA_ENCODE | syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT => {
                (PointerType::Json, true)
            }
            syscalls::SYSCALL_SCHEMA_DECODE | syscalls::SYSCALL_SCHEMA_DECODE_DIRECT => {
                (PointerType::NoritoBytes, true)
            }
            syscalls::SYSCALL_SCHEMA_INFO | syscalls::SYSCALL_SCHEMA_INFO_DIRECT => {
                (PointerType::NoritoBytes, false)
            }
            _ => return Ok(None),
        };
        let schema = Self::quote_codec_tlv_payload_len(vm, 10, PointerType::Name, false)?;
        let value = if has_value {
            Self::quote_codec_tlv_payload_len(vm, 11, value_type, false)?
        } else {
            0
        };
        Ok(Some(Self::schema_gas(
            schema.saturating_add(value),
            Self::maximum_host_output_payload(),
        )))
    }

    fn input_publish_gas(envelope_len: usize) -> u64 {
        let bytes = u64::try_from(envelope_len).unwrap_or(u64::MAX);
        INPUT_PUBLISH_GAS_BASE.saturating_add(INPUT_PUBLISH_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn mutation_gas(payload_len: usize) -> u64 {
        let bytes = u64::try_from(payload_len).unwrap_or(u64::MAX);
        MUTATION_GAS.saturating_add(MUTATION_GAS_PER_BYTE.saturating_mul(bytes))
    }

    fn quote_canonical_state_map_path(vm: &IVM) -> Result<u64, VMError> {
        let base_len = quote_tlv_payload_len_at(
            vm,
            Self::resolve_code_tlv_addr(vm, vm.register(10)),
            PointerType::Name,
        )?;
        let key_len = quote_tlv_payload_len_at(
            vm,
            Self::resolve_code_tlv_addr(vm, vm.register(11)),
            PointerType::NoritoBytes,
        )?;
        let (input_len, output_bound) = quote_canonical_state_map_path_lengths(base_len, key_len)?;
        Ok(Self::path_gas(input_len, output_bound))
    }

    fn quote_state_map_key_at(vm: &IVM) -> Result<u64, VMError> {
        let page_len = quote_tlv_payload_len_at(
            vm,
            Self::resolve_code_tlv_addr(vm, vm.register(10)),
            PointerType::NoritoBytes,
        )?;
        let base_len = quote_tlv_payload_len_at(
            vm,
            Self::resolve_code_tlv_addr(vm, vm.register(11)),
            PointerType::Name,
        )?;
        if page_len > syscalls::STATE_MAP_MAX_PAGE_BYTES
            || base_len > syscalls::STATE_MAP_MAX_BASE_BYTES
        {
            return Err(VMError::NoritoInvalid);
        }
        Ok(Self::path_gas(
            page_len.saturating_add(base_len),
            syscalls::STATE_MAP_MAX_KEY_BYTES,
        ))
    }
}

// Provide a Default impl to satisfy clippy::new_without_default without changing API.
impl Default for CoreHost {
    fn default() -> Self {
        Self::new()
    }
}

impl IVMHost for CoreHost {
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError> {
        let metering = require_host_syscall_metering_spec(vm.syscall_policy(), number)?;
        if metering.metering == crate::syscall_metering::SyscallMetering::Staged {
            return Ok(0);
        }
        if is_sm_syscall(number) && !self.sm_enabled {
            return Ok(0);
        }
        let quote = match number {
            syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | syscalls::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT => {
                Self::quote_canonical_state_map_path(vm)?
            }
            syscalls::SYSCALL_STATE_MAP_KEY_AT => Self::quote_state_map_key_at(vm)?,
            syscalls::SYSCALL_STATE_GET => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                crate::host::state_get_gas_quote(path_len)
            }
            syscalls::SYSCALL_STATE_LEN => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                crate::host::state_path_gas(path_len)
            }
            syscalls::SYSCALL_STATE_COUNT => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                reserve_available_syscall_gas_at_least(vm, crate::host::state_path_gas(path_len))?
            }
            syscalls::SYSCALL_STATE_KEYS => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                let minimum = crate::host::state_keys_prepare_minimum(path_len, vm.register(12))?;
                reserve_available_syscall_gas_at_least(vm, minimum)?
            }
            syscalls::SYSCALL_STATE_SET => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                let value_len = quote_tlv_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(11)),
                    PointerType::NoritoBytes,
                )?;
                crate::host::validate_state_value_payload_len(value_len)?;
                crate::host::state_value_gas(path_len, value_len)
            }
            syscalls::SYSCALL_STATE_DEL | syscalls::SYSCALL_STATE_HAS => {
                let path_len = crate::host::quote_state_path_payload_len_at(
                    vm,
                    Self::resolve_code_tlv_addr(vm, vm.register(10)),
                )?;
                crate::host::state_path_gas(path_len)
            }
            syscalls::SYSCALL_CORE_QUERY_GET | syscalls::SYSCALL_CORE_QUERY_PAGE => {
                STATE_QUERY_GAS_BASE
            }
            syscalls::SYSCALL_JSON_BUILD => {
                reserve_available_syscall_gas_at_least(vm, JSON_GAS_BASE)?
            }
            syscalls::SYSCALL_SCHEMA_ENCODE
            | syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT
            | syscalls::SYSCALL_SCHEMA_DECODE
            | syscalls::SYSCALL_SCHEMA_DECODE_DIRECT
            | syscalls::SYSCALL_SCHEMA_INFO
            | syscalls::SYSCALL_SCHEMA_INFO_DIRECT => {
                Self::schema_gas_quote(number, vm)?.ok_or(VMError::UnknownSyscall(number))?
            }
            syscalls::SYSCALL_ALLOC => crate::host::allocation_gas(vm.register(10)),
            syscalls::SYSCALL_AXT_COMMIT => {
                reserve_available_syscall_gas_at_least(vm, metering.minimum_gas)?
            }
            _ => {
                if let Some(quote) = common_syscall_gas_quote(number, vm)? {
                    quote
                } else if let Some(quote) = Self::codec_gas_quote(number, vm)? {
                    quote
                } else if metering.quote_strategy
                    == crate::host::HostSyscallQuoteStrategy::ReserveAvailable
                {
                    reserve_available_syscall_gas_at_least(vm, metering.minimum_gas)?
                } else {
                    // Stateful policy hooks and proof paths keep the generic
                    // deterministic bound; response-producing codec helpers
                    // above use header-only payload bounds.
                    conservative_syscall_gas_quote(number, vm)
                }
            }
        };
        Ok(quote)
    }

    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if crate::dev_env::decode_trace_enabled() {
            eprintln!("[CoreHost] syscall number=0x{number:02x}");
        }
        // Enforce both ABI policy and exhaustive metering classification for
        // direct host calls as well as VM-dispatched execution.
        require_host_syscall_metering_spec(vm.syscall_policy(), number)?;
        if crate::syscalls::is_numeric_v1_syscall(number) {
            return crate::numeric_v1::execute(number, vm);
        }
        let canonical = crate::syscalls::canonical_helper_syscall(number);
        if crate::syscalls::is_json_getter_syscall(canonical) {
            let cost = crate::json::typed_getter(vm, number, Self::resolve_code_tlv_addr)?;
            return Ok(crate::json::typed_getter_gas(
                cost.input_bytes,
                cost.output_bytes,
            ));
        }
        if number == crate::syscalls::SYSCALL_JSON_BUILD {
            return crate::json::build_json(vm, Self::resolve_code_tlv_addr);
        }
        match number {
            syscalls::SYSCALL_CORE_QUERY_GET | syscalls::SYSCALL_CORE_QUERY_PAGE => Err(
                VMError::metered_not_implemented(STATE_QUERY_GAS_BASE, number),
            ),
            // Durable state: pointer-ABI paths (Name) and NoritoBytes values
            syscalls::SYSCALL_STATE_GET => {
                // r10 = &Name path; return r10 = &NoritoBytes in host-owned public memory (or 0).
                let (path, path_len) = self.decode_state_path_tlv(vm, vm.register(10))?;
                if let Some(stored) = self.state.value_payload_ref(path.as_ref())? {
                    let gas = crate::host::state_value_gas(path_len, stored.len());
                    preflight_reserved_syscall_gas(vm, gas)?;
                    crate::host::validate_declared_state_value_payload(vm, &path, stored)?;
                    let val = stored.to_vec();
                    self.log_read_key(path.as_ref());
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!(
                            "[CoreHost] STATE_GET path='{}' hit bytes={}",
                            path,
                            val.len()
                        );
                    }
                    let mut buf = Vec::with_capacity(7 + val.len() + 32);
                    buf.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    buf.push(1);
                    buf.extend_from_slice(&(val.len() as u32).to_be_bytes());
                    buf.extend_from_slice(&val);
                    let h: [u8; 32] = IrohaHash::new(&val).into();
                    buf.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&buf)?;
                    vm.set_register(10, p);
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] STATE_GET returned r10=0x{p:08x}");
                    }
                    Ok(gas)
                } else {
                    let gas = crate::host::state_path_gas(path_len);
                    preflight_reserved_syscall_gas(vm, gas)?;
                    self.log_read_key(path.as_ref());
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] STATE_GET path='{path}' miss");
                    }
                    vm.set_register(10, 0);
                    Ok(gas)
                }
            }
            syscalls::SYSCALL_STATE_SET => {
                // r10 = &Name path; r11 = &NoritoBytes value
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[CoreHost] STATE_SET args r8=0x{a:08x} r9=0x{b:08x} r10=0x{path:08x} r11=0x{val:08x} r12=0x{aux:08x} r13=0x{aux2:08x}",
                        a = vm.register(8),
                        b = vm.register(9),
                        path = vm.register(10),
                        val = vm.register(11),
                        aux = vm.register(12),
                        aux2 = vm.register(13)
                    );
                }
                let path_ptr = vm.register(10);
                let val_ptr = vm.register(11);
                if path_ptr == 0 || val_ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let (path, path_len) = self.decode_state_path_tlv(vm, path_ptr)?;
                let p_val = self.decode_tlv(vm, val_ptr, PointerType::NoritoBytes)?;
                crate::host::validate_state_value_payload_len(p_val.payload.len())?;
                crate::host::validate_declared_state_value_payload(vm, &path, p_val.payload)?;
                self.state.set(path.as_ref(), p_val.payload.to_vec())?;
                self.log_write_key(path.as_ref());
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[CoreHost] STATE_SET path='{path}' bytes={}",
                        p_val.payload.len()
                    );
                }
                Ok(crate::host::state_value_gas(path_len, p_val.payload.len()))
            }
            syscalls::SYSCALL_STATE_DEL => {
                // r10 = &Name path
                let (path, path_len) = self.decode_state_path_tlv(vm, vm.register(10))?;
                let gas = crate::host::state_path_gas(path_len);
                self.state.del(path.as_ref())?;
                self.log_write_key(path.as_ref());
                Ok(gas)
            }
            syscalls::SYSCALL_STATE_KEYS => {
                let (prefix, path_len) = self.decode_state_scan_path_tlv(vm, vm.register(10))?;
                let (selected, total, scan_work_gas) = self.state_keys_page_with_prefix(
                    vm,
                    &prefix,
                    path_len,
                    vm.register(11),
                    vm.register(12),
                )?;
                preflight_reserved_state_keys_page(
                    vm,
                    &selected,
                    scan_work_gas,
                    0,
                    u64::try_from(selected.len()).unwrap_or(u64::MAX),
                )?;
                self.log_read_key(prefix.as_ref());
                let payload = to_bytes(&selected).map_err(|_| VMError::NoritoInvalid)?;
                let gas = STATE_QUERY_GAS_BASE
                    .saturating_add(scan_work_gas)
                    .saturating_add(u64::try_from(payload.len()).unwrap_or(u64::MAX));
                preflight_reserved_syscall_gas(vm, gas)?;
                let out = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, out);
                vm.set_register(11, total);
                vm.set_register(12, u64::try_from(selected.len()).unwrap_or(u64::MAX));
                Ok(gas)
            }
            syscalls::SYSCALL_STATE_HAS => {
                let (path, path_len) = self.decode_state_path_tlv(vm, vm.register(10))?;
                let present = self.state.get_ref(path.as_ref()).is_some();
                self.log_read_key(path.as_ref());
                vm.set_register(10, u64::from(present));
                Ok(crate::host::state_path_gas(path_len))
            }
            syscalls::SYSCALL_STATE_LEN => {
                let (path, path_len) = self.decode_state_path_tlv(vm, vm.register(10))?;
                let stored_len = self
                    .state
                    .value_payload_ref(path.as_ref())?
                    .map(<[u8]>::len);
                if let Some(stored_len) = stored_len {
                    let gas = crate::host::state_path_gas(path_len);
                    preflight_reserved_syscall_gas(vm, gas)?;
                    self.log_read_key(path.as_ref());
                    vm.set_register(10, u64::try_from(stored_len).unwrap_or(u64::MAX));
                    vm.set_register(11, 1);
                    Ok(gas)
                } else {
                    let gas = crate::host::state_path_gas(path_len);
                    preflight_reserved_syscall_gas(vm, gas)?;
                    self.log_read_key(path.as_ref());
                    vm.set_register(10, 0);
                    vm.set_register(11, 0);
                    Ok(gas)
                }
            }
            syscalls::SYSCALL_STATE_COUNT => {
                let (prefix, path_len) = self.decode_state_scan_path_tlv(vm, vm.register(10))?;
                let (_, total, scan_work_gas) =
                    self.state_keys_page_with_prefix(vm, &prefix, path_len, u64::MAX, 0)?;
                let gas = STATE_QUERY_GAS_BASE.saturating_add(scan_work_gas);
                preflight_reserved_syscall_gas(vm, gas)?;
                self.log_read_key(prefix.as_ref());
                vm.set_register(10, total);
                Ok(gas)
            }
            syscalls::SYSCALL_ALLOC => {
                // r10 = number of bytes to allocate on the VM heap.
                let size = vm.register(10);
                let addr = vm.alloc_heap(size)?;
                vm.set_register(10, addr);
                Ok(crate::host::allocation_gas(size))
            }
            syscalls::SYSCALL_DECODE_INT => {
                // r10 = &NoritoBytes (Norito-framed i64) -> r10 = parsed i64
                let addr = vm.register(10);
                if addr == 0 {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] DECODE_INT addr=0 (treat as zero)");
                    }
                    vm.set_register(10, 0);
                    return Ok(Self::numeric_payload_gas(0, 0));
                }
                let tlv = vm.validate_tlv(addr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                // Enforce ABI policy allows the input pointer type.
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let input_len = tlv.payload.len();
                let val: i64 = decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                vm.set_register(10, val as u64);
                Ok(Self::numeric_payload_gas(input_len, 0))
            }
            syscalls::SYSCALL_ENCODE_INT => {
                // r10 = value (i64) -> r10 = &NoritoBytes (Norito-framed i64)
                let val = vm.register(10) as i64;
                let body = crate::host::canonical_norito_bytes(&val)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::numeric_payload_gas(0, body.len()))
            }
            syscalls::SYSCALL_BUILD_PATH_KEY_NORITO
            | syscalls::SYSCALL_BUILD_PATH_KEY_NORITO_DIRECT => {
                // r10 = &Name base; r11 = &NoritoBytes key
                // -> r10 = &Name("<base>/<lowercase hex(canonical key)>")
                let base_tlv = self.decode_tlv(vm, vm.register(10), PointerType::Name)?;
                let key_tlv = self.decode_tlv(vm, vm.register(11), PointerType::NoritoBytes)?;
                let base_name = self.decode_name_payload(base_tlv.payload)?;
                let input_len = base_tlv.payload.len().saturating_add(key_tlv.payload.len());
                let path_name = canonical_typed_state_map_path(vm, &base_name, key_tlv.payload)?;
                let body = to_bytes(&path_name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let hh: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&hh);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::path_gas(input_len, body.len()))
            }
            syscalls::SYSCALL_STATE_MAP_KEY_AT => {
                let page = self.decode_tlv(vm, vm.register(10), PointerType::NoritoBytes)?;
                let base_tlv = self.decode_tlv(vm, vm.register(11), PointerType::Name)?;
                let base = self.decode_name_payload(base_tlv.payload)?;
                validate_declared_state_map_base(vm, &base)?;
                let key = canonical_state_map_key_at(page.payload, &base, vm.register(12))?;
                if let Some(key) = key.as_deref() {
                    validate_declared_state_map_key(vm, &base, key)?;
                }
                let gas = Self::path_gas(
                    page.payload.len().saturating_add(base_tlv.payload.len()),
                    key.as_ref().map_or(0, Vec::len),
                );
                if let Some(key) = key {
                    let ptr = Self::alloc_norito_bytes_tlv(vm, &key)?;
                    vm.set_register(10, ptr);
                } else {
                    vm.set_register(10, 0);
                }
                Ok(gas)
            }
            syscalls::SYSCALL_JSON_ENCODE => {
                // r10 = &Json (Norito-framed) -> r10 = &NoritoBytes (same payload)
                let r10_before = vm.register(10);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_ENCODE enter r10=0x{r10_before:08x}");
                }
                let tlv = vm.validate_tlv(r10_before)?;
                if tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let input_len = tlv.payload.len();
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_ENCODE exit r10=0x{p:08x}");
                }
                Ok(Self::json_gas(input_len, body.len()))
            }
            syscalls::SYSCALL_JSON_DECODE => {
                // r10 = &NoritoBytes(canonical Json frame) -> r10 = &Json
                let r10_before = vm.register(10);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_DECODE enter r10=0x{r10_before:08x}");
                }
                if r10_before == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::json_gas(0, 0));
                }
                let tlv = self.decode_tlv(vm, r10_before, PointerType::NoritoBytes)?;
                let input_len = tlv.payload.len();
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_DECODE exit r10=0x{p:08x}");
                }
                Ok(Self::json_gas(input_len, body.len()))
            }
            syscalls::SYSCALL_JSON_OBJECT => {
                let out_json = Json::from(njson::Value::Object(njson::Map::new()));
                let body = to_bytes(&out_json).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::json_gas(0, body.len()))
            }
            syscalls::SYSCALL_JSON_SET_I64
            | syscalls::SYSCALL_JSON_SET_ACCOUNT_ID
            | syscalls::SYSCALL_JSON_SET_I64_DIRECT
            | syscalls::SYSCALL_JSON_SET_ACCOUNT_ID_DIRECT => {
                let direct = number != syscalls::canonical_helper_syscall(number);
                let json_tlv = if direct {
                    self.decode_tlv(vm, vm.register(10), PointerType::Json)?
                } else {
                    let json_tlv = vm.validate_tlv(vm.register(10))?;
                    if json_tlv.type_id != PointerType::Json {
                        return Err(VMError::NoritoInvalid);
                    }
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, json_tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: json_tlv.type_id as u16,
                        });
                    }
                    json_tlv
                };
                let key_tlv = if direct {
                    self.decode_tlv(vm, vm.register(11), PointerType::Name)?
                } else {
                    let key_tlv = vm.validate_tlv(vm.register(11))?;
                    if key_tlv.type_id != PointerType::Name {
                        return Err(VMError::NoritoInvalid);
                    }
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, key_tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: key_tlv.type_id as u16,
                        });
                    }
                    key_tlv
                };

                let json: Json =
                    decode_from_bytes(json_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let mut input_len = json_tlv.payload.len().saturating_add(key_tlv.payload.len());
                let value: njson::Value = json
                    .try_into_any_norito()
                    .map_err(|_| VMError::DecodeError)?;
                let mut obj = match value {
                    njson::Value::Object(map) => map,
                    _ => return Err(VMError::DecodeError),
                };
                let key_name: Name =
                    decode_from_bytes(key_tlv.payload).map_err(|_| VMError::DecodeError)?;

                let field = match syscalls::canonical_helper_syscall(number) {
                    syscalls::SYSCALL_JSON_SET_I64 => {
                        input_len = input_len.saturating_add(core::mem::size_of::<i64>());
                        njson::Value::from(vm.register(12) as i64)
                    }
                    syscalls::SYSCALL_JSON_SET_ACCOUNT_ID => {
                        let value_tlv = if direct {
                            self.decode_tlv(vm, vm.register(12), PointerType::AccountId)?
                        } else {
                            let value_tlv = vm.validate_tlv(vm.register(12))?;
                            if value_tlv.type_id != PointerType::AccountId {
                                return Err(VMError::NoritoInvalid);
                            }
                            let policy = vm.syscall_policy();
                            if !pointer_abi::is_type_allowed_for_policy(policy, value_tlv.type_id) {
                                return Err(VMError::AbiTypeNotAllowed {
                                    abi: vm.abi_version(),
                                    type_id: value_tlv.type_id as u16,
                                });
                            }
                            value_tlv
                        };
                        input_len = input_len.saturating_add(value_tlv.payload.len());
                        let account: AccountId = decode_from_bytes(value_tlv.payload)
                            .map_err(|_| VMError::DecodeError)?;
                        njson::Value::from(account.to_string())
                    }
                    _ => return Err(VMError::UnknownSyscall(number)),
                };

                obj.insert(key_name.to_string(), field);
                let out_json = Json::from(njson::Value::Object(obj));
                let body = to_bytes(&out_json).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::json_gas(input_len, body.len()))
            }
            syscalls::SYSCALL_TLV_LEN => {
                // r10 = &TLV -> r10 = payload length
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::tlv_len_gas(0));
                }
                let tlv = vm.validate_tlv(addr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let payload_len = tlv.payload.len();
                vm.set_register(10, payload_len as u64);
                Ok(Self::tlv_len_gas(payload_len))
            }
            syscalls::SYSCALL_DECODE_ARGUMENT_RECORD => {
                crate::argument_record::decode_argument_record(vm)
            }
            syscalls::SYSCALL_STATE_VALUE_ENCODE => {
                crate::state_value_runtime::encode_state_value(vm, Self::resolve_code_tlv_addr)
            }
            syscalls::SYSCALL_STATE_VALUE_DECODE => {
                crate::state_value_runtime::decode_state_value(vm, Self::resolve_code_tlv_addr)
            }
            syscalls::SYSCALL_NORMALIZE_NORITO_BYTES => crate::host::normalize_norito_bytes(vm),
            syscalls::SYSCALL_SCHEMA_ENCODE | syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT => {
                // r10 = &Name schema; r11 = &Json -> r10 = &NoritoBytes (schema-typed)
                let direct = number == syscalls::SYSCALL_SCHEMA_ENCODE_DIRECT;
                let s_tlv = if direct {
                    self.decode_tlv(vm, vm.register(10), PointerType::Name)?
                } else {
                    let tlv = vm.validate_tlv(vm.register(10))?;
                    if tlv.type_id != PointerType::Name {
                        return Err(VMError::NoritoInvalid);
                    }
                    tlv
                };
                let v_tlv = if direct {
                    self.decode_tlv(vm, vm.register(11), PointerType::Json)?
                } else {
                    let tlv = vm.validate_tlv(vm.register(11))?;
                    if tlv.type_id != PointerType::Json {
                        return Err(VMError::NoritoInvalid);
                    }
                    tlv
                };
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                let json: Json =
                    decode_from_bytes(v_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let input_len = v_tlv.payload.len();
                if let Some(bytes) = self.schema.encode_json(&schema, json.get().as_bytes()) {
                    if crate::dev_env::decode_trace_enabled() {
                        // Try immediate roundtrip for known schemas to validate encoding
                        let roundtrip_ok = match schema.as_str() {
                            "Order" => {
                                #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                                struct Order {
                                    qty: i64,
                                    side: String,
                                }
                                norito::decode_from_bytes::<Order>(&bytes).is_ok()
                            }
                            "OrderByTime" => {
                                #[derive(norito::Decode, norito::Encode, Clone, Debug)]
                                struct OrderByTime {
                                    qty: i64,
                                    side: String,
                                    tif: u32,
                                }
                                norito::decode_from_bytes::<OrderByTime>(&bytes).is_ok()
                            }
                            _ => true,
                        };
                        eprintln!(
                            "[CoreHost] SCHEMA_ENCODE immediate_roundtrip schema={schema} ok={roundtrip_ok} len={len}",
                            len = bytes.len()
                        );
                    }
                    Self::validate_codec_output_payload_len(bytes.len())?;
                    let gas = Self::schema_gas(input_len, bytes.len());
                    preflight_reserved_syscall_gas(vm, gas)?;
                    let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                    out.extend_from_slice(&bytes);
                    let h: [u8; 32] = IrohaHash::new(&bytes).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
                    vm.set_register(10, p);
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!(
                            "[CoreHost] SCHEMA_ENCODE exit r10=0x{p:08x} bytes={len}",
                            len = bytes.len()
                        );
                    }
                    Ok(gas)
                } else {
                    Err(VMError::NoritoInvalid)
                }
            }
            syscalls::SYSCALL_SCHEMA_DECODE | syscalls::SYSCALL_SCHEMA_DECODE_DIRECT => {
                // r10 = &Name schema; r11 = &NoritoBytes -> r10 = &Json (Norito-framed)
                let direct = number == syscalls::SYSCALL_SCHEMA_DECODE_DIRECT;
                let s_tlv = if direct {
                    self.decode_tlv(vm, vm.register(10), PointerType::Name)?
                } else {
                    let s_tlv = vm.validate_tlv(vm.register(10))?;
                    if s_tlv.type_id != PointerType::Name {
                        return Err(VMError::NoritoInvalid);
                    }
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, s_tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: s_tlv.type_id as u16,
                        });
                    }
                    s_tlv
                };
                let b_tlv = if direct {
                    self.decode_tlv(vm, vm.register(11), PointerType::NoritoBytes)?
                } else {
                    let b_tlv = vm.validate_tlv(vm.register(11))?;
                    if b_tlv.type_id != PointerType::NoritoBytes {
                        return Err(VMError::NoritoInvalid);
                    }
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, b_tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: b_tlv.type_id as u16,
                        });
                    }
                    b_tlv
                };
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                let input_len = b_tlv.payload.len();
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[CoreHost] SCHEMA_DECODE enter schema={} b_len={len}",
                        schema,
                        len = input_len
                    );
                }
                if let Some(min) = self.schema.decode_to_json(&schema, b_tlv.payload) {
                    let json_str =
                        core::str::from_utf8(&min).map_err(|_| VMError::NoritoInvalid)?;
                    let json =
                        Json::from_str_norito(json_str).map_err(|_| VMError::NoritoInvalid)?;
                    let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                    Self::validate_codec_output_payload_len(body.len())?;
                    let gas = Self::schema_gas(input_len, body.len());
                    preflight_reserved_syscall_gas(vm, gas)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = IrohaHash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_host_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(gas)
                } else {
                    Err(VMError::NoritoInvalid)
                }
            }
            syscalls::SYSCALL_SCHEMA_INFO | syscalls::SYSCALL_SCHEMA_INFO_DIRECT => {
                // r10 = &Name (base or exact) -> r10 = &Json {current: {name,id,version}, versions:[{name,id,version}...]}
                let tlv = if number == syscalls::SYSCALL_SCHEMA_INFO_DIRECT {
                    self.decode_tlv(vm, vm.register(10), PointerType::Name)?
                } else {
                    let tlv = vm.validate_tlv(vm.register(10))?;
                    if tlv.type_id != PointerType::Name {
                        return Err(VMError::NoritoInvalid);
                    }
                    tlv
                };
                let input_len = tlv.payload.len();
                let name = self.decode_name_payload(tlv.payload)?;
                let raw = name.as_ref();
                let family = self
                    .schema
                    .resolve_family(raw)
                    .ok_or(VMError::NoritoInvalid)?;
                let (cur_name, cur_info) =
                    self.schema.current(&family).ok_or(VMError::NoritoInvalid)?;
                let list = self
                    .schema
                    .list_versions(&family)
                    .ok_or(VMError::NoritoInvalid)?;
                let current = {
                    let mut map = njson::Map::new();
                    map.insert("name".to_owned(), njson::Value::from(cur_name));
                    map.insert(
                        "id".to_owned(),
                        njson::Value::from(hex::encode(cur_info.id)),
                    );
                    map.insert("version".to_owned(), njson::Value::from(cur_info.version));
                    njson::Value::Object(map)
                };
                let mut vers = Vec::new();
                for (n, i) in list {
                    let mut map = njson::Map::new();
                    map.insert("name".to_owned(), njson::Value::from(n));
                    map.insert("id".to_owned(), njson::Value::from(hex::encode(i.id)));
                    map.insert("version".to_owned(), njson::Value::from(i.version));
                    vers.push(njson::Value::Object(map));
                }
                let body_value = {
                    let mut map = njson::Map::new();
                    map.insert("current".to_owned(), current);
                    map.insert("versions".to_owned(), njson::Value::Array(vers));
                    njson::Value::Object(map)
                };
                let json = Json::from(&body_value);
                let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let gas = Self::schema_gas(input_len, body.len());
                preflight_reserved_syscall_gas(vm, gas)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(gas)
            }
            syscalls::SYSCALL_NAME_DECODE => {
                // r10 = &NoritoBytes(canonical Name) -> r10 = &Name
                let r10_before = vm.register(10);
                if r10_before == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::name_decode_gas(0, 0));
                }
                let tlv = vm.validate_tlv(r10_before)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                // Enforce ABI policy allows NoritoBytes as input
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let input_len = tlv.payload.len();
                let name: Name =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                if to_bytes(&name)
                    .map_err(|_| VMError::DecodeError)?
                    .as_slice()
                    != tlv.payload
                {
                    return Err(VMError::DecodeError);
                }
                // Build a host-owned Name TLV using the normalized form.
                let body = to_bytes(&name).map_err(|_| VMError::NoritoInvalid)?;
                Self::validate_codec_output_payload_len(body.len())?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::name_decode_gas(input_len, body.len()))
            }
            syscalls::SYSCALL_POINTER_TO_NORITO => {
                let original = vm.register(10);
                if original == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let ptr = Self::resolve_code_tlv_addr(vm, original);
                let tlv = vm.validate_tlv(ptr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let mut body = Vec::with_capacity(2 + 1 + 4 + tlv.payload.len() + 32);
                body.extend_from_slice(&(tlv.type_id_raw().to_be_bytes()));
                body.push(tlv.version);
                body.extend_from_slice(&(tlv.payload.len() as u32).to_be_bytes());
                body.extend_from_slice(tlv.payload);
                let inner_hash: [u8; 32] = iroha_crypto::Hash::new(tlv.payload).into();
                body.extend_from_slice(&inner_hash);
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::pointer_gas(body.len()))
            }
            syscalls::SYSCALL_POINTER_FROM_NORITO => {
                if vm.register(10) == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::pointer_gas(0));
                }
                let ptr = vm.register(10);
                let tlv = self.decode_tlv(vm, ptr, PointerType::NoritoBytes)?;
                let encoded_len = tlv.payload.len();
                let policy = vm.syscall_policy();
                let (inner_type, inner_version, inner_payload) = {
                    let inner = pointer_abi::validate_tlv_bytes(tlv.payload)?;
                    (inner.type_id, inner.version, inner.payload.to_vec())
                };
                let expected =
                    u16::try_from(vm.register(11)).map_err(|_| VMError::NoritoInvalid)?;
                if expected != 0 && expected != inner_type as u16 {
                    return Err(VMError::NoritoInvalid);
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, inner_type) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: inner_type as u16,
                    });
                }
                let mut out = Vec::with_capacity(7 + inner_payload.len() + 32);
                out.extend_from_slice(&(inner_type as u16).to_be_bytes());
                out.push(inner_version);
                out.extend_from_slice(&(inner_payload.len() as u32).to_be_bytes());
                out.extend_from_slice(&inner_payload);
                let h: [u8; 32] = iroha_crypto::Hash::new(&inner_payload).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_host_tlv(&out)?;
                vm.set_register(10, p);
                Ok(Self::pointer_gas(encoded_len))
            }
            syscalls::SYSCALL_TLV_EQ => {
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
                vm.set_register(10, if eq { 1 } else { 0 });
                Ok(Self::tlv_eq_gas(left_len, right_len))
            }
            syscalls::SYSCALL_DEBUG_PRINT => {
                let value = vm.register(10);
                if cfg!(any(test, debug_assertions)) {
                    eprintln!("[IVM] debug_print r10={value}");
                }
                Ok(DEBUG_GAS)
            }
            syscalls::SYSCALL_EXIT => {
                let status = vm.register(10);
                vm.request_exit();
                vm.set_register(10, status);
                Ok(DEBUG_GAS)
            }
            syscalls::SYSCALL_ABORT => {
                vm.request_abort();
                Ok(DEBUG_GAS)
            }
            syscalls::SYSCALL_CONTRACT_ABORT => {
                vm.request_contract_abort(vm.register(10));
                Ok(DEBUG_GAS)
            }
            syscalls::SYSCALL_DEBUG_LOG => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Ok(DEBUG_GAS);
                }
                let tlv = vm.validate_tlv(Self::resolve_code_tlv_addr(vm, ptr))?;
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
            syscalls::SYSCALL_SM3_HASH => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest = Sm3Digest::hash(tlv.payload);
                let bytes = digest.as_bytes();
                let addr = Self::alloc_blob_tlv(vm, bytes)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_SHA256_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest = <Sha256 as Sha2Digest>::digest(tlv.payload);
                let addr = Self::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_SHA3_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest = <Sha3_256 as Sha3Digest>::digest(tlv.payload);
                let addr = Self::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_BLAKE2B256_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest = Self::blake2b256(tlv.payload);
                let addr = Self::alloc_blob_tlv(vm, &digest)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_KECCAK256_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest = <Keccak256 as Sha3Digest>::digest(tlv.payload);
                let addr = Self::alloc_blob_tlv(vm, digest.as_slice())?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_IROHA_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let payload_len = tlv.payload.len();
                let digest: [u8; IrohaHash::LENGTH] = IrohaHash::new(tlv.payload).into();
                let addr = Self::alloc_blob_tlv(vm, &digest)?;
                vm.set_register(10, addr);
                Ok(Self::hash_syscall_gas(payload_len))
            }
            syscalls::SYSCALL_SM2_VERIFY
            | syscalls::SYSCALL_SM4_GCM_SEAL
            | syscalls::SYSCALL_SM4_GCM_OPEN
            | syscalls::SYSCALL_SM4_CCM_SEAL
            | syscalls::SYSCALL_SM4_CCM_OPEN => {
                if !self.sm_enabled {
                    return Err(VMError::PermissionDenied);
                }
                let mut default = crate::host::DefaultHost::new().with_sm_enabled(true);
                default.syscall(number, vm)
            }
            syscalls::SYSCALL_UNREGISTER_DOMAIN => {
                // r10 = &DomainId
                Self::expect_tlv(vm, 10, PointerType::DomainId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_TRANSFER_DOMAIN => {
                // r10 = &DomainId, r11 = &AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::DomainId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                // r10=&AccountId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                let value_len = Self::expect_tlv_payload_len(vm, 12, PointerType::Json)?;
                Ok(Self::mutation_gas(value_len))
            }
            syscalls::SYSCALL_NFT_MINT_ASSET => {
                // r10=&NftId, r11=&AccountId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                // r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::NftId)?;
                Self::expect_tlv(vm, 12, PointerType::AccountId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_TRANSFER_V1 => {
                if self.fastpq_batch_active {
                    self.push_fastpq_batch_entry(vm)
                } else {
                    Err(VMError::PermissionDenied)
                }
            }
            syscalls::SYSCALL_TRANSFER_ASSET_SCOPED => {
                // r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId,
                // r13=&Quantity, r14=&DataSpaceId
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
                Self::expect_amount(vm, 13)?;
                Self::expect_tlv(vm, 14, PointerType::DataSpaceId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch(vm),
            syscalls::SYSCALL_NFT_SET_METADATA => {
                // r10=&NftId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                Self::expect_tlv(vm, 12, PointerType::Json)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_NFT_BURN_ASSET => {
                // r10=&NftId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Ok(Self::mutation_gas(0))
            }
            syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
                // Validate host-owned public TLVs in place; materialize immutable
                // literals into the host arena.
                let original = vm.register(10);
                if original == 0 {
                    vm.set_register(10, 0);
                    return Ok(Self::input_publish_gas(0));
                }
                if original >= Memory::HEAP_START {
                    let tlv = vm.validate_tlv(original)?;
                    let envelope_len = 7usize.saturating_add(tlv.payload.len()).saturating_add(32);
                    return Ok(Self::input_publish_gas(envelope_len));
                }
                let resolved = Self::resolve_literal_pointer(vm, original as usize)
                    .ok_or(VMError::NoritoInvalid)? as u64;
                let bytes_vec = vm.clone_tlv(resolved)?;
                let total = bytes_vec.len();
                let dst = vm.alloc_host_tlv(&bytes_vec)?;
                vm.set_register(10, dst);
                Ok(Self::input_publish_gas(total))
            }
            syscalls::SYSCALL_GET_AUTHORITY | syscalls::SYSCALL_SYSVAR_AUTHORITY => {
                // Return the domainless account subject so contracts can compare
                // authority() against AccountId literals and stored AccountId state.
                const ACCOUNT: &str = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
                let authority = AccountId::parse_encoded(ACCOUNT)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|_| VMError::NoritoInvalid)?;
                let payload = to_bytes(&authority).map_err(|_| VMError::NoritoInvalid)?;
                let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
                tlv.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                tlv.push(1);
                tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                tlv.extend_from_slice(&payload);
                let h: [u8; 32] = IrohaHash::new(&payload).into();
                tlv.extend_from_slice(&h);
                let ptr = vm.alloc_host_tlv(&tlv)?;
                vm.set_register(10, ptr);
                Ok(Self::sysvar_gas(payload.len()))
            }
            syscalls::SYSCALL_CURRENT_TIME_MS | syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS => {
                vm.set_register(10, self.current_time_ms);
                Ok(Self::sysvar_gas(0))
            }
            syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT => {
                vm.set_register(10, 0);
                Ok(Self::sysvar_gas(0))
            }
            syscalls::SYSCALL_SYSVAR_CHAIN_ID
            | syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS
            | syscalls::SYSCALL_SYSVAR_ENTRYPOINT => {
                vm.set_register(10, 0);
                Ok(Self::sysvar_gas(0))
            }
            syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),
            syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(vm),
            _ => Err(Self::unsupported_syscall_error(number)),
        }
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any
    where
        Self: 'static,
    {
        self
    }

    fn begin_tx(&mut self, _declared: &crate::parallel::StateAccessSet) -> Result<(), VMError> {
        self.access_log.read_keys.clear();
        self.access_log.write_keys.clear();
        self.access_log.reg_tags.clear();
        self.access_log.state_writes.clear();
        Ok(())
    }

    fn checkpoint(&self) -> Option<Box<dyn core::any::Any + Send>> {
        Some(Box::new(CoreHostSnapshot {
            state: self.state.checkpoint(),
            axt_state: self.axt_state.clone(),
            axt_policy: Arc::clone(&self.axt_policy),
            axt_policy_snapshot: self.axt_policy_snapshot.clone(),
            axt_proof_cache: self.axt_proof_cache.clone(),
            axt_proof_cache_slot: self.axt_proof_cache_slot,
            slot_length_ms: self.slot_length_ms,
            max_clock_skew_ms: self.max_clock_skew_ms,
            axt_active: self.axt_active,
            fastpq_batch_active: self.fastpq_batch_active,
            fastpq_batch_has_entries: self.fastpq_batch_has_entries,
            sm_enabled: self.sm_enabled,
            current_time_ms: self.current_time_ms,
            access_log: self.access_log.clone(),
        }))
    }

    fn restore(&mut self, snapshot: &dyn core::any::Any) -> bool {
        if let Some(saved) = snapshot.downcast_ref::<CoreHostSnapshot>() {
            let _ = self.state.restore(&saved.state);
            self.axt_state = saved.axt_state.clone();
            self.axt_policy = Arc::clone(&saved.axt_policy);
            self.axt_policy_snapshot = saved.axt_policy_snapshot.clone();
            self.axt_proof_cache = saved.axt_proof_cache.clone();
            self.axt_proof_cache_slot = saved.axt_proof_cache_slot;
            self.slot_length_ms = saved.slot_length_ms;
            self.max_clock_skew_ms = saved.max_clock_skew_ms;
            self.axt_active = saved.axt_active;
            self.fastpq_batch_active = saved.fastpq_batch_active;
            self.fastpq_batch_has_entries = saved.fastpq_batch_has_entries;
            self.sm_enabled = saved.sm_enabled;
            self.current_time_ms = saved.current_time_ms;
            self.access_log = saved.access_log.clone();
            return true;
        }
        false
    }

    fn access_logging_supported(&self) -> bool {
        true
    }

    fn finish_tx(&mut self) -> Result<crate::host::AccessLog, VMError> {
        Ok(self.access_log.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IVM, encoding, instruction, syscalls};
    use ivm_abi::metadata::{
        EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, EmbeddedStateDescriptor,
        EmbeddedStateType, LITERAL_SECTION_MAGIC, ProgramMetadata,
    };

    fn state_interface(name: &str, ty: EmbeddedStateType) -> EmbeddedContractInterfaceV1 {
        EmbeddedContractInterfaceV1 {
            seiyaku_name: "StateMapHostFixture".to_owned(),
            compiler_fingerprint: "ivm-core-host-tests".to_owned(),
            abi_hash: crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![EmbeddedEntrypointDescriptor {
                name: "inspect".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::View,
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
            states: vec![EmbeddedStateDescriptor {
                name: name.to_owned(),
                ty,
            }],
            error_codes: Vec::new(),
        }
    }

    fn state_map_interface(name: &str, key: EmbeddedStateType) -> EmbeddedContractInterfaceV1 {
        state_interface(
            name,
            EmbeddedStateType::StateMap {
                key: Box::new(key),
                value: Box::new(EmbeddedStateType::Bytes),
            },
        )
    }

    fn load_state_map_schema(vm: &mut IVM, name: &str, key: EmbeddedStateType) {
        let mut artifact = ProgramMetadata::default().encode();
        artifact.extend_from_slice(&state_map_interface(name, key).encode_section());
        artifact.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
        vm.load_program(&artifact)
            .expect("load StateMap CNTR schema");
    }

    fn assemble_state_map_program(words: &[u32], name: &str, key: EmbeddedStateType) -> Vec<u8> {
        let mut artifact = ProgramMetadata::default().encode();
        artifact.extend_from_slice(&state_map_interface(name, key).encode_section());
        for word in words {
            artifact.extend_from_slice(&word.to_le_bytes());
        }
        artifact
    }

    fn build_typed_map_path(
        vm: &mut IVM,
        host: &mut CoreHost,
        base: &str,
        key: &[u8],
    ) -> Result<Name, VMError> {
        let base: Name = base.parse().expect("valid test map base");
        let base_pointer = vm.alloc_host_tlv(&make_pointer_tlv(
            PointerType::Name,
            &norito::to_bytes(&base).expect("encode test map base"),
        ))?;
        let key_pointer = vm.alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, key))?;
        vm.set_register(10, base_pointer);
        vm.set_register(11, key_pointer);
        host.syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, vm)?;
        let output = vm.validate_tlv(vm.register(10))?;
        norito::decode_from_bytes(output.payload).map_err(|_| VMError::DecodeError)
    }

    fn decode_typed_map_page_key(
        vm: &mut IVM,
        host: &mut CoreHost,
        base: &str,
        key: &[u8],
    ) -> Result<Vec<u8>, VMError> {
        let base: Name = base.parse().expect("valid test map base");
        let path = crate::host::canonical_state_map_path(&base, key)?;
        let page = norito::to_bytes(&vec![path]).expect("encode test map page");
        let page_pointer = vm.alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &page))?;
        let base_pointer = vm.alloc_host_tlv(&make_pointer_tlv(
            PointerType::Name,
            &norito::to_bytes(&base).expect("encode test map base"),
        ))?;
        vm.set_register(10, page_pointer);
        vm.set_register(11, base_pointer);
        vm.set_register(12, 0);
        host.syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, vm)?;
        Ok(vm.validate_tlv(vm.register(10))?.payload.to_vec())
    }

    fn set_raw_state_path(
        vm: &mut IVM,
        host: &mut CoreHost,
        path: &Name,
        value: &[u8],
    ) -> Result<u64, VMError> {
        let path_pointer = vm.alloc_host_tlv(&make_pointer_tlv(
            PointerType::Name,
            &norito::to_bytes(path).expect("encode raw state path"),
        ))?;
        let value_pointer =
            vm.alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, value))?;
        vm.set_register(10, path_pointer);
        vm.set_register(11, value_pointer);
        host.syscall(syscalls::SYSCALL_STATE_SET, vm)
    }

    fn state_value_record(
        ty: &EmbeddedStateType,
        atoms: Vec<ivm_abi::state_value::StateValueAtomV1>,
    ) -> Vec<u8> {
        let schema = crate::state_value_runtime::schema_for_embedded_state_type(ty)
            .expect("valid embedded state type");
        let schema_payload = norito::to_bytes(&schema).expect("encode state-value schema");
        let record = ivm_abi::state_value::StateValueRecordV1 {
            schema_hash: ivm_abi::state_value::state_value_schema_hash_v1(&schema_payload),
            atoms,
        };
        norito::to_bytes(&record).expect("encode state-value record")
    }

    fn bytes_state_value_record(value: &[u8]) -> Vec<u8> {
        state_value_record(
            &EmbeddedStateType::Bytes,
            vec![ivm_abi::state_value::StateValueAtomV1::Pointer(
                make_pointer_tlv(PointerType::Blob, value),
            )],
        )
    }

    fn int_state_value_record(value: i128) -> Vec<u8> {
        let envelope =
            crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(value))
                .expect("encode integer state leaf");
        state_value_record(
            &EmbeddedStateType::Int,
            vec![ivm_abi::state_value::StateValueAtomV1::Pointer(envelope)],
        )
    }

    fn alloc_state_path(vm: &mut IVM, path: &Name) -> u64 {
        vm.alloc_host_tlv(&make_pointer_tlv(
            PointerType::Name,
            &norito::to_bytes(path).expect("encode state path"),
        ))
        .expect("allocate state path")
    }

    fn alloc_state_value(vm: &mut IVM, value: &[u8]) -> u64 {
        vm.alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, value))
            .expect("allocate state value")
    }

    fn assemble_program(words: &[u32]) -> Vec<u8> {
        let mut code = Vec::with_capacity(words.len() * 4);
        for word in words {
            code.extend_from_slice(&word.to_le_bytes());
        }
        let mut program = ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        }
        .encode();
        program.extend_from_slice(&code);
        program
    }

    fn assemble_state_runtime_program(
        words: &[u32],
        name: &str,
        ty: EmbeddedStateType,
        write: bool,
    ) -> Vec<u8> {
        let access_key = if matches!(&ty, EmbeddedStateType::StateMap { .. }) {
            format!("state:{name}[*]")
        } else {
            format!("state:{name}")
        };
        let mut interface = state_interface(name, ty);
        let entrypoint = interface
            .entrypoints
            .first_mut()
            .expect("state fixture has one entrypoint");
        if write {
            entrypoint.kind = iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage;
            entrypoint.permission = Some("Execute".to_owned());
            entrypoint.write_keys.push(access_key);
        } else {
            entrypoint.read_keys.push(access_key);
        }
        let mut program = ProgramMetadata::default().encode();
        program.extend_from_slice(&interface.encode_section());
        for word in words {
            program.extend_from_slice(&word.to_le_bytes());
        }
        program
    }

    fn assemble_state_map_read_program(words: &[u32], name: &str) -> Vec<u8> {
        assemble_state_runtime_program(
            words,
            name,
            EmbeddedStateType::StateMap {
                key: Box::new(EmbeddedStateType::Bytes),
                value: Box::new(EmbeddedStateType::Bytes),
            },
            false,
        )
    }

    fn assemble_state_value_read_program(words: &[u32], name: &str) -> Vec<u8> {
        assemble_state_runtime_program(words, name, EmbeddedStateType::Bytes, false)
    }

    fn assemble_state_value_write_program(words: &[u32], name: &str) -> Vec<u8> {
        assemble_state_runtime_program(words, name, EmbeddedStateType::Bytes, true)
    }

    fn assemble_program_with_literals(literals: &[&[u8]]) -> (Vec<u8>, Vec<u64>) {
        let mut code = Vec::new();
        code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
        let mut program = ProgramMetadata::default().encode();
        let offsets_len = literals.len().saturating_mul(core::mem::size_of::<u64>());
        let data_start = 16_usize.saturating_add(offsets_len);
        let data_len = literals.iter().map(|literal| literal.len()).sum::<usize>();
        let mut offsets = Vec::with_capacity(literals.len());
        let mut data = Vec::with_capacity(data_len);
        let mut cursor = u64::try_from(data_start).expect("literal table offset fits u64");
        for literal in literals {
            offsets.push(cursor);
            data.extend_from_slice(literal);
            cursor = cursor
                .saturating_add(u64::try_from(literal.len()).expect("literal length fits u64"));
        }
        let post_pad = (4 - ((16 + offsets_len + data.len()) % 4)) % 4;
        program.extend_from_slice(&LITERAL_SECTION_MAGIC);
        program.extend_from_slice(
            &u32::try_from(literals.len())
                .expect("literal count fits u32")
                .to_le_bytes(),
        );
        program.extend_from_slice(
            &u32::try_from(post_pad)
                .expect("literal padding fits u32")
                .to_le_bytes(),
        );
        program.extend_from_slice(
            &u32::try_from(data.len())
                .expect("literal bytes fit u32")
                .to_le_bytes(),
        );
        for offset in &offsets {
            program.extend_from_slice(&offset.to_le_bytes());
        }
        program.extend_from_slice(&data);
        program.extend(std::iter::repeat_n(0_u8, post_pad));
        program.extend_from_slice(&code);
        (program, offsets)
    }

    fn make_pointer_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
        v.extend_from_slice(&(pointer_type as u16).to_be_bytes());
        v.push(1);
        v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        v.extend_from_slice(payload);
        let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new(payload).into();
        v.extend_from_slice(&hash);
        v
    }

    fn make_tlv(payload: &[u8]) -> Vec<u8> {
        make_pointer_tlv(PointerType::NoritoBytes, payload)
    }

    fn make_numeric_tlv(value: Numeric) -> Vec<u8> {
        let payload = norito::to_bytes(&value).expect("encode numeric");
        make_pointer_tlv(PointerType::NoritoBytes, &payload)
    }

    fn make_amount_tlv(value: Quantity) -> Vec<u8> {
        crate::numeric_tlv::encode_quantity(&value).expect("encode quantity pointer envelope")
    }

    #[test]
    fn core_host_expect_tlv_enforces_pointer_policy() {
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&assemble_program(&[encoding::wide::encode_halt()]))
            .expect("load program");
        let ptr = vm
            .alloc_input_tlv(&make_tlv(b"payload"))
            .expect("alloc payload tlv");
        vm.set_register(10, ptr);

        let _guard =
            crate::pointer_abi::PointerPolicyGuard::install(crate::SyscallPolicy::AbiV1, 2);
        let err = CoreHost::expect_tlv(&vm, 10, PointerType::NoritoBytes).unwrap_err();
        assert!(matches!(
            err,
            VMError::AbiTypeNotAllowed { abi: 2, type_id }
                if type_id == PointerType::NoritoBytes as u16
        ));
    }

    #[test]
    fn core_host_amount_arguments_require_canonical_quantity_pointer() {
        let mut vm = IVM::new(u64::MAX);
        let canonical = "1.25".parse::<Quantity>().expect("canonical quantity");
        let canonical_ptr = vm
            .alloc_input_tlv(&make_amount_tlv(canonical.clone()))
            .expect("allocate canonical quantity");
        vm.set_register(13, canonical_ptr);
        assert_eq!(CoreHost::expect_amount(&vm, 13), Ok(()));

        let legacy_ptr = vm
            .alloc_input_tlv(&make_numeric_tlv(canonical.into_numeric()))
            .expect("allocate legacy Numeric pointer");
        vm.set_register(13, legacy_ptr);
        assert_eq!(
            CoreHost::expect_amount(&vm, 13),
            Err(VMError::NoritoInvalid)
        );

        let malformed_payload =
            norito::to_bytes(&Numeric::new(1_250_u32, 3)).expect("encode malformed quantity");
        let noncanonical_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Quantity, &malformed_payload))
            .expect("allocate malformed quantity");
        vm.set_register(13, noncanonical_ptr);
        assert_eq!(CoreHost::expect_amount(&vm, 13), Err(VMError::DecodeError));
    }

    #[test]
    fn core_host_decoder_enforces_pointer_policy() {
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&assemble_program(&[encoding::wide::encode_halt()]))
            .expect("load program");
        let ptr = vm
            .alloc_input_tlv(&make_tlv(b"payload"))
            .expect("alloc payload tlv");
        let host = CoreHost::new();

        let _guard =
            crate::pointer_abi::PointerPolicyGuard::install(crate::SyscallPolicy::AbiV1, 2);
        let err = host
            .decode_tlv(&vm, ptr, PointerType::NoritoBytes)
            .unwrap_err();
        assert!(matches!(
            err,
            VMError::AbiTypeNotAllowed { abi: 2, type_id }
                if type_id == PointerType::NoritoBytes as u16
        ));
    }

    #[test]
    fn durable_state_accepts_validated_public_heap_tlvs() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        let path = Name::from_str("heap_state").expect("valid state path");
        let path_payload = norito::to_bytes(&path).expect("encode state path");
        let path_envelope = make_pointer_tlv(PointerType::Name, &path_payload);
        let path_pointer = vm
            .alloc_heap(path_envelope.len() as u64)
            .expect("allocate path envelope");
        vm.store_bytes(path_pointer, &path_envelope)
            .expect("store path envelope");

        let value_payload = norito::to_bytes(&17_i64).expect("encode state value");
        let value_envelope = make_pointer_tlv(PointerType::NoritoBytes, &value_payload);
        let value_pointer = vm
            .alloc_heap(value_envelope.len() as u64)
            .expect("allocate value envelope");
        vm.store_bytes(value_pointer, &value_envelope)
            .expect("store value envelope");
        vm.set_register(10, path_pointer);
        vm.set_register(11, value_pointer);

        host.syscall(syscalls::SYSCALL_STATE_SET, &mut vm)
            .expect("set state from public heap TLVs");
        assert_eq!(
            host.state_bytes(path.as_ref()).as_deref(),
            Some(value_payload.as_slice())
        );
    }

    #[test]
    fn state_keys_syscall_returns_sorted_prefix_page() {
        let mut host = CoreHost::new();
        host.insert_state_value("orders/2", b"two");
        host.insert_state_value("orders/1", b"one");
        host.insert_state_value("accounts/1", b"account");
        let mut vm = IVM::new(u64::MAX);
        let prefix: Name = "orders".parse().expect("state prefix");
        let prefix_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&prefix).expect("encode prefix"),
            ))
            .expect("alloc prefix");

        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 1);
        vm.set_register(12, 1);
        let gas = host
            .syscall(syscalls::SYSCALL_STATE_KEYS, &mut vm)
            .expect("STATE_KEYS");

        assert!(gas > 0);
        assert_eq!(vm.register(11), 2);
        assert_eq!(vm.register(12), 1);
        let tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("state keys tlv");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let keys: Vec<Name> = norito::decode_from_bytes(tlv.payload).expect("decode keys");
        assert_eq!(keys, vec!["orders/2".parse::<Name>().expect("second key")]);

        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 0);
        host.syscall(syscalls::SYSCALL_STATE_KEYS, &mut vm)
            .expect("zero-sized state page");
        assert_eq!(vm.register(12), 0);
        let empty_page = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("empty state keys page");
        assert!(
            norito::decode_from_bytes::<Vec<Name>>(empty_page.payload)
                .expect("decode empty page")
                .is_empty()
        );

        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, syscalls::STATE_KEYS_MAX_ITEMS + 1);
        assert!(matches!(
            host.syscall(syscalls::SYSCALL_STATE_KEYS, &mut vm),
            Err(VMError::NoritoInvalid)
        ));

        let key: Name = "orders/2".parse().expect("state key");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&key).expect("encode key"),
            ))
            .expect("alloc key");

        vm.set_register(10, key_ptr);
        host.syscall(syscalls::SYSCALL_STATE_HAS, &mut vm)
            .expect("STATE_HAS");
        assert_eq!(vm.register(10), 1);

        vm.set_register(10, key_ptr);
        host.syscall(syscalls::SYSCALL_STATE_LEN, &mut vm)
            .expect("STATE_LEN");
        assert_eq!(vm.register(10), 3);
        assert_eq!(vm.register(11), 1);

        vm.set_register(10, prefix_ptr);
        let count_gas = STATE_QUERY_GAS_BASE
            + u64::try_from(crate::host::state_path_name_payload_len(&prefix).expect("path len"))
                .expect("path length fits")
            + u64::try_from(1 + "orders/1".len() + 1 + "orders/2".len()).expect("scan length fits");
        assert_eq!(
            host.syscall(syscalls::SYSCALL_STATE_COUNT, &mut vm),
            Ok(count_gas)
        );
        assert_eq!(vm.register(10), 2);
    }

    #[test]
    fn state_map_key_at_decodes_canonical_hex_and_returns_null_past_page() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, "orders", EmbeddedStateType::Int);
        let base: Name = "orders".parse().expect("map base");
        let key = crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(-7))
            .expect("encode canonical int key");
        let path = crate::host::canonical_state_map_path(&base, &key).expect("canonical path");
        let page = norito::to_bytes(&vec![path]).expect("encode state key page");
        let page_ptr = vm
            .alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &page))
            .expect("allocate page");
        let base_ptr = vm
            .alloc_host_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&base).expect("encode base"),
            ))
            .expect("allocate base");

        vm.set_register(10, page_ptr);
        vm.set_register(11, base_ptr);
        vm.set_register(12, 0);
        host.syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, &mut vm)
            .expect("decode first key");
        let output = vm.validate_tlv(vm.register(10)).expect("key output");
        assert_eq!(output.payload, key);

        vm.set_register(10, page_ptr);
        vm.set_register(11, base_ptr);
        vm.set_register(12, 1);
        host.syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, &mut vm)
            .expect("past-end lookup");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn state_map_key_at_prepare_is_length_only_and_execution_fails_closed() {
        let host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, "orders", EmbeddedStateType::Bytes);
        let malformed_page = vm
            .alloc_host_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &[0xff]))
            .expect("allocate malformed page");
        let base: Name = "orders".parse().expect("map base");
        let base_ptr = vm
            .alloc_host_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&base).expect("encode base"),
            ))
            .expect("allocate base");
        vm.set_register(10, malformed_page);
        vm.set_register(11, base_ptr);
        vm.set_register(12, 0);

        assert!(
            host.prepare_syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, &vm)
                .is_ok(),
            "prepare must not decode the guest page"
        );
        let mut execution_host = host.clone();
        assert!(matches!(
            execution_host.syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, &mut vm),
            Err(VMError::DecodeError)
        ));
    }

    #[test]
    fn state_map_key_at_rejects_oversized_page_before_decoding() {
        let host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, "orders", EmbeddedStateType::Bytes);
        let mut oversized = Vec::with_capacity(7);
        oversized.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        oversized.push(1);
        oversized.extend_from_slice(
            &u32::try_from(syscalls::STATE_MAP_MAX_PAGE_BYTES + 1)
                .expect("state map page bound fits u32")
                .to_be_bytes(),
        );
        let page_ptr = vm
            .alloc_input_tlv(&oversized)
            .expect("allocate forged oversized page header");
        let base: Name = "orders".parse().expect("map base");
        let base_ptr = vm
            .alloc_host_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&base).expect("encode base"),
            ))
            .expect("allocate base");
        vm.set_register(10, page_ptr);
        vm.set_register(11, base_ptr);
        vm.set_register(12, 0);

        assert!(matches!(
            host.prepare_syscall(syscalls::SYSCALL_STATE_MAP_KEY_AT, &vm),
            Err(VMError::NoritoInvalid)
        ));
    }

    #[test]
    fn state_map_numeric_key_ingress_rejects_missing_schema_and_invalid_frames() {
        use crate::numeric::PointerAbiFaultV1;
        use iroha_primitives::{bigint::BigInt, numeric::Numeric, numeric_abi::IntValueV1};

        let canonical_int =
            crate::numeric_tlv::encode_int(&BigInt::one()).expect("encode canonical integer key");
        let mut schemaless_vm = IVM::new(u64::MAX);
        assert_eq!(
            build_typed_map_path(
                &mut schemaless_vm,
                &mut CoreHost::new(),
                "values",
                &canonical_int,
            ),
            Err(VMError::NoritoInvalid),
            "schema-bound key helpers must fail closed without CNTR metadata"
        );

        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, "values", EmbeddedStateType::Int);
        let mut host = CoreHost::new();

        assert!(matches!(
            build_typed_map_path(&mut vm, &mut host, "values", &[0x11]),
            Err(VMError::PointerAbiFault(
                PointerAbiFaultV1::TruncatedEnvelope
            ))
        ));
        assert!(matches!(
            decode_typed_map_page_key(&mut vm, &mut host, "values", &[0x11]),
            Err(VMError::PointerAbiFault(
                PointerAbiFaultV1::TruncatedEnvelope
            ))
        ));

        let decimal = crate::numeric_tlv::encode_decimal(&Numeric::one())
            .expect("encode cross-typed decimal key");
        assert_eq!(
            build_typed_map_path(&mut vm, &mut host, "values", &decimal),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::WrongType))
        );
        assert_eq!(
            decode_typed_map_page_key(&mut vm, &mut host, "values", &decimal),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::WrongType)),
            "iteration must not surface a cross-typed persisted key"
        );

        let mut noncanonical_body = Vec::new();
        noncanonical_body.extend_from_slice(&1_u32.to_le_bytes());
        noncanonical_body.push(0);
        let noncanonical_frame =
            norito::core::frame_bare_with_header_flags::<IntValueV1>(&noncanonical_body, 0)
                .expect("build structurally valid noncanonical integer frame");
        let noncanonical = make_pointer_tlv(PointerType::Int, &noncanonical_frame);
        assert_eq!(
            build_typed_map_path(&mut vm, &mut host, "values", &noncanonical),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::NonCanonical))
        );
        assert_eq!(
            decode_typed_map_page_key(&mut vm, &mut host, "values", &noncanonical),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::NonCanonical)),
            "iteration must not surface a noncanonical persisted key"
        );

        let base: Name = "values".parse().expect("map base");
        let canonical_zero =
            crate::numeric_tlv::encode_int(&BigInt::zero()).expect("encode canonical zero key");
        let canonical_path = crate::host::canonical_state_map_path(&base, &canonical_zero)
            .expect("build canonical zero path");
        let canonical_value = bytes_state_value_record(b"canonical");
        set_raw_state_path(&mut vm, &mut host, &canonical_path, &canonical_value)
            .expect("direct STATE_SET accepts the canonical typed path");
        let noncanonical_path = crate::host::canonical_state_map_path(&base, &noncanonical)
            .expect("build adversarial alternate path bytes");
        assert_eq!(
            set_raw_state_path(&mut vm, &mut host, &noncanonical_path, b"alternate"),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::NonCanonical)),
            "direct raw STATE_SET must not bypass typed map-key validation"
        );
        assert_eq!(host.state_paths(), vec![canonical_path.to_string()]);
    }

    #[test]
    fn state_map_nonnumeric_keys_remain_schema_bound_and_canonical() {
        let blob_key = make_pointer_tlv(PointerType::Blob, b"opaque bytes");
        let mut bytes_vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut bytes_vm, "values", EmbeddedStateType::Bytes);
        let bytes_path =
            build_typed_map_path(&mut bytes_vm, &mut CoreHost::new(), "values", &blob_key)
                .expect("canonical bytes key remains valid");
        assert_eq!(
            bytes_path.as_ref(),
            format!("values/{}", hex::encode(&blob_key))
        );

        let name: Name = "alice".parse().expect("name key");
        let name_key = make_pointer_tlv(
            PointerType::Name,
            &norito::to_bytes(&name).expect("encode name key"),
        );
        let mut name_vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut name_vm, "values", EmbeddedStateType::Name);
        build_typed_map_path(&mut name_vm, &mut CoreHost::new(), "values", &name_key)
            .expect("canonical typed Name key remains valid");

        assert_eq!(
            build_typed_map_path(&mut name_vm, &mut CoreHost::new(), "values", &blob_key,),
            Err(VMError::NoritoInvalid),
            "pointer-compatible bytes cannot cross a declared key type"
        );
    }

    #[test]
    fn typed_state_operations_reject_map_bases_and_wrong_value_schemas_atomically() {
        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, "values", EmbeddedStateType::Int);
        let mut host = CoreHost::new();
        let base: Name = "values".parse().expect("map base");
        let valid_value = bytes_state_value_record(b"stable");

        for syscall in [
            syscalls::SYSCALL_STATE_GET,
            syscalls::SYSCALL_STATE_DEL,
            syscalls::SYSCALL_STATE_HAS,
            syscalls::SYSCALL_STATE_LEN,
        ] {
            let path_ptr = alloc_state_path(&mut vm, &base);
            vm.set_register(10, path_ptr);
            assert_eq!(
                host.syscall(syscall, &mut vm),
                Err(VMError::NoritoInvalid),
                "value syscall {syscall:#x} must reject a bare StateMap base"
            );
            assert_eq!(vm.register(10), path_ptr, "failure must publish no output");
        }
        let path_ptr = alloc_state_path(&mut vm, &base);
        let value_ptr = alloc_state_value(&mut vm, &valid_value);
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_STATE_SET, &mut vm),
            Err(VMError::NoritoInvalid),
            "STATE_SET must not conflate a map collection with one map value"
        );
        assert!(host.state_paths().is_empty());

        for syscall in [syscalls::SYSCALL_STATE_KEYS, syscalls::SYSCALL_STATE_COUNT] {
            let path_ptr = alloc_state_path(&mut vm, &base);
            vm.set_register(10, path_ptr);
            vm.set_register(11, 0);
            vm.set_register(12, 0);
            host.syscall(syscall, &mut vm).unwrap_or_else(|error| {
                panic!("scan syscall {syscall:#x} rejected map base: {error}")
            });
        }

        let key = crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::zero())
            .expect("encode canonical map key");
        let path = crate::host::canonical_state_map_path(&base, &key).expect("map child path");
        set_raw_state_path(&mut vm, &mut host, &path, &valid_value)
            .expect("canonical typed map value");
        assert_eq!(host.state_bytes(path.as_ref()), Some(valid_value.clone()));

        let wrong_schema = int_state_value_record(7);
        assert!(matches!(
            set_raw_state_path(&mut vm, &mut host, &path, &wrong_schema),
            Err(VMError::DecodeError | VMError::NoritoInvalid)
        ));

        let mut wrong_hash: ivm_abi::state_value::StateValueRecordV1 =
            norito::decode_from_bytes(&valid_value).expect("decode valid state record");
        wrong_hash.schema_hash[0] ^= 0x80;
        let wrong_hash = norito::to_bytes(&wrong_hash).expect("encode wrong-hash record");
        assert!(matches!(
            set_raw_state_path(&mut vm, &mut host, &path, &wrong_hash),
            Err(VMError::DecodeError | VMError::NoritoInvalid)
        ));

        let wrong_pointer = state_value_record(
            &EmbeddedStateType::Bytes,
            vec![ivm_abi::state_value::StateValueAtomV1::Pointer(
                crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::one())
                    .expect("encode wrong nominal pointer"),
            )],
        );
        for malformed in [wrong_pointer.as_slice(), &[0xff][..]] {
            assert!(matches!(
                set_raw_state_path(&mut vm, &mut host, &path, malformed),
                Err(VMError::DecodeError | VMError::NoritoInvalid)
            ));
        }
        assert_eq!(
            host.state_bytes(path.as_ref()),
            Some(valid_value),
            "every rejected write must leave the prior typed value unchanged"
        );
    }

    #[test]
    fn typed_state_get_rejects_preexisting_untyped_bytes_without_publication() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        let base: Name = "values".parse().expect("map base");
        let key = crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::one())
            .expect("encode canonical map key");
        let path = crate::host::canonical_state_map_path(&base, &key).expect("map child path");

        host.insert_state_value(path.as_ref(), b"preexisting-untyped");
        load_state_map_schema(&mut vm, base.as_ref(), EmbeddedStateType::Int);
        let path_ptr = alloc_state_path(&mut vm, &path);
        vm.set_register(10, path_ptr);

        assert!(matches!(
            host.syscall(syscalls::SYSCALL_STATE_GET, &mut vm),
            Err(VMError::DecodeError | VMError::NoritoInvalid)
        ));
        assert_eq!(
            vm.register(10),
            path_ptr,
            "invalid state must not be published"
        );
        assert_eq!(
            host.state_bytes(path.as_ref()).as_deref(),
            Some(b"preexisting-untyped".as_slice()),
            "a failed read is side-effect free"
        );
    }

    #[test]
    fn empty_state_keys_limit_sixty_four_fits_default_gas() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(1_000_000);
        let prefix: Name = "empty".parse().expect("prefix");
        let prefix_payload = norito::to_bytes(&prefix).expect("encode prefix");
        let prefix_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &prefix_payload))
            .expect("allocate prefix");
        vm.load_program(&assemble_state_map_read_program(
            &[
                encoding::wide::encode_syscallx(syscalls::SYSCALL_STATE_KEYS),
                encoding::wide::encode_halt(),
            ],
            prefix.as_ref(),
        ))
        .expect("load program");
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, syscalls::STATE_KEYS_MAX_ITEMS);
        assert!(
            host.prepare_syscall(syscalls::SYSCALL_STATE_KEYS, &vm)
                .is_ok(),
            "the empty-page minimum must fit the V1 default"
        );

        vm.run_with_host(&mut host)
            .expect("empty 64-item page under default gas");
        assert_eq!(vm.register(11), 0);
        assert_eq!(vm.register(12), 0);
        let page = vm.validate_tlv(vm.register(10)).expect("page TLV");
        assert!(
            norito::decode_from_bytes::<Vec<Name>>(page.payload)
                .expect("decode empty page")
                .is_empty()
        );
    }

    #[test]
    fn state_keys_quote_minus_one_fails_before_selected_key_materialization() {
        let key_text = "pick/0000";
        let mut host = CoreHost::new();
        host.insert_state_value(key_text, b"value");
        let mut vm = IVM::new(u64::MAX);
        let prefix: Name = "pick".parse().expect("prefix");
        let prefix_payload = norito::to_bytes(&prefix).expect("encode prefix");
        let prefix_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &prefix_payload))
            .expect("allocate prefix");
        vm.load_program(&assemble_state_map_read_program(
            &[
                encoding::wide::encode_syscallx(syscalls::SYSCALL_STATE_KEYS),
                encoding::wide::encode_halt(),
            ],
            prefix.as_ref(),
        ))
        .expect("load program");
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 1);
        let (_, response_tail) = crate::host::state_keys_response_tail_after_item(0, 0, key_text)
            .expect("response tail");
        let scan_work =
            u64::try_from(prefix_payload.len() + 1 + key_text.len()).expect("scan work fits");
        let combined = STATE_QUERY_GAS_BASE
            .saturating_add(scan_work)
            .saturating_add(u64::try_from(response_tail).expect("response fits"));
        vm.set_gas_limit(combined.saturating_add(4));

        assert_eq!(vm.run_with_host(&mut host), Err(VMError::OutOfGas));
        assert_eq!(host.state_scan_examined.load(Ordering::Relaxed), 1);
        assert!(host.access_log.read_keys.is_empty());
        assert_eq!(vm.register(10), prefix_ptr);
        assert_eq!(vm.register(11), 0);
        assert_eq!(vm.register(12), 1);
    }

    #[test]
    fn hostile_state_scan_stops_before_the_unaffordable_nth_key() {
        const FAILING_ITEM: u64 = 8;
        let mut host = CoreHost::new();
        for index in 0..64 {
            host.insert_state_value(format!("scan/{index:04}"), b"value");
        }
        let mut vm = IVM::new(u64::MAX);
        let prefix: Name = "scan".parse().expect("prefix");
        let prefix_payload = norito::to_bytes(&prefix).expect("encode prefix");
        let prefix_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &prefix_payload))
            .expect("allocate prefix");
        vm.load_program(&assemble_state_map_read_program(
            &[
                encoding::wide::encode_syscallx(syscalls::SYSCALL_STATE_KEYS),
                encoding::wide::encode_halt(),
            ],
            prefix.as_ref(),
        ))
        .expect("load program");
        vm.set_register(10, prefix_ptr);
        vm.set_register(11, u64::MAX);
        vm.set_register(12, syscalls::STATE_KEYS_MAX_ITEMS);
        let empty_tail = crate::host::state_keys_prepare_minimum(
            prefix_payload.len(),
            syscalls::STATE_KEYS_MAX_ITEMS,
        )
        .expect("empty page minimum")
        .saturating_sub(crate::host::state_path_gas(prefix_payload.len()));
        let key_len = "scan/0000".len();
        let failing_work = u64::try_from(prefix_payload.len())
            .expect("prefix fits")
            .saturating_add(
                FAILING_ITEM.saturating_mul(1 + u64::try_from(key_len).expect("key length fits")),
            );
        let reserve = STATE_QUERY_GAS_BASE
            .saturating_add(failing_work)
            .saturating_add(empty_tail)
            .saturating_sub(1);
        vm.set_gas_limit(reserve.saturating_add(5));

        assert_eq!(vm.run_with_host(&mut host), Err(VMError::OutOfGas));
        assert_eq!(
            host.state_scan_examined.load(Ordering::Relaxed),
            FAILING_ITEM - 1
        );
        assert!(host.access_log.read_keys.is_empty());
        assert_eq!(vm.register(10), prefix_ptr);
        assert_eq!(vm.register(11), u64::MAX);
        assert_eq!(vm.register(12), syscalls::STATE_KEYS_MAX_ITEMS);
    }

    #[test]
    fn state_get_quote_minus_one_observes_no_state_or_guest_output() {
        let mut host = CoreHost::new();
        host.insert_state_value("large", vec![0xabu8; syscalls::STATE_MAX_VALUE_BYTES]);
        let mut vm = IVM::new(u64::MAX);
        let key: Name = "large".parse().expect("state key");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&key).expect("encode key"),
            ))
            .expect("alloc key");
        let program = assemble_state_value_read_program(
            &[
                encoding::wide::encode_sys(
                    instruction::wide::system::SCALL,
                    syscalls::SYSCALL_STATE_GET as u8,
                ),
                encoding::wide::encode_halt(),
            ],
            key.as_ref(),
        );
        vm.load_program(&program).expect("load program");
        vm.set_register(10, key_ptr);
        vm.set_register(11, 0xfeed);
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_STATE_GET, &vm)
            .expect("quote bounded maximum value");
        vm.set_gas_limit(quote.saturating_add(4));

        let error = vm
            .run_with_host(&mut host)
            .expect_err("response cost exceeds the available syscall reserve");

        assert_eq!(error, VMError::OutOfGas);
        assert_eq!(
            vm.remaining_gas(),
            quote - 1,
            "an unaffordable up-front quote performs no host work and is not debited"
        );
        assert!(
            !host.access_log.read_keys.contains("large"),
            "preparation must reject quote-minus-one before the host observes the state key"
        );
        assert_eq!(
            vm.register(10),
            key_ptr,
            "STATE_GET must not allocate output"
        );
        assert_eq!(vm.register(11), 0xfeed, "STATE_GET must not mutate outputs");
    }

    #[test]
    fn state_len_of_maximum_value_uses_only_the_path_quote() {
        let mut host = CoreHost::new();
        host.insert_state_value("maximum", vec![0x5au8; syscalls::STATE_MAX_VALUE_BYTES]);
        let mut vm = IVM::new(u64::MAX);
        let key: Name = "maximum".parse().expect("state key");
        let key_payload = norito::to_bytes(&key).expect("encode key");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
            .expect("allocate key");
        vm.load_program(&assemble_state_value_read_program(
            &[
                encoding::wide::encode_syscallx(syscalls::SYSCALL_STATE_LEN),
                encoding::wide::encode_halt(),
            ],
            key.as_ref(),
        ))
        .expect("load program");
        vm.set_register(10, key_ptr);
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_STATE_LEN, &vm)
            .expect("quote state length");
        assert_eq!(quote, crate::host::state_path_gas(key_payload.len()));
        vm.set_gas_limit(quote.saturating_add(5));

        vm.run_with_host(&mut host)
            .expect("read maximum value length");
        assert_eq!(vm.register(10), syscalls::STATE_MAX_VALUE_BYTES as u64);
        assert_eq!(vm.register(11), 1);
        assert!(host.access_log.read_keys.contains("maximum"));
    }

    #[test]
    fn unaffordable_state_set_does_not_decode_or_mutate_before_quote_debit() {
        let mut vm = IVM::new(u64::MAX);
        let program = assemble_state_value_write_program(
            &[
                encoding::wide::encode_sys(
                    instruction::wide::system::SCALL,
                    syscalls::SYSCALL_STATE_SET as u8,
                ),
                encoding::wide::encode_halt(),
            ],
            "declared",
        );
        vm.load_program(&program).expect("load program");
        let path_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                b"hash-valid but not a Norito Name",
            ))
            .expect("allocate malformed state path");
        let value_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &[0xabu8; 128]))
            .expect("allocate state value");
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        let mut host = CoreHost::new();
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_STATE_SET, &vm)
            .expect("preparation must inspect headers only");
        vm.set_gas_limit(quote.saturating_add(4));

        let error = vm
            .run_with_host(&mut host)
            .expect_err("the state quote is one gas beyond the post-SCALL budget");

        assert_eq!(error, VMError::OutOfGas);
        assert!(host.state.keys().next().is_none());
        assert!(host.access_log.read_keys.is_empty());
        assert!(host.access_log.write_keys.is_empty());
        assert_eq!(vm.register(10), path_ptr);
        assert_eq!(vm.register(11), value_ptr);
    }

    #[test]
    fn decode_int_syscall_sets_register() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let payload = norito::to_bytes(&12345i64).expect("encode i64");
        let tlv = make_tlv(&payload);
        let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_DECODE_INT as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ptr);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 12345);
    }

    #[test]
    fn core_host_schema_helpers_charge_payload_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        let schema: Name = "Order".parse().expect("schema name");
        let json = Json::from_str_norito(r#"{"qty":10,"side":"buy"}"#).expect("json");
        let schema_bytes = norito::to_bytes(&schema).expect("encode schema");
        let json_bytes = norito::to_bytes(&json).expect("encode json");
        let schema_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &schema_bytes))
            .expect("alloc schema");
        let json_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, &json_bytes))
            .expect("alloc json");

        vm.set_register(10, schema_ptr);
        vm.set_register(11, json_ptr);
        let encode_gas = host
            .syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &mut vm)
            .expect("schema encode");
        let encoded_ptr = vm.register(10);
        let encoded = vm.validate_tlv(encoded_ptr).expect("encoded tlv");
        assert_eq!(encoded.type_id, PointerType::NoritoBytes);
        let encoded_len = encoded.payload.len();
        assert_eq!(
            encode_gas,
            CoreHost::schema_gas(json_bytes.len(), encoded_len)
        );

        vm.set_register(10, schema_ptr);
        vm.set_register(11, encoded_ptr);
        let decode_gas = host
            .syscall(syscalls::SYSCALL_SCHEMA_DECODE, &mut vm)
            .expect("schema decode");
        let decoded = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("decoded tlv");
        assert_eq!(decoded.type_id, PointerType::Json);
        assert_eq!(
            decode_gas,
            CoreHost::schema_gas(encoded_len, decoded.payload.len())
        );

        vm.set_register(10, schema_ptr);
        let info_gas = host
            .syscall(syscalls::SYSCALL_SCHEMA_INFO, &mut vm)
            .expect("schema info");
        let info = vm.validate_tlv(vm.register(10)).expect("info tlv");
        assert_eq!(info.type_id, PointerType::Json);
        assert_eq!(
            info_gas,
            CoreHost::schema_gas(schema_bytes.len(), info.payload.len())
        );
    }

    #[test]
    fn schema_prepare_reserves_without_side_effects() {
        let host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let schema: Name = "Order".parse().expect("schema name");
        let json = Json::from_str_norito(&format!(
            r#"{{"qty":9223372036854775807,"side":"{}"}}"#,
            "cd".repeat(8 * 1024)
        ))
        .expect("large JSON");
        let schema_payload = norito::to_bytes(&schema).expect("encode schema");
        let json_payload = norito::to_bytes(&json).expect("encode JSON");
        let schema_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &schema_payload))
            .expect("allocate schema TLV");
        let json_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, &json_payload))
            .expect("allocate JSON TLV");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, json_ptr);
        let r10_before = vm.register(10);
        let r11_before = vm.register(11);
        let paths_before = host.state_paths();
        let writes_before = vm.memory.write_log();
        crate::memory::reset_memory_clone_count();

        let quote = host
            .prepare_syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &vm)
            .expect("quote schema encode");
        assert_eq!(crate::memory::memory_clone_count(), 0);
        assert_eq!(vm.memory.write_log(), writes_before);
        assert_eq!(vm.register(10), r10_before, "quote must not mutate r10");
        assert_eq!(vm.register(11), r11_before, "quote must not mutate r11");
        assert_eq!(
            host.state_paths(),
            paths_before,
            "quote must not mutate state"
        );

        let mut execution_host = host.clone();
        let mut execution_vm = vm.clone();
        let actual = execution_host
            .syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &mut execution_vm)
            .expect("execute schema encode");
        assert!(actual <= quote);
        assert!(quote < vm.remaining_gas());
        assert!(quote > u64::try_from(json_payload.len()).expect("length fits"));
    }

    #[test]
    fn schema_prepare_validates_headers_without_decoding_payloads() {
        let host = CoreHost::new();
        let schema: Name = "Order".parse().expect("schema name");
        let schema_payload = norito::to_bytes(&schema).expect("encode schema");

        let mut wrong_type_vm = IVM::new(10_000);
        let schema_ptr = wrong_type_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &schema_payload))
            .expect("allocate schema TLV");
        let value_ptr = wrong_type_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Blob, b"{}"))
            .expect("allocate wrong value TLV");
        wrong_type_vm.set_register(10, schema_ptr);
        wrong_type_vm.set_register(11, value_ptr);
        assert!(matches!(
            host.prepare_syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &wrong_type_vm),
            Err(VMError::NoritoInvalid)
        ));

        let mut malformed_vm = IVM::new(10_000);
        let schema_ptr = malformed_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &schema_payload))
            .expect("allocate schema TLV");
        let value_ptr = malformed_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, &[0xff, 0x00, 0x80]))
            .expect("allocate malformed JSON TLV");
        malformed_vm.set_register(10, schema_ptr);
        malformed_vm.set_register(11, value_ptr);
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &malformed_vm)
            .expect("header-valid malformed payload receives a safe bound");
        assert!(quote > 3);
        assert!(quote > malformed_vm.remaining_gas());
        assert_eq!(malformed_vm.register(10), schema_ptr);
        assert_eq!(malformed_vm.register(11), value_ptr);
    }

    #[test]
    fn codec_prepare_rejects_unknown_and_unsupported_pointer_types_from_the_header() {
        let host = CoreHost::new();
        let mut unknown = make_pointer_tlv(PointerType::Json, b"{}");
        unknown[..2].copy_from_slice(&u16::MAX.to_be_bytes());
        let unsupported = make_pointer_tlv(PointerType::AccountId, b"not decoded during quote");

        for (label, envelope) in [("unknown", unknown), ("unsupported", unsupported)] {
            let mut vm = IVM::new(10_000);
            let pointer = vm
                .alloc_input_tlv(&envelope)
                .expect("allocate wrong-type envelope");
            vm.set_register(10, pointer);
            assert_eq!(
                host.prepare_syscall(syscalls::SYSCALL_JSON_DECODE, &vm),
                Err(VMError::NoritoInvalid),
                "{label} pointer types must fail closed before gas debit"
            );
        }
    }

    #[test]
    fn json_get_json_quote_reserves_heap_sized_pointer_output() {
        let host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let syscall = crate::encoding::wide::encode_sys(
            crate::instruction::wide::system::SCALL,
            u8::try_from(syscalls::SYSCALL_JSON_GET_JSON).expect("syscall fits u8"),
        );
        let mut program = syscall.to_le_bytes().to_vec();
        program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        vm.load_code(&program).expect("load JSON getter program");
        let large_field = "x".repeat((Memory::INPUT_SIZE as usize) * 2);
        let json = Json::from(norito::json!({ "field": large_field }));
        let json_payload = norito::to_bytes(&json).expect("encode JSON");
        let key: Name = "field".parse().expect("key");
        let key_payload = norito::to_bytes(&key).expect("encode key");
        let json_pointer = vm
            .alloc_host_tlv(&make_pointer_tlv(PointerType::Json, &json_payload))
            .expect("allocate JSON TLV");
        let key_pointer = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
            .expect("allocate key TLV");
        vm.set_register(10, json_pointer);
        vm.set_register(11, key_pointer);

        let quote = host
            .prepare_syscall(syscalls::SYSCALL_JSON_GET_JSON, &vm)
            .expect("quote JSON getter");
        let expected = CoreHost::json_gas(
            json_payload.len().saturating_add(key_payload.len()),
            CoreHost::maximum_host_pointer_output_payload().saturating_add(16),
        );

        assert_eq!(quote, expected);
        assert_eq!(
            CoreHost::maximum_host_pointer_output_payload(),
            usize::try_from(Memory::HEAP_SIZE.max(Memory::INPUT_SIZE))
                .expect("V1 memory size fits usize")
                - (7 + iroha_crypto::Hash::LENGTH)
        );
        assert!(
            quote
                > CoreHost::json_gas(
                    json_payload.len().saturating_add(key_payload.len()),
                    Memory::INPUT_SIZE as usize + 16,
                ),
            "JSON_GET_JSON must reserve beyond the fixed INPUT arena"
        );

        let mut direct_vm = vm.clone();
        let actual = CoreHost::new()
            .syscall(syscalls::SYSCALL_JSON_GET_JSON, &mut direct_vm)
            .expect("execute heap-sized JSON getter");
        assert!(actual <= quote, "actual gas must fit the prepared quote");

        let mut host = host;
        vm.run_with_host(&mut host)
            .expect("dispatcher must not trap on the heap-sized JSON result");
        let (some, words) = crate::sum::read_words(
            &vm,
            vm.register(10),
            crate::sum::SumLayoutV1::option(1).expect("JSON option layout"),
        )
        .expect("read JSON option");
        assert!(some);
        let output_pointer = words[0];
        assert!((Memory::HEAP_START..Memory::INPUT_START).contains(&output_pointer));
        let output = vm
            .validate_tlv(output_pointer)
            .expect("heap-backed JSON result");
        assert_eq!(output.type_id, PointerType::Json);
        assert!(output.payload.len() > Memory::INPUT_SIZE as usize);
    }

    #[test]
    fn codec_results_spill_to_owned_heap_after_input_exhaustion() {
        let mut vm = IVM::new(u64::MAX);
        vm.alloc_input_tlv(&vec![0; Memory::INPUT_SIZE as usize])
            .expect("fill INPUT exactly");

        CoreHost::new()
            .syscall(syscalls::SYSCALL_JSON_OBJECT, &mut vm)
            .expect("materialize codec result after INPUT exhaustion");

        let output_pointer = vm.register(10);
        assert!((Memory::HEAP_START..Memory::INPUT_START).contains(&output_pointer));
        let output = vm
            .validate_tlv(output_pointer)
            .expect("validate HEAP result");
        assert_eq!(output.type_id, PointerType::Json);
        let decoded: Json = decode_from_bytes(output.payload).expect("decode empty JSON object");
        assert_eq!(decoded, Json::from(njson::Value::Object(njson::Map::new())));
    }

    #[test]
    fn json_get_json_rejects_forged_regions_and_corrupted_heap_envelopes() {
        let json = Json::from(norito::json!({ "field": { "nested": true } }));
        let json_payload = norito::to_bytes(&json).expect("encode JSON fixture");
        let json_envelope = make_pointer_tlv(PointerType::Json, &json_payload);
        let key: Name = "field".parse().expect("field key");
        let key_payload = norito::to_bytes(&key).expect("encode field key");

        for (label, pointer) in [
            ("unallocated HEAP", Memory::HEAP_START),
            ("OUTPUT", Memory::OUTPUT_START),
            ("stack", Memory::STACK_START),
        ] {
            let mut vm = IVM::new(u64::MAX);
            vm.store_bytes(pointer, &json_envelope)
                .unwrap_or_else(|error| panic!("store {label} JSON envelope: {error:?}"));
            let key_pointer = vm
                .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
                .expect("allocate field key");
            vm.set_register(10, pointer);
            vm.set_register(11, key_pointer);
            let registers_before = [vm.register(10), vm.register(11)];
            let writes_before = vm.memory.write_log();

            assert_eq!(
                CoreHost::new().prepare_syscall(syscalls::SYSCALL_JSON_GET_JSON, &vm),
                Err(VMError::NoritoInvalid),
                "{label} bytes must fail during quote preparation"
            );
            assert_eq!([vm.register(10), vm.register(11)], registers_before);
            assert_eq!(vm.memory.write_log(), writes_before);
            assert_eq!(
                CoreHost::new().syscall(syscalls::SYSCALL_JSON_GET_JSON, &mut vm),
                Err(VMError::NoritoInvalid),
                "{label} bytes must fail during execution"
            );
            assert_eq!([vm.register(10), vm.register(11)], registers_before);
        }

        let mut partial_vm = IVM::new(u64::MAX);
        let owned_json_bytes = json_envelope
            .len()
            .checked_sub(8)
            .expect("JSON envelope exceeds one HEAP alignment unit");
        let partial_pointer = partial_vm
            .alloc_heap(u64::try_from(owned_json_bytes).expect("partial TLV length fits u64"))
            .expect("allocate truncated HEAP range");
        partial_vm
            .store_bytes(partial_pointer, &json_envelope)
            .expect("store across unowned HEAP boundary");
        let key_pointer = partial_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
            .expect("allocate partial-case key");
        partial_vm.set_register(10, partial_pointer);
        partial_vm.set_register(11, key_pointer);
        assert_eq!(
            CoreHost::new().prepare_syscall(syscalls::SYSCALL_JSON_GET_JSON, &partial_vm),
            Err(VMError::NoritoInvalid)
        );

        let mut corrupted_vm = IVM::new(u64::MAX);
        let mut corrupted = json_envelope;
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        let corrupted_pointer = corrupted_vm
            .alloc_host_tlv(&corrupted)
            .expect("allocate hash-corrupted JSON envelope");
        let key_pointer = corrupted_vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
            .expect("allocate corrupted-case key");
        corrupted_vm.set_register(10, corrupted_pointer);
        corrupted_vm.set_register(11, key_pointer);
        CoreHost::new()
            .prepare_syscall(syscalls::SYSCALL_JSON_GET_JSON, &corrupted_vm)
            .expect("header-only quote remains bounded for a corrupted payload hash");
        let registers_before = [corrupted_vm.register(10), corrupted_vm.register(11)];
        assert_eq!(
            CoreHost::new().syscall(syscalls::SYSCALL_JSON_GET_JSON, &mut corrupted_vm),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(
            [corrupted_vm.register(10), corrupted_vm.register(11)],
            registers_before
        );
    }

    fn configure_oversized_codec_case(vm: &mut IVM, number: u32, oversized_pointer: u64) {
        let key: Name = "field".parse().expect("codec fixture key");
        let key_payload = norito::to_bytes(&key).expect("encode codec fixture key");
        let key_pointer = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_payload))
            .expect("allocate codec fixture key");
        match number {
            syscalls::SYSCALL_SCHEMA_ENCODE | syscalls::SYSCALL_SCHEMA_DECODE => {
                vm.set_register(10, key_pointer);
                vm.set_register(11, oversized_pointer);
            }
            syscalls::SYSCALL_JSON_SET_I64
            | syscalls::SYSCALL_JSON_GET_JSON
            | syscalls::SYSCALL_JSON_GET_NAME => {
                vm.set_register(10, oversized_pointer);
                vm.set_register(11, key_pointer);
                vm.set_register(12, 1);
            }
            _ => vm.set_register(10, oversized_pointer),
        }
    }

    fn generic_codec_input_cap_cases() -> [(u32, PointerType); 7] {
        [
            (syscalls::SYSCALL_SCHEMA_ENCODE, PointerType::Json),
            (syscalls::SYSCALL_SCHEMA_DECODE, PointerType::NoritoBytes),
            (syscalls::SYSCALL_JSON_ENCODE, PointerType::Json),
            (syscalls::SYSCALL_JSON_DECODE, PointerType::NoritoBytes),
            (syscalls::SYSCALL_JSON_SET_I64, PointerType::Json),
            (syscalls::SYSCALL_JSON_GET_NAME, PointerType::Json),
            (syscalls::SYSCALL_NAME_DECODE, PointerType::NoritoBytes),
        ]
    }

    #[test]
    fn codec_prepare_rejects_over_generic_cap_owned_payloads_without_output_mutation() {
        let host = CoreHost::new();
        let payload = vec![0x5a; gas::HOST_CODEC_MAX_INPUT_BYTES + 1];
        for (number, pointer_type) in generic_codec_input_cap_cases() {
            let mut vm = IVM::new(u64::MAX);
            let oversized_pointer = vm
                .alloc_input_tlv(&make_pointer_tlv(pointer_type, &payload))
                .expect("allocate oversized codec fixture");
            configure_oversized_codec_case(&mut vm, number, oversized_pointer);
            let registers_before = [vm.register(10), vm.register(11), vm.register(12)];
            let writes_before = vm.memory.write_log();

            assert_eq!(
                host.prepare_syscall(number, &vm),
                Err(VMError::NoritoInvalid),
                "oversized owned payload for syscall {number:#x} must fail before debit"
            );
            assert_eq!(
                [vm.register(10), vm.register(11), vm.register(12)],
                registers_before,
                "preparation for syscall {number:#x} mutated output registers"
            );
            assert_eq!(
                vm.memory.write_log(),
                writes_before,
                "preparation for syscall {number:#x} mutated guest memory"
            );
        }
    }

    #[test]
    fn codec_prepare_rejects_over_cap_code_literals_without_output_mutation() {
        let host = CoreHost::new();
        let payload = vec![0xa5; gas::HOST_CODEC_MAX_INPUT_BYTES + 1];
        for (number, pointer_type) in generic_codec_input_cap_cases() {
            let literal = make_pointer_tlv(pointer_type, &payload);
            let (program, literal_pointers) = assemble_program_with_literals(&[&literal]);
            let mut vm = IVM::new(u64::MAX);
            vm.load_program(&program)
                .expect("load oversized literal fixture");
            configure_oversized_codec_case(&mut vm, number, literal_pointers[0]);
            let registers_before = [vm.register(10), vm.register(11), vm.register(12)];
            let writes_before = vm.memory.write_log();

            assert_eq!(
                host.prepare_syscall(number, &vm),
                Err(VMError::NoritoInvalid),
                "oversized literal payload for syscall {number:#x} must fail before debit"
            );
            assert_eq!(
                [vm.register(10), vm.register(11), vm.register(12)],
                registers_before,
                "literal preparation for syscall {number:#x} mutated output registers"
            );
            assert_eq!(
                vm.memory.write_log(),
                writes_before,
                "literal preparation for syscall {number:#x} mutated guest memory"
            );
        }
    }

    #[test]
    fn near_codec_input_cap_quote_covers_successful_execution() {
        let mut low = 0_usize;
        let mut high = gas::HOST_CODEC_MAX_INPUT_BYTES;
        let mut selected = None;
        while low <= high {
            let middle = low + (high - low) / 2;
            let json = Json::from_str_norito(&format!(r#"{{"payload":"{}"}}"#, "x".repeat(middle)))
                .expect("construct near-cap JSON");
            let payload = norito::to_bytes(&json).expect("encode near-cap JSON");
            if payload.len() <= gas::HOST_CODEC_MAX_INPUT_BYTES {
                selected = Some(payload);
                low = middle.saturating_add(1);
            } else if middle == 0 {
                break;
            } else {
                high = middle - 1;
            }
        }
        let payload = selected.expect("one JSON payload fits the codec cap");
        assert!(
            gas::HOST_CODEC_MAX_INPUT_BYTES.saturating_sub(payload.len()) < 64,
            "fixture should exercise the cap, got {} of {} bytes",
            payload.len(),
            gas::HOST_CODEC_MAX_INPUT_BYTES
        );

        let mut vm = IVM::new(u64::MAX);
        let pointer = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, &payload))
            .expect("allocate near-cap JSON");
        vm.set_register(10, pointer);
        let mut host = CoreHost::new();
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_JSON_ENCODE, &vm)
            .expect("quote near-cap JSON encode");
        let actual = host
            .syscall(syscalls::SYSCALL_JSON_ENCODE, &mut vm)
            .expect("execute near-cap JSON encode");
        assert!(actual <= quote, "actual {actual} exceeds quote {quote}");
        let output = vm
            .validate_tlv(vm.register(10))
            .expect("near-cap JSON output");
        assert!(output.payload.len() <= gas::HOST_CODEC_MAX_OUTPUT_BYTES);
    }

    #[test]
    fn schema_encode_preflights_exact_gas_before_guest_output() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let schema: Name = "Order".parse().expect("schema name");
        let json = Json::from_str_norito(&format!(
            r#"{{"qty":9223372036854775807,"side":"{}"}}"#,
            "cd".repeat(1024)
        ))
        .expect("large JSON");
        let schema_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&schema).expect("encode schema"),
            ))
            .expect("allocate schema TLV");
        let json_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Json,
                &norito::to_bytes(&json).expect("encode JSON"),
            ))
            .expect("allocate JSON TLV");
        let scall = encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(syscalls::SYSCALL_SCHEMA_ENCODE).expect("syscall id fits in u8"),
        );
        let program = assemble_program(&[scall, encoding::wide::encode_halt()]);
        vm.load_program(&program).expect("load schema program");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, json_ptr);
        vm.set_gas_limit(20);

        let error = vm
            .run_with_host(&mut host)
            .expect_err("schema response exceeds the syscall reserve");

        assert_eq!(error, VMError::OutOfGas);
        assert_eq!(
            vm.remaining_gas(),
            20 - gas::cost_of(scall).expect("SCALL is scheduled"),
            "an unaffordable syscall quote must not be partially debited"
        );
        assert_eq!(
            vm.register(10),
            schema_ptr,
            "must not allocate guest output"
        );
        assert_eq!(vm.register(11), json_ptr, "must not mutate arguments");
    }

    #[test]
    fn core_host_codec_helpers_charge_payload_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        load_state_map_schema(&mut vm, "orders", EmbeddedStateType::Int);

        vm.set_register(10, 42);
        let encode_int_gas = host
            .syscall(syscalls::SYSCALL_ENCODE_INT, &mut vm)
            .expect("encode int");
        let int_ptr = vm.register(10);
        let int_tlv = vm.validate_tlv(int_ptr).expect("int tlv");
        let int_len = int_tlv.payload.len();
        assert_eq!(encode_int_gas, CoreHost::numeric_payload_gas(0, int_len));
        vm.set_register(10, int_ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_DECODE_INT, &mut vm),
            Ok(CoreHost::numeric_payload_gas(int_len, 0))
        );

        let base: Name = "orders".parse().expect("base name");
        let base_bytes = norito::to_bytes(&base).expect("encode base");
        let base_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &base_bytes))
            .expect("alloc base");
        let key_bytes =
            crate::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(7))
                .expect("encode canonical int key");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &key_bytes))
            .expect("alloc key");
        vm.set_register(10, base_ptr);
        vm.set_register(11, key_ptr);
        let path_norito_gas = host
            .syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &mut vm)
            .expect("path norito key");
        let path_norito_tlv = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("path norito tlv");
        let path_name: Name =
            norito::decode_from_bytes(path_norito_tlv.payload).expect("decode canonical path");
        assert_eq!(
            path_name.as_ref(),
            format!("orders/{}", hex::encode(&key_bytes))
        );
        assert_eq!(
            path_norito_gas,
            CoreHost::path_gas(
                base_bytes.len() + key_bytes.len(),
                path_norito_tlv.payload.len()
            )
        );

        let json = Json::from_str_norito(r#"{"qty":10}"#).expect("json");
        let json_bytes = norito::to_bytes(&json).expect("encode json");
        let json_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, &json_bytes))
            .expect("alloc json");
        vm.set_register(10, json_ptr);
        let json_encode_gas = host
            .syscall(syscalls::SYSCALL_JSON_ENCODE, &mut vm)
            .expect("json encode");
        let json_encoded_ptr = vm.register(10);
        let json_encoded = vm
            .memory
            .validate_tlv(json_encoded_ptr)
            .expect("json encoded tlv");
        let json_encoded_len = json_encoded.payload.len();
        assert_eq!(
            json_encode_gas,
            CoreHost::json_gas(json_bytes.len(), json_encoded_len)
        );

        vm.set_register(10, json_encoded_ptr);
        let json_decode_gas = host
            .syscall(syscalls::SYSCALL_JSON_DECODE, &mut vm)
            .expect("json decode");
        let decoded_json_ptr = vm.register(10);
        let decoded_json = vm
            .memory
            .validate_tlv(decoded_json_ptr)
            .expect("decoded json tlv");
        assert_eq!(
            json_decode_gas,
            CoreHost::json_gas(json_encoded_len, decoded_json.payload.len())
        );

        let key: Name = "answer".parse().expect("json key");
        let key_name_bytes = norito::to_bytes(&key).expect("encode key name");
        let key_name_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &key_name_bytes))
            .expect("alloc key name");
        vm.set_register(10, 0);
        let object_gas = host
            .syscall(syscalls::SYSCALL_JSON_OBJECT, &mut vm)
            .expect("json object");
        let object_ptr = vm.register(10);
        let object = vm.validate_tlv(object_ptr).expect("object tlv");
        let object_len = object.payload.len();
        assert_eq!(object_gas, CoreHost::json_gas(0, object_len));

        vm.set_register(10, object_ptr);
        vm.set_register(11, key_name_ptr);
        vm.set_register(12, 99);
        let set_gas = host
            .syscall(syscalls::SYSCALL_JSON_SET_I64, &mut vm)
            .expect("json set");
        let object_with_value_ptr = vm.register(10);
        let object_with_value = vm
            .memory
            .validate_tlv(object_with_value_ptr)
            .expect("json set tlv");
        let object_with_value_len = object_with_value.payload.len();
        assert_eq!(
            set_gas,
            CoreHost::json_gas(
                object_len + key_name_bytes.len() + core::mem::size_of::<i64>(),
                object_with_value_len
            )
        );

        vm.set_register(10, object_with_value_ptr);
        vm.set_register(11, key_name_ptr);
        let get_gas = host
            .syscall(syscalls::SYSCALL_JSON_GET_INT, &mut vm)
            .expect("json get");
        let (present, words) = crate::sum::read_words(
            &vm,
            vm.register(10),
            crate::sum::SumLayoutV1::option(1).expect("int Option layout"),
        )
        .expect("read int Option");
        assert!(present, "numeric JSON integer tokens must be accepted");
        assert_eq!(words.len(), 1);
        let int = vm.validate_tlv(words[0]).expect("int TLV");
        assert_eq!(int.type_id, PointerType::Int);
        assert_eq!(
            get_gas,
            CoreHost::json_gas(
                object_with_value_len + key_name_bytes.len(),
                int.payload.len() + 16,
            )
        );

        let name: Name = "wonderland".parse().expect("name");
        let name_bytes = norito::to_bytes(&name).expect("encode name");
        let name_norito_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &name_bytes))
            .expect("alloc name bytes");
        vm.set_register(10, name_norito_ptr);
        let name_decode_gas = host
            .syscall(syscalls::SYSCALL_NAME_DECODE, &mut vm)
            .expect("name decode");
        let name_tlv = vm.validate_tlv(vm.register(10)).expect("name tlv");
        assert_eq!(
            name_decode_gas,
            CoreHost::name_decode_gas(name_bytes.len(), name_tlv.payload.len())
        );
    }

    #[test]
    fn name_decode_rejects_retired_and_noncanonical_payload_forms() {
        let name: Name = "canonical".parse().expect("name");
        let canonical = norito::to_bytes(&name).expect("encode canonical Name");
        let alternate_layout = {
            let _flags = norito::core::DecodeFlagsGuard::enter(0);
            norito::to_bytes(&name).expect("encode alternate Name layout")
        };
        assert_ne!(alternate_layout, canonical);

        for (label, payload) in [
            ("raw UTF-8", name.as_ref().as_bytes().to_vec()),
            (
                "framed String",
                norito::to_bytes(&name.to_string()).expect("encode String"),
            ),
            ("alternate Name layout", alternate_layout),
        ] {
            let mut vm = IVM::new(u64::MAX);
            let pointer = vm
                .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &payload))
                .unwrap_or_else(|error| panic!("allocate {label} payload: {error:?}"));
            vm.set_register(10, pointer);

            assert_eq!(
                CoreHost::new().syscall(syscalls::SYSCALL_NAME_DECODE, &mut vm),
                Err(VMError::DecodeError),
                "{label} must not be accepted as a first-release Name frame"
            );
            assert_eq!(vm.register(10), pointer);
        }
    }

    fn maximum_state_map_base() -> Name {
        let mut low = 1_usize;
        let mut high = syscalls::STATE_MAP_MAX_BASE_BYTES;
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            let candidate: Name = "m".repeat(middle).parse().expect("ASCII map base");
            if crate::host::state_path_name_payload_len(&candidate)
                .is_ok_and(|len| len <= syscalls::STATE_MAP_MAX_BASE_BYTES)
            {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        "m".repeat(low).parse().expect("maximum map base")
    }

    #[test]
    fn maximum_state_map_path_roundtrips_into_durable_state() {
        let base = maximum_state_map_base();
        let base_payload = norito::to_bytes(&base).expect("encode maximum base");
        let key_payload = make_pointer_tlv(
            PointerType::Blob,
            &vec![0x5au8; syscalls::STATE_MAP_MAX_KEY_BYTES - TLV_ENVELOPE_OVERHEAD],
        );
        assert_eq!(key_payload.len(), syscalls::STATE_MAP_MAX_KEY_BYTES);
        let mut vm = IVM::new(u64::MAX);
        load_state_map_schema(&mut vm, base.as_ref(), EmbeddedStateType::Bytes);
        let base_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &base_payload))
            .expect("allocate base");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &key_payload))
            .expect("allocate key");
        vm.set_register(10, base_ptr);
        vm.set_register(11, key_ptr);
        let mut host = CoreHost::new();
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &vm)
            .expect("quote maximum path");
        let actual = host
            .syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &mut vm)
            .expect("build maximum path");
        assert!(actual <= quote);
        let path_ptr = vm.register(10);
        let path_tlv = vm.validate_tlv(path_ptr).expect("path TLV");
        let path: Name = norito::decode_from_bytes(path_tlv.payload).expect("decode path");
        assert!(
            crate::host::state_path_name_payload_len(&path).expect("path length")
                <= syscalls::STATE_MAX_PATH_BYTES
        );

        let value = bytes_state_value_record(b"roundtrip");
        let value_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &value))
            .expect("allocate value");
        vm.set_register(10, path_ptr);
        vm.set_register(11, value_ptr);
        host.syscall(syscalls::SYSCALL_STATE_SET, &mut vm)
            .expect("store under maximum path");
        assert_eq!(
            host.state_bytes(path.as_ref()).as_deref(),
            Some(value.as_slice())
        );
    }

    #[test]
    fn state_map_path_quote_minus_one_allocates_no_output() {
        let base = maximum_state_map_base();
        let base_payload = norito::to_bytes(&base).expect("encode maximum base");
        let key_payload = make_pointer_tlv(
            PointerType::Blob,
            &vec![0x3cu8; syscalls::STATE_MAP_MAX_KEY_BYTES - TLV_ENVELOPE_OVERHEAD],
        );
        assert_eq!(key_payload.len(), syscalls::STATE_MAP_MAX_KEY_BYTES);
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&assemble_state_map_program(
            &[
                encoding::wide::encode_sys(
                    instruction::wide::system::SCALL,
                    syscalls::SYSCALL_BUILD_PATH_KEY_NORITO as u8,
                ),
                encoding::wide::encode_halt(),
            ],
            base.as_ref(),
            EmbeddedStateType::Bytes,
        ))
        .expect("load program");
        let base_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &base_payload))
            .expect("allocate base");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &key_payload))
            .expect("allocate key");
        vm.set_register(10, base_ptr);
        vm.set_register(11, key_ptr);
        let mut host = CoreHost::new();
        let quote = host
            .prepare_syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &vm)
            .expect("quote path");
        vm.set_gas_limit(quote.saturating_add(4));

        assert_eq!(vm.run_with_host(&mut host), Err(VMError::OutOfGas));
        assert_eq!(vm.register(10), base_ptr);
        assert_eq!(vm.register(11), key_ptr);
        assert!(host.state_paths().is_empty());
    }

    #[test]
    fn state_map_path_rejects_oversized_key_during_preparation() {
        let base: Name = "map".parse().expect("base");
        let base_payload = norito::to_bytes(&base).expect("encode base");
        let key_payload = vec![0u8; syscalls::STATE_MAP_MAX_KEY_BYTES + 1];
        let mut vm = IVM::new(u64::MAX);
        let base_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, &base_payload))
            .expect("allocate base");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::NoritoBytes, &key_payload))
            .expect("allocate key");
        vm.set_register(10, base_ptr);
        vm.set_register(11, key_ptr);
        let host = CoreHost::new();
        assert_eq!(
            host.prepare_syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &vm),
            Err(VMError::NoritoInvalid)
        );
    }

    #[test]
    fn decode_int_syscall_rejects_non_norito_i64_payloads() {
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_DECODE_INT as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        let cases = vec![
            ("utf8-decimal", b"-77".to_vec()),
            (
                "norito-string",
                norito::to_bytes(&"-19".to_string()).expect("encode string"),
            ),
        ];

        for (label, payload) in cases {
            let mut vm = IVM::new(u64::MAX);
            vm.set_host(CoreHost::new());
            let ptr = vm
                .alloc_input_tlv(&make_tlv(&payload))
                .expect("alloc payload tlv");
            vm.load_program(&program).expect("load program");
            vm.set_register(10, ptr);
            let err = vm
                .run()
                .expect_err("decode_int should reject non-i64 payload");
            assert!(
                matches!(err, VMError::DecodeError),
                "decode_int payload variant {label} should yield DecodeError, got {err:?}"
            );
        }
    }

    #[test]
    fn tlv_eq_syscall_compares_payloads() {
        let mut direct_vm = IVM::new(u64::MAX);
        let mut direct_host = CoreHost::new();
        let direct_ptr1 = direct_vm
            .alloc_input_tlv(&make_tlv(b"same"))
            .expect("alloc tlv1");
        let direct_ptr2 = direct_vm
            .alloc_input_tlv(&make_tlv(b"same"))
            .expect("alloc tlv2");
        direct_vm.set_register(10, direct_ptr1);
        direct_vm.set_register(11, direct_ptr2);
        assert_eq!(
            direct_host.syscall(syscalls::SYSCALL_TLV_EQ, &mut direct_vm),
            Ok(CoreHost::tlv_eq_gas(4, 4))
        );
        assert_eq!(direct_vm.register(10), 1);

        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_TLV_EQ as u8,
            ),
            encoding::wide::encode_halt(),
        ]);

        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let ptr1 = vm.alloc_input_tlv(&make_tlv(b"same")).expect("alloc tlv1");
        let ptr2 = vm.alloc_input_tlv(&make_tlv(b"same")).expect("alloc tlv2");
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ptr1);
        vm.set_register(11, ptr2);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 1);

        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let ptr1 = vm.alloc_input_tlv(&make_tlv(b"left")).expect("alloc tlv1");
        let ptr2 = vm.alloc_input_tlv(&make_tlv(b"right")).expect("alloc tlv2");
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ptr1);
        vm.set_register(11, ptr2);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn core_host_tlv_eq_rejects_equal_invalid_raw_addresses() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        vm.set_register(10, Memory::OUTPUT_START);
        vm.set_register(11, Memory::OUTPUT_START);

        assert!(host.prepare_syscall(syscalls::SYSCALL_TLV_EQ, &vm).is_err());
        assert!(host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm).is_err());

        vm.set_register(10, 0);
        vm.set_register(11, Memory::OUTPUT_START);
        assert!(host.prepare_syscall(syscalls::SYSCALL_TLV_EQ, &vm).is_err());
        assert!(host.syscall(syscalls::SYSCALL_TLV_EQ, &mut vm).is_err());
    }

    #[test]
    fn core_host_debug_log_quote_and_execution_charge_payload_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let pointer = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Blob, &[b'x'; 256]))
            .expect("allocate debug log fixture");
        vm.set_register(10, pointer);
        let mut host = CoreHost::new();

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
    fn core_host_debug_log_accepts_owned_heap_and_rejects_unowned_regions() {
        let mut vm = IVM::new(u64::MAX);
        let envelope = make_pointer_tlv(PointerType::Blob, b"heap-backed debug message");
        let heap_pointer = vm
            .alloc_heap(u64::try_from(envelope.len()).expect("envelope length fits u64"))
            .expect("allocate owned heap envelope");
        vm.store_bytes(heap_pointer, &envelope)
            .expect("store owned heap envelope");
        vm.set_register(10, heap_pointer);
        let mut host = CoreHost::new();

        let quoted = host
            .prepare_syscall(syscalls::SYSCALL_DEBUG_LOG, &vm)
            .expect("quote owned heap debug message");
        let actual = host
            .syscall(syscalls::SYSCALL_DEBUG_LOG, &mut vm)
            .expect("log owned heap debug message");
        assert_eq!(quoted, actual);
        assert_eq!(actual, debug_log_gas(b"heap-backed debug message".len()));

        for forged in [
            Memory::OUTPUT_START,
            Memory::STACK_START,
            Memory::HEAP_START
                .checked_add(vm.memory.heap_allocated_len())
                .expect("unallocated heap pointer fits"),
        ] {
            vm.set_register(10, forged);
            assert_eq!(
                host.prepare_syscall(syscalls::SYSCALL_DEBUG_LOG, &vm),
                Err(VMError::NoritoInvalid),
                "quote accepted forged pointer 0x{forged:016x}"
            );
            assert_eq!(
                host.syscall(syscalls::SYSCALL_DEBUG_LOG, &mut vm),
                Err(VMError::NoritoInvalid),
                "dispatch accepted forged pointer 0x{forged:016x}"
            );
            assert_eq!(vm.register(10), forged);
        }
    }

    #[test]
    fn tlv_len_syscall_charges_payload_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        let ptr = vm.alloc_input_tlv(&make_tlv(b"length")).expect("alloc tlv");

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_TLV_LEN, &mut vm),
            Ok(CoreHost::tlv_len_gas(6))
        );
        assert_eq!(vm.register(10), 6);
    }

    #[test]
    fn core_host_sysvars_charge_gas() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new().with_current_time_ms(42);

        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS, &mut vm),
            Ok(CoreHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 42);

        assert_eq!(
            host.syscall(syscalls::SYSCALL_SYSVAR_CHAIN_ID, &mut vm),
            Ok(CoreHost::sysvar_gas(0))
        );
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn pointer_to_norito_roundtrips_via_pointer_from_norito() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, b"wonderland"))
            .expect("alloc tlv");
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_POINTER_TO_NORITO as u8,
            ),
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_POINTER_FROM_NORITO as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ptr);
        vm.set_register(11, PointerType::Name as u64);
        vm.run().expect("run");
        let out_ptr = vm.register(10);
        let tlv = vm.validate_tlv(out_ptr).expect("out tlv");
        assert_eq!(tlv.type_id, PointerType::Name);
        assert_eq!(tlv.payload, b"wonderland");
    }

    #[test]
    fn pointer_norito_helpers_charge_envelope_bytes() {
        let mut vm = IVM::new(u64::MAX);
        let mut host = CoreHost::new();
        let payload = b"wonderland";
        let ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, payload))
            .expect("alloc tlv");
        let envelope_len = 2 + 1 + 4 + payload.len() + iroha_crypto::Hash::LENGTH;

        vm.set_register(10, ptr);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_TO_NORITO, &mut vm),
            Ok(CoreHost::pointer_gas(envelope_len))
        );
        let wrapped_ptr = vm.register(10);
        let wrapped = vm.validate_tlv(wrapped_ptr).expect("wrapped tlv");
        assert_eq!(wrapped.type_id, PointerType::NoritoBytes);
        assert_eq!(wrapped.payload.len(), envelope_len);

        vm.set_register(10, wrapped_ptr);
        vm.set_register(11, PointerType::Name as u64);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm),
            Ok(CoreHost::pointer_gas(envelope_len))
        );
        let roundtrip = vm.validate_tlv(vm.register(10)).expect("roundtrip tlv");
        assert_eq!(roundtrip.type_id, PointerType::Name);
        assert_eq!(roundtrip.payload, payload);

        let retired_carrier = make_pointer_tlv(
            PointerType::Blob,
            &make_pointer_tlv(PointerType::Name, payload),
        );
        let retired_carrier_ptr = vm
            .alloc_input_tlv(&retired_carrier)
            .expect("allocate retired Blob carrier");
        vm.set_register(10, retired_carrier_ptr);
        vm.set_register(11, PointerType::Name as u64);
        assert_eq!(
            host.prepare_syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &vm),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(vm.register(10), retired_carrier_ptr);

        vm.set_register(10, 0);
        vm.set_register(11, 0);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_POINTER_FROM_NORITO, &mut vm),
            Ok(CoreHost::pointer_gas(0))
        );
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn json_decode_null_pointer_returns_zero() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_JSON_DECODE as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, 0);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn json_decode_rejects_retired_blob_payload_forms() {
        let mut host = CoreHost::new();
        let json = Json::from_str_norito(
            r#"{"fastpq_binding":{"verified_effect_type":"aed_to_pkr_settlement"}}"#,
        )
        .expect("json");
        let encoded = norito::to_bytes(&json).expect("encode json");
        for payload in [json.get().as_bytes(), encoded.as_slice()] {
            let mut vm = IVM::new(u64::MAX);
            let ptr = vm
                .alloc_input_tlv(&make_pointer_tlv(PointerType::Blob, payload))
                .expect("allocate retired blob carrier");
            vm.set_register(10, ptr);
            assert_eq!(
                host.syscall(syscalls::SYSCALL_JSON_DECODE, &mut vm),
                Err(VMError::NoritoInvalid)
            );
            assert_eq!(vm.register(10), ptr);
        }
    }

    #[test]
    fn name_decode_null_pointer_returns_zero() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_NAME_DECODE as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, 0);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn pointer_from_norito_null_pointer_returns_zero() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_POINTER_FROM_NORITO as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, 0);
        vm.set_register(11, 0);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn input_publish_tlv_null_pointer_is_noop() {
        let mut vm = IVM::new(u64::MAX);
        vm.set_host(CoreHost::new());
        let ptr = vm.alloc_input_tlv(&make_tlv(b"reuse")).expect("alloc tlv");
        let program = assemble_program(&[
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            ),
            encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 10, 10, 10),
            encoding::wide::encode_sys(
                instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            ),
            encoding::wide::encode_halt(),
        ]);
        vm.load_program(&program).expect("load program");
        vm.set_register(10, ptr);
        vm.run().expect("run");
        assert_eq!(vm.register(10), 0);
    }

    #[test]
    fn input_publish_tlv_charges_envelope_bytes() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let tlv = make_tlv(b"reuse");
        let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
        vm.set_register(10, ptr);

        assert_eq!(
            host.syscall(syscalls::SYSCALL_INPUT_PUBLISH_TLV, &mut vm),
            Ok(CoreHost::input_publish_gas(tlv.len()))
        );
    }

    #[test]
    fn json_quantity_getter_accepts_only_canonical_strings() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let json = Json::from_str_norito(r#"{"amount":"1.25"}"#).expect("quantity JSON");
        let json_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Json,
                &norito::to_bytes(&json).expect("encode JSON"),
            ))
            .expect("allocate JSON");
        let key: Name = "amount".parse().expect("amount key");
        let key_ptr = vm
            .alloc_input_tlv(&make_pointer_tlv(
                PointerType::Name,
                &norito::to_bytes(&key).expect("encode key"),
            ))
            .expect("allocate key");

        for syscall in [
            syscalls::SYSCALL_JSON_GET_QUANTITY,
            syscalls::SYSCALL_JSON_GET_QUANTITY_DIRECT,
        ] {
            vm.set_register(10, json_ptr);
            vm.set_register(11, key_ptr);
            host.syscall(syscall, &mut vm).expect("get quantity");
            let (some, words) = crate::sum::read_words(
                &vm,
                vm.register(10),
                crate::sum::SumLayoutV1::option(1).expect("quantity option layout"),
            )
            .expect("quantity option");
            assert!(some);
            let tlv = vm.validate_tlv(words[0]).expect("quantity TLV");
            assert_eq!(tlv.type_id, PointerType::Quantity);
            let quantity = QuantityValueV1::decode_frame(tlv.payload)
                .expect("decode quantity frame")
                .into_quantity();
            assert_eq!(quantity.to_string(), "1.25");
        }

        for invalid in [
            r#"{"amount":"-1"}"#,
            r#"{"amount":"1.2500"}"#,
            r#"{"amount":1}"#,
        ] {
            let invalid = Json::from_str_norito(invalid).expect("invalid quantity JSON");
            let invalid_ptr = vm
                .alloc_input_tlv(&make_pointer_tlv(
                    PointerType::Json,
                    &norito::to_bytes(&invalid).expect("encode invalid JSON"),
                ))
                .expect("allocate invalid JSON");
            vm.set_register(10, invalid_ptr);
            vm.set_register(11, key_ptr);
            host.syscall(syscalls::SYSCALL_JSON_GET_QUANTITY, &mut vm)
                .expect("invalid quantity is Option::none");
            assert_eq!(
                crate::sum::read_words(
                    &vm,
                    vm.register(10),
                    crate::sum::SumLayoutV1::option(1).expect("quantity option layout"),
                ),
                Ok((false, vec![]))
            );
        }
    }

    #[test]
    fn alloc_syscall_charges_by_abi_words() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        vm.set_register(10, 8);

        assert_eq!(host.syscall(syscalls::SYSCALL_ALLOC, &mut vm), Ok(2));
        assert!(vm.register(10) >= Memory::HEAP_START);
    }

    #[test]
    fn mutation_validation_syscalls_charge_nonzero_gas() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(u64::MAX);
        let account = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::AccountId, b""))
            .expect("alloc account");
        let name = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Name, b"k"))
            .expect("alloc name");
        let json_payload = b"{\"v\":1}";
        let json = vm
            .alloc_input_tlv(&make_pointer_tlv(PointerType::Json, json_payload))
            .expect("alloc json");
        vm.set_register(10, account);
        vm.set_register(11, name);
        vm.set_register(12, json);

        assert_eq!(
            host.syscall(syscalls::SYSCALL_SET_ACCOUNT_DETAIL, &mut vm),
            Ok(CoreHost::mutation_gas(json_payload.len()))
        );
    }

    #[test]
    fn fastpq_batch_requires_entries() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(1_000);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm),
            Ok(gas::G_FASTPQ_BATCH)
        );
        let err = host
            .syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm)
            .expect_err("ending empty batch should fail");
        assert!(matches!(err.as_unmetered(), VMError::DecodeError));
        assert_eq!(err.metered_gas(), Some(gas::G_FASTPQ_BATCH));
    }

    #[test]
    fn fastpq_batch_validates_transfer_entries() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(1_000);
        let from_account = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let from = make_pointer_tlv(PointerType::AccountId, from_account.as_bytes());
        vm.memory.preload_input(0, &from).expect("preload from");
        let to_account = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
        let to = make_pointer_tlv(PointerType::AccountId, to_account.as_bytes());
        vm.memory
            .preload_input(from.len() as u64 + 8, &to)
            .expect("preload to");
        let asset = make_pointer_tlv(
            PointerType::AssetDefinitionId,
            b"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        );
        vm.memory
            .preload_input(from.len() as u64 + to.len() as u64 + 16, &asset)
            .expect("preload asset");
        let amount_offset = from.len() as u64 + to.len() as u64 + asset.len() as u64 + 24;
        let amount = make_amount_tlv(Quantity::from(10_u64));
        vm.memory
            .preload_input(amount_offset, &amount)
            .expect("preload amount");
        vm.set_register(10, Memory::INPUT_START);
        vm.set_register(11, Memory::INPUT_START + from.len() as u64 + 8);
        vm.set_register(
            12,
            Memory::INPUT_START + from.len() as u64 + to.len() as u64 + 16,
        );
        vm.set_register(13, Memory::INPUT_START + amount_offset);

        assert_eq!(
            host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm),
            Ok(gas::G_FASTPQ_BATCH)
        );
        host.syscall(syscalls::SYSCALL_TRANSFER_V1, &mut vm)
            .expect("push entry");
        assert_eq!(
            host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm),
            Ok(gas::G_FASTPQ_BATCH)
        );
    }
}
