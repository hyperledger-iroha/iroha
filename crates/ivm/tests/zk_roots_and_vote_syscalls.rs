use ivm::{IVM, IVMHost, Memory, PointerType, syscalls, zk_verify};

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn zk_roots_get_roundtrip() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk roots get roundtrip gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let mut vm = IVM::new(1_000_000);
    let mut host = ivm::host::DefaultHost::new();

    // Build request
    let req = zk_verify::RootsGetRequest {
        asset_id: "rose#domain".to_string(),
        max: 10,
    };
    let payload = norito::to_bytes(&req).expect("encode");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    // Place TLV into INPUT region and pass pointer in r10
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);

    // Call syscall; r10 becomes pointer to NoritoBytes TLV response
    host.syscall(syscalls::SYSCALL_ZK_ROOTS_GET, &mut vm)
        .expect("syscall ok");
    let resp_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(resp_ptr).expect("valid tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let resp: zk_verify::RootsGetResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode response");
    // DefaultHost returns empty roots
    assert_eq!(resp.latest, [0u8; 32]);
    assert!(resp.roots.is_empty());
    assert_eq!(resp.height, 0);
}

#[test]
fn zk_vote_get_tally_roundtrip() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: zk vote tally roundtrip gated. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    let mut vm = IVM::new(1_000_000);
    let mut host = ivm::host::DefaultHost::new();

    let req = zk_verify::VoteGetTallyRequest {
        election_id: "election#domain".to_string(),
    };
    let payload = norito::to_bytes(&req).expect("encode");
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &payload);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    host.syscall(syscalls::SYSCALL_ZK_VOTE_GET_TALLY, &mut vm)
        .expect("syscall ok");
    let resp_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(resp_ptr).expect("valid tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let resp: zk_verify::VoteGetTallyResponse =
        norito::decode_from_bytes(tlv.payload).expect("decode response");
    assert!(!resp.finalized);
    assert!(resp.tally.is_empty());
}
