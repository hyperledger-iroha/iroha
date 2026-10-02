//! Fixed ordinary-State reserved positions, never OEM credential-audit claims.

use halo2_proofs::halo2curves::pasta::{Fp, Fq};

use crate::kagemusha_v1_poseidon::{encode, hash};

const DOMAIN: u64 = u64::from_le_bytes(*b"kgmorst1");

/// Canonical Eq/Fp and Ep/Fq Poseidon constants for the two reserved ordinary-State positions.
///
/// The common State public shape retains these positions because payment and transport
/// formulas share that shape. Ordinary Guard authenticates its exact five original SHA
/// digests directly; these constants make the unused positions deterministic and supply
/// no OEM credential audit, hardware guarantee or financial authority. The fixed preimages
/// are domain `kgmorst1`, arity two, version one and parity-purpose tag one/two.
pub fn kagemusha_ordinary_state_reserved_guard_positions_v1() -> ([u8; 32], [u8; 32]) {
    (
        encode(hash::<Fp>(DOMAIN, &[Fp::from(1), Fp::from(1)])),
        encode(hash::<Fq>(DOMAIN, &[Fq::from(1), Fq::from(2)])),
    )
}

#[cfg(any(
    test,
    feature = "kagemusha-real-proof-harness",
    feature = "kagemusha-production-prover"
))]
pub(super) fn constrain_ordinary_state_reserved_guard_positions_v1<
    F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    eq: [halo2_base::AssignedValue<F>; 2],
    ep: [halo2_base::AssignedValue<F>; 2],
) {
    let (expected_eq, expected_ep) = kagemusha_ordinary_state_reserved_guard_positions_v1();
    let ctx = builder.main(0);
    for (actual, bytes) in [(eq, expected_eq), (ep, expected_ep)] {
        let expected = crate::kagemusha_v1_poseidon::digest_limbs::<F>(bytes);
        for (actual, expected) in actual.into_iter().zip(expected) {
            let constant = ctx.load_constant(expected);
            ctx.constrain_equal(&actual, &constant);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, decode, digest_limbs};
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_proofs::dev::MockProver;

    #[test]
    fn reserved_positions_are_canonical_nonzero_role_distinct_native_poseidon() {
        let (eq, ep) = kagemusha_ordinary_state_reserved_guard_positions_v1();
        assert_ne!(eq, [0; 32]);
        assert_ne!(ep, [0; 32]);
        assert_ne!(eq, ep);
        assert!(decode::<Fp>(eq).is_some());
        assert!(decode::<Fq>(ep).is_some());
        assert_eq!(eq, encode(hash::<Fp>(DOMAIN, &[Fp::from(1), Fp::from(1)])));
        assert_eq!(ep, encode(hash::<Fq>(DOMAIN, &[Fq::from(1), Fq::from(2)])));
    }

    fn check<F: KagemushaPoseidonFieldV1>() {
        let (eq, ep) = kagemusha_ordinary_state_reserved_guard_positions_v1();
        let public: Vec<F> = digest_limbs::<F>(eq)
            .into_iter()
            .chain(digest_limbs::<F>(ep))
            .collect();
        for changed in [None, Some(0), Some(1), Some(2), Some(3)] {
            let mut actual = public.clone();
            if let Some(index) = changed {
                actual[index] += F::ONE;
            }
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(9)
                .use_instance_columns(1);
            let cells: Vec<_> = actual
                .iter()
                .map(|value| builder.main(0).load_witness(*value))
                .collect();
            constrain_ordinary_state_reserved_guard_positions_v1(
                &mut builder,
                [cells[0], cells[1]],
                [cells[2], cells[3]],
            );
            builder.assigned_instances = vec![cells];
            builder.calculate_params(Some(9));
            let result = MockProver::run(9, &builder, vec![actual])
                .expect("reserved-position circuit fits")
                .verify();
            assert_eq!(result.is_ok(), changed.is_none());
        }
    }

    #[test]
    fn both_state_parities_reject_each_substituted_reserved_limb() {
        check::<Fp>();
        check::<Fq>();
    }
}
