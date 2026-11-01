//! Four-field canonical Goldilocks arithmetic for admitted AVX2 DIF windows.
//!
//! This module owns the scalar-equivalent carry/borrow corrections. Its safe
//! token checks the existing `simd` feature and actual CPU support before any
//! target-feature call. Callers retain validation, scheduling and clearing
//! allocations; the window creates no heap or table. Register/spill erasure is
//! not claimed.

#![allow(unsafe_code)]

#[cfg(test)]
use super::dif_test_counts::Counts;
use super::{
    GOLDILOCKS_EPSILON_V1, GOLDILOCKS_MODULUS_V1, GoldilocksFieldV1 as F, dif_output::DifOutputV1,
};
use core::arch::x86_64::*;

/// Opaque proof that this process admitted the optional AVX2 implementation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Native(());

impl Native {
    /// Admit the existing feature and runtime CPU capability, with no override.
    pub(crate) fn admit() -> Option<Self> {
        (cfg!(feature = "simd") && std::is_x86_feature_detected!("avx2")).then_some(Self(()))
    }

    /// Apply DIF to disjoint equal halves containing canonical fields.
    ///
    /// The root and `first` describe public powers. The caller validates those
    /// fields before private writes; every operation preserves canonicality.
    #[cfg(feature = "simd")]
    pub(crate) fn apply(self, even: &mut [F], odd: &mut [F], root: F, first: usize) {
        assert_eq!(even.len(), odd.len());
        // SAFETY: Native can only be created by actual runtime admission. Each
        // load/store extent is checked; the mutable borrows cannot alias.
        unsafe {
            window::<()>(even, odd, root, first);
        }
    }

    /// Observe actual native/tail iterations for an explicitly requested control.
    #[cfg(test)]
    pub(crate) fn apply_counted(
        self,
        even: &mut [F],
        odd: &mut [F],
        root: F,
        first: usize,
    ) -> Counts {
        assert_eq!(even.len(), odd.len());
        // SAFETY: same admitted token and bounded disjoint slices as apply.
        unsafe { window::<Counts>(even, odd, root, first) }
    }
}

/// Compare unsigned lanes using the signed AVX2 comparison after flipping bit 63.
#[target_feature(enable = "avx2")]
#[inline]
fn less(left: __m256i, right: __m256i) -> __m256i {
    let sign = _mm256_set1_epi64x(i64::MIN);
    _mm256_cmpgt_epi64(_mm256_xor_si256(right, sign), _mm256_xor_si256(left, sign))
}

#[target_feature(enable = "avx2")]
#[inline]
fn canonical(value: __m256i) -> __m256i {
    let modulus = _mm256_set1_epi64x(GOLDILOCKS_MODULUS_V1 as i64);
    let maximum = _mm256_set1_epi64x((GOLDILOCKS_MODULUS_V1 - 1) as i64);
    _mm256_sub_epi64(value, _mm256_and_si256(less(maximum, value), modulus))
}

#[target_feature(enable = "avx2")]
#[inline]
pub(super) fn multiply(left: __m256i, right: __m256i) -> __m256i {
    let epsilon = _mm256_set1_epi64x(GOLDILOCKS_EPSILON_V1 as i64);
    let left_high = _mm256_srli_epi64::<32>(left);
    let right_high = _mm256_srli_epi64::<32>(right);
    // Four exact 32x32 products reconstruct each independent 128-bit product.
    let low_low = _mm256_mul_epu32(left, right);
    let low_high = _mm256_mul_epu32(left, right_high);
    let high_low = _mm256_mul_epu32(left_high, right);
    let high_high = _mm256_mul_epu32(left_high, right_high);
    let middle = _mm256_add_epi64(
        _mm256_srli_epi64::<32>(low_low),
        _mm256_add_epi64(
            _mm256_and_si256(low_high, epsilon),
            _mm256_and_si256(high_low, epsilon),
        ),
    );
    let low = _mm256_or_si256(
        _mm256_and_si256(low_low, epsilon),
        _mm256_slli_epi64::<32>(middle),
    );
    let high = _mm256_add_epi64(
        high_high,
        _mm256_add_epi64(
            _mm256_srli_epi64::<32>(middle),
            _mm256_add_epi64(
                _mm256_srli_epi64::<32>(low_high),
                _mm256_srli_epi64::<32>(high_low),
            ),
        ),
    );
    // Mirror F::reduce exactly, including both wrapping carry corrections.
    let high_high = _mm256_srli_epi64::<32>(high);
    let high_low = _mm256_and_si256(high, epsilon);
    let reduced_low = _mm256_sub_epi64(
        _mm256_sub_epi64(low, high_high),
        _mm256_and_si256(less(low, high_high), epsilon),
    );
    let folded_high = _mm256_sub_epi64(_mm256_slli_epi64::<32>(high_low), high_low);
    let reduced = _mm256_add_epi64(reduced_low, folded_high);
    canonical(_mm256_add_epi64(
        reduced,
        _mm256_and_si256(less(reduced, reduced_low), epsilon),
    ))
}

#[target_feature(enable = "avx2")]
#[inline]
pub(super) fn add(left: __m256i, right: __m256i) -> __m256i {
    let sum = _mm256_add_epi64(left, right);
    let epsilon = _mm256_set1_epi64x(GOLDILOCKS_EPSILON_V1 as i64);
    canonical(_mm256_add_epi64(
        sum,
        _mm256_and_si256(less(sum, left), epsilon),
    ))
}

#[target_feature(enable = "avx2")]
#[inline]
pub(super) fn subtract(left: __m256i, right: __m256i) -> __m256i {
    let epsilon = _mm256_set1_epi64x(GOLDILOCKS_EPSILON_V1 as i64);
    _mm256_sub_epi64(
        _mm256_sub_epi64(left, right),
        _mm256_and_si256(less(left, right), epsilon),
    )
}

#[target_feature(enable = "avx2")]
unsafe fn window<O: DifOutputV1>(even: &mut [F], odd: &mut [F], root: F, first: usize) -> O {
    let mut scalar_twiddle = root.pow(first as u128);
    let mut offset = 0;
    #[cfg(test)]
    let mut outcome = O::default();
    if even.len() >= 4 {
        // These lanes and step contain only verifier-public root powers.
        let t1 = scalar_twiddle.mul(root);
        let t2 = t1.mul(root);
        let t3 = t2.mul(root);
        let root2 = root.mul(root);
        let root4 = root2.mul(root2);
        let step = _mm256_set1_epi64x(root4.0 as i64);
        let mut twiddles = _mm256_set_epi64x(
            t3.0 as i64,
            t2.0 as i64,
            t1.0 as i64,
            scalar_twiddle.0 as i64,
        );
        while offset + 4 <= even.len() {
            // SAFETY: F is repr(transparent) u64; complete unaligned groups are
            // within both disjoint halves. Read both inputs before either write.
            unsafe {
                let left = _mm256_loadu_si256(even.as_ptr().add(offset).cast());
                let right = _mm256_loadu_si256(odd.as_ptr().add(offset).cast());
                _mm256_storeu_si256(even.as_mut_ptr().add(offset).cast(), add(left, right));
                _mm256_storeu_si256(
                    odd.as_mut_ptr().add(offset).cast(),
                    multiply(subtract(left, right), twiddles),
                );
            }
            offset += 4;
            #[cfg(test)]
            if O::OBSERVED {
                outcome.record_vector();
            }
            twiddles = multiply(twiddles, step);
        }
        // Lane zero now belongs to exactly first+offset, including a short tail.
        scalar_twiddle = F(_mm256_extract_epi64::<0>(twiddles) as u64);
    }
    for (even, odd) in even[offset..].iter_mut().zip(&mut odd[offset..]) {
        let left = *even;
        let right = *odd;
        *even = left.add(right);
        *odd = left.sub(right).mul(scalar_twiddle);
        scalar_twiddle = scalar_twiddle.mul(root);
        #[cfg(test)]
        if O::OBSERVED {
            outcome.record_scalar();
        }
    }
    #[cfg(test)]
    {
        outcome
    }
    #[cfg(not(test))]
    {
        O::default()
    }
}
