use std::collections::HashSet;

use iroha_data_model::prelude::PublicKey;
use ivm::mock_wsv::{
    AccountId, AssetDefinitionId, DomainId, Mintable, MockWorldStateView, PermissionToken,
};

#[test]
fn test_mock_wsv_basic_ops() {
    let d: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc1: AccountId = format!("{pk1}@{d}").parse().unwrap();
    let acc2: AccountId = format!("{pk2}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[
        ((acc1.clone(), asset.clone()), 100),
        ((acc2.clone(), asset.clone()), 0),
    ]);
    wsv.grant_permission(&acc2, PermissionToken::MintAsset(asset.clone()));

    assert_eq!(wsv.balance(acc1.clone(), asset.clone()), 100);
    assert!(wsv.transfer(&acc1, acc1.clone(), acc2.clone(), asset.clone(), 30));
    assert_eq!(wsv.balance(acc1.clone(), asset.clone()), 70);
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), 30);
    assert!(wsv.mint(&acc2, acc2.clone(), asset.clone(), 10));
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), 40);
    assert!(wsv.burn(&acc2, acc2.clone(), asset.clone(), 25));
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), 15);
    assert!(!wsv.burn(&acc2, acc2.clone(), asset, 100));
}

#[test]
fn test_register_and_mint_once() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc: AccountId = format!("{pk}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    assert!(wsv.register_domain(&acc, d.clone()));
    assert!(wsv.register_account(&acc, acc.clone()));
    assert!(wsv.register_asset_definition(&acc, asset.clone(), Mintable::Once));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), 50));
    assert_eq!(wsv.balance(acc.clone(), asset.clone()), 50);
    // second mint should fail for Mintable::Once assets
    assert!(!wsv.mint(&acc, acc.clone(), asset, 10));
}

#[test]
fn test_register_and_mint_limited() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc: AccountId = format!("{pk}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "ticket#domain".parse().unwrap();
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    assert!(wsv.register_domain(&acc, d.clone()));
    assert!(wsv.register_account(&acc, acc.clone()));
    let limited = Mintable::limited_from_u32(2).expect("non-zero budget");
    assert!(wsv.register_asset_definition(&acc, asset.clone(), limited));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), 10));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), 5));
    // third mint exhausts budget
    assert!(!wsv.mint(&acc, acc.clone(), asset, 1));
}

#[test]
fn test_limited_asset_budget_exhaustion() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc: AccountId = format!("{pk}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "badge#domain".parse().unwrap();
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    assert!(wsv.register_domain(&acc, d.clone()));
    assert!(wsv.register_account(&acc, acc.clone()));
    let limited = Mintable::limited_from_u32(1).expect("non-zero budget");
    assert!(wsv.register_asset_definition(&acc, asset.clone(), limited));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), 42));
    // budget exhausted after a single mint
    assert!(!wsv.mint(&acc, acc.clone(), asset, 1));
}

#[test]
fn test_balance_permission() {
    let d: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc1: AccountId = format!("{pk1}@{d}").parse().unwrap();
    let acc2: AccountId = format!("{pk2}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[((acc1.clone(), asset.clone()), 100)]);
    wsv.grant_permission(&acc1, PermissionToken::ReadAccountAssets(acc1.clone()));

    // acc2 should not see acc1's balance without permission
    assert_eq!(wsv.balance_checked(&acc2, &acc1, &asset), None);
    // grant permission to acc2
    wsv.grant_permission(&acc2, PermissionToken::ReadAccountAssets(acc1.clone()));
    assert_eq!(wsv.balance_checked(&acc2, &acc1, &asset), Some(100));
}

#[test]
fn unregister_asset_after_burning_out() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc: AccountId = format!("{pk}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[((acc.clone(), asset.clone()), 10)]);
    assert!(wsv.burn(&acc, acc.clone(), asset.clone(), 10));
    assert!(wsv.unregister_asset_definition(&asset));
    assert!(wsv.unregister_account(&acc));
}

#[test]
fn unregister_account_after_transferring_everything_out() {
    let d: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc1: AccountId = format!("{pk1}@{d}").parse().unwrap();
    let acc2: AccountId = format!("{pk2}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "asset#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::with_balances(&[
        ((acc1.clone(), asset.clone()), 25),
        ((acc2.clone(), asset.clone()), 0),
    ]);

    assert!(wsv.transfer(&acc1, acc1.clone(), acc2.clone(), asset.clone(), 25));
    assert!(wsv.unregister_account(&acc1));
}

#[test]
fn unregister_account_clears_permissions_and_roles() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc: AccountId = format!("{pk}@{d}").parse().unwrap();
    let asset: AssetDefinitionId = "rose#domain".parse().unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);

    assert!(wsv.register_domain(&acc, d.clone()));
    assert!(wsv.register_account(&acc, acc.clone()));
    assert!(wsv.register_asset_definition(&acc, asset.clone(), Mintable::Infinitely));

    let mut perms = HashSet::new();
    perms.insert(PermissionToken::MintAsset(asset.clone()));
    assert!(wsv.create_role("minter", perms));
    assert!(wsv.grant_role(&acc, "minter"));
    wsv.grant_permission(&acc, PermissionToken::ReadAccountAssets(acc.clone()));

    assert!(wsv.has_permission(&acc, &PermissionToken::MintAsset(asset.clone())));
    assert!(wsv.has_permission(&acc, &PermissionToken::ReadAccountAssets(acc.clone())));

    assert!(wsv.unregister_account(&acc));
    assert!(!wsv.has_permission(&acc, &PermissionToken::MintAsset(asset.clone())));
    assert!(!wsv.has_permission(&acc, &PermissionToken::ReadAccountAssets(acc.clone())));
}
