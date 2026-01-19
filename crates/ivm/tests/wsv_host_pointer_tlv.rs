use std::collections::HashMap;

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::isi::transfer::{TransferAssetBatch, TransferAssetBatchEntry};
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, IVMHost, Memory, PointerType,
    mock_wsv::{AccountId, AssetDefinitionId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};
use norito::to_bytes;

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_account_tlv(account: &AccountId) -> Vec<u8> {
    let buf = to_bytes(account).expect("encode account into Norito");
    make_tlv(PointerType::AccountId as u16, &buf)
}

fn make_asset_tlv(asset: &AssetDefinitionId) -> Vec<u8> {
    let buf = to_bytes(asset).expect("encode asset into Norito");
    make_tlv(PointerType::AssetDefinitionId as u16, &buf)
}

fn make_numeric_tlv(amount: impl Into<Numeric>) -> Vec<u8> {
    let buf = to_bytes(&amount.into()).expect("encode numeric into Norito");
    make_tlv(PointerType::NoritoBytes as u16, &buf)
}

fn make_transfer_batch_tlv(
    entries: &[(AccountId, AccountId, AssetDefinitionId, Numeric)],
) -> Vec<u8> {
    let batch_entries = entries
        .iter()
        .map(|(from, to, asset, amount)| {
            TransferAssetBatchEntry::new(
                from.clone(),
                to.clone(),
                asset.clone(),
                amount.clone(),
            )
        })
        .collect();
    let batch = TransferAssetBatch::new(batch_entries);
    let buf = to_bytes(&batch).expect("encode transfer batch into Norito");
    make_tlv(PointerType::NoritoBytes as u16, &buf)
}

fn num(value: u64) -> Numeric {
    Numeric::from(value)
}

#[test]
fn balance_syscall_with_tlv_pointers() {
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let wsv =
        MockWorldStateView::with_balances(&[((alice.clone(), asset.clone()), num(50))]);
    let host = WsvHost::new(wsv, bob.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Preload TLVs for alice and asset
    let acc = make_account_tlv(&alice);
    vm.memory.preload_input(0, &acc).expect("preload input");
    let asset_tlv = make_asset_tlv(&asset);
    vm.memory
        .preload_input(acc.len() as u64 + 8, &asset_tlv)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START); // account ptr
    vm.set_register(11, Memory::INPUT_START + acc.len() as u64 + 8); // asset ptr

    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_ACCOUNT_BALANCE as u8]);
    vm.load_program(&prog).unwrap();
    // Bob lacks permission -> PermissionDenied
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));

    // Grant and retry
    let mut wsv2 =
        MockWorldStateView::with_balances(&[((alice.clone(), asset.clone()), num(50))]);
    wsv2.grant_permission(&bob, PermissionToken::ReadAccountAssets(alice.clone()));
    let host = WsvHost::new(wsv2, bob, HashMap::new(), HashMap::new());
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("balance tlv syscall failed");
    let tlv = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("balance tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode balance");
    assert_eq!(value, Numeric::from(50_u64));
}

#[test]
fn transfer_syscall_with_tlv_pointers() {
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
        ((alice.clone(), asset.clone()), num(50)),
        ((bob.clone(), asset.clone()), num(0)),
    ]);
    let host = WsvHost::new(wsv, bob.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let acc_from = make_account_tlv(&alice);
    vm.memory
        .preload_input(0, &acc_from)
        .expect("preload input");
    let acc_to = make_account_tlv(&bob);
    vm.memory
        .preload_input(acc_from.len() as u64 + 8, &acc_to)
        .expect("preload input");
    let asset_tlv = make_asset_tlv(&asset);
    vm.memory
        .preload_input(acc_from.len() as u64 + acc_to.len() as u64 + 16, &asset_tlv)
        .expect("preload input");
    let amount_tlv = make_numeric_tlv(10_u64);
    let amount_offset = acc_from.len() as u64 + acc_to.len() as u64 + asset_tlv.len() as u64 + 24;
    vm.memory
        .preload_input(amount_offset, &amount_tlv)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acc_from.len() as u64 + 8);
    vm.set_register(
        12,
        Memory::INPUT_START + acc_from.len() as u64 + acc_to.len() as u64 + 16,
    );
    vm.set_register(13, Memory::INPUT_START + amount_offset);

    let prog = assemble_syscalls(&[syscalls::SYSCALL_TRANSFER_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));

    let mut wsv2 = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), num(50)),
        ((bob.clone(), asset.clone()), num(0)),
    ]);
    wsv2.grant_permission(&bob, PermissionToken::TransferAsset(asset.clone()));
    let host = WsvHost::new(wsv2, bob, HashMap::new(), HashMap::new());
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("transfer tlv syscall failed");
}

#[test]
fn mint_syscall_with_tlv_pointers() {
    let pk: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let bob: AccountId = format!("{pk}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let wsv = MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), num(0))]);
    let host = WsvHost::new(wsv, bob.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let acc = make_account_tlv(&bob);
    vm.memory.preload_input(0, &acc).expect("preload input");
    let asset_tlv = make_asset_tlv(&asset);
    vm.memory
        .preload_input(acc.len() as u64 + 8, &asset_tlv)
        .expect("preload input");
    let amount_tlv = make_numeric_tlv(20_u64);
    let amount_offset = acc.len() as u64 + asset_tlv.len() as u64 + 16;
    vm.memory
        .preload_input(amount_offset, &amount_tlv)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acc.len() as u64 + 8);
    vm.set_register(12, Memory::INPUT_START + amount_offset);

    let prog = assemble_syscalls(&[syscalls::SYSCALL_MINT_ASSET as u8]);
    vm.load_program(&prog).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));

    let mut wsv2 =
        MockWorldStateView::with_balances(&[((bob.clone(), asset.clone()), num(0))]);
    wsv2.grant_permission(&bob, PermissionToken::MintAsset(asset.clone()));
    let host = WsvHost::new(wsv2, bob, HashMap::new(), HashMap::new());
    vm.set_host(host);
    vm.load_program(&prog).unwrap();
    vm.run().expect("mint tlv syscall failed");
}

#[test]
fn transfer_batch_syscalls_buffer_entries() {
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let pk3: PublicKey = "ed012026DB3C0E3D6A4C53E2CD59000B2D5F9ECB41D4EDD5E0C83F9F1B40D0F0A5BF42"
        .parse()
        .unwrap();
    let alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let carol: AccountId = format!("{pk3}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), num(50)),
        ((bob.clone(), asset.clone()), num(0)),
        ((carol.clone(), asset.clone()), num(0)),
    ]);
    wsv.grant_permission(&bob, PermissionToken::TransferAsset(asset.clone()));
    let mut account_map = HashMap::new();
    account_map.insert(1, alice.clone());
    account_map.insert(2, bob.clone());
    account_map.insert(3, carol.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1, asset.clone());
    let mut host = WsvHost::new(wsv, bob.clone(), account_map, asset_map);
    let mut vm = IVM::new(u64::MAX);

    host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm)
        .expect("begin batch");

    vm.set_register(10, 1);
    vm.set_register(11, 2);
    vm.set_register(12, 1);
    let amount1 = make_numeric_tlv(10_u64);
    let amount1_ptr = vm.alloc_input_tlv(&amount1).expect("alloc amount 1 tlv");
    vm.set_register(13, amount1_ptr);
    host.syscall(syscalls::SYSCALL_TRANSFER_ASSET, &mut vm)
        .expect("push entry 1");

    vm.set_register(10, 1);
    vm.set_register(11, 3);
    vm.set_register(12, 1);
    let amount2 = make_numeric_tlv(5_u64);
    let amount2_ptr = vm.alloc_input_tlv(&amount2).expect("alloc amount 2 tlv");
    vm.set_register(13, amount2_ptr);
    host.syscall(syscalls::SYSCALL_TRANSFER_ASSET, &mut vm)
        .expect("push entry 2");

    host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm)
        .expect("finish batch");

    assert_eq!(
        host.wsv.balance(bob.clone(), asset.clone()),
        num(10),
        "bob should receive first transfer"
    );
    assert_eq!(
        host.wsv.balance(carol.clone(), asset.clone()),
        num(5),
        "carol should receive second transfer"
    );
    assert_eq!(
        host.wsv.balance(alice.clone(), asset),
        num(35),
        "alice balance must decrease by combined amount"
    );
}

#[test]
fn transfer_batch_apply_syscall_executes_batch() {
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let pk3: PublicKey = "ed012026DB3C0E3D6A4C53E2CD59000B2D5F9ECB41D4EDD5E0C83F9F1B40D0F0A5BF42"
        .parse()
        .unwrap();
    let alice: AccountId = format!("{pk1}@domain").parse().unwrap();
    let bob: AccountId = format!("{pk2}@domain").parse().unwrap();
    let carol: AccountId = format!("{pk3}@domain").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset.clone()), num(50)),
        ((bob.clone(), asset.clone()), num(0)),
        ((carol.clone(), asset.clone()), num(0)),
    ]);
    wsv.grant_permission(&bob, PermissionToken::TransferAsset(asset.clone()));
    let host = WsvHost::new(wsv, bob.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let batch_tlv = make_transfer_batch_tlv(&[
        (alice.clone(), bob.clone(), asset.clone(), num(10)),
        (alice.clone(), carol.clone(), asset.clone(), num(5)),
    ]);
    vm.memory
        .preload_input(0, &batch_tlv)
        .expect("preload batch tlv");
    vm.set_register(10, Memory::INPUT_START);

    let prog = assemble_syscalls(&[syscalls::SYSCALL_TRANSFER_V1_BATCH_APPLY as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().expect("transfer batch apply should succeed");

    let host = vm
        .host_mut_any()
        .and_then(|host| host.downcast_mut::<WsvHost>())
        .expect("mock host");
    assert_eq!(
        host.wsv.balance(bob.clone(), asset.clone()),
        num(10),
        "bob receives the first transfer"
    );
    assert_eq!(
        host.wsv.balance(carol.clone(), asset.clone()),
        num(5),
        "carol receives the second transfer"
    );
    assert_eq!(
        host.wsv.balance(alice.clone(), asset.clone()),
        num(35),
        "alice decreases by the combined amount"
    );
}
