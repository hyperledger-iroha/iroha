use iroha_zkp_halo2::backend::pallas::PallasBackend;
use ivm::{
    IVMHost,
    host::{DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

#[test]
fn zk_verify_batch_syscall_mixed() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    // Build one valid and one invalid envelope (flip t) under ToyP61 IPA
    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_BATCH);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_BATCH.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let mut bad_pub = env_ok.public.clone();
    bad_pub.t[0] = bad_pub.t[0].wrapping_add(1);
    let env_bad = h2::OpenVerifyEnvelope {
        public: bad_pub,
        ..env_ok.clone()
    };
    let payload = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

    // Build TLV
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);

    // Host with gating enabled
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 50,
        verifier_max_batch: 8,
        ..ZkHalo2Config::default()
    };
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(11), 0, "overall status OK");
    assert_eq!(vm.register(12), 1, "first failing index = 1");

    // Decode statuses
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    assert_eq!(out_tlv.type_id, ivm::PointerType::NoritoBytes);
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![1, 0]);
}

#[test]
fn zk_verify_batch_syscall_mixed_three_items() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    // Common params
    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);

    // First valid
    let label = ivm::host::LABEL_BATCH;
    let mut tr1 = h2::Transcript::new(label);
    let p_g1 = poly.commit(&params).unwrap();
    let z1 = h2::PrimeField64::from(5u64);
    let (proof1, t1) = poly.open(&params, &mut tr1, z1, p_g1).unwrap();
    let env_ok1 = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z1, t1, p_g1),
        proof: nh::proof_to_wire(&proof1),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    // Invalid by flipping t
    let mut bad_pub = env_ok1.public.clone();
    bad_pub.t[0] = bad_pub.t[0].wrapping_add(1);
    let env_bad = h2::OpenVerifyEnvelope {
        public: bad_pub,
        ..env_ok1.clone()
    };

    // Second valid with different z/transcript
    let mut tr2 = h2::Transcript::new(label);
    let p_g2 = poly.commit(&params).unwrap();
    let z2 = h2::PrimeField64::from(7u64);
    let (proof2, t2) = poly.open(&params, &mut tr2, z2, p_g2).unwrap();
    let env_ok2 = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z2, t2, p_g2),
        proof: nh::proof_to_wire(&proof2),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    let payload = norito::to_bytes(&vec![env_ok1, env_bad, env_ok2]).expect("encode");
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);

    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 50,
        verifier_max_batch: 8,
        ..ZkHalo2Config::default()
    };
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(11), 0);
    assert_eq!(vm.register(12), 1);
    assert_eq!(vm.register(12), 1);
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![1, 0, 1]);
}

#[test]
fn zk_verify_batch_syscall_rejects_tampered_bound_metadata() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let metadata = h2::PolyOpenTranscriptMetadata {
        vk_commitment: Some([0x11; 32]),
        public_inputs_schema_hash: Some([0x22; 32]),
        domain_tag: Some([0x33; 32]),
    };
    let mut tr = h2::Transcript::new(ivm::host::LABEL_BATCH);
    let (proof, t) = poly
        .open_with_metadata(&params, &mut tr, z, p_g, metadata)
        .unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_BATCH.to_string(),
        vk_commitment: metadata.vk_commitment,
        public_inputs_schema_hash: metadata.public_inputs_schema_hash,
        domain_tag: metadata.domain_tag,
    };

    let env_bad = h2::OpenVerifyEnvelope {
        domain_tag: Some([0x44; 32]),
        ..env_ok.clone()
    };
    let payload = norito::to_bytes(&vec![env_ok, env_bad]).expect("encode batch");

    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);

    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 50,
        verifier_max_batch: 8,
        ..ZkHalo2Config::default()
    };
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(11), 0);
    assert_eq!(vm.register(12), 1);
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![1, 0]);
}

#[test]
fn zk_verify_batch_syscall_k_gated_sandwich() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    // Params for allowed item (k=8)
    let params_lo = h2::Params::new(8).unwrap();

    // Polynomial reused across commits/opens
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);

    // First (gated by k)
    // Gate by setting n=4096 in wire params; proof/public are dummies (won't be verified)
    let label = ivm::host::LABEL_BATCH;
    let env_gated1 = h2::OpenVerifyEnvelope {
        params: h2::IpaParams {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 4096,
            g: Vec::new(),
            h: Vec::new(),
            u: [0; 32],
        },
        public: h2::PolyOpenPublic {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 4096,
            z: [0; 32],
            t: [0; 32],
            p_g: [0; 32],
        },
        proof: h2::IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: [0; 32],
            b_final: [0; 32],
        },
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    // Middle (allowed; k=8)
    let mut tr2 = h2::Transcript::new(label);
    let p_g2 = poly.commit(&params_lo).unwrap();
    let z2 = h2::PrimeField64::from(7u64);
    let (proof2, t2) = poly.open(&params_lo, &mut tr2, z2, p_g2).unwrap();
    let env_ok = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params_lo),
        public: nh::poly_open_public::<PallasBackend>(params_lo.n(), z2, t2, p_g2),
        proof: nh::proof_to_wire(&proof2),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    // Last (gated by k)
    let env_gated2 = h2::OpenVerifyEnvelope {
        params: h2::IpaParams {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 4096,
            g: Vec::new(),
            h: Vec::new(),
            u: [0; 32],
        },
        public: h2::PolyOpenPublic {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 4096,
            z: [0; 32],
            t: [0; 32],
            p_g: [0; 32],
        },
        proof: h2::IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: [0; 32],
            b_final: [0; 32],
        },
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };

    let payload = norito::to_bytes(&vec![env_gated1, env_ok, env_gated2]).expect("encode");
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);

    // Host with max_k=8 allows middle item only
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 8,
        verifier_budget_ms: 50,
        verifier_max_batch: 8,
        ..ZkHalo2Config::default()
    };
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(11), 0);
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![0, 1, 0]);
    assert_eq!(vm.register(12), 0);
}
