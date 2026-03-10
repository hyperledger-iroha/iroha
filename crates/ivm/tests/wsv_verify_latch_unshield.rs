//! End-to-end: preload a real Norito OpenVerifyEnvelope TLV, run a ZK
//! verify syscall on the mock WSV host to set the latch, then enqueue an
//! Unshield instruction via the vendor bridge and assert it applies.

use std::collections::HashMap;

use iroha_data_model::{
    prelude::{Mintable, *},
    proof::VerifyingKeyId,
};
use ivm::{
    IVM, IVMHost, PointerType,
    host::{ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    mock_wsv::{MockWorldStateView, PermissionToken, WsvHost, ZkAssetMode, ZkPolicyConfig},
    syscalls,
};
mod common;

fn json_value<T: norito::json::JsonSerialize + ?Sized>(value: &T) -> norito::json::Value {
    norito::json::to_value(value).expect("serialize json value")
}

fn json_object<const N: usize>(
    pairs: [(&'static str, norito::json::Value); N],
) -> norito::json::Value {
    norito::json::object(pairs).expect("serialize json object")
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn sample_account() -> AccountId {
    AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    )
}

fn build_open_verify_envelope_bytes(k: u32) -> Vec<u8> {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    use iroha_zkp_halo2::backend::pallas::PallasBackend;
    let params = h2::Params::new(k as usize).expect("params");
    // Build a trivial polynomial and opening proof
    let coeffs: Vec<h2::PrimeField64> = vec![0u64.into(); params.n()];
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_UNSHIELD);
    let p_g = poly.commit(&params).expect("commit");
    let z = h2::PrimeField64::from(1u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
    let env = h2::OpenVerifyEnvelope {
        params: nh::params_to_wire(&params),
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_UNSHIELD.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode env")
}

#[test]
fn wsv_verify_latch_allows_unshield_then_resets() {
    // Prepare caller and asset
    let caller: AccountId = sample_account();
    let asset: AssetDefinitionId = "rose#wonderland".parse().unwrap();

    // Seed WSV with caller account and permissions; register asset and enable ZK policy
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    let domain: DomainId = "wonderland".parse().unwrap();
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    let vk_unshield = VerifyingKeyId::new("halo2/ipa", "vk_unshield_ref");
    wsv.insert_verifying_key(vk_unshield.clone(), vec![1, 2, 3, 4]);
    wsv.grant_permission(&caller, PermissionToken::RegisterZkAsset(asset.clone()));
    let ok = wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: Some(vk_unshield.clone()),
            vk_shield: None,
        },
    );
    assert!(ok, "register_zk_asset must succeed");
    // Unshield permission and read-balance permission for later query
    wsv.grant_permission(&caller, PermissionToken::Unshield(asset.clone()));
    assert!(wsv.has_permission(&caller, &PermissionToken::Unshield(asset.clone())));
    wsv.grant_permission(
        &caller,
        PermissionToken::ReadAccountAssets(ivm::mock_wsv::AccountId::from(&caller)),
    );

    // Host with ZK (Halo2 IPA) enabled and sufficient max_k
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 200,
        verifier_max_batch: 8,
        ..ZkHalo2Config::default()
    };
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    )
    .with_zk_halo2_config(cfg);

    // VM
    let mut vm = IVM::new(1_000_000);
    let mut host = host;

    // Build an Unshield instruction (encode as InstructionBox Norito bytes)
    let proof_attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x01]),
        vk_unshield.clone(),
    );
    let proof_json = norito::json::to_value(&proof_attachment).expect("proof json");
    let inputs_json: norito::json::Value =
        norito::json::Value::Array(vec![norito::json::Value::Array(
            (0..32).map(|_| norito::json::Value::from(0u64)).collect(),
        )]);
    let make_unshield_envelope = || {
        json_object([
            ("type", json_value("zk.Unshield")),
            (
                "payload",
                json_object([
                    ("asset", json_value(&asset.to_string())),
                    ("to", json_value(&caller)),
                    ("public_amount", json_value(&1u64)),
                    ("inputs", inputs_json.clone()),
                    ("proof", proof_json.clone()),
                    ("root_hint", norito::json::Value::Null),
                ]),
            ),
        ])
    };
    let unshield_bytes = norito::json::to_vec(&make_unshield_envelope()).expect("encode json");

    // Negative: without verify latch, vendor execute must be denied
    let tlv = make_tlv(PointerType::Json as u16, &unshield_bytes);
    let p = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, p);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("unshield must be gated without verify");
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Verify: preload a real OpenVerifyEnvelope and call verify syscall
    let env_bytes = build_open_verify_envelope_bytes(8);
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    let p_env = vm.alloc_input_tlv(&tlv_env).unwrap();
    vm.set_register(10, p_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD, &mut vm)
        .expect("verify syscall ok");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 1, "verify must succeed");

    // Positive: unshield now succeeds thanks to the latch
    let tlv2 = make_tlv(PointerType::Json as u16, &unshield_bytes);
    let p2 = vm.alloc_input_tlv(&tlv2).unwrap();
    vm.set_register(10, p2);
    let gas2 = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("unshield after verify");
    assert_eq!(gas2, 0);

    // One-shot latch: a subsequent unshield without a fresh verify is denied
    let tlv3 = make_tlv(PointerType::Json as u16, &unshield_bytes);
    let p3 = vm.alloc_input_tlv(&tlv3).unwrap();
    vm.set_register(10, p3);
    let err2 = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("latch is one-shot; second unshield must require verify");
    assert!(matches!(err2, ivm::VMError::PermissionDenied));

    // Query balance via JSON vendor query to ensure it increased
    let q = json_object([
        ("type", json_value("wsv.get_balance")),
        (
            "payload",
            json_object([
                ("account_id", json_value(&caller)),
                ("asset_id", json_value(&asset.to_string())),
            ]),
        ),
    ]);
    let q_bytes = norito::json::to_vec(&q).unwrap();
    let tlv_q = make_tlv(PointerType::Json as u16, &q_bytes);
    let p_q = vm.alloc_input_tlv(&tlv_q).unwrap();
    vm.set_register(10, p_q);
    let _ = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY, &mut vm)
        .expect("query ok");
    let p_out = vm.register(10);
    let tlv_out = vm.memory.validate_tlv(p_out).expect("out tlv");
    assert_eq!(tlv_out.type_id, PointerType::Json);
    let val: norito::json::Value = common::json_from_payload(tlv_out.payload);
    let bal: iroha_primitives::numeric::Numeric = val
        .get("balance")
        .and_then(|v| v.as_str())
        .expect("balance present")
        .parse()
        .expect("parse numeric balance");
    assert_eq!(bal, iroha_primitives::numeric::Numeric::from(1u64));
}

#[test]
fn unshield_rejects_mismatched_verifying_key() {
    let caller: AccountId = sample_account();
    let asset: AssetDefinitionId = "iris#wonderland".parse().unwrap();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    let domain: DomainId = "wonderland".parse().unwrap();
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    let vk_expected = VerifyingKeyId::new("halo2/ipa", "vk_expected");
    let vk_other = VerifyingKeyId::new("halo2/ipa", "vk_other");
    wsv.insert_verifying_key(vk_expected.clone(), vec![0x10, 0x20, 0x30, 0x40]);
    wsv.insert_verifying_key(vk_other.clone(), vec![0x99, 0x88, 0x77, 0x66]);
    wsv.grant_permission(&caller, PermissionToken::RegisterZkAsset(asset.clone()));
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: Some(vk_expected.clone()),
            vk_shield: None,
        },
    ));
    wsv.grant_permission(&caller, PermissionToken::Unshield(asset.clone()));
    let mut host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);

    // Successful verify latch
    let env_bytes = build_open_verify_envelope_bytes(8);
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    let ptr_env = vm.alloc_input_tlv(&tlv_env).unwrap();
    vm.set_register(10, ptr_env);
    {
        let gas = host
            .syscall(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD, &mut vm)
            .expect("verify latch");
        assert_eq!(gas, 0);
        assert_eq!(vm.register(10), 1);
    }

    let proof = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xAA]),
        vk_other,
    );
    let env = json_object([
        ("type", json_value("zk.Unshield")),
        (
            "payload",
            json_object([
                ("asset", json_value(&asset.to_string())),
                ("to", json_value(&caller)),
                ("public_amount", json_value(&1u64)),
                ("inputs", json_value(&vec![vec![0u64; 32]])),
                ("proof", norito::json::to_value(&proof).unwrap()),
                ("root_hint", norito::json::Value::Null),
            ]),
        ),
    ]);
    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&env).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("unshield must reject mismatched vk");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}

#[test]
fn unshield_accepts_and_checks_inline_verifying_key() {
    let caller: AccountId = sample_account();
    let asset: AssetDefinitionId = "daisy#wonderland".parse().unwrap();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    let domain: DomainId = "wonderland".parse().unwrap();
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_inline_ok");
    let vk_bytes_good = vec![0x11, 0x22, 0x33, 0x44];
    wsv.insert_verifying_key(vk_id.clone(), vk_bytes_good.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterZkAsset(asset.clone()));
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: Some(vk_id.clone()),
            vk_shield: None,
        },
    ));
    wsv.grant_permission(&caller, PermissionToken::Unshield(asset.clone()));
    wsv.grant_permission(
        &caller,
        PermissionToken::ReadAccountAssets(ivm::mock_wsv::AccountId::from(&caller)),
    );
    let mut host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);

    let verify_env = build_open_verify_envelope_bytes(8);
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &verify_env);
    let ptr_env = vm.alloc_input_tlv(&tlv_env).unwrap();
    vm.set_register(10, ptr_env);
    {
        let gas = host
            .syscall(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD, &mut vm)
            .expect("verify latch");
        assert_eq!(gas, 0);
        assert_eq!(vm.register(10), 1);
    }

    let good_attachment = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x10]),
        iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vk_bytes_good.clone()),
    );
    let env_good = json_object([
        ("type", json_value("zk.Unshield")),
        (
            "payload",
            json_object([
                ("asset", json_value(&asset.to_string())),
                ("to", json_value(&caller)),
                ("public_amount", json_value(&1u64)),
                ("inputs", json_value(&vec![vec![0u64; 32]])),
                ("proof", norito::json::to_value(&good_attachment).unwrap()),
                ("root_hint", norito::json::Value::Null),
            ]),
        ),
    ]);
    let tlv_good = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&env_good).unwrap(),
    );
    let ptr_good = vm.alloc_input_tlv(&tlv_good).unwrap();
    vm.set_register(10, ptr_good);
    {
        let gas = host
            .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
            .expect("inline vk should succeed");
        assert_eq!(gas, 0);
    }

    // Latch consumed; re-arm before testing mismatch
    let verify_env = build_open_verify_envelope_bytes(8);
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &verify_env);
    let ptr_env = vm.alloc_input_tlv(&tlv_env).unwrap();
    vm.set_register(10, ptr_env);
    {
        let gas = host
            .syscall(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD, &mut vm)
            .expect("verify latch");
        assert_eq!(gas, 0);
        assert_eq!(vm.register(10), 1);
    }

    let bad_attachment = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x11]),
        iroha_data_model::proof::VerifyingKeyBox::new(
            "halo2/ipa".into(),
            vec![0xDE, 0xAD, 0xBE, 0xEF],
        ),
    );
    let env_bad = json_object([
        ("type", json_value("zk.Unshield")),
        (
            "payload",
            json_object([
                ("asset", json_value(&asset.to_string())),
                ("to", json_value(&caller)),
                ("public_amount", json_value(&1u64)),
                ("inputs", json_value(&vec![vec![0u64; 32]])),
                ("proof", norito::json::to_value(&bad_attachment).unwrap()),
                ("root_hint", norito::json::Value::Null),
            ]),
        ),
    ]);
    let tlv_bad = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&env_bad).unwrap(),
    );
    let ptr_bad = vm.alloc_input_tlv(&tlv_bad).unwrap();
    vm.set_register(10, ptr_bad);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("inline mismatch must fail");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}
