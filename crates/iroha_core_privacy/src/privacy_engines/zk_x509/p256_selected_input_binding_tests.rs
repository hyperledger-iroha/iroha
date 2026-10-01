//! Public polynomial controls for selected-byte ownership in the existing cross-trace bus.

use super::super::p256_cross_trace_bus::{P256CrossTraceLaneChallengesV1, build_regular_row_v1};
use super::super::p256_external_binding_air::p256_selected_input_writer_id_v1;
use super::*;
use crate::privacy_engines::transparent_stark::GoldilocksFp4V1 as E;

fn challenges() -> P256CrossTraceChallengesV1 {
    P256CrossTraceChallengesV1 {
        lanes: core::array::from_fn(|lane| P256CrossTraceLaneChallengesV1 {
            terms: core::array::from_fn(|term| F((17 + 31 * lane + 3 * term) as u64)),
        }),
    }
}

fn byte_row(byte: usize, active: bool) -> [F; P256_BINDING_SINK_BASE_WIDTH_V1] {
    let dummy =
        p256_input_selection_byte_v1(&ZK_X509_P256_OPTIONAL_CERTIFICATE_DUMMY_V1, byte).unwrap();
    let real = if active {
        ((byte * 73 + 19) % 256) as u8
    } else {
        p256_inactive_real_byte_v1(byte).unwrap()
    };
    let selected = if active { real } else { dummy };
    let mut base = [F::ZERO; P256_BINDING_SINK_BASE_WIDTH_V1];
    base[SINK_SELECTION_ACTIVE_BASE] = F(u64::from(active));
    base[SINK_SELECTION_REAL_BASE] = F(u64::from(real));
    base[SINK_SELECTION_SELECTED_BASE] = F(u64::from(selected));
    write_byte_bits_v1(
        &mut base[SINK_SELECTION_REAL_BITS_BASE..SINK_SELECTION_REAL_BITS_BASE + 8],
        real,
    );
    write_byte_bits_v1(
        &mut base[SINK_SELECTION_SELECTED_BITS_BASE..SINK_SELECTION_SELECTED_BITS_BASE + 8],
        selected,
    );
    base
}

#[test]
fn selected_input_schedule_preserves_legacy_events_and_has_exact_fixed_limb_owners() {
    for role in [
        P256EcdsaRoleV1::CertificateOrCrl,
        P256EcdsaRoleV1::WalletOwnership,
    ] {
        let schedule = P256CrossTraceSinkFixedV1::compile_v1(role).unwrap();
        let legacy = super::super::p256_external_binding_air::compile_zk_x509_p256_external_cross_sources_v1(role).unwrap();
        assert!(legacy.len() < P256_INPUT_SELECTION_ROW_START_V1);
        for (row, sources) in legacy.iter().enumerate() {
            let actual = schedule.row_v1(row).unwrap();
            for (slot, source) in sources.iter().enumerate() {
                match source {
                    Some(source) => {
                        assert_eq!(
                            actual.product.events[2 * slot],
                            active_cross_event_v1(
                                P256CrossTraceEndpointV1::Writer,
                                source.writer_id.0 as usize * 16 + source.writer_limb as usize
                            )
                            .unwrap()
                        );
                        match source.external {
                            super::super::p256_external_binding_air::P256ExternalBindingCrossExternalSourceV1::Dynamic { address } =>
                                assert_eq!(actual.product.events[2 * slot + 1], active_cross_event_v1(P256CrossTraceEndpointV1::External, address as usize).unwrap()),
                            super::super::p256_external_binding_air::P256ExternalBindingCrossExternalSourceV1::Constant { value } => {
                                assert_eq!(actual.product.events[2 * slot + 1], P256CrossTraceEventFixedV1::inactive());
                                assert_eq!(actual.constant_value[slot], value);
                            }
                        }
                    }
                    None => assert_eq!(
                        &actual.product.events[2 * slot..2 * slot + 2],
                        &[P256CrossTraceEventFixedV1::inactive(); 2]
                    ),
                }
            }
        }
        let writers = P256CrossTraceWriterSourceFixedV1::compile_v1(role).unwrap();
        for (word, id) in [47, 48, 52, 53].into_iter().enumerate() {
            for limb in 0..16 {
                // Native execution appends initial writers in local rows48..63
                // of the writer ID's64-row segment, packed two factors per row.
                let ordinal = id * 64 + 48 + limb;
                let fixed = writers.row_v1(ordinal / 2).unwrap();
                let slot = ordinal % 2;
                assert_eq!(
                    fixed.events[slot],
                    active_cross_event_v1(P256CrossTraceEndpointV1::Writer, id * 16 + limb)
                        .unwrap()
                );
                if word < 2 {
                    assert_eq!(fixed.multiplicity_65[slot], F::ONE);
                    assert_eq!(fixed.multiplicity_small[slot], F::ZERO);
                } else {
                    assert_eq!(
                        fixed.multiplicity_small[slot],
                        F(1 + u64::from(word == 3 && role == P256EcdsaRoleV1::WalletOwnership))
                    );
                }
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for byte in 0..P256_INPUT_SELECTION_BYTES_V1 {
            let fixed = schedule
                .row_v1(P256_INPUT_SELECTION_ROW_START_V1 + byte)
                .unwrap();
            assert_eq!(fixed.active, [F::ZERO; 3]);
            assert_eq!(
                &fixed.product.events[1..],
                &[P256CrossTraceEventFixedV1::inactive(); 5]
            );
            if byte % 2 == 0 {
                let word = byte / 32;
                let limb = 15 - (byte % 32) / 2;
                let (endpoint, address) = if word < 4 {
                    (
                        P256CrossTraceEndpointV1::Writer,
                        [47, 48, 52, 53][word] * 16 + limb,
                    )
                } else {
                    (P256CrossTraceEndpointV1::DigestInput, limb)
                };
                assert_eq!(
                    fixed.product.events[0],
                    active_cross_event_v1(endpoint, address).unwrap()
                );
                assert!(seen.insert((word, limb)));
            } else {
                assert_eq!(
                    fixed.product.events[0],
                    P256CrossTraceEventFixedV1::inactive()
                );
            }
        }
        assert_eq!(seen.len(), 80);
        for row in [
            P256_INPUT_SELECTION_ROW_START_V1 - 1,
            P256_INPUT_SELECTION_SELECTOR_ROW_V1,
            P256_BINDING_SINK_AGGREGATE_TRACE_SIZE_V1 - 1,
        ] {
            assert_eq!(
                schedule.row_v1(row).unwrap().product.events,
                [P256CrossTraceEventFixedV1::inactive(); 6]
            );
        }
    }
    for (word, id) in [47, 48, 52, 53].into_iter().enumerate() {
        assert_eq!(
            p256_selected_input_writer_id_v1(word).unwrap().unwrap().0,
            id
        );
    }
    assert_eq!(p256_selected_input_writer_id_v1(4).unwrap(), None);
    assert!(p256_selected_input_writer_id_v1(5).is_err());
    assert!(p256_selected_input_writer_id_v1(usize::MAX).is_err());
    assert!(
        p256_input_selection_byte_v1(&ZK_X509_P256_OPTIONAL_CERTIFICATE_DUMMY_V1, 160).is_err()
    );
    assert!(
        p256_input_selection_byte_v1(&ZK_X509_P256_OPTIONAL_CERTIFICATE_DUMMY_V1, usize::MAX)
            .is_err()
    );
}

#[test]
fn every_selected_pair_binds_actual_opened_bytes_in_f_and_fp4_with_optional_selection() {
    for active in [false, true] {
        let fixed_provider = P256BindingSinkFixedProviderV1::new_with_optional_certificate_v1(
            P256EcdsaRoleV1::CertificateOrCrl,
            true,
        )
        .unwrap();
        for byte in (0..160).step_by(2) {
            let row = P256_INPUT_SELECTION_ROW_START_V1 + byte;
            let base = byte_row(byte, active);
            let next = byte_row(byte + 1, active);
            let fixed = fixed_provider.row_v1(row).unwrap();
            let product_fixed = fixed_provider.fixed.row_v1(row).unwrap().product;
            let expected = base[SINK_SELECTION_SELECTED_BASE]
                .mul(F(256))
                .add(next[SINK_SELECTION_SELECTED_BASE]);
            let mut source = [F::ZERO; 6];
            source[0] = expected;
            let mut current_aux =
                build_regular_row_v1(product_fixed, source, [F(7); 4], challenges());
            let running = current_aux.products.map(|values| values[6]);
            let mut next_aux = build_regular_row_v1(
                fixed_provider.fixed.row_v1(row + 1).unwrap().product,
                [F::ZERO; 6],
                running,
                challenges(),
            );
            current_aux.terminal = [F(11); 4];
            next_aux.terminal = [F(11); 4];
            let aux = flatten_regular_aux_v1(current_aux);
            let next_aux = flatten_regular_aux_v1(next_aux);
            let evaluate =
                |a: &[F; P256_BINDING_SINK_BASE_WIDTH_V1],
                 b: &[F; P256_BINDING_SINK_BASE_WIDTH_V1],
                 fixed: &[F; P256_BINDING_SINK_FIXED_WIDTH_V1]| {
                    let residues = evaluate_p256_binding_sink_aggregate_residues_v1(
                        a,
                        b,
                        &aux,
                        &next_aux,
                        fixed,
                        challenges(),
                    )
                    .unwrap();
                    let extension = evaluate_p256_binding_sink_aggregate_residues_over_field_v1(
                        &a.map(E::from_base),
                        &b.map(E::from_base),
                        &aux.map(E::from_base),
                        &next_aux.map(E::from_base),
                        &fixed.map(E::from_base),
                        challenges(),
                    )
                    .unwrap();
                    assert_eq!(
                        extension,
                        residues
                            .iter()
                            .copied()
                            .map(E::from_base)
                            .collect::<Vec<_>>()
                    );
                    residues
                };
            assert!(
                evaluate(&base, &next, &fixed).iter().all(|x| *x == F::ZERO),
                "byte={byte},active={active}"
            );
            // Preserve range and real/selected equality under active mutations; the
            // product copy must still reject either half of the packed limb.
            for half in 0..2 {
                let mut changed = [base, next];
                let value = (changed[half][SINK_SELECTION_SELECTED_BASE].0 as u8) ^ 1;
                changed[half][SINK_SELECTION_SELECTED_BASE] = F(u64::from(value));
                write_byte_bits_v1(
                    &mut changed[half]
                        [SINK_SELECTION_SELECTED_BITS_BASE..SINK_SELECTION_SELECTED_BITS_BASE + 8],
                    value,
                );
                if active {
                    changed[half][SINK_SELECTION_REAL_BASE] = F(u64::from(value));
                    write_byte_bits_v1(
                        &mut changed[half]
                            [SINK_SELECTION_REAL_BITS_BASE..SINK_SELECTION_REAL_BITS_BASE + 8],
                        value,
                    );
                }
                assert_ne!(evaluate(&changed[0], &changed[1], &fixed)[0], F::ZERO);
            }
            for coordinate in 0..3 {
                let mut changed = fixed;
                changed[SINK_EVENTS_FIXED + coordinate] =
                    changed[SINK_EVENTS_FIXED + coordinate].add(F::ONE);
                assert!(
                    evaluate(&base, &next, &changed)
                        .iter()
                        .any(|x| *x != F::ZERO)
                );
            }
            let mut swapped = [base, next];
            swapped.swap(0, 1);
            if base[SINK_SELECTION_SELECTED_BASE] != next[SINK_SELECTION_SELECTED_BASE] {
                assert_ne!(evaluate(&swapped[0], &swapped[1], &fixed)[0], F::ZERO);
            }
        }
    }
}

#[test]
fn selected_pair_source_is_affine_at_nonbase_ood_points_and_old_rows_remain_exact() {
    let w = E::canonical([3, 5, 7, 11]).unwrap();
    let mut base = [E::ZERO; P256_BINDING_SINK_BASE_WIDTH_V1];
    let mut next = base;
    base[0] = w;
    base[SINK_SELECTION_SELECTED_BASE] = w.mul(w);
    next[SINK_SELECTION_SELECTED_BASE] = w.mul(w).mul(w);
    let source = sink_sources_from_opened_base_v1(&base, &next);
    assert_eq!(
        source[0],
        w.add(w.mul(w).mul_base(F(256))).add(w.mul(w).mul(w))
    );
    assert_eq!(&source[1..], &[E::ZERO; 5]);
    base[SINK_SELECTION_SELECTED_BASE] = E::ZERO;
    next[SINK_SELECTION_SELECTED_BASE] = E::ZERO;
    assert_eq!(sink_sources_from_opened_base_v1(&base, &next)[0], w);
}

#[test]
fn selected_input_regions_keep_native_shapes_fixed_widths_and_selector_requirements() {
    assert_eq!(
        (
            P256_BINDING_SINK_BASE_WIDTH_V1,
            P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
            P256_BINDING_SINK_FIXED_WIDTH_V1
        ),
        (25, 38, 36)
    );
    assert_eq!(
        (
            P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1,
            P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1
        ),
        (116, 46)
    );
    for optional in [false, true] {
        let provider = P256BindingSinkFixedProviderV1::new_with_optional_certificate_v1(
            P256EcdsaRoleV1::CertificateOrCrl,
            optional,
        )
        .unwrap();
        for row in [
            0,
            159,
            160,
            P256_INPUT_SELECTION_ROW_START_V1 - 1,
            P256_INPUT_SELECTION_SELECTOR_ROW_V1 + 1,
        ] {
            let fixed = provider.row_v1(row).unwrap();
            assert_eq!(fixed[SINK_SELECTION_BYTE_FIXED], F::ZERO);
            assert_eq!(fixed[SINK_SELECTION_SELECTOR_FIXED], F::ZERO);
        }
        assert_eq!(
            provider.row_v1(P256_INPUT_SELECTION_ROW_START_V1).unwrap()[SINK_SELECTION_BYTE_FIXED],
            F::ONE
        );
        assert_eq!(
            provider
                .row_v1(P256_INPUT_SELECTION_SELECTOR_ROW_V1 - 1)
                .unwrap()[SINK_SELECTION_BYTE_FIXED],
            F::ONE
        );
        let selector = provider
            .row_v1(P256_INPUT_SELECTION_SELECTOR_ROW_V1)
            .unwrap();
        assert_eq!(selector[SINK_SELECTION_SELECTOR_FIXED], F::ONE);
        assert_eq!(
            selector[SINK_SELECTION_REQUIRE_ACTIVE_FIXED],
            F(u64::from(!optional))
        );
    }
}

#[test]
fn inactive_neighbor_sources_at_both_region_edges_and_every_odd_row_are_gated_in_f_and_fp4() {
    let provider = P256BindingSinkFixedProviderV1::new_with_optional_certificate_v1(
        P256EcdsaRoleV1::CertificateOrCrl,
        true,
    )
    .unwrap();
    let base_at = |row: usize| {
        if (P256_INPUT_SELECTION_ROW_START_V1..P256_INPUT_SELECTION_SELECTOR_ROW_V1).contains(&row)
        {
            byte_row(row - P256_INPUT_SELECTION_ROW_START_V1, true)
        } else {
            let mut base = [F::ZERO; P256_BINDING_SINK_BASE_WIDTH_V1];
            base[SINK_SELECTION_ACTIVE_BASE] = F::ONE;
            base
        }
    };
    let rows = [
        P256_INPUT_SELECTION_ROW_START_V1 - 1,
        P256_INPUT_SELECTION_SELECTOR_ROW_V1,
        P256_INPUT_SELECTION_SELECTOR_ROW_V1 + 1,
    ]
    .into_iter()
    .chain(
        (1..160)
            .step_by(2)
            .map(|byte| P256_INPUT_SELECTION_ROW_START_V1 + byte),
    );
    for row in rows {
        let base = base_at(row);
        let next = base_at(row + 1);
        let fixed = provider.row_v1(row).unwrap();
        let product = provider.fixed.row_v1(row).unwrap().product;
        assert_eq!(product.events, [P256CrossTraceEventFixedV1::inactive(); 6]);
        let mut current_aux = build_regular_row_v1(product, [F::ZERO; 6], [F(13); 4], challenges());
        current_aux.terminal = [F(17); 4];
        // Continuation authenticates only the next entering product and constant
        // terminal; next-row active factors are constrained at their own row.
        let aux = flatten_regular_aux_v1(current_aux);
        let residues = evaluate_p256_binding_sink_aggregate_residues_v1(
            &base,
            &next,
            &aux,
            &aux,
            &fixed,
            challenges(),
        )
        .unwrap();
        assert!(residues.iter().all(|x| *x == F::ZERO), "row={row}");
        let extension = evaluate_p256_binding_sink_aggregate_residues_over_field_v1(
            &base.map(E::from_base),
            &next.map(E::from_base),
            &aux.map(E::from_base),
            &aux.map(E::from_base),
            &fixed.map(E::from_base),
            challenges(),
        )
        .unwrap();
        assert_eq!(
            extension,
            residues.into_iter().map(E::from_base).collect::<Vec<_>>()
        );
        let mut wrong = aux;
        wrong[0] = F::ONE;
        assert_ne!(
            evaluate_p256_binding_sink_aggregate_residues_v1(
                &base,
                &next,
                &wrong,
                &aux,
                &fixed,
                challenges()
            )
            .unwrap()[0],
            F::ZERO
        );
    }
}
