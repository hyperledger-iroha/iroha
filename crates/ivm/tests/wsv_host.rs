use std::collections::HashMap;

use iroha_crypto::PublicKey;
use ivm::{
    IVM, VMError,
    mock_wsv::{AccountId, AssetDefinitionId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};

mod common;
use common::assemble_syscalls;

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

    let wsv = MockWorldStateView::with_balances(&[((alice.clone(), asset.clone()), 50)]);
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
    let mut wsv2 = MockWorldStateView::with_balances(&[((alice.clone(), asset.clone()), 50)]);
    wsv2.grant_permission(&bob, PermissionToken::ReadAccountAssets(alice.clone()));
    let host = WsvHost::new(wsv2, bob, acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("balance syscall failed");
    assert_eq!(vm.register(10), 50);
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
        ((alice.clone(), asset.clone()), 50),
        ((bob.clone(), asset.clone()), 0),
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
    vm.set_register(13, 10); // amount
    let prog = assemble_syscalls(&[syscalls::SYSCALL_TRANSFER_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::PermissionDenied)));

    let mut wsv2 = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), 50),
        ((bob.clone(), asset.clone()), 0),
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

    let wsv = MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), 0)]);
    let mut acc_map = HashMap::new();
    acc_map.insert(1, bob.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());

    let host = WsvHost::new(wsv, bob.clone(), acc_map.clone(), asset_map.clone());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.set_register(10, 1); // account index bob
    vm.set_register(11, 1); // asset index
    vm.set_register(12, 20); // amount
    let prog = assemble_syscalls(&[syscalls::SYSCALL_MINT_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    let result = vm.run();
    assert!(matches!(result, Err(VMError::PermissionDenied)));

    let mut wsv2 = MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), 0)]);
    wsv2.grant_permission(&bob, PermissionToken::MintAsset(asset.clone()));
    let host = WsvHost::new(wsv2, bob, acc_map, asset_map);
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("mint syscall failed");
}
