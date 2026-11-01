// Original per-link quotient oracle and controls for production linear grouping.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection::observe_v1;

#[allow(clippy::too_many_arguments)]
fn accumulate_original_with_native_batches_v1<'a>(
    links: &[LinkV1],
    widths: &[u8; 2 * LINK_COUNT_V1],
    alphas: &[E],
    masks: usize,
    policy: main_bounded_transform::MainBoundedTransformPolicyV1,
    chunk: &mut Vec<E>,
    mut native: impl FnMut(
        ColumnV1,
        usize,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    mut original_mask: impl FnMut(ColumnV1) -> Result<&'a [F], ZkX509StarkErrorV1>,
    transform: &mut impl main_native_transform::MainNativeTransformV1,
    mut uncertain: impl FnMut() -> bool,
) -> Result<(), ZkX509StarkErrorV1> {
    if links.is_empty()
        || links.len() > LINK_COUNT_V1
        || alphas.len() != links.len()
        || alphas.iter().any(|value| !value.is_canonical())
        || masks == 0
        || masks > MASK_DEGREE + 1
        || chunk.iter().any(|value| !value.is_canonical())
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    for index in 0..links.len() {
        for side in 0..2 {
            match widths[2 * index + side] {
                1 => {}
                4 if contiguous_family_v1(links, index, side) => {}
                _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
            }
        }
    }
    let mut points = [F::ZERO; MAX_POINTS];
    let mut point_count = 0;
    let mut max_log = 0;
    for link in links {
        if !link.point.is_canonical() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if !points[..point_count].contains(&link.point) {
            if point_count == MAX_POINTS {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            points[point_count] = link.point;
            point_count += 1;
        }
        for column in [Some(link.left), link.right].into_iter().flatten() {
            if column.native_log2 == 0
                || column.native_log2 > ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
                || link.point.pow(1_u128 << column.native_log2) != F::ONE
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            max_log = max_log.max(column.native_log2);
        }
    }
    let max_rows = 1_usize << max_log;
    let count = max_rows
        .checked_add(masks)
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
    let charge = payload_v1(max_rows, masks, links.len())?;
    let policy = policy.reserve_additional_v1(charge)?;
    if chunk.capacity() < count - 1 {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    main_bounded_transform::check_completion_v1(uncertain())?;
    // Publication is deferred until every individual endpoint has passed and all
    // transforms have completed. These additional buffers are fully charged.
    let mut result = extensions_v1(count)?;
    let mut quotient = extensions_v1(count)?;
    let mut weighted_mask = extensions_v1(masks)?;
    let mut endpoints = fields_v1(2 * links.len())?;
    let mut seen = [false; 2 * LINK_COUNT_V1];
    let mut scalar = fields_v1(4)?;
    for (index, link) in links.iter().enumerate() {
        if link.right.is_none() {
            endpoints[2 * index + 1] = F::ONE;
            seen[2 * index + 1] = true;
        }
    }
    for log in 1..=max_log {
        if !links
            .iter()
            .flat_map(|link| [Some(link.left), link.right])
            .flatten()
            .any(|column| column.native_log2 == log)
        {
            continue;
        }
        let rows = 1_usize << log;
        let root = goldilocks_primitive_root_v1(log).map_err(map_transparent_error_v1)?;
        let mut weighted = matrix_v1(rows)?;
        let mut inverse = fields_v1(rows)?;
        for &point in &points[..point_count] {
            if !links.iter().any(|link| {
                link.point == point
                    && [Some(link.left), link.right]
                        .into_iter()
                        .flatten()
                        .any(|column| column.native_log2 == log)
            }) {
                continue;
            }
            let endpoint = inverse_table_v1(&mut inverse, root, point)?;
            let point_inverse = point.inv().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            let diagonal = F((rows - 1) as u64)
                .mul(F(2).inv().ok_or(ZkX509StarkErrorV1::ProfileMismatch)?)
                .mul(point_inverse);
            weighted_mask.fill(E::ZERO);
            let mut caches: [NativeFamilyCacheV1; 2] =
                core::array::from_fn(|_| NativeFamilyCacheV1::default());
            for (index, (link, &alpha)) in links.iter().zip(alphas).enumerate() {
                if link.point != point {
                    continue;
                }
                for (side, column) in [Some(link.left), link.right].into_iter().enumerate() {
                    let Some(column) = column.filter(|column| column.native_log2 == log) else {
                        continue;
                    };
                    main_bounded_transform::check_completion_v1(uncertain())?;
                    let values = caches[side].take_v1(
                        index,
                        column,
                        usize::from(widths[2 * index + side]),
                        &mut native,
                    )?;
                    // Validate only this original link/side now. A later cached
                    // lane must not outrank the other side's mask or endpoint.
                    if values.len() != rows || values.0.capacity() != rows {
                        return Err(ZkX509StarkErrorV1::ProofTooLarge);
                    }
                    if values.iter().any(|value| !value.is_canonical()) {
                        return Err(ZkX509StarkErrorV1::ProfileMismatch);
                    }
                    let mask = original_mask(column)?;
                    if mask.len() != masks || mask.iter().any(|value| !value.is_canonical()) {
                        return Err(ZkX509StarkErrorV1::ProfileMismatch);
                    }
                    scalar[0] = values[endpoint];
                    endpoints[2 * index + side] = scalar[0];
                    seen[2 * index + side] = true;
                    if seen[2 * index]
                        && seen[2 * index + 1]
                        && endpoints[2 * index] != endpoints[2 * index + 1]
                    {
                        return Err(ZkX509StarkErrorV1::ConstraintOpening);
                    }
                    let scale = if side == 0 { alpha } else { E::ZERO.sub(alpha) };
                    scalar[1] = scalar[0].mul(diagonal);
                    let mut x = F::ONE;
                    for row in 0..rows {
                        if row != endpoint {
                            scalar[2] = values[row].sub(scalar[0]).mul(inverse[row]);
                            // L_i'(a) = -x_i/(a*(x_i-a)); only public inversion.
                            scalar[1] = scalar[1]
                                .sub(values[row].mul(x).mul(inverse[row]).mul(point_inverse));
                            for lane in 0..4 {
                                weighted[lane][row] = weighted[lane][row]
                                    .add(scale.coefficients()[lane].mul(scalar[2]));
                            }
                        }
                        x = x.mul(root);
                    }
                    for lane in 0..4 {
                        weighted[lane][endpoint] =
                            weighted[lane][endpoint].add(scale.coefficients()[lane].mul(scalar[1]));
                    }
                    for (target, &value) in weighted_mask.iter_mut().zip(mask) {
                        *target = target.add(scale.mul_base(value));
                    }
                    scalar.fill(F::ZERO);
                    // Drop this owner before the next side can allocate its batch.
                }
            }
            if caches
                .iter()
                .any(|cache| !cache.columns.is_empty() || cache.next.is_some())
            {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            drop(caches);
            mask_quotient_v1(&weighted_mask, rows, point, &mut quotient)?;
            for (target, &value) in result.iter_mut().zip(quotient.iter()) {
                *target = target.add(value);
            }
            quotient.fill(E::ZERO);
            weighted_mask.fill(E::ZERO);
        }
        policy.inverse_native_with_v1(&mut weighted, root, transform, &mut uncertain)?;
        for row in 0..rows {
            result[row] = result[row].add(
                E::from_coefficients(core::array::from_fn(|lane| weighted[lane][row]))
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            );
        }
    }
    if seen[..2 * links.len()].iter().any(|seen| !seen) {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    for index in 0..links.len() {
        if endpoints[2 * index] != endpoints[2 * index + 1] {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
    }
    main_bounded_transform::check_completion_v1(uncertain())?;
    if result[count - 1] != E::ZERO {
        return Err(ZkX509StarkErrorV1::ConstraintOpening);
    }
    // Infallible publication only: the caller's existing admitted allocation is
    // unchanged on every earlier error or unwind, including device uncertainty.
    if chunk.len() < count - 1 {
        chunk.resize(count - 1, E::ZERO);
    }
    for (target, &value) in chunk.iter_mut().zip(&result[..count - 1]) {
        *target = target.add(value);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_variant_v1<'a>(
    linear: bool,
    links: &[LinkV1],
    widths: &[u8; 2 * LINK_COUNT_V1],
    alphas: &[E],
    masks: usize,
    policy: main_bounded_transform::MainBoundedTransformPolicyV1,
    chunk: &mut Vec<E>,
    native: impl FnMut(ColumnV1, usize) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    original_mask: impl FnMut(ColumnV1) -> Result<&'a [F], ZkX509StarkErrorV1>,
    transform: &mut impl main_native_transform::MainNativeTransformV1,
    uncertain: impl FnMut() -> bool,
) -> Result<(), ZkX509StarkErrorV1> {
    if linear {
        accumulate_with_native_batches_v1(
            links,
            widths,
            alphas,
            masks,
            policy,
            chunk,
            native,
            original_mask,
            transform,
            uncertain,
        )
    } else {
        accumulate_original_with_native_batches_v1(
            links,
            widths,
            alphas,
            masks,
            policy,
            chunk,
            native,
            original_mask,
            transform,
            uncertain,
        )
    }
}

fn small_links_v1() -> [LinkV1; 4] {
    core::array::from_fn(|column| LinkV1 {
        left: ColumnV1 {
            group: 0,
            column,
            native_log2: 3,
        },
        right: Some(ColumnV1 {
            group: 1,
            column,
            native_log2: 3,
        }),
        point: F::ONE,
    })
}

fn small_widths_v1() -> [u8; 2 * LINK_COUNT_V1] {
    let mut widths = [1; 2 * LINK_COUNT_V1];
    widths[0] = 4;
    widths[1] = 4;
    widths
}

fn fixture_masks_v1(
    links: &[LinkV1],
    length: usize,
) -> std::collections::BTreeMap<(usize, usize), Vec<F>> {
    links
        .iter()
        .flat_map(|link| [Some(link.left), link.right])
        .flatten()
        .map(|column| {
            (
                (column.group, column.column),
                (0..length)
                    .map(|index| F((31 * index + 17 * column.group + column.column + 1) as u64))
                    .collect(),
            )
        })
        .collect()
}

fn fixture_native_v1(
    links: &[LinkV1],
    first: ColumnV1,
    width: usize,
) -> Vec<ZeroizingMainTraceColumnV1> {
    (0..width)
        .map(|lane| {
            let column = ColumnV1 {
                column: first.column + lane,
                ..first
            };
            let mut points = Vec::new();
            for link in links {
                if (link.left == column || link.right == Some(column))
                    && !points.contains(&link.point)
                {
                    points.push(link.point);
                }
            }
            assert!(!points.is_empty());
            let root = goldilocks_primitive_root_v1(column.native_log2).unwrap();
            let slope = F((17 * column.group + column.column + 3) as u64);
            let mut x = F::ONE;
            ZeroizingMainTraceColumnV1(
                (0..1usize << column.native_log2)
                    .map(|_| {
                        let factor = points
                            .iter()
                            .fold(F::ONE, |product, point| product.mul(x.sub(*point)));
                        let value = F::ONE.add(factor.mul(slope));
                        x = x.mul(root);
                        value
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn linear_grouping_matches_original_all_mixed_domains_points_and_worker_counts() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut plan = MainTerminalLinkPlanV1::new_v1(&layout).unwrap();
    let widths = family_widths_v1(&plan, &layout).unwrap();
    let last = goldilocks_primitive_root_v1(19).unwrap().pow((1 << 19) - 1);
    for link in &mut plan.links {
        link.point = if link.point == F::ONE {
            F::ONE
        } else if link.point == last {
            F::ZERO.sub(F::ONE)
        } else {
            goldilocks_primitive_root_v1(2).unwrap()
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
                _ => panic!("unexpected native geometry"),
            };
        }
    }
    const MASKS: usize = 23;
    let masks = fixture_masks_v1(&plan.links, MASKS);
    let alphas: [E; LINK_COUNT_V1] =
        core::array::from_fn(|index| E::canonical([index as u64 + 1, 3, 5, 7]).unwrap());
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let mut outputs = [vec![E::ONE; 16 + MASKS - 1], vec![E::ONE; 16 + MASKS - 1]];
                let mut orders = [Vec::new(), Vec::new()];
                let mut source_counts = [0; 2];
                for variant in 0..2 {
                    run_variant_v1(
                        variant == 1,
                        &plan.links,
                        &widths,
                        &alphas,
                        MASKS,
                        main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(
                            1 << 19,
                            8,
                        ),
                        &mut outputs[variant],
                        |column, width| {
                            source_counts[variant] += 1;
                            Ok(fixture_native_v1(&plan.links, column, width))
                        },
                        |column| {
                            orders[variant].push((column.group, column.column, column.native_log2));
                            Ok(masks[&(column.group, column.column)].as_slice())
                        },
                        &mut main_native_transform::ExactNativeTransformV1,
                        || false,
                    )
                    .unwrap();
                }
                assert_eq!(outputs[0], outputs[1]);
                assert_eq!(orders[0], orders[1]);
                assert_eq!(source_counts, [259; 2]);
                assert_eq!(orders[0].len(), 364);
            });
    }
}

#[test]
fn linear_grouping_matches_independent_masked_polynomial_division() {
    use crate::privacy_engines::transparent_stark::masked_trace_coefficients_with_mask_v1;
    let links = small_links_v1();
    const MASKS: usize = 23;
    let masks = fixture_masks_v1(&links, MASKS);
    let alphas: [E; 4] =
        core::array::from_fn(|index| E::canonical([index as u64 + 1, 3, 5, 7]).unwrap());
    let mut expected = vec![E::ONE; 8 + MASKS - 1];
    for (link, alpha) in links.iter().zip(alphas) {
        let mut difference = vec![F::ZERO; 8 + MASKS];
        for (side, column) in [link.left, link.right.unwrap()].into_iter().enumerate() {
            let coefficients = masked_trace_coefficients_with_mask_v1(
                &fixture_native_v1(&links, column, 1)[0],
                column.native_log2,
                &masks[&(column.group, column.column)],
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
        divide_linear_in_place_v1(&mut difference, link.point).unwrap();
        for (target, value) in expected.iter_mut().zip(difference) {
            *target = target.add(alpha.mul_base(value));
        }
    }
    let mut actual = vec![E::ONE; expected.len()];
    run_variant_v1(
        true,
        &links,
        &small_widths_v1(),
        &alphas,
        MASKS,
        main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(1 << 19, 8),
        &mut actual,
        |column, width| Ok(fixture_native_v1(&links, column, width)),
        |column| Ok(masks[&(column.group, column.column)].as_slice()),
        &mut main_native_transform::ExactNativeTransformV1,
        || false,
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn linear_grouping_preserves_each_refusal_order_and_clears_all_owners() {
    use std::cell::{Cell, RefCell};
    let links = small_links_v1();
    const MASKS: usize = 23;
    let masks = fixture_masks_v1(&links, MASKS);
    for failure in 0..10 {
        let alphas = if failure == 1 {
            [E::ZERO; 4]
        } else {
            [E::ONE; 4]
        };
        let mut results = Vec::new();
        let mut all_events = Vec::new();
        for linear in [false, true] {
            let events = RefCell::new(Vec::new());
            let uncertain = Cell::new(false);
            let mut output = vec![E::ONE; 8 + MASKS - 1];
            let before = output.clone();
            let (result, erased) = observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_variant_v1(
                        linear,
                        &links,
                        &small_widths_v1(),
                        &alphas,
                        MASKS,
                        main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(
                            1 << 19,
                            8,
                        ),
                        &mut output,
                        |column, width| {
                            events
                                .borrow_mut()
                                .push((0, column.group, column.column, width));
                            let mut values = fixture_native_v1(&links, column, width);
                            if column.group == 0 {
                                match failure {
                                    0..=2 => {
                                        values[0].0[0] = values[0].0[0].add(F::ONE);
                                        if failure == 2 {
                                            values[1].0[0] = values[1].0[0].sub(F::ONE);
                                        }
                                    }
                                    4 | 6 => values[1].0[0] = F(u64::MAX),
                                    5 => {
                                        values[1].0.pop();
                                    }
                                    _ => {}
                                }
                            } else {
                                match failure {
                                    3 => return Err(ZkX509StarkErrorV1::AllocationFailure),
                                    5 => values[0].0[0] = F(2),
                                    7 => {
                                        uncertain.set(true);
                                        return Err(ZkX509StarkErrorV1::AllocationFailure);
                                    }
                                    9 => std::panic::resume_unwind(Box::new(())),
                                    _ => {}
                                }
                            }
                            Ok(values)
                        },
                        |column| {
                            events
                                .borrow_mut()
                                .push((1, column.group, column.column, 0));
                            if failure == 4 && column.group == 1 {
                                Err(ZkX509StarkErrorV1::TranscriptMismatch)
                            } else {
                                Ok(masks[&(column.group, column.column)].as_slice())
                            }
                        },
                        &mut main_native_transform::WordInjectionV1(|_: &mut [Vec<u64>], _, _| {
                            events.borrow_mut().push((2, 0, 0, 0));
                            assert_eq!(failure, 8);
                            Err(TransformError::DeviceFailure(
                                "injected terminal failure".into(),
                            ))
                        }),
                        || uncertain.get(),
                    )
                }))
            });
            let panicked = result.is_err();
            let result = result.unwrap_or(Err(ZkX509StarkErrorV1::InternalInvariant));
            assert!(result.is_err());
            assert_eq!(output, before);
            assert!(erased.iter().all(|record| record.nonzero_after == 0));
            results.push((panicked, result));
            all_events.push(events.into_inner());
        }
        assert_eq!(results[0], results[1], "failure {failure}");
        assert_eq!(all_events[0], all_events[1], "failure {failure}");
    }
}

#[test]
fn linear_grouping_keeps_allocation_refusals_and_payload_unchanged() {
    let links = small_links_v1();
    let masks = fixture_masks_v1(&links, 23);
    for step in 0..12 {
        let mut results = Vec::new();
        for linear in [false, true] {
            let mut output = vec![E::ONE; 30];
            let before = output.clone();
            let (result, erased) = observe_v1(|| {
                allocation_failure::at_v1(step, || {
                    run_variant_v1(
                        linear,
                        &links,
                        &small_widths_v1(),
                        &[E::ONE; 4],
                        23,
                        main_bounded_transform::MainBoundedTransformPolicyV1::for_test_v1(
                            1 << 19,
                            8,
                        ),
                        &mut output,
                        |column, width| Ok(fixture_native_v1(&links, column, width)),
                        |column| Ok(masks[&(column.group, column.column)].as_slice()),
                        &mut main_native_transform::ExactNativeTransformV1,
                        || false,
                    )
                })
            });
            if result.is_err() {
                assert_eq!(output, before);
            }
            assert!(erased.iter().all(|record| record.nonzero_after == 0));
            results.push(result);
        }
        assert_eq!(results[0], results[1], "allocation step {step}");
    }
}

#[test]
#[ignore = "actual maximum bound fixture; original versus production linear terminal grouping, complete source/transform/erase cost"]
fn maximum_fixture_terminal_linear_grouping_matches_all_coefficients_and_reports_complete_cost() {
    use crate::privacy_engines::transparent_stark::sample_trace_mask_v1;
    use rand::SeedableRng;
    use std::time::{Duration, Instant};
    assert!(!cfg!(debug_assertions), "run release benchmark");
    assert_eq!(rayon::current_num_threads(), 20);
    assert_eq!(rayon::current_thread_index(), None);
    use crate::privacy_engines::zk_x509::{
        main_assembly::build_zk_x509_main_trace_assembly_v1,
        relation::{
            ZkX509GovernanceV1,
            release_fixture::{build_zk_x509_release_fixture_v1, reference_statement_context_v1},
        },
    };
    // The same real maximum signed fixture and bound capability used by the
    // existing native dispatch/commitment tests. No replacement source closure
    // or test-only arithmetic implements the production column recurrence.
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    let trust_anchor = fixture.authoritative_state.trust_anchor();
    let crl = fixture.authoritative_state.crl_record();
    let assembly = build_zk_x509_main_trace_assembly_v1(
        &fixture.statement,
        ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        },
        &fixture.witness,
    )
    .unwrap();
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let digest = |seed| PrivacyOuterDigestV1::from_bytes([seed; 48]);
    let binding = derive_zk_x509_credential_pre_aux_binding_v1(
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x81; 32],
            assembly.verifier_profile.compiled_profile_digest,
            core::array::from_fn(|index| {
                digest(u8::try_from(index).expect("fixture value fits u8") + 1)
            }),
        ),
        digest(0x91),
        digest(0xa1),
        digest(0xb1),
    )
    .unwrap();
    let sha =
        main_log19_sha_base_sources_v1(&assembly.sha_schedule, &assembly.sha_witnesses).unwrap();
    let p256 = P256MainBaseSourceV1::new_v1(&assembly).unwrap();
    let bound = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
        &layout, &assembly, sha, p256, &binding,
    )
    .unwrap();
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(
        &layout,
        &fixture.statement,
        &assembly.projection_trace,
    )
    .unwrap();
    let mut io =
        MainIoTraceGroupSourceV1::for_main_v1(&layout, &fixture.statement, &assembly.io).unwrap();
    projection
        .bind_challenges_v1(&binding.main_post_base())
        .unwrap();
    io.bind_challenges_v1(&binding.main_post_base()).unwrap();
    let sources = MainTraceReplaySourcesV1::Bound {
        log19: &bound,
        projection: &projection,
        io: &io,
    };
    let plan = MainTerminalLinkPlanV1::new_v1(&layout).unwrap();
    let widths = family_widths_v1(&plan, &layout).unwrap();
    let policy = main_bounded_transform::MainBoundedTransformPolicyV1::for_assembly_v1(
        &layout,
        assembly.allocated_payload_bytes_v1(),
    )
    .unwrap();
    let cap = layout
        .as_shared()
        .unwrap()
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .unwrap();
    assert_eq!(cap, 589_824);
    let mut rng = rand_chacha::ChaCha12Rng::from_seed([0x47; 32]);
    let mut masks = std::collections::BTreeMap::new();
    for column in plan
        .links
        .iter()
        .flat_map(|link| [Some(link.left), link.right])
        .flatten()
    {
        masks
            .entry((column.group, column.column))
            .or_insert_with(|| sample_trace_mask_v1(MASK_DEGREE, &mut rng).unwrap());
    }
    let alphas: [E; LINK_COUNT_V1] =
        core::array::from_fn(|index| E::canonical([index as u64 + 1, 3, 5, 7]).unwrap());
    let retained_before = bound.allocated_payload_bytes_v1();
    let execute = |linear: bool| {
        let start = Instant::now();
        let mut output = ZeroizingExtensionColumnV1(Vec::new());
        output.0.try_reserve_exact(cap).unwrap();
        assert_eq!(output.0.capacity(), cap);
        output.0.resize(cap, E::ONE);
        let allocation = start.elapsed();
        let start = Instant::now();
        let mut source_calls = 0;
        let mut mask_calls = 0;
        run_variant_v1(
            linear,
            &plan.links,
            &widths,
            &alphas,
            MASK_DEGREE + 1,
            policy,
            &mut output.0,
            |column, width| {
                source_calls += 1;
                sources.native_columns_v1(
                    &layout,
                    MainTraceColumnKindV1::Aux,
                    column.group,
                    column.column..column.column + width,
                )
            },
            |column| {
                mask_calls += 1;
                Ok(masks[&(column.group, column.column)].coefficients())
            },
            &mut main_native_transform::ExactNativeTransformV1,
            fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1,
        )
        .unwrap();
        let computation = start.elapsed();
        assert_eq!((source_calls, mask_calls), (259, 364));
        assert_eq!(bound.allocated_payload_bytes_v1(), retained_before);
        (output, allocation, computation)
    };
    // Same real source recurrences and original masks in both arms. The current
    // algorithm is the independent oracle; comparison is outside timed work.
    let (expected, _, _) = execute(false);
    let (warm, _, _) = execute(true);
    assert_eq!(warm.0, expected.0);
    drop(warm);
    const ROUNDS: usize = 4;
    let mut elapsed = [[Duration::ZERO; ROUNDS]; 2];
    for round in 0..ROUNDS {
        for variant in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let (actual, allocation, computation) = execute(variant == 1);
            assert_eq!(
                actual.0, expected.0,
                "all full output coefficients and untouched tail"
            );
            let start = Instant::now();
            drop(actual);
            let clear = start.elapsed();
            elapsed[variant][round] = allocation + computation + clear;
            eprintln!(
                "terminal_linear_complete_cost variant={variant} round={round} allocation_seconds={:.9} computation_seconds={:.9} clear_seconds={:.9} total_seconds={:.9} source_calls=259 individual_mask_checks=364 payload_delta=0 original_source_order=true all_coefficients_exact=true fixture=maximum-depth-three bound_sources_retained_once=true excludes_fixture_setup_and_oracle_comparison=true",
                allocation.as_secs_f64(),
                computation.as_secs_f64(),
                clear.as_secs_f64(),
                elapsed[variant][round].as_secs_f64(),
            );
        }
    }
    for (variant, times) in elapsed.iter_mut().enumerate() {
        times.sort_unstable();
        let median = (times[1] + times[2]).as_secs_f64() / 2.0;
        eprintln!("terminal_linear_complete_median variant={variant} seconds={median:.9}");
    }
}
