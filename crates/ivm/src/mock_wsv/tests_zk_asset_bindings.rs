//! ZK and opaque asset-binding regression tests for the mock world-state host.

use super::*;

#[test]
fn register_without_vk_allows_shield() {
    let caller: AccountId = test_account_id(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
        "domain",
    );
    let domain: DomainId = DomainId::try_new("domain", "universal").unwrap();
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("domain", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&caller, domain.clone()));
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_asset_definition(&caller, asset.clone(), Mintable::Infinitely));
    wsv.grant_permission(&caller, PermissionToken::RegisterZkAsset(asset.clone()));
    wsv.grant_permission(&caller, PermissionToken::Shield(asset.clone()));
    wsv.grant_permission(&caller, PermissionToken::MintAsset(asset.clone()));
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: None,
            vk_shield: None,
        },
    ));
    wsv.mint(
        &caller,
        caller.clone(),
        asset.clone(),
        Quantity::from(10_u64),
    );
    assert!(wsv.shield(&caller, &asset, Quantity::from(3_u64), [7u8; 32]));
}

#[test]
fn unshield_appends_private_change_outputs_and_rejects_duplicate_inputs_without_mutation() {
    let caller: AccountId = test_account_id(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
        "domain",
    );
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("domain", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let mut wsv = MockWorldStateView::with_balances(&[(
        (caller.clone(), asset.clone()),
        Quantity::from(10_u64),
    )]);
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_shield: true,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: None,
            vk_shield: None,
        },
    ));
    wsv.drain_zk_events();

    let proof = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0xA5]),
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "unshield_vk"),
    );
    let inputs = [[1u8; 32], [2u8; 32]];
    let outputs = [[9u8; 32], [10u8; 32]];
    assert!(wsv.unshield(
        &caller,
        &asset,
        Quantity::from(4_u64),
        &inputs,
        &outputs,
        &proof,
    ));
    assert_eq!(
        wsv.balance(caller.clone(), asset.clone()),
        Quantity::from(14_u64)
    );
    let (latest_root, roots, depth) = wsv.get_roots(&asset, 8);
    assert_eq!(depth, 2);
    assert_eq!(roots.len(), 2);
    assert_eq!(latest_root, roots[1]);

    let events = wsv.drain_zk_events();
    assert_eq!(
        events,
        vec![
            ZkEvent::CommitmentAdded {
                asset: asset.clone(),
                commitment: outputs[0],
                new_root: roots[0],
            },
            ZkEvent::CommitmentAdded {
                asset: asset.clone(),
                commitment: outputs[1],
                new_root: roots[1],
            },
            ZkEvent::Unshielded {
                asset: asset.clone(),
                to: caller.clone(),
                public_amount: Quantity::from(4_u64),
            },
        ]
    );

    let duplicate_inputs = [[3u8; 32], [3u8; 32]];
    assert!(!wsv.unshield(
        &caller,
        &asset,
        Quantity::from(1_u64),
        &duplicate_inputs,
        &[[11u8; 32]],
        &proof,
    ));
    assert_eq!(
        wsv.balance(caller.clone(), asset.clone()),
        Quantity::from(14_u64)
    );
    let (latest_after_failure, roots_after_failure, depth_after_failure) = wsv.get_roots(&asset, 8);
    assert_eq!(depth_after_failure, 2);
    assert_eq!(roots_after_failure, roots);
    assert_eq!(latest_after_failure, latest_root);
    assert!(wsv.drain_zk_events().is_empty());
}

#[test]
fn register_asset_definition_does_not_require_domain_row_for_opaque_id() {
    let caller: AccountId = test_account_id(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
        "domain",
    );
    let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonder", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let opaque = norito::decode_from_bytes::<AssetDefinitionId>(
        &norito::to_bytes(&asset).expect("encode asset definition"),
    )
    .expect("decode opaque canonical asset definition");

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);

    assert!(
        wsv.register_asset_definition(&caller, opaque.clone(), Mintable::Infinitely),
        "opaque asset definition ids should register without a matching domain row"
    );
    assert!(
        wsv.asset_definitions.contains_key(&opaque),
        "registered opaque asset definition should be stored"
    );
}

#[test]
fn unregister_domain_ignores_opaque_asset_definition_ids() {
    let caller: AccountId = test_account_id(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
        "domain",
    );
    let domain: DomainId = DomainId::try_new("wonder", "universal").unwrap();
    let projected =
        iroha_data_model::asset::AssetDefinitionId::new(domain.clone(), "rose".parse().unwrap());
    let opaque = norito::decode_from_bytes::<AssetDefinitionId>(
        &norito::to_bytes(&projected).expect("encode asset definition"),
    )
    .expect("decode opaque canonical asset definition");

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    assert!(wsv.register_domain(&caller, domain.clone()));
    assert!(wsv.register_asset_definition(&caller, opaque, Mintable::Infinitely));
    assert!(
        wsv.unregister_domain(&domain),
        "opaque asset definitions must not pin a domain because they have no domain projection"
    );
}
