use std::collections::HashMap;
use std::str::FromStr;

use iroha_crypto::{Hash, PublicKey};
use iroha_primitives::json::Json;
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, PointerType, VMError,
    mock_wsv::{
        AccountId, AssetDefinitionId, DomainId, MockWorldStateView, Name, PermissionToken,
        ScopedAccountId, WsvHost,
    },
    syscalls,
};

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_numeric_tlv(amount: impl Into<Numeric>) -> Vec<u8> {
    let buf = norito::to_bytes(&amount.into()).expect("encode numeric into Norito");
    make_tlv(PointerType::NoritoBytes as u16, &buf)
}

fn test_account(domain: DomainId, public_key: PublicKey) -> ScopedAccountId {
    ScopedAccountId::new(domain, public_key)
}

#[test]
fn test_balance_syscall_permission() {
    let domain: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let alice = test_account(domain.clone(), pk1);
    let bob = test_account(domain, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let wsv = MockWorldStateView::with_balances(&[(
        (alice.clone(), asset.clone()),
        Numeric::from(50_u64),
    )]);
    // Bob has no permission initially
    let mut acc_map = HashMap::new();
    acc_map.insert(1, AccountId::from(&alice));
    acc_map.insert(2, AccountId::from(&bob));
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new_with_subject_map(
        wsv,
        AccountId::from(&bob),
        acc_map.clone(),
        asset_map.clone(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.set_register(10, 1); // account index alice
    vm.set_register(11, 1); // asset index
    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_ACCOUNT_BALANCE as u8]);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::PermissionDenied)));

    // Grant permission and retry using a fresh WSV instance
    let mut wsv2 = MockWorldStateView::with_balances(&[(
        (alice.clone(), asset.clone()),
        Numeric::from(50_u64),
    )]);
    wsv2.grant_permission(
        &bob,
        PermissionToken::ReadAccountAssets(AccountId::from(&alice)),
    );
    let host = WsvHost::new_with_subject_map(wsv2, AccountId::from(&bob), acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("balance syscall failed");
    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("balance tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode balance");
    assert_eq!(value, Numeric::from(50_u64));
}

#[test]
fn test_transfer_syscall_permission() {
    let domain: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let alice = test_account(domain.clone(), pk1);
    let bob = test_account(domain, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let wsv = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), Numeric::from(50_u64)),
        ((bob.clone(), asset.clone()), Numeric::from(0_u64)),
    ]);
    let mut acc_map = HashMap::new();
    acc_map.insert(1, AccountId::from(&alice));
    acc_map.insert(2, AccountId::from(&bob));
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new_with_subject_map(
        wsv,
        AccountId::from(&bob),
        acc_map.clone(),
        asset_map.clone(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.set_register(10, 1); // from alice
    vm.set_register(11, 2); // to bob
    vm.set_register(12, 1); // asset index
    let amount_tlv = make_numeric_tlv(10_u64);
    let amount_ptr = vm.alloc_input_tlv(&amount_tlv).expect("alloc amount tlv");
    vm.set_register(13, amount_ptr); // amount
    let prog = assemble_syscalls(&[syscalls::SYSCALL_TRANSFER_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::PermissionDenied)));

    let mut wsv2 = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), Numeric::from(50_u64)),
        ((bob.clone(), asset.clone()), Numeric::from(0_u64)),
    ]);
    wsv2.grant_permission(&bob, PermissionToken::TransferAsset(asset.clone()));
    let host = WsvHost::new_with_subject_map(wsv2, AccountId::from(&bob), acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("transfer syscall failed");
}

#[test]
fn test_mint_syscall_permission() {
    let domain: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let _alice = test_account(domain.clone(), pk1);
    let bob = test_account(domain, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let wsv =
        MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), Numeric::from(0_u64))]);
    let mut acc_map = HashMap::new();
    acc_map.insert(1, AccountId::from(&bob));
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new_with_subject_map(
        wsv,
        AccountId::from(&bob),
        acc_map.clone(),
        asset_map.clone(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.set_register(10, 1); // account index bob
    vm.set_register(11, 1); // asset index
    let amount_tlv = make_numeric_tlv(20_u64);
    let amount_ptr = vm.alloc_input_tlv(&amount_tlv).expect("alloc amount tlv");
    vm.set_register(12, amount_ptr); // amount
    let prog = assemble_syscalls(&[syscalls::SYSCALL_MINT_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::PermissionDenied)));

    let mut wsv2 =
        MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), Numeric::from(0_u64))]);
    wsv2.grant_permission(&bob, PermissionToken::MintAsset(asset.clone()));
    let host = WsvHost::new_with_subject_map(wsv2, AccountId::from(&bob), acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("mint syscall failed");
}

#[test]
fn test_json_get_numeric_reads_decimal_strings() {
    let domain: DomainId = "domain".parse().expect("domain");
    let public_key: PublicKey =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key");
    let scoped_subject = test_account(domain, public_key);
    let host = WsvHost::new_with_subject(
        MockWorldStateView::new(),
        AccountId::from(&scoped_subject),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let json = Json::from_str_norito(r#"{"amount":"0.00001"}"#).expect("json");
    let json_payload = norito::to_bytes(&json).expect("encode json");
    let key_payload =
        norito::to_bytes(&Name::from_str("amount").expect("name")).expect("encode key");
    let json_ptr = vm
        .alloc_input_tlv(&make_tlv(PointerType::Json as u16, &json_payload))
        .expect("alloc json");
    let key_ptr = vm
        .alloc_input_tlv(&make_tlv(PointerType::Name as u16, &key_payload))
        .expect("alloc key");

    vm.set_register(10, json_ptr);
    vm.set_register(11, key_ptr);
    let prog = assemble_syscalls(&[syscalls::SYSCALL_JSON_GET_NUMERIC as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().expect("json_get_numeric syscall failed");

    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("numeric tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode numeric");
    assert_eq!(value, "0.00001".parse::<Numeric>().expect("parse numeric"));
}
