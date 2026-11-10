//! Local ownership and typed-preimage controls; composed finality coverage lives in activation tests.

use super::*;

#[test]
fn canonical_copy_retains_its_original_charge_and_refunds_after_drop() {
    let original = vec![7u8; 4096];
    let budget = AllocationBudget::new(2 * 1024 * 1024);
    let copy = funded_copy(&original, &budget).unwrap();
    assert_eq!(copy.value, original);
    assert!(copy._retained.belongs_to(&budget));
    assert!(budget.reserved_bytes() >= copy.value.len());
    drop(copy.value);
    assert!(budget.reserved_bytes() > 0);
    drop(copy._retained);
    assert_eq!(budget.reserved_bytes(), 0);

    let zero = AllocationBudget::new(0);
    assert!(matches!(
        funded_copy(&original, &zero),
        Err(WorldStateSnapshotError::Allocation(_))
    ));
    assert_eq!(zero.reserved_bytes(), 0);
    let retry = funded_copy(&original, &budget).unwrap();
    assert_eq!(retry.value, original);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn canonical_copy_preserves_enclosing_decoder_refusal_and_retries_same_original() {
    let original = vec![9u8; 1024];
    let budget = AllocationBudget::new(2 * 1024 * 1024);
    let enclosing = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        64,
    ));
    let result = enclosing.with(|| funded_copy(&original, &budget));
    assert!(matches!(result, Err(WorldStateSnapshotError::Encoding(_))));
    assert_eq!(budget.reserved_bytes(), 0);
    let retry = funded_copy(&original, &budget).unwrap();
    assert_eq!(retry.value, original);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn oversized_original_is_refused_before_pool_admission() {
    let original = vec![0u8; MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1 + 1];
    let zero = AllocationBudget::new(0);
    assert!(matches!(
        funded_copy(&original, &zero),
        Err(WorldStateSnapshotError::Invalid(_))
    ));
    assert_eq!(zero.reserved_bytes(), 0);
}

#[test]
fn balance_membership_and_absence_refuse_different_pre_tail_rows() {
    // A structural snapshot is sufficient to exercise this private target matcher. It has
    // no certification authority; the activation test invokes the actual cut owner.
    let asset = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("privacy", "universal").unwrap(),
        "ironwood_core".parse().unwrap(),
    );
    let owner = crate::smartcontracts::isi::sorafs_bootstrap::tests::account(1);
    let key = AssetId::with_scope(asset, owner, AssetBalanceScope::Global);
    let value = AssetValue::new(7u32.into());
    let changed = AssetValue::new(8u32.into());
    let mut snapshot = WorldStateSnapshotV1 {
        schema_hash: ironwood_world_schema_v1(),
        entries: Vec::new(),
    };
    require_balance(&snapshot, &key, None).unwrap();
    assert!(require_balance(&snapshot, &key, Some(&value)).is_err());
    snapshot.entries.push(WorldStateSnapshotEntryV1 {
        field_id: "world.assets".into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(hash_value(&key).unwrap()),
        value_hash: hash_value(&value).unwrap(),
    });
    require_balance(&snapshot, &key, Some(&value)).unwrap();
    assert!(require_balance(&snapshot, &key, None).is_err());
    assert!(require_balance(&snapshot, &key, Some(&changed)).is_err());
}

#[test]
fn global_scope_reader_retains_original_refusal() {
    let global = crate::sumeragi::lanes::routing::test_support::world(SumeragiRootScope::Global);
    let view = global.view();
    let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    assert!(matches!(
        norito::with_decode_limits_scope(zero, || require_global(&view)),
        Err(WorldStateSnapshotError::RootScopeRead(_))
    ));
    require_global(&view).unwrap();
    assert!(matches!(
        require_global(&crate::state::World::new().view()),
        Err(WorldStateSnapshotError::Invalid(_))
    ));
}
