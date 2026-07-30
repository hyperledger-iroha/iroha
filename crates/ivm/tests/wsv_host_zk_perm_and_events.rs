use std::collections::HashMap;

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    isi::{InstructionBox, zk as dm_zk},
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
};
use iroha_primitives::numeric::Quantity;
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    mock_wsv::{
        AccountId, AssetDefinitionId, DomainId, Mintable, MockWorldStateView, PermissionToken,
        WsvHost, ZkAssetMode, ZkPolicyConfig,
    },
    syscalls,
};
use ivm_abi::codec::encode_canonical_norito;

fn make_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let hash: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&hash);
    out
}

fn account(public_key: &str) -> AccountId {
    let public_key: PublicKey = public_key.parse().expect("public key");
    AccountId::new(public_key)
}

fn setup_asset(name: &str) -> (AccountId, AssetDefinitionId, MockWorldStateView) {
    let caller = account("ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774");
    let domain = DomainId::try_new("domain", "universal").expect("domain id");
    let asset = AssetDefinitionId::new(domain.clone(), name.parse().expect("asset name"));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    (caller, asset, wsv)
}

fn boxed_payload(instruction: impl Into<InstructionBox>) -> Vec<u8> {
    encode_canonical_norito(&instruction.into()).expect("canonical InstructionBox")
}

#[test]
fn direct_zk_register_and_shield_setup_emit_events() {
    let (caller, asset, mut wsv) = setup_asset("rose");
    wsv.grant_permission(&caller, PermissionToken::MintAsset(asset.clone()));
    assert!(wsv.mint(
        &caller,
        caller.clone(),
        asset.clone(),
        Quantity::from(10_u64)
    ));
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: None,
            vk_shield: None,
        }
    ));
    assert!(wsv.shield(&caller, &asset, Quantity::from(3_u64), [7; 32]));

    let events = wsv.drain_zk_events();
    assert!(events.iter().any(
        |event| matches!(event, ivm::mock_wsv::ZkEvent::ZkPolicyUpdated { asset: id, .. } if id == &asset)
    ));
    assert!(events.iter().any(
        |event| matches!(event, ivm::mock_wsv::ZkEvent::CommitmentAdded { asset: id, .. } if id == &asset)
    ));
}

#[test]
fn canonical_unshield_requires_verify_even_with_permission() {
    let (caller, asset, mut wsv) = setup_asset("gold");
    wsv.grant_permission(&caller, PermissionToken::Unshield(asset.clone()));
    let mut host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);

    let instruction = dm_zk::Unshield::new(
        asset,
        caller,
        Quantity::from(1_u64),
        vec![[1; 32]],
        ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0x01]),
            VerifyingKeyId::new("halo2/ipa", "unshield_vk"),
        ),
        None,
    );
    let payload = boxed_payload(instruction);
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &payload))
        .expect("allocate instruction");
    vm.set_register(10, pointer);
    vm.set_register(11, syscalls::SMARTCONTRACT_INSTRUCTION_TAG_UNSHIELD);

    assert_eq!(
        host.syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
        Err(VMError::PermissionDenied)
    );
}

#[test]
fn zk_transfer_uses_direct_wsv_api_and_is_never_authorized_by_a0_tags() {
    let (caller, asset, mut wsv) = setup_asset("lily");
    let expected_vk = VerifyingKeyId::new("halo2/ipa", "vk_transfer_ref");
    let other_vk = VerifyingKeyId::new("halo2/ipa", "vk_other_ref");
    wsv.insert_verifying_key(expected_vk.clone(), vec![7; 4]);
    wsv.insert_verifying_key(other_vk.clone(), vec![9; 4]);
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: Some(expected_vk.clone()),
            vk_unshield: None,
            vk_shield: None,
        }
    ));

    let matching_proof = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![0xAA]),
        expected_vk,
    );
    let other_proof = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![0xBB]),
        other_vk,
    );
    assert!(!wsv.zk_transfer(&asset, &[[1; 32]], &[[2; 32]], &other_proof));
    assert!(wsv.zk_transfer(&asset, &[[1; 32]], &[[2; 32]], &matching_proof));

    let mut host = WsvHost::new_with_subject(wsv, caller, HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    let verify = iroha_data_model::zk::OpenVerifyEnvelope::new(
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        ivm::host::LABEL_TRANSFER,
        [0; 32],
        vec![1, 2, 3],
        vec![4, 5, 6],
    );
    let verify_payload = encode_canonical_norito(&verify).expect("canonical verify envelope");
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &verify_payload))
        .expect("allocate verify envelope");
    vm.set_register(10, pointer);
    host.syscall(syscalls::SYSCALL_ZK_VERIFY_TRANSFER, &mut vm)
        .expect("arm transfer latch");
    assert_eq!((vm.register(10), vm.register(11)), (1, 0));

    let transfer =
        dm_zk::ZkTransfer::new(asset, vec![[3; 32]], vec![[4; 32]], matching_proof, None);
    let payload = boxed_payload(transfer);
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &payload))
        .expect("allocate transfer instruction");
    vm.set_register(10, pointer);
    for tag in [
        syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
        syscalls::SMARTCONTRACT_INSTRUCTION_TAG_UNSHIELD,
        syscalls::SMARTCONTRACT_INSTRUCTION_TAG_RECORD_SCCP_MESSAGE,
    ] {
        vm.set_register(11, tag);
        assert_eq!(
            host.syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, &mut vm),
            Err(VMError::PermissionDenied),
            "tag {tag} must not authorize ZkTransfer"
        );
    }
}
