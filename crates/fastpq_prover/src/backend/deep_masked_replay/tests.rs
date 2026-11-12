//! Independent Horner/FFT parity, entropy rejection and bounded-work regressions.

use super::*;
use rand::{SeedableRng, TryRng, rngs::StdRng};

fn limits(passes: usize) -> ReplayLimits {
    ReplayLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: passes,
    }
}
fn small(
    rows: usize,
    width: usize,
    mask: usize,
    passes: usize,
) -> (MaskedTraceReplay, Vec<Vec<u64>>) {
    let plan = MaskedReplayPlan::with_shape(rows, width, width, mask, 3.min(rows), limits(passes))
        .unwrap();
    // Dense coefficients, including the highest degree, exercise every FFT lane.
    let source: Vec<Vec<u64>> = (0..width)
        .map(|column| {
            let mut values: Vec<u64> = (0..rows)
                .map(|degree| ((column + 3) * 7919 + degree * degree * 17 + degree * 13) as u64)
                .collect();
            cyclotomic::fft(&mut values, plan.trace_domain);
            values
        })
        .collect();
    let mut rng = StdRng::from_seed([19; 32]);
    let replay = MaskedTraceReplay::from_validated_columns(
        plan,
        |column| source[column].as_slice(),
        &mut rng,
    )
    .unwrap();
    (replay, source)
}
fn horner(coefficients: &[u64], point: u64) -> u64 {
    coefficients.iter().rev().fold(0, |value, &coefficient| {
        add_mod(mul_mod(value, point), coefficient)
    })
}
fn explicit_columns(replay: &MaskedTraceReplay) -> Vec<Vec<u64>> {
    (0..replay.plan.width)
        .map(|column| {
            (0..replay.coefficient_extent())
                .map(|degree| replay.coefficient(column, degree))
                .collect()
        })
        .collect()
}

#[test]
fn masked_128_stripes_match_full_materialization_horner_and_next_rotation() {
    let (mut replay, source) = small(16, 3, 7, 1);
    let plan = replay.plan();
    let columns = explicit_columns(&replay);
    for (column, coefficients) in columns.iter().enumerate() {
        for (row, &expected) in source[column].iter().enumerate().take(plan.rows) {
            assert_eq!(
                horner(
                    coefficients,
                    field_pow(plan.trace_domain.generator, row as u64)
                ),
                expected,
                "mask vanishes on every source row"
            );
        }
    }
    let materialized: Vec<Vec<u64>> = columns
        .iter()
        .map(|coefficients| {
            let mut values = vec![0; plan.rows * plan.stripes];
            let mut power = 1;
            for (value, &coefficient) in values.iter_mut().zip(coefficients) {
                *value = mul_mod(coefficient, power);
                power = mul_mod(power, COSET_OFFSET);
            }
            cyclotomic::fft(
                &mut values,
                Domain {
                    log_size: (plan.rows * plan.stripes).ilog2(),
                    generator: plan.domain.generator,
                },
            );
            values
        })
        .collect();
    let mut visited = vec![false; plan.rows * plan.stripes];
    let mut count = 0;
    replay
        .visit_all(|stripe| {
            count += 1;
            for row in 0..stripe.rows() {
                let mut current = [0; 3];
                let mut next = [0; 3];
                stripe.fill_row(row, &mut current)?;
                stripe.fill_row((row + 1) % stripe.rows(), &mut next)?;
                let index = stripe.global_index(row);
                let point = stripe.point(row);
                let next_point = mul_mod(point, plan.trace_domain.generator);
                assert!(!visited[index]);
                visited[index] = true;
                for column in 0..3 {
                    assert_eq!(current[column], materialized[column][index]);
                    assert_eq!(current[column], horner(&columns[column], point));
                    assert_eq!(next[column], horner(&columns[column], next_point));
                    // A quotient-style callback uses both shifted private values at
                    // the natural x, catching an index/rotation error beyond roots.
                    assert_eq!(
                        sub_mod(mul_mod(current[column], next[column]), point),
                        sub_mod(
                            mul_mod(
                                horner(&columns[column], point),
                                horner(&columns[column], next_point)
                            ),
                            point
                        )
                    );
                }
            }
            assert!(stripe.fill_row(stripe.rows(), &mut [0; 3]).is_err());
            assert!(stripe.fill_row(0, &mut [0; 2]).is_err());
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 128);
    assert!(visited.into_iter().all(|value| value));
    assert!(
        replay
            .visit_all(|_| panic!("exhausted pass must not invoke callback"))
            .is_err()
    );
}

/// The rows of `indices` as canonical leaf payloads, through the direct evaluator.
fn evaluated(replay: &mut MaskedTraceReplay, indices: &[usize]) -> Result<Vec<Vec<u64>>> {
    let width = replay.width();
    let mut payload = vec![0xa5; indices.len() * width * 8];
    replay.evaluate_selected_rows(indices, &mut payload)?;
    Ok(payload
        .chunks_exact(width * 8)
        .map(|row| {
            row.as_chunks::<8>()
                .0
                .iter()
                .map(|word| u64::from_le_bytes(*word))
                .collect()
        })
        .collect())
}

#[test]
fn selected_rows_match_horner_at_frontiers_and_refuse_other_selections() {
    let (mut replay, _) = small(4, 1, 4, 2);
    let coefficients = explicit_columns(&replay);
    let domain = replay.plan.domain;
    // First and last row, both sides of a stripe boundary and adjacent leaves.
    let indices = [0, 1, 127, 128, 129, 511];
    let rows = evaluated(&mut replay, &indices).unwrap();
    for (&index, row) in indices.iter().zip(&rows) {
        assert_eq!(row, &[horner(&coefficients[0], domain.point(index))]);
    }
    assert_eq!(replay.remaining_passes, 1);
    // Empty, out-of-range, unsorted, duplicate and overlong selections, and an
    // output of another length, are refused without charging a pass.
    assert_eq!(QUERY_COUNT, 77);
    for invalid in [
        vec![],
        vec![512],
        vec![3, 1],
        vec![0, 0],
        (0..=2 * QUERY_COUNT).collect(),
    ] {
        assert!(evaluated(&mut replay, &invalid).is_err());
    }
    assert!(replay.evaluate_selected_rows(&[0, 1], &mut [0; 8]).is_err());
    assert!(replay.evaluate_selected_rows(&[0], &mut [0; 16]).is_err());
    assert_eq!(replay.remaining_passes, 1);
    assert!(
        replay
            .visit_all(|_| Err(invalid("intentional callback error")))
            .is_err()
    );
    assert_eq!(replay.remaining_passes, 0);
    assert!(evaluated(&mut replay, &[0]).is_err());
}

#[test]
fn selected_rows_accept_the_exact_query_plus_sibling_boundary() {
    assert_eq!(QUERY_COUNT, 77);
    let (mut replay, _) = small(4, 1, 4, 1);
    let coefficients = explicit_columns(&replay);
    let domain = replay.plan.domain;
    // All 154 distinct current and sibling rows are one selection.
    let indices = (0..2 * QUERY_COUNT).collect::<Vec<_>>();
    let rows = evaluated(&mut replay, &indices).unwrap();
    assert_eq!(rows.len(), 154);
    for (&index, row) in indices.iter().zip(&rows) {
        assert_eq!(row, &[horner(&coefficients[0], domain.point(index))]);
    }
    assert!(replay.ensure_pass_available().is_err());
}

#[test]
fn masked_replay_highest_coefficients_and_fresh_entropy_are_retained() {
    let (replay, _) = small(256, 1, TRACE_MASK_COEFFICIENTS, 1);
    assert_eq!(TRACE_MASK_COEFFICIENTS, 162);
    assert_eq!(replay.coefficient_extent(), 418);
    assert_eq!(replay.coefficient(0, 417), replay.entropy.trace[161]);
    assert_ne!(replay.coefficient(0, 417), 0);
    assert_ne!(replay.coefficient(0, 255), 0);
    assert_eq!(replay.composition_mask().len(), 512);
    assert_eq!(replay.quotient_mask().len(), 3);
    assert!(
        replay
            .composition_mask()
            .iter()
            .any(|value| value.coefficients()[1] != 0)
    );
    assert!(!replay.has_candidate_geometry());
    assert!(
        super::super::deep_polynomial::DeepPolynomialSource::from_replay(&replay, [&[], &[]])
            .is_err()
    );
    let mut rng = StdRng::from_seed([77; 32]);
    let first = ReplayEntropy::sample(replay.plan, &mut rng).unwrap();
    let second = ReplayEntropy::sample(replay.plan, &mut rng).unwrap();
    assert_ne!(&*first.trace, &*second.trace);
    assert_ne!(&*first.quotient, &*second.quotient);
    assert_ne!(&*first.composition, &*second.composition);
}

struct CountingRng {
    calls: usize,
    value: u64,
    failure: bool,
}
#[derive(Debug)]
struct CountingRngError;
impl core::fmt::Display for CountingRngError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("test entropy failure")
    }
}
impl std::error::Error for CountingRngError {}
impl TryRng for CountingRng {
    type Error = CountingRngError;
    fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
        // A 32-bit draw keeps the low half of the 64-bit draw.
        self.try_next_u64()
            .map(|value| u32::try_from(value & u64::from(u32::MAX)).expect("masked to 32 bits"))
    }
    fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
        self.calls += 1;
        if self.failure {
            Err(CountingRngError)
        } else {
            Ok(self.value)
        }
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> core::result::Result<(), Self::Error> {
        for chunk in destination.chunks_mut(8) {
            chunk.copy_from_slice(&self.try_next_u64()?.to_le_bytes()[..chunk.len()]);
        }
        Ok(())
    }
}
impl TryCryptoRng for CountingRng {}

#[test]
fn masked_entropy_is_unbiased_bounded_and_preflight_precedes_reads() {
    let plan = MaskedReplayPlan::new(limits(4)).unwrap();
    assert_eq!(plan.entropy_attempts, 5);
    assert_eq!(plan.coefficient_bytes, 301 * 65_536 * 8);
    assert_eq!(plan.stripe_bytes, plan.coefficient_bytes);
    assert_eq!(plan.source_bytes, 342 * 65_536 * 8);
    assert_eq!(plan.entropy_bytes, 301 * 162 * 8 + 78 * 32 + 131_072 * 32);
    // 77 queried rows and at most 77 leaf siblings retain 301 values each.
    assert_eq!(plan.max_selected, 154);
    assert_eq!(plan.selected_bytes, 154 * 301 * 8);
    assert_eq!(plan.payload_bytes, 499_885_600);
    assert_eq!(plan.maximum_column_transforms, 301 * (1 + 4 * 128));
    assert!(
        MaskedReplayPlan::new(ReplayLimits {
            max_payload_bytes: plan.payload_bytes - 1,
            ..limits(4)
        })
        .is_err()
    );
    assert!(
        MaskedReplayPlan::new(ReplayLimits {
            max_work_units: plan.work_units - 1,
            ..limits(4)
        })
        .is_err()
    );
    assert!(MaskedReplayPlan::new(limits(0)).is_err());
    assert!(MaskedReplayPlan::new(limits(usize::MAX)).is_err());
    assert!(MaskedReplayPlan::with_shape(3, 1, 1, 1, 1, limits(1)).is_err());
    assert!(MaskedReplayPlan::with_shape(4, 1, 1, 5, 1, limits(1)).is_err());
    let mut rng = CountingRng {
        calls: 0,
        value: GOLDILOCKS_MODULUS,
        failure: false,
    };
    assert!(
        MaskedTraceReplay::new(
            ReplayLimits {
                max_payload_bytes: 0,
                ..limits(1)
            },
            &[],
            &mut rng
        )
        .is_err()
    );
    assert_eq!(rng.calls, 0);
    assert!(MaskedTraceReplay::new(limits(1), &[], &mut rng).is_err());
    assert_eq!(rng.calls, 0);
    assert!(sample_base(&mut rng, plan.entropy_attempts).is_err());
    assert_eq!(rng.calls, 5);
    rng.value = GOLDILOCKS_MODULUS - 1;
    assert_eq!(sample_base(&mut rng, 5).unwrap(), GOLDILOCKS_MODULUS - 1);
    rng.value = 0;
    assert_eq!(sample_base(&mut rng, 5).unwrap(), 0);
    rng.failure = true;
    assert!(sample_base(&mut rng, 5).is_err());
    assert_eq!(rng.calls, 8);
    let small = MaskedReplayPlan::with_shape(4, 1, 1, 2, 2, limits(1)).unwrap();
    assert!(ReplayEntropy::sample(small, &mut rng).is_err());
}

#[test]
fn nested_replay_refuses_foreign_dimensions_and_small_fixture_is_not_a_candidate() {
    let mut replay = MaskedTraceReplay::arithmetic_fixture(&[&[1, 2, 3, 4]], 4, 1).unwrap();
    for rows in [0, 2, 3, 1024] {
        assert!(
            replay
                .visit_subdomain(rows, |_, _| panic!("invalid domain visited"))
                .is_err()
        );
    }
    assert!(replay.ensure_pass_available().is_ok());
    let mut stripes = 0;
    replay
        .visit_subdomain(16, |stripe, step| {
            assert_eq!(step, 32);
            for row in 0..stripe.rows() {
                assert_eq!(stripe.global_index(row) % step, 0);
            }
            stripes += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(stripes, 4);
    assert!(replay.ensure_pass_available().is_err());
    assert!(MaskedTraceReplay::arithmetic_fixture(&[], 4, 1).is_err());
    assert!(MaskedTraceReplay::arithmetic_fixture(&[&[0; 4]], 32, 1).is_err());
    assert!(
        MaskedTraceReplay::arithmetic_fixture(&[&[0, 0, 0, GOLDILOCKS_MODULUS]], 4, 1).is_err()
    );
}

#[test]
fn masked_stripe_fft_matches_fp4_and_horner_at_the_parallel_threshold() {
    use crate::backend::{
        polynomial_field::PolynomialField, polynomial_transform::PolynomialDomain,
    };

    for rows in [16, 64, 4096] {
        let (mut replay, _) = small(rows, 1, 3, 1);
        let coefficients = explicit_columns(&replay).remove(0);
        let plan = replay.plan();
        let mut seen = 0;
        replay
            .visit_subdomain(4 * rows, |stripe, step| {
                assert_eq!(step, 32);
                seen += 1;
                let offset = stripe.point(0);
                let offset_n = field_pow(offset, rows as u64);
                // On this coset X^N=offset^N. Fold the explicit masked
                // polynomial independently before the shared four-lane FFT.
                let effective = (0..rows)
                    .map(|degree| {
                        F::embed_base(coefficients[degree]).add(
                            F::embed_base(*coefficients.get(rows + degree).unwrap_or(&0))
                                .mul_base(offset_n),
                        )
                    })
                    .collect::<Vec<_>>();
                let domain =
                    PolynomialDomain::new(rows, F::embed_base(offset), rows, 2 * rows * F::BYTES)?;
                let expected = domain.evaluate(&effective, rows)?;
                for index in 0..rows {
                    let mut actual = [0];
                    stripe.fill_row(index, &mut actual)?;
                    assert!(actual[0] < GOLDILOCKS_MODULUS);
                    assert_eq!(expected.value(index)?, F::embed_base(actual[0]));
                    assert_eq!(stripe.point(index), domain.point(index)?.coefficients()[0]);
                    if rows <= 64
                        || [0, 1, 2, rows / 3, rows / 2 - 1, rows / 2, rows - 1].contains(&index)
                    {
                        assert_eq!(actual[0], horner(&coefficients, stripe.point(index)));
                    }
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(seen, 4);
        assert_eq!(replay.remaining_passes, 0);
        assert_eq!(plan.rows, rows);
    }
}

#[test]
fn physical_source_shape_canonicality_and_public_projection_precede_entropy() {
    let zeros = vec![0; TRACE_ROWS];
    let mut rng = CountingRng {
        calls: 0,
        value: 0,
        failure: true,
    };
    for width in [0, 300, 301, 302, 341, 343] {
        assert!(matches!(
            MaskedTraceReplay::new(limits(1), &vec![zeros.as_slice(); width], &mut rng),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    for length in [0, TRACE_ROWS - 1, TRACE_ROWS + 1] {
        let changed = vec![0; length];
        let mut columns = vec![zeros.as_slice(); 342];
        columns[341] = &changed;
        assert!(matches!(
            MaskedTraceReplay::new(limits(1), &columns, &mut rng),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    for (column, row) in [(0, 0), (150, 7), (341, TRACE_ROWS - 1)] {
        let mut changed = zeros.clone();
        changed[row] = GOLDILOCKS_MODULUS;
        let mut columns = vec![zeros.as_slice(); 342];
        columns[column] = &changed;
        assert!(matches!(
            MaskedTraceReplay::new(limits(1), &columns, &mut rng),
            Err(Error::NonCanonicalGoldilocksElement { context: "deep_source_trace", indices })
                if indices == [column, row]
        ));
    }
    assert!(matches!(
        MaskedTraceReplay::new(limits(1), &vec![zeros.as_slice(); 342], &mut rng),
        Err(Error::InvalidTraceShape { details }) if details.contains("source public column")
    ));
    assert_eq!(rng.calls, 0);
}

#[test]
fn selected_rows_equal_the_committed_stripe_rows_in_every_column_and_worker_count() {
    let (mut replay, _) = small(16, 3, 7, 3);
    let columns = explicit_columns(&replay);
    let plan = replay.plan();
    // The rows the commitment pass hashed: every stripe through the transform.
    let mut committed = vec![vec![0; 3]; plan.lde_rows()];
    replay
        .visit_all(|stripe| {
            for row in 0..stripe.rows() {
                stripe.fill_row(row, &mut committed[stripe.global_index(row)])?;
            }
            Ok(())
        })
        .unwrap();
    // Mask extent 7 exceeds no row count here; rows span several stripes.
    let indices = [0, 1, 63, 64, 127, 128, 129, 2047];
    for threads in [1, 4] {
        let rows = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| evaluated(&mut replay, &indices))
            .unwrap();
        for (&index, row) in indices.iter().zip(&rows) {
            assert_eq!(row, &committed[index], "row {index}");
            for (column, &value) in row.iter().enumerate() {
                assert_eq!(value, horner(&columns[column], plan.domain.point(index)));
            }
        }
    }
    assert!(replay.ensure_pass_available().is_err());
}

#[test]
fn row_payload_bytes_are_the_little_endian_words_of_the_gathered_row() {
    let (mut replay, _) = small(16, 3, 7, 1);
    let width = replay.width();
    replay
        .visit_all(|stripe| {
            let mut row = vec![0; width];
            let mut payload = vec![0xa5; width * 8];
            for index in 0..stripe.rows() {
                stripe.fill_row(index, &mut row)?;
                stripe.fill_row_bytes(index, &mut payload)?;
                let expected: Vec<u8> = row.iter().flat_map(|word| word.to_le_bytes()).collect();
                assert_eq!(payload, expected);
            }
            // A row past the stripe and a payload of another width are refused.
            assert!(stripe.fill_row_bytes(stripe.rows(), &mut payload).is_err());
            assert!(
                stripe
                    .fill_row_bytes(0, &mut payload[..width * 8 - 1])
                    .is_err()
            );
            assert!(
                stripe
                    .fill_row_bytes(0, &mut vec![0; width * 8 + 8])
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
}
