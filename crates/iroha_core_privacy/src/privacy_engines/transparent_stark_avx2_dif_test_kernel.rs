//! Independent modular-oracle controls for the production Goldilocks DIF window.

use super::super::{dif_test_counts::Counts, goldilocks_avx2::Native};
use super::*;

#[test]
#[ignore = "mandatory production AVX2 DIF edge/recurrence/tail control"]
fn required_avx2_dif_windows_match_independent_modular_arithmetic() {
    let native = Native::admit().expect("simd and actual AVX2 are mandatory");
    let p = u128::from(GOLDILOCKS_MODULUS_V1);
    let edges = [
        0,
        1,
        (1 << 32) - 1,
        1 << 32,
        (1 << 63) - 1,
        1 << 63,
        (1 << 63) + 1,
        GOLDILOCKS_MODULUS_V1 - 2,
        GOLDILOCKS_MODULUS_V1 - 1,
    ];
    let roots = [
        F::ZERO,
        F::ONE,
        F(7),
        F(GOLDILOCKS_MODULUS_V1 - 1),
        goldilocks_primitive_root_v1(22).unwrap(),
    ];
    let mut total = Counts::default();
    for length in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 17, 1023, 1024, 1025, 8191] {
        for first in [0, 1, 3, 1024, 3072] {
            for root in roots {
                for alignment in 0..4 {
                    let mut even = zeroize::Zeroizing::new(vec![F::ZERO; alignment + length + 1]);
                    let mut odd = zeroize::Zeroizing::new(vec![F::ZERO; alignment + length + 1]);
                    for i in 0..length {
                        even[alignment + i] = F(edges[(i + first) % edges.len()]);
                        odd[alignment + i] = F(edges[(i * 3 + first + 1) % edges.len()]);
                    }
                    let counts = native.apply_counted(
                        &mut even[alignment..alignment + length],
                        &mut odd[alignment..alignment + length],
                        root,
                        first,
                    );
                    assert_eq!(counts.vector_blocks, length / 4);
                    assert_eq!(counts.scalar_pairs, length % 4);
                    total = total.plus(counts);
                    let mut power = 1_u128;
                    let mut base = u128::from(root.0);
                    let mut exponent = first;
                    while exponent != 0 {
                        if exponent & 1 != 0 {
                            power = power * base % p;
                        }
                        base = base * base % p;
                        exponent >>= 1;
                    }
                    for i in 0..length {
                        let left = u128::from(edges[(i + first) % edges.len()]);
                        let right = u128::from(edges[(i * 3 + first + 1) % edges.len()]);
                        assert_eq!(u128::from(even[alignment + i].0), (left + right) % p);
                        assert_eq!(
                            u128::from(odd[alignment + i].0),
                            ((left + p - right) % p) * power % p
                        );
                        power = power * u128::from(root.0) % p;
                    }
                    assert!(
                        even[..alignment]
                            .iter()
                            .chain(&even[alignment + length..])
                            .all(|v| *v == F::ZERO)
                    );
                    assert!(
                        odd[..alignment]
                            .iter()
                            .chain(&odd[alignment + length..])
                            .all(|v| *v == F::ZERO)
                    );
                }
            }
        }
    }
    assert!(total.vector_blocks > 0 && total.scalar_pairs > 0);
    println!(
        "dif_window_control vector_blocks={} scalar_pairs={} parity=exact",
        total.vector_blocks, total.scalar_pairs
    );
}
