//! Actual immutable-owner terminal reuse and deliberately corrupted private caches.

use super::*;

#[test]
fn bound_writer_replay_requires_exact_main_registration_and_clears_private_rows() {
    let bound = P256MainBoundSourceV1 {
        signatures: None,
        fixed: None,
        post_base: None,
        terminal_claims: None,
    };
    let execution = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    assert!(matches!(
        bound.value_execution_aux_stream_v1(execution),
        Err(P256AggregateAdapterErrorV1::Phase)
    ));
    for (adapter, local) in [
        (P256MainAdapterV1::ValueBus, 1),
        (P256MainAdapterV1::Arithmetic, 0),
        (P256MainAdapterV1::BindingSink, 0),
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, local).unwrap();
        assert!(matches!(
            bound.value_execution_aux_stream_v1(registration),
            Err(P256AggregateAdapterErrorV1::Topology)
        ));
    }
    let mut row = P256CrossTraceWriterAuxRowV1 {
        event_values: [F(19); 2],
        powers: [[[F(19); 8]; 4]; 2],
        selected_power: [[F(19); 4]; 2],
        product_before: [[F(19); 4]; 2],
        terminal: [F(19); 4],
    };
    zeroize::Zeroize::zeroize(&mut row);
    assert!(
        flatten_writer_aux_v1(row)
            .iter()
            .all(|value| *value == F::ZERO)
    );
    assert!(
        p256_value_aux_replay_scratch_v1()
            >= core::mem::size_of::<P256MainBoundWriterReplayV1<'static>>()
                + core::mem::size_of::<P256ValueExecutionAggregateStreamV1<'static>>()
                + core::mem::size_of::<P256AggregateAuxRowScratchV1<116>>()
    );
}

#[test]
#[ignore = "full native rows for all five signature owners and mutated cached terminals"]
fn bound_execution_terminal_replay_matches_independent_construction_and_rejects_mutations() {
    let mut base = p256_main_base_source_fixture_for_test_v1().unwrap();
    let token = super::tests::main_post_base_v1(83);
    let mut bound = base.bind_v1(token).unwrap();
    let retained = bound.allocated_payload_bytes_v1();
    for index in 0..P256_X5S1_SIGNATURES_V1 {
        let registration =
            P256MainRegistrationV1::new_v1(index, P256MainAdapterV1::ValueBus, 0).unwrap();
        let signature = bound.signature_v1(registration).unwrap();
        let value = signature.value.as_ref().unwrap();
        let mut reference = P256ValueExecutionAggregateStreamV1::new_v1(value).unwrap();
        let mut reused = bound.value_execution_aux_stream_v1(registration).unwrap();
        assert_eq!(reference.terminal_v1(), reused.terminal_v1());
        assert_eq!(
            reference.arithmetic_copy_terminal_v1(),
            reused.arithmetic_copy_terminal_v1()
        );
        assert_eq!(reference.value_terminal_v1(), reused.value_terminal_v1());
        for _ in 0..P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1 {
            let wanted = zeroize::Zeroizing::new(reference.next_aux_row_v1().unwrap().unwrap());
            let actual = zeroize::Zeroizing::new(reused.next_aux_row_v1().unwrap().unwrap());
            assert_eq!(*actual, *wanted);
        }
        assert!(reference.next_aux_row_v1().unwrap().is_none());
        assert!(reused.next_aux_row_v1().unwrap().is_none());
        drop(reference);
        drop(reused);
        let ((), observations) = super::super::private_table::inspection::observe_v1(|| {
            let mut replay = bound.value_execution_aux_stream_v1(registration).unwrap();
            let _row = zeroize::Zeroizing::new(replay.next_aux_row_v1().unwrap().unwrap());
        });
        assert!(observations.iter().any(|item| item.nonzero_before > 0));
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
        assert_eq!(bound.allocated_payload_bytes_v1(), retained);
    }
    let registration = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap();
    bound.post_base = Some(super::tests::main_post_base_v1(84));
    assert!(matches!(
        bound.value_execution_aux_stream_v1(registration),
        Err(P256AggregateAdapterErrorV1::Challenge)
    ));
    bound.post_base = Some(token);
    // Mutate both sides of a public terminal equality so that structural claim
    // checking alone still passes. Actual native accumulation must reject it.
    for writer in [false, true] {
        {
            let claims = &mut bound.terminal_claims.as_mut().unwrap().certificate_or_crl[0];
            if writer {
                claims.cross_sources[0].terminal[0] =
                    claims.cross_sources[0].terminal[0].add(F::ONE);
                claims.cross_sources[1].start[0] = claims.cross_sources[1].start[0].add(F::ONE);
            } else {
                claims.buses.value_arithmetic_copy[0] =
                    claims.buses.value_arithmetic_copy[0].add(F::ONE);
                claims.buses.arithmetic_value_copy[0] =
                    claims.buses.arithmetic_value_copy[0].add(F::ONE);
            }
        }
        bound.ensure_bound_v1().unwrap();
        let mut output =
            zeroize::Zeroizing::new(vec![F(97); P256_VALUE_BUS_AGGREGATE_TRACE_SIZE_V1]);
        assert!(
            bound
                .fill_value_aux_columns_v1(registration, 0, &mut [&mut output])
                .is_err()
        );
        assert!(output.iter().all(|value| *value == F::ZERO));
        {
            let claims = &mut bound.terminal_claims.as_mut().unwrap().certificate_or_crl[0];
            if writer {
                claims.cross_sources[0].terminal[0] =
                    claims.cross_sources[0].terminal[0].sub(F::ONE);
                claims.cross_sources[1].start[0] = claims.cross_sources[1].start[0].sub(F::ONE);
            } else {
                claims.buses.value_arithmetic_copy[0] =
                    claims.buses.value_arithmetic_copy[0].sub(F::ONE);
                claims.buses.arithmetic_value_copy[0] =
                    claims.buses.arithmetic_value_copy[0].sub(F::ONE);
            }
        }
    }
    bound.zeroize_private_v1();
    assert!(bound.private_is_zeroized_v1());
    assert!(matches!(
        bound.value_execution_aux_stream_v1(registration),
        Err(P256AggregateAdapterErrorV1::Phase)
    ));
}
