//! `DefaultHost` does not apply batch-verifier gates for `ZK_VERIFY_BATCH`.

use iroha_zkp_halo2 as h2;
use ivm::{
    IVMHost, PointerType,
    host::{self, DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

fn make_tlv(payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(PointerType::NoritoBytes as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

fn dummy_envelope(n: u32, curve: h2::ZkCurveId) -> h2::OpenVerifyEnvelope {
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
fn verify_batch_reports_disabled_before_batch_size_and_backend_gates() {
    let payload = norito::to_bytes(&vec![
        dummy_envelope(8, h2::ZkCurveId::Pallas),
        dummy_envelope(8, h2::ZkCurveId::Pallas),
    ])
    .expect("encode batch");
    let tlv = make_tlv(&payload);

    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Unsupported,
        max_k: 18,
        verifier_budget_ms: 50,
        verifier_max_batch: 1,
        ..ZkHalo2Config::default()
    };

    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");

    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), host::ERR_DISABLED);
}

#[test]
fn verify_batch_reports_disabled_before_curve_and_k_gates() {
    let payload =
        norito::to_bytes(&vec![dummy_envelope(4096, h2::ZkCurveId::Bn254)]).expect("encode batch");
    let tlv = make_tlv(&payload);

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
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect("syscall ok");

    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), host::ERR_DISABLED);
}
