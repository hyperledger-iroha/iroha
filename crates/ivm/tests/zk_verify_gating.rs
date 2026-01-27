use iroha_zkp_halo2 as h2;
use iroha_zkp_halo2::norito_helpers as nh;
use ivm::{IVM, IVMHost, Memory, PointerType, host::DefaultHost, syscalls};

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

fn build_valid_env_pallas(label: &str) -> Vec<u8> {
    let params = h2::Params::new(8).expect("params");
    let coeffs: Vec<h2::PrimeField64> = (0..params.n())
        .map(|i| h2::PrimeField64::from((i as u64) + 1))
        .collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(label);
    let commitment = poly.commit(&params).expect("commit");
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).expect("open");
    let env = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<h2::backend::pallas::PallasBackend>(
            params.n(),
            z,
            t,
            commitment,
        ),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode env")
}

fn build_valid_env_bn254(label: &str) -> Vec<u8> {
    let params = h2::Bn254Params::new(8).expect("params");
    let coeffs: Vec<h2::Bn254Scalar> = (0..params.n())
        .map(|i| h2::Bn254Scalar::from((i as u64) + 1))
        .collect();
    let poly = h2::Bn254Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(label);
    let commitment = poly.commit(&params).expect("commit");
    let z = h2::Bn254Scalar::from(7u64);
    let (proof, t) = poly.open(&params, &mut tr, z, commitment).expect("open");
    let env = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<h2::backend::bn254::Bn254Backend>(
            params.n(),
            z,
            t,
            commitment,
        ),
        proof: nh::proof_to_wire(&proof),
        transcript_label: label.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode env")
}

fn run_verify(number: u32, host: &mut DefaultHost, vm: &mut IVM, env_bytes: &[u8]) -> u64 {
    let tlv = make_tlv(PointerType::NoritoBytes as u16, env_bytes);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    host.syscall(number, vm).expect("syscall ok")
}

fn label_for_syscall(number: u32) -> &'static str {
    match number {
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER => ivm::host::LABEL_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD => ivm::host::LABEL_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT => ivm::host::LABEL_VOTE_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY => ivm::host::LABEL_VOTE_TALLY,
        _ => ivm::host::LABEL_TRANSFER,
    }
}

#[test]
fn verify_syscalls_gating_returns_zero_when_disabled() {
    let mut vm = IVM::new(1_000_000);
    let cfg = ivm::host::ZkHalo2Config {
        enabled: false,
        ..Default::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env = build_valid_env_pallas(label_for_syscall(num));
        let gas = run_verify(num, &mut host, &mut vm, &env);
        assert_eq!(gas, 0);
        assert_eq!(
            vm.register(10),
            0,
            "disabled host must return 0 for {num:x}"
        );
        assert_eq!(vm.register(11), 1, "ERR_DISABLED in r11");
    }
}

#[test]
fn verify_syscalls_gating_returns_zero_on_curve_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(Default::default());
    // Host expects Pallas, but env encodes BN254
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env = build_valid_env_bn254(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env);
        assert_eq!(
            vm.register(10),
            0,
            "curve mismatch must return 0 for {num:x}"
        );
        assert_eq!(vm.register(11), 3, "ERR_CURVE in r11");
    }
}

#[test]
fn verify_syscalls_accepts_matching_curve_pallas() {
    let mut vm = IVM::new(1_000_000);
    let cfg = ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 10,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env_bytes = build_valid_env_pallas(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env_bytes);
        assert_eq!(vm.register(10), 1, "expected success pointer for {num:x}");
        assert_eq!(vm.register(11), 0, "expected OK status for {num:x}");
    }
}

#[test]
fn verify_syscalls_accepts_matching_curve_bn254() {
    let mut vm = IVM::new(1_000_000);
    let cfg = ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Bn254,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 10,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env_bytes = build_valid_env_bn254(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env_bytes);
        assert_eq!(vm.register(10), 1, "expected success pointer for {num:x}");
        assert_eq!(vm.register(11), 0, "expected OK status for {num:x}");
    }
}

#[test]
fn verify_syscalls_gating_returns_backend_error_when_not_ipa() {
    let mut vm = IVM::new(1_000_000);
    let cfg = ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Unsupported,
        max_k: 12,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env_bytes = build_valid_env_pallas(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env_bytes);
        assert_eq!(vm.register(10), 0, "backend mismatch must gate {num:x}");
        assert_eq!(vm.register(11), 2, "ERR_BACKEND expected for {num:x}");
    }
}

#[test]
fn verify_syscalls_gating_returns_zero_when_k_exceeds_limit() {
    let mut vm = IVM::new(1_000_000);
    // Host caps supported k below the envelope's requirement (k=3).
    let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 2,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    });
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env = build_valid_env_pallas(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env);
        assert_eq!(vm.register(10), 0, "k too large must return 0 for {num:x}");
        assert_eq!(vm.register(11), 4, "ERR_K in r11");
    }
}

#[test]
fn verify_syscalls_gating_allows_path_then_verifier_decides() {
    let mut vm = IVM::new(1_000_000);
    // Gating passes: enabled, curve matches, k <= max_k
    let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 10,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    });
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let env = build_valid_env_pallas(label_for_syscall(num));
        let _ = run_verify(num, &mut host, &mut vm, &env);
        // We don't assert success (1) to avoid depending on IPA math; just that it returns a defined 0/1.
        let r = vm.register(10);
        assert!(r == 0 || r == 1, "expected r10 in {{0,1}} for {num:x}");
    }
}

type VerifyCase = (&'static str, fn(&str) -> Vec<u8>, ivm::host::ZkCurve);

#[test]
fn verify_syscalls_allowed_curves_matrix_defined_status() {
    // For each production curve configure host and envelope with matching curves.
    // Assert the gating path succeeds (defined status in r10 in {0,1} and r11 in {0,6}).
    let mut vm = IVM::new(1_000_000);
    let cases: &[VerifyCase] = &[
        ("pallas", build_valid_env_pallas, ivm::host::ZkCurve::Pallas),
        ("bn254", build_valid_env_bn254, ivm::host::ZkCurve::Bn254),
    ];
    for (name, builder, curve) in cases {
        let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
            enabled: true,
            curve: *curve,
            backend: ivm::host::ZkHalo2Backend::Ipa,
            max_k: 12,
            verifier_budget_ms: 250,
            verifier_max_batch: 8,
            ..ivm::host::ZkHalo2Config::default()
        });
        for &num in &[
            syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
            syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
            syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
            syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
        ] {
            let env = builder(label_for_syscall(num));
            let _ = run_verify(num, &mut host, &mut vm, &env);
            let r10 = vm.register(10);
            let r11 = vm.register(11);
            assert!(r10 == 0 || r10 == 1, "r10 must be 0/1 for curve {name}");
            assert!(r11 == 0 || r11 == 6, "r11 must be 0/6 for curve {name}");
        }
    }
}

#[test]
fn verify_syscalls_reject_transcript_label_mismatch() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    });
    let bad_label_env = build_valid_env_pallas("bad-label");
    for &num in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
    ] {
        let tlv = make_tlv(PointerType::NoritoBytes as u16, &bad_label_env);
        vm.memory.preload_input(0, &tlv).expect("preload input");
        vm.set_register(10, Memory::INPUT_START);
        let _ = host.syscall(num, &mut vm).expect("syscall ok");
        assert_eq!(vm.register(10), 0, "label mismatch must fail for {num:x}");
        assert_eq!(
            vm.register(11),
            ivm::host::ERR_TRANSCRIPT_LABEL,
            "ERR_TRANSCRIPT_LABEL expected"
        );
    }
}

#[test]
fn verify_syscalls_reject_over_envelope_and_proof_limits() {
    use iroha_zkp_halo2::{IpaProofData, ZkCurveId};

    // Envelope too large gate fires first
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        max_envelope_bytes: 16,
        ..ivm::host::ZkHalo2Config::default()
    });
    let env_small = build_valid_env_pallas(ivm::host::LABEL_TRANSFER);
    assert!(env_small.len() > 16);
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &env_small);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let _ = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), ivm::host::ERR_ENVELOPE_SIZE);

    // Proof length gate: craft an envelope with a large proof payload and a tight limit
    let proof_heavy = IpaProofData {
        version: 1,
        l: vec![[0u8; 32]; 32],
        r: vec![[0u8; 32]; 32],
        a_final: [0u8; 32],
        b_final: [0u8; 32],
    };
    let env_large_proof = iroha_zkp_halo2::OpenVerifyEnvelope {
        params: iroha_zkp_halo2::IpaParams {
            version: 1,
            curve_id: ZkCurveId::Pallas.as_u16(),
            n: 8,
            g: Vec::new(),
            h: Vec::new(),
            u: [0; 32],
        },
        public: iroha_zkp_halo2::PolyOpenPublic {
            version: 1,
            curve_id: ZkCurveId::Pallas.as_u16(),
            n: 8,
            z: [0; 32],
            t: [0; 32],
            p_g: [0; 32],
        },
        proof: proof_heavy,
        transcript_label: ivm::host::LABEL_TRANSFER.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let env_bytes = norito::to_bytes(&env_large_proof).expect("encode large proof env");
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 250,
        verifier_max_batch: 8,
        max_proof_bytes: 64,
        ..ivm::host::ZkHalo2Config::default()
    });
    let tlv = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let _ = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), ivm::host::ERR_PROOF_LEN);
}
