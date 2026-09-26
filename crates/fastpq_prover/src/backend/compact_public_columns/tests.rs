//! Projection, periodic polynomial, and full-reference-row reconstruction checks.

use super::*;
use crate::{
    GoldilocksFp4V1,
    backend::{
        FriDomain, GOLDILOCKS_MODULUS, compact_transfer_air::CompactTransferAir, field_pow, mul_mod,
    },
    fft::Planner,
    gadgets::{
        compact_blake2b_air::{CompactHashWitness, CompactRow},
        compact_smt_air::{
            DigestLimbs, PATH_LEVELS, PhysicalSmtWitness, PublicStatement, PublicUpdate, SmtRow,
            SmtWitness,
        },
        compact_trace_columns::smt_row_cells,
        transfer_integer_air::IntegerAirField,
    },
};
use fastpq_isi::FASTPQ_FINAL_V1;
use iroha_crypto::Hash;

fn digest(seed: u8) -> DigestLimbs {
    let hash = Hash::new([seed; 33]);
    let bytes: &[u8; 32] = hash.as_ref();
    core::array::from_fn(|limb| {
        u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
    })
}

fn root(mut child: DigestLimbs, siblings: &[DigestLimbs; PATH_LEVELS], path: u32) -> DigestLimbs {
    for (level, sibling) in siblings.iter().enumerate() {
        let (left, right) = if path >> level & 1 == 0 {
            (child, *sibling)
        } else {
            (*sibling, child)
        };
        let mut bytes = b"fastpq:v1:smt:node|".to_vec();
        for limb in left.into_iter().chain(right) {
            bytes.extend(limb.to_le_bytes());
        }
        let hash = Hash::new(bytes);
        let bytes: &[u8; 32] = hash.as_ref();
        child = core::array::from_fn(|limb| {
            u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
        });
    }
    child
}

fn statement() -> (PublicStatement, [DigestLimbs; PATH_LEVELS]) {
    let siblings = core::array::from_fn(|level| digest(u8::try_from(level + 17).unwrap()));
    let path = 0xa59c_71e3;
    let first = digest(1);
    let second = digest(2);
    let old_root = root(first, &siblings, path);
    (
        PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: first,
                    new_leaf: second,
                    path,
                },
                PublicUpdate {
                    old_leaf: second,
                    new_leaf: first,
                    path,
                },
            ],
            old_root,
            new_root: old_root,
        },
        siblings,
    )
}

fn physical_fixture() -> PhysicalSmtWitness {
    let (statement, siblings) = statement();
    SmtWitness::from_inputs(&statement, &[siblings, siblings])
        .unwrap()
        .into_physical()
}

// Independent native hash rows supply the periodic interpolation data. This
// does not call the projection owner's public-value formulas.
fn period_rows() -> Vec<[u64; COLUMN_COUNT]> {
    let mut payload = b"fastpq:v1:smt:node|".to_vec();
    payload.extend((0_u16..64).map(|byte| (byte * 19 + 7).to_le_bytes()[0]));
    let hash = CompactHashWitness::from_bytes(&payload).unwrap();
    (0..PHYSICAL_HASH_ROWS)
        .map(|phase| {
            let hash = hash
                .rows()
                .get(phase)
                .copied()
                .unwrap_or_else(CompactRow::zero);
            smt_row_cells(&SmtRow {
                hash,
                old_child: core::array::from_fn(|limb| 10 + limb as u64),
                new_child: core::array::from_fn(|limb| 20 + limb as u64),
                sibling: core::array::from_fn(|limb| 30 + limb as u64),
                starting_root: core::array::from_fn(|limb| 40 + limb as u64),
            })
        })
        .collect()
}

fn period_coefficients() -> Vec<Vec<u64>> {
    let rows = period_rows();
    let mut columns: Vec<Vec<u64>> = PUBLIC_COLUMNS
        .iter()
        .map(|&column| rows.iter().map(|row| row[column]).collect())
        .collect();
    Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut columns);
    columns
}

fn public_horner<F: PolynomialField>(
    coefficients: &[Vec<u64>],
    point: F,
) -> [F; PUBLIC_COLUMN_COUNT] {
    let reduced_point = point.power((PHYSICAL_ROW_COUNT / PHYSICAL_HASH_ROWS) as u64);
    core::array::from_fn(|column| {
        coefficients[column]
            .iter()
            .rev()
            .fold(F::ZERO, |sum, &value| {
                sum.mul(reduced_point).add(F::embed_base(value))
            })
    })
}

#[test]
fn exact_schema_omits_only_the_source_verified_public_cells() {
    assert_eq!(
        LAYOUT_ID,
        "fastpq:compact:smt-public-columns:v1:342-to-301:period512:rows65536:public32-35,53-63,276-301:node83:execute408"
    );
    assert_eq!(COLUMN_COUNT, 342);
    assert_eq!(COMMITTED_COLUMN_COUNT, 301);
    assert_eq!(PUBLIC_POLYNOMIAL_DEGREE, 65_408);
    let expected: Vec<_> = (32..36).chain(53..64).chain(276..302).collect();
    assert_eq!(PUBLIC_COLUMNS.as_slice(), expected);
    let mut complete: Vec<_> = PUBLIC_COLUMNS
        .into_iter()
        .chain(COMMITTED_COLUMNS)
        .collect();
    complete.sort_unstable();
    assert_eq!(complete, (0..342).collect::<Vec<_>>());
    assert!(COMMITTED_COLUMNS.windows(2).all(|pair| pair[0] < pair[1]));
    for retained in [31, 36, 52, 64, 275, 302, 309, 310, 341] {
        assert!(COMMITTED_COLUMNS.contains(&retained));
    }
    // These limbs are fixed only in part, and cannot be removed as whole cells.
    let original = period_rows();
    let mut different = b"fastpq:v1:smt:node|".to_vec();
    different.extend([0xff; 64]);
    let changed = CompactHashWitness::from_bytes(&different).unwrap();
    assert_ne!(original[0][36], changed.rows()[0].message[4]);
    assert_ne!(original[0][52], changed.rows()[0].message[20]);
}

#[test]
fn every_physical_row_projects_exactly_and_every_phase_reconstructs_full_reference_cells() {
    let reconstruction = PublicColumnReconstruction::new(&FASTPQ_FINAL_V1).unwrap();
    let witness = physical_fixture();
    let mut point = 1;
    for (index, row) in witness.rows().iter().enumerate() {
        let complete = smt_row_cells(row);
        let position = PhysicalRowIndex::new(index).unwrap();
        let committed = project_base_row(position, &complete).unwrap();
        for (retained, &source) in COMMITTED_COLUMNS.iter().enumerate() {
            assert_eq!(committed[retained], complete[source]);
        }
        // Every phase in the first hash plus every invocation boundary and final
        // row uses its actual full-subgroup point, including both SMT updates.
        if index < PHYSICAL_HASH_ROWS || position.phase() == 0 || position.phase() == 511 {
            assert_eq!(
                reconstruction.reconstruct_at(point, &committed).unwrap(),
                complete
            );
        }
        point = mul_mod(point, reconstruction.generator);
    }
    assert_eq!(point, 1);
    assert_eq!(witness.rows().len(), PHYSICAL_ROW_COUNT);
}

#[test]
fn complete_source_columns_are_borrowed_in_exact_retained_order() {
    let zeros = vec![0; PHYSICAL_ROW_COUNT];
    let known: Vec<Vec<u64>> = (0..PUBLIC_COLUMN_COUNT)
        .map(|slot| {
            (0..PHYSICAL_ROW_COUNT)
                .map(|row| base_values(PhysicalRowIndex::new(row).unwrap())[slot])
                .collect()
        })
        .collect();
    let mut columns = vec![zeros.as_slice(); COLUMN_COUNT];
    for (&column, values) in PUBLIC_COLUMNS.iter().zip(&known) {
        columns[column] = values;
    }
    // Distinct cells on both sides of every omitted range make a retained
    // column permutation visible even though the other rows share zero storage.
    let sentinel_sources = [31, 36, 52, 64, 275, 302, 341];
    let sentinels: Vec<Vec<u64>> = sentinel_sources
        .iter()
        .enumerate()
        .map(|(slot, _)| {
            let mut values = zeros.clone();
            values[0] = slot as u64 + 1;
            values[PHYSICAL_ROW_COUNT - 1] = slot as u64 + 101;
            values
        })
        .collect();
    for (&column, values) in sentinel_sources.iter().zip(&sentinels) {
        columns[column] = values;
    }
    let source = SourceTraceColumns::new(&columns).unwrap();
    let first = source.committed_row(0).unwrap();
    let last = source.committed_row(PHYSICAL_ROW_COUNT - 1).unwrap();
    for (slot, &column) in sentinel_sources.iter().enumerate() {
        let retained = COMMITTED_COLUMNS.binary_search(&column).unwrap();
        assert_eq!(first[retained], slot as u64 + 1);
        assert_eq!(last[retained], slot as u64 + 101);
        assert!(core::ptr::eq(
            source.committed_column(retained).unwrap(),
            sentinels[slot].as_slice()
        ));
    }
    assert_eq!(
        first.iter().filter(|&&value| value != 0).count(),
        sentinels.len()
    );
    assert_eq!(
        last.iter().filter(|&&value| value != 0).count(),
        sentinels.len()
    );
    for (index, &column) in COMMITTED_COLUMNS.iter().enumerate() {
        assert!(core::ptr::eq(
            source.committed_column(index).unwrap(),
            columns[column]
        ));
    }
    assert!(source.committed_column(COMMITTED_COLUMN_COUNT).is_err());
    assert!(source.committed_row(PHYSICAL_ROW_COUNT).is_err());
    assert!(SourceTraceColumns::new(&columns[..COLUMN_COUNT - 1]).is_err());

    let short = [0_u64; 1];
    let last = COMMITTED_COLUMNS[COMMITTED_COLUMN_COUNT - 1];
    columns[last] = &short;
    assert!(SourceTraceColumns::new(&columns).is_err());
    columns[last] = sentinels[sentinels.len() - 1].as_slice();

    let mut invalid = zeros.clone();
    invalid[PHYSICAL_ROW_COUNT - 1] = GOLDILOCKS_MODULUS;
    columns[last] = &invalid;
    assert!(matches!(
        SourceTraceColumns::new(&columns),
        Err(crate::Error::NonCanonicalGoldilocksElement { context, indices })
            if context == "deep_source_trace" && indices == [last, PHYSICAL_ROW_COUNT - 1]
    ));
    columns[last] = sentinels[sentinels.len() - 1].as_slice();

    let mut changed = known[PUBLIC_COLUMN_COUNT - 1].clone();
    changed[PHYSICAL_ROW_COUNT - 1] += 1;
    columns[PUBLIC_COLUMNS[PUBLIC_COLUMN_COUNT - 1]] = &changed;
    assert!(SourceTraceColumns::new(&columns).is_err());
}

#[test]
fn periodic_polynomials_match_full_ifft_reduced_lde_and_horner() {
    let reconstruction = PublicColumnReconstruction::new(&FASTPQ_FINAL_V1).unwrap();
    let rows = period_rows();
    let coefficients = period_coefficients();
    let repetitions = PHYSICAL_ROW_COUNT / PHYSICAL_HASH_ROWS;
    let selected = [0, 4, 15, 26, 39, 40];
    let mut full: Vec<Vec<u64>> = selected
        .iter()
        .map(|&slot| {
            (0..PHYSICAL_ROW_COUNT)
                .map(|row| rows[row % PHYSICAL_HASH_ROWS][PUBLIC_COLUMNS[slot]])
                .collect()
        })
        .collect();
    Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut full);
    for (&slot, full) in selected.iter().zip(&full) {
        for (degree, &coefficient) in full.iter().enumerate() {
            let expected = if degree % repetitions == 0 {
                coefficients[slot][degree / repetitions]
            } else {
                0
            };
            assert_eq!(coefficient, expected, "slot={slot}, degree={degree}");
        }
        assert!(
            full[PUBLIC_POLYNOMIAL_DEGREE + 1..]
                .iter()
                .all(|&value| value == 0)
        );
    }
    // Q(X)=q(X^128); the smaller LDE therefore uses coset shift a^128.
    let mut short_params = FASTPQ_FINAL_V1;
    short_params.omega_coset = field_pow(short_params.omega_coset, repetitions as u64);
    let lde = Planner::new(&short_params).lde_columns(&coefficients);
    let lde_rows = PHYSICAL_ROW_COUNT * FASTPQ_FINAL_V1.fri.blowup_factor as usize;
    let period_lde = PHYSICAL_HASH_ROWS * FASTPQ_FINAL_V1.fri.blowup_factor as usize;
    let domain = FriDomain::from_lde_parameters(
        FASTPQ_FINAL_V1.lde_root,
        FASTPQ_FINAL_V1.lde_log_size,
        lde_rows,
        FASTPQ_FINAL_V1.omega_coset,
    )
    .unwrap();
    for index in (0..period_lde).chain([period_lde, 123_456, lde_rows - 1]) {
        let point = domain.point(index);
        let actual = reconstruction.evaluate(point).unwrap();
        for column in 0..PUBLIC_COLUMN_COUNT {
            assert_eq!(
                actual[column],
                lde[column][index % period_lde],
                "column={column}, index={index}"
            );
        }
    }
    for point in [0, 1, 7, GOLDILOCKS_MODULUS - 1, domain.point(79)] {
        assert_eq!(
            reconstruction.evaluate(point).unwrap(),
            public_horner(&coefficients, point)
        );
    }
    // The nonzero prefix/length columns are not constants away from execution rows.
    assert_ne!(
        reconstruction.evaluate(domain.point(0)).unwrap()[LENGTH_OFFSET],
        83
    );
}

#[test]
fn arbitrary_fp4_points_preserve_coordinates_and_next_trace_point() {
    type F = GoldilocksFp4V1;
    let reconstruction = PublicColumnReconstruction::new(&FASTPQ_FINAL_V1).unwrap();
    let coefficients = period_coefficients();
    let current: [F; COMMITTED_COLUMN_COUNT] = core::array::from_fn(|column| {
        F::new([column as u64 + 1, 3, 5, GOLDILOCKS_MODULUS - 1]).unwrap()
    });
    let next = current.map(|value| value.add(F::new([7, 11, 13, 17]).unwrap()));
    let (statement, _) = statement();
    let reference_air = CompactTransferAir::new(&statement, None).unwrap();
    for point in [
        F::ZERO,
        F::ONE,
        F::new([2, 3, 5, 7]).unwrap(),
        F::new([0, 1, 0, 0]).unwrap(),
        F::new([GOLDILOCKS_MODULUS - 1, 11, 13, 17]).unwrap(),
    ] {
        let known = public_horner(&coefficients, point);
        assert_eq!(reconstruction.evaluate(point).unwrap(), known);
        let next_point = point.scale_base(reconstruction.generator);
        let next_known = public_horner(&coefficients, next_point);
        let mut expected_current = [F::ZERO; COLUMN_COUNT];
        let mut expected_next = [F::ZERO; COLUMN_COUNT];
        for (retained, &column) in COMMITTED_COLUMNS.iter().enumerate() {
            expected_current[column] = current[retained];
            expected_next[column] = next[retained];
        }
        for (public, &column) in PUBLIC_COLUMNS.iter().enumerate() {
            expected_current[column] = known[public];
            expected_next[column] = next_known[public];
        }
        let (actual_current, actual_next) = reconstruction
            .reconstruct_pair_at(point, &current, &next)
            .unwrap();
        assert_eq!(
            (actual_current, actual_next),
            (expected_current, expected_next)
        );
        assert_eq!(
            reference_air
                .evaluate_at(point, &actual_current, &actual_next)
                .unwrap(),
            reference_air
                .evaluate_at(point, &expected_current, &expected_next)
                .unwrap()
        );
    }
    let point = F::new([2, 3, 5, 7]).unwrap();
    let public = reconstruction.evaluate(point).unwrap();
    assert!(
        public
            .iter()
            .any(|value| value.coefficients()[1..].iter().any(|&word| word != 0))
    );
    assert_ne!(
        public,
        reconstruction.evaluate(F::from_base(2).unwrap()).unwrap()
    );
    let lde_generator = FriDomain::from_lde_parameters(
        FASTPQ_FINAL_V1.lde_root,
        FASTPQ_FINAL_V1.lde_log_size,
        PHYSICAL_ROW_COUNT * 8,
        FASTPQ_FINAL_V1.omega_coset,
    )
    .unwrap()
    .generator;
    assert_ne!(
        reconstruction
            .evaluate(point.scale_base(reconstruction.generator))
            .unwrap(),
        reconstruction
            .evaluate(point.scale_base(lde_generator))
            .unwrap()
    );
}

#[test]
fn malformed_geometry_widths_public_cells_and_all_field_coordinates_fail() {
    type F = GoldilocksFp4V1;
    let reconstruction = PublicColumnReconstruction::new(&FASTPQ_FINAL_V1).unwrap();
    let index = PhysicalRowIndex::new(0).unwrap();
    let complete = period_rows()[0];
    let committed = project_base_row(index, &complete).unwrap();
    for length in [
        0,
        COMMITTED_COLUMN_COUNT - 1,
        COMMITTED_COLUMN_COUNT + 1,
        COLUMN_COUNT,
    ] {
        assert!(reconstruction.reconstruct_at(1, &vec![0; length]).is_err());
    }
    for length in [0, COLUMN_COUNT - 1, COLUMN_COUNT + 1] {
        assert!(project_base_row(index, &vec![0; length]).is_err());
    }
    for column in PUBLIC_COLUMNS {
        let mut changed = complete;
        changed[column] = changed[column].add(1);
        assert!(
            project_base_row(index, &changed).is_err(),
            "public column {column}"
        );
    }
    for column in 0..COLUMN_COUNT {
        for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
            let mut changed = complete;
            changed[column] = invalid;
            assert!(matches!(project_base_row(index, &changed),
                Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column]));
        }
    }
    for column in 0..COMMITTED_COLUMN_COUNT {
        let mut changed = committed;
        changed[column] = GOLDILOCKS_MODULUS;
        assert!(matches!(reconstruction.reconstruct_at(1, &changed),
            Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column]));
        for lane in 0..4 {
            let mut opening = [F::ZERO; COMMITTED_COLUMN_COUNT];
            let mut value = [0; 4];
            value[lane] = GOLDILOCKS_MODULUS;
            opening[column] = F::from_coefficients_unchecked_for_test(value);
            assert!(matches!(reconstruction.reconstruct_at(F::ONE, &opening),
                Err(Error::NonCanonicalGoldilocksElement { indices, .. }) if indices == [column, lane]));
        }
    }
    for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
        assert!(reconstruction.reconstruct_at(invalid, &committed).is_err());
    }
    for lane in 0..4 {
        let mut point = [0; 4];
        point[lane] = GOLDILOCKS_MODULUS;
        assert!(
            reconstruction
                .reconstruct_at(
                    F::from_coefficients_unchecked_for_test(point),
                    &[F::ZERO; COMMITTED_COLUMN_COUNT]
                )
                .is_err()
        );
    }
    let mut invalid_params = FASTPQ_FINAL_V1;
    invalid_params.trace_log_size = 15;
    assert!(PublicColumnReconstruction::new(&invalid_params).is_err());
    invalid_params = FASTPQ_FINAL_V1;
    invalid_params.omega_coset = 1;
    assert!(PublicColumnReconstruction::new(&invalid_params).is_err());
    assert!(PhysicalRowIndex::new(PHYSICAL_ROW_COUNT).is_none());
}
