//! Constrained building blocks and fail-closed gap inventory for zk-X509.
//!
//! This module contains algebraic constraints, not native re-checks disguised
//! as proofs.  Every row evaluator returns zero only when its polynomial
//! identities hold over Goldilocks.  The completed prover will place these
//! rows in masked committed segments and evaluate the same identities at
//! transcript-derived points. Consensus activation remains unavailable until
//! every gap in [`ZK_X509_AIR_GAPS_V1`] is closed by numeric trace material,
//! an independently replaying verifier, and adversarial tests.

use thiserror::Error;

use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;

/// Exact remaining end-to-end implementation gaps.
///
/// These variants intentionally describe missing executable code surfaces,
/// not broad research milestones. Removing one requires the corresponding
/// canonical constructor, verifier replay, and mutation corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZkX509AirGapV1 {
    /// Construct and independently verify the complete 49-registration aggregate.
    FullAggregateProverAndVerifier,
    /// Encode and bind the main and compact-CA subproofs in one canonical envelope.
    CombinedCredentialEnvelope,
    /// Expose the complete verifier through the consensus privacy dispatcher.
    ConsensusVerifierIntegration,
    /// Pin deterministic end-to-end KATs and measured release resources.
    ReleaseKatAndResourceMeasurements,
}

/// Exhaustive fail-closed inventory in stable manifest order.
pub(crate) const ZK_X509_AIR_GAPS_V1: [ZkX509AirGapV1; 4] = [
    ZkX509AirGapV1::FullAggregateProverAndVerifier,
    ZkX509AirGapV1::CombinedCredentialEnvelope,
    ZkX509AirGapV1::ConsensusVerifierIntegration,
    ZkX509AirGapV1::ReleaseKatAndResourceMeasurements,
];

/// Stable digest input for implemented components and remaining gaps.
pub(crate) const ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1: &[u8] = b"byte-memory-permutation=complete|strict-der-segment=complete|projection-segment=complete|shared-current-next-deep-ali=complete|rfc5280-base-row-provider=complete|rfc5280-aux-products-and-claims=complete|sha-call-witness-assembly-and-terminal-binding=complete|p256-witness-assembly-and-terminal-binding=complete|compact-ca-subproof=complete|full-49-registration-prover-and-verifier=pending|combined-main-ca-envelope=pending|consensus-verifier-integration=pending|release-kat-and-resource-measurements=pending|activation=false";

/// SHA-256 of the dedicated compact-CA prover/verifier descriptor.
///
/// The pin lives beside the fail-closed gap manifest so closing the gap binds
/// the exact X5C1/X5C2 proof system rather than only its component name.
pub(crate) const ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1: [u8; 32] = [
    0x11, 0x4e, 0xc7, 0x82, 0x6c, 0x04, 0x75, 0xf8, 0x7a, 0x87, 0xe9, 0x21, 0x4c, 0x04, 0xbc, 0x37,
    0x58, 0x84, 0x2d, 0x70, 0x5a, 0x5e, 0xfb, 0x45, 0xa1, 0x0b, 0x73, 0x4d, 0xab, 0x30, 0xc4, 0x38,
];

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
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::privacy_engines::zk_x509::{
        accumulator_air::{
            ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1, ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1,
            ZkX509CaAccumulatorTraceV1,
        },
        accumulator_stark::{
            ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1, ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
            ZK_X509_CA_ACCUMULATOR_CHUNKS_V1, ZK_X509_CA_ACCUMULATOR_CLAIM_ENVELOPE_BYTES_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1,
            ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1,
            ZK_X509_CA_ACCUMULATOR_DEEP_OPENING_BYTES_V1, ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
            ZK_X509_CA_ACCUMULATOR_INNER_MAX_PROOF_BYTES_V1,
            ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1, ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            ZkX509CaAccumulatorProofErrorV1, ZkX509CaAccumulatorStarkPublicV1,
            ZkX509CaAccumulatorSubproofBindingV1, ca_accumulator_proof_binding_digest_v1,
            ca_accumulator_subproof_binding_from_proof_v1, prove_zk_x509_ca_accumulator_stark_v1,
            verify_zk_x509_ca_accumulator_stark_v1,
        },
        credential_pre_aux::ZkX509CredentialMainPreAuxV1,
        profile::{
            ZK_X509_CA_FRI_LDE_LOG2_V1, ZK_X509_CA_FRI_ROUNDS_V1,
            ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1, ZK_X509_CA_FRI_TERMINAL_LOG2_V1,
            ZK_X509_CA_TRACE_MASK_DEGREE_V1, ZK_X509_FRI_QUERY_COUNT_V1, ZK_X509_GRINDING_BITS_V1,
        },
        sha_call_bus_stark::ZkX509ShaCallScheduleV1,
    };

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
    fn gap_inventory_is_explicit_unique_and_manifested() {
        assert_eq!(ZK_X509_AIR_GAPS_V1.len(), 4);
        for (index, gap) in ZK_X509_AIR_GAPS_V1.iter().enumerate() {
            assert!(!ZK_X509_AIR_GAPS_V1[..index].contains(gap));
        }
        let descriptor = String::from_utf8_lossy(ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1);
        assert_eq!(descriptor.matches("=complete").count(), 9);
        assert_eq!(
            descriptor.matches("=pending").count(),
            ZK_X509_AIR_GAPS_V1.len()
        );
        assert!(descriptor.ends_with("activation=false"));
    }

    #[test]
    fn compact_ca_gap_pin_names_the_exact_dedicated_prover_and_verifier() {
        let digest: [u8; 32] = Sha256::digest(ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1).into();
        assert_eq!(digest, ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1);
        let descriptor = String::from_utf8_lossy(ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1);
        for required in [
            "wire-envelope-X5C1+inner-X5C2",
            "strict-version-adapter-claim-addresses-length-and-no-trailing-bytes",
            "dedicated-lde-log14",
            "trace-mask306-coefficients",
            "fri58-distinct-post-grinding20",
            "one-shared-deep-point-current+next",
            "all-four-terminal-families-algebraically-bound",
            "typed-outer-binding=public-root+channel+ordered-sha13+rfc91",
            "shared-X5S1-pre-aux-after-six-main-plus-one-ca-base-roots",
            "checked-native-lde-scratch-resident-and-work-ceilings",
            "producer-self-verifies",
        ] {
            assert!(
                descriptor.contains(required),
                "compact-CA descriptor must bind {required}"
            );
        }

        assert_eq!(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1, 7);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1, 128);
        assert_eq!(ZK_X509_CA_FRI_LDE_LOG2_V1, 14);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1, 695);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1, 128);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1, 80);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1, 1_379);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1, 3);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_CHUNKS_V1, 13);
        assert_eq!(ZK_X509_CA_TRACE_MASK_DEGREE_V1 + 1, 306);
        assert_eq!(ZK_X509_FRI_QUERY_COUNT_V1, 58);
        assert_eq!(ZK_X509_CA_FRI_ROUNDS_V1, 5);
        assert_eq!(ZK_X509_CA_FRI_TERMINAL_LOG2_V1, 9);
        assert_eq!(ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1, 15);
        assert_eq!(ZK_X509_GRINDING_BITS_V1, 20);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_CLAIM_ENVELOPE_BYTES_V1, 1_310);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_DEEP_OPENING_BYTES_V1, 52_768);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_INNER_MAX_PROOF_BYTES_V1, 1_036_984);
        assert_eq!(ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1, 1_038_294);

        let _prover: fn(
            &ZkX509CaAccumulatorTraceV1,
            &ZkX509ShaCallScheduleV1,
            ZkX509CredentialMainPreAuxV1,
        ) -> Result<Vec<u8>, ZkX509CaAccumulatorProofErrorV1> =
            prove_zk_x509_ca_accumulator_stark_v1;
        let _verifier: fn(
            ZkX509CaAccumulatorStarkPublicV1,
            &ZkX509ShaCallScheduleV1,
            ZkX509CredentialMainPreAuxV1,
            &[u8],
        ) -> Result<(), ZkX509CaAccumulatorProofErrorV1> = verify_zk_x509_ca_accumulator_stark_v1;
        let _binding: fn(
            ZkX509CaAccumulatorStarkPublicV1,
            &ZkX509ShaCallScheduleV1,
            ZkX509CredentialMainPreAuxV1,
            &[u8],
        ) -> Result<
            ZkX509CaAccumulatorSubproofBindingV1,
            ZkX509CaAccumulatorProofErrorV1,
        > = ca_accumulator_subproof_binding_from_proof_v1;
        let _binding_digest: fn(
            ZkX509CaAccumulatorStarkPublicV1,
            &ZkX509ShaCallScheduleV1,
            ZkX509CredentialMainPreAuxV1,
            &[u8],
        ) -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> =
            ca_accumulator_proof_binding_digest_v1;
    }
}
