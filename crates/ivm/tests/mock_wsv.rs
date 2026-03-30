use std::collections::HashSet;

use iroha_data_model::prelude::PublicKey;
use iroha_primitives::numeric::Numeric;
use ivm::mock_wsv::{
    AccountId, AssetDefinitionId, DomainId, Mintable, MockWorldStateView, NftId, PermissionToken,
};

fn num(value: u64) -> Numeric {
    Numeric::from(value)
}

fn test_account(domain: &DomainId, public_key: PublicKey) -> AccountId {
    let _domain = domain;
    AccountId::new(public_key)
}

#[test]
fn test_mock_wsv_basic_ops() {
    let d: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc1 = test_account(&d, pk1);
    let acc2 = test_account(&d, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::with_balances(&[
        ((acc1.clone(), asset.clone()), num(100)),
        ((acc2.clone(), asset.clone()), num(0)),
    ]);
    wsv.grant_permission(&acc2, PermissionToken::MintAsset(asset.clone()));

    assert_eq!(wsv.balance(acc1.clone(), asset.clone()), num(100));
    assert!(wsv.transfer(&acc1, acc1.clone(), acc2.clone(), asset.clone(), num(30),));
    assert_eq!(wsv.balance(acc1.clone(), asset.clone()), num(70));
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), num(30));
    assert!(wsv.mint(&acc2, acc2.clone(), asset.clone(), num(10)));
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), num(40));
    assert!(wsv.burn(&acc2, acc2.clone(), asset.clone(), num(25)));
    assert_eq!(wsv.balance(acc2.clone(), asset.clone()), num(15));
    assert!(!wsv.burn(&acc2, acc2.clone(), asset, num(100)));
}

#[test]
fn test_mock_wsv_rejects_scaled_numeric() {
    let d: DomainId = "domain".parse().unwrap();
    let pk1: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let pk2: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc1 = test_account(&d, pk1);
    let acc2 = test_account(&d, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::with_balances(&[
        ((acc1.clone(), asset.clone()), Numeric::from(1_000_u64)),
        ((acc2.clone(), asset.clone()), Numeric::from(0_u64)),
    ]);
    assert!(!wsv.transfer(
        &acc1,
        acc1.clone(),
        acc2.clone(),
        asset.clone(),
        Numeric::new(25_u64, 2)
    ));
    assert_eq!(
        wsv.balance(acc1.clone(), asset.clone()),
        Numeric::from(1_000_u64)
    );
    assert_eq!(wsv.balance(acc2.clone(), asset), Numeric::from(0_u64));
}

#[test]
fn test_register_and_mint_once() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc = test_account(&d, pk);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(acc.clone());
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    assert!(wsv.register_asset_definition(&acc, asset.clone(), Mintable::Once));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), num(50)));
    assert_eq!(wsv.balance(acc.clone(), asset.clone()), num(50));
    // second mint should fail for Mintable::Once assets
    assert!(!wsv.mint(&acc, acc.clone(), asset, num(10)));
}

#[test]
fn test_register_and_mint_limited() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc = test_account(&d, pk);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "ticket".parse().unwrap(),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(acc.clone());
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    let limited = Mintable::limited_from_u32(2).expect("non-zero budget");
    assert!(wsv.register_asset_definition(&acc, asset.clone(), limited));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), num(10)));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), num(5)));
    // third mint exhausts budget
    assert!(!wsv.mint(&acc, acc.clone(), asset, num(1)));
}

#[test]
fn test_limited_asset_budget_exhaustion() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        .parse()
        .unwrap();
    let acc = test_account(&d, pk);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "badge".parse().unwrap(),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(acc.clone());
    wsv.grant_permission(&acc, PermissionToken::RegisterDomain);
    wsv.grant_permission(&acc, PermissionToken::RegisterAccount);
    wsv.grant_permission(&acc, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&acc, PermissionToken::MintAsset(asset.clone()));

    let limited = Mintable::limited_from_u32(1).expect("non-zero budget");
    assert!(wsv.register_asset_definition(&acc, asset.clone(), limited));
    assert!(wsv.mint(&acc, acc.clone(), asset.clone(), num(42)));
    // budget exhausted after a single mint
    assert!(!wsv.mint(&acc, acc.clone(), asset, num(1)));
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
    let acc1 = test_account(&d, pk1);
    let acc2 = test_account(&d, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::with_balances(&[((acc1.clone(), asset.clone()), num(100))]);
    wsv.add_account_unchecked(acc2.clone());
    wsv.grant_permission(&acc1, PermissionToken::ReadAccountAssets(acc1.clone()));

    // acc2 should not see acc1's balance without permission
    assert_eq!(wsv.balance_checked(&acc2, &acc1, &asset), None);
    // grant permission to acc2
    wsv.grant_permission(&acc2, PermissionToken::ReadAccountAssets(acc1.clone()));
    assert_eq!(wsv.balance_checked(&acc2, &acc1, &asset), Some(num(100)));
}

#[test]
fn unregister_asset_after_burning_out() {
    let d: DomainId = "domain".parse().unwrap();
    let pk: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let acc = test_account(&d, pk);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::with_balances(&[((acc.clone(), asset.clone()), num(10))]);
    assert!(wsv.burn(&acc, acc.clone(), asset.clone(), num(10)));
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
    let acc1 = test_account(&d, pk1);
    let acc2 = test_account(&d, pk2);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "asset".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::with_balances(&[
        ((acc1.clone(), asset.clone()), num(25)),
        ((acc2.clone(), asset.clone()), num(0)),
    ]);

    assert!(wsv.transfer(&acc1, acc1.clone(), acc2.clone(), asset.clone(), num(25),));
    assert!(wsv.unregister_account(&acc1));
}

#[test]
fn unregister_account_detaches_subject_and_preserves_state_for_relink() {
    let first_domain: DomainId = "domain".parse().unwrap();
    let second_domain: DomainId = "domain-2".parse().unwrap();
    let account_pk: PublicKey =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .unwrap();
    let admin_pk: PublicKey =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
            .parse()
            .unwrap();
    let account = test_account(&first_domain, account_pk.clone());
    let account_relinked = test_account(&second_domain, account_pk);
    let admin = test_account(&first_domain, admin_pk);
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "domain".parse().unwrap(),
        "rose".parse().unwrap(),
    );

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(admin.clone());
    wsv.grant_permission(&admin, PermissionToken::RegisterDomain);
    wsv.grant_permission(&admin, PermissionToken::RegisterAccount);
    wsv.grant_permission(&admin, PermissionToken::RegisterAssetDefinition);

    assert!(wsv.register_domain(&admin, second_domain.clone()));
    assert!(wsv.register_account(&admin, account.clone()));
    assert!(wsv.register_asset_definition(&admin, asset.clone(), Mintable::Infinitely));

    let mut perms = HashSet::new();
    perms.insert(PermissionToken::MintAsset(asset.clone()));
    assert!(wsv.create_role("minter", perms));
    assert!(wsv.grant_role(&account, "minter"));
    wsv.grant_permission(
        &account,
        PermissionToken::ReadAccountAssets(account.clone()),
    );

    assert!(wsv.has_permission(&account, &PermissionToken::MintAsset(asset.clone())));
    assert!(wsv.has_permission(
        &account,
        &PermissionToken::ReadAccountAssets(account.clone())
    ));

    assert!(wsv.unregister_account(&account));
    assert!(!wsv.has_permission(&account, &PermissionToken::MintAsset(asset.clone())));
    assert!(!wsv.has_permission(
        &account,
        &PermissionToken::ReadAccountAssets(account.clone())
    ));

    assert!(wsv.register_account(&admin, account_relinked.clone()));
    assert!(wsv.has_permission(
        &account_relinked,
        &PermissionToken::MintAsset(asset.clone())
    ));
    assert!(wsv.has_permission(
        &account_relinked,
        &PermissionToken::ReadAccountAssets(account_relinked.clone())
    ));
}

#[test]
fn unregister_domain_rejects_cross_domain_nfts() {
    let nft_domain: DomainId = "nft-domain".parse().unwrap();
    let holder_domain: DomainId = "holder-domain".parse().unwrap();
    let pk: PublicKey = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        .parse()
        .unwrap();
    let owner = test_account(&holder_domain, pk);
    let nft_id: NftId = format!("artifact${nft_domain}").parse().unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(owner.clone());
    wsv.grant_permission(&owner, PermissionToken::RegisterDomain);
    wsv.grant_permission(&owner, PermissionToken::RegisterAccount);

    assert!(wsv.register_domain(&owner, nft_domain.clone()));
    assert!(wsv.create_nft(owner.clone(), owner.clone(), nft_id.clone()));
    assert!(!wsv.unregister_domain(&nft_domain));

    assert!(wsv.burn_nft(&owner, &nft_id));
    assert!(wsv.unregister_domain(&nft_domain));
}

#[test]
fn account_domain_links_are_queryable_and_removed_with_account_unregister() {
    let first_domain: DomainId = "domain".parse().unwrap();
    let second_domain: DomainId = "domain-2".parse().unwrap();
    let admin_pk: PublicKey =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
            .parse()
            .unwrap();
    let subject_pk: PublicKey =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .unwrap();

    let admin = test_account(&first_domain, admin_pk);
    let scoped_first = test_account(&first_domain, subject_pk.clone());
    let scoped_second = test_account(&second_domain, subject_pk);
    let admin_subject = admin.clone();
    let subject = scoped_first.clone();

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(admin.clone());
    wsv.grant_permission(&admin, PermissionToken::RegisterDomain);
    wsv.grant_permission(&admin, PermissionToken::RegisterAccount);

    assert!(wsv.register_domain(&admin, first_domain.clone()));
    assert!(wsv.register_domain(&admin, second_domain.clone()));
    assert!(wsv.register_account(&admin, scoped_first.clone()));
    assert!(wsv.link_subject_to_domain(admin.clone(), first_domain.clone()));
    assert!(wsv.link_subject_to_domain(subject.clone(), first_domain.clone()));
    assert!(wsv.link_subject_to_domain(subject.clone(), second_domain.clone()));

    wsv.grant_permission(&scoped_first, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.has_permission(&scoped_second, &PermissionToken::RegisterAssetDefinition));

    assert_eq!(
        wsv.domains_for_account(&subject),
        vec![first_domain.clone(), second_domain.clone()]
    );
    let first_domain_subjects = wsv.linked_subjects_for_domain(&first_domain);
    assert_eq!(first_domain_subjects.len(), 2);
    assert!(first_domain_subjects.contains(&admin_subject));
    assert!(first_domain_subjects.contains(&subject));
    assert_eq!(
        wsv.linked_subjects_for_domain(&second_domain),
        vec![subject.clone()]
    );

    assert!(wsv.unregister_account(&scoped_first));
    assert!(wsv.domains_for_account(&subject).is_empty());
    assert!(!wsv.has_permission(&scoped_second, &PermissionToken::RegisterAssetDefinition));
}
