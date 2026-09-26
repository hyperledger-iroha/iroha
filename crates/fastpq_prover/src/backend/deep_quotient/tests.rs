//! Preflight, fixed-polynomial mapping and actual shared quotient controls.

use super::*;
use crate::backend::{
    GOLDILOCKS_MODULUS,
    compact_public_columns::{COMMITTED_COLUMNS, PublicColumnReconstruction, project_base_row},
    fixed_domain::FixedTraceDomain,
    polynomial_division::VanishingDivisionPlan,
    polynomial_reference,
};
use crate::gadgets::compact_smt_air::{DigestLimbs, PATH_LEVELS, PublicStatement, PublicUpdate};
use fastpq_isi::FASTPQ_FINAL_V1;
use iroha_crypto::Hash;

fn limits() -> DeepQuotientLimits {
    DeepQuotientLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
    }
}

fn dense(seed: u64) -> F {
    F::new([seed + 1, 2 * seed + 3, 3 * seed + 5, 5 * seed + 7]).unwrap()
}

fn digest(seed: u8) -> DigestLimbs {
    let hash = Hash::new([seed; 33]);
    let bytes: &[u8; 32] = hash.as_ref();
    core::array::from_fn(|limb| {
        u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
    })
}

fn air() -> CompactTransferAir {
    CompactTransferAir::new(
        &PublicStatement {
            updates: [
                PublicUpdate {
                    old_leaf: digest(1),
                    new_leaf: digest(2),
                    path: 17,
                },
                PublicUpdate {
                    old_leaf: digest(3),
                    new_leaf: digest(4),
                    path: 31,
                },
            ],
            old_root: digest(5),
            new_root: digest(6),
        },
        None,
    )
    .unwrap()
}

fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul(point).add(value))
}

fn shape<T>(result: Result<T>) {
    assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
}

#[test]
fn source_shapes_coordinates_and_preparation_limits_fail_before_fft() {
    for width in [
        0,
        COMMITTED_COLUMN_COUNT - 1,
        COMMITTED_COLUMN_COUNT + 1,
        COLUMN_COUNT,
    ] {
        shape(PreparedDeepTrace::prepare(&vec![&[][..]; width], limits()));
    }
    let empty = [&[][..]; COMMITTED_COLUMN_COUNT];
    let too_long = vec![0; TRACE_ROWS + 1];
    let mut source = empty;
    source[COMMITTED_COLUMN_COUNT - 1] = &too_long;
    shape(PreparedDeepTrace::prepare(&source, limits()));
    for column in 0..COMMITTED_COLUMN_COUNT {
        let bad = [0, GOLDILOCKS_MODULUS];
        let mut source = empty;
        source[column] = &bad;
        assert!(matches!(PreparedDeepTrace::prepare(&source, limits()),
            Err(Error::NonCanonicalGoldilocksElement { context: "deep_quotient_source", indices }) if indices == [column, 1]));
    }
    let (bytes, work) = preparation_cost(&empty).unwrap();
    for policy in [
        DeepQuotientLimits {
            max_payload_bytes: bytes - 1,
            ..limits()
        },
        DeepQuotientLimits {
            max_work_units: work - 1,
            ..limits()
        },
    ] {
        assert!(matches!(
            PreparedDeepTrace::prepare(&empty, policy),
            Err(Error::VerifierLimitExceeded { .. })
        ));
    }
    assert!(preparation_cost(&empty).unwrap().0 >= PUBLIC_COLUMN_COUNT * TRACE_ROWS * F::BYTES);
}

#[test]
fn all_public_coefficients_match_projection_and_independent_period_interpolation() {
    let owned: Vec<Vec<u64>> = (0..COMMITTED_COLUMN_COUNT)
        .map(|column| {
            (0..column % 7)
                .map(|degree| (11 * column + 13 * degree + 1) as u64)
                .collect()
        })
        .collect();
    let projected: Vec<_> = owned.iter().map(Vec::as_slice).collect();
    let prepared = PreparedDeepTrace::prepare(&projected, limits()).unwrap();
    assert!(prepared.column(COLUMN_COUNT).is_err());
    for (slot, &reference) in COMMITTED_COLUMNS.iter().enumerate() {
        let values = prepared.column(reference).unwrap();
        assert_eq!(values.len(), projected[slot].len().max(1));
        for (degree, &value) in projected[slot].iter().enumerate() {
            assert_eq!(values[degree], F::embed_base(value));
        }
        assert_eq!(prepared.degree_bounds()[reference], values.len());
    }
    let stride = TRACE_ROWS / PHYSICAL_HASH_ROWS;
    for (slot, &reference) in PUBLIC_COLUMNS.iter().enumerate() {
        let values = prepared.column(reference).unwrap();
        assert_eq!(values.len(), TRACE_ROWS);
        assert_eq!(
            prepared.degree_bounds()[reference],
            PUBLIC_POLYNOMIAL_DEGREE + 1
        );
        for (degree, value) in values.iter().enumerate() {
            assert_eq!(value.coefficients()[1..], [0; 3]);
            if degree % stride != 0 {
                assert_eq!(*value, F::ZERO, "reference={reference}, degree={degree}");
            }
        }
        if [0, 4, 15, 39, 40].contains(&slot) {
            let period = (0..PHYSICAL_HASH_ROWS)
                .map(|phase| base_values(PhysicalRowIndex::new(phase).unwrap())[slot])
                .collect::<Vec<_>>();
            let expected = polynomial_reference::interpolate(&period);
            for (degree, &value) in expected.iter().enumerate() {
                assert_eq!(values[degree * stride], F::embed_base(value));
            }
        }
    }
    let reconstruction = PublicColumnReconstruction::new(&FASTPQ_FINAL_V1).unwrap();
    let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, TRACE_ROWS)
        .unwrap()
        .generator;
    for x in [
        F::ZERO,
        F::ONE,
        F::embed_base(generator.power(407)),
        dense(17),
        dense(17).mul_base(generator),
    ] {
        let expected = reconstruction.evaluate(x).unwrap();
        for (slot, &reference) in PUBLIC_COLUMNS.iter().enumerate() {
            let sparse = prepared
                .column(reference)
                .unwrap()
                .iter()
                .step_by(stride)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(
                horner(&sparse, x.power(stride as u64)),
                expected[slot],
                "slot={slot}"
            );
        }
    }
    let relation = air();
    let plan = prepared.plan(&relation, limits()).unwrap();
    assert!(plan.payload_bytes() > NUMERATOR_ROWS * COLUMN_COUNT * F::BYTES);
    assert!(plan.work_units() > NUMERATOR_ROWS);
    for policy in [
        DeepQuotientLimits {
            max_payload_bytes: plan.payload_bytes() - 1,
            ..limits()
        },
        DeepQuotientLimits {
            max_work_units: plan.work_units() - 1,
            ..limits()
        },
    ] {
        assert!(matches!(
            prepared.plan(&relation, policy),
            Err(Error::VerifierLimitExceeded { .. })
        ));
    }
    assert!(prepared.build(&relation, &[F::ONE; 922], limits()).is_err());
    let mut alpha = [F::ONE; 923];
    alpha[922] = F::from_coefficients_unchecked_for_test([0, 0, GOLDILOCKS_MODULUS, 0]);
    assert!(matches!(
        prepared.build(&relation, &alpha, limits()),
        Err(Error::NonCanonicalGoldilocksElement {
            context: "masked_numerator_alpha",
            ..
        })
    ));
}

#[test]
fn shared_coefficient_plan_checks_complete_degree_padding_and_resources() {
    let relation = air();
    let one = [F::ONE];
    let columns = [&one[..]; COLUMN_COUNT];
    let degrees = [1; COLUMN_COUNT];
    let make = |columns: &[&[F]], degrees: &[usize], rows, extent, policy| {
        // Only inspect preflight outcomes here; no retained borrow escapes.
        MaskedQuotientPlan::from_coefficients(
            &relation,
            columns,
            degrees,
            rows,
            dense(3),
            extent,
            policy,
        )
        .map(|plan| (plan.payload_bytes(), plan.work_units()))
    };
    let policy = limits().shared();
    let (bytes, work) = make(
        &columns,
        &degrees,
        NUMERATOR_ROWS,
        QUOTIENT_COEFFICIENTS,
        policy,
    )
    .unwrap();
    for width in [0, COLUMN_COUNT - 1, COLUMN_COUNT + 1] {
        shape(make(
            &vec![&one[..]; width],
            &degrees,
            NUMERATOR_ROWS,
            QUOTIENT_COEFFICIENTS,
            policy,
        ));
        shape(make(
            &columns,
            &vec![1; width],
            NUMERATOR_ROWS,
            QUOTIENT_COEFFICIENTS,
            policy,
        ));
    }
    let mut wrong = degrees;
    wrong[341] = 0;
    shape(make(
        &columns,
        &wrong,
        NUMERATOR_ROWS,
        QUOTIENT_COEFFICIENTS,
        policy,
    ));
    wrong[341] = 2;
    shape(make(
        &columns,
        &wrong,
        NUMERATOR_ROWS,
        QUOTIENT_COEFFICIENTS,
        policy,
    ));
    let padding = [F::ONE, F::ONE];
    let mut wrong_columns = columns;
    wrong_columns[341] = &padding;
    shape(make(
        &wrong_columns,
        &degrees,
        NUMERATOR_ROWS,
        QUOTIENT_COEFFICIENTS,
        policy,
    ));
    for lane in 0..4 {
        let mut words = [0; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let malformed = [F::from_coefficients_unchecked_for_test(words)];
        let mut wrong_columns = columns;
        wrong_columns[341] = &malformed;
        assert!(
            matches!(make(&wrong_columns, &degrees, NUMERATOR_ROWS, QUOTIENT_COEFFICIENTS, policy),
            Err(Error::NonCanonicalGoldilocksElement {context: "coefficient_quotient_source", indices}) if indices == [0, lane])
        );
    }
    for policy in [
        MaskedQuotientLimits {
            max_payload_bytes: bytes - 1,
            ..policy
        },
        MaskedQuotientLimits {
            max_work_units: work - 1,
            ..policy
        },
    ] {
        assert!(matches!(
            make(
                &columns,
                &degrees,
                NUMERATOR_ROWS,
                QUOTIENT_COEFFICIENTS,
                policy
            ),
            Err(Error::VerifierLimitExceeded { .. })
        ));
    }
    let bounds = relation
        .numerator_degree_bounds(&[TRACE_ROWS; COLUMN_COUNT])
        .unwrap();
    assert_eq!(bounds.combined_numerator(), 196_479);
    assert_eq!(bounds.conditional_quotients().combined, 130_943);
    assert!(NUMERATOR_ROWS >= bounds.combined_numerator());
    assert!(QUOTIENT_COEFFICIENTS >= bounds.conditional_quotients().combined);
}

#[test]
fn small_full_polynomial_transform_and_exact_division_preserve_guarded_quotient() {
    let expected = (0..6).map(|index| dense(23 + index)).collect::<Vec<_>>();
    let mut numerator = vec![F::ZERO; 16];
    for (degree, &value) in expected.iter().enumerate() {
        numerator[degree] = numerator[degree].sub(value);
        numerator[degree + 4] = numerator[degree + 4].add(value);
    }
    let domain = PolynomialDomain::new(16, dense(31), 16, usize::MAX).unwrap();
    assert_eq!(domain.numerator_rotation(4).unwrap(), 4);
    let lanes = domain.evaluate(&numerator, 10).unwrap();
    let evaluations: Vec<_> = (0..16).map(|index| lanes.value(index).unwrap()).collect();
    for (index, &value) in evaluations.iter().enumerate() {
        assert_eq!(value, horner(&numerator, domain.point(index).unwrap()));
    }
    let full = domain.interpolate(&evaluations).unwrap();
    assert_eq!(&*full, numerator);
    let plan = VanishingDivisionPlan::new(4, 16, 10, 8, usize::MAX, usize::MAX).unwrap();
    let quotient = plan.divide(&full).unwrap();
    assert_eq!(quotient.degree_bound(), 6);
    let guarded = quotient.into_coefficients();
    assert_eq!(&guarded[..6], expected);
    assert_eq!(&guarded[6..], [F::ZERO; 2]);
    numerator[0] = numerator[0].add(F::ONE);
    shape(plan.divide(&numerator));
}

fn actual_smt_statement() -> (PublicStatement, [DigestLimbs; PATH_LEVELS]) {
    let siblings = core::array::from_fn(|level| digest((level + 17) as u8));
    let path = 0xa59c_71e3;
    let first = digest(1);
    let second = digest(2);
    let mut root = first;
    for (level, sibling) in siblings.iter().enumerate().take(PATH_LEVELS) {
        let (left, right) = if path >> level & 1 == 0 {
            (root, *sibling)
        } else {
            (*sibling, root)
        };
        let mut payload = b"fastpq:v1:smt:node|".to_vec();
        for digest in [left, right] {
            for limb in digest {
                payload.extend_from_slice(&limb.to_le_bytes());
            }
        }
        let hash = Hash::new(payload);
        let bytes: &[u8; 32] = hash.as_ref();
        root = core::array::from_fn(|i| {
            u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap())
        });
    }
    let statement = PublicStatement {
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
        old_root: root,
        new_root: root,
    };
    (statement, siblings)
}

/// Rebuild the identical fixture statement under a caller-selected test context.
/// This does not construct a witness or run a polynomial transform.
pub(in crate::backend) fn actual_smt_relation(caller_context: Option<&[u8]>) -> CompactTransferAir {
    CompactTransferAir::new(&actual_smt_statement().0, caller_context).unwrap()
}

/// Construct the actual prover-only SMT fixture and all 301 subgroup coefficients.
///
/// Shared by explicitly ignored full quotient/proof diagnostics. This allocates
/// the genuine 65,536-row witness; its private rows are never a verifier input.
pub(in crate::backend) fn actual_smt_fixture() -> (CompactTransferAir, Vec<Vec<u64>>) {
    use crate::{
        Planner,
        gadgets::{compact_smt_air::PhysicalSmtWitness, compact_trace_columns::smt_row_cells},
    };
    let (statement, siblings) = actual_smt_statement();
    let witness = PhysicalSmtWitness::from_inputs(&statement, &[siblings, siblings]).unwrap();
    let mut projected = vec![vec![0; TRACE_ROWS]; COMMITTED_COLUMN_COUNT];
    for (index, row) in witness.rows().iter().enumerate() {
        let values =
            project_base_row(PhysicalRowIndex::new(index).unwrap(), &smt_row_cells(row)).unwrap();
        for (column, value) in projected.iter_mut().zip(values) {
            column[index] = value;
        }
    }
    drop(witness);
    Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut projected);
    let relation =
        CompactTransferAir::new(&statement, Some(b"actual unmasked quotient control")).unwrap();
    (relation, projected)
}

#[test]
#[ignore = "actual 65536x301 unmasked SMT quotient on262144 points; several GiB; no PCS qualification"]
fn actual_unmasked_smt_uses_shared_4n_quotient_and_matches_full_ood_air() {
    let (relation, projected) = actual_smt_fixture();
    let refs = projected.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepared = PreparedDeepTrace::prepare(&refs, limits()).unwrap();
    drop(refs);
    drop(projected);
    let alphas = (0..923).map(|slot| dense(101 + slot)).collect::<Vec<_>>();
    let quotient = prepared.build(&relation, &alphas, limits()).unwrap();
    assert_eq!(quotient.len(), QUOTIENT_COEFFICIENTS);
    let bound = relation
        .numerator_degree_bounds(prepared.degree_bounds())
        .unwrap()
        .conditional_quotients()
        .combined;
    assert!(quotient[bound..].iter().all(|value| value.is_zero()));
    let z = dense(1031);
    let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, TRACE_ROWS)
        .unwrap()
        .generator;
    let current = (0..COLUMN_COUNT)
        .map(|column| horner(prepared.column(column).unwrap(), z))
        .collect::<Vec<_>>();
    let next = (0..COLUMN_COUNT)
        .map(|column| horner(prepared.column(column).unwrap(), z.mul_base(generator)))
        .collect::<Vec<_>>();
    let numerator = relation
        .evaluate_at(z, &current, &next)
        .unwrap()
        .into_iter()
        .zip(alphas)
        .fold(F::ZERO, |sum, (residue, alpha)| sum.add(residue.mul(alpha)));
    assert_eq!(
        numerator,
        z.power(TRACE_ROWS as u64)
            .sub(F::ONE)
            .mul(horner(&quotient, z))
    );
}
