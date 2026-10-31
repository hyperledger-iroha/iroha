//! Isolated validated native and retained DEEP accumulation cost controls.

use super::*;

fn boundary_columns(length: usize) -> Vec<ZeroizingMainTraceColumnV1> {
    let p = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    (0..BATCH)
        .map(|column| {
            ZeroizingMainTraceColumnV1(
                (0..length)
                    .map(|index| F(p - 1 - ((index + column) % 31) as u64))
                    .collect(),
            )
        })
        .collect()
}

fn scales(column: usize) -> [E; 2] {
    let p = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    core::array::from_fn(|side| {
        E::from_coefficients(core::array::from_fn(|lane| {
            F(p - 1 - (column + side + lane) as u64)
        }))
        .unwrap()
    })
}

#[test]
#[ignore = "native19 validated DEEP accumulation cost; component timing only"]
fn widened_native_weighted_accumulation_cost_preserves_scalar_batches() {
    assert!(!cfg!(debug_assertions), "use an optimized build");
    let native = boundary_columns(1 << 19);
    let masks = boundary_columns(MASK_LENGTH);
    let points = MainNativeDeepPointsV1::new_v1(19, E::ONE).unwrap();
    let inputs = (0..BATCH)
        .map(|column| {
            let values = [
                E::from_base(native[column][0]),
                E::from_base(native[column][1]),
            ];
            // Validate every original claim once outside the accumulation timer.
            assert_eq!(
                points.evaluate_v1(&native[column], &masks[column]).unwrap(),
                values
            );
            (&*native[column], &*masks[column], values, scales(column))
        })
        .collect::<Vec<_>>();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for width in [1, 4, BATCH] {
            let inputs = &inputs[..width];
            for round in 0..2 {
                let mut owners: [_; 2] =
                    core::array::from_fn(|_| MainNativeDeepQuotientV1::new_v1(&points).unwrap());
                let mut elapsed = [std::time::Duration::ZERO; 2];
                for mode in if round == 0 { [0, 1] } else { [1, 0] } {
                    let owner = &mut owners[mode];
                    let start = std::time::Instant::now();
                    pool.install(|| {
                        for _ in 0..4 {
                            let columns = std::hint::black_box(inputs);
                            if mode == 0 {
                                for (side, target) in owner.coefficients.iter_mut().enumerate() {
                                    target.par_iter_mut().enumerate().for_each(
                                        |(index, target)| {
                                            for (native, mask, _, scales) in columns {
                                                let source = if index < points.native_rows {
                                                    native[index]
                                                } else {
                                                    mask[index - points.native_rows]
                                                };
                                                *target = target.add(scales[side].mul_base(source));
                                            }
                                        },
                                    );
                                }
                                for (_, _, values, scales) in columns {
                                    for side in 0..2 {
                                        owner.values[side] =
                                            owner.values[side].add(values[side].mul(scales[side]));
                                    }
                                }
                                owner.columns += columns.len();
                            } else {
                                owner.accumulate_validated_batch_v1(
                                    columns,
                                    owner.columns + columns.len(),
                                );
                            }
                        }
                    });
                    elapsed[mode] = start.elapsed();
                }
                assert_eq!(owners[0].columns, owners[1].columns);
                assert_eq!(owners[0].values, owners[1].values);
                for side in 0..2 {
                    assert_eq!(
                        &*owners[0].coefficients[side],
                        &*owners[1].coefficients[side]
                    );
                }
                eprintln!(
                    "x509_native_weighted log=19 width={width} workers={workers} iterations=4 round={round} scalar_seconds={:.6} widened_seconds={:.6} parity=exact",
                    elapsed[0].as_secs_f64(),
                    elapsed[1].as_secs_f64()
                );
            }
        }
    }
}

#[test]
#[ignore = "native19 retained DEEP accumulation cost; component timing only"]
fn widened_retained_weighted_accumulation_cost_preserves_scalar_batches() {
    assert!(!cfg!(debug_assertions), "use an optimized build");
    let coefficients = boundary_columns((1 << 19) + MASK_LENGTH);
    let powers = MainDeepPointPowersV1::new_v1(
        [
            E::ONE,
            E::from_base(goldilocks_primitive_root_v1(19).unwrap()),
        ],
        coefficients[0].len(),
    )
    .unwrap();
    let inputs = (0..BATCH)
        .map(|column| {
            let values = powers.points.map(|point| {
                coefficients[column]
                    .iter()
                    .rev()
                    .fold(E::ZERO, |sum, &value| {
                        sum.mul(point).add(E::from_base(value))
                    })
            });
            // Validate each original polynomial before timing only its mixing.
            assert_eq!(powers.evaluate_v1(&coefficients[column]).unwrap(), values);
            (&*coefficients[column], values, scales(column))
        })
        .collect::<Vec<_>>();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for width in [1, 4, BATCH] {
            let inputs = &inputs[..width];
            for round in 0..2 {
                let mut owners: [_; 2] =
                    core::array::from_fn(|_| MainGroupedDeepQuotientV1::new_v1(&powers).unwrap());
                let mut elapsed = [std::time::Duration::ZERO; 2];
                for mode in if round == 0 { [0, 1] } else { [1, 0] } {
                    let owner = &mut owners[mode];
                    let start = std::time::Instant::now();
                    pool.install(|| {
                        for _ in 0..4 {
                            let columns = std::hint::black_box(inputs);
                            if mode == 0 {
                                for (side, target) in owner.coefficients.iter_mut().enumerate() {
                                    target.par_iter_mut().enumerate().for_each(
                                        |(degree, target)| {
                                            for (coefficients, _, scales) in columns {
                                                *target = target.add(
                                                    scales[side].mul_base(coefficients[degree]),
                                                );
                                            }
                                        },
                                    );
                                }
                                for (_, values, scales) in columns {
                                    for side in 0..2 {
                                        owner.values[side] =
                                            owner.values[side].add(values[side].mul(scales[side]));
                                    }
                                }
                                owner.columns += columns.len();
                            } else {
                                let count = owner.columns + columns.len();
                                MainMixedDeepQuotientV1::accumulate_validated_retained_batch_v1(
                                    owner, columns, count,
                                );
                            }
                        }
                    });
                    elapsed[mode] = start.elapsed();
                }
                assert_eq!(owners[0].columns, owners[1].columns);
                assert_eq!(owners[0].values, owners[1].values);
                for side in 0..2 {
                    assert_eq!(
                        &*owners[0].coefficients[side],
                        &*owners[1].coefficients[side]
                    );
                }
                eprintln!(
                    "x509_retained_weighted log=19 width={width} workers={workers} iterations=4 round={round} scalar_seconds={:.6} widened_seconds={:.6} parity=exact",
                    elapsed[0].as_secs_f64(),
                    elapsed[1].as_secs_f64()
                );
            }
        }
    }
}
