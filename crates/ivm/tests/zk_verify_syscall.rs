use ivm::{IVMHost, syscalls};

#[test]
fn zk_verify_transfer_default_host_reports_disabled() {
    let payload = vec![0xAA, 0xBB];

    // Build a TLV envelope: type(0x0009)=NoritoBytes, ver=1, len=payload.len(), hash=Hash(payload)
    let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
    tlv.push(1u8); // version
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let hash: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&hash);

    // Allocate TLV in VM input region and invoke the verify syscall
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = ivm::host::DefaultHost::new();
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    // Pass pointer in r10 and call verify syscall
    vm.set_register(10, ptr);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), ivm::host::ERR_DISABLED);
}
