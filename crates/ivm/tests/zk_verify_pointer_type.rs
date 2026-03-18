//! ZK verify syscalls must accept only `&NoritoBytes` envelopes. Wrong types are rejected.

use ivm::{IVM, IVMHost, Memory, PointerType, host::DefaultHost, syscalls};

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn reject_non_norito_bytes_pointer_type() {
    // Minimal valid Norito payload bytes (structure won't be fully decoded due to type)
    let bogus_payload = vec![0u8; 8];
    // Use Blob instead of NoritoBytes to trigger type check failure
    let tlv = make_tlv(PointerType::Blob as u16, &bogus_payload);

    let mut vm = IVM::new(1_000_000);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let mut host = DefaultHost::new();

    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let res = host.syscall(num, &mut vm);
        assert!(
            matches!(res, Err(ivm::VMError::NoritoInvalid)),
            "expected NoritoInvalid for 0x{num:02x}"
        );
    }
}
