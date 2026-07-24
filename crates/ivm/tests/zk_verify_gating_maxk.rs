use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
use ivm::{
    IVMHost,
    host::{self, DefaultHost, ZkHalo2Backend, ZkHalo2Config},
    syscalls,
};

fn build_envelope_bytes() -> Vec<u8> {
    let envelope = OpenVerifyEnvelope::new(
        BackendTag::Halo2IpaPasta,
        host::LABEL_TRANSFER,
        [1; 32],
        vec![1, 2, 3],
        vec![4, 5, 6],
    );
    norito::to_bytes(&envelope).expect("encode canonical envelope")
}

fn make_tlv(payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
    tlv.push(1u8);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

#[test]
fn max_k_configuration_does_not_admit_without_verifier_registry() {
    let payload = build_envelope_bytes();
    let tlv = make_tlv(&payload);
    let mut vm = ivm::IVM::new(u64::MAX);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc");
    vm.set_register(10, ptr);
    let cfg = ZkHalo2Config {
        enabled: true,
        backend: ZkHalo2Backend::Ipa,
        max_k: 1,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let _gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), host::ERR_BACKEND);
}

#[test]
fn verify_gated_by_enabled_flag_returns_zero() {
    let payload = build_envelope_bytes();
    let tlv = make_tlv(&payload);
    let mut vm = ivm::IVM::new(u64::MAX);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc");
    vm.set_register(10, ptr);
    let cfg = ZkHalo2Config {
        enabled: false,
        backend: ZkHalo2Backend::Ipa,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let _ = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("syscall ok");
    assert_eq!(vm.register(10), 0, "verify must be disabled by config");
    assert_eq!(vm.register(11), host::ERR_DISABLED);
}
