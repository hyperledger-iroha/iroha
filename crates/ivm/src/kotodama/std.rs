//! Minimal helpers used by tests and examples to build TLVs for INPUT.
//!
//! These utilities construct Norito TLV envelopes expected by the pointer‑ABI
//! and place them into the VM INPUT region using the internal bump allocator
//! (`IVM::alloc_input_tlv`). In addition, Kotodama provides language‑level
//! intrinsics (e.g., `zk_verify_transfer`, `zk_vote_verify_ballot`, and the
//! vendor bridge wrappers) that lower to SCALLs with Norito TLVs. These helpers
//! remain useful for tests and for building TLVs from host code.

use crate::{
    IVM, Memory, PointerType,
    host::IVMHost,
    instruction::wide,
    schema_registry::{DefaultRegistry, SchemaRegistry},
    syscalls,
};

/// Build a TLV with type `NoritoBytes` for the given payload and place it in
/// the VM INPUT region. Returns the pointer to the TLV.
pub fn input_tlv_norito_bytes(vm: &mut IVM, payload: &[u8]) -> u64 {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    vm.alloc_input_tlv(&tlv).expect("alloc input tlv")
}

/// Write a TLV into INPUT starting at offset 0 and return its fixed pointer.
/// This variant is useful when programs expect the TLV at `Memory::INPUT_START`.
pub fn preload_input_tlv(vm: &mut IVM, payload: &[u8]) -> u64 {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    vm.memory.preload_input(0, &tlv).expect("preload input tlv");
    Memory::INPUT_START
}

/// Emit a single SCALL instruction into the given code buffer.
pub fn emit_scall(code: &mut Vec<u8>, number: u32) {
    let word = crate::encoding::wide::encode_sys(wide::system::SCALL, (number & 0xFF) as u8);
    code.extend_from_slice(&word.to_le_bytes());
}

/// Emit a HALT instruction into the given code buffer.
pub fn emit_halt(code: &mut Vec<u8>) {
    code.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
}

/// Convenience wrapper: perform a ZK verify syscall by building a NoritoBytes TLV
/// from `env_bytes`, placing it into INPUT, and invoking the host syscall.
/// Returns the value of `r10` after the syscall (1 = success, 0 = failure).
pub fn zk_verify_with_env(
    host: &mut dyn IVMHost,
    vm: &mut IVM,
    env_bytes: &[u8],
    number: u32,
) -> u64 {
    let ptr = input_tlv_norito_bytes(vm, env_bytes);
    vm.set_register(10, ptr);
    let _ = host.syscall(number, vm).expect("syscall ok");
    vm.register(10)
}

/// Convenience wrapper specifically for ZK_VERIFY_TRANSFER.
pub fn zk_verify_transfer(host: &mut dyn IVMHost, vm: &mut IVM, env_bytes: &[u8]) -> u64 {
    zk_verify_with_env(host, vm, env_bytes, syscalls::SYSCALL_ZK_VERIFY_TRANSFER)
}

/// Convenience wrapper specifically for ZK_VERIFY_UNSHIELD.
pub fn zk_verify_unshield(host: &mut dyn IVMHost, vm: &mut IVM, env_bytes: &[u8]) -> u64 {
    zk_verify_with_env(host, vm, env_bytes, syscalls::SYSCALL_ZK_VERIFY_UNSHIELD)
}

/// Vendor bridge: enqueue a built-in instruction by passing a Norito-encoded
/// `InstructionBox` in a `NoritoBytes` TLV via INPUT and invoking the vendor
/// syscall. Returns the host gas cost (0 for `DefaultHost`; CoreHost returns
/// metered costs) and leaves r10 unspecified.
pub fn vendor_execute_instruction_bytes(
    host: &mut dyn IVMHost,
    vm: &mut IVM,
    instr_bytes: &[u8],
) -> u64 {
    let ptr = input_tlv_norito_bytes(vm, instr_bytes);
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, vm)
        .expect("syscall ok")
}

/// Vendor bridge helper for queries (JSON envelope path). Accepts arbitrary
/// Norito bytes and performs `SMARTCONTRACT_EXECUTE_QUERY`.
pub fn vendor_execute_query_bytes(host: &mut dyn IVMHost, vm: &mut IVM, query_bytes: &[u8]) -> u64 {
    let ptr = input_tlv_norito_bytes(vm, query_bytes);
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, vm)
        .expect("syscall ok")
}

/// ZK batch verify helper: serialize a slice of `OpenVerifyEnvelope` and invoke
/// the `ZK_VERIFY_BATCH` syscall. Returns `(status, out_ptr)` where `status`
/// is the value left in `r11` and `out_ptr` is the pointer returned in `r10`
/// (0 if no pointer was returned). On success (`status=0`), the caller can
/// decode `&NoritoBytes(Vec<u8>)` at `out_ptr` for per‑item results.
pub fn zk_verify_batch_envs(
    host: &mut dyn IVMHost,
    vm: &mut IVM,
    envs: &[iroha_zkp_halo2::OpenVerifyEnvelope],
) -> (u64, u64) {
    // Encode as a Vec<T> to satisfy NoritoSerialize; slices [T] are not supported directly.
    let payload = norito::to_bytes(&envs.to_vec()).expect("encode envelopes");
    let ptr = input_tlv_norito_bytes(vm, &payload);
    vm.set_register(10, ptr);
    let _ = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, vm)
        .expect("syscall ok");
    (vm.register(11), vm.register(10))
}

/// Helper: encode an Order JSON payload into Norito bytes using the default registry.
pub fn encode_order_json(json: &[u8]) -> Option<Vec<u8>> {
    DefaultRegistry::new().encode_json("Order", json)
}

/// Helper: decode Norito bytes (Order) into minified JSON using the default registry.
pub fn decode_order_to_json(bytes: &[u8]) -> Option<Vec<u8>> {
    let registry = DefaultRegistry::new();
    if let Some(v) = registry.decode_to_json("Order", bytes) {
        return Some(v);
    }

    // Fallback: attempt to decode using other known versions of the Order schema.
    registry
        .list_versions("Order")
        .into_iter()
        .flatten()
        .filter_map(|(name, _)| if name == "Order" { None } else { Some(name) })
        .find_map(|name| registry.decode_to_json(&name, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_helpers_roundtrip() {
        let j = br#"{"qty":10, "side":"buy"}"#;
        let enc = encode_order_json(j).expect("encode");
        let dec = decode_order_to_json(&enc).expect("decode");
        let s = std::str::from_utf8(&dec).unwrap();
        assert!(s.contains("qty") && s.contains("side"));
    }
}
