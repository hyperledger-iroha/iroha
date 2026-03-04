use ivm::{IVMHost, syscalls};

fn build_env(k: u32) -> Vec<u8> {
    use h2::{PrimeField64 as F, norito_helpers as nh};
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};
    let n = 1usize << k;
    let params = h2::Params::new(n).unwrap();
    let coeffs: Vec<F> = (0u64..(n as u64)).map(|i| F::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_TRANSFER);
    let p_g = poly.commit(&params).unwrap();
    let z = F::from(3u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let params_wire = nh::params_to_wire(&params);
    let public_wire = nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g);
    let proof_wire = nh::proof_to_wire(&proof);
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: public_wire,
        proof: proof_wire,
        transcript_label: ivm::host::LABEL_TRANSFER.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode env")
}

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
fn zk_verify_positive_for_small_k_under_max_k() {
    let mut vm = ivm::IVM::new(1_000_000);
    // Allow up to k=8 for this test matrix
    let cfg = ivm::host::ZkHalo2Config {
        enabled: true,
        max_k: 8,
        ..Default::default()
    };
    let mut host = ivm::host::DefaultHost::new().with_zk_halo2_config(cfg);
    for &k in &[3u32, 4u32] {
        let env = build_env(k);
        let tlv = make_tlv(ivm::PointerType::NoritoBytes as u16, &env);
        let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
        vm.set_register(10, ptr);
        let _ = host
            .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
            .expect("syscall ok");
        assert_eq!(vm.register(10), 1, "expected r10=1 for k={k} <= max_k");
    }
}
