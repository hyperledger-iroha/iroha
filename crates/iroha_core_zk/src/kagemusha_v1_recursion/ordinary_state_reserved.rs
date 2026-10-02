//! First-release ordinary State OUTER protocol commitments in the four former audit positions.
//! They name real release-selected State protocols and never claim OEM credential audits.

/// Return the actual admitted outer State Eq/Ep protocol commitments. This is a data projection
/// of the independently authenticated native verifier, not a proof, clock or monetary grant.
pub fn kagemusha_ordinary_state_outer_protocol_positions_v1(
    verifier: &super::KagemushaAuthenticatedRecursiveVerifierV1,
) -> ([u8; 32], [u8; 32]) {
    let material = verifier.state_checkpoint_material();
    (
        material.binding.outer_eq_protocol_digest,
        material.binding.outer_ep_protocol_digest,
    )
}

/// Validate the complete variable public commitments. Active parent acceptance must additionally
/// use these same cells in load_and_constrain_parent_protocol and fold every actual outer opening;
/// the Native current State verifier pins them to its genuine installed release protocols.
/// Keeping them public avoids a fixed self-referential State-inner/State-outer key constant.
pub(super) fn constrain_ordinary_state_outer_protocol_positions_v1<
    F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    eq: [halo2_base::AssignedValue<F>; 2],
    ep: [halo2_base::AssignedValue<F>; 2],
) {
    use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for pair in [eq, ep] {
        for limb in pair {
            range.range_check(ctx, limb, 128);
        }
        let sum = range.gate().sum(ctx, pair);
        let zero = range.gate().is_zero(ctx, sum);
        range.gate().assert_is_const(ctx, &zero, &F::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, encode};
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };
    fn check<F: KagemushaPoseidonFieldV1>() {
        // Data-only stand-ins exercise copy/zero/range relations, never an installed verifier.
        let expected: Vec<F> = digest_limbs::<F>(encode(Fp::from(101)))
            .into_iter()
            .chain(digest_limbs::<F>(encode(Fq::from(102))))
            .collect();
        for changed in [None, Some(0), Some(1), Some(2), Some(3)] {
            let mut actual = expected.clone();
            if let Some(i) = changed {
                actual[i] += F::ONE;
            }
            let mut b = BaseCircuitBuilder::<F>::new(false)
                .use_k(10)
                .use_lookup_bits(8)
                .use_instance_columns(1);
            let cells = actual
                .iter()
                .map(|v| b.main(0).load_witness(*v))
                .collect::<Vec<_>>();
            constrain_ordinary_state_outer_protocol_positions_v1(
                &mut b,
                [cells[0], cells[1]],
                [cells[2], cells[3]],
            );
            for (cell, value) in cells.iter().zip(&expected) {
                let c = b.main(0).load_constant(*value);
                b.main(0).constrain_equal(cell, &c);
            }
            b.assigned_instances = vec![cells];
            b.calculate_params(Some(9));
            assert_eq!(
                MockProver::run(10, &b, vec![actual])
                    .unwrap()
                    .verify()
                    .is_ok(),
                changed.is_none()
            );
        }
        let mut b = BaseCircuitBuilder::<F>::new(false)
            .use_k(10)
            .use_lookup_bits(8)
            .use_instance_columns(1);
        let zero = b.main(0).load_zero();
        constrain_ordinary_state_outer_protocol_positions_v1(&mut b, [zero, zero], [zero, zero]);
        b.calculate_params(Some(9));
        assert!(
            MockProver::run(10, &b, vec![vec![]])
                .unwrap()
                .verify()
                .is_err()
        );
    }
    #[test]
    fn ordinary_outer_protocol_cells_require_real_exact_purpose_copies_both_fields() {
        check::<Fp>();
        check::<Fq>();
    }
}
