//! Claim accumulators over genuine 2^16 IPA generators.

use ff::Field;
use group::CurveAffine;
use iroha_fold::claims::{Claim, ClaimAccumulator, claim_for, decide_claims, fold_claim, verify_claim_fold};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve};
use iroha_plonk::pcs::ipa::PinnedParams;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const DOMAIN: [u8; 32] = *b"iroha_fold claim test domain!!!!";

fn run<C: PastaCurve>(seed: u64) {
    let params = PinnedParams::<C>::derive(16).expect("params");
    let g = params.params().g();
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut challenges = |prefix_zeros: usize| -> [C::ScalarExt; 16] {
        core::array::from_fn(|i| if i < prefix_zeros { C::ScalarExt::ZERO } else { C::ScalarExt::random(&mut rng) })
    };
    // Three full claims and one k12 source with four leading zero challenges.
    let claims: Vec<Claim<C>> = [0, 0, 4, 0]
        .into_iter()
        .map(|zeros| claim_for::<C>(g, challenges(zeros)).expect("claim"))
        .collect();
    let (mut acc, mut s) = ClaimAccumulator::from_claim(&DOMAIN, &claims[0]);
    decide_claims(g, &acc, &s).expect("one true claim");
    for claim in &claims[1..] {
        let (next, witness, proof) = fold_claim(&DOMAIN, &acc, &s, claim).expect("fold");
        assert_eq!(verify_claim_fold(&DOMAIN, &acc, claim, &proof), next);
        acc = next;
        s = witness;
    }
    decide_claims(g, &acc, &s).expect("folded true claims decide");
    // A false claim (shifted point, true challenges) never decides after folding.
    let mut bad = claim_for::<C>(g, challenges(0)).expect("claim");
    bad.g = (C::from(bad.g) + C::from(C::AffineExt::generator())).to_affine();
    let (folded, witness, _) = fold_claim(&DOMAIN, &acc, &s, &bad).expect("fold");
    assert!(decide_claims(g, &folded, &witness).is_err());
    // Nor does a tampered fold message.
    let good = claim_for::<C>(g, challenges(0)).expect("claim");
    let (_, witness, mut proof) = fold_claim(&DOMAIN, &acc, &s, &good).expect("fold");
    proof.k += C::ScalarExt::ONE;
    let tampered = verify_claim_fold(&DOMAIN, &acc, &good, &proof);
    assert!(decide_claims(g, &tampered, &witness).is_err());
}

#[test]
fn pallas_claims_fold_and_decide() {
    run::<Ep>(7);
    let _ = Fq::ZERO;
}

#[test]
fn vesta_claims_fold_and_decide() {
    run::<Eq>(8);
    let _ = Fp::ZERO;
}
