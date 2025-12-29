#![cfg(feature = "ivm_vrf_tests")]
use ivm::{self, IVM, IVMHost, Memory, PointerType};

fn build_vrf_req(
    variant: u8,
    pk_len: usize,
    proof_len: usize,
    chain_id: &[u8],
    input: &[u8],
) -> Vec<u8> {
    use ivm::vrf::VrfVerifyRequest;
    let req = VrfVerifyRequest {
        variant,
        pk: vec![0u8; pk_len],
        proof: vec![0u8; proof_len],
        chain_id: chain_id.to_vec(),
        input: input.to_vec(),
    };
    let body = norito::to_bytes(&req).expect("encode req");
    let mut tlv = Vec::with_capacity(7 + body.len() + 32);
    tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(body.len() as u32).to_be_bytes());
    tlv.extend_from_slice(&body);
    let h: [u8; 32] = iroha_crypto::Hash::new(&body).into();
    tlv.extend_from_slice(&h);
    tlv
}

#[test]
fn vrf_verify_chain_mismatch_sets_err_chain() {
    // Configure host with a fixed chain id
    let host = ivm::host::DefaultHost::new().with_chain_id(b"expected".to_vec());
    let mut vm = IVM::new(0);
    vm.set_host(host);
    // Build request with different chain id; use variant 1 and lengths to pass pk/proof length checks
    let tlv = build_vrf_req(1, 48, 96, b"different", b"msg");
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let _ = unsafe {
        let host_ptr = vm
            .host_mut_any()
            .unwrap()
            .downcast_mut::<ivm::host::DefaultHost>()
            .unwrap() as *mut ivm::host::DefaultHost;
        (*host_ptr).syscall(ivm::syscalls::SYSCALL_VRF_VERIFY, &mut vm)
    }
    .expect("syscall ok");
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 8, "ERR_CHAIN");
}
// Gated by feature at crate level
