#![cfg(feature = "ivm_zk_tests")]

use iroha_zkp_halo2::{
    GoldilocksParams, GoldilocksPolynomial, GoldilocksScalar, OpenVerifyEnvelope, Transcript,
    backend::goldilocks::GoldilocksBackend, norito_helpers as nh,
};
use ivm::{IVMHost, syscalls};

fn tlv_from_env(env: &OpenVerifyEnvelope) -> Vec<u8> {
    let payload = norito::to_bytes(env).expect("encode envelope");
    let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let hash: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&hash);
    tlv
}

fn make_goldilocks_envelope() -> OpenVerifyEnvelope {
    let params = GoldilocksParams::new(8).expect("params");
    let coeffs: Vec<GoldilocksScalar> = (0u64..8).map(|i| GoldilocksScalar::from(i + 1)).collect();
    let poly = GoldilocksPolynomial::from_coeffs(coeffs);
    let label = ivm::host::LABEL_TRANSFER;
    let mut tr = Transcript::new(label);
    let p_g = poly.commit(&params).expect("commit");
    let z = GoldilocksScalar::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<GoldilocksBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

#[test]
fn zk_verify_transfer_goldilocks_opening_succeeds() {
    let env = make_goldilocks_envelope();
    let tlv = tlv_from_env(&env);

    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = ivm::host::DefaultHost::new().with_zk_curve_str("goldilocks");
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 1);
    assert_eq!(vm.register(11), 0);
}

#[test]
fn zk_verify_transfer_goldilocks_rejected_by_toy_curve() {
    let env = make_goldilocks_envelope();
    let tlv = tlv_from_env(&env);

    let mut vm = ivm::IVM::new(1_000_000);
    // Default host uses the Toy/Pallas curve; Goldilocks envelope must be rejected.
    let mut host = ivm::host::DefaultHost::new();
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 3); // ERR_CURVE
}
