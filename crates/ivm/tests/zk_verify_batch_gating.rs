use iroha_zkp_halo2 as h2;
use iroha_zkp_halo2::backend::pallas::PallasBackend;
use ivm::{
    IVMHost,
    host::{DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

fn make_tlv_from_bytes(payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ivm::PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

fn dummy_envelope(n: u32, curve: h2::ZkCurveId, _label: &str) -> h2::OpenVerifyEnvelope {
    h2::OpenVerifyEnvelope {
        params: h2::IpaParams {
            version: 1,
            curve_id: curve.as_u16(),
            n,
            g: Vec::new(),
            h: Vec::new(),
            u: [0; 32],
        },
        public: h2::PolyOpenPublic {
            version: 1,
            curve_id: curve.as_u16(),
            n,
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
        transcript_label: ivm::host::LABEL_BATCH.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

#[test]
fn verify_batch_gated_by_max_k_returns_zeroes() {
    // Build two dummy envelopes with k=12 (n=4096), exceeding max_k gate=8
    let env = dummy_envelope(4096, h2::ZkCurveId::Pallas, "batch-k");
    let payload = norito::to_bytes(&vec![env.clone(), env]).expect("encode batch");
    let tlv = make_tlv_from_bytes(&payload);

    // Host with enabled=true but max_k too small
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
    // Overall OK; per-item statuses indicate gating (zeros)
    assert_eq!(vm.register(11), 0);
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![0, 0]);
}

#[test]
fn verify_batch_gated_by_enabled_flag_returns_disabled() {
    use iroha_zkp_halo2 as h2;

    // Build a small dummy envelope (k=8) to exercise the enabled gate
    let env = dummy_envelope(8, h2::ZkCurveId::Pallas, "batch-disabled");
    let payload = norito::to_bytes(&vec![env]).expect("encode batch");
    let tlv = make_tlv_from_bytes(&payload);

    let cfg = ZkHalo2Config {
        enabled: false, // disabled gate
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
    // Disabled gate: no output pointer; r11 indicates disabled
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 1);
}

#[test]
fn verify_batch_exceeds_max_batch_err_batch() {
    use iroha_zkp_halo2 as h2;

    // Build two dummy envelopes (k=8) so the host can enforce batch size
    let env = dummy_envelope(8, h2::ZkCurveId::Pallas, "batch-limit");
    let payload = norito::to_bytes(&vec![env.clone(), env]).expect("encode batch");
    let tlv = make_tlv_from_bytes(&payload);

    // Host gating: verifier_max_batch = 1, but we send 2 envelopes
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 50,
        verifier_max_batch: 1, // tighter than payload length
        ..ZkHalo2Config::default()
    };
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");
    // Expect overall ERR_BATCH (r11=7) and no output pointer
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 7);
}

#[test]
fn verify_batch_backend_mismatch_rejected() {
    use iroha_zkp_halo2 as h2;

    // Build a minimal envelope; backend gating should short-circuit
    let env = dummy_envelope(8, h2::ZkCurveId::Pallas, "backend-mis");
    let payload = norito::to_bytes(&vec![env]).expect("encode");
    let tlv = make_tlv_from_bytes(&payload);

    // Backend mismatch by setting to Unsupported
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Unsupported,
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
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 2, "ERR_BACKEND");
}

#[test]
fn verify_batch_curve_mismatch_item_zero() {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    // Build a valid ToyP61 envelope
    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..params.n() as u64)
        .map(|i| h2::PrimeField64::from(i + 1))
        .collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_BATCH);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(3u64);
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

    // Clone and set curve_id in params to Pasta to force a per-item curve mismatch
    let mut env_bad = env_ok.clone();
    env_bad.params.curve_id = h2::ZkCurveId::Pasta.as_u16();

    // Batch: [bad-curve, ok]
    let payload = norito::to_bytes(&vec![env_bad, env_ok]).expect("encode");
    let tlv = make_tlv_from_bytes(&payload);

    // Host configured for Toy curve
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

    // Overall OK; per-item statuses reflect curve gating: [0, 1]; first failing index = 0
    assert_eq!(vm.register(11), 0);
    assert_eq!(vm.register(12), 0);
    let out_ptr = vm.register(10);
    let out_tlv = vm.memory.validate_tlv(out_ptr).unwrap();
    let statuses: Vec<u8> = norito::decode_from_bytes(out_tlv.payload).expect("decode");
    assert_eq!(statuses, vec![0, 1]);
}
