//! Actual NFT/RWA encoding Results retain every native reader and the final State fence.
use super::*;
use crate::{
    kura::Kura, query::store::LiveQueryStore,
    state::authority_registry::grouped_ownership::nft_rwa_test_support as fixture,
};
fn state() -> State {
    State::new_for_testing(
        fixture::fixture(),
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
fn nft_held_encoding_result_retains_three_native_and_final_state_fences() {
    for changed in 0..6 {
        let state = state();
        let generation = state.state_view_generation();
        let checked = CheckedNfts::capture(&state.world, 732).unwrap();
        let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
            "world.nfts",
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
            1 => state.world.nfts.block().commit(),
            2 => state.world.nfts_by_owner.block().commit(),
            3 => state.world.nfts_by_domain.block().commit(),
            4 => {
                let mut publication = state.state_view_publication();
                let guard = publication.begin();
                assert!(
                    finish_nfts_encoding(&state, generation, checked, snapshot)
                        .unwrap()
                        .is_none()
                );
                drop(guard);
                continue;
            }
            5 => {
                let mut publication = state.state_view_publication();
                drop(publication.begin());
            }
            _ => unreachable!(),
        }
        let result = finish_nfts_encoding(&state, generation, checked, snapshot);
        if changed == 0 {
            assert_eq!(result.err(), Some(LeafError::RowLimit));
        } else {
            assert!(result.unwrap().is_none());
        }
    }
}
#[test]
fn rwa_held_encoding_result_retains_four_native_and_final_state_fences() {
    for changed in 0..7 {
        let state = state();
        let generation = state.state_view_generation();
        let checked = CheckedRwas::capture(&state.world, 1482).unwrap();
        let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
            "world.rwas",
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
            1 => state.world.rwas.block().commit(),
            2 => state.world.rwas_by_owner.block().commit(),
            3 => state.world.rwas_by_status.block().commit(),
            4 => state.world.rwas_by_frozen.block().commit(),
            5 => {
                let mut publication = state.state_view_publication();
                let guard = publication.begin();
                assert!(
                    finish_rwas_encoding(&state, generation, checked, snapshot)
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
        let result = finish_rwas_encoding(&state, generation, checked, snapshot);
        if changed == 0 {
            assert_eq!(result.err(), Some(LeafError::RowLimit));
        } else {
            assert!(result.unwrap().is_none());
        }
    }
}
fn pool_refusal_and_retry(rwa: bool) {
    let _pin = crossbeam_epoch::pin();
    let state = state();
    let pool = state.ivm_execution_budget();
    let baseline = pool.reserved_bytes();
    let original_limit = pool.limit_bytes();
    pool.set_limit_bytes(0);
    let capture = |limits| {
        if rwa {
            capture_rwas_once(&state, limits)
        } else {
            capture_nfts_once(&state, limits)
        }
    };
    assert!(matches!(
        capture(limits()),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    assert_eq!(pool.reserved_bytes(), baseline);
    pool.set_limit_bytes(original_limit);
    let snapshot = capture(limits()).unwrap().unwrap();
    assert_eq!(
        snapshot.table_id(),
        if rwa { "world.rwas" } else { "world.nfts" }
    );
    assert_eq!(snapshot.row_count(), 1);
    assert!(pool.reserved_bytes() > baseline);
    let retained = std::sync::Arc::new(snapshot);
    let last = retained.clone();
    drop(retained);
    assert!(pool.reserved_bytes() > baseline);
    drop(last);
    assert_eq!(pool.reserved_bytes(), baseline);
}
#[test]
fn nft_encoding_refusal_retry_last_output_drop_use_original_state_pool() {
    pool_refusal_and_retry(false);
}
#[test]
fn rwa_encoding_refusal_retry_last_output_drop_use_original_state_pool() {
    pool_refusal_and_retry(true);
}
