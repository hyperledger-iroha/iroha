// Source-family batching controls; every weighted coefficient and check stays ordered.

use super::*;
use crate::privacy_engines::transparent_stark::{
    goldilocks_ifft_v1, masked_trace_coefficients_with_mask_v1,
};
use crate::privacy_engines::zk_x509::private_table::inspection::observe_v1;

fn links_v1(log: u8) -> [LinkV1; 4] {
    core::array::from_fn(|column| LinkV1 {
        left: ColumnV1 {
            group: 0,
            column,
            native_log2: log,
        },
        right: Some(ColumnV1 {
            group: 1,
            column,
            native_log2: log,
        }),
        point: F::ONE,
    })
}
fn widths_v1() -> [u8; 2 * LINK_COUNT_V1] {
    let mut widths = [1; 2 * LINK_COUNT_V1];
    widths[0] = 4;
    widths[1] = 4;
    widths
}
fn source_v1(first: ColumnV1, width: usize) -> Vec<ZeroizingMainTraceColumnV1> {
    (0..width)
        .map(|lane| {
            let root = goldilocks_primitive_root_v1(first.native_log2).unwrap();
            let slope = F((3 + first.group * 7 + first.column + lane) as u64);
            let mut x = F::ONE;
            ZeroizingMainTraceColumnV1(
                (0..1_usize << first.native_log2)
                    .map(|_| {
                        let value = F::ONE.add(x.sub(F::ONE).mul(slope.add(x.mul(F(7)))));
                        x = x.mul(root);
                        value
                    })
                    .collect(),
            )
        })
        .collect()
}
fn mask_index_v1(column: ColumnV1) -> usize {
    4 * column.group + column.column
}
fn masks_v1() -> Vec<Vec<F>> {
    (0..8)
        .map(|column| {
            (0..MASK_DEGREE + 1)
                .map(|index| F((31 * index + column + 1) as u64))
                .collect()
        })
        .collect()
}
fn alphas_v1() -> [E; 4] {
    core::array::from_fn(|index| E::canonical([index as u64 + 1, 3, 5, 7]).unwrap())
}
fn policy_v1() -> main_bounded_transform::MainBoundedTransformPolicyV1 {
    main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(1 << 19, 8)
}
fn transform_v1(
    words: &mut [Vec<u64>],
    root: u64,
    direction: Direction,
) -> Result<Backend, TransformError> {
    assert_eq!(direction, Direction::Inverse);
    for column in words {
        let mut fields = column.iter().copied().map(F).collect::<Vec<_>>();
        goldilocks_ifft_v1(&mut fields, F(root)).unwrap();
        for (word, value) in column.iter_mut().zip(fields) {
            *word = value.0;
        }
    }
    Ok(Backend::Cpu)
}

#[test]
fn canonical_families_cover_only_existing_grouped_sources_and_preserve_request_order() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let plan = MainTerminalLinkPlanV1::new_v1(&layout).unwrap();
    let widths = family_widths_v1(&plan, &layout).unwrap();
    assert_eq!(&widths[..16], &[1; 16], "DER/RFC stays scalar");
    assert_eq!(widths.iter().filter(|&&width| width == 4).count(), 35);
    assert_eq!(364 - 3 * 35, 259);
    for (index, link) in plan.links.iter().enumerate() {
        for (side, column) in [Some(link.left), link.right].into_iter().enumerate() {
            if widths[2 * index + side] == 4 {
                assert!(contiguous_family_v1(&plan.links, index, side));
                let column = column.unwrap();
                let (registration, _) = registered_main_group_column_v1(
                    &layout,
                    column.group,
                    MainTraceColumnKindV1::Aux,
                    column.column,
                )
                .unwrap();
                let identity = p256_main_registration_from_main_layout_v1(registration).unwrap();
                assert!(matches!(
                    (identity.adapter_v1(), identity.local_instance_v1()),
                    (P256MainAdapterV1::Arithmetic, 0) | (P256MainAdapterV1::ValueBus, 0 | 1)
                ));
            }
            if let Some(column) = column {
                let (registration, _) = registered_main_group_column_v1(
                    &layout,
                    column.group,
                    MainTraceColumnKindV1::Aux,
                    column.column,
                )
                .unwrap();
                if p256_main_registration_from_main_layout_v1(registration)
                    .is_ok_and(|identity| identity.adapter_v1() == P256MainAdapterV1::BindingSink)
                {
                    assert_eq!(widths[2 * index + side], 1, "local-two sink stays scalar");
                }
            }
        }
    }
    let mut bad_layout = layout;
    bad_layout.registered_segments.swap(0, 1);
    assert!(family_widths_v1(&plan, &bad_layout).is_err());
}

#[test]
fn family_contiguity_rejects_cross_point_domain_group_gap_absent_side_and_overflow() {
    assert!(contiguous_family_v1(&links_v1(3), 0, 0));
    assert!(!contiguous_family_v1(&links_v1(3), 1, 0));
    assert!(!contiguous_family_v1(&links_v1(3), usize::MAX, 0));
    assert!(!contiguous_family_v1(&links_v1(3), 0, 2));
    for variant in 0..6 {
        let mut links = links_v1(3);
        match variant {
            0 => links[1].point = F::ZERO.sub(F::ONE),
            1 => links[1].left.native_log2 = 4,
            2 => links[1].left.group += 1,
            3 => links[1].left.column += 1,
            4 => links[0].left.column = usize::MAX,
            _ => links[1].right = None,
        }
        assert!(!contiguous_family_v1(&links, 0, usize::from(variant == 5)));
    }
}

#[test]
fn batched_coefficients_match_scalar_and_independent_masked_linear_division() {
    let masks = masks_v1();
    let alphas = alphas_v1();
    for log in [1, 3, 5, 8] {
        let links = links_v1(log);
        let count = (1_usize << log) + MASK_DEGREE + 1;
        let mut expected = vec![E::ZERO; count - 1];
        for (link, alpha) in links.iter().zip(alphas) {
            let mut difference = vec![F::ZERO; count];
            for (side, column) in [link.left, link.right.unwrap()].into_iter().enumerate() {
                let coefficients = masked_trace_coefficients_with_mask_v1(
                    &source_v1(column, 1)[0],
                    log,
                    &masks[mask_index_v1(column)],
                )
                .unwrap();
                for (target, value) in difference.iter_mut().zip(coefficients) {
                    *target = if side == 0 {
                        target.add(value)
                    } else {
                        target.sub(value)
                    };
                }
            }
            divide_linear_in_place_v1(&mut difference, F::ONE).unwrap();
            for (target, value) in expected.iter_mut().zip(difference) {
                *target = target.add(alpha.mul_base(value));
            }
        }
        for batched in [false, true] {
            let widths = if batched {
                widths_v1()
            } else {
                [1; 2 * LINK_COUNT_V1]
            };
            let mut output = vec![E::ZERO; count - 1];
            let mut requests = Vec::new();
            let mut checks = Vec::new();
            accumulate_with_batches_v1(
                &links,
                &widths,
                &alphas,
                MASK_DEGREE + 1,
                policy_v1(),
                &mut output,
                |column, width| {
                    requests.push((column.group, column.column, width));
                    Ok(source_v1(column, width))
                },
                |column| {
                    checks.push((column.group, column.column));
                    Ok(&masks[mask_index_v1(column)])
                },
                transform_v1,
                || false,
            )
            .unwrap();
            assert_eq!(output, expected, "log={log}, batched={batched}");
            assert_eq!(
                checks,
                (0..4)
                    .flat_map(|lane| [(0, lane), (1, lane)])
                    .collect::<Vec<_>>()
            );
            assert_eq!(requests.len(), if batched { 2 } else { 8 });
        }
    }
}

#[test]
fn cached_later_lane_shape_and_canonicality_do_not_outrank_current_right_mask_or_endpoint() {
    let masks = masks_v1();
    for failure in 0..2 {
        let mut output = vec![E::ONE; 8 + MASK_DEGREE];
        let before = output.clone();
        let mut checks = Vec::new();
        let (result, erased) = observe_v1(|| {
            accumulate_with_batches_v1(
                &links_v1(3),
                &widths_v1(),
                &alphas_v1(),
                MASK_DEGREE + 1,
                policy_v1(),
                &mut output,
                |column, width| {
                    let mut values = source_v1(column, width);
                    if column.group == 0 {
                        if failure == 0 {
                            values[1].0[0] = F(u64::MAX);
                        } else {
                            values[1].0.pop();
                        }
                    } else if failure == 1 {
                        values[0].0[0] = F(2);
                    }
                    Ok(values)
                },
                |column| {
                    checks.push((column.group, column.column));
                    if column.group == 1 && failure == 0 {
                        Err(ZkX509StarkErrorV1::TranscriptMismatch)
                    } else {
                        Ok(&masks[mask_index_v1(column)])
                    }
                },
                transform_v1,
                || false,
            )
        });
        assert_eq!(
            result,
            Err(if failure == 0 {
                ZkX509StarkErrorV1::TranscriptMismatch
            } else {
                ZkX509StarkErrorV1::ConstraintOpening
            })
        );
        assert_eq!(checks, [(0, 0), (1, 0)]);
        assert_eq!(output, before);
        assert!(erased.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn batch_errors_unwind_and_public_admission_clear_owners_without_publication() {
    let masks = masks_v1();
    for failure in 0..7 {
        let mut output = vec![E::ONE; 8 + MASK_DEGREE];
        let before = output.clone();
        let mut calls = 0;
        let (result, erased) = observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut widths = widths_v1();
                if failure == 6 {
                    widths[0] = 3;
                }
                accumulate_with_batches_v1(
                    &links_v1(3),
                    &widths,
                    &alphas_v1(),
                    MASK_DEGREE + 1,
                    if failure == 5 {
                        main_bounded_transform::MainBoundedTransformPolicyV1::cpu_v1()
                    } else {
                        policy_v1()
                    },
                    &mut output,
                    |column, width| {
                        calls += 1;
                        let mut values = source_v1(column, width);
                        if column.group == 1 {
                            match failure {
                                0 => return Err(ZkX509StarkErrorV1::AllocationFailure),
                                1 => {
                                    values.pop();
                                }
                                2 => values[1].0[0] = F(u64::MAX),
                                3 => {
                                    values[1].0.pop();
                                }
                                4 => panic!("injected family source unwind"),
                                _ => {}
                            }
                        }
                        Ok(values)
                    },
                    |column| Ok(&masks[mask_index_v1(column)]),
                    transform_v1,
                    || false,
                )
            }))
        });
        assert!(result.is_err() || result.unwrap().is_err());
        assert_eq!(output, before);
        assert_eq!(calls, if failure >= 5 { 0 } else { 2 });
        assert!(erased.iter().all(|record| record.nonzero_after == 0));
    }
}

#[test]
fn two_caches_peak_at_seven_native_columns_and_release_each_family() {
    let ((), erased) = observe_v1(|| {
        let mut left = NativeFamilyCacheV1::default();
        let mut right = NativeFamilyCacheV1::default();
        let mut peak = 0;
        for family in 0..2 {
            for lane in 0..4 {
                let index = 4 * family + lane;
                let column = ColumnV1 {
                    group: 0,
                    column: index,
                    native_log2: 3,
                };
                let current = left
                    .take_v1(index, column, 4, &mut |column, width| {
                        peak = peak.max(right.columns.len() + width);
                        Ok(source_v1(column, width))
                    })
                    .unwrap();
                assert_eq!(left.columns.len(), 3 - lane);
                drop(current);
                let current = right
                    .take_v1(
                        index,
                        ColumnV1 { group: 1, ..column },
                        4,
                        &mut |column, width| {
                            peak = peak.max(left.columns.len() + width);
                            Ok(source_v1(column, width))
                        },
                    )
                    .unwrap();
                drop(current);
            }
            assert!(left.columns.is_empty() && right.columns.is_empty());
            assert!(left.next.is_none() && right.next.is_none());
        }
        assert_eq!(peak, 7);
    });
    assert_eq!(erased.len(), 16);
    assert!(
        erased
            .iter()
            .all(|record| record.cells == 8 && record.nonzero_after == 0)
    );
    let rows = 1 << 19;
    let previous = (6 * rows + 384 + 4) * 8 + (2 * (rows + 1816) + 1816) * 32 + METADATA_BYTES;
    assert_eq!(
        payload_v1(rows, 1816, LINK_COUNT_V1).unwrap() - previous,
        25_165_824
    );
}

#[test]
fn batched_mixed_domains_points_and_asymmetric_sides_match_scalar_check_order() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut plan = MainTerminalLinkPlanV1::new_v1(&layout).unwrap();
    let widths = family_widths_v1(&plan, &layout).unwrap();
    let old_last = goldilocks_primitive_root_v1(19).unwrap().pow((1 << 19) - 1);
    let small_scalar = goldilocks_primitive_root_v1(2).unwrap();
    for link in &mut plan.links {
        link.point = if link.point == F::ONE {
            F::ONE
        } else if link.point == old_last {
            F::ZERO.sub(F::ONE)
        } else {
            small_scalar
        };
        for column in [Some(&mut link.left), link.right.as_mut()]
            .into_iter()
            .flatten()
        {
            column.native_log2 = match column.native_log2 {
                5 => 1,
                8 => 2,
                16 => 3,
                19 => 4,
                _ => panic!("unexpected public domain"),
            };
        }
    }
    let masks = plan
        .links
        .iter()
        .flat_map(|link| [Some(link.left), link.right])
        .flatten()
        .map(|column| {
            (
                (column.group, column.column),
                (0..MASK_DEGREE + 1)
                    .map(|index| F((index * 31 + column.group * 17 + column.column + 1) as u64))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let alphas = core::array::from_fn::<_, LINK_COUNT_V1, _>(|index| {
        E::canonical([index as u64 + 1, 3, 5, 7]).unwrap()
    });
    let mut outputs = [
        vec![E::ONE; 16 + MASK_DEGREE],
        vec![E::ONE; 16 + MASK_DEGREE],
    ];
    let mut orders = [Vec::new(), Vec::new()];
    let mut source_counts = [0, 0];
    for variant in 0..2 {
        let active_widths = if variant == 0 {
            [1; 2 * LINK_COUNT_V1]
        } else {
            widths
        };
        accumulate_with_batches_v1(
            &plan.links,
            &active_widths,
            &alphas,
            MASK_DEGREE + 1,
            policy_v1(),
            &mut outputs[variant],
            |column, width| {
                source_counts[variant] += 1;
                let mut values = Vec::with_capacity(width);
                for lane in 0..width {
                    let current = ColumnV1 {
                        column: column.column + lane,
                        ..column
                    };
                    let point = plan
                        .links
                        .iter()
                        .find(|link| link.left == current || link.right == Some(current))
                        .unwrap()
                        .point;
                    let slope = F((current.group * 17 + current.column + 1) as u64);
                    let root = goldilocks_primitive_root_v1(current.native_log2).unwrap();
                    let mut x = F::ONE;
                    values.push(ZeroizingMainTraceColumnV1(
                        (0..1_usize << current.native_log2)
                            .map(|_| {
                                let value = F::ONE.add(x.sub(point).mul(slope));
                                x = x.mul(root);
                                value
                            })
                            .collect(),
                    ));
                }
                Ok(values)
            },
            |column| {
                orders[variant].push((column.group, column.column, column.native_log2));
                Ok(masks
                    .get(&(column.group, column.column))
                    .unwrap()
                    .as_slice())
            },
            transform_v1,
            || false,
        )
        .unwrap();
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(orders[0], orders[1]);
    assert_eq!(orders[0].len(), 364);
    assert_eq!(source_counts, [364, 259]);
}

#[test]
fn batched_individual_endpoint_checks_reject_zero_alpha_and_cancelling_mutations() {
    let masks = masks_v1();
    for alphas in [[E::ZERO; 4], [E::ONE; 4]] {
        let mut output = vec![E::ONE; 8 + MASK_DEGREE];
        let before = output.clone();
        let mut requests = 0;
        let (result, erased) = observe_v1(|| {
            accumulate_with_batches_v1(
                &links_v1(3),
                &widths_v1(),
                &alphas,
                MASK_DEGREE + 1,
                policy_v1(),
                &mut output,
                |column, width| {
                    requests += 1;
                    let mut values = source_v1(column, width);
                    if column.group == 0 {
                        // A weighted sum could cancel these mutations, but each
                        // original endpoint must independently match its right side.
                        for value in values[0].iter_mut() {
                            *value = value.add(F::ONE);
                        }
                        for value in values[1].iter_mut() {
                            *value = value.sub(F::ONE);
                        }
                    }
                    Ok(values)
                },
                |column| Ok(&masks[mask_index_v1(column)]),
                transform_v1,
                || false,
            )
        });
        assert_eq!(result, Err(ZkX509StarkErrorV1::ConstraintOpening));
        assert_eq!(requests, 2);
        assert_eq!(output, before);
        assert!(erased.iter().all(|record| record.nonzero_after == 0));
    }
}
