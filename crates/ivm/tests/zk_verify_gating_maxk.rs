use ivm::{
    IVMHost,
    host::{DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

fn build_envelope_bytes(k: u32) -> Vec<u8> {
    use iroha_zkp_halo2::{IpaParams, IpaProofData, OpenVerifyEnvelope, PolyOpenPublic, ZkCurveId};

    // Build a minimal, self-consistent envelope that advertises vector length 2^k.
    let n = 1u32
        .checked_shl(k)
        .expect("k too large for vector length encoding");
    let curve_id = ZkCurveId::Pallas.as_u16();
    let zero = [0u8; 32];
    let env = OpenVerifyEnvelope {
        params: IpaParams {
            version: 1,
            curve_id,
            n,
            g: Vec::new(),
            h: Vec::new(),
            u: zero,
        },
        public: PolyOpenPublic {
            version: 1,
            curve_id,
            n,
            z: zero,
            t: zero,
            p_g: zero,
        },
        proof: IpaProofData {
            version: 1,
            l: Vec::new(),
            r: Vec::new(),
            a_final: zero,
            b_final: zero,
        },
        transcript_label: ivm::host::LABEL_TRANSFER.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode")
}

fn make_tlv(payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
    tlv.push(1u8);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

#[test]
fn verify_gated_by_max_k_returns_zero() {
    // Envelope with k=12 (n=4096)
    let payload = build_envelope_bytes(12);
    let tlv = make_tlv(&payload);
    let mut vm = ivm::IVM::new(1_000_000);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc");
    vm.set_register(10, ptr);
    // Host config with max_k=8, enabled=true, curve=Pallas, backend=Ipa
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 8,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    // Syscall should succeed (no error), but r10 must be 0 due to gating
    let _gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0, "verify must be gated by max_k");
    assert_eq!(vm.register(11), 4, "ERR_K expected when k exceeds limit");
}

#[test]
fn verify_gated_by_enabled_flag_returns_zero() {
    let payload = build_envelope_bytes(8);
    let tlv = make_tlv(&payload);
    let mut vm = ivm::IVM::new(1_000_000);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc");
    vm.set_register(10, ptr);
    let cfg = ZkHalo2Config {
        enabled: false,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let _ = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0, "verify must be disabled by config");
    assert_eq!(vm.register(11), 1, "ERR_DISABLED expected when gating");
}
