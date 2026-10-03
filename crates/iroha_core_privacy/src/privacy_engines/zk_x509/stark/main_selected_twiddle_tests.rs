//! Public-table cost experiment; production dispatch and memory admission are unchanged.

use super::*;

fn powers(common: u8) -> Vec<F> {
    let root = goldilocks_primitive_root_v1(common).unwrap();
    let mut value = F::ONE;
    (0..1usize << (common - 1))
        .map(|_| {
            let current = value;
            value = value.mul(root);
            current
        })
        .collect()
}

// This public fixture helper duplicates only the experiment's recursion. It is
// neither a production implementation nor an admitted alternative proof path.
fn cached_prune(
    values: &mut [F],
    offset: usize,
    destinations: &[usize],
    prefix: usize,
    powers: &[F],
) {
    assert!(prefix > 0 && prefix <= values.len());
    if destinations.is_empty() || values.len() == 1 {
        return;
    }
    let half = values.len() / 2;
    let stride = powers.len() * 2 / values.len();
    let split = destinations.partition_point(|&index| index < offset + half);
    let (left_destinations, right_destinations) = destinations.split_at(split);
    let left_needed = !left_destinations.is_empty();
    let right_needed = !right_destinations.is_empty();
    let (left, right) = values.split_at_mut(half);
    let pairs = prefix.min(half);
    let apply = |first: usize, left: &mut [F], right: &mut [F]| {
        for (j, (left, right)) in left.iter_mut().zip(right).enumerate() {
            let a = *left;
            let b = *right;
            if left_needed {
                *left = a.add(b);
            }
            if right_needed {
                *right = a.sub(b).mul(powers[(first + j) * stride]);
            }
        }
    };
    if pairs >= PARALLEL_VALUES_V1 / 2 && rayon::current_num_threads() > 1 {
        left[..pairs]
            .par_chunks_mut(PAIRS_PER_TASK_V1)
            .zip(right[..pairs].par_chunks_mut(PAIRS_PER_TASK_V1))
            .enumerate()
            .for_each(|(chunk, (a, b))| apply(chunk * PAIRS_PER_TASK_V1, a, b));
    } else {
        apply(0, &mut left[..pairs], &mut right[..pairs]);
    }
    if left_needed && right_needed && pairs >= PARALLEL_VALUES_V1 / 2 {
        rayon::join(
            || cached_prune(left, offset, left_destinations, pairs, powers),
            || cached_prune(right, offset + half, right_destinations, pairs, powers),
        );
    } else {
        if left_needed {
            cached_prune(left, offset, left_destinations, pairs, powers);
        }
        if right_needed {
            cached_prune(right, offset + half, right_destinations, pairs, powers);
        }
    }
}

fn cached_evaluate(
    coefficients: &[F],
    native: u8,
    common: u8,
    selected: &[usize],
    powers: &[F],
    destinations: &[usize],
) -> Column {
    let rows = 1usize << common;
    assert!(native < common && coefficients.len() <= rows && !coefficients.is_empty());
    assert_eq!(powers.len(), rows / 2);
    assert!(
        coefficients
            .iter()
            .all(|value| F::canonical(value.0).is_some())
    );
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    assert_ne!(shift.pow(rows as u128), F::ONE);
    assert_ne!(shift.pow((1usize << native) as u128), F::ONE);
    assert_eq!(destinations.len(), selected.len());
    assert!(!selected.is_empty() && selected.last().unwrap() < &rows);
    assert!(selected.windows(2).all(|w| w[0] < w[1]));
    let reversed = |row: usize| row.reverse_bits() >> (usize::BITS - u32::from(common));
    assert!(destinations.windows(2).all(|w| w[0] < w[1]));
    let mut scratch = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    scratch.try_reserve_exact(rows).unwrap();
    assert_eq!(scratch.capacity(), rows);
    scratch.resize(rows, F::ZERO);
    let mut scale = F::ONE;
    for (target, &coefficient) in scratch.iter_mut().zip(coefficients) {
        *target = coefficient.mul(scale);
        scale = scale.mul(F(GOLDILOCKS_GENERATOR_V1));
    }
    cached_prune(&mut scratch, 0, destinations, coefficients.len(), powers);
    let mut output = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    output.try_reserve_exact(selected.len()).unwrap();
    assert_eq!(output.capacity(), selected.len());
    output.extend(selected.iter().map(|&row| scratch[reversed(row)]));
    Column::from_vec_v1(output.into_vec())
}

fn destinations(common: u8, selected: &[usize]) -> Vec<usize> {
    let mut result = selected
        .iter()
        .map(|row| row.reverse_bits() >> (usize::BITS - u32::from(common)))
        .collect::<Vec<_>>();
    result.sort_unstable();
    result
}

#[test]
fn public_twiddle_stride_experiment_matches_independent_fft_and_mask_boundary_geometry() {
    for common in 2..=10 {
        let rows = 1usize << common;
        let roots = powers(common);
        let root = goldilocks_primitive_root_v1(common).unwrap();
        for (i, value) in roots.iter().enumerate() {
            assert_eq!(*value, root.pow(i as u128));
        }
        for length in [1, rows / 2 - 1, rows / 2, rows / 2 + 1, rows - 1, rows] {
            let coefficients = (0..length)
                .map(|i| F((i * 137 + 19) as u64))
                .collect::<Vec<_>>();
            let full = Column::from_vec_v1(
                masked_trace_coefficients_on_coset_v1(&coefficients, 1, common).unwrap(),
            );
            for selected in [vec![0, rows / 2, rows - 1], (0..rows).collect()] {
                let order = destinations(common, &selected);
                let output = cached_evaluate(&coefficients, 1, common, &selected, &roots, &order);
                let bounded = evaluate_v1(&coefficients, 1, common, &selected).unwrap();
                for (i, &row) in selected.iter().enumerate() {
                    assert_eq!(output[i], full[row]);
                    assert_eq!(output[i], bounded[i]);
                }
            }
        }
    }
}

#[test]
#[ignore = "full registered native-domain relative CPU cost experiment; no production policy or performance qualification"]
fn registered_public_twiddle_table_cost_keeps_complete_masks_and_query_coordinates() {
    use std::time::Instant;
    let common = 22;
    let rows = 1usize << common;
    let selected = (0..136)
        .flat_map(|query| {
            let block = (query * 1729 + 17) % (rows / 16);
            block * 16..block * 16 + 16
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 2176);
    let started = Instant::now();
    let roots = powers(common);
    let order = destinations(common, &selected);
    println!(
        "selected_twiddle_setup common={common} public_power_bytes={} public_index_bytes={} elapsed_ns={}",
        roots.capacity() * size_of::<F>(),
        order.capacity() * size_of::<usize>(),
        started.elapsed().as_nanos()
    );
    let domains = AggregateProofLayoutV1::for_full_profile_v1()
        .unwrap()
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(domains, [5, 8, 15, 16, 18, 19].into_iter().collect());
    let workers = 20;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .unwrap();
    for native in domains {
        for width in [1, 8] {
            let inputs = (0..width)
                .map(|column| {
                    let values = (0..1usize << native)
                        .map(|i| F((i * 17 + column * 43 + 13) as u64))
                        .collect::<Vec<_>>();
                    let mask = (0..1816)
                        .map(|i| F((i * 29 + column * 71 + 7) as u64))
                        .collect::<Vec<_>>();
                    Column::from_vec_v1(
                        masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            for round in 0..3 {
                let mut results = Vec::new();
                for cached in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let started = Instant::now();
                    let output = pool.install(|| {
                        inputs
                            .par_iter()
                            .map(|input| {
                                if cached {
                                    cached_evaluate(
                                        input, native, common, &selected, &roots, &order,
                                    )
                                } else {
                                    evaluate_v1(input, native, common, &selected).unwrap()
                                }
                            })
                            .collect::<Vec<_>>()
                    });
                    println!(
                        "selected_twiddle_cost native={native} common={common} width={width} workers={workers} round={round} cached={cached} selected={} coefficient_count={} elapsed_ns={}",
                        selected.len(),
                        inputs[0].len(),
                        started.elapsed().as_nanos()
                    );
                    assert!(output.iter().all(|c| c.len() == selected.len()));
                    results.push(output);
                }
                for (a, b) in results[0].iter().zip(&results[1]) {
                    assert_eq!(&**a, &**b);
                }
            }
        }
    }
}
