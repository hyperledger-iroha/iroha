//! Minimal core host shim validating pointer-ABI TLVs for representative syscalls.
//!
//! This host does not execute real ISI. It only validates the pointer-ABI
//! arguments and returns success. It is intended for end-to-end tests that
//! exercise TLV validation from VM bytecode through host dispatch.

use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_crypto::{Hash as IrohaHash, Sm3Digest};
use iroha_data_model::{
    account::AccountId,
    isi::transfer::TransferAssetBatch,
    nexus::{AxtPolicyEntry, AxtPolicySnapshot, DataSpaceId},
    prelude::{AssetDefinitionId, Name, NftId},
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec},
};
use norito::{decode_from_bytes, json as njson, to_bytes};
use sha2::{Digest as Sha2Digest, Sha256};
use sha3_hash::{Digest as Sha3Digest, Sha3_256};

use crate::{
    VMError,
    axt::{self, AxtPolicy},
    host::{AccessLog, IVMHost},
    ivm::IVM,
    memory::Memory,
    metadata::{CONTRACT_INTERFACE_SECTION_MAGIC, LITERAL_SECTION_MAGIC},
    mock_wsv::{MockWorldStateView, SpaceDirectoryAxtPolicy},
    parallel::StateUpdate,
    pointer_abi::{self, PointerType},
    schema_registry::{DefaultRegistry, SchemaInfo, SchemaRegistry},
    state_overlay::{DurableStateOverlay, DurableStateSnapshot},
    syscalls,
};

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

pub struct CoreHost {
    // Simple in-memory state for STATE_{GET,SET,DEL} syscalls keyed by path.
    state: DurableStateOverlay,
    schema: Box<dyn SchemaRegistry + Send + Sync>,
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

fn parse_json_numeric_field(field: &njson::Value) -> Result<Numeric, VMError> {
    match field {
        njson::Value::String(raw) => raw.parse::<Numeric>().map_err(|_| VMError::DecodeError),
        njson::Value::Number(njson::native::Number::I64(value)) => Ok(Numeric::from(*value)),
        njson::Value::Number(njson::native::Number::U64(value)) => Ok(Numeric::from(*value)),
        _ => Err(VMError::DecodeError),
    }
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
            schema: Box::new(DefaultRegistry::new()),
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
        }
    }

    /// Construct a CoreHost with a specific schema registry implementation.
    pub fn new_with_registry(reg: Box<dyn SchemaRegistry + Send + Sync>) -> Self {
        Self {
            state: DurableStateOverlay::in_memory(),
            schema: reg,
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

    /// Borrow the raw Norito payload stored under `path`, if present.
    pub fn state_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.state.get(path)
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
        let tlv = vm.memory.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::AxtDescriptor {
            return Err(VMError::NoritoInvalid);
        }
        let descriptor: axt::AxtDescriptor =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        axt::validate_descriptor(&descriptor)?;
        let binding = axt::compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        self.reset_axt_proof_cache_for_slot(self.max_policy_slot());
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        self.axt_active = true;
        Ok(0)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let ds_tlv = vm.memory.validate_tlv(vm.register(10))?;
        if ds_tlv.type_id != PointerType::DataSpaceId {
            return Err(VMError::NoritoInvalid);
        }
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
            let manifest_tlv = vm.memory.validate_tlv(manifest_ptr)?;
            if manifest_tlv.type_id != PointerType::NoritoBytes {
                return Err(VMError::NoritoInvalid);
            }
            norito::decode_from_bytes(manifest_tlv.payload).map_err(|_| VMError::NoritoInvalid)?
        };
        self.axt_policy.allow_touch(dsid, &manifest)?;
        state.record_touch(dsid, manifest)?;
        Ok(0)
    }

    fn handle_axt_verify_ds_proof(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let Some(state_view) = self.axt_state.as_ref() else {
            return Err(VMError::PermissionDenied);
        };
        let ds_tlv = vm.memory.validate_tlv(vm.register(10))?;
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
            return Ok(0);
        }

        let proof_tlv = vm.memory.validate_tlv(proof_ptr)?;
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
            if entry.valid {
                let state = self.axt_state.as_mut().expect("axt_state checked above");
                state.record_proof(dsid, Some(proof.clone()), None)?;
                return Ok(0);
            }
            return Err(VMError::PermissionDenied);
        }

        match norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof.payload) {
            Ok(envelope) => {
                if envelope.dsid != dsid || envelope.manifest_root != policy.manifest_root {
                    self.cache_proof_entry(
                        dsid,
                        digest,
                        expiry_with_skew,
                        Some(policy.current_slot),
                        Some(envelope.manifest_root),
                        false,
                    );
                    return Err(VMError::PermissionDenied);
                }
                if envelope.proof.is_empty() && proof.payload.is_empty() {
                    self.cache_proof_entry(
                        dsid,
                        digest,
                        expiry_with_skew,
                        Some(policy.current_slot),
                        Some(envelope.manifest_root),
                        false,
                    );
                    return Err(VMError::NoritoInvalid);
                }
            }
            Err(_) => {
                if proof.payload.as_slice() != policy.manifest_root {
                    self.cache_proof_entry(
                        dsid,
                        digest,
                        expiry_with_skew,
                        Some(policy.current_slot),
                        Some(policy.manifest_root),
                        false,
                    );
                    return Err(VMError::PermissionDenied);
                }
            }
        }

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

        let state = self.axt_state.as_mut().expect("axt_state checked above");
        state.record_proof(dsid, Some(proof.clone()), None)?;
        self.cache_proof_entry(
            dsid,
            digest,
            expiry_with_skew,
            Some(policy.current_slot),
            Some(policy.manifest_root),
            true,
        );
        Ok(0)
    }

    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        if self.axt_state.is_none() {
            return Err(VMError::PermissionDenied);
        }
        let handle_tlv = vm.memory.validate_tlv(vm.register(10))?;
        if handle_tlv.type_id != PointerType::AssetHandle {
            return Err(VMError::NoritoInvalid);
        }
        let handle: axt::AssetHandle =
            norito::decode_from_bytes(handle_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;

        let intent_tlv = vm.memory.validate_tlv(vm.register(11))?;
        if intent_tlv.type_id != PointerType::NoritoBytes {
            return Err(VMError::NoritoInvalid);
        }
        let intent: axt::RemoteSpendIntent =
            norito::decode_from_bytes(intent_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;

        let proof: Option<axt::ProofBlob> = match vm.register(12) {
            0 => None,
            ptr => {
                let proof_tlv = vm.memory.validate_tlv(ptr)?;
                if proof_tlv.type_id != PointerType::ProofBlob {
                    return Err(VMError::NoritoInvalid);
                }
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
        if let Some(per_use) = handle.budget.per_use
            && resolved_amount.amount > per_use
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
            match norito::decode_from_bytes::<axt::AxtProofEnvelope>(&proof_blob.payload) {
                Ok(envelope) => {
                    if envelope.dsid != intent.asset_dsid
                        || envelope.manifest_root != policy.manifest_root
                    {
                        return Err(VMError::PermissionDenied);
                    }
                }
                Err(_) => {
                    if proof_blob.payload.as_slice() != policy.manifest_root.as_slice() {
                        return Err(VMError::PermissionDenied);
                    }
                }
            }
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
        Ok(0)
    }

    fn handle_axt_commit(&mut self) -> Result<u64, VMError> {
        self.axt_state.take().ok_or(VMError::PermissionDenied)?;
        self.axt_active = false;
        Ok(0)
    }

    /// Attach a schema registry implementation.
    pub fn with_schema_registry(mut self, reg: Box<dyn SchemaRegistry + Send + Sync>) -> Self {
        self.schema = reg;
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
            return Err(VMError::PermissionDenied);
        }
        self.fastpq_batch_active = true;
        self.fastpq_batch_has_entries = false;
        Ok(0)
    }

    fn push_fastpq_batch_entry(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        Self::expect_tlv(vm, 10, PointerType::AccountId)?;
        Self::expect_tlv(vm, 11, PointerType::AccountId)?;
        Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
        Self::expect_tlv(vm, 13, PointerType::NoritoBytes)?;
        self.fastpq_batch_has_entries = true;
        Ok(0)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        if !self.fastpq_batch_has_entries {
            self.fastpq_batch_active = false;
            return Err(VMError::DecodeError);
        }
        self.fastpq_batch_active = false;
        self.fastpq_batch_has_entries = false;
        Ok(0)
    }

    fn apply_fastpq_batch(&mut self, vm: &IVM) -> Result<u64, VMError> {
        if self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
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
        Ok(0)
    }

    fn build_schema_fallback(&self, schema: &str, payload: &[u8]) -> njson::Value {
        let info = self.schema.info(schema);
        let mut schema_meta = njson::Map::new();
        schema_meta.insert("name".to_owned(), njson::Value::from(schema));
        schema_meta.insert(
            "id".to_owned(),
            info.map(|SchemaInfo { id, .. }| njson::Value::from(hex::encode(id)))
                .unwrap_or(njson::Value::Null),
        );
        schema_meta.insert(
            "version".to_owned(),
            info.map(|SchemaInfo { version, .. }| njson::Value::from(version))
                .unwrap_or(njson::Value::Null),
        );

        let base = Self::schema_base_name(schema);
        let known_versions = self
            .schema
            .list_versions(base)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, info)| Self::schema_info_to_json(name, info))
            .collect();

        let mut root = njson::Map::new();
        root.insert("schema".to_owned(), njson::Value::Object(schema_meta));
        root.insert(
            "payload_base64".to_owned(),
            njson::Value::from(BASE64_STANDARD.encode(payload)),
        );
        root.insert(
            "payload_len".to_owned(),
            njson::Value::from(payload.len() as u64),
        );
        root.insert(
            "known_versions".to_owned(),
            njson::Value::Array(known_versions),
        );
        njson::Value::Object(root)
    }

    fn schema_base_name(name: &str) -> &str {
        let trimmed_digits = name.trim_end_matches(|c: char| c.is_ascii_digit());
        trimmed_digits.trim_end_matches(['v', 'V', '.'])
    }

    fn schema_info_to_json(name: String, info: SchemaInfo) -> njson::Value {
        let mut map = njson::Map::new();
        map.insert("name".to_owned(), njson::Value::from(name));
        map.insert("id".to_owned(), njson::Value::from(hex::encode(info.id)));
        map.insert("version".to_owned(), njson::Value::from(info.version));
        njson::Value::Object(map)
    }

    fn load_u64(vm: &IVM, addr: usize) -> Option<u64> {
        let slice = vm.memory.load_region(addr as u64, 8).ok()?;
        Some(u64::from_le_bytes(slice.try_into().ok()?))
    }

    fn literal_table_info(vm: &IVM) -> Option<(usize, usize, usize, usize, usize)> {
        let limit = vm.pc() as usize;
        let prefix = vm.memory.load_region(0, limit as u64).ok()?;
        let mut literal_start = 0usize;
        if vm.metadata().version_minor == 1
            && limit >= 8
            && prefix[0..4] == CONTRACT_INTERFACE_SECTION_MAGIC
        {
            let contract_payload_len = u32::from_le_bytes(prefix[4..8].try_into().ok()?) as usize;
            let contract_section_len = 8usize.checked_add(contract_payload_len)?;
            literal_start = contract_section_len;
        }
        if literal_start + 4 > limit
            || prefix[literal_start..literal_start + 4] != LITERAL_SECTION_MAGIC
        {
            if crate::dev_env::decode_trace_enabled() {
                eprintln!("[CoreHost] literal table not found (limit=0x{limit:08x})");
            }
            return None;
        }
        if literal_start + 16 > limit {
            return None;
        }
        let literal_count = u32::from_le_bytes(
            prefix[literal_start + 4..literal_start + 8]
                .try_into()
                .ok()?,
        ) as usize;
        let post_pad = u32::from_le_bytes(
            prefix[literal_start + 8..literal_start + 12]
                .try_into()
                .ok()?,
        ) as usize;
        let data_len = u32::from_le_bytes(
            prefix[literal_start + 12..literal_start + 16]
                .try_into()
                .ok()?,
        ) as usize;
        let offsets_start = literal_start + 16;
        let lits_bytes = literal_count.checked_mul(8)?;
        let data_start = offsets_start.checked_add(lits_bytes)?;
        let data_end = data_start.checked_add(data_len)?.checked_add(post_pad)?;
        if data_end > limit {
            return None;
        }
        Some((
            literal_start,
            offsets_start,
            data_start,
            data_end,
            literal_count,
        ))
    }

    fn resolve_literal_pointer(vm: &IVM, src: usize) -> Option<usize> {
        let (start, offsets_start, data_start, data_end, count) = Self::literal_table_info(vm)?;
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[CoreHost] literal table start=0x{start:08x} offsets=0x{offsets_start:08x} data=0x{data_start:08x}..0x{data_end:08x} count={count} src=0x{src:08x}"
            );
        }
        if src >= data_start && src < data_end {
            return Some(src);
        }
        if count == 0 {
            return None;
        }
        if src >= offsets_start && src < data_start {
            let idx = (src - offsets_start) / 8;
            if idx >= count {
                return None;
            }
            let offset = Self::load_u64(vm, offsets_start + idx * 8)? as usize;
            let target = start.checked_add(offset)?;
            if target >= data_start && target < data_end {
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] literal pointer via table idx={idx} -> 0x{target:08x}");
                }
                return Some(target);
            }
            return None;
        }
        if src < offsets_start {
            // Interpret as relative offset or header pointer; fall back to first entry.
            let rel = start.checked_add(src)?;
            if rel >= data_start && rel < data_end {
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] literal pointer relative -> 0x{rel:08x}");
                }
                return Some(rel);
            }
            let offset = Self::load_u64(vm, offsets_start)? as usize;
            let target = start.checked_add(offset)?;
            if target >= data_start && target < data_end {
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] literal pointer fallback first -> 0x{target:08x}");
                }
                return Some(target);
            }
        }
        None
    }

    fn expect_tlv(vm: &IVM, reg: usize, ty: PointerType) -> Result<(), VMError> {
        let addr = Self::resolve_code_tlv_addr(vm, vm.register(reg));
        // First, check type id directly from the TLV header to catch mismatches deterministically
        let hdr = vm.memory.load_region(addr, 7)?;
        let raw_type = u16::from_be_bytes([hdr[0], hdr[1]]);
        if raw_type != ty as u16 {
            if crate::dev_env::decode_trace_enabled() {
                eprintln!(
                    "[CoreHost] type mismatch at r{reg}: expect=0x{:04x} got=0x{:04x}",
                    ty as u16, raw_type
                );
            }
            return Err(VMError::NoritoInvalid);
        }
        // Then validate full TLV envelope and hash
        let tlv = vm.memory.validate_tlv(addr)?;
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
        // Enforce ABI-based pointer-ABI schema policy
        let policy = vm.syscall_policy();
        if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id as u16,
            });
        }
        Ok(())
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

    fn decode_tlv<'a>(
        &self,
        vm: &'a IVM,
        addr: u64,
        expected: PointerType,
    ) -> Result<pointer_abi::Tlv<'a>, VMError> {
        let input_lo = Memory::INPUT_START;
        let input_hi = Memory::INPUT_START + Memory::INPUT_SIZE;
        let tlv = if addr >= input_lo && addr + 7 <= input_hi {
            let tlv = vm.memory.validate_tlv(addr)?;
            if tlv.type_id != expected {
                return Err(VMError::NoritoInvalid);
            }
            tlv
        } else {
            self.decode_tlv_from_code(vm, addr, expected)?
        };
        let policy = vm.syscall_policy();
        if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
            return Err(VMError::AbiTypeNotAllowed {
                abi: vm.abi_version(),
                type_id: tlv.type_id as u16,
            });
        }
        Ok(tlv)
    }

    fn decode_tlv_from_code<'a>(
        &self,
        vm: &'a IVM,
        addr: u64,
        expected: PointerType,
    ) -> Result<pointer_abi::Tlv<'a>, VMError> {
        let resolved_addr = Self::resolve_code_tlv_addr(vm, addr);
        let code_len = vm.memory.code_len();
        if resolved_addr >= code_len || resolved_addr + 7 > code_len {
            return Err(VMError::NoritoInvalid);
        }
        let mut hdr = [0u8; 7];
        vm.memory
            .load_bytes(resolved_addr, &mut hdr)
            .map_err(|_| VMError::NoritoInvalid)?;
        let type_id = u16::from_be_bytes([hdr[0], hdr[1]]);
        if type_id != expected as u16 {
            return Err(VMError::NoritoInvalid);
        }
        if hdr[2] != 1 {
            return Err(VMError::NoritoInvalid);
        }
        let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
        let total = 7usize
            .checked_add(len)
            .and_then(|x| x.checked_add(IrohaHash::LENGTH))
            .ok_or(VMError::NoritoInvalid)?;
        let code_len = code_len as usize;
        if resolved_addr as usize + total > code_len {
            return Err(VMError::NoritoInvalid);
        }
        let envelope = vm
            .memory
            .load_region(resolved_addr, total as u64)
            .map_err(|_| VMError::NoritoInvalid)?;
        pointer_abi::validate_tlv_bytes(envelope)
    }

    fn alloc_norito_bytes_tlv(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        let mut out = Vec::with_capacity(7 + payload.len() + IrohaHash::LENGTH);
        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
        out.push(1);
        let len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(payload);
        let h: [u8; 32] = IrohaHash::new(payload).into();
        out.extend_from_slice(&h);
        vm.alloc_input_tlv(&out)
    }

    fn ensure_unsigned_scale0(numeric: Numeric) -> Result<Numeric, VMError> {
        if numeric.scale() != 0 || numeric.mantissa().is_negative() {
            return Err(VMError::AssertionFailed);
        }
        Ok(numeric)
    }

    fn decode_numeric(&self, vm: &IVM, ptr: u64) -> Result<Numeric, VMError> {
        let tlv = self.decode_tlv(vm, ptr, PointerType::NoritoBytes)?;
        let numeric =
            decode_from_bytes::<Numeric>(tlv.payload).map_err(|_| VMError::DecodeError)?;
        Self::ensure_unsigned_scale0(numeric)
    }
}

// Provide a Default impl to satisfy clippy::new_without_default without changing API.
impl Default for CoreHost {
    fn default() -> Self {
        Self::new()
    }
}

impl IVMHost for CoreHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if crate::dev_env::decode_trace_enabled() {
            eprintln!("[CoreHost] syscall number=0x{number:02x}");
        }
        // Enforce ABI policy gating to ensure uniform behaviour across hosts.
        if !crate::syscalls::is_syscall_allowed(vm.syscall_policy(), number) {
            return Err(VMError::UnknownSyscall(number));
        }
        match number {
            // Durable state: pointer-ABI paths (Name) and NoritoBytes values
            syscalls::SYSCALL_STATE_GET => {
                // r10 = &Name path; return r10 = &NoritoBytes value in INPUT (or 0 if absent)
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Name)?;
                let path = self.decode_name_payload(tlv.payload)?;
                self.log_read_key(path.as_ref());
                if let Some(val) = self.state.get(path.as_ref()) {
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
                    let p = vm.alloc_input_tlv(&buf)?;
                    vm.set_register(10, p);
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] STATE_GET returned r10=0x{p:08x}");
                    }
                } else {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] STATE_GET path='{path}' miss");
                    }
                    vm.set_register(10, 0);
                }
                Ok(0)
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
                let p_path = self.decode_tlv(vm, path_ptr, PointerType::Name)?;
                let p_val = self.decode_tlv(vm, val_ptr, PointerType::NoritoBytes)?;
                let path = self.decode_name_payload(p_path.payload)?;
                self.log_write_key(path.as_ref());
                self.state.set(path.as_ref(), p_val.payload.to_vec())?;
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[CoreHost] STATE_SET path='{path}' bytes={}",
                        p_val.payload.len()
                    );
                }
                Ok(0)
            }
            syscalls::SYSCALL_STATE_DEL => {
                // r10 = &Name path
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let p_path = self.decode_tlv(vm, ptr, PointerType::Name)?;
                let path = self.decode_name_payload(p_path.payload)?;
                self.log_write_key(path.as_ref());
                self.state.del(path.as_ref())?;
                Ok(0)
            }
            syscalls::SYSCALL_ALLOC => {
                // r10 = number of bytes to allocate on the VM heap.
                let size = vm.register(10);
                let addr = vm.alloc_heap(size)?;
                vm.set_register(10, addr);
                Ok(0)
            }
            syscalls::SYSCALL_DECODE_INT => {
                // r10 = &NoritoBytes (Norito-framed i64) -> r10 = parsed i64
                let addr = vm.register(10);
                if addr == 0 {
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!("[CoreHost] DECODE_INT addr=0 (treat as zero)");
                    }
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
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
                let val: i64 = decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                vm.set_register(10, val as u64);
                Ok(0)
            }
            syscalls::SYSCALL_BUILD_PATH_MAP_KEY => {
                // r10 = &Name base; r11 = key(int). Return new &Name in INPUT: "<base>/<key>"
                let r10_before = vm.register(10);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] BUILD_PATH_MAP_KEY enter r10=0x{r10_before:08x}");
                }
                let tlv = vm.memory.validate_tlv(r10_before)?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let base_name = self.decode_name_payload(tlv.payload)?;
                let base = base_name.as_ref();
                let key = vm.register(11) as i64;
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] BUILD_PATH_MAP_KEY base='{base}' key={key}");
                }
                let mut s = String::with_capacity(base.len() + 1 + 20);
                s.push_str(base);
                s.push('/');
                use core::fmt::Write as _;
                let _ = write!(&mut s, "{key}");
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] BUILD_PATH_MAP_KEY out='{s}'");
                }
                let path_name = Name::from_str(&s).map_err(|_| VMError::NoritoInvalid)?;
                // Build Name TLV and allocate in INPUT
                let body = to_bytes(&path_name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                if crate::dev_env::decode_trace_enabled() {
                    let tlv2 = vm.memory.validate_tlv(p)?;
                    if let Ok(name) = decode_from_bytes::<Name>(tlv2.payload) {
                        eprintln!(
                            "[CoreHost] BUILD_PATH_MAP_KEY exit r10=0x{p:08x} -> {}",
                            name.as_ref()
                        );
                    }
                }
                Ok(0)
            }
            syscalls::SYSCALL_ENCODE_INT => {
                // r10 = value (i64) -> r10 = &NoritoBytes (Norito-framed i64)
                let val = vm.register(10) as i64;
                let body = to_bytes(&val).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_FROM_INT => {
                let val = vm.register(10) as i64;
                if val < 0 {
                    return Err(VMError::AssertionFailed);
                }
                let payload =
                    to_bytes(&Numeric::new(val, 0)).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_TO_INT => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let numeric = self.decode_numeric(vm, ptr)?;
                let value = numeric
                    .try_mantissa_i128()
                    .ok_or(VMError::AssertionFailed)?;
                if value > i64::MAX as i128 {
                    return Err(VMError::AssertionFailed);
                }
                vm.set_register(10, (value as i64) as u64);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_ADD => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let out = lhs.checked_add(rhs).ok_or(VMError::AssertionFailed)?;
                let payload = to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_SUB => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let out = lhs.checked_sub(rhs).ok_or(VMError::AssertionFailed)?;
                if out.mantissa().is_negative() {
                    return Err(VMError::AssertionFailed);
                }
                let payload = to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_MUL => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let out = lhs
                    .checked_mul(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_DIV => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let out = lhs
                    .checked_div(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_REM => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let out = lhs
                    .checked_rem(rhs, NumericSpec::unconstrained())
                    .ok_or(VMError::AssertionFailed)?;
                let payload = to_bytes(&out).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_NEG => {
                let val = self.decode_numeric(vm, vm.register(10))?;
                if !val.is_zero() {
                    return Err(VMError::AssertionFailed);
                }
                let payload = to_bytes(&val).map_err(|_| VMError::NoritoInvalid)?;
                let p = Self::alloc_norito_bytes_tlv(vm, &payload)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NUMERIC_EQ
            | syscalls::SYSCALL_NUMERIC_NE
            | syscalls::SYSCALL_NUMERIC_LT
            | syscalls::SYSCALL_NUMERIC_LE
            | syscalls::SYSCALL_NUMERIC_GT
            | syscalls::SYSCALL_NUMERIC_GE => {
                let lhs = self.decode_numeric(vm, vm.register(10))?;
                let rhs = self.decode_numeric(vm, vm.register(11))?;
                let cmp = lhs.cmp(&rhs);
                let result = match number {
                    syscalls::SYSCALL_NUMERIC_EQ => cmp == core::cmp::Ordering::Equal,
                    syscalls::SYSCALL_NUMERIC_NE => cmp != core::cmp::Ordering::Equal,
                    syscalls::SYSCALL_NUMERIC_LT => cmp == core::cmp::Ordering::Less,
                    syscalls::SYSCALL_NUMERIC_LE => {
                        cmp == core::cmp::Ordering::Less || cmp == core::cmp::Ordering::Equal
                    }
                    syscalls::SYSCALL_NUMERIC_GT => cmp == core::cmp::Ordering::Greater,
                    syscalls::SYSCALL_NUMERIC_GE => {
                        cmp == core::cmp::Ordering::Greater || cmp == core::cmp::Ordering::Equal
                    }
                    _ => false,
                };
                vm.set_register(10, if result { 1 } else { 0 });
                Ok(0)
            }
            syscalls::SYSCALL_BUILD_PATH_KEY_NORITO => {
                // r10 = &Name base; r11 = &NoritoBytes key -> r10 = &Name("<base>/<hex(hash))>")
                let base_tlv = vm.memory.validate_tlv(vm.register(10))?;
                if base_tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let key_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if key_tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let base_name = self.decode_name_payload(base_tlv.payload)?;
                let base = base_name.as_ref();
                let h: [u8; 32] = IrohaHash::new(key_tlv.payload).into();
                let mut s = String::with_capacity(base.len() + 1 + 64);
                use core::fmt::Write as _;
                let _ = write!(&mut s, "{base}/");
                for b in &h {
                    let _ = write!(&mut s, "{b:02x}");
                }
                let path_name = Name::from_str(&s).map_err(|_| VMError::NoritoInvalid)?;
                let body = to_bytes(&path_name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let hh: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&hh);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_JSON_ENCODE => {
                // r10 = &Json (Norito-framed) -> r10 = &NoritoBytes (same payload)
                let r10_before = vm.register(10);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_ENCODE enter r10=0x{r10_before:08x}");
                }
                let tlv = vm.memory.validate_tlv(r10_before)?;
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
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_ENCODE exit r10=0x{p:08x}");
                }
                Ok(0)
            }
            syscalls::SYSCALL_JSON_DECODE => {
                // r10 = &NoritoBytes (Norito-framed Json) -> r10 = &Json
                let r10_before = vm.register(10);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_DECODE enter r10=0x{r10_before:08x}");
                }
                if r10_before == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(r10_before)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                if !matches!(tlv.type_id, PointerType::NoritoBytes | PointerType::Json) {
                    return Err(VMError::NoritoInvalid);
                }
                let json: Json =
                    decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
                let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!("[CoreHost] JSON_DECODE exit r10=0x{p:08x}");
                }
                Ok(0)
            }
            syscalls::SYSCALL_TLV_LEN => {
                // r10 = &TLV -> r10 = payload length
                let addr = vm.register(10);
                if addr == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(addr)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                vm.set_register(10, tlv.payload.len() as u64);
                Ok(0)
            }
            syscalls::SYSCALL_JSON_GET_I64
            | syscalls::SYSCALL_JSON_GET_JSON
            | syscalls::SYSCALL_JSON_GET_NAME
            | syscalls::SYSCALL_JSON_GET_ACCOUNT_ID
            | syscalls::SYSCALL_JSON_GET_NFT_ID
            | syscalls::SYSCALL_JSON_GET_BLOB_HEX
            | syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID
            | syscalls::SYSCALL_JSON_GET_NUMERIC => {
                // r10=&Json, r11=&Name key -> r10=value
                let json_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let key_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if json_tlv.type_id != PointerType::Json || key_tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, json_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: json_tlv.type_id as u16,
                    });
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, key_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: key_tlv.type_id as u16,
                    });
                }

                let json: Json =
                    decode_from_bytes(json_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let value: njson::Value = json
                    .try_into_any_norito()
                    .map_err(|_| VMError::DecodeError)?;
                let obj = value.as_object().ok_or(VMError::DecodeError)?;
                let key_name: Name =
                    decode_from_bytes(key_tlv.payload).map_err(|_| VMError::DecodeError)?;
                let field = obj.get(key_name.as_ref()).ok_or(VMError::DecodeError)?;

                match number {
                    syscalls::SYSCALL_JSON_GET_I64 => {
                        let n = match field {
                            njson::Value::Number(njson::native::Number::I64(v)) => *v,
                            njson::Value::Number(njson::native::Number::U64(v)) => {
                                i64::try_from(*v).map_err(|_| VMError::DecodeError)?
                            }
                            _ => return Err(VMError::DecodeError),
                        };
                        vm.set_register(10, n as u64);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_JSON => {
                        let out_json =
                            Json::from_norito_value_ref(field).map_err(|_| VMError::DecodeError)?;
                        let body = to_bytes(&out_json).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_NAME => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let nm = Name::from_str(raw).map_err(|_| VMError::DecodeError)?;
                        let body = to_bytes(&nm).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_ACCOUNT_ID => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let acct = iroha_data_model::account::AccountId::parse_encoded(raw)
                            .map_err(|_| VMError::DecodeError)?
                            .into_account_id();
                        let body = to_bytes(&acct).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_NFT_ID => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let nft = NftId::from_str(raw).map_err(|_| VMError::DecodeError)?;
                        let body = to_bytes(&nft).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::NftId as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_BLOB_HEX => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let raw = raw.strip_prefix("0x").unwrap_or(raw);
                        if raw.len() % 2 != 0 {
                            return Err(VMError::DecodeError);
                        }
                        let bytes = hex::decode(raw).map_err(|_| VMError::DecodeError)?;
                        let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                        out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                        out.extend_from_slice(&bytes);
                        let h: [u8; 32] = IrohaHash::new(&bytes).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID => {
                        let raw = field.as_str().ok_or(VMError::DecodeError)?;
                        let asset = AssetDefinitionId::parse_address_literal(raw)
                            .map_err(|_| VMError::DecodeError)?;
                        let body = to_bytes(&asset).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(
                            &(PointerType::AssetDefinitionId as u16).to_be_bytes(),
                        );
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    syscalls::SYSCALL_JSON_GET_NUMERIC => {
                        let numeric = parse_json_numeric_field(field)?;
                        let body = to_bytes(&numeric).map_err(|_| VMError::NoritoInvalid)?;
                        let mut out = Vec::with_capacity(7 + body.len() + 32);
                        out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                        out.push(1);
                        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&body);
                        let h: [u8; 32] = IrohaHash::new(&body).into();
                        out.extend_from_slice(&h);
                        let p = vm.alloc_input_tlv(&out)?;
                        vm.set_register(10, p);
                        Ok(0)
                    }
                    _ => Err(VMError::UnknownSyscall(number)),
                }
            }

            syscalls::SYSCALL_SCHEMA_ENCODE => {
                // r10 = &Name schema; r11 = &Json -> r10 = &NoritoBytes (schema-typed)
                let s_tlv = vm.memory.validate_tlv(vm.register(10))?;
                let v_tlv = vm.memory.validate_tlv(vm.register(11))?;
                if s_tlv.type_id != PointerType::Name || v_tlv.type_id != PointerType::Json {
                    return Err(VMError::NoritoInvalid);
                }
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                let json: Json =
                    decode_from_bytes(v_tlv.payload).map_err(|_| VMError::DecodeError)?;
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
                    let mut out = Vec::with_capacity(7 + bytes.len() + 32);
                    out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                    out.extend_from_slice(&bytes);
                    let h: [u8; 32] = IrohaHash::new(&bytes).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    if crate::dev_env::decode_trace_enabled() {
                        eprintln!(
                            "[CoreHost] SCHEMA_ENCODE exit r10=0x{p:08x} bytes={len}",
                            len = bytes.len()
                        );
                    }
                    Ok(0)
                } else {
                    Err(VMError::NoritoInvalid)
                }
            }
            syscalls::SYSCALL_SCHEMA_DECODE => {
                // r10 = &Name schema; r11 = &NoritoBytes -> r10 = &Json (Norito-framed)
                let s_ptr = vm.register(10);
                let b_ptr = vm.register(11);
                let s_tlv = vm.memory.validate_tlv(s_ptr)?;
                let b_tlv = vm.memory.validate_tlv(b_ptr)?;
                if s_tlv.type_id != PointerType::Name || b_tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, s_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: s_tlv.type_id as u16,
                    });
                }
                if !pointer_abi::is_type_allowed_for_policy(policy, b_tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: b_tlv.type_id as u16,
                    });
                }
                let schema = self.decode_name_payload(s_tlv.payload)?.to_string();
                if crate::dev_env::decode_trace_enabled() {
                    eprintln!(
                        "[CoreHost] SCHEMA_DECODE enter schema={} b_len={len}",
                        schema,
                        len = b_tlv.payload.len()
                    );
                }
                if let Some(min) = self.schema.decode_to_json(&schema, b_tlv.payload) {
                    let json_str =
                        core::str::from_utf8(&min).map_err(|_| VMError::NoritoInvalid)?;
                    let json =
                        Json::from_str_norito(json_str).map_err(|_| VMError::NoritoInvalid)?;
                    let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = IrohaHash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(0)
                } else {
                    let fallback = self.build_schema_fallback(&schema, b_tlv.payload);
                    let json = Json::from(&fallback);
                    let body = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
                    let mut out = Vec::with_capacity(7 + body.len() + 32);
                    out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                    out.push(1);
                    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                    out.extend_from_slice(&body);
                    let h: [u8; 32] = IrohaHash::new(&body).into();
                    out.extend_from_slice(&h);
                    let p = vm.alloc_input_tlv(&out)?;
                    vm.set_register(10, p);
                    Ok(0)
                }
            }
            syscalls::SYSCALL_SCHEMA_INFO => {
                // r10 = &Name (base or exact) -> r10 = &Json {current: {name,id,version}, versions:[{name,id,version}...]}
                let tlv = vm.memory.validate_tlv(vm.register(10))?;
                if tlv.type_id != PointerType::Name {
                    return Err(VMError::NoritoInvalid);
                }
                let name = self.decode_name_payload(tlv.payload)?;
                let raw = name.as_ref();
                let base = raw
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .trim_end_matches(['v', 'V']);
                let (cur_name, cur_info) =
                    self.schema.current(base).ok_or(VMError::NoritoInvalid)?;
                let list = self
                    .schema
                    .list_versions(base)
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
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Json as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_NAME_DECODE => {
                // r10 = &NoritoBytes (prefer Norito Name; legacy UTF-8 string is also accepted)
                // -> r10 = &Name
                let r10_before = vm.register(10);
                if r10_before == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(r10_before)?;
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
                let name: Name = if let Ok(name) = decode_from_bytes(tlv.payload) {
                    name
                } else if let Ok(raw) = core::str::from_utf8(tlv.payload) {
                    Name::from_str(raw).map_err(|_| VMError::NoritoInvalid)?
                } else {
                    let raw: String =
                        decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
                    Name::from_str(&raw).map_err(|_| VMError::NoritoInvalid)?
                };
                // Build Name TLV and mirror into INPUT using the normalized form.
                let body = to_bytes(&name).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::Name as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
                out.extend_from_slice(&body);
                let h: [u8; 32] = IrohaHash::new(&body).into();
                out.extend_from_slice(&h);
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_POINTER_TO_NORITO => {
                let original = vm.register(10);
                if original == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let ptr = Self::resolve_code_tlv_addr(vm, original);
                let tlv = vm.memory.validate_tlv(ptr)?;
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
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_POINTER_FROM_NORITO => {
                if vm.register(10) == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let ptr = vm.register(10);
                let tlv = self.decode_tlv(vm, ptr, PointerType::NoritoBytes)?;
                let policy = vm.syscall_policy();
                let (inner_type, inner_version, inner_payload) = {
                    let inner = pointer_abi::validate_tlv_bytes(tlv.payload)?;
                    (inner.type_id, inner.version, inner.payload.to_vec())
                };
                let expected = vm.register(11) as u16;
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
                let p = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, p);
                Ok(0)
            }
            syscalls::SYSCALL_TLV_EQ => {
                let ptr1 = vm.register(10);
                let ptr2 = vm.register(11);
                if ptr1 == ptr2 {
                    vm.set_register(10, 1);
                    return Ok(0);
                }
                if ptr1 == 0 || ptr2 == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let tlv1 = match vm.memory.validate_tlv(ptr1) {
                    Ok(t) => t,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };
                let tlv2 = match vm.memory.validate_tlv(ptr2) {
                    Ok(t) => t,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };
                let eq = tlv1.type_id == tlv2.type_id
                    && tlv1.version == tlv2.version
                    && tlv1.payload == tlv2.payload;
                vm.set_register(10, if eq { 1 } else { 0 });
                Ok(0)
            }
            syscalls::SYSCALL_DEBUG_PRINT => {
                let value = vm.register(10);
                if cfg!(any(test, debug_assertions)) {
                    eprintln!("[IVM] debug_print r10={value}");
                }
                Ok(0)
            }
            syscalls::SYSCALL_EXIT => {
                let status = vm.register(10);
                vm.request_exit();
                vm.set_register(10, status);
                Ok(0)
            }
            syscalls::SYSCALL_ABORT => {
                vm.set_register(10, 0);
                vm.request_abort();
                Ok(0)
            }
            syscalls::SYSCALL_DEBUG_LOG => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Ok(0);
                }
                let tlv = vm
                    .memory
                    .validate_tlv(Self::resolve_code_tlv_addr(vm, ptr))?;
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
                        Ok(0)
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
                let digest = Sm3Digest::hash(tlv.payload);
                let bytes = digest.as_bytes();
                let mut out = Vec::with_capacity(7 + bytes.len() + IrohaHash::LENGTH);
                out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                out.extend_from_slice(bytes);
                let hash: [u8; IrohaHash::LENGTH] = IrohaHash::new(bytes).into();
                out.extend_from_slice(&hash);
                let addr = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, addr);
                Ok(0)
            }
            syscalls::SYSCALL_SHA256_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let digest = <Sha256 as Sha2Digest>::digest(tlv.payload);
                let mut out = Vec::with_capacity(7 + digest.len() + IrohaHash::LENGTH);
                out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(digest.len() as u32).to_be_bytes());
                out.extend_from_slice(digest.as_slice());
                let hash: [u8; IrohaHash::LENGTH] = IrohaHash::new(digest.as_slice()).into();
                out.extend_from_slice(&hash);
                let addr = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, addr);
                Ok(0)
            }
            syscalls::SYSCALL_SHA3_HASH => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Err(VMError::NoritoInvalid);
                }
                let tlv = self.decode_tlv(vm, ptr, PointerType::Blob)?;
                let digest = <Sha3_256 as Sha3Digest>::digest(tlv.payload);
                let mut out = Vec::with_capacity(7 + digest.len() + IrohaHash::LENGTH);
                out.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(digest.len() as u32).to_be_bytes());
                out.extend_from_slice(digest.as_slice());
                let hash: [u8; IrohaHash::LENGTH] = IrohaHash::new(digest.as_slice()).into();
                out.extend_from_slice(&hash);
                let addr = vm.alloc_input_tlv(&out)?;
                vm.set_register(10, addr);
                Ok(0)
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
                Ok(0)
            }
            syscalls::SYSCALL_TRANSFER_DOMAIN => {
                // r10 = &DomainId, r11 = &AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::DomainId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(0)
            }
            syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                // r10=&AccountId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                Self::expect_tlv(vm, 12, PointerType::Json)?;
                Ok(0)
            }
            syscalls::SYSCALL_NFT_MINT_ASSET => {
                // r10=&NftId, r11=&AccountId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(0)
            }
            syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                // r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::NftId)?;
                Self::expect_tlv(vm, 12, PointerType::AccountId)?;
                Ok(0)
            }
            syscalls::SYSCALL_TRANSFER_ASSET => {
                if self.fastpq_batch_active {
                    self.push_fastpq_batch_entry(vm)
                } else {
                    // r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId,
                    // r13=&NoritoBytes(Numeric)
                    Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                    Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                    Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
                    Self::expect_tlv(vm, 13, PointerType::NoritoBytes)?;
                    Ok(0)
                }
            }
            syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch(vm),
            syscalls::SYSCALL_NFT_SET_METADATA => {
                // r10=&NftId, r11=&Json
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::Json)?;
                Ok(0)
            }
            syscalls::SYSCALL_NFT_BURN_ASSET => {
                // r10=&NftId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Ok(0)
            }
            syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
                // Mirror TLV into INPUT (no-op if already INPUT); validate envelope/policy.
                let original = vm.register(10);
                if original == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let input_lo = Memory::INPUT_START;
                let input_hi = Memory::INPUT_START + Memory::INPUT_SIZE;
                let mut src = original;
                // Fast-path: if `src` already points to a valid INPUT TLV, keep it.
                if src >= input_lo && src < input_hi {
                    let tlv = vm.memory.validate_tlv(src)?;
                    let policy = vm.syscall_policy();
                    if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                        return Err(VMError::AbiTypeNotAllowed {
                            abi: vm.abi_version(),
                            type_id: tlv.type_id as u16,
                        });
                    }
                    return Ok(0);
                }
                if let Some(resolved) = Self::resolve_literal_pointer(vm, src as usize) {
                    src = resolved as u64;
                }
                let mut header = [0u8; 7];
                let read_ok = vm.memory.load_bytes(src, &mut header).is_ok();
                if !read_ok || header[2] != 1 {
                    return Err(VMError::NoritoInvalid);
                }
                let len = u32::from_be_bytes([header[3], header[4], header[5], header[6]]) as usize;
                let total = 7usize
                    .checked_add(len)
                    .and_then(|v| v.checked_add(32))
                    .ok_or(VMError::NoritoInvalid)?;
                let bytes_vec = vm
                    .memory
                    .load_region(src, total as u64)
                    .map_err(|_| VMError::NoritoInvalid)?
                    .to_vec();
                let tlv = pointer_abi::validate_tlv_bytes(&bytes_vec)?;
                let policy = vm.syscall_policy();
                if !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
                    return Err(VMError::AbiTypeNotAllowed {
                        abi: vm.abi_version(),
                        type_id: tlv.type_id as u16,
                    });
                }
                let dst = vm.alloc_input_tlv(&bytes_vec)?;
                vm.set_register(10, dst);
                Ok(0)
            }
            syscalls::SYSCALL_GET_AUTHORITY => {
                // Return the domainless account subject so contracts can compare
                // authority() against AccountId literals and stored AccountId state.
                const ACCOUNT: &str =
                    "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
                let authority = AccountId::parse_encoded(ACCOUNT)
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .map_err(|_| VMError::NoritoInvalid)?;
                let payload = to_bytes(&authority).map_err(|_| VMError::NoritoInvalid)?;
                let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
                tlv.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
                tlv.push(1);
                tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                tlv.extend_from_slice(&payload);
                let h: [u8; 32] = IrohaHash::new(payload).into();
                tlv.extend_from_slice(&h);
                let ptr = vm.alloc_input_tlv(&tlv)?;
                vm.set_register(10, ptr);
                Ok(0)
            }
            syscalls::SYSCALL_CURRENT_TIME_MS => {
                vm.set_register(10, self.current_time_ms);
                Ok(0)
            }
            syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),
            syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(),
            _ => Err(VMError::UnknownSyscall(number)),
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

    fn assemble_program(words: &[u32]) -> Vec<u8> {
        let mut code = Vec::with_capacity(words.len() * 4);
        for word in words {
            code.extend_from_slice(&word.to_le_bytes());
        }
        let mut program = Vec::with_capacity(16 + code.len());
        program.extend_from_slice(b"IVM\0");
        program.push(1); // version major
        program.push(0); // version minor
        program.push(0); // mode flags
        program.push(0); // vector length (unused when mode == 0)
        program.extend_from_slice(&0u64.to_le_bytes()); // max_cycles
        program.push(1); // abi_version
        program.extend_from_slice(&code);
        program
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
        let tlv = vm.memory.validate_tlv(out_ptr).expect("out tlv");
        assert_eq!(tlv.type_id, PointerType::Name);
        assert_eq!(tlv.payload, b"wonderland");
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
    fn fastpq_batch_requires_entries() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(1_000);
        host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm)
            .expect("begin batch");
        let err = host
            .syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm)
            .expect_err("ending empty batch should fail");
        assert!(matches!(err, VMError::DecodeError));
    }

    #[test]
    fn fastpq_batch_validates_transfer_entries() {
        let mut host = CoreHost::new();
        let mut vm = IVM::new(1_000);
        let from_account = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
        let from = make_pointer_tlv(PointerType::AccountId, from_account.as_bytes());
        vm.memory.preload_input(0, &from).expect("preload from");
        let to_account = "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76";
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
        let amount = make_numeric_tlv(Numeric::from(10_u64));
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

        host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm)
            .expect("begin batch");
        host.syscall(syscalls::SYSCALL_TRANSFER_ASSET, &mut vm)
            .expect("push entry");
        host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm)
            .expect("finish batch");
    }
}
