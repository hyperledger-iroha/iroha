/// The original vector emitter is an independent schedule oracle for the bounded producer.
mod rlc_streaming_tests {
    use super::super::rlc_streaming;
    use super::*;
    use halo2_base::{ContextCell, EXTERNAL_CELL_TYPE_ID};
    use halo2_proofs::plonk::Assigned;
    use std::collections::BTreeMap;

    /// Compute expected endpoints without calling either row emitter.
    fn endpoint(values: &[u128], challenge: u128) -> u128 {
        let mut coefficients = values
            .iter()
            .map(|value| value % CLAIM_CARRIER_RLC_MODULUS_V1)
            .collect::<Vec<_>>();
        for values in values.chunks(CLAIM_CARRIER_RLC_QUOTIENTS_PER_COEFFICIENT_V1) {
            let mut pack = 0_u128;
            let mut power = 1_u128;
            for (index, value) in values.iter().enumerate() {
                pack += (value / CLAIM_CARRIER_RLC_MODULUS_V1) * power;
                if index + 1 < values.len() {
                    power *= CLAIM_CARRIER_RLC_QUOTIENT_RADIX_V1;
                }
            }
            coefficients.push(pack);
        }
        coefficients
            .into_iter()
            .fold(0, |accumulator, coefficient| {
                claim_rlc_native_step_v1(accumulator, challenge, coefficient)
                    .unwrap()
                    .1
            })
    }

    /// Use distinct virtual coordinates and all quotient classes around the packing boundary.
    fn machine<F: KagemushaPoseidonFieldV1>(
        capacity: usize,
    ) -> KagemushaClaimCarrierRlcMachineV1<F> {
        let mut offset = 0;
        let mut assign = |value: u128| {
            let cell = ContextCell::new(EXTERNAL_CELL_TYPE_ID, 37, offset);
            offset += 1;
            AssignedValue {
                value: Assigned::Trivial(F::from_u128(value)),
                cell: Some(cell),
            }
        };
        let challenge_a = assign(2);
        let challenge_b = assign(3);
        let carriers = std::array::from_fn(|carrier| {
            let choices = [
                0,
                1,
                CLAIM_CARRIER_RLC_MODULUS_V1 - 1,
                CLAIM_CARRIER_RLC_MODULUS_V1,
                2 * CLAIM_CARRIER_RLC_MODULUS_V1,
                u128::MAX,
            ];
            let values = (0..capacity)
                .map(|index| choices[(index + carrier) % choices.len()])
                .collect::<Vec<_>>();
            ClaimRlcCarrierV1 {
                expected_a: assign(endpoint(&values, 2)),
                expected_b: assign(endpoint(&values, 3)),
                values: values.into_iter().map(&mut assign).collect(),
            }
        });
        KagemushaClaimCarrierRlcMachineV1 {
            challenge_a,
            challenge_b,
            carriers,
            use_unknown: false,
        }
    }

    /// Nonsecret coordinates in the exact order used by immediate and deferred copy assignment.
    #[derive(Debug, PartialEq, Eq)]
    enum CopyEvent {
        Virtual {
            bus: usize,
            cell: Option<ContextCell>,
        },
        Pack {
            carrier: usize,
            pack: usize,
            stored: usize,
            loaded: usize,
        },
    }

    fn check_rows<F: KagemushaPoseidonFieldV1>() {
        for capacity in [
            1,
            4,
            79,
            80,
            81,
            160,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ] {
            for unknown in [false, true] {
                let mut machine = machine::<F>(capacity);
                machine.use_unknown = unknown;
                let input_before = format!("{machine:?}");
                let expected = machine.build_rows_with_capacity(capacity).unwrap();
                assert_eq!(
                    expected.len(),
                    2 * (4 + 3 * capacity + 2 * capacity.div_ceil(80))
                );
                let mut expected_copies = Vec::new();
                let mut expected_stores = BTreeMap::new();
                let mut expected_loads = BTreeMap::new();
                for (logical, row) in expected.iter().enumerate() {
                    match row.binding {
                        Some(ClaimRlcBusBindingV1::Virtual(value)) => {
                            expected_copies.push(CopyEvent::Virtual {
                                bus: logical * 2,
                                cell: value.cell,
                            })
                        }
                        Some(ClaimRlcBusBindingV1::PackStore { carrier, pack }) => {
                            assert!(
                                expected_stores
                                    .insert((carrier, pack), logical * 2)
                                    .is_none()
                            );
                        }
                        Some(ClaimRlcBusBindingV1::PackLoad { carrier, pack }) => {
                            assert!(
                                expected_loads
                                    .insert((carrier, pack), logical * 2)
                                    .is_none()
                            );
                        }
                        None => {}
                    }
                }
                for ((carrier, pack), stored) in expected_stores {
                    expected_copies.push(CopyEvent::Pack {
                        carrier,
                        pack,
                        stored,
                        loaded: expected_loads.remove(&(carrier, pack)).unwrap(),
                    });
                }
                assert!(expected_loads.is_empty());
                let mut actual_copies = Vec::new();
                let mut stores = BTreeMap::new();
                let mut loads = BTreeMap::new();
                rlc_streaming::reset_cleanup_counts();
                let count =
                    rlc_streaming::emit_rows_with_capacity(&machine, capacity, |logical, row| {
                        let original = &expected[logical];
                        assert_eq!(
                            row.values, original.values,
                            "logical row {logical}, capacity {capacity}"
                        );
                        assert_eq!(
                            std::mem::discriminant(&row.mode),
                            std::mem::discriminant(&original.mode)
                        );
                        assert_eq!(row.store_pack, original.store_pack);
                        assert_eq!(row.load_pack, original.load_pack);
                        assert_eq!(row.ternary_power, original.ternary_power);
                        assert_eq!(
                            row.fixed_encoding().unwrap(),
                            claim_rlc_fixed_encoding_v1(original).unwrap()
                        );
                        let projected = row.physical_values();
                        let original_projected = claim_rlc_physical_values_v1(original);
                        for (half, values) in projected.iter().enumerate() {
                            for (column, value) in values.iter().enumerate() {
                                assert_eq!(
                                    *value,
                                    original_projected[half][column],
                                    "absolute row {}, column {column}",
                                    logical * 2 + half
                                );
                            }
                        }
                        match (row.binding, original.binding) {
                            (
                                Some(rlc_streaming::Binding::Virtual(cell)),
                                Some(ClaimRlcBusBindingV1::Virtual(value)),
                            ) => {
                                assert_eq!(cell, value.cell);
                                actual_copies.push(CopyEvent::Virtual {
                                    bus: logical * 2,
                                    cell,
                                });
                            }
                            (
                                Some(rlc_streaming::Binding::PackStore { carrier, pack }),
                                Some(ClaimRlcBusBindingV1::PackStore {
                                    carrier: old_carrier,
                                    pack: old_pack,
                                }),
                            ) => {
                                assert_eq!((carrier, pack), (old_carrier, old_pack));
                                assert!(stores.insert((carrier, pack), logical * 2).is_none());
                            }
                            (
                                Some(rlc_streaming::Binding::PackLoad { carrier, pack }),
                                Some(ClaimRlcBusBindingV1::PackLoad {
                                    carrier: old_carrier,
                                    pack: old_pack,
                                }),
                            ) => {
                                assert_eq!((carrier, pack), (old_carrier, old_pack));
                                assert!(loads.insert((carrier, pack), logical * 2).is_none());
                            }
                            (None, None) => {}
                            _ => panic!("binding differs at logical row {logical}"),
                        }
                        Ok(())
                    })
                    .unwrap();
                for ((carrier, pack), stored) in stores {
                    actual_copies.push(CopyEvent::Pack {
                        carrier,
                        pack,
                        stored,
                        loaded: loads.remove(&(carrier, pack)).unwrap(),
                    });
                }
                assert!(loads.is_empty());
                assert_eq!(actual_copies, expected_copies);
                assert_eq!(count, expected.len());
                assert_eq!(rlc_streaming::cleanup_counts(), [count, 2, 2, 0, 0, 1]);
                assert_eq!(
                    format!("{machine:?}"),
                    input_before,
                    "owned input witnesses must survive row cleanup"
                );
            }
        }
    }

    #[test]
    fn streaming_rlc_fp_matches_frozen_rows_physical_tags_and_copy_order() {
        check_rows::<Fp>();
    }

    #[test]
    fn streaming_rlc_fq_matches_frozen_rows_physical_tags_and_copy_order() {
        check_rows::<Fq>();
    }

    fn check_sink_failure<F: KagemushaPoseidonFieldV1>(unwind: bool) {
        let capacity = 81;
        let machine = machine::<F>(capacity);
        let expected = machine.build_rows_with_capacity(capacity).unwrap();
        let carrier_rows = expected.len() / 2;
        let mut failure_rows = vec![
            0,
            1,
            2,
            3,
            4,
            carrier_rows - 2,
            carrier_rows - 1,
            carrier_rows,
            expected.len() - 1,
        ];
        failure_rows.extend(
            expected
                .iter()
                .enumerate()
                .filter_map(|(index, row)| (row.store_pack || row.load_pack).then_some(index)),
        );
        failure_rows.sort_unstable();
        failure_rows.dedup();
        for failure in failure_rows {
            rlc_streaming::reset_cleanup_counts();
            let mut calls = 0;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rlc_streaming::emit_rows_with_capacity(&machine, capacity, |index, _| {
                    assert_eq!(index, calls);
                    calls += 1;
                    if index == failure {
                        if unwind {
                            panic!("injected sink unwind");
                        }
                        return Err(PlonkError::Synthesis);
                    }
                    Ok(())
                })
            }));
            if unwind {
                assert!(outcome.is_err());
            } else {
                assert!(outcome.unwrap().is_err());
            }
            assert_eq!(calls, failure + 1);
            let carriers = failure / carrier_rows + 1;
            assert_eq!(
                rlc_streaming::cleanup_counts(),
                [failure + 1, carriers, carriers, 0, 0, 1]
            );
        }
    }

    #[test]
    fn streaming_rlc_sink_error_stops_prefix_and_cleans_both_fields() {
        check_sink_failure::<Fp>(false);
        check_sink_failure::<Fq>(false);
    }

    #[test]
    fn streaming_rlc_sink_unwind_stops_prefix_and_cleans_both_fields() {
        check_sink_failure::<Fp>(true);
        check_sink_failure::<Fq>(true);
    }

    fn check_invalid<F: KagemushaPoseidonFieldV1>() {
        for kind in 0..5 {
            let mut machine = machine::<F>(4);
            let capacity = match kind {
                0 => 0,
                1 => KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1 + 1,
                2 => {
                    machine.carriers[1].values.pop();
                    4
                }
                3 => {
                    machine.challenge_a.value = Assigned::Trivial(F::ZERO);
                    4
                }
                _ => {
                    machine.challenge_b.value = Assigned::Trivial(F::from_u128(
                        (1_u128 << CLAIM_CARRIER_RLC_CHALLENGE_BITS_V1) + 1,
                    ));
                    4
                }
            };
            rlc_streaming::reset_cleanup_counts();
            let mut calls = 0;
            assert!(
                rlc_streaming::emit_rows_with_capacity(&machine, capacity, |_, _| {
                    calls += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(calls, 0);
            assert_eq!(rlc_streaming::cleanup_counts(), [0; 6]);
        }
        for bad_expected in [false, true] {
            let capacity = 81;
            let mut machine = machine::<F>(capacity);
            let carrier_rows = machine.required_rows_with_capacity(capacity).unwrap() / 4;
            if bad_expected {
                machine.carriers[1].expected_b.value =
                    Assigned::Trivial(F::from_u128(CLAIM_CARRIER_RLC_MODULUS_V1));
            } else {
                machine.carriers[1].values[capacity - 1].value =
                    Assigned::Trivial(F::from_u128(u128::MAX) + F::ONE);
            }
            let expected_calls = if bad_expected {
                2 * carrier_rows - 2
            } else {
                carrier_rows + 2 + 3 * (capacity - 1)
            };
            rlc_streaming::reset_cleanup_counts();
            let mut calls = 0;
            assert!(
                rlc_streaming::emit_rows_with_capacity(&machine, capacity, |index, _| {
                    assert_eq!(index, calls);
                    calls += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(calls, expected_calls);
            assert_eq!(rlc_streaming::cleanup_counts(), [calls, 2, 2, 0, 0, 1]);
        }
    }

    #[test]
    fn streaming_rlc_rejects_preflight_and_late_invalid_inputs_without_retaining_rows() {
        check_invalid::<Fp>();
        check_invalid::<Fq>();
    }

    fn check_physical_cleanup<F: KagemushaPoseidonFieldV1>() {
        for exit in 0..3 {
            rlc_streaming::reset_cleanup_counts();
            let result = std::panic::catch_unwind(|| {
                rlc_streaming::with_physical_guard_for_test::<F>(|| match exit {
                    0 => Ok(()),
                    1 => Err(PlonkError::Synthesis),
                    _ => panic!("injected physical assignment unwind"),
                })
            });
            match exit {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(result.unwrap().is_err()),
                _ => assert!(result.is_err()),
            }
            assert_eq!(rlc_streaming::cleanup_counts(), [0, 0, 0, 1, 0, 0]);
        }
    }

    #[test]
    fn streaming_rlc_physical_guard_cleans_success_error_and_unwind_in_both_fields() {
        check_physical_cleanup::<Fp>();
        check_physical_cleanup::<Fq>();
    }

    include!("rlc_streaming_proof_tests.rs");
}
