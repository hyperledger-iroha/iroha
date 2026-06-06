//! `DefaultHost` implements standalone `ZK_VERIFY_BATCH` status-vector output.

use iroha_zkp_halo2 as h2;
use iroha_zkp_halo2::norito_helpers as nh;
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

fn valid_batch_envelope() -> h2::OpenVerifyEnvelope {
    let params = h2::Params::new(8).expect("params");
    let coeffs: Vec<h2::PrimeField64> = (0..params.n())
        .map(|i| h2::PrimeField64::from((i as u64) + 1))
        .collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut transcript = h2::Transcript::new(ivm::host::LABEL_BATCH);
    let commitment = poly.commit(&params).expect("commit");
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly
        .open(&params, &mut transcript, z, commitment)
        .expect("open");
    h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<h2::backend::pallas::PallasBackend>(
            params.n(),
            z,
            t,
            commitment,
        ),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_BATCH.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    }
}

fn batch_payload(envs: Vec<h2::OpenVerifyEnvelope>) -> Vec<u8> {
    norito::to_bytes(&envs).expect("encode batch payload")
}

fn decode_statuses(vm: &ivm::IVM) -> Vec<u8> {
    let output = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("batch output tlv");
    assert_eq!(output.type_id, PointerType::NoritoBytes);
    norito::decode_from_bytes(output.payload).expect("status vector")
}

#[test]
fn zk_verify_batch_syscall_returns_status_vector_in_default_host() {
    let env_ok = valid_batch_envelope();
    let mut env_bad = env_ok.clone();
    env_bad.public.t[0] = env_bad.public.t[0].wrapping_add(1);
    let payload = batch_payload(vec![env_ok, env_bad]);
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

    assert_ne!(vm.register(10), 0);
    assert_eq!(vm.register(11), host::ERR_VERIFY);
    assert_eq!(vm.register(12), 1);
    assert_eq!(decode_statuses(&vm), vec![1, 0]);
}

#[test]
fn zk_verify_batch_syscall_rejects_non_norito_pointer_before_disabled_status() {
    let payload = batch_payload(vec![valid_batch_envelope()]);
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
