use std::collections::HashMap;

use iroha_crypto::{Hash, PublicKey};
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, PointerType, VMError,
    mock_wsv::{AccountId, AssetDefinitionId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let h: [u8; 32] = Hash::new(&payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_numeric_tlv(amount: impl Into<Numeric>) -> Vec<u8> {
    let buf = norito::to_bytes(&amount.into()).expect("encode numeric into Norito");
    make_tlv(PointerType::NoritoBytes as u16, &buf)
}

#[test]
fn test_balance_syscall_permission() {
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let wsv = MockWorldStateView::with_balances(&[(
        (alice.clone(), asset.clone()),
        Numeric::from(50_u64),
    )]);
    // Bob has no permission initially
    let mut acc_map = HashMap::new();
    acc_map.insert(1, alice.clone());
    acc_map.insert(2, bob.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new(wsv, bob.clone(), acc_map.clone(), asset_map.clone());
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
    wsv2.grant_permission(&bob, PermissionToken::ReadAccountAssets(alice.clone()));
    let host = WsvHost::new(wsv2, bob, acc_map, asset_map);
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
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let wsv = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), Numeric::from(50_u64)),
        ((bob.clone(), asset.clone()), Numeric::from(0_u64)),
    ]);
    let mut acc_map = HashMap::new();
    acc_map.insert(1, alice.clone());
    acc_map.insert(2, bob.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new(wsv, bob.clone(), acc_map.clone(), asset_map.clone());
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
    let host = WsvHost::new(wsv2, bob, acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("transfer syscall failed");
}

#[test]
fn test_mint_syscall_permission() {
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let _alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let wsv =
        MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), Numeric::from(0_u64))]);
    let mut acc_map = HashMap::new();
    acc_map.insert(1, bob.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new(wsv, bob.clone(), acc_map.clone(), asset_map.clone());
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
    let host = WsvHost::new(wsv2, bob, acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("mint syscall failed");
}
