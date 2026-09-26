//! Small checked-domain parity for the producer's exact base twist/FFT kernel.

use super::*;
use crate::backend::GOLDILOCKS_MODULUS;

fn domain(rows: usize, offset: F) -> PolynomialDomain {
    PolynomialDomain::new(rows, offset, rows, 2 * rows * F::BYTES).unwrap()
}

fn coefficients(length: usize) -> Vec<u64> {
    let mut state = 0x71cd_53e2_98af_602b_u64;
    let mut result = (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % GOLDILOCKS_MODULUS
        })
        .collect::<Vec<_>>();
    if let Some(last) = result.last_mut() {
        *last = GOLDILOCKS_MODULUS - 1;
    }
    result
}

#[test]
fn base_fft_matches_four_lane_evaluation_and_horner_on_small_checked_cosets() {
    for rows in [16, 64, 4096] {
        for offset in [
            F::ONE,
            F::embed_base(super::super::deep_geometry::COSET_OFFSET),
        ] {
            let domain = domain(rows, offset);
            for length in [0, 1, 2, rows - 1, rows] {
                let source = coefficients(length);
                let actual = base_lde_on_domain(domain, &source).unwrap();
                let mut lifted = source
                    .iter()
                    .map(|&value| F::embed_base(value))
                    .collect::<Vec<_>>();
                // The shared Fp4 owner uses one zero padding coefficient for the
                // empty polynomial, while the base helper accepts an empty slice.
                if lifted.is_empty() {
                    lifted.push(F::ZERO);
                }
                let expected = domain.evaluate(&lifted, length).unwrap();
                assert_eq!(actual.len(), rows);
                for (index, &value) in actual.iter().enumerate() {
                    assert!(value < GOLDILOCKS_MODULUS);
                    assert_eq!(expected.value(index).unwrap(), F::embed_base(value));
                    // Exhaust all small domains and sample natural-order and
                    // butterfly boundaries at the 4096-point parallel threshold.
                    if rows <= 64
                        || [0, 1, 2, rows / 3, rows / 2 - 1, rows / 2, rows - 1].contains(&index)
                    {
                        let point = domain.point(index).unwrap();
                        let horner = source.iter().rev().fold(F::ZERO, |sum, &coefficient| {
                            sum.mul(point).add(F::embed_base(coefficient))
                        });
                        assert_eq!(horner, F::embed_base(value));
                    }
                }
            }
        }
    }
}

#[test]
fn base_fft_rejects_nonbase_offsets_and_oversized_small_extents() {
    let extension = F::new([3, 1, 2, 4]).unwrap();
    assert!(matches!(
        base_lde_on_domain(domain(16, extension), &[1, 2, 3]),
        Err(Error::InvalidTraceShape { .. })
    ));
    assert!(matches!(
        base_lde_on_domain(domain(16, F::ONE), &[0; 17]),
        Err(Error::InvalidTraceShape { .. })
    ));
}

#[test]
fn fixed_deep_wrapper_rejects_malformed_sources_before_large_allocation() {
    let geometry = DeepGeometry::new().unwrap();
    assert!(matches!(
        base_lde(&geometry, &vec![0; TRACE_ROWS + 1]),
        Err(Error::InvalidTraceShape { .. })
    ));
    for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
        for index in [0, 7, 15] {
            let mut values = [0; 16];
            values[index] = invalid;
            assert!(matches!(
                base_lde(&geometry, &values),
                Err(Error::NonCanonicalGoldilocksElement { context, indices })
                    if context == "deep_prover_base_lde" && indices == [index]
            ));
        }
    }
}
