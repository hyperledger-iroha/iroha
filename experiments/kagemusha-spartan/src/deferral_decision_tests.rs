//! A valid folding proof does not establish the exported matrix claim.
//! These component programs isolate the native decision; they are not monetary
//! parameter manifests or substitutes for the complete native Burn cases.

use super::*;
use crate::{
    fs::SpikeRng,
    ipa::{F, Gens},
    r1cs::{Instance, Shape, Witness},
    uniform::UniformCircuit,
};
use ff::Field;
use iroha_pasta::PastaCurve;

fn check<C: PastaCurve>()
where
    F<C>: Scalar,
{
    let kind = |sx, row_bits, col_bits| {
        let shape = Shape {
            label: "matrix decision fixture",
            sx,
            row_bits,
            col_bits,
            k: 3,
        };
        Kind {
            sx,
            sz: shape.sz(),
            deg: (0, 0),
            eval: Box::new(move |_: &[F<C>], rx: &[F<C>], ry: &[F<C>]| {
                crate::r1cs::matrix_evals(&shape, rx, ry)
            }),
            eval_many: None,
        }
    };
    let poly = MatPoly::new([kind(9, 2, 6), kind(8, 2, 5)]);
    let mut rng = SpikeRng::new(b"independent native matrix decision");
    let claims: [Claim<F<C>>; 4] = std::array::from_fn(|slot| {
        let point = if slot % 2 == 0 {
            (0..poly.n()).map(|_| rng.scalar()).collect()
        } else {
            let kind = slot / 2;
            let rx: Vec<_> = (0..poly.kinds[kind].sx).map(|_| rng.scalar()).collect();
            let ry: Vec<_> = (0..poly.kinds[kind].sz).map(|_| rng.scalar()).collect();
            poly.fresh_point(kind, rng.scalar(), &[], &rx, &ry)
        };
        Claim {
            value: poly.eval(&point),
            point,
        }
    });
    let polynomial = restrict(&poly, &claims);
    let build = |claims: &[Claim<F<C>>; 4], polynomial: &[F<C>]| {
        let mut wiring = Wiring::default();
        let family = wiring.family(0, 1);
        let fold = fold_arrays(&mut wiring, family, claims, polynomial);
        fix_coords(&mut wiring, &fold, 1, &poly.fixed_coords(0));
        fix_coords(&mut wiring, &fold, 3, &poly.fixed_coords(1));
        expose(&mut wiring, family, &fold, 32);
        let (arrays, pi, counts, tables, copies) = wiring.finish();
        UniformCircuit::build_with_copies(
            "matrix decision fixture",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            4,
            b"same matrix decision program",
        )
    };
    let honest = build(&claims, &polynomial);
    let gens = Gens::<C>::new(honest.0.shape.cols());
    let fixed = crate::spartan::fixed_commitments(&honest.0.shape, &gens, &honest.0);
    let prove = |c: &UniformCircuit<F<C>>, i: &Instance<F<C>>, w: &Witness<F<C>>| {
        let mut rng = SpikeRng::new(b"matrix decision proof");
        crate::spartan::prove_circuit(&c.shape, &gens, i, w, &mut rng, c).0
    };
    let proof = prove(&honest.0, &honest.1, &honest.2);
    assert!(
        crate::spartan::verify_circuit(
            &honest.0.shape,
            &gens,
            &honest.1,
            &proof.to_bytes(),
            &honest.0,
        )
        .is_ok()
    );
    let claim = claim_of(&proof.late);
    assert_eq!(poly.eval(&claim.point), claim.value);

    // Change one private input claim and bend q through that false value.
    // All folding equations remain true; only the final matrix decision fails.
    let mut false_claims = claims;
    false_claims[1].value += F::<C>::ONE;
    let mut false_polynomial = polynomial;
    for (coefficient, correction) in false_polynomial.iter_mut().zip(lagrange4::<F<C>>()[1]) {
        *coefficient += correction;
    }
    let false_fold = build(&false_claims, &false_polynomial);
    assert_eq!(honest.0.program_digest(), false_fold.0.program_digest());
    assert_eq!(honest.1.io, false_fold.1.io);
    assert_eq!(
        fixed,
        crate::spartan::fixed_commitments(&false_fold.0.shape, &gens, &false_fold.0),
    );
    let proof = prove(&false_fold.0, &false_fold.1, &false_fold.2);
    assert!(
        crate::spartan::verify_circuit(
            &honest.0.shape,
            &gens,
            &honest.1,
            &proof.to_bytes(),
            &honest.0,
        )
        .is_ok()
    );
    let trace = crate::verifier::trace_circuit(
        &honest.0.shape,
        &gens,
        &honest.1,
        &proof,
        &[],
        &crate::spartan::Link::None,
        &honest.0,
        &fixed,
    );
    assert!(trace.accepts);
    let claim = claim_of(&proof.late);
    assert_ne!(poly.eval(&claim.point), claim.value);
}

#[test]
fn valid_folding_proof_still_requires_native_matrix_decision_on_both_curves() {
    check::<iroha_pasta::Ep>();
    check::<iroha_pasta::Eq>();
}
