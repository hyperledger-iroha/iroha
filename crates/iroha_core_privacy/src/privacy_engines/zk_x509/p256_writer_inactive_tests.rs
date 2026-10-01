//! Public fixed-schedule writer identity parity and mutation controls.

use super::*;

fn challenges() -> P256CrossTraceChallengesV1 {
    P256CrossTraceChallengesV1 {
        lanes: core::array::from_fn(|lane| P256CrossTraceLaneChallengesV1 {
            terms: core::array::from_fn(|term| F((11 + 31 * lane + 3 * term) as u64)),
        }),
    }
}

fn fixed_row(multiplicities: [u16; 2]) -> P256CrossTraceWriterFixedRowV1 {
    P256CrossTraceWriterFixedRowV1 {
        events: core::array::from_fn(|slot| {
            if multiplicities[slot] == 0 {
                P256CrossTraceEventFixedV1::inactive()
            } else {
                P256CrossTraceEventFixedV1::active(P256CrossTraceTagV1 {
                    endpoint: P256CrossTraceEndpointV1::Writer,
                    address: 23 + slot as u32,
                })
            }
        }),
        multiplicity_small: multiplicities.map(|m| F(u64::from(if m <= 2 { m } else { 0 }))),
        multiplicity_64: multiplicities.map(|m| F(u64::from(m == 64))),
        multiplicity_65: multiplicities.map(|m| F(u64::from(m == 65))),
        multiplicity_129: multiplicities.map(|m| F(u64::from(m == 129))),
        boundary: P256CrossTraceBoundaryFixedV1::for_row(0, 2).expect("public boundary"),
    }
}

#[test]
fn writer_inactive_slots_preserve_all_multiplicities_and_boundary_values() {
    let inputs = [
        F::ZERO,
        F::ONE,
        F(0xffff_fffe_ffff_ffff),
        F(0x1234_5678_9abc_def0),
    ];
    for left in [0, 1, 2, 64, 65, 129] {
        for right in [0, 1, 2, 64, 65, 129] {
            let fixed = fixed_row([left, right]);
            for source in inputs {
                for incoming in inputs {
                    let values = [source, source.add(F::ONE)];
                    let running = core::array::from_fn(|lane| incoming.add(F(lane as u64)));
                    assert_eq!(
                        build_writer_row_v1(fixed, values, running, challenges()),
                        direct_writer_row_v1(fixed, values, running, challenges()),
                    );
                }
            }
        }
    }
}

#[test]
fn writer_inactive_gate_preserves_noncanonical_fixed_tuple_behavior() {
    for slot in 0..2 {
        for field in 0..7 {
            for changed in [F::ONE, F(2), F(0xffff_fffe_ffff_ffff)] {
                let mut fixed = fixed_row([0, 0]);
                match field {
                    0 => fixed.events[slot].active = changed,
                    1 => fixed.events[slot].endpoint = changed,
                    2 => fixed.events[slot].address = changed,
                    3 => fixed.multiplicity_small[slot] = changed,
                    4 => fixed.multiplicity_64[slot] = changed,
                    5 => fixed.multiplicity_65[slot] = changed,
                    6 => fixed.multiplicity_129[slot] = changed,
                    _ => unreachable!(),
                }
                assert_eq!(
                    build_writer_row_v1(fixed, [F(7), F(29)], [F(3); 4], challenges()),
                    direct_writer_row_v1(fixed, [F(7), F(29)], [F(3); 4], challenges()),
                );
            }
        }
    }
}

#[test]
#[ignore = "streams both complete public writer schedules against the direct arithmetic oracle"]
fn writer_inactive_fast_path_matches_every_canonical_native_row() {
    for (role, expected_active) in [
        (P256EcdsaRoleV1::CertificateOrCrl, 14_240),
        (P256EcdsaRoleV1::WalletOwnership, 14_240),
    ] {
        let schedule =
            P256CrossTraceWriterSourceFixedV1::compile_v1(role).expect("public schedule");
        let mut running = [F::ONE; 4];
        let mut active = 0;
        for row_index in 0..P256_CROSS_TRACE_VALUE_BUS_TRACE_SIZE_V1 {
            let fixed = schedule.row_v1(row_index).expect("fixed native row");
            active += fixed
                .events
                .iter()
                .filter(|event| event.active == F::ONE)
                .count();
            let values = [
                F((row_index * 37 + 11) as u64),
                F((row_index * 53 + 17) as u64),
            ];
            let expected = direct_writer_row_v1(fixed, values, running, challenges());
            let actual = build_writer_row_v1(fixed, values, running, challenges());
            assert_eq!(actual, expected, "role={role:?}, row={row_index}");
            running = core::array::from_fn(|lane| {
                actual.product_before[1][lane].mul(actual.selected_power[1][lane])
            });
        }
        assert_eq!(active, expected_active);
    }
}

// Independent eager arithmetic oracle retained from the pre-optimization owner.
fn direct_writer_row_v1(
    fixed: P256CrossTraceWriterFixedRowV1,
    source_values: [F; P256_VALUE_BUS_FACTORS_PER_PACKED_ROW_V1],
    product_before: [F; P256_CROSS_TRACE_LANES_V1],
    challenges: P256CrossTraceChallengesV1,
) -> P256CrossTraceWriterAuxRowV1 {
    let event_values =
        core::array::from_fn(|slot| fixed.events[slot].active.mul(source_values[slot]));
    let mut powers = [[[F::ONE; P256_CROSS_TRACE_WRITER_POWERS_V1]; P256_CROSS_TRACE_LANES_V1];
        P256_VALUE_BUS_FACTORS_PER_PACKED_ROW_V1];
    let mut selected_power =
        [[F::ONE; P256_CROSS_TRACE_LANES_V1]; P256_VALUE_BUS_FACTORS_PER_PACKED_ROW_V1];
    let mut product_states =
        [[F::ONE; P256_CROSS_TRACE_LANES_V1]; P256_VALUE_BUS_FACTORS_PER_PACKED_ROW_V1];
    let mut running = product_before;
    for slot in 0..P256_VALUE_BUS_FACTORS_PER_PACKED_ROW_V1 {
        product_states[slot] = running;
        for lane in 0..P256_CROSS_TRACE_LANES_V1 {
            powers[slot][lane][0] = compress_event_v1(
                fixed.events[slot],
                event_values[slot],
                challenges.lanes[lane],
            );
            for power in 1..P256_CROSS_TRACE_WRITER_POWERS_V1 {
                powers[slot][lane][power] =
                    powers[slot][lane][power - 1].mul(powers[slot][lane][power - 1]);
            }
            selected_power[slot][lane] = F::ONE
                .sub(fixed.events[slot].active)
                .add(
                    fixed.multiplicity_small[slot]
                        .mul(F(2).sub(fixed.multiplicity_small[slot]))
                        .mul(powers[slot][lane][0]),
                )
                .add(
                    fixed.multiplicity_small[slot]
                        .mul(fixed.multiplicity_small[slot].sub(F::ONE))
                        .mul(F(0x7fff_ffff_8000_0001))
                        .mul(powers[slot][lane][1]),
                )
                .add(fixed.multiplicity_64[slot].mul(powers[slot][lane][6]))
                .add(
                    fixed.multiplicity_65[slot]
                        .mul(powers[slot][lane][6])
                        .mul(powers[slot][lane][0]),
                )
                .add(
                    fixed.multiplicity_129[slot]
                        .mul(powers[slot][lane][7])
                        .mul(powers[slot][lane][0]),
                );
            running[lane] = running[lane].mul(selected_power[slot][lane]);
        }
    }
    P256CrossTraceWriterAuxRowV1 {
        event_values,
        powers,
        selected_power,
        product_before: product_states,
        terminal: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
    }
}

fn arbitrary_writer_residues<A: PolynomialAirFieldV1>(mut cell: impl FnMut() -> A) -> Vec<A> {
    let fixed = P256CrossTraceWriterFixedRowV1 {
        events: core::array::from_fn(|_| P256CrossTraceEventFixedV1 {
            active: cell(),
            endpoint: cell(),
            address: cell(),
        }),
        multiplicity_small: core::array::from_fn(|_| cell()),
        multiplicity_64: core::array::from_fn(|_| cell()),
        multiplicity_65: core::array::from_fn(|_| cell()),
        multiplicity_129: core::array::from_fn(|_| cell()),
        boundary: P256CrossTraceBoundaryFixedV1 {
            first: cell(),
            last: cell(),
            continuation: cell(),
        },
    };
    let sources = core::array::from_fn(|_| cell());
    let mut aux = || P256CrossTraceWriterAuxRowV1 {
        event_values: core::array::from_fn(|_| cell()),
        powers: core::array::from_fn(|_| {
            core::array::from_fn(|_| core::array::from_fn(|_| cell()))
        }),
        selected_power: core::array::from_fn(|_| core::array::from_fn(|_| cell())),
        product_before: core::array::from_fn(|_| core::array::from_fn(|_| cell())),
        terminal: core::array::from_fn(|_| cell()),
    };
    let current = aux();
    let next = aux();
    evaluate_zk_x509_p256_cross_trace_writer_row_constraints_v1(
        fixed,
        sources,
        &current,
        &next,
        challenges(),
    )
}

#[test]
fn complete_writer_residues_have_degree_three_including_small_code_and_accumulators() {
    let mut samples = (0..6)
        .map(|sample| {
            let mut index = 0;
            arbitrary_writer_residues(|| {
                index += 1;
                F(index + 1).add(F(sample).mul(F(index + 3)))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        samples[0].len(),
        P256_CROSS_TRACE_WRITER_CONSTRAINT_COUNT_V1
    );
    for order in 1..=4 {
        samples = samples
            .windows(2)
            .map(|pair| {
                pair[1]
                    .iter()
                    .zip(&pair[0])
                    .map(|(&right, &left)| right.sub(left))
                    .collect()
            })
            .collect();
        if order == 3 {
            assert!(samples.iter().flatten().any(|&x| x != F::ZERO));
        }
    }
    assert!(samples.iter().flatten().all(|&x| x == F::ZERO));
}

#[test]
fn complete_writer_fp4_residues_match_ten_sample_polynomial_lifting_without_boolean_shortcuts() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    let w = E::canonical([0, 1, 0, 0]).unwrap();
    let mut index = 0_u64;
    let actual = arbitrary_writer_residues(|| {
        index += 1;
        E::canonical([index + 1, index + 3, index + 5, index + 7]).unwrap()
    });
    let mut expected = vec![E::ZERO; actual.len()];
    for sample in 0..10 {
        let t = F(sample);
        let mut index = 0_u64;
        let scalar = arbitrary_writer_residues(|| {
            index += 1;
            [index + 1, index + 3, index + 5, index + 7]
                .into_iter()
                .rev()
                .fold(F::ZERO, |sum, c| sum.mul(t).add(F(c)))
        });
        let mut numerator = E::ONE;
        let mut denominator = F::ONE;
        for other in 0..10 {
            if other != sample {
                numerator = numerator.mul(w.sub(E::from_base(F(other))));
                denominator = denominator.mul(t.sub(F(other)));
            }
        }
        let weight = numerator.mul_base(denominator.inv().unwrap());
        for (out, value) in expected.iter_mut().zip(scalar) {
            *out = out.add(weight.mul_base(value));
        }
    }
    assert_eq!(actual, expected);
    assert!(actual.iter().any(|x| x.coefficients()[1] != F::ZERO));
}

#[test]
fn small_writer_codes_select_identity_factor_and_square_in_both_fields() {
    use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;
    assert_eq!(F(2).mul(F(0x7fff_ffff_8000_0001)), F::ONE);
    for code in 0..=2 {
        for value in [F::ZERO, F::ONE, F(65535)] {
            let fixed = fixed_row([code, 0]);
            let row = build_writer_row_v1(fixed, [value, F::ZERO], [F::ONE; 4], challenges());
            for lane in 0..4 {
                let expected = match code {
                    0 => F::ONE,
                    1 => row.powers[0][lane][0],
                    2 => row.powers[0][lane][0].mul(row.powers[0][lane][0]),
                    _ => unreachable!(),
                };
                assert_eq!(row.selected_power[0][lane], expected);
                let f = E::from_base(F(u64::from(code)));
                let factor = E::canonical([value.0, 3, 5, 7]).unwrap();
                let selected = E::ONE
                    .sub(E::from_base(F(u64::from(code != 0))))
                    .add(f.mul(E::from_base(F(2)).sub(f)).mul(factor))
                    .add(
                        f.mul(f.sub(E::ONE))
                            .mul_base(F(0x7fff_ffff_8000_0001))
                            .mul(factor.mul(factor)),
                    );
                assert_eq!(
                    selected,
                    match code {
                        0 => E::ONE,
                        1 => factor,
                        2 => factor.mul(factor),
                        _ => unreachable!(),
                    }
                );
            }
        }
    }
}

#[test]
fn multiplicity_two_accepts_zero_factors_and_still_constrains_selected_power() {
    let fixed = fixed_row([2, 0]);
    let c = challenges();
    let terms = c.lanes[0].terms;
    let value = F::ZERO
        .sub(terms[0].add(terms[1]).add(terms[2].mul(F(23))))
        .mul(terms[3].inv().unwrap());
    let row = build_writer_row_v1(fixed, [value, F::ZERO], [F::ONE; 4], c);
    assert_eq!(row.powers[0][0][0], F::ZERO);
    assert_eq!(row.selected_power[0][0], F::ZERO);
    let mut next = row;
    next.product_before[0] =
        core::array::from_fn(|lane| row.product_before[1][lane].mul(row.selected_power[1][lane]));
    assert!(
        evaluate_zk_x509_p256_cross_trace_writer_row_constraints_v1(
            fixed,
            [value, F::ZERO],
            &row,
            &next,
            c
        )
        .iter()
        .all(|x| *x == F::ZERO)
    );
    let mut changed = row;
    changed.selected_power[0][0] = F::ONE;
    assert!(
        evaluate_zk_x509_p256_cross_trace_writer_row_constraints_v1(
            fixed,
            [value, F::ZERO],
            &changed,
            &next,
            c
        )
        .iter()
        .any(|x| *x != F::ZERO)
    );
}
