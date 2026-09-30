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
        multiplicity_one: multiplicities.map(|m| F(u64::from(m == 1))),
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
    for left in [0, 1, 64, 65, 129] {
        for right in [0, 1, 64, 65, 129] {
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
                    3 => fixed.multiplicity_one[slot] = changed,
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
        (P256EcdsaRoleV1::CertificateOrCrl, 14_208),
        (P256EcdsaRoleV1::WalletOwnership, 14_224),
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
                .add(fixed.multiplicity_one[slot].mul(powers[slot][lane][0]))
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
