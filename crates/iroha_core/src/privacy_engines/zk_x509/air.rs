//! Constrained building blocks and gap inventory for the segmented zk-X509 AIR.
//!
//! This module contains algebraic constraints, not native re-checks disguised
//! as proofs.  Every row evaluator returns zero only when its polynomial
//! identities hold over Goldilocks.  The completed prover will place these
//! rows in masked committed segments and evaluate the same identities at
//! transcript-derived points.
//!
//! The full relation is deliberately not marked ready yet.  The exact
//! remaining segment gaps are represented by [`ZK_X509_AIR_GAPS_V1`] so a
//! release cannot accidentally equate these foundational gates with a
//! complete DER/SHA-256/P-256 execution proof.

use thiserror::Error;

use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;

/// Stable incomplete AIR component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZkX509AirGapV1 {
    /// Bind execution-order and address-sorted private byte accesses with a
    /// challenge-based permutation product.
    ByteMemoryPermutation,
    /// Constrain the complete closed DER grammar and RFC 5280 path state.
    DerAndPathStateMachine,
    /// Wire every scheduled SHA-256 bit gate to padded certificate, CRL,
    /// accumulator, transcript, and projection byte streams.
    Sha256ScheduleAndWiring,
    /// Constrain P-256 base/scalar arithmetic, inversion, group operations,
    /// strict DER `(r,s)`, and all certificate/CRL/wallet ECDSA equations.
    P256Ecdsa,
    /// Bind all segment boundary digests and public projections through the
    /// final composition segment.
    CrossSegmentComposition,
    /// Emit and verify the exact segmented Merkle/FRI proof wire with the
    /// finalized degree schedule and union-bound soundness analysis.
    SegmentedProofAndSoundness,
    /// Fix measured release rows, peak memory, and proving time.
    ReleaseResourceMeasurements,
}

/// Exhaustive code-level gap inventory.  Removing an entry requires its
/// constrained implementation, differential tests, and adversarial corpus.
pub(crate) const ZK_X509_AIR_GAPS_V1: [ZkX509AirGapV1; 7] = [
    ZkX509AirGapV1::ByteMemoryPermutation,
    ZkX509AirGapV1::DerAndPathStateMachine,
    ZkX509AirGapV1::Sha256ScheduleAndWiring,
    ZkX509AirGapV1::P256Ecdsa,
    ZkX509AirGapV1::CrossSegmentComposition,
    ZkX509AirGapV1::SegmentedProofAndSoundness,
    ZkX509AirGapV1::ReleaseResourceMeasurements,
];

/// Stable manifest encoding of every currently incomplete AIR component.
pub(crate) const ZK_X509_AIR_GAP_DESCRIPTOR_V1: &[u8] = b"byte-memory-permutation|der-and-rfc5280-path-state-machine|sha256-schedule-and-wiring|p256-ecdsa|cross-segment-composition|segmented-proof-and-soundness|release-resource-measurements";

/// Failure of an implemented zk-X509 AIR primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509AirErrorV1 {
    /// A purported bit is outside `{0,1}`.
    #[error("zk-X509 AIR boolean constraint failed")]
    NonBoolean,
    /// A bit decomposition does not reconstruct its packed value.
    #[error("zk-X509 AIR range decomposition constraint failed")]
    RangeDecomposition,
    /// A selected bitwise gate equation is unsatisfied.
    #[error("zk-X509 AIR bit-gate constraint failed")]
    BitGate,
    /// Gate selectors are not a one-hot fixed row.
    #[error("zk-X509 AIR gate selector constraint failed")]
    GateSelector,
}

/// One degree-two range-check row for a private byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ByteRangeAirRowV1 {
    /// Packed byte value.
    pub(crate) value: F,
    /// Little-endian bit decomposition.
    pub(crate) bits: [F; 8],
}

impl ByteRangeAirRowV1 {
    /// Construct the canonical witness row for one byte.
    pub(crate) fn from_byte(value: u8) -> Self {
        Self {
            value: F(u64::from(value)),
            bits: core::array::from_fn(|bit| F(u64::from((value >> bit) & 1))),
        }
    }

    /// Evaluate all local degree-two AIR identities.
    pub(crate) fn validate(self) -> Result<(), ZkX509AirErrorV1> {
        validate_bits_v1(&self.bits)?;
        let packed = self
            .bits
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (bit, value)| {
                sum.add(value.mul(F(1_u64 << bit)))
            });
        if packed != self.value {
            return Err(ZkX509AirErrorV1::RangeDecomposition);
        }
        Ok(())
    }
}

/// One degree-two range-check row for an address, clock, or SHA-256 word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct U32RangeAirRowV1 {
    /// Packed 32-bit value.
    pub(crate) value: F,
    /// Little-endian bit decomposition.
    pub(crate) bits: [F; 32],
}

impl U32RangeAirRowV1 {
    /// Construct the canonical witness row.
    pub(crate) fn from_u32(value: u32) -> Self {
        Self {
            value: F(u64::from(value)),
            bits: core::array::from_fn(|bit| F(u64::from((value >> bit) & 1))),
        }
    }

    /// Evaluate all local degree-two AIR identities.
    pub(crate) fn validate(self) -> Result<(), ZkX509AirErrorV1> {
        validate_bits_v1(&self.bits)?;
        let packed = self
            .bits
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (bit, value)| {
                sum.add(value.mul(F(1_u64 << bit)))
            });
        if packed != self.value {
            return Err(ZkX509AirErrorV1::RangeDecomposition);
        }
        Ok(())
    }
}

/// One fixed-selector Boolean gate row used by SHA-256 and P-256 bit logic.
///
/// Selectors are fixed preprocessing columns in the final segment.  Keeping
/// them in this row type makes differential tests evaluate the exact equations
/// that the verifier will mix into its composition polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BooleanGateAirRowV1 {
    /// Select `out = left AND right`.
    pub(crate) select_and: F,
    /// Select `out = left XOR right`.
    pub(crate) select_xor: F,
    /// Select `left + right + carry_in = out + 2*carry_out`.
    pub(crate) select_full_adder: F,
    /// First Boolean input.
    pub(crate) left: F,
    /// Second Boolean input.
    pub(crate) right: F,
    /// Full-adder carry input; zero on AND/XOR rows.
    pub(crate) carry_in: F,
    /// Boolean result.
    pub(crate) out: F,
    /// Full-adder carry output; zero on AND/XOR rows.
    pub(crate) carry_out: F,
}

impl BooleanGateAirRowV1 {
    /// Canonical AND witness row.
    pub(crate) fn and(left: bool, right: bool) -> Self {
        Self::new_selector([1, 0, 0], left, right, false, left & right, false)
    }

    /// Canonical XOR witness row.
    pub(crate) fn xor(left: bool, right: bool) -> Self {
        Self::new_selector([0, 1, 0], left, right, false, left ^ right, false)
    }

    /// Canonical one-bit full-adder witness row.
    pub(crate) fn full_adder(left: bool, right: bool, carry_in: bool) -> Self {
        let sum = u8::from(left) + u8::from(right) + u8::from(carry_in);
        Self::new_selector([0, 0, 1], left, right, carry_in, sum & 1 == 1, sum >= 2)
    }

    fn new_selector(
        selectors: [u64; 3],
        left: bool,
        right: bool,
        carry_in: bool,
        out: bool,
        carry_out: bool,
    ) -> Self {
        Self {
            select_and: F(selectors[0]),
            select_xor: F(selectors[1]),
            select_full_adder: F(selectors[2]),
            left: F(u64::from(left)),
            right: F(u64::from(right)),
            carry_in: F(u64::from(carry_in)),
            out: F(u64::from(out)),
            carry_out: F(u64::from(carry_out)),
        }
    }

    /// Evaluate the selector, Boolean, and selected gate equations.
    ///
    /// The bit equations have degree two; multiplying them by one fixed
    /// selector yields degree three, within the profile's degree-four ceiling.
    pub(crate) fn validate(self) -> Result<(), ZkX509AirErrorV1> {
        validate_bits_v1(&[
            self.select_and,
            self.select_xor,
            self.select_full_adder,
            self.left,
            self.right,
            self.carry_in,
            self.out,
            self.carry_out,
        ])?;
        if self
            .select_and
            .add(self.select_xor)
            .add(self.select_full_adder)
            != F::ONE
        {
            return Err(ZkX509AirErrorV1::GateSelector);
        }

        let and_constraint = self.out.sub(self.left.mul(self.right));
        let xor_constraint = self.out.sub(
            self.left
                .add(self.right)
                .sub(F(2).mul(self.left.mul(self.right))),
        );
        let adder_constraint = self
            .left
            .add(self.right)
            .add(self.carry_in)
            .sub(self.out)
            .sub(F(2).mul(self.carry_out));
        let selected = self
            .select_and
            .mul(and_constraint)
            .add(self.select_xor.mul(xor_constraint))
            .add(self.select_full_adder.mul(adder_constraint));
        if selected != F::ZERO {
            return Err(ZkX509AirErrorV1::BitGate);
        }
        if self.select_full_adder == F::ZERO
            && (self.carry_in != F::ZERO || self.carry_out != F::ZERO)
        {
            return Err(ZkX509AirErrorV1::BitGate);
        }
        Ok(())
    }
}

fn validate_bits_v1(bits: &[F]) -> Result<(), ZkX509AirErrorV1> {
    if bits
        .iter()
        .copied()
        .any(|bit| bit.mul(bit.sub(F::ONE)) != F::ZERO)
    {
        return Err(ZkX509AirErrorV1::NonBoolean);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_and_word_range_rows_reject_every_local_malleation() {
        for value in 0..=u8::MAX {
            ByteRangeAirRowV1::from_byte(value)
                .validate()
                .expect("canonical byte row");
        }
        for value in [0, 1, u32::from(u8::MAX), 1 << 31, u32::MAX] {
            U32RangeAirRowV1::from_u32(value)
                .validate()
                .expect("canonical word row");
        }

        let mut changed = ByteRangeAirRowV1::from_byte(173);
        changed.bits[4] = F(2);
        assert_eq!(changed.validate(), Err(ZkX509AirErrorV1::NonBoolean));
        let mut changed = ByteRangeAirRowV1::from_byte(173);
        changed.value = changed.value.add(F::ONE);
        assert_eq!(
            changed.validate(),
            Err(ZkX509AirErrorV1::RangeDecomposition)
        );
        let mut changed = U32RangeAirRowV1::from_u32(0xdead_beef);
        changed.bits[31] = changed.bits[31].sub(F::ONE);
        assert_eq!(
            changed.validate(),
            Err(ZkX509AirErrorV1::RangeDecomposition)
        );
    }

    #[test]
    fn boolean_gate_rows_cover_truth_tables_and_reject_adversarial_outputs() {
        for left in [false, true] {
            for right in [false, true] {
                BooleanGateAirRowV1::and(left, right)
                    .validate()
                    .expect("AND row");
                BooleanGateAirRowV1::xor(left, right)
                    .validate()
                    .expect("XOR row");
                for carry in [false, true] {
                    BooleanGateAirRowV1::full_adder(left, right, carry)
                        .validate()
                        .expect("full-adder row");
                }
            }
        }

        let mut changed = BooleanGateAirRowV1::xor(true, false);
        changed.out = F::ZERO;
        assert_eq!(changed.validate(), Err(ZkX509AirErrorV1::BitGate));
        let mut changed = BooleanGateAirRowV1::and(true, true);
        changed.select_xor = F::ONE;
        assert_eq!(changed.validate(), Err(ZkX509AirErrorV1::GateSelector));
        let mut changed = BooleanGateAirRowV1::and(false, false);
        changed.carry_out = F::ONE;
        assert_eq!(changed.validate(), Err(ZkX509AirErrorV1::BitGate));
    }

    #[test]
    fn incomplete_air_gap_inventory_is_explicit_and_unique() {
        assert_eq!(ZK_X509_AIR_GAPS_V1.len(), 7);
        for (index, gap) in ZK_X509_AIR_GAPS_V1.iter().enumerate() {
            assert!(!ZK_X509_AIR_GAPS_V1[..index].contains(gap));
        }
    }
}
