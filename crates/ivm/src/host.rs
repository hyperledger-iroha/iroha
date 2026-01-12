//! Host interface trait for handling syscalls, with default dummy implementations.
//!
//! The default host provides a minimal but functional set of syscalls used by
//! the tests. It supports heap allocation and retrieval of private inputs so
//! that zero‑knowledge programs can run without a custom environment.
//!
//! The host also exposes basic hardware feature discovery and proof generation
//! helpers used by some tests.
use std::{any::Any, collections::HashSet};

use iroha_crypto::{Sm2PublicKey, Sm2Signature, Sm3Digest, Sm4Key};
use iroha_data_model::{
    isi::transfer::TransferAssetBatch,
    nexus::{AxtPolicySnapshot, DataSpaceId},
};
use iroha_primitives::json::Json;
use norito::{NoritoSerialize, decode_from_bytes};

use crate::{
    axt::{self, AssetHandle, ProofBlob, RemoteSpendIntent, TouchManifest},
    error::VMError,
    ivm::IVM,
    parallel::{StateAccessSet, StateKey, StateUpdate},
    pointer_abi::{self, PointerType},
    syscalls, zk,
};

/// Runtime record of logical state touches performed by a host during a transaction.
#[derive(Clone, Default, Debug)]
pub struct AccessLog {
    pub read_keys: HashSet<StateKey>,
    pub write_keys: HashSet<StateKey>,
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

pub const LABEL_TRANSFER: &str = "zk_verify_transfer/v1";
pub const LABEL_UNSHIELD: &str = "zk_verify_unshield/v1";
pub const LABEL_VOTE_BALLOT: &str = "zk_verify_ballot/v1";
pub const LABEL_VOTE_TALLY: &str = "zk_verify_tally/v1";
pub const LABEL_BATCH: &str = "zk_verify_batch/v1";

/// Trait for IVM host environment to handle syscalls (SCALL).
pub trait IVMHost {
    /// Handle a syscall invoked by the VM. `number` is the syscall ID and the
    /// mutable reference to the VM gives access to registers and memory.
    ///
    /// The handler must return the additional **gas cost** that should be
    /// charged for the call. The VM will deduct this amount from the remaining
    /// gas after the syscall completes. Returning an error aborts execution.
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError>;

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

// Compile-time signature guard to force downstream hosts to adopt the Result-based
// transaction lifecycle surface.
type BeginTxSignatureGuard =
    for<'a, 'b> fn(&'a mut dyn IVMHost, &'b StateAccessSet) -> Result<(), VMError>;
type FinishTxSignatureGuard = for<'a> fn(&'a mut dyn IVMHost) -> Result<AccessLog, VMError>;
fn begin_tx_signature_guard(
    host: &mut dyn IVMHost,
    declared: &StateAccessSet,
) -> Result<(), VMError> {
    IVMHost::begin_tx(host, declared)
}

fn finish_tx_signature_guard(host: &mut dyn IVMHost) -> Result<AccessLog, VMError> {
    IVMHost::finish_tx(host)
}

const _: BeginTxSignatureGuard = begin_tx_signature_guard;
const _: FinishTxSignatureGuard = finish_tx_signature_guard;

/// A basic host implementation used in tests. It supports heap allocation and
/// reading private inputs.
#[derive(Clone)]
pub struct DefaultHost {
    private_inputs: Vec<u64>,
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
    access_log: AccessLog,
}

impl DefaultHost {
    pub fn new() -> Self {
        DefaultHost {
            private_inputs: Vec::new(),
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
            access_log: AccessLog::default(),
        }
    }

    /// Provide private inputs that can later be retrieved via `SYSCALL_GET_PRIVATE_INPUT`.
    pub fn with_private_inputs(inputs: Vec<u64>) -> Self {
        DefaultHost {
            private_inputs: inputs,
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
            access_log: AccessLog::default(),
        }
    }

    /// Configure Halo2 verification limits for this host.
    pub fn with_zk_halo2_config(mut self, cfg: ZkHalo2Config) -> Self {
        self.zk_cfg = cfg;
        self
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

    fn expected_transcript_label(number: u32) -> &'static str {
        match number {
            syscalls::SYSCALL_ZK_VERIFY_TRANSFER => LABEL_TRANSFER,
            syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => LABEL_UNSHIELD,
            syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => LABEL_VOTE_BALLOT,
            syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => LABEL_VOTE_TALLY,
            syscalls::SYSCALL_ZK_VERIFY_BATCH => LABEL_BATCH,
            _ => LABEL_TRANSFER,
        }
    }

    fn validate_transcript_label(&self, number: u32, label: &str) -> Result<(), u64> {
        if label.len() > self.zk_cfg.max_transcript_label_len {
            return Err(ERR_TRANSCRIPT_LABEL);
        }
        if self.zk_cfg.enforce_transcript_label_ascii && !label.is_ascii() {
            return Err(ERR_TRANSCRIPT_LABEL);
        }
        let expected = Self::expected_transcript_label(number);
        if label != expected {
            return Err(ERR_TRANSCRIPT_LABEL);
        }
        Ok(())
    }

    fn proof_len_bytes(proof: &iroha_zkp_halo2::IpaProofData) -> Option<usize> {
        proof
            .encoded_len_exact()
            .or_else(|| norito::to_bytes(proof).ok().map(|b| b.len()))
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
        let _ = vm.register(13);
        self.fastpq_batch_has_entries = true;
        Ok(0)
    }

    fn finish_fastpq_batch(&mut self) -> Result<u64, VMError> {
        if !self.fastpq_batch_active {
            return Err(VMError::PermissionDenied);
        }
        self.fastpq_batch_active = false;
        if !self.fastpq_batch_has_entries {
            return Err(VMError::DecodeError);
        }
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

    /// Retrieve and clear the output committed by the last program run.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pub_output)
    }

    /// Check if a nullifier has been recorded.
    pub fn has_nullifier(&self, n: u64) -> bool {
        self.nullifiers.contains(&n)
    }

    /// Validate a TLV pointer in register `reg` has the expected `PointerType`.
    fn expect_tlv(vm: &IVM, reg: usize, ty: PointerType) -> Result<(), VMError> {
        let addr = vm.register(reg);
        // Fast header check first
        let hdr = vm.memory.load_region(addr, 7)?;
        let raw_type = u16::from_be_bytes([hdr[0], hdr[1]]);
        if raw_type != ty as u16 {
            return Err(VMError::NoritoInvalid);
        }
        // Full TLV validation (bounds + hash) and type confirm
        let tlv = vm.memory.validate_tlv(addr)?;
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
        Ok(())
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
        vm.alloc_input_tlv(&out)
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
        let descriptor: axt::AxtDescriptor =
            norito::decode_from_bytes(tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        axt::validate_descriptor(&descriptor)?;
        let binding = axt::compute_binding(&descriptor).map_err(|_| VMError::NoritoInvalid)?;
        self.axt_state = Some(axt::HostAxtState::new(descriptor, binding));
        Ok(0)
    }

    fn handle_axt_touch(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
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
            norito::decode_from_bytes(manifest_tlv.payload).map_err(|_| VMError::NoritoInvalid)?
        };
        self.axt_policy.allow_touch(dsid, &manifest)?;
        state.record_touch(dsid, manifest)?;
        Ok(0)
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
            state.record_proof(dsid, None, None)?;
            return Ok(0);
        }
        let proof_tlv = vm.memory.validate_tlv(proof_ptr)?;
        if proof_tlv.type_id != PointerType::ProofBlob {
            return Err(VMError::NoritoInvalid);
        }
        let proof: ProofBlob =
            norito::decode_from_bytes(proof_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        state.record_proof(dsid, Some(proof), None)?;
        Ok(0)
    }

    fn handle_axt_use_asset_handle(&mut self, vm: &mut IVM) -> Result<u64, VMError> {
        let state = self.axt_state.as_mut().ok_or(VMError::PermissionDenied)?;
        let handle_ptr = vm.register(10);
        let handle_tlv = vm.memory.validate_tlv(handle_ptr)?;
        if handle_tlv.type_id != PointerType::AssetHandle {
            return Err(VMError::NoritoInvalid);
        }
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
        let intent: RemoteSpendIntent =
            norito::decode_from_bytes(op_tlv.payload).map_err(|_| VMError::NoritoInvalid)?;
        if !state.expected_dsids().contains(&intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }
        if !state.has_touch(&intent.asset_dsid) {
            return Err(VMError::PermissionDenied);
        }

        let amount = intent
            .op
            .amount
            .parse::<u128>()
            .map_err(|_| VMError::NoritoInvalid)?;
        if amount > handle.budget.remaining {
            return Err(VMError::PermissionDenied);
        }
        if let Some(per_use) = handle.budget.per_use
            && amount > per_use
        {
            return Err(VMError::PermissionDenied);
        }

        let proof = match vm.register(12) {
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

        let usage = axt::HandleUsage {
            handle,
            intent,
            proof,
            amount,
        };
        self.axt_policy.allow_handle(&usage)?;
        state.record_handle(usage)?;
        Ok(0)
    }

    fn handle_axt_commit(&mut self) -> Result<u64, VMError> {
        let state = self.axt_state.take().ok_or(VMError::PermissionDenied)?;
        match Self::validate_axt_commit(&state) {
            Ok(()) => Ok(0),
            Err(err) => {
                self.axt_state = Some(state);
                Err(err)
            }
        }
    }

    fn validate_axt_commit(state: &axt::HostAxtState) -> Result<(), VMError> {
        state.validate_commit()
    }
}

impl Default for DefaultHost {
    fn default() -> Self {
        Self::new()
    }
}

impl IVMHost for DefaultHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            crate::syscalls::SYSCALL_DEBUG_PRINT => {
                let value = vm.register(10);
                if cfg!(any(test, debug_assertions)) {
                    eprintln!("[IVM] debug_print r10={value}");
                }
                Ok(0)
            }
            crate::syscalls::SYSCALL_EXIT => {
                let status = vm.register(10);
                vm.request_exit();
                vm.set_register(10, status);
                Ok(0)
            }
            crate::syscalls::SYSCALL_ABORT => {
                vm.set_register(10, 0);
                vm.request_abort();
                Ok(0)
            }
            crate::syscalls::SYSCALL_DEBUG_LOG => {
                let ptr = vm.register(10);
                if ptr == 0 {
                    return Ok(0);
                }
                let tlv = vm.memory.validate_tlv(ptr)?;
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
            // Basic pointer‑ABI validations to mirror core host behavior in tests
            crate::syscalls::SYSCALL_SET_ACCOUNT_DETAIL => {
                // r10=&AccountId, r11=&Name, r12=&Json
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::Name)?;
                Self::expect_tlv(vm, 12, PointerType::Json)?;
                Ok(0)
            }
            crate::syscalls::SYSCALL_NFT_MINT_ASSET => {
                // r10=&NftId, r11=&AccountId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                Ok(0)
            }
            crate::syscalls::SYSCALL_NFT_TRANSFER_ASSET => {
                // r10=&AccountId(from), r11=&NftId, r12=&AccountId(to)
                Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                Self::expect_tlv(vm, 11, PointerType::NftId)?;
                Self::expect_tlv(vm, 12, PointerType::AccountId)?;
                Ok(0)
            }
            crate::syscalls::SYSCALL_TRANSFER_ASSET => {
                if self.fastpq_batch_active {
                    self.push_fastpq_batch_entry(vm)
                } else {
                    // r10=&AccountId(from), r11=&AccountId(to), r12=&AssetDefinitionId, r13=amount
                    Self::expect_tlv(vm, 10, PointerType::AccountId)?;
                    Self::expect_tlv(vm, 11, PointerType::AccountId)?;
                    Self::expect_tlv(vm, 12, PointerType::AssetDefinitionId)?;
                    let _ = vm.register(13);
                    Ok(0)
                }
            }
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN => self.begin_fastpq_batch(),
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_END => self.finish_fastpq_batch(),
            crate::syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY => self.apply_fastpq_batch(vm),
            crate::syscalls::SYSCALL_NFT_SET_METADATA => {
                // r10=&NftId, r11=&Json
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Self::expect_tlv(vm, 11, PointerType::Json)?;
                Ok(0)
            }
            crate::syscalls::SYSCALL_NFT_BURN_ASSET => {
                // r10=&NftId
                Self::expect_tlv(vm, 10, PointerType::NftId)?;
                Ok(0)
            }
            crate::syscalls::SYSCALL_ALLOC => {
                // Allocate `x10` bytes on the VM heap and return the pointer in `x10`.
                let size = vm.register(10);
                let addr = vm.alloc_heap(size)?;
                vm.set_register(10, addr);
                // Charge 1 unit of extra gas for the allocation.
                Ok(1)
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
                if tlv.type_id != PointerType::NoritoBytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_TYPE);
                    return Ok(0);
                }
                let req: VrfVerifyRequest = match norito::decode_from_bytes(tlv.payload) {
                    Ok(v) => v,
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        return Ok(0);
                    }
                };

                // Prehash input with domain separation; enforce configured chain_id when present
                if let Some(cid) = &self.chain_id
                    && req.chain_id != *cid
                {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_CHAIN);
                    return Ok(0);
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
                    let mut arr = [0u8; 48];
                    arr.copy_from_slice(bytes);
                    let ct = G1Affine::from_compressed(&arr);
                    if ct.is_some().into() {
                        Some(ct.unwrap())
                    } else {
                        None
                    }
                }
                fn to_g2(bytes: &[u8]) -> Option<G2Affine> {
                    if bytes.len() != 96 {
                        return None;
                    }
                    let mut arr = [0u8; 96];
                    arr.copy_from_slice(bytes);
                    let ct = G2Affine::from_compressed(&arr);
                    if ct.is_some().into() {
                        Some(ct.unwrap())
                    } else {
                        None
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
                            return Ok(0);
                        };
                        let Some(sig) = to_g2(&req.proof) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PROOF);
                            return Ok(0);
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
                            return Ok(0);
                        };
                        let Some(sig) = to_g1(&req.proof) else {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_PROOF);
                            return Ok(0);
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
                        return Ok(0);
                    }
                };

                if !ok {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_VERIFY);
                    return Ok(0);
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
                Ok(0)
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
                if tlv.type_id != PointerType::NoritoBytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_TYPE);
                    return Ok(0);
                }
                let req: VrfVerifyBatchRequest = match norito::decode_from_bytes(tlv.payload) {
                    Ok(v) => v,
                    Err(_) => {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        return Ok(0);
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
                    return Ok(0);
                }

                // Shared helpers
                use blstrs::{Bls12, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective};
                use group::{Curve, Group as _, prime::PrimeCurveAffine};
                use pairing::{MillerLoopResult as _, MultiMillerLoop as _};
                fn to_g1(bytes: &[u8]) -> Option<G1Affine> {
                    if bytes.len() != 48 {
                        return None;
                    }
                    let mut arr = [0u8; 48];
                    arr.copy_from_slice(bytes);
                    let ct = G1Affine::from_compressed(&arr);
                    if ct.is_some().into() {
                        Some(ct.unwrap())
                    } else {
                        None
                    }
                }
                fn to_g2(bytes: &[u8]) -> Option<G2Affine> {
                    if bytes.len() != 96 {
                        return None;
                    }
                    let mut arr = [0u8; 96];
                    arr.copy_from_slice(bytes);
                    let ct = G2Affine::from_compressed(&arr);
                    if ct.is_some().into() {
                        Some(ct.unwrap())
                    } else {
                        None
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
                        return Ok(0);
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
                                return Ok(0);
                            };
                            let Some(sig) = to_g2(&it.proof) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PROOF);
                                vm.set_register(12, idx as u64);
                                return Ok(0);
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
                                return Ok(0);
                            };
                            let Some(sig) = to_g1(&it.proof) else {
                                vm.set_register(10, 0);
                                vm.set_register(11, ERR_PROOF);
                                vm.set_register(12, idx as u64);
                                return Ok(0);
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
                            return Ok(0);
                        }
                    };
                    if !ok {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_VERIFY);
                        vm.set_register(12, idx as u64);
                        return Ok(0);
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
                Ok(0)
            }
            crate::syscalls::SYSCALL_GROW_HEAP => {
                // Increase heap limit by `x10` bytes.
                let size = vm.register(10);
                let new_limit = vm.grow_heap(size)?;
                vm.set_register(10, new_limit);
                Ok(0)
            }
            crate::syscalls::SYSCALL_GET_PRIVATE_INPUT => {
                // Load a private input provided by the host. The index is in `x10`.
                let idx = vm.register(10) as usize;
                if let Some(&val) = self.private_inputs.get(idx) {
                    vm.set_register(10, val);
                    Ok(0)
                } else {
                    Err(VMError::UnknownSyscall(number))
                }
            }
            crate::syscalls::SYSCALL_COMMIT_OUTPUT => {
                // Make the VM's output buffer available to the host.
                self.pub_output = vm.read_output().to_vec();
                Ok(0)
            }
            crate::syscalls::SYSCALL_USE_NULLIFIER => {
                // Record a nullifier and fail if it has already been used.
                let n = vm.register(10);
                if !self.nullifiers.insert(n) {
                    Err(VMError::NullifierAlreadyUsed)
                } else {
                    Ok(0)
                }
            }
            crate::syscalls::SYSCALL_PROVE_EXECUTION => {
                let trace = vm.register_trace();
                match zk::verify_trace(&trace, vm.constraints(), vm.memory_log(), vm.register_log())
                {
                    Ok(_) => {
                        vm.set_register(10, 1);
                        Ok(0)
                    }
                    Err(e) => Err(e),
                }
            }
            crate::syscalls::SYSCALL_VERIFY_PROOF => {
                let trace = vm.register_trace();
                match zk::verify_trace(&trace, vm.constraints(), vm.memory_log(), vm.register_log())
                {
                    Ok(_) => {
                        vm.set_register(10, 1);
                        Ok(0)
                    }
                    Err(_) => Err(VMError::AssertionFailed),
                }
            }
            crate::syscalls::SYSCALL_VERIFY_SIGNATURE => {
                // r10 = &Blob message TLV, r11 = &Blob signature TLV, r12 = &Blob public key TLV, r13 = scheme code
                let decode_blob = |vm: &IVM, reg: usize| -> Result<Vec<u8>, VMError> {
                    let ptr = vm.register(reg);
                    let tlv = vm.memory.validate_tlv(ptr)?;
                    if tlv.type_id != PointerType::Blob {
                        return Err(VMError::NoritoInvalid);
                    }
                    Ok(tlv.payload.to_vec())
                };
                let msg = decode_blob(vm, 10)?;
                let sig = decode_blob(vm, 11)?;
                let pk = decode_blob(vm, 12)?;
                // Ensure these bindings are considered used even if signature schemes are disabled.
                let _ = (&msg, &sig, &pk);
                let scheme_code = vm.register(13) as u8;
                let _scheme = match scheme_code {
                    1 => {
                        #[cfg(feature = "ed25519")]
                        {
                            crate::signature::SignatureScheme::Ed25519
                        }
                        #[cfg(not(feature = "ed25519"))]
                        {
                            vm.set_register(10, 0);
                            return Ok(0);
                        }
                    }
                    2 => {
                        #[cfg(feature = "secp256k1")]
                        {
                            crate::signature::SignatureScheme::Secp256k1
                        }
                        #[cfg(not(feature = "secp256k1"))]
                        {
                            vm.set_register(10, 0);
                            return Ok(0);
                        }
                    }
                    3 => {
                        #[cfg(feature = "ml-dsa")]
                        {
                            crate::signature::SignatureScheme::MlDsa
                        }
                        #[cfg(not(feature = "ml-dsa"))]
                        {
                            vm.set_register(10, 0);
                            return Ok(0);
                        }
                    }
                    _ => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };
                #[cfg(any(feature = "ed25519", feature = "secp256k1", feature = "ml-dsa"))]
                {
                    let ok = crate::signature::verify_signature(_scheme, &msg, &sig, &pk);
                    vm.set_register(10, if ok { 1 } else { 0 });
                    Ok(0)
                }
                // When no signature scheme features are enabled, the match above already
                // returned Ok(0) for all codes; nothing remains to execute here.
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
                let digest = Sm3Digest::hash(tlv.payload);
                let bytes = digest.as_bytes();
                let addr = DefaultHost::alloc_blob_tlv(vm, bytes)?;
                vm.set_register(10, addr);
                Ok(0)
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
                let distid = if distid_ptr != 0 {
                    let distid_tlv = vm.memory.validate_tlv(distid_ptr)?;
                    if distid_tlv.type_id != PointerType::Blob {
                        return Err(VMError::NoritoInvalid);
                    }
                    std::str::from_utf8(distid_tlv.payload)
                        .map(|s| s.to_owned())
                        .map_err(|_| VMError::NoritoInvalid)?
                } else {
                    Sm2PublicKey::default_distid()
                };

                let msg = msg_tlv.payload;
                let sig_bytes = sig_tlv.payload;
                if sig_bytes.len() != Sm2Signature::LENGTH {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let mut sig_buf = [0u8; Sm2Signature::LENGTH];
                sig_buf.copy_from_slice(sig_bytes);
                let signature = match Sm2Signature::from_bytes(&sig_buf) {
                    Ok(sig) => sig,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };

                let public_key = match Sm2PublicKey::from_sec1_bytes(&distid, pk_tlv.payload) {
                    Ok(pk) => pk,
                    Err(_) => {
                        vm.set_register(10, 0);
                        return Ok(0);
                    }
                };

                let ok = public_key.verify(msg, &signature).is_ok();
                vm.set_register(10, if ok { 1 } else { 0 });
                Ok(0)
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

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                if nonce_tlv.payload.len() != 12 {
                    vm.set_register(10, 0);
                    return Ok(0);
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
                Ok(0)
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

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                if nonce_tlv.payload.len() != 12 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let mut nonce = [0u8; 12];
                nonce.copy_from_slice(nonce_tlv.payload);
                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);

                if ct_tlv.payload.len() < 16 {
                    vm.set_register(10, 0);
                    return Ok(0);
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
                Ok(0)
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

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(0);
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
                Ok(0)
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

                if key_tlv.payload.len() != 16 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(key_tlv.payload);
                let key = Sm4Key::new(key_bytes);

                let aad = aad_opt.as_ref().map(|tlv| tlv.payload).unwrap_or(&[]);
                let tag_len = if tag_len_raw == 0 { 16 } else { tag_len_raw };

                if ct_tlv.payload.len() < tag_len {
                    vm.set_register(10, 0);
                    return Ok(0);
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
                Ok(0)
            }
            crate::syscalls::SYSCALL_INPUT_PUBLISH_TLV => {
                // Mirror a TLV into INPUT (no-op if already INPUT); validate envelope/policy.
                let src = vm.register(10);
                if src == 0 {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                let input_lo = crate::memory::Memory::INPUT_START;
                let input_hi =
                    crate::memory::Memory::INPUT_START + crate::memory::Memory::INPUT_SIZE;
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
                // Read header to determine total length
                let hdr = vm
                    .memory
                    .load_region(src, 7)
                    .map_err(|_| VMError::NoritoInvalid)?;
                let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
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
                Ok(0)
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
                Ok(0)
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
                Ok(0)
            }
            // --- ZK verify/state-read stubs ---
            crate::syscalls::SYSCALL_ZK_VERIFY_TRANSFER
            | crate::syscalls::SYSCALL_ZK_VERIFY_UNSHIELD
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT
            | crate::syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => {
                // Expect a NoritoBytes TLV pointer in r10 containing a bare Norito payload
                // for `Halo2OpenVerify`. Decode and verify via `iroha_zkp_halo2`.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                if tlv.payload.len() > self.zk_cfg.max_envelope_bytes {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_ENVELOPE_SIZE);
                    return Ok(0);
                }
                // Decode the envelope first to apply config gating
                let decoded_env: Result<iroha_zkp_halo2::OpenVerifyEnvelope, _> =
                    norito::decode_from_bytes(tlv.payload);
                if decoded_env.is_err() {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_DECODE);
                    return Ok(0);
                }
                let env = decoded_env.unwrap();
                // Enforce config gates
                if !self.zk_cfg.enabled {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_DISABLED);
                    return Ok(0);
                }
                if let Err(code) = self.validate_transcript_label(number, &env.transcript_label) {
                    vm.set_register(10, 0);
                    vm.set_register(11, code);
                    return Ok(0);
                }
                if let Some(proof_len) = Self::proof_len_bytes(&env.proof) {
                    if proof_len > self.zk_cfg.max_proof_bytes {
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_PROOF_LEN);
                        return Ok(0);
                    }
                } else {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_DECODE);
                    return Ok(0);
                }
                // Envelope must carry VK binding metadata
                if env.vk_commitment.is_none() || env.public_inputs_schema_hash.is_none() {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_VK_MISSING);
                    return Ok(0);
                }
                // Backend must be IPA and curve must be allowed
                if self.zk_cfg.backend != ZkHalo2Backend::Ipa {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_BACKEND);
                    return Ok(0);
                }
                let curve_ok = match (self.zk_cfg.curve, env.params.curve_id) {
                    (ZkCurve::Pallas, cid)
                        if cid == iroha_zkp_halo2::ZkCurveId::Pallas.as_u16() =>
                    {
                        true
                    }
                    (ZkCurve::Pasta, cid) if cid == iroha_zkp_halo2::ZkCurveId::Pasta.as_u16() => {
                        true
                    }
                    (ZkCurve::Goldilocks, cid)
                        if cid == iroha_zkp_halo2::ZkCurveId::Goldilocks.as_u16() =>
                    {
                        true
                    }
                    (ZkCurve::Bn254, cid) if cid == iroha_zkp_halo2::ZkCurveId::Bn254.as_u16() => {
                        true
                    }
                    _ => false,
                };
                if !curve_ok {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_CURVE);
                    return Ok(0);
                }
                // Enforce max_k based on params.n = 2^k
                let n = env.params.n as usize;
                let k = if n.is_power_of_two() {
                    n.trailing_zeros()
                } else {
                    u32::MAX
                };
                if k == u32::MAX || k > self.zk_cfg.max_k {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_K);
                    return Ok(0);
                }
                // Re-encode the envelope to Norito bytes and run verifier on the same bytes
                let env_bytes = norito::to_bytes(&env).map_err(|_| VMError::NoritoInvalid)?;
                match crate::zk_verify::verify_open_envelope(&env_bytes) {
                    Ok(ok) => {
                        vm.set_register(10, if ok { 1 } else { 0 });
                        vm.set_register(11, if ok { 0 } else { ERR_VERIFY });
                        Ok(0)
                    }
                    Err(_e) => {
                        // Non-VerificationFailed errors are treated as invalid payloads
                        vm.set_register(10, 0);
                        vm.set_register(11, ERR_DECODE);
                        Ok(0)
                    }
                }
            }
            crate::syscalls::SYSCALL_ZK_ROOTS_GET | crate::syscalls::SYSCALL_ZK_VOTE_GET_TALLY => {
                // Expect a NoritoBytes TLV pointer in r10 (request). Stub returns no data and
                // writes a Norito TLV response into INPUT and returns a pointer.
                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                if number == crate::syscalls::SYSCALL_ZK_ROOTS_GET {
                    // Decode request
                    let _req: crate::zk_verify::RootsGetRequest =
                        norito::decode_from_bytes(tlv.payload)
                            .map_err(|_| VMError::NoritoInvalid)?;
                    // Stubbed response (empty roots)
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
                }
                Ok(0)
            }
            crate::syscalls::SYSCALL_ZK_VERIFY_BATCH => {
                // r10 = &NoritoBytes(Vec<OpenVerifyEnvelope>)
                // Returns r10 = &NoritoBytes(Vec<u8>) with per-envelope results:
                //   1 = verified true; 0 = verified false.
                // Overall errors use r11 codes similar to single-verify:
                //   1=ERR_DISABLED, 2=ERR_BACKEND, 3=ERR_CURVE, 4=ERR_K, 5=ERR_DECODE, 7=ERR_BATCH
                const OK: u64 = 0;

                let ptr = vm.register(10);
                let tlv = vm.memory.validate_tlv(ptr)?;
                if tlv.type_id != PointerType::NoritoBytes {
                    return Err(VMError::NoritoInvalid);
                }
                // Decode batch
                let envs: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                    match norito::decode_from_bytes(tlv.payload) {
                        Ok(v) => v,
                        Err(_) => {
                            vm.set_register(10, 0);
                            vm.set_register(11, ERR_DECODE);
                            return Ok(0);
                        }
                    };
                if envs.len() as u32 > self.zk_cfg.verifier_max_batch {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_BATCH);
                    return Ok(0);
                }
                if !self.zk_cfg.enabled {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_DISABLED);
                    return Ok(0);
                }
                if self.zk_cfg.backend != ZkHalo2Backend::Ipa {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_BACKEND);
                    return Ok(0);
                }

                // Pre-screen curves and k per envelope; build per-item status, then run verifier
                let mut statuses: Vec<u8> = Vec::with_capacity(envs.len());
                let mut first_error: Option<u64> = None;
                // Optionally: enforce budget (best-effort; skip actual timing in this build)
                for env in &envs {
                    let mut status = 0u8;
                    if let Err(code) = self.validate_transcript_label(number, &env.transcript_label)
                    {
                        first_error.get_or_insert(code);
                        statuses.push(status);
                        continue;
                    }
                    if env.vk_commitment.is_none() || env.public_inputs_schema_hash.is_none() {
                        first_error.get_or_insert(ERR_VK_MISSING);
                        statuses.push(status);
                        continue;
                    }
                    let Some(proof_len) = Self::proof_len_bytes(&env.proof) else {
                        first_error.get_or_insert(ERR_DECODE);
                        statuses.push(status);
                        continue;
                    };
                    if proof_len > self.zk_cfg.max_proof_bytes {
                        first_error.get_or_insert(ERR_PROOF_LEN);
                        statuses.push(status);
                        continue;
                    }
                    // Curve gating
                    let curve_ok = match (self.zk_cfg.curve, env.params.curve_id) {
                        (ZkCurve::Pallas, cid)
                            if cid == iroha_zkp_halo2::ZkCurveId::Pallas.as_u16() =>
                        {
                            true
                        }
                        (ZkCurve::Pasta, cid)
                            if cid == iroha_zkp_halo2::ZkCurveId::Pasta.as_u16() =>
                        {
                            true
                        }
                        (ZkCurve::Goldilocks, cid)
                            if cid == iroha_zkp_halo2::ZkCurveId::Goldilocks.as_u16() =>
                        {
                            true
                        }
                        (ZkCurve::Bn254, cid)
                            if cid == iroha_zkp_halo2::ZkCurveId::Bn254.as_u16() =>
                        {
                            true
                        }
                        _ => false,
                    };
                    if !curve_ok {
                        first_error.get_or_insert(ERR_CURVE);
                        statuses.push(status);
                        continue;
                    }
                    // k gating
                    let n = env.params.n as usize;
                    let k = if n.is_power_of_two() {
                        n.trailing_zeros()
                    } else {
                        u32::MAX
                    };
                    if k == u32::MAX || k > self.zk_cfg.max_k {
                        first_error.get_or_insert(ERR_K);
                        statuses.push(status);
                        continue;
                    }
                    // Verify this envelope
                    let bytes = match norito::to_bytes(env) {
                        Ok(b) => b,
                        Err(_) => {
                            first_error.get_or_insert(ERR_DECODE);
                            statuses.push(status);
                            continue;
                        }
                    };
                    if bytes.len() > self.zk_cfg.max_envelope_bytes {
                        first_error.get_or_insert(ERR_ENVELOPE_SIZE);
                        statuses.push(status);
                        continue;
                    }
                    match crate::zk_verify::verify_open_envelope(&bytes) {
                        Ok(ok) => {
                            status = if ok { 1 } else { 0 };
                            if !ok {
                                first_error.get_or_insert(ERR_VERIFY);
                            }
                        }
                        Err(_) => {
                            first_error.get_or_insert(ERR_DECODE);
                        }
                    }
                    if status == 0 {
                        statuses.push(0);
                    } else {
                        statuses.push(status);
                    }
                }

                if statuses.is_empty() {
                    vm.set_register(10, 0);
                    vm.set_register(11, ERR_DECODE);
                    return Ok(0);
                }

                // Encode statuses as NoritoBytes and return pointer. Also set r12 to the
                // index of the first failing item (status=0) if any; otherwise set to u64::MAX.
                let body = norito::to_bytes(&statuses).map_err(|_| VMError::NoritoInvalid)?;
                let mut out = Vec::with_capacity(7 + body.len() + 32);
                out.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
                out.push(1);
                out.extend_from_slice(&(body.len() as u32).to_be_bytes());
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
                    vm.set_register(11, OK);
                }
                Ok(0)
            }
            syscalls::SYSCALL_AXT_BEGIN => self.handle_axt_begin(vm),
            syscalls::SYSCALL_AXT_TOUCH => self.handle_axt_touch(vm),
            syscalls::SYSCALL_AXT_COMMIT => self.handle_axt_commit(),
            syscalls::SYSCALL_VERIFY_DS_PROOF => self.handle_axt_verify_ds_proof(vm),
            syscalls::SYSCALL_USE_ASSET_HANDLE => self.handle_axt_use_asset_handle(vm),
            _ => Err(VMError::UnknownSyscall(number)),
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
    use crate::pointer_abi::PointerType;

    #[test]
    fn downcast_default_host() {
        let mut host: Box<dyn IVMHost + Send> = Box::new(DefaultHost::new());
        assert!(host.as_any().downcast_mut::<DefaultHost>().is_some());
    }

    #[test]
    fn expect_tlv_enforces_pointer_policy() {
        crate::set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let mut tlv = Vec::new();
        tlv.extend_from_slice(&(PointerType::TestOnly as u16).to_be_bytes());
        tlv.push(1);
        tlv.extend_from_slice(&0u32.to_be_bytes());
        let hash: [u8; 32] = iroha_crypto::Hash::new([]).into();
        tlv.extend_from_slice(&hash);
        let ptr = vm.alloc_input_tlv(&tlv).expect("allocate TLV");
        vm.set_register(10, ptr);
        let err = DefaultHost::expect_tlv(&vm, 10, PointerType::TestOnly).unwrap_err();
        assert!(matches!(
            err,
            VMError::AbiTypeNotAllowed { abi: 1, type_id } if type_id == PointerType::TestOnly as u16
        ));
    }
}
