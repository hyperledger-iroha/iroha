//! Algebra, allocation and row/frontier parity against the materialized planner.

use super::*;
use crate::{
    Error,
    backend::{
        AirQuotientDomain, GOLDILOCKS_MODULUS, GoldilocksFp4V1, add_mod,
        compact_v1::{Context, Oracle},
        field_pow,
        merkle_multiproof::{MultiproofLimits, MultiproofPlan},
        sub_mod,
    },
    fft::Planner,
};
use fastpq_isi::GoldilocksDigest384V1 as Digest;
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;

fn source(rows: usize, width: usize) -> Vec<Vec<u64>> {
    (0..width)
        .map(|column| {
            (0..rows)
                .map(|row| ((column + 11) * (row * row + 7) + row * 31) as u64)
                .collect()
        })
        .collect()
}

fn reference(columns: &[Vec<u64>]) -> Vec<Vec<u64>> {
    let planner = Planner::new(&FASTPQ_FINAL_V1);
    let mut coefficients = columns.to_vec();
    planner.ifft_columns(&mut coefficients);
    planner.lde_columns(&coefficients)
}

#[test]
fn replay_matches_every_materialized_cell_and_next_rotation() {
    for rows in [1, 2, 4, 16, 64] {
        for width in [1, 3] {
            let columns = source(rows, width);
            let materialized = reference(&columns);
            let plan = TraceReplayPlan::new(rows, width).unwrap();
            let replay = TraceReplay::new(plan, &columns).unwrap();
            let mut visits = vec![0; plan.lde_rows];
            replay
                .replay_all(|stripe| {
                    let mut current = vec![0; width];
                    let mut next = vec![0; width];
                    for row in 0..rows {
                        let index = stripe.global_index(row);
                        visits[index] += 1;
                        stripe.fill_row(row, &mut current);
                        stripe.fill_row((row + 1) % rows, &mut next);
                        for column in 0..width {
                            assert_eq!(current[column], materialized[column][index]);
                            assert_eq!(
                                next[column],
                                materialized[column][(index + plan.stripes) % plan.lde_rows]
                            );
                        }
                    }
                    Ok(())
                })
                .unwrap();
            assert!(visits.iter().all(|&count| count == 1));
            let indices = [plan.lde_rows - 1, 0, plan.stripes - 1, 0];
            let selected = replay.selected_rows(&indices).unwrap();
            let expected: Vec<Vec<u64>> = indices
                .iter()
                .map(|&index| materialized.iter().map(|c| c[index]).collect())
                .collect();
            assert_eq!(
                norito::encode_canonical(&selected).unwrap(),
                norito::encode_canonical(&expected).unwrap()
            );
            assert!(replay.selected_rows(&[]).unwrap().is_empty());
        }
    }
}

#[test]
fn highest_degree_basis_mixing_and_quotients_match_natural_domain_reference() {
    let plan = TraceReplayPlan::new(16, 3).unwrap();
    let generator = plan.trace_domain.generator;
    // Include X^(N-1), a dense coefficient polynomial and a separate X^3
    // basis. Direct Horner evaluation is independent of both FFT schedules.
    let coefficients = [
        (0..16)
            .map(|power| u64::from(power == 15))
            .collect::<Vec<_>>(),
        (0..16)
            .map(|power| GOLDILOCKS_MODULUS - 1 - power * 7919)
            .collect(),
        (0..16)
            .map(|power| if power == 3 { 37 } else { 0 })
            .collect(),
    ];
    let evaluate = |column: &[u64], point: u64| {
        column.iter().rev().fold(0, |sum, &coefficient| {
            add_mod(mul_mod(sum, point), coefficient)
        })
    };
    let columns: Vec<_> = coefficients
        .iter()
        .map(|column| {
            (0..plan.trace_rows)
                .map(|row| evaluate(column, field_pow(generator, row as u64)))
                .collect()
        })
        .collect();
    let replay = TraceReplay::new(plan, &columns).unwrap();
    let materialized = reference(&columns);
    let mixing = [
        GoldilocksFp4V1::new([2, 3, 5, 7]).unwrap(),
        GoldilocksFp4V1::new([11, 13, 17, 19]).unwrap(),
        GoldilocksFp4V1::new([23, 29, 31, 37]).unwrap(),
    ];
    let mixed = replay
        .collect_rows(GoldilocksFp4V1::ZERO, |stripe, rows| {
            rows.map(|row| {
                Ok(stripe
                    .columns()
                    .zip(mixing)
                    .fold(GoldilocksFp4V1::ZERO, |sum, (column, mix)| {
                        sum.add(mix.mul_base(column[row]))
                    }))
            })
            .collect()
        })
        .unwrap();
    let weights = AirQuotientDomain::new(&FASTPQ_FINAL_V1, plan.lde_rows).unwrap();
    let quotient = replay
        .collect_rows(GoldilocksFp4V1::ZERO, |stripe, rows| {
            let mut current = vec![0; plan.width];
            let mut next = vec![0; plan.width];
            rows.map(|row| {
                stripe.fill_row(row, &mut current);
                stripe.fill_row((row + 1) % plan.trace_rows, &mut next);
                let index = stripe.global_index(row);
                let point = plan.lde_domain.point(index);
                let residue = sub_mod(mul_mod(current[0], next[1]), mul_mod(point, next[2]));
                Ok(mixing[0]
                    .mul_base(residue)
                    .mul_base(weights.weights_at(index)?.all_rows))
            })
            .collect()
        })
        .unwrap();
    for index in 0..plan.lde_rows {
        let point = plan.lde_domain.point(index);
        let next = (index + plan.stripes) % plan.lde_rows;
        for column in 0..plan.width {
            assert_eq!(
                materialized[column][index],
                evaluate(&coefficients[column], point)
            );
        }
        let expected_mix = materialized
            .iter()
            .zip(mixing)
            .fold(GoldilocksFp4V1::ZERO, |sum, (column, mix)| {
                sum.add(mix.mul_base(column[index]))
            });
        assert_eq!(mixed[index].to_le_bytes(), expected_mix.to_le_bytes());
        let residue = sub_mod(
            mul_mod(materialized[0][index], materialized[1][next]),
            mul_mod(point, materialized[2][next]),
        );
        let expected_quotient = mixing[0]
            .mul_base(residue)
            .mul_base(weights.weights_at(index).unwrap().all_rows);
        assert_eq!(
            quotient[index].to_le_bytes(),
            expected_quotient.to_le_bytes()
        );
    }
}

#[test]
fn replay_reuses_one_stripe_and_respects_its_exact_payload_and_work_plan() {
    let plan = TraceReplayPlan::new(16, 3).unwrap();
    let replay = TraceReplay::new(plan, &source(16, 3)).unwrap();
    assert_eq!(
        replay.coefficients.len() * size_of::<u64>(),
        plan.coefficient_bytes
    );
    let mut transforms = plan.width; // Initial column interpolation.
    for _ in 0..4 {
        let mut address = None;
        let mut visits = 0;
        replay
            .replay_all(|stripe| {
                assert_eq!(std::mem::size_of_val(stripe.values), plan.stripe_bytes);
                assert_eq!(
                    plan.coefficient_bytes + plan.stripe_bytes,
                    plan.peak_trace_bytes
                );
                let pointer = stripe.values.as_ptr();
                assert_eq!(*address.get_or_insert(pointer), pointer);
                visits += 1;
                transforms += plan.width;
                Ok(())
            })
            .unwrap();
        assert_eq!(visits, plan.stripes);
    }
    assert_eq!(transforms, plan.maximum_column_transforms);
    let production = TraceReplayPlan::new(65_536, 342).unwrap();
    assert_eq!(production.stripe_bytes, 179_306_496);
    assert_eq!(production.peak_trace_bytes, 358_612_992);
    assert_eq!(production.maximum_column_transforms, 11_286);
    assert_eq!(production.stripe_bytes * production.stripes, 1_434_451_968);
}

#[test]
fn replay_rejects_invalid_geometry_cells_and_selections_and_stops_on_error() {
    for (rows, width) in [(0, 3), (3, 3), (usize::MAX, 3), (16, 0), (16, 513)] {
        assert!(TraceReplayPlan::new(rows, width).is_err());
    }
    let plan = TraceReplayPlan::new(4, 3).unwrap();
    let mut columns = source(4, 3);
    columns[2][3] = GOLDILOCKS_MODULUS;
    assert!(
        matches!(TraceReplay::new(plan, &columns), Err(Error::NonCanonicalGoldilocksElement { context: "compact_base_trace", indices }) if indices == [2, 3])
    );
    columns[2].pop();
    assert!(TraceReplay::new(plan, &columns).is_err());
    assert!(TraceReplay::new(plan, &columns[..2]).is_err());
    let replay = TraceReplay::new(plan, &source(4, 3)).unwrap();
    assert!(replay.selected_rows(&[plan.lde_rows]).is_err());
    assert!(replay.selected_rows(&vec![0; plan.lde_rows + 1]).is_err());
    let mut visits = 0;
    assert!(
        replay
            .replay_all(|_| {
                visits += 1;
                Err(shape("injected row-commitment failure"))
            })
            .is_err()
    );
    assert_eq!(visits, 1);
}

fn leaf(context: &Context, index: usize, row: &[u64]) -> Digest {
    let bytes: Vec<_> = row.iter().flat_map(|value| value.to_le_bytes()).collect();
    context
        .hash_leaf(Oracle::Row, index as u32, &bytes)
        .unwrap()
}

fn parent(
    context: &Context,
    level: usize,
    index: usize,
    left: Digest,
    right: Digest,
) -> Result<Digest> {
    context
        .hash_parent(Oracle::Row, level as u32, index as u32, left, right)
        .map_err(|_| shape("test parent failed"))
}

fn levels(context: &Context, leaves: Vec<Digest>) -> Vec<Vec<Digest>> {
    let mut levels = vec![leaves];
    while levels.last().unwrap().len() > 1 {
        let level = levels.len();
        let next = levels
            .last()
            .unwrap()
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| parent(context, level, index, pair[0], pair[1]).unwrap())
            .collect();
        levels.push(next);
    }
    levels
}

#[test]
fn replayed_row_roots_paths_and_query_frontiers_are_byte_identical() {
    // The same fixed row hash/framing owner is exercised on a small tree. This
    // is storage/commitment parity, not qualification of a reduced AIR profile.
    let context = Context::new(b"trace-stripe-row-parity").unwrap();
    let columns = source(4, 342);
    let materialized = reference(&columns);
    let plan = TraceReplayPlan::new(4, 342).unwrap();
    let replay = TraceReplay::new(plan, &columns).unwrap();
    let replayed_leaves = replay
        .collect_rows(Digest::default(), |stripe, indices| {
            let mut row = vec![0; plan.width];
            indices
                .map(|local| {
                    stripe.fill_row(local, &mut row);
                    Ok(leaf(&context, stripe.global_index(local), &row))
                })
                .collect()
        })
        .unwrap();
    let reference_leaves: Vec<_> = (0..plan.lde_rows)
        .map(|index| {
            let row: Vec<_> = materialized.iter().map(|column| column[index]).collect();
            leaf(&context, index, &row)
        })
        .collect();
    let replayed = levels(&context, replayed_leaves);
    let reference = levels(&context, reference_leaves);
    assert_eq!(replayed, reference);
    let limits = MultiproofLimits {
        max_depth: 5,
        max_queried_leaves: 32,
        max_siblings: 32,
        max_parent_hashes: 32,
    };
    for indices in [
        vec![0],
        vec![31],
        vec![0, 1],
        vec![0, 8, 24],
        vec![7, 23, 31],
        (0..32).collect(),
    ] {
        let opening = MultiproofPlan::new(32, &indices, limits).unwrap();
        let replayed_frontier = opening
            .open_with(&replayed, |l, i, a, b| parent(&context, l, i, a, b))
            .unwrap();
        let reference_frontier = opening
            .open_with(&reference, |l, i, a, b| parent(&context, l, i, a, b))
            .unwrap();
        let wire = |frontier: Vec<Digest>| {
            frontier
                .into_iter()
                .map(WireDigest::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            norito::encode_canonical(&wire(replayed_frontier)).unwrap(),
            norito::encode_canonical(&wire(reference_frontier)).unwrap()
        );
        let rows = replay.selected_rows(&indices).unwrap();
        let leaves: Vec<_> = indices
            .iter()
            .zip(&rows)
            .map(|(&index, row)| leaf(&context, index, row))
            .collect();
        assert_eq!(
            leaves,
            indices
                .iter()
                .map(|&index| reference[0][index])
                .collect::<Vec<_>>()
        );
    }
}
