//! Exact bounded Goldilocks product differences using the existing field reducer.

use super::GoldilocksFieldV1 as F;
use zeroize::Zeroize as _;

const TERMS_V1: usize = 16;

#[derive(Default)]
struct ScratchV1 {
    sums: [u128; 2],
    carries: [u64; 2],
}

/// Exact private accumulator storage held by one active ordered product sum.
pub(crate) const GOLDILOCKS_PRODUCT_DIFFERENCE_SCRATCH_BYTES_V1: usize =
    core::mem::size_of::<ScratchV1>();

impl ScratchV1 {
    #[inline]
    fn add_v1(&mut self, a: F, b: F, c: F, d: F) {
        let (sum, carry) = self.sums[0].overflowing_add(u128::from(a.0) * u128::from(b.0));
        self.sums[0] = sum;
        self.carries[0] += u64::from(carry);
        let (sum, carry) = self.sums[1].overflowing_add(u128::from(c.0) * u128::from(d.0));
        self.sums[1] = sum;
        self.carries[1] += u64::from(carry);
    }
    #[inline]
    fn reduced_v1(&self) -> F {
        // Exactly the audited SHA/DEEP identity: 2^128 = -2^32 mod p.
        // A public block has at most 16 terms, hence at most 15 carries; the
        // correction is canonical and uses the existing complete u128 reducer.
        let positive = F::reduce(self.sums[0]).sub(F(self.carries[0] << 32));
        let negative = F::reduce(self.sums[1]).sub(F(self.carries[1] << 32));
        positive.sub(negative)
    }
}
impl Drop for ScratchV1 {
    fn drop(&mut self) {
        self.sums.zeroize();
        self.carries.zeroize();
        #[cfg(test)]
        tests::observe_clear_v1(&self.sums, &self.carries);
    }
}

#[inline]
pub(super) fn sum_v1(length: usize, mut term: impl FnMut(usize) -> (F, F, F, F)) -> F {
    if length == 0 {
        return F::ZERO;
    }
    if length == 1 {
        let (a, b, c, d) = term(0);
        return a.mul(b).sub(c.mul(d));
    }
    let mut result = F::ZERO;
    for first in (0..length).step_by(TERMS_V1) {
        let mut scratch = ScratchV1::default();
        for index in first..first + (length - first).min(TERMS_V1) {
            let (a, b, c, d) = term(index);
            scratch.add_v1(a, b, c, d);
        }
        let value = scratch.reduced_v1();
        result = if first == 0 { value } else { result.add(value) };
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    thread_local! {
        static CLEARS: RefCell<Option<Vec<([u128; 2], [u64; 2])>>> = const { RefCell::new(None) };
    }
    pub(super) fn observe_clear_v1(sums: &[u128; 2], carries: &[u64; 2]) {
        CLEARS.with_borrow_mut(|slot| {
            if let Some(clears) = slot {
                clears.push((*sums, *carries));
            }
        });
    }
    #[test]
    fn arithmetic_product_sum_all_lengths_boundaries_and_order_match_original() {
        let p = super::super::GOLDILOCKS_MODULUS_V1;
        let values = [0, 1, 2, (1 << 32) - 1, 1 << 32, p / 2, p - 2, p - 1];
        for length in [0, 1, 2, 3, 7, 8, 15, 16, 17, 31, 32, 65] {
            for seed in 0..64 {
                let term = |index: usize| {
                    (
                        F(values[(index + seed) % 8]),
                        F(values[(index * 3 + seed / 2) % 8]),
                        F(values[(index * 5 + seed / 3) % 8]),
                        F(values[(index * 7 + seed / 5) % 8]),
                    )
                };
                let expected = (0..length).fold(F::ZERO, |sum, index| {
                    let (a, b, c, d) = term(index);
                    sum.add(a.mul(b)).sub(c.mul(d))
                });
                let mut seen = Vec::new();
                let actual = sum_v1(length, |index| {
                    seen.push(index);
                    term(index)
                });
                assert_eq!(actual, expected);
                assert_eq!(seen, (0..length).collect::<Vec<_>>());
            }
            for negative in [false, true] {
                let term = |_| {
                    if negative {
                        (F::ZERO, F::ZERO, F(p - 1), F(p - 1))
                    } else {
                        (F(p - 1), F(p - 1), F::ZERO, F::ZERO)
                    }
                };
                let expected = if negative {
                    F::ZERO.sub(F(length as u64))
                } else {
                    F(length as u64)
                };
                assert_eq!(sum_v1(length, term), expected);
            }
        }
        assert_eq!(GOLDILOCKS_PRODUCT_DIFFERENCE_SCRATCH_BYTES_V1, 48);
    }
    #[test]
    fn arithmetic_product_sum_full_scratch_clears_on_success_and_term_unwind() {
        for unwind in [false, true] {
            CLEARS.with_borrow_mut(|slot| *slot = Some(Vec::new()));
            let result = std::panic::catch_unwind(|| {
                sum_v1(16, |index| {
                    assert!(!(unwind && index == 9), "injected product-sum term unwind");
                    (
                        F(u64::MAX - (1 << 32)),
                        F(u64::MAX - (1 << 32)),
                        F(17),
                        F(23),
                    )
                })
            });
            assert_eq!(result.is_err(), unwind);
            let clears = CLEARS.take().unwrap();
            assert_eq!(clears, vec![([0; 2], [0; 2])]);
        }
    }
}
