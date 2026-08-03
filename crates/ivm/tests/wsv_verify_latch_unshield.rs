//! Canonical V1 Unshield dispatch through the operation-tagged A0 bridge.

use std::collections::HashMap;

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::{InstructionBox, zk::Unshield},
    prelude::{AccountId, AssetDefinitionId, DomainId, Mintable},
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
};
use iroha_primitives::numeric::Quantity;
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    host::{ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    mock_wsv::{MockWorldStateView, PermissionToken, WsvHost, ZkAssetMode, ZkPolicyConfig},
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

fn verify_gas(payload_len: usize) -> u64 {
    64_u64.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
}

fn mutation_gas(payload_len: usize) -> u64 {
    16_u64.saturating_add(u64::try_from(payload_len).unwrap_or(u64::MAX))
}

fn sample_account() -> AccountId {
    AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    )
}

fn setup(name: &str, vk_id: &VerifyingKeyId) -> (AccountId, AssetDefinitionId, WsvHost) {
    let caller = sample_account();
    let domain = DomainId::try_new("wonderland", "universal").expect("domain id");
    let asset = AssetDefinitionId::derive_from_components(
        domain.clone(),
        name.parse().expect("asset name"),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    wsv.insert_verifying_key(vk_id.clone(), vec![1, 2, 3, 4]);
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: Some(vk_id.clone()),
            vk_shield: None,
        }
    ));
    wsv.grant_permission(&caller, PermissionToken::Unshield(asset.clone()));
    let host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new()).with_zk_halo2_config(
        ZkHalo2Config {
            enabled: true,
            curve: ZkCurve::Pallas,
            backend: ZkHalo2Backend::Ipa,
            max_k: 18,
            verifier_budget_ms: 200,
            verifier_max_batch: 8,
            ..ZkHalo2Config::default()
        },
    );
    (caller, asset, host)
}

fn arm_unshield_latch(host: &mut WsvHost, vm: &mut IVM) {
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope::new(
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        ivm::host::LABEL_UNSHIELD,
        [0; 32],
        vec![1, 2, 3],
        vec![4, 5, 6],
    );
    let payload = encode_canonical_norito(&envelope).expect("canonical verify envelope");
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &payload))
        .expect("allocate verify envelope");
    vm.set_register(10, pointer);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_ZK_VERIFY_UNSHIELD, vm),
        Ok(verify_gas(payload.len()))
    );
    assert_eq!((vm.register(10), vm.register(11)), (1, 0));
}

fn unshield_payload(instruction: Unshield) -> Vec<u8> {
    encode_canonical_norito(&InstructionBox::from(instruction))
        .expect("canonical Unshield InstructionBox")
}

fn execute_unshield(host: &mut WsvHost, vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
    execute_unshield_with_tag(
        host,
        vm,
        payload,
        syscalls::SMARTCONTRACT_INSTRUCTION_TAG_UNSHIELD,
    )
}

fn execute_unshield_with_tag(
    host: &mut WsvHost,
    vm: &mut IVM,
    payload: &[u8],
    tag: u64,
) -> Result<u64, VMError> {
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, payload))
        .expect("allocate Unshield instruction");
    vm.set_register(10, pointer);
    vm.set_register(11, tag);
    host.syscall(syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION, vm)
}

fn proof(vk_id: VerifyingKeyId, byte: u8) -> ProofAttachment {
    ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![byte]),
        vk_id,
    )
}

#[test]
fn wsv_verify_latch_allows_unshield_then_resets() {
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_unshield_ref");
    let (caller, asset, mut host) = setup("rose", &vk_id);
    let mut vm = IVM::new(1_000_000);
    let payload = unshield_payload(Unshield::new(
        asset.clone(),
        caller.clone(),
        Quantity::from(1_u64),
        vec![[0; 32]],
        proof(vk_id, 0x01),
        None,
    ));

    assert_eq!(
        execute_unshield(&mut host, &mut vm, &payload),
        Err(VMError::PermissionDenied)
    );
    arm_unshield_latch(&mut host, &mut vm);
    assert_eq!(
        execute_unshield_with_tag(
            &mut host,
            &mut vm,
            &payload,
            syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
        ),
        Err(VMError::PermissionDenied),
        "a mismatched tag must reject before consuming the verify latch"
    );
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &payload),
        Ok(mutation_gas(payload.len()))
    );
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &payload),
        Err(VMError::PermissionDenied)
    );
    assert_eq!(host.wsv.balance(caller, asset), Quantity::from(1_u64));
}

#[test]
fn wsv_unshield_does_not_accept_or_append_guest_change_outputs() {
    let vk_id = VerifyingKeyId::new("halo2/ipa", "vk_unshield_output_free");
    let (caller, asset, mut host) = setup("violet", &vk_id);
    assert_eq!(host.wsv.drain_zk_events().len(), 1);
    let mut vm = IVM::new(u64::MAX);
    arm_unshield_latch(&mut host, &mut vm);

    let payload = unshield_payload(Unshield::new(
        asset.clone(),
        caller.clone(),
        Quantity::from(2_u64),
        vec![[5; 32]],
        proof(vk_id, 0x01),
        None,
    ));
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &payload),
        Ok(mutation_gas(payload.len()))
    );

    let (latest_root, roots, depth) = host.wsv.get_roots(&asset, 8);
    let empty_root = iroha_data_model::zk::CONFIDENTIAL_TREE_POSEIDON_PASTA_V1_EMPTY_ROOT;
    assert_eq!(latest_root, empty_root);
    assert_eq!(
        hex::encode(latest_root),
        "ce4066b230f348190183f90dd35871c13823a358bb37c2ce8b43526ae7197c3c"
    );
    assert_eq!(depth, 0);
    assert_eq!(roots, vec![empty_root]);
    let events = host.wsv.drain_zk_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        ivm::mock_wsv::ZkEvent::Unshielded {
            asset: event_asset,
            to,
            public_amount,
        } if event_asset == &asset
            && to == &caller
            && public_amount == &Quantity::from(2_u64)
    ));
}

#[test]
fn unshield_rejects_mismatched_verifying_key() {
    let expected = VerifyingKeyId::new("halo2/ipa", "vk_expected");
    let (caller, asset, mut host) = setup("iris", &expected);
    let other = VerifyingKeyId::new("halo2/ipa", "vk_other");
    host.wsv
        .insert_verifying_key(other.clone(), vec![9, 8, 7, 6]);
    let mut vm = IVM::new(u64::MAX);
    arm_unshield_latch(&mut host, &mut vm);

    let payload = unshield_payload(Unshield::new(
        asset,
        caller,
        Quantity::from(1_u64),
        vec![[0; 32]],
        proof(other, 0xAA),
        None,
    ));
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &payload),
        Err(VMError::PermissionDenied)
    );
}

#[test]
fn unshield_accepts_registered_key_then_rejects_unknown_key() {
    let expected = VerifyingKeyId::new("halo2/ipa", "vk_unshield_ok");
    let (caller, asset, mut host) = setup("daisy", &expected);
    let mut vm = IVM::new(u64::MAX);
    arm_unshield_latch(&mut host, &mut vm);

    let good = unshield_payload(Unshield::new(
        asset.clone(),
        caller.clone(),
        Quantity::from(1_u64),
        vec![[0; 32]],
        proof(expected, 0x10),
        None,
    ));
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &good),
        Ok(mutation_gas(good.len()))
    );

    arm_unshield_latch(&mut host, &mut vm);
    let bad = unshield_payload(Unshield::new(
        asset,
        caller,
        Quantity::from(1_u64),
        vec![[1; 32]],
        proof(
            VerifyingKeyId::new("halo2/ipa", "vk_unshield_mismatch"),
            0x11,
        ),
        None,
    ));
    assert_eq!(
        execute_unshield(&mut host, &mut vm, &bad),
        Err(VMError::PermissionDenied)
    );
}
