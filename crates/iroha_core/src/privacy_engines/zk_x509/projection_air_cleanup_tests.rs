//! Actual-cell clearing across projection construction, transfer and unwind.

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
fn projection_constructor_clears_complete_rows_on_late_digest_failure() {
    let (statement, witness) = tests::fixture();
    let (specs, mut invocations) = fixed_specs_v1(&statement).unwrap();
    let fixed = compile_fixed_trace_v1(&specs).unwrap();
    // The first digest has already been retained when the second one fails.
    invocations[1].expected_digest[0] ^= 1;
    let (result, observations) = observe_v1(|| build_base_trace_v1(&fixed, &invocations, &witness));
    assert!(matches!(
        result,
        Err(ZkX509ProjectionAirErrorV1::ProjectionMismatch)
    ));
    assert_cleared(
        &observations,
        fixed.rows.len() * ZK_X509_PROJECTION_BASE_WIDTH_V1 + ZK_X509_PROJECTION_HASH_SLOTS_V1,
    );
    assert!(observations.iter().any(|value| {
        value.cells == ZK_X509_PROJECTION_HASH_SLOTS_V1 && value.nonzero_before == 1
    }));
}

#[test]
fn projection_constructor_transfers_rows_and_clears_temporaries_on_success_and_unwind() {
    let (statement, witness) = tests::fixture();
    let (specs, invocations) = fixed_specs_v1(&statement).unwrap();
    let fixed = compile_fixed_trace_v1(&specs).unwrap();
    for unwind in [false, true] {
        let (result, observations) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let (base, messages, digests) =
                    build_base_trace_v1(&fixed, &invocations, &witness).unwrap();
                assert_eq!(base.rows.len(), ZK_X509_PROJECTION_TRACE_SIZE_V1);
                assert!(base.rows.iter().flatten().any(|value| *value != F::ZERO));
                for (index, invocation) in invocations.iter().enumerate() {
                    assert!(messages[index].capacity() >= ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1);
                    if invocation.active {
                        assert!(!messages[index].is_empty());
                        assert_eq!(digests[index], invocation.expected_digest);
                    }
                }
                if unwind {
                    panic!("injected after private material construction");
                }
                drop((base, messages, digests));
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_cleared(
            &observations,
            fixed.rows.len() * ZK_X509_PROJECTION_BASE_WIDTH_V1,
        );
    }
}

#[test]
fn projection_channel_construction_clears_earlier_private_channels_on_error() {
    let (statement, witness) = tests::fixture();
    let (_, invocations) = fixed_specs_v1(&statement).unwrap();
    // Chain SPKIs and serial/attribute channels exist before this missing
    // message is rejected. Their real live byte cells must be cleared.
    let (result, observations) =
        observe_v1(|| build_io_channels_v1(&statement, &witness, &invocations, &[], &[]));
    assert!(matches!(result, Err(ZkX509ProjectionAirErrorV1::Topology)));
    assert_cleared(&observations, 3 * ZK_X509_PROJECTION_SPKI_DER_BYTES_V1);
}

fn private_channel(value: u8) -> ZkX509ProjectionIoChannelV1 {
    ZkX509ProjectionIoChannelV1 {
        producer: endpoint(ZkX509IoSegmentRoleV1::Projection, 0),
        consumers: vec![
            endpoint(ZkX509IoSegmentRoleV1::Sha256, 0),
            endpoint(ZkX509IoSegmentRoleV1::P256, 0),
        ],
        value: vec![value; 19],
        public_value: None,
    }
}

#[test]
fn projection_channel_transfer_preserves_values_and_clears_partial_error_and_unwind() {
    let ((), observations) = observe_v1(|| {
        let witness = private_channel(0xa5).into_witness(7).unwrap();
        assert_eq!(witness.declaration.channel, 7);
        assert_eq!(witness.producer_value, vec![0xa5; 19]);
        assert_eq!(witness.consumer_values, vec![vec![0xa5; 19]; 2]);
        drop(witness);
    });
    assert_cleared(&observations, 3 * 19);

    let (result, observations) = observe_v1(|| {
        projection_io_witnesses_v1(
            vec![
                private_channel(0x31),
                private_channel(0x32),
                private_channel(0x33),
            ],
            u32::MAX,
        )
    });
    assert!(matches!(result, Err(ZkX509ProjectionAirErrorV1::Resource)));
    // The transferred first channel and the untransferred iterator tail clear.
    assert_cleared(&observations, 5 * 19);

    let (result, observations) = observe_v1(|| {
        std::panic::catch_unwind(|| {
            let _channels = vec![private_channel(0x71), private_channel(0x72)];
            panic!("injected before channel transfer");
        })
    });
    assert!(result.is_err());
    assert_cleared(&observations, 2 * 19);
}

#[test]
fn projection_auxiliary_clears_success_failure_and_unwind() {
    let (statement, witness) = tests::fixture();
    let trace = build_zk_x509_projection_trace_v1(&statement, &witness).unwrap();
    let mut wrong_fixed = trace.fixed.clone();
    wrong_fixed.copy_sigma[0] = wrong_fixed.copy_sigma[0].add(F::ONE);
    let (result, observations) = observe_v1(|| {
        build_zk_x509_projection_aux_trace_v1(&trace.base, &wrong_fixed, tests::challenges())
    });
    assert!(matches!(
        result,
        Err(ZkX509ProjectionAirErrorV1::Constraint)
    ));
    assert_cleared(
        &observations,
        ZK_X509_PROJECTION_TRACE_SIZE_V1 * ZK_X509_PROJECTION_AUX_WIDTH_V1,
    );
    for unwind in [false, true] {
        let (result, observations) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let aux = build_zk_x509_projection_aux_trace_v1(
                    &trace.base,
                    &trace.fixed,
                    tests::challenges(),
                )
                .unwrap();
                assert_eq!(aux.rows.len(), ZK_X509_PROJECTION_TRACE_SIZE_V1);
                if unwind {
                    panic!("injected after auxiliary construction");
                }
                drop(aux);
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_cleared(
            &observations,
            ZK_X509_PROJECTION_TRACE_SIZE_V1 * ZK_X509_PROJECTION_AUX_WIDTH_V1,
        );
    }
}

#[test]
fn projection_extracted_witness_clears_nested_buffers_and_redacts_debug() {
    let (_, witness) = tests::fixture();
    let expected_cells = witness.chain_spki_der.iter().map(Vec::len).sum::<usize>()
        + witness.leaf_serial.len()
        + witness
            .disclosed_attribute_values
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + witness.attribute_salts.len();
    assert_eq!(
        format!("{witness:?}"),
        "ZkX509ProjectionWitnessV1 { <private material redacted> }"
    );
    for unwind in [false, true] {
        let owned = witness.clone();
        let (result, observations) = observe_v1(|| {
            std::panic::catch_unwind(move || {
                let _owned = owned;
                if unwind {
                    panic!("injected with extracted projection witness");
                }
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_cleared(&observations, expected_cells);
    }
    assert!(
        !witness.leaf_serial.is_empty(),
        "borrowed source remains intact"
    );
}
