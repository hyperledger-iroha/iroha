//! Exact native keyed table census and original execution-owner refusal controls.

use super::*;
use crate::state::World;
use iroha_data_model::sumeragi_finality::keyed_state::StateKeyV1;
use norito::codec::Encode as _;
use std::collections::BTreeMap;

#[test]
fn complete_native_table_comparison_rejects_omission_extra_rows_and_replacement() {
    let pool = iroha_allocation::AllocationBudget::new(1 << 20);
    let mut tree = KeyedStateTreeV1::new([3; 32], &pool);
    let first = DefinitionId([1; 32]);
    let second = DefinitionId([2; 32]);
    let rows = BTreeMap::from([(first, 17_u64), (second, 18)]);
    for (id, value) in &rows {
        tree.replace(
            &StateKeyV1 {
                table: "world.runtime_definitions".into(),
                key: id.encode(),
            },
            Some(&value.encode()),
        )
        .unwrap();
    }
    tree.replace(
        &StateKeyV1 {
            table: "world.runtime_selection".into(),
            key: Vec::new(),
        },
        Some(&7_u64.encode()),
    )
    .unwrap();
    let root = tree.root();
    let _all_remaining = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    table(&tree, "world.runtime_definitions", rows.iter()).unwrap();
    cell(&tree, "world.runtime_selection", &7_u64).unwrap();
    assert!(cell(&tree, "world.runtime_selection", &8_u64).is_err());
    assert!(cell(&tree, "world.private_dataspaces", &7_u64).is_err());
    assert!(table(&tree, "world.runtime_definitions", rows.iter().take(1)).is_err());
    assert!(table(&tree, "world.runtime_definitions", rows.iter().rev()).is_err());
    let extra = BTreeMap::from([(first, 17_u64), (second, 18), (DefinitionId([3; 32]), 19)]);
    assert!(table(&tree, "world.runtime_definitions", extra.iter()).is_err());
    let replaced = BTreeMap::from([(first, 17_u64), (second, 19)]);
    assert!(table(&tree, "world.runtime_definitions", replaced.iter()).is_err());
    assert_eq!(tree.root(), root);
    assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
}

#[test]
fn native_keyed_cut_uses_original_execution_and_refuses_restored_or_substituted_census() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
            let mut world = World::new();
            crate::runtime::activate_additions_for_testing(
                &world,
                crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
            )
            .unwrap();
            crate::runtime::keyed_state::install_for_test(&mut world);
            let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap();
            chain.commit_at(2_000, vec![]);
            let state = chain.state();
            let original = state.runtime_retirement_cut().unwrap();
            let native = original.view().native_execution_tip().unwrap();
            let cut = state
                .keyed_versions
                .lock()
                .published_at(native)
                .unwrap()
                .clone();
            let tip = chain.committed(2);
            assert_eq!(
                &cut.execution.as_ref().unwrap().root(),
                tip.commitment().execution.world_state_root.as_ref()
            );
            let foreign = World::new();
            let mut replaced = state.query_view();
            replaced.world = foreign.view();
            assert!(authenticate(state, &replaced, &tip).is_err());
            let mut changed = cut.clone();
            changed.execution = None;
            let originals = state.keyed_versions.lock().install(Some(changed));
            assert!(matches!(
                state.runtime_retirement_cut(),
                Err(RetirementCutError::Invalid(
                    "restored published State has no original execution census"
                ))
            ));
            *state.keyed_versions.lock() = originals;
            let mut changed = cut.clone();
            changed.execution = Some(changed.published.clone());
            assert_ne!(
                changed.execution.as_ref().unwrap().root(),
                cut.execution.as_ref().unwrap().root(),
                "actual publication tail differs from the original execution"
            );
            let originals = state.keyed_versions.lock().install(Some(changed));
            assert!(matches!(
                state.runtime_retirement_cut(),
                Err(RetirementCutError::Invalid(
                    "original keyed schema or execution root differs"
                ))
            ));
            *state.keyed_versions.lock() = originals;
            state.runtime_retirement_cut().unwrap();
            original
                .with_census_scope(|| exact(&7_u64, &7_u64.encode()))
                .unwrap()
                .unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_keyed_comparison_keeps_the_original_codec_refusal() {
    struct Refusing;
    impl norito::SerializePayload for Refusing {
        fn serialize(&self, _: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
            norito::core::reserve_decode_allocation(1)
        }
    }
    let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let original = zero.with(|| exact(&Refusing, &[])).unwrap_err();
    let RetirementCutError::Codec(original) = original else {
        panic!("original codec cause was replaced")
    };
    assert_eq!(
        original.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
}
