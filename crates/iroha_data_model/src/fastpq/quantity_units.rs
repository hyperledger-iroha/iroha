//! Bounded integer units covering the complete ledger quantity domain.
//!
//! These are local arithmetic values, with no Norito codec or proof-profile identity.
//! TODO: migrate transfer SMT values, transitions and succinct arithmetic to the
//! complete domain before treating these units as a production proof capability.

use core::cmp::Ordering;

use iroha_primitives::{
    bigint::BigInt,
    numeric::{MAX_DECIMAL_SCALE, MAX_MANTISSA_BITS, Numeric, Quantity},
};

/// Number of little-endian 32-bit limbs needed for any ledger quantity at scale 28.
/// The largest possible unit count is `(2^511 - 1) * 10^28 < 2^605`.
pub const FASTPQ_QUANTITY_UNIT_LIMBS: usize = 19;
/// Fixed byte width of a normalized quantity's integer units, excluding its scale.
pub const FASTPQ_QUANTITY_UNIT_BYTES: usize = FASTPQ_QUANTITY_UNIT_LIMBS * 4;

// A ledger-domain change requires reviewing the unit bound and eventual proof geometry.
const _: () = assert!(MAX_MANTISSA_BITS == 512 && MAX_DECIMAL_SCALE == 28);

/// Exact, non-negative integer units at one explicitly bound decimal scale.
///
/// Construction covers every ledger `Quantity` at any common scale through 28,
/// including intermediate unit counts wider than the canonical mantissa. Private
/// fields and checked limb construction ensure conversion back to the ledger
/// domain succeeds. Arithmetic rejects scale mismatches, negative differences
/// and results outside that domain. Equal quantities at different witness scales
/// remain distinct values; selecting and authenticating the asset scale is the
/// caller's responsibility.
///
/// This type does not select a wire layout, qualify a proof profile, or extend
/// the existing `u64` transfer gadget by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastpqQuantityUnits {
    limbs: [u32; FASTPQ_QUANTITY_UNIT_LIMBS],
    scale: u32,
}

impl FastpqQuantityUnits {
    /// Normalize a ledger quantity into integer units at the requested scale.
    /// Returns `None` for a scale above 28 or below the quantity's exact scale.
    /// Work is bounded by 19 limbs and at most 28 multiplications by ten.
    #[must_use]
    pub fn from_quantity(quantity: &Quantity, scale: u32) -> Option<Self> {
        if scale > MAX_DECIMAL_SCALE || scale < quantity.scale() {
            return None;
        }
        let mut limbs = [0_u32; FASTPQ_QUANTITY_UNIT_LIMBS];
        for (index, byte) in quantity.mantissa().to_twos_bytes().into_iter().enumerate() {
            *limbs.get_mut(index / 4)? |= u32::from(byte) << ((index % 4) * 8);
        }
        for _ in quantity.scale()..scale {
            let mut carry = 0_u64;
            for limb in &mut limbs {
                let product = u64::from(*limb) * 10 + carry;
                (*limb, carry) = split_wide_limb(product);
            }
            if carry != 0 {
                return None;
            }
        }
        Some(Self { limbs, scale })
    }

    /// Validate a fixed limb array at an explicit decimal scale.
    /// Returns `None` if the scale or exact decimal lies outside the ledger domain.
    /// Canonical trailing-zero removal happens only when interpreting the decimal;
    /// the selected witness scale and integer limbs are retained exactly.
    #[must_use]
    pub fn from_limbs(limbs: [u32; FASTPQ_QUANTITY_UNIT_LIMBS], scale: u32) -> Option<Self> {
        if scale > MAX_DECIMAL_SCALE {
            return None;
        }
        let value = Self { limbs, scale };
        value.to_quantity()?;
        Some(value)
    }

    /// Exact common decimal scale used by these integer units.
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Fixed little-endian limbs; each limb is an unsigned 32-bit integer.
    #[must_use]
    pub const fn limbs(&self) -> &[u32; FASTPQ_QUANTITY_UNIT_LIMBS] {
        &self.limbs
    }

    /// Fixed little-endian integer bytes, independent of host word size or endian.
    /// These bytes exclude the scale and are not a standalone wire encoding.
    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; FASTPQ_QUANTITY_UNIT_BYTES] {
        let mut bytes = [0_u8; FASTPQ_QUANTITY_UNIT_BYTES];
        for (chunk, limb) in bytes.chunks_exact_mut(4).zip(self.limbs) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }
        bytes
    }

    /// Reconstruct the canonical ledger quantity represented by these exact units.
    /// Returns `None` if a private/internal value violates the construction invariant.
    #[must_use]
    pub fn to_quantity(&self) -> Option<Quantity> {
        if self.scale > MAX_DECIMAL_SCALE {
            return None;
        }
        // The appended sign byte makes every possible fixed limb array positive.
        // The bounded BigInt domain comfortably contains all 608 unit bits.
        let mut bytes = [0_u8; FASTPQ_QUANTITY_UNIT_BYTES + 1];
        bytes[..FASTPQ_QUANTITY_UNIT_BYTES].copy_from_slice(&self.to_le_bytes());
        let mantissa = BigInt::from_twos_bytes(&bytes).ok()?;
        let numeric = Numeric::try_new(mantissa, self.scale).ok()?;
        Quantity::from_canonical_numeric(numeric).ok()
    }

    /// Narrow only when the exact integer unit count fits the existing `u64` gadget.
    #[must_use]
    pub fn try_to_u64(&self) -> Option<u64> {
        self.limbs[2..]
            .iter()
            .all(|limb| *limb == 0)
            .then(|| u64::from(self.limbs[0]) | (u64::from(self.limbs[1]) << 32))
    }

    /// Compare integer units only when both operands use the same scale.
    #[must_use]
    pub fn checked_cmp(&self, rhs: &Self) -> Option<Ordering> {
        (self.scale == rhs.scale).then(|| self.limbs.iter().rev().cmp(rhs.limbs.iter().rev()))
    }

    /// Add at one common scale and require the exact result to remain a ledger quantity.
    #[must_use]
    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        if self.scale != rhs.scale {
            return None;
        }
        let mut limbs = [0_u32; FASTPQ_QUANTITY_UNIT_LIMBS];
        let mut carry = false;
        for ((out, left), right) in limbs.iter_mut().zip(self.limbs).zip(rhs.limbs) {
            let (sum, limb_overflow) = left.overflowing_add(right);
            let (sum, carry_overflow) = sum.overflowing_add(u32::from(carry));
            *out = sum;
            carry = limb_overflow || carry_overflow;
        }
        if carry {
            return None;
        }
        Self::from_limbs(limbs, self.scale)
    }

    /// Subtract at one common scale and reject negative or nonrepresentable results.
    #[must_use]
    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        if self.scale != rhs.scale {
            return None;
        }
        let mut limbs = [0_u32; FASTPQ_QUANTITY_UNIT_LIMBS];
        let mut borrow = false;
        for ((out, left), right) in limbs.iter_mut().zip(self.limbs).zip(rhs.limbs) {
            let (difference, limb_underflow) = left.overflowing_sub(right);
            let (difference, borrow_underflow) = difference.overflowing_sub(u32::from(borrow));
            *out = difference;
            borrow = limb_underflow || borrow_underflow;
        }
        if borrow {
            return None;
        }
        Self::from_limbs(limbs, self.scale)
    }
}

// A wide multiplication result contributes its low word to this limb and
// carries its high word into the next limb. Explicit bytes retain both parts
// without a lossy integer conversion or a host-endian dependency.
fn split_wide_limb(value: u64) -> (u32, u64) {
    let bytes = value.to_le_bytes();
    (
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        value >> 32,
    )
}

#[cfg(test)]
mod tests;
