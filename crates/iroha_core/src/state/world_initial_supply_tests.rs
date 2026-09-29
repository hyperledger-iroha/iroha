//! Initial World asset supplies are derived from exact retained balances.

use super::*;

#[test]
fn initial_supply_sums_retained_balances_once() {
    let alice = iroha_test_samples::ALICE_ID.clone();
    let bob = iroha_test_samples::BOB_ID.clone();
    let domain = DomainId::try_new("supply", "universal").unwrap();
    let definition_id =
        AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse().unwrap());
    let definition = AssetDefinition::numeric(
        definition_id.clone(),
        "xor",
        AssetBalancePolicy::Global,
        None,
    )
    .build(&alice);
    let alice_asset = AssetId::of(definition_id.clone(), alice.clone());
    let bob_asset = AssetId::of(definition_id.clone(), bob.clone());
    let world = World::with_assets(
        [Domain::new(domain).build(&alice)],
        [
            Account::new(alice.clone()).build(&alice),
            Account::new(bob.clone()).build(&bob),
        ],
        [definition],
        [
            Asset::new(alice_asset.clone(), Quantity::from(99_u32)),
            Asset::new(alice_asset.clone(), Quantity::from(3_u32)),
            Asset::new(bob_asset.clone(), Quantity::from(5_u32)),
        ],
        [],
    );
    let view = world.view();
    assert_eq!(
        view.asset_total_amount(&definition_id).unwrap(),
        Quantity::from(8_u32)
    );
    assert_eq!(
        view.assets().get(&alice_asset).unwrap().0,
        Quantity::from(3_u32)
    );
    assert_eq!(
        view.assets().get(&bob_asset).unwrap().0,
        Quantity::from(5_u32)
    );
}
