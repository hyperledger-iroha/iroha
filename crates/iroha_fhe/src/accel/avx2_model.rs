//! Portable lane model of the AVX2 intrinsics that `avx2.rs` uses. Test builds only.
//!
//! Each function reproduces the documented behaviour of the Intel intrinsic of
//! the same name on four 64-bit lanes, lane zero being the lowest. Hosts
//! without AVX2 compile the AVX2 kernel against this module, so its schedule
//! runs and is compared with the scalar reference on every host.
//!
//! The model is itself checked: the known-answer test below pins the lane
//! order, the signed comparison and the 32-bit multiply on every host, and on
//! x86-64 `lane_model_agrees_with_the_cpu` compares every modelled intrinsic
//! with the CPU's on boundary and random lanes. A model result is evidence
//! about the kernel's schedule, not a substitute for running the instructions.
// Every function keeps the name of the intrinsic it models, leading underscore included.
#![allow(clippy::used_underscore_items)]

/// Four 64-bit lanes; index zero is the lowest lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Vector(pub(super) [u64; 4]);

const ALL_ONES: u64 = u64::MAX;

fn zip_lanes(lhs: Vector, rhs: Vector, lane: impl Fn(u64, u64) -> u64) -> Vector {
    Vector(core::array::from_fn(|index| {
        lane(lhs.0[index], rhs.0[index])
    }))
}

/// Wrapping 64-bit lane sum.
pub(super) fn _mm256_add_epi64(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, u64::wrapping_add)
}

/// Wrapping 64-bit lane difference.
pub(super) fn _mm256_sub_epi64(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, u64::wrapping_sub)
}

/// Bitwise AND.
pub(super) fn _mm256_and_si256(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, |left, right| left & right)
}

/// Bitwise AND of the complement of the first operand with the second.
pub(super) fn _mm256_andnot_si256(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, |left, right| !left & right)
}

/// All ones in each lane whose operands are equal, zero otherwise.
pub(super) fn _mm256_cmpeq_epi64(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(
        lhs,
        rhs,
        |left, right| if left == right { ALL_ONES } else { 0 },
    )
}

/// All ones in each lane where the first operand is greater as a signed 64-bit integer.
pub(super) fn _mm256_cmpgt_epi64(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, |left, right| {
        if left.cast_signed() > right.cast_signed() {
            ALL_ONES
        } else {
            0
        }
    })
}

/// Lane `INDEX` as a signed integer.
pub(super) fn _mm256_extract_epi64<const INDEX: i32>(value: Vector) -> i64 {
    value.0[usize::try_from(INDEX).expect("lane index is 0..=3")].cast_signed()
}

/// Unsigned product of the low 32 bits of each 64-bit lane.
pub(super) fn _mm256_mul_epu32(lhs: Vector, rhs: Vector) -> Vector {
    zip_lanes(lhs, rhs, |left, right| {
        (left & 0xFFFF_FFFF) * (right & 0xFFFF_FFFF)
    })
}

/// Lanes from the highest (`lane3`) to the lowest (`lane0`), the argument order of the intrinsic.
pub(super) fn _mm256_set_epi64x(lane3: i64, lane2: i64, lane1: i64, lane0: i64) -> Vector {
    Vector([
        lane0.cast_unsigned(),
        lane1.cast_unsigned(),
        lane2.cast_unsigned(),
        lane3.cast_unsigned(),
    ])
}

/// The same value in every lane.
pub(super) fn _mm256_set1_epi64x(value: i64) -> Vector {
    Vector([value.cast_unsigned(); 4])
}

/// All lanes zero.
pub(super) fn _mm256_setzero_si256() -> Vector {
    Vector([0; 4])
}

/// Logical right shift of each lane by `IMM8` bits; a count above 63 clears the lane.
pub(super) fn _mm256_srli_epi64<const IMM8: i32>(value: Vector) -> Vector {
    let count = u32::try_from(IMM8).expect("shift count is non-negative");
    Vector(
        value
            .0
            .map(|lane| lane.checked_shr(count).unwrap_or_default()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIGH_BIT: u64 = 1 << 63;

    #[test]
    fn modelled_intrinsics_have_the_documented_lane_semantics() {
        // Argument order of `set` is highest lane first; `extract` indexes from the lowest.
        let ordered = _mm256_set_epi64x(3, 2, 1, 0);
        assert_eq!(ordered, Vector([0, 1, 2, 3]));
        assert_eq!(_mm256_extract_epi64::<0>(ordered), 0);
        assert_eq!(_mm256_extract_epi64::<1>(ordered), 1);
        assert_eq!(_mm256_extract_epi64::<2>(ordered), 2);
        assert_eq!(_mm256_extract_epi64::<3>(ordered), 3);
        assert_eq!(_mm256_set1_epi64x(-1), Vector([ALL_ONES; 4]));
        assert_eq!(_mm256_setzero_si256(), Vector([0; 4]));
        let lhs = Vector([ALL_ONES, HIGH_BIT, 0x1_0000_0003, 7]);
        let rhs = Vector([1, HIGH_BIT, 0x2_0000_0005, 9]);
        assert_eq!(
            _mm256_add_epi64(lhs, rhs),
            Vector([0, 0, 0x3_0000_0008, 16])
        );
        assert_eq!(
            _mm256_sub_epi64(lhs, rhs),
            Vector([ALL_ONES - 1, 0, 0xFFFF_FFFE_FFFF_FFFE, ALL_ONES - 1])
        );
        assert_eq!(
            _mm256_and_si256(lhs, rhs),
            Vector([1, HIGH_BIT, 0x0_0000_0001, 1])
        );
        assert_eq!(
            _mm256_andnot_si256(lhs, rhs),
            Vector([0, 0, 0x2_0000_0004, 8])
        );
        assert_eq!(_mm256_cmpeq_epi64(lhs, rhs), Vector([0, ALL_ONES, 0, 0]));
        // Signed order: all ones is -1 and the high bit alone is the most negative value.
        assert_eq!(
            _mm256_cmpgt_epi64(
                Vector([1, 0, HIGH_BIT, HIGH_BIT - 1]),
                Vector([ALL_ONES, 0, 0, HIGH_BIT])
            ),
            Vector([ALL_ONES, 0, 0, ALL_ONES])
        );
        // Only the low words multiply: 3 * 5, and (2^32 - 1)^2 without the high words.
        assert_eq!(
            _mm256_mul_epu32(
                Vector([0x1_0000_0003, ALL_ONES, 0, 1 << 32]),
                Vector([0x2_0000_0005, ALL_ONES, 9, 9])
            ),
            Vector([15, 0xFFFF_FFFE_0000_0001, 0, 0])
        );
        assert_eq!(
            _mm256_srli_epi64::<32>(Vector([ALL_ONES, HIGH_BIT, 1 << 32, 1])),
            Vector([0xFFFF_FFFF, 1 << 31, 1, 0])
        );
        assert_eq!(
            _mm256_srli_epi64::<64>(Vector([ALL_ONES; 4])),
            Vector([0; 4])
        );
        assert_eq!(
            _mm256_srli_epi64::<0>(Vector([ALL_ONES; 4])),
            Vector([ALL_ONES; 4])
        );
    }

    #[cfg(target_arch = "x86_64")]
    mod cpu {
        use super::super::*;
        use core::arch::x86_64 as x86;

        fn splitmix64(state: &mut u64) -> u64 {
            *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut value = *state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            value ^ (value >> 31)
        }

        #[target_feature(enable = "avx2")]
        fn real(lanes: Vector) -> x86::__m256i {
            x86::_mm256_set_epi64x(
                lanes.0[3].cast_signed(),
                lanes.0[2].cast_signed(),
                lanes.0[1].cast_signed(),
                lanes.0[0].cast_signed(),
            )
        }

        #[target_feature(enable = "avx2")]
        fn lanes_of(value: x86::__m256i) -> Vector {
            Vector([
                x86::_mm256_extract_epi64::<0>(value).cast_unsigned(),
                x86::_mm256_extract_epi64::<1>(value).cast_unsigned(),
                x86::_mm256_extract_epi64::<2>(value).cast_unsigned(),
                x86::_mm256_extract_epi64::<3>(value).cast_unsigned(),
            ])
        }

        #[target_feature(enable = "avx2")]
        fn compare(lhs: Vector, rhs: Vector) {
            let (real_lhs, real_rhs) = (real(lhs), real(rhs));
            // `set` and `extract` round-trip in the modelled lane order.
            assert_eq!(lanes_of(real_lhs), lhs);
            assert_eq!(
                _mm256_set_epi64x(
                    x86::_mm256_extract_epi64::<3>(real_lhs),
                    x86::_mm256_extract_epi64::<2>(real_lhs),
                    x86::_mm256_extract_epi64::<1>(real_lhs),
                    x86::_mm256_extract_epi64::<0>(real_lhs),
                ),
                lhs
            );
            assert_eq!(
                lanes_of(x86::_mm256_add_epi64(real_lhs, real_rhs)),
                _mm256_add_epi64(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_sub_epi64(real_lhs, real_rhs)),
                _mm256_sub_epi64(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_and_si256(real_lhs, real_rhs)),
                _mm256_and_si256(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_andnot_si256(real_lhs, real_rhs)),
                _mm256_andnot_si256(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_cmpeq_epi64(real_lhs, real_rhs)),
                _mm256_cmpeq_epi64(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_cmpgt_epi64(real_lhs, real_rhs)),
                _mm256_cmpgt_epi64(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_mul_epu32(real_lhs, real_rhs)),
                _mm256_mul_epu32(lhs, rhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_srli_epi64::<32>(real_lhs)),
                _mm256_srli_epi64::<32>(lhs)
            );
            assert_eq!(
                lanes_of(x86::_mm256_set1_epi64x(lhs.0[0].cast_signed())),
                _mm256_set1_epi64x(lhs.0[0].cast_signed())
            );
            assert_eq!(
                lanes_of(x86::_mm256_setzero_si256()),
                _mm256_setzero_si256()
            );
        }

        #[target_feature(enable = "avx2")]
        fn compare_all() {
            const BOUNDARY: [u64; 10] = [
                0,
                1,
                0xFFFF_FFFF,
                1 << 32,
                (1 << 32) + 1,
                (1 << 62) - 1,
                (1 << 63) - 1,
                1 << 63,
                u64::MAX - 1,
                u64::MAX,
            ];
            for (lhs_index, &lhs) in BOUNDARY.iter().enumerate() {
                for (rhs_index, &rhs) in BOUNDARY.iter().enumerate() {
                    // Rotate the pair through the lanes so every lane sees every boundary pair.
                    let other_lhs = BOUNDARY[(lhs_index + rhs_index) % BOUNDARY.len()];
                    let other_rhs = BOUNDARY[(lhs_index + 2 * rhs_index + 1) % BOUNDARY.len()];
                    for rotation in 0..4 {
                        let mut left = [lhs, other_lhs, rhs, other_rhs];
                        let mut right = [rhs, other_rhs, lhs, other_lhs];
                        left.rotate_left(rotation);
                        right.rotate_left(rotation);
                        compare(Vector(left), Vector(right));
                    }
                }
            }
            let mut state = 0xA5A5_u64;
            for _ in 0..4_096 {
                let lhs = Vector(core::array::from_fn(|_| splitmix64(&mut state)));
                let rhs = Vector(core::array::from_fn(|_| splitmix64(&mut state)));
                compare(lhs, rhs);
            }
        }

        /// The model and the CPU agree on every intrinsic the kernel uses.
        #[test]
        #[allow(unsafe_code)]
        fn lane_model_agrees_with_the_cpu() {
            if !std::arch::is_x86_feature_detected!("avx2") {
                eprintln!("AVX2 is not available on this CPU; the comparison did not run");
                return;
            }
            // SAFETY: AVX2 support was detected on this CPU immediately above.
            unsafe { compare_all() };
        }
    }
}
