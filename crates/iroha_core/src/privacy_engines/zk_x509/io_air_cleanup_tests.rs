//! Actual private I/O table cleanup and stable-order parity controls.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection::{
    ErasureObservationV1, observe_v1,
};

fn assert_cleared(observations: &[ErasureObservationV1], minimum_cells: usize) {
    assert!(observations.iter().map(|value| value.cells).sum::<usize>() >= minimum_cells);
    assert!(observations.iter().any(|value| value.nonzero_before > 0));
    assert!(observations.iter().all(|value| value.nonzero_after == 0));
}

#[test]
fn io_constructor_in_place_sort_matches_original_stable_consumer_order() {
    let mut witnesses = tests::valid_witnesses();
    let consumers = vec![
        ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::Sha256,
            instance: 0,
        },
        ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::Sha256,
            instance: 1,
        },
        ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::P256,
            instance: 0,
        },
        ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::CaAccumulator,
            instance: 0,
        },
        ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::Projection,
            instance: 0,
        },
    ];
    witnesses[0].declaration.consumers = consumers.clone();
    witnesses[0].consumer_values = vec![witnesses[0].producer_value.clone(); consumers.len()];
    let (_, execution, sorted) = build_zk_x509_io_base_tables_v1(&witnesses).unwrap();
    let mut reference = PrivateTableV1::new(execution.to_vec(), zeroize_io_accesses_v1);
    reference.sort_by_key(|access| (access.channel.0, access.offset.0, access.is_write != F::ONE));
    assert_eq!(sorted.as_slice(), reference.as_slice());
    for offset in 0..witnesses[0].producer_value.len() {
        let reads: Vec<_> = sorted
            .iter()
            .filter(|access| {
                access.channel == F::ZERO
                    && access.offset == F(offset as u64)
                    && access.is_write == F::ZERO
            })
            .map(|access| access.endpoint)
            .collect();
        assert_eq!(reads, consumers);
    }
}

#[test]
fn io_constructor_clears_partial_public_failure_and_complete_sorted_failure() {
    let mut witnesses = tests::valid_witnesses();
    witnesses[2].consumer_values[0][9] ^= 1;
    let (result, observations) = observe_v1(|| build_zk_x509_io_base_tables_v1(&witnesses));
    assert!(matches!(result, Err(ZkX509IoAirErrorV1::PublicInput)));
    assert_cleared(&observations, 100 * 4);

    let mut witnesses = tests::valid_witnesses();
    witnesses[0].consumer_values[0][0] ^= 1;
    let (result, observations) = observe_v1(|| build_zk_x509_io_base_tables_v1(&witnesses));
    assert!(matches!(result, Err(ZkX509IoAirErrorV1::SortedMemory)));
    assert_cleared(&observations, 158 * 8);
}

#[test]
fn io_constructor_returned_tables_clear_on_caller_success_and_unwind() {
    let witnesses = tests::valid_witnesses();
    for unwind in [false, true] {
        let (result, observations) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let (declarations, execution, sorted) =
                    build_zk_x509_io_base_tables_v1(&witnesses).unwrap();
                assert_eq!(execution.len(), 158);
                assert_eq!(sorted.len(), 158);
                validate_execution_topology_v1(&declarations, &execution).unwrap();
                validate_sorted_v1(&declarations, &sorted).unwrap();
                if unwind {
                    panic!("injected after base table construction");
                }
                drop((execution, sorted));
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_cleared(&observations, 158 * 8);
    }
}

#[test]
fn io_permutation_and_complete_owner_clear_on_success_failure_and_unwind() {
    let witnesses = tests::valid_witnesses();
    let (_, execution, sorted) = build_zk_x509_io_base_tables_v1(&witnesses).unwrap();
    let ((), observations) = observe_v1(|| {
        let rows = build_permutation_rows_v1(&execution, &sorted, tests::challenges()).unwrap();
        validate_permutation_rows_v1(&execution, &sorted, &rows, tests::challenges()).unwrap();
        drop(rows);
    });
    assert_cleared(&observations, 158 * (8 + 4 * IO_PERMUTATION_LANES_V1));

    for mode in 0..3 {
        let (result, observations) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut trace = build_zk_x509_io_trace_v1(&witnesses, tests::challenges()).unwrap();
                if mode == 1 {
                    trace.permutation_rows[7].sorted_product_after[0] = F::ZERO;
                    assert_eq!(
                        trace.validate(tests::challenges()),
                        Err(ZkX509IoAirErrorV1::Permutation)
                    );
                } else {
                    trace.validate(tests::challenges()).unwrap();
                }
                if mode == 2 {
                    panic!("injected after complete private trace construction");
                }
                drop(trace);
            })
        });
        assert_eq!(result.is_err(), mode == 2);
        assert_cleared(&observations, 158 * (16 + 4 * IO_PERMUTATION_LANES_V1));
    }
}

#[test]
fn io_access_erasure_preserves_public_endpoint_topology() {
    let endpoint = ZkX509IoEndpointV1 {
        role: ZkX509IoSegmentRoleV1::Sha256,
        instance: 7,
    };
    let mut access = io_access_v1(3, 5, 0xa7, true, endpoint).unwrap();
    let ((), observations) =
        observe_v1(|| zeroize_io_accesses_v1(core::slice::from_mut(&mut access)));
    assert_eq!(access.channel, F::ZERO);
    assert_eq!(access.offset, F::ZERO);
    assert_eq!(access.value, F::ZERO);
    assert_eq!(access.is_write, F::ZERO);
    assert_eq!(access.endpoint, endpoint);
    assert_cleared(&observations, 4);
}

#[test]
fn io_private_owner_debug_is_redacted() {
    let witnesses = tests::valid_witnesses();
    assert_eq!(
        format!("{:?}", witnesses[0]),
        "ZkX509IoChannelWitnessV1 { <private material redacted> }"
    );
    let trace = build_zk_x509_io_trace_v1(&witnesses, tests::challenges()).unwrap();
    assert_eq!(
        format!("{trace:?}"),
        "ZkX509IoTraceV1 { <private material redacted> }"
    );
}
