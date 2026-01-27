use ivm::{IVMHost, syscalls};

#[test]
fn zk_verify_transfer_halo2_opening_succeeds() {
    // Build a tiny polynomial and its IPA opening proof using the toy Halo2 backend
    use h2::{PrimeField64 as F, norito_helpers as nh};
    use iroha_zkp_halo2::{self as h2, backend::pallas::PallasBackend};

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<F> = (0u64..8u64).map(|i| F::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_TRANSFER);
    let p_g = poly.commit(&params).unwrap();
    let z = F::from(7u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();

    // Compose the Norito payload for the syscall
    let params_wire = nh::params_to_wire(&params);
    let public_wire = h2::norito_helpers::poly_open_public::<PallasBackend>(params.n(), z, t, p_g);
    let proof_wire = nh::proof_to_wire(&proof);
    // Build a Norito envelope as the TLV payload
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: public_wire,
        proof: proof_wire,
        transcript_label: ivm::host::LABEL_TRANSFER.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let payload = norito::to_bytes(&env).expect("encode");

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
    // r10=1 indicates success
    assert_eq!(vm.register(10), 1);
}
