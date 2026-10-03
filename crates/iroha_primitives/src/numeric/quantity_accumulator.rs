//! Allocation-free canonical quantity sums and one exactly admitted final magnitude.

use super::{
    BigInt, BigIntAdmissionCloneError, Numeric, NumericOperationError, QUANTITY_SUM_RELATION_LIMBS,
    Quantity, aligned_quantity_sum_limbs,
};
use std::alloc::Layout;

/// Bounded, allocation-free aggregate of canonical nonnegative quantities.
///
/// Every addition has the same result-domain checks as [`Quantity::checked_add`].
/// Arithmetic scratch stays on the stack. A caller can inspect the exact final
/// heap layout, reserve it from its original pool, and materialize once while
/// retaining that charge until the returned quantity drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityAccumulator {
    limbs: [u64; QUANTITY_SUM_RELATION_LIMBS],
    scale: u32,
}

impl Default for QuantityAccumulator {
    fn default() -> Self {
        Self::zero()
    }
}

impl QuantityAccumulator {
    /// Start with the canonical zero without allocating.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            limbs: [0; QUANTITY_SUM_RELATION_LIMBS],
            scale: 0,
        }
    }

    /// Add a borrowed quantity without cloning it or allocating scratch.
    ///
    /// # Errors
    /// Refuses a result outside the signed 512-bit canonical mantissa domain.
    /// On refusal the previous aggregate remains intact.
    pub fn try_add(&mut self, quantity: &Quantity) -> Result<(), NumericOperationError> {
        let scale = self.scale.max(quantity.scale());
        let rhs = aligned_quantity_sum_limbs(quantity, scale)
            .ok_or(NumericOperationError::MantissaOverflow)?;
        self.add_aligned(rhs, scale)
    }

    /// Add a second complete canonical aggregate without materializing either.
    ///
    /// # Errors
    /// Refuses an unrepresentable canonical sum and preserves both operands.
    pub fn try_add_accumulator(&mut self, other: &Self) -> Result<(), NumericOperationError> {
        let scale = self.scale.max(other.scale);
        let rhs = Self::align_limbs(other.limbs, other.scale, scale)?;
        self.add_aligned(rhs, scale)
    }

    fn align_limbs(
        mut limbs: [u64; QUANTITY_SUM_RELATION_LIMBS],
        from: u32,
        to: u32,
    ) -> Result<[u64; QUANTITY_SUM_RELATION_LIMBS], NumericOperationError> {
        for _ in from..to {
            let mut carry = 0_u128;
            for limb in &mut limbs {
                let product = u128::from(*limb) * 10 + carry;
                *limb = u64::try_from(product & u128::from(u64::MAX)).expect("masked decimal limb");
                carry = product >> 64;
            }
            if carry != 0 {
                return Err(NumericOperationError::MantissaOverflow);
            }
        }
        Ok(limbs)
    }

    fn add_aligned(
        &mut self,
        rhs: [u64; QUANTITY_SUM_RELATION_LIMBS],
        mut scale: u32,
    ) -> Result<(), NumericOperationError> {
        let mut sum = Self::align_limbs(self.limbs, self.scale, scale)?;
        let mut carry = 0_u128;
        for (limb, rhs) in sum.iter_mut().zip(rhs) {
            let value = u128::from(*limb) + u128::from(rhs) + carry;
            *limb = u64::try_from(value & u128::from(u64::MAX)).expect("masked addition limb");
            carry = value >> 64;
        }
        if carry != 0 {
            return Err(NumericOperationError::MantissaOverflow);
        }
        if sum.iter().all(|limb| *limb == 0) {
            scale = 0;
        }
        while scale != 0 {
            let mut quotient = [0_u64; QUANTITY_SUM_RELATION_LIMBS];
            let mut remainder = 0_u128;
            for index in (0..sum.len()).rev() {
                let value = (remainder << 64) | u128::from(sum[index]);
                quotient[index] = u64::try_from(value / 10).expect("division quotient limb");
                remainder = value % 10;
            }
            if remainder != 0 {
                break;
            }
            sum = quotient;
            scale -= 1;
        }
        if sum[8..].iter().any(|limb| *limb != 0) || sum[7] >> 63 != 0 {
            return Err(NumericOperationError::MantissaOverflow);
        }
        self.limbs = sum;
        self.scale = scale;
        Ok(())
    }

    /// Exact native-digit backing needed by [`Self::try_into_quantity`].
    ///
    /// # Errors
    /// Refuses an unrepresentable physical allocation layout.
    pub fn admission_layout(&self) -> Result<Layout, BigIntAdmissionCloneError> {
        BigInt::unsigned_words_admission_layout(&self.limbs)
    }

    /// Materialize through one exact fallible allocation, with no heap scratch.
    ///
    /// The caller must reserve and retain the matching original-pool charge;
    /// this method neither acquires a pool charge nor substitutes another pool.
    /// Zero needs no heap allocation.
    ///
    /// # Errors
    /// Refuses an unrepresentable layout or a physical allocator refusal.
    pub fn try_into_quantity(self) -> Result<Quantity, BigIntAdmissionCloneError> {
        Ok(Quantity(Numeric {
            mantissa: BigInt::try_from_unsigned_words_for_admission(&self.limbs)?,
            scale: self.scale,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    #[test]
    fn bounded_sum_matches_canonical_arithmetic_at_every_scale_and_carry() {
        let mut state = 0x23ac_514a_9853_0041_u64;
        for scale in 0..=28 {
            let mut sum = QuantityAccumulator::zero();
            let mut expected = Quantity::zero();
            for _ in 0..64 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let amount = Quantity::try_from_numeric(Numeric::new(
                    BigInt::from_i128(i128::from(state)),
                    scale,
                ))
                .unwrap();
                expected = expected.checked_add(&amount).unwrap();
                sum.try_add(&amount).unwrap();
                let actual = sum.try_into_quantity().unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    actual.admission_clone_layout().unwrap(),
                    sum.admission_layout().unwrap()
                );
                let wire = norito::to_bytes(&actual).unwrap();
                assert_eq!(
                    norito::decode_from_bytes::<Quantity>(&wire).unwrap(),
                    actual
                );
            }
        }
    }

    #[test]
    fn mixed_scale_normalization_zero_and_large_operands_match() {
        let mut sum = QuantityAccumulator::default();
        assert_eq!(sum.admission_layout().unwrap().size(), 0);
        assert_eq!(sum.try_into_quantity().unwrap(), Quantity::zero());
        let mut expected = Quantity::zero();
        for text in [
            "0",
            "0.0000000000000000000000000001",
            "999999999999999999999999999999999",
            "0.9999999999999999999999999999",
            "0",
            "1",
        ] {
            let amount = Quantity::from_str(text).unwrap();
            expected = expected.checked_add(&amount).unwrap();
            sum.try_add(&amount).unwrap();
            assert_eq!(sum.try_into_quantity().unwrap(), expected);
        }
    }

    #[test]
    fn separately_accumulated_exposure_preserves_original_grouping_and_refusal() {
        let max = Quantity::try_from_numeric(Numeric::new(
            BigInt::from_twos_bytes(&[vec![0xff; 63], vec![0x7f]].concat()).unwrap(),
            0,
        ))
        .unwrap();
        let bonded = max.checked_sub(&Quantity::one()).unwrap();
        let half = Quantity::from_str("0.5").unwrap();
        let mut left = QuantityAccumulator::zero();
        left.try_add(&bonded).unwrap();
        let mut pending = QuantityAccumulator::zero();
        pending.try_add(&half).unwrap();
        let unchanged = left;
        assert!(left.try_add_accumulator(&pending).is_err());
        assert_eq!(left, unchanged);
        pending.try_add(&half).unwrap();
        let original_pending = pending;
        left.try_add_accumulator(&pending).unwrap();
        assert_eq!(pending, original_pending);
        assert_eq!(left.try_into_quantity().unwrap(), max);
        assert_eq!(
            left.try_add_accumulator(&pending),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(left.try_into_quantity().unwrap(), max);
    }
    #[test]
    fn overflow_retains_original_sum() {
        let max = Quantity::try_from_numeric(Numeric::new(
            BigInt::from_twos_bytes(&[vec![0xff; 63], vec![0x7f]].concat()).unwrap(),
            0,
        ))
        .unwrap();
        let mut sum = QuantityAccumulator::zero();
        sum.try_add(&max).unwrap();
        let before = sum;
        assert_eq!(
            sum.try_add(&Quantity::one()),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(sum, before);
        assert_eq!(sum.try_into_quantity().unwrap(), max);
    }
}
