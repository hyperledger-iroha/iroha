use iroha_crypto::PublicKey;
use ivm::mock_wsv::{
    AccountId, AssetDefinitionId, DomainId, Mintable, MockWorldStateView, PermissionToken,
    ZkAssetMode, ZkPolicyConfig,
};

fn account(public_key: &str) -> AccountId {
    let public_key: PublicKey = public_key.parse().expect("public key");
    AccountId::new(public_key)
}

fn setup_asset(name: &str) -> (AccountId, AssetDefinitionId, MockWorldStateView) {
    let caller = account("ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774");
    let domain = DomainId::try_new("domain", "universal").expect("domain id");
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
    (caller, asset, wsv)
}

#[test]
fn direct_zk_register_emits_policy_event() {
    let (_caller, asset, mut wsv) = setup_asset("rose");
    assert!(wsv.register_zk_asset(
        asset.clone(),
        ZkPolicyConfig {
            mode: ZkAssetMode::Hybrid,
            allow_unshield: true,
            vk_transfer: None,
            vk_unshield: None,
        }
    ));

    let events = wsv.drain_zk_events();
    assert!(events.iter().any(
        |event| matches!(event, ivm::mock_wsv::ZkEvent::ZkPolicyUpdated { asset: id, .. } if id == &asset)
    ));
}
