//! Public FCMP++ commitment-conservation equation.
//!
//! Full-chain membership proves that each pseudo-out is a rerandomization of
//! the amount commitment in one consumed output. It does not prove that the
//! newly created output commitments preserve their aggregate. The transaction
//! boundary must therefore check the independent Edwards-group equation
//! `sum(pseudo_out) == sum(new_output.amount_commitment)`.
use super::{
    FCMP_MAX_INPUTS_NATIVE_V1, FCMP_MAX_OUTPUTS_NATIVE_V1, FcmpNativeErrorV1, FcmpOutputTupleV1,
    FcmpProofInputPublicV1, FcmpTreeRootV1, decode_fcmp_plus_plus_wire_v1,
    field::decode_edwards_point, membership::verify_fcmp_membership_parsed_v1,
    verify_fcmp_range_v1,
};
use curve25519_dalek::{edwards::EdwardsPoint, traits::Identity as _};
use std::collections::BTreeSet;
/// Verify the complete FCMP++ transaction relation needed before ledger effects may be derived.
///
/// This combines native membership/SAL, the independent public commitment-conservation equation,
/// and the ordered aggregate strict-positive output range proof. Callers admitting outputs must use
/// this entry point; no production membership-only verifier is exposed.
pub fn verify_fcmp_transaction_v1(
    context_hash: [u8; 32],
    proof_wire: &[u8],
    public_inputs: &[FcmpProofInputPublicV1],
    new_outputs: &[FcmpOutputTupleV1],
    root: FcmpTreeRootV1,
) -> Result<(), FcmpNativeErrorV1> {
    let parsed = decode_fcmp_plus_plus_wire_v1(proof_wire, public_inputs, root)?;
    if usize::from(parsed.output_count) != new_outputs.len() {
        return Err(FcmpNativeErrorV1::ProofHeaderMismatch);
    }
    verify_fcmp_commitment_balance_v1(public_inputs, new_outputs)?;
    verify_fcmp_range_v1(context_hash, new_outputs, &parsed.range_proof)?;
    verify_fcmp_membership_parsed_v1(context_hash, &parsed, public_inputs, root)
}
/// Verify exact conservation of the public pseudo-out and new-output commitment aggregates.
///
/// The public transaction fee is intentionally absent from this equation: Iroha's canonical
/// `FeePaymentIntent` is paid outside the confidential asset pool and is already bound through the
/// statement's transaction-intent digest.
pub fn verify_fcmp_commitment_balance_v1(
    public_inputs: &[FcmpProofInputPublicV1],
    new_outputs: &[FcmpOutputTupleV1],
) -> Result<(), FcmpNativeErrorV1> {
    if public_inputs.is_empty() || public_inputs.len() > FCMP_MAX_INPUTS_NATIVE_V1 {
        return Err(FcmpNativeErrorV1::InputCount {
            actual: public_inputs.len(),
            max: FCMP_MAX_INPUTS_NATIVE_V1,
        });
    }
    if new_outputs.is_empty() || new_outputs.len() > FCMP_MAX_OUTPUTS_NATIVE_V1 {
        return Err(FcmpNativeErrorV1::OutputCount {
            actual: new_outputs.len(),
            max: FCMP_MAX_OUTPUTS_NATIVE_V1,
        });
    }
    let mut pseudo_out_encodings = BTreeSet::new();
    let mut pseudo_out_sum = EdwardsPoint::identity();
    for input in public_inputs {
        if !pseudo_out_encodings.insert(input.pseudo_out) {
            return Err(FcmpNativeErrorV1::DuplicatePseudoOut);
        }
        pseudo_out_sum += decode_edwards_point(input.pseudo_out, false)?;
    }
    let mut output_ids = BTreeSet::new();
    let mut output_sum = EdwardsPoint::identity();
    for output in new_outputs {
        if !output_ids.insert(output.output_id()) {
            return Err(FcmpNativeErrorV1::DuplicateOutput);
        }
        output_sum += decode_edwards_point(output.components().2, false)?;
    }
    // Individual points are already required to be non-identity. Rejecting a
    // cancelling aggregate closes the remaining degenerate zero-sum case.
    if pseudo_out_sum == EdwardsPoint::identity() || output_sum == EdwardsPoint::identity() {
        return Err(FcmpNativeErrorV1::CommitmentBalanceIdentity);
    }
    if pseudo_out_sum != output_sum {
        return Err(FcmpNativeErrorV1::CommitmentBalanceEquation);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::fcmp_plus_plus::output_from_multiples;
    use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};
    fn public(pseudo_out: EdwardsPoint, key_image: u64) -> FcmpProofInputPublicV1 {
        FcmpProofInputPublicV1::new(
            (ED25519_BASEPOINT_POINT * Scalar::from(11_u64))
                .compress()
                .to_bytes(),
            (ED25519_BASEPOINT_POINT * Scalar::from(13_u64))
                .compress()
                .to_bytes(),
            (ED25519_BASEPOINT_POINT * Scalar::from(17_u64))
                .compress()
                .to_bytes(),
            pseudo_out.compress().to_bytes(),
            (ED25519_BASEPOINT_POINT * Scalar::from(key_image))
                .compress()
                .to_bytes(),
        )
        .expect("canonical public input")
    }
    #[test]
    fn balance_accepts_exact_ordered_split_and_rejects_every_degeneracy() {
        let first = output_from_multiples(31, 37, 41);
        let second = output_from_multiples(43, 47, 53);
        let expected_sum = decode_edwards_point(first.components().2, false)
            .expect("first commitment")
            + decode_edwards_point(second.components().2, false).expect("second commitment");
        let input = public(expected_sum, 59);
        verify_fcmp_commitment_balance_v1(&[input], &[first, second])
            .expect("exact commitment split balances");
        let changed = output_from_multiples(43, 47, 61);
        assert_eq!(
            verify_fcmp_commitment_balance_v1(&[input], &[first, changed]),
            Err(FcmpNativeErrorV1::CommitmentBalanceEquation)
        );
        assert_eq!(
            verify_fcmp_commitment_balance_v1(&[input], &[first, first]),
            Err(FcmpNativeErrorV1::DuplicateOutput)
        );
        assert_eq!(
            verify_fcmp_commitment_balance_v1(&[input, input], &[first, second]),
            Err(FcmpNativeErrorV1::DuplicatePseudoOut)
        );
        assert_eq!(
            verify_fcmp_commitment_balance_v1(&[], &[first]),
            Err(FcmpNativeErrorV1::InputCount {
                actual: 0,
                max: FCMP_MAX_INPUTS_NATIVE_V1,
            })
        );
        assert_eq!(
            verify_fcmp_commitment_balance_v1(
                &[input],
                &[
                    first,
                    second,
                    changed,
                    output_from_multiples(67, 71, 73),
                    output_from_multiples(79, 83, 89)
                ],
            ),
            Err(FcmpNativeErrorV1::OutputCount {
                actual: FCMP_MAX_OUTPUTS_NATIVE_V1 + 1,
                max: FCMP_MAX_OUTPUTS_NATIVE_V1,
            })
        );
    }
    #[test]
    fn balance_rejects_cancelling_identity_aggregate() {
        let commitment = ED25519_BASEPOINT_POINT * Scalar::from(97_u64);
        let inverse = -commitment;
        let first = FcmpOutputTupleV1::new(
            (ED25519_BASEPOINT_POINT * Scalar::from(101_u64))
                .compress()
                .to_bytes(),
            (ED25519_BASEPOINT_POINT * Scalar::from(103_u64))
                .compress()
                .to_bytes(),
            commitment.compress().to_bytes(),
        )
        .expect("first output");
        let second = FcmpOutputTupleV1::new(
            (ED25519_BASEPOINT_POINT * Scalar::from(107_u64))
                .compress()
                .to_bytes(),
            (ED25519_BASEPOINT_POINT * Scalar::from(109_u64))
                .compress()
                .to_bytes(),
            inverse.compress().to_bytes(),
        )
        .expect("second output");
        let inputs = [public(commitment, 113), public(inverse, 127)];
        assert_eq!(
            verify_fcmp_commitment_balance_v1(&inputs, &[first, second]),
            Err(FcmpNativeErrorV1::CommitmentBalanceIdentity)
        );
    }
}
