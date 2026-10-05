//! Actual repo_agreement encoding refusals retain all native owners and State fences.
use super::*;
use crate::{
    kura::Kura, query::store::LiveQueryStore,
    state::authority_registry::grouped_ownership::repo_agreement_test_support as fixture,
};
fn state() -> State {
    State::new_for_testing(
        fixture::fixture(true),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}
fn limits() -> LeafLimits {
    LeafLimits {
        max_tables: 1,
        max_rows: 2,
        max_payload_bytes: 16384,
        max_ordered_table_bytes: 32768,
        max_streamed_value_bytes: 65536,
    }
}
#[test]
fn repo_encoding_refusal_stays_behind_every_original_native_and_state_fence() {
    for changed in 0..7 {
        let state = state();
        let generation = state.state_view_generation();
        let checked =
            CheckedRepoAgreements::capture(&state.world, fixture::TEST_WORK_ALLOWANCE).unwrap();
        let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
            "world.repo_agreements",
            LeafLimits {
                max_rows: 0,
                ..limits()
            },
            &state.ivm_execution_budget(),
            checked.rows().iter(),
        );
        assert!(matches!(&snapshot, Err(LeafError::RowLimit)));
        match changed {
            0 => {}
            1 => state.world.repo_agreements.block().commit(),
            2 => state.world.repo_agreements_by_initiator.block().commit(),
            3 => state.world.repo_agreements_by_counterparty.block().commit(),
            4 => state.world.repo_agreements_by_custodian.block().commit(),
            5 => {
                let mut publication = state.state_view_publication();
                let guard = publication.begin();
                assert!(
                    finish_repo_agreements_encoding(&state, generation, checked, snapshot)
                        .unwrap()
                        .is_none()
                );
                drop(guard);
                continue;
            }
            6 => {
                let mut publication = state.state_view_publication();
                drop(publication.begin());
            }
            _ => unreachable!(),
        }
        let result = finish_repo_agreements_encoding(&state, generation, checked, snapshot);
        if changed == 0 {
            assert_eq!(result.err(), Some(LeafError::RowLimit));
        } else {
            assert!(result.unwrap().is_none());
        }
    }
}
#[test]
fn repo_actual_encoding_refusal_and_retry_use_original_state_pool() {
    let _pin = crossbeam_epoch::pin();
    let state = state();
    let pool = state.ivm_execution_budget();
    let baseline = pool.reserved_bytes();
    let limit = pool.limit_bytes();
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_repo_agreements_once(&state, limits()),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    assert_eq!(pool.reserved_bytes(), baseline);
    pool.set_limit_bytes(limit);
    let snapshot = capture_repo_agreements_once(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.table_id(), "world.repo_agreements");
    assert_eq!(snapshot.row_count(), 1);
    assert!(pool.reserved_bytes() > baseline);
    drop(snapshot);
    assert_eq!(pool.reserved_bytes(), baseline);
}
