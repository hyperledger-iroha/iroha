//! `DefaultHost` leaves `ZK_VERIFY_BATCH` to `CoreHost`.

use iroha_zkp_halo2 as h2;
use ivm::{
    IVMHost, PointerType, VMError,
    host::{self, DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

fn make_tlv(type_id: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(type_id as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

fn dummy_batch_payload() -> Vec<u8> {
    let env = h2::OpenVerifyEnvelope {
        params: h2::IpaParams {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 8,
            g: Vec::new(),
            h: Vec::new(),
            u: [0; 32],
        },
        public: h2::PolyOpenPublic {
            version: 1,
            curve_id: h2::ZkCurveId::Pallas.as_u16(),
            n: 8,
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
    };
    norito::to_bytes(&vec![env]).expect("encode batch payload")
}

#[test]
fn zk_verify_batch_syscall_is_disabled_in_default_host() {
    let payload = dummy_batch_payload();
    let tlv = make_tlv(PointerType::NoritoBytes, &payload);
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 18,
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

#[test]
fn zk_verify_batch_syscall_rejects_non_norito_pointer_before_disabled_status() {
    let payload = dummy_batch_payload();
    let tlv = make_tlv(PointerType::Blob, &payload);
    let mut vm = ivm::IVM::new(1_000_000);
    let mut host = DefaultHost::new();
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);

    let err = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_BATCH, &mut vm)
        .expect_err("non-NoritoBytes pointer must be rejected");
    assert_eq!(err, VMError::NoritoInvalid);
}
