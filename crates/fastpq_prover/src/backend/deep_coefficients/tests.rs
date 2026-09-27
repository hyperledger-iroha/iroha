//! Projection preflight and small/one-column full-domain IFFT equivalence checks.

use super::*;
use crate::{
    backend::{
        GOLDILOCKS_MODULUS,
        compact_public_columns::{PUBLIC_COLUMN_COUNT, PUBLIC_COLUMNS, PublicColumnReconstruction},
        field_pow,
    },
    fft::Planner,
    gadgets::{
        compact_blake2b_air::{CompactHashWitness, CompactRow},
        compact_trace_columns::hash_row_cells,
    },
};
use fastpq_isi::FASTPQ_FINAL_V1;

fn limits() -> DeepCoefficientLimits {
    let charge = DeepTraceCoefficients::required_resources().unwrap();
    DeepCoefficientLimits {
        max_payload_bytes: charge.payload_bytes,
        max_work_units: charge.work_units,
    }
}

// Independent native hash witness supplies the 512-period public cells. The
// remaining trace cells below are zero test data, never an asserted valid AIR.
fn public_columns() -> [Vec<u64>; PUBLIC_COLUMN_COUNT] {
    let mut payload = b"fastpq:v1:smt:node|".to_vec();
    payload.extend((0_u8..64).map(|byte| byte.wrapping_mul(19).wrapping_add(7)));
    let hash = CompactHashWitness::from_bytes(&payload).unwrap();
    let period = (0..512)
        .map(|phase| {
            hash_row_cells(
                &hash
                    .rows()
                    .get(phase)
                    .copied()
                    .unwrap_or_else(CompactRow::zero),
            )
        })
        .collect::<Vec<_>>();
    core::array::from_fn(|index| {
        (0..TRACE_ROWS)
            .map(|row| period[row % 512][PUBLIC_COLUMNS[index]])
            .collect()
    })
}

fn source<'a>(
    public: &'a [Vec<u64>; PUBLIC_COLUMN_COUNT],
    zero: &'a [u64],
) -> [&'a [u64]; COLUMN_COUNT] {
    core::array::from_fn(|column| match PUBLIC_COLUMNS.binary_search(&column) {
        Ok(index) => public[index].as_slice(),
        Err(_) => zero,
    })
}

fn horner(coefficients: &[u64], point: u64) -> u64 {
    coefficients.iter().rev().fold(0, |sum, &coefficient| {
        ((u128::from(sum) * u128::from(point) + u128::from(coefficient))
            % u128::from(GOLDILOCKS_MODULUS)) as u64
    })
}

#[test]
fn retained_order_guarded_views_and_ifft_match_planner_and_independent_horner() {
    for rows in [16, 64, 4096] {
        let trace = FixedTraceDomain::new(&FASTPQ_FINAL_V1, rows).unwrap();
        let domain = cyclotomic::Domain {
            log_size: rows.ilog2(),
            generator: trace.generator,
        };
        let expected = (0..COLUMN_COUNT)
            .map(|column| {
                (0..rows.min(8))
                    .map(|degree| (17 * column + 31 * degree + 1) as u64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let points = (0..rows)
            .map(|row| field_pow(trace.generator, row as u64))
            .collect::<Vec<_>>();
        let complete = expected
            .iter()
            .map(|polynomial| {
                points
                    .iter()
                    .map(|&point| horner(polynomial, point))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let views = core::array::from_fn(|column| complete[column].as_slice());
        let mut copied = copy_retained(&views).unwrap();
        let mut planner = COMMITTED_COLUMNS
            .iter()
            .map(|&column| complete[column].clone())
            .collect::<Vec<_>>();
        for (index, column) in copied.iter().enumerate() {
            assert_eq!(&**column, complete[COMMITTED_COLUMNS[index]]);
        }
        // Use the actual existing CPU planner as a parity oracle, while the
        // known coefficient vectors independently fix the interpolation answer.
        Planner::new(&FASTPQ_FINAL_V1).ifft_columns(&mut planner);
        interpolate_columns(&mut copied, domain);
        let owned = DeepTraceCoefficients { columns: copied };
        let output = owned.coefficients();
        assert_eq!(output.len(), COMMITTED_COLUMN_COUNT);
        for (index, actual) in output.into_iter().enumerate() {
            let polynomial = &expected[COMMITTED_COLUMNS[index]];
            assert_eq!(&actual[..polynomial.len()], polynomial);
            assert!(actual[polynomial.len()..].iter().all(|&value| value == 0));
            assert_eq!(actual, planner[index]);
            assert!(core::ptr::eq(
                actual.as_ptr(),
                owned.columns[index].as_ptr()
            ));
        }
    }
}

#[test]
fn one_full_n_column_preserves_exact_trace_generator_and_highest_coefficient() {
    let trace = FixedTraceDomain::new(&DeepGeometry::polynomial_parameters(), TRACE_ROWS).unwrap();
    assert_eq!(
        trace.generator,
        DeepGeometry::new().unwrap().trace_generator()
    );
    assert_eq!(trace.generator, FASTPQ_FINAL_V1.trace_root);
    // 5 + 7*X^(N-1) on the exact order-N subgroup. Consecutive powers of the
    // inverse root make this independent source vector linear-time to build.
    let inverse = field_pow(trace.generator, TRACE_ROWS as u64 - 1);
    let mut inverse_point = 1;
    let values = (0..TRACE_ROWS)
        .map(|_| {
            let value =
                ((5_u128 + 7 * u128::from(inverse_point)) % u128::from(GOLDILOCKS_MODULUS)) as u64;
            inverse_point = ((u128::from(inverse_point) * u128::from(inverse))
                % u128::from(GOLDILOCKS_MODULUS)) as u64;
            value
        })
        .collect::<Vec<_>>();
    let mut columns = [SecretPolynomial::from_slice(&values).unwrap()];
    interpolate_columns(
        &mut columns,
        cyclotomic::Domain {
            log_size: TRACE_ROWS.ilog2(),
            generator: trace.generator,
        },
    );
    assert_eq!(columns[0][0], 5);
    assert_eq!(columns[0][TRACE_ROWS - 1], 7);
    assert!(
        columns[0][1..TRACE_ROWS - 1]
            .iter()
            .all(|&value| value == 0)
    );
}

#[test]
fn fixed_shape_and_resource_preflight_reject_before_projection_or_fft() {
    let charge = DeepTraceCoefficients::required_resources().unwrap();
    assert!(charge.payload_bytes >= (COLUMN_COUNT + COMMITTED_COLUMN_COUNT) * TRACE_ROWS * 8);
    assert!(charge.work_units > COMMITTED_COLUMN_COUNT * TRACE_ROWS * TRACE_ROWS.ilog2() as usize);
    let zero = vec![0; TRACE_ROWS];
    let complete = [&zero[..]; COLUMN_COUNT];
    for width in [
        0,
        COMMITTED_COLUMN_COUNT,
        COLUMN_COUNT - 1,
        COLUMN_COUNT + 1,
    ] {
        assert!(matches!(
            DeepTraceCoefficients::from_columns(&vec![&zero[..]; width], limits()),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    let mut wrong_length = complete;
    wrong_length[COLUMN_COUNT - 1] = &zero[..TRACE_ROWS - 1];
    assert!(matches!(
        DeepTraceCoefficients::from_columns(&wrong_length, limits()),
        Err(Error::InvalidTraceShape { .. })
    ));
    // The all-zero matrix has wrong public cells. Limits must still fail first.
    assert!(
        matches!(DeepTraceCoefficients::from_columns(&complete, DeepCoefficientLimits { max_payload_bytes: charge.payload_bytes - 1, ..limits() }),
        Err(Error::VerifierLimitExceeded { limit: "max_deep_coefficient_payload_bytes", actual, max }) if actual == charge.payload_bytes && max == actual - 1)
    );
    assert!(
        matches!(DeepTraceCoefficients::from_columns(&complete, DeepCoefficientLimits { max_work_units: charge.work_units - 1, ..limits() }),
        Err(Error::VerifierLimitExceeded { limit: "max_deep_coefficient_work_units", actual, max }) if actual == charge.work_units && max == actual - 1)
    );
    assert!(check_limit("test", 7, 7).is_ok());
    assert!(check_limit("test", 8, 7).is_err());
}

#[test]
fn full_physical_preflight_reconstructs_public_cells_and_rejects_late_corruption() {
    let public = public_columns();
    let zero = vec![0; TRACE_ROWS];
    let complete = source(&public, &zero);
    validate_complete_rows(&complete).unwrap();
    let reconstruction =
        PublicColumnReconstruction::new(&DeepGeometry::polynomial_parameters()).unwrap();
    for row in [0, 1, 3, 5, 407, 408, 511, 512, TRACE_ROWS - 1] {
        let cells = core::array::from_fn::<_, COLUMN_COUNT, _>(|column| complete[column][row]);
        let projected = project_base_row(PhysicalRowIndex::new(row).unwrap(), &cells).unwrap();
        let point = field_pow(FASTPQ_FINAL_V1.trace_root, row as u64);
        assert_eq!(
            reconstruction.reconstruct_at(point, &projected).unwrap(),
            cells
        );
    }
    for column in [
        COMMITTED_COLUMNS[COMMITTED_COLUMN_COUNT - 1],
        PUBLIC_COLUMNS[PUBLIC_COLUMN_COUNT - 1],
    ] {
        let mut malformed = complete[column].to_vec();
        malformed[TRACE_ROWS - 1] = GOLDILOCKS_MODULUS;
        let mut changed = complete;
        changed[column] = &malformed;
        assert!(
            matches!(DeepTraceCoefficients::from_columns(&changed, limits()),
            Err(Error::NonCanonicalGoldilocksElement { context: "compact_public_base_row", indices }) if indices == [column])
        );
        assert_eq!(malformed[TRACE_ROWS - 1], GOLDILOCKS_MODULUS);
    }
    // A canonical but incorrect omitted cell must be rejected, never dropped.
    let column = PUBLIC_COLUMNS[0];
    let mut malformed = complete[column].to_vec();
    malformed[TRACE_ROWS - 1] = 1;
    let mut changed = complete;
    changed[column] = &malformed;
    assert!(matches!(
        DeepTraceCoefficients::from_columns(&changed, limits()),
        Err(Error::InvalidTraceShape { .. })
    ));
}
