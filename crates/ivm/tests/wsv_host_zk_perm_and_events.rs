use std::collections::HashMap;

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::proof::VerifyingKeyId;
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, IVMHost, Memory, PointerType,
    mock_wsv::{
        AccountId, AssetDefinitionId, DomainId, MockWorldStateView, PermissionToken, WsvHost,
    },
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
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn account(domain: &str, public_key: &str) -> AccountId {
    let _domain = DomainId::try_new(domain, "universal").unwrap();
    let public_key: PublicKey = public_key.parse().unwrap();
    AccountId::new(public_key)
}

fn canonical_account(account: AccountId) -> AccountId {
    let value = norito::json::to_value(&account).expect("serialize account");
    let literal = value.as_str().expect("account literal");
    AccountId::parse_encoded(literal)
        .expect("canonical I105 account id must parse")
        .into_account_id()
}

fn execute_json_instruction(vm: &mut IVM, env: norito::json::Value, offset: u64, label: &str) {
    let bytes = norito::json::to_vec(&env).expect("serialize envelope");
    let tlv = make_tlv(PointerType::Json as u16, &bytes);
    vm.memory
        .preload_input(offset, &tlv)
        .expect("preload json envelope");
    vm.set_register(10, Memory::INPUT_START + offset);

    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());

    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&code);
    vm.load_program(&program).expect("load program");
    vm.run().expect(label);
}

fn build_open_verify_envelope_bytes() -> Vec<u8> {
    let env = iroha_data_model::zk::OpenVerifyEnvelope::new(
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        ivm::host::LABEL_TRANSFER,
        [0u8; 32],
        vec![1, 2, 3],
        vec![4, 5, 6],
    );
    norito::to_bytes(&env).expect("encode env")
}

#[test]
fn zk_register_shield_permissions_and_events() {
    // Setup caller and WSV with an asset definition and balance
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let domain: DomainId = DomainId::try_new("domain", "universal").unwrap();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, domain));
    wsv.grant_permission(&alice, PermissionToken::RegisterAssetDefinition);
    let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("domain", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    // Seed asset def and balance
    assert!(wsv.register_asset_definition(&alice, ad.clone(), ivm::mock_wsv::Mintable::Infinitely));
    // Mint to alice using direct WSV primitive for setup
    wsv.grant_permission(&alice, PermissionToken::MintAsset(ad.clone()));
    assert!(wsv.mint(&alice, alice.clone(), ad.clone(), Numeric::from(10_u64)));

    // Grant ZK-related permissions
    wsv.grant_permission(&alice, PermissionToken::RegisterZkAsset(ad.clone()));
    wsv.grant_permission(&alice, PermissionToken::Shield(ad.clone()));
    wsv.grant_permission(&alice, PermissionToken::Unshield(ad.clone()));

    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // 1) Register ZK asset policy via the JSON envelope path exercised by Torii/Core.
    let register_payload = json_object([
        ("asset", json_value(&ad.to_string())),
        ("mode", json_value("Hybrid")),
        ("allow_shield", json_value(&true)),
        ("allow_unshield", json_value(&true)),
        ("vk_transfer", json_value(&Option::<VerifyingKeyId>::None)),
        ("vk_unshield", json_value(&Option::<VerifyingKeyId>::None)),
        ("vk_shield", json_value(&Option::<VerifyingKeyId>::None)),
    ]);
    let register_env = json_object([
        ("type", json_value("zk.RegisterZkAsset")),
        ("payload", register_payload),
    ]);
    execute_json_instruction(&mut vm, register_env, 0, "register zk asset");

    // 2) Shield 3 units: build Shield instruction and execute
    let note_commitment: norito::json::Value =
        norito::json::Value::Array((0..32).map(|_| norito::json::Value::from(7u64)).collect());
    let zero_bytes = |len: usize| {
        norito::json::Value::Array((0..len).map(|_| norito::json::Value::from(0u64)).collect())
    };
    let enc_payload = json_object([
        (
            "version",
            json_value(&iroha_data_model::confidential::CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1),
        ),
        ("ephemeral_pubkey", zero_bytes(32)),
        ("nonce", zero_bytes(24)),
        ("ciphertext", json_value(&"")),
    ]);
    let shield_payload = json_object([
        ("asset", json_value(&ad.to_string())),
        ("from", json_value(&alice)),
        ("amount", json_value(&3u64)),
        ("note_commitment", note_commitment.clone()),
        ("enc_payload", enc_payload),
    ]);
    let shield_env = json_object([
        ("type", json_value("zk.Shield")),
        ("payload", shield_payload),
    ]);
    execute_json_instruction(&mut vm, shield_env, 64, "shield");

    // Inspect events and state via host downcast
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_mut::<WsvHost>().unwrap();
    let events = host.wsv.drain_zk_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ivm::mock_wsv::ZkEvent::ZkPolicyUpdated { .. }))
    );
    assert!(events.iter().any(
        |e| matches!(e, ivm::mock_wsv::ZkEvent::CommitmentAdded { asset, .. } if asset == &ad)
    ));
}

#[test]
fn unshield_requires_verify_even_with_permission() {
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let domain: DomainId = DomainId::try_new("domain", "universal").unwrap();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, domain));
    wsv.grant_permission(&alice, PermissionToken::RegisterAssetDefinition);
    let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("domain", "universal").unwrap(),
        "gold".parse().unwrap(),
    );
    assert!(wsv.register_asset_definition(&alice, ad.clone(), ivm::mock_wsv::Mintable::Infinitely));
    // Grant permissions including Unshield
    wsv.grant_permission(&alice, PermissionToken::Unshield(ad.clone()));
    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Build Unshield instruction envelope and attempt without prior ZK_VERIFY_UNSHIELD
    use iroha_data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyBox};
    let proof = ProofBox::new("halo2/ipa".into(), Vec::new());
    let vk = VerifyingKeyBox::new("halo2/ipa".into(), Vec::new());
    let attach = ProofAttachment::new_inline("halo2/ipa".into(), proof, vk);
    let attach_val = norito::json::to_value(&attach).expect("attach to json");
    let inputs_val: norito::json::Value =
        norito::json::Value::Array(vec![norito::json::Value::Array(
            (0..32).map(|_| norito::json::Value::from(1u64)).collect(),
        )]);
    let unshield_env = json_object([
        ("type", json_value("zk.Unshield")),
        (
            "payload",
            json_object([
                ("asset", json_value(&ad.to_string())),
                ("to", json_value(&alice)),
                ("public_amount", json_value(&1u64)),
                ("inputs", inputs_val),
                ("proof", attach_val),
                ("root_hint", norito::json::Value::Null),
            ]),
        ),
    ]);
    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&unshield_env).unwrap(),
    );
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ivm::ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();
    // Expect PermissionDenied due to missing verify
    let res = vm.run();
    assert!(matches!(res, Err(ivm::VMError::PermissionDenied)));
}

#[test]
fn zk_transfer_requires_matching_vk_reference() {
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let domain: DomainId = DomainId::try_new("domain", "universal").unwrap();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, domain));
    wsv.grant_permission(&alice, PermissionToken::RegisterAssetDefinition);
    let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("domain", "universal").unwrap(),
        "lily".parse().unwrap(),
    );
    assert!(wsv.register_asset_definition(&alice, ad.clone(), ivm::mock_wsv::Mintable::Infinitely));
    let vk_transfer = VerifyingKeyId::new("halo2/ipa", "vk_transfer_ref");
    let vk_other = VerifyingKeyId::new("halo2/ipa", "vk_other_ref");
    wsv.insert_verifying_key(vk_transfer.clone(), vec![7u8; 4]);
    wsv.insert_verifying_key(vk_other.clone(), vec![9u8; 4]);
    wsv.grant_permission(&alice, PermissionToken::RegisterZkAsset(ad.clone()));
    assert!(wsv.register_zk_asset(
        ad.clone(),
        ivm::mock_wsv::ZkPolicyConfig {
            mode: ivm::mock_wsv::ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: Some(vk_transfer.clone()),
            vk_unshield: None,
            vk_shield: None,
        }
    ));

    let mut host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);

    let proof_ref = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xAA]),
        vk_transfer.clone(),
    );
    let proof_other = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xBB]),
        vk_other.clone(),
    );
    let proof_missing = iroha_data_model::proof::ProofAttachment {
        backend: "halo2/ipa".into(),
        proof: iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xCC]),
        vk_ref: None,
        vk_inline: None,
        vk_commitment: None,
        envelope_hash: None,
        lane_privacy: None,
    };
    let proof_inline = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xDD]),
        iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![7u8; 4]),
    );
    let proof_inline_bad_backend = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xEE]),
        iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![8u8; 4]),
    );
    let proof_json_ref = norito::json::to_value(&proof_ref).expect("proof to json");
    let proof_json_other = norito::json::to_value(&proof_other).expect("proof to json");
    let proof_json_inline = norito::json::to_value(&proof_inline).expect("proof to json");
    let proof_json_inline_bad =
        norito::json::to_value(&proof_inline_bad_backend).expect("proof to json");
    let proof_json_missing = norito::json::to_value(&proof_missing).expect("proof to json");

    let mk_inputs = |seed: u64| {
        norito::json::Value::Array(vec![norito::json::Value::Array(
            (0..32)
                .map(|i| norito::json::Value::from(seed + i as u64))
                .collect(),
        )])
    };
    let outputs_val = norito::json::Value::Array(vec![
        norito::json::Value::Array((0..32).map(|_| norito::json::Value::from(5u64)).collect()),
        norito::json::Value::Array((0..32).map(|_| norito::json::Value::from(6u64)).collect()),
    ]);
    let mk_payload = |proof: norito::json::Value, seed: u64| {
        json_object([
            ("asset", json_value(&ad.to_string())),
            ("inputs", mk_inputs(seed)),
            ("outputs", outputs_val.clone()),
            ("proof", proof),
            ("root_hint", norito::json::Value::Null),
        ])
    };
    let transfer_env_ref = json_object([
        ("type", json_value("zk.ZkTransfer")),
        ("payload", mk_payload(proof_json_ref.clone(), 0)),
    ]);
    let transfer_env_other = json_object([
        ("type", json_value("zk.ZkTransfer")),
        ("payload", mk_payload(proof_json_other.clone(), 32)),
    ]);
    let transfer_env_missing = json_object([
        ("type", json_value("zk.ZkTransfer")),
        ("payload", mk_payload(proof_json_missing.clone(), 64)),
    ]);
    let transfer_env_inline = json_object([
        ("type", json_value("zk.ZkTransfer")),
        ("payload", mk_payload(proof_json_inline.clone(), 96)),
    ]);
    let transfer_env_inline_bad = json_object([
        ("type", json_value("zk.ZkTransfer")),
        ("payload", mk_payload(proof_json_inline_bad.clone(), 128)),
    ]);

    // Without verify latch, execution fails
    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_ref).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("latch missing");
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Verify transfer envelope to arm latch
    let env_bytes = build_open_verify_envelope_bytes();
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    let ptr_env = vm.alloc_input_tlv(&tlv_env).unwrap();
    let env_offset = ptr_env - Memory::INPUT_START;
    vm.set_register(10, ptr_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify");
    assert_eq!(gas, 0);
    assert_eq!((vm.register(10), vm.register(11)), (1, 0));

    // With matching vk_ref transfer succeeds
    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_ref).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let gas = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("transfer with matching vk");
    assert_eq!(gas, 0);

    // Re-arm latch
    let env_bytes = build_open_verify_envelope_bytes();
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    vm.memory
        .preload_input(env_offset, &tlv_env)
        .expect("preload verify env");
    vm.set_register(10, ptr_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify");
    assert_eq!(gas, 0);
    assert_eq!((vm.register(10), vm.register(11)), (1, 0));

    // Mismatched vk_ref is rejected
    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_other).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("vk mismatch must fail");
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Re-arm latch again and reject proofs missing any verifying key material
    let env_bytes = build_open_verify_envelope_bytes();
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    vm.memory
        .preload_input(env_offset, &tlv_env)
        .expect("preload verify env");
    vm.set_register(10, ptr_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 1);

    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_missing).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("missing verifying key must fail");
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Re-arm and allow inline verifying key with matching backend
    let env_bytes = build_open_verify_envelope_bytes();
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    vm.memory
        .preload_input(env_offset, &tlv_env)
        .expect("preload verify env");
    vm.set_register(10, ptr_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 1);

    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_inline).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let gas = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect("inline vk ok");
    assert_eq!(gas, 0);

    // Inline with mismatched backend must be rejected
    let env_bytes = build_open_verify_envelope_bytes();
    let tlv_env = make_tlv(PointerType::NoritoBytes as u16, &env_bytes);
    vm.memory
        .preload_input(env_offset, &tlv_env)
        .expect("preload verify env");
    vm.set_register(10, ptr_env);
    let gas = host
        .syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("verify");
    assert_eq!(gas, 0);
    assert_eq!(vm.register(10), 1);

    let tlv = make_tlv(
        PointerType::Json as u16,
        &norito::json::to_vec(&transfer_env_inline_bad).unwrap(),
    );
    let ptr = vm.alloc_input_tlv(&tlv).unwrap();
    vm.set_register(10, ptr);
    let err = host
        .syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm)
        .expect_err("inline backend mismatch must fail");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}
