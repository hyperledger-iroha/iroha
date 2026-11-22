use super::*;
use iroha_pasta::{Ep as Pallas, Eq as Vesta};

fn scalar<S: PrimeField>(seed: u64) -> S {
    let mut v = S::from(seed);
    for _ in 0..7 {
        v = v.square() + S::from(seed + 1);
    }
    v
}

/// Four checked sources over the first `2^k` generators. `pattern` 0 gives
/// distinct sources, 1 the repeated single-parent slots, and 2 places zero and
/// one challenges in the leading block bits.
fn claims<C: PastaCurve>(gens: &Gens<C>, k: usize, pattern: usize) -> Vec<Claim<C>> {
    let mut out: Vec<Claim<C>> = (0..SOURCE_COUNT)
        .map(|j| {
            let mut us = (0..k)
                .map(|i| scalar::<F<C>>((11 + 5 * i + 97 * j) as u64))
                .collect::<Vec<_>>();
            if pattern == 2 {
                us[j % k] = F::<C>::ZERO;
                us[(j + 1) % k] = F::<C>::ONE;
            }
            let g = msm_public::<C>(&s_vector(&us), &gens.g[..1 << k], MemoryBudget::DEFAULT)
                .unwrap()
                .to_affine();
            Claim { us, g }
        })
        .collect();
    if pattern == 1 {
        out[2] = out[0].clone();
        out[3] = out[1].clone();
    }
    out
}

fn assert_same<C: PastaCurve>(a: &(Fold<C>, Trace<C>), b: &(Fold<C>, Trace<C>), case: &str) {
    let ((fa, ta), (fb, tb)) = (a, b);
    for (x, y) in fa.sources.iter().zip(&fb.sources) {
        assert!(x.g == y.g && x.us == y.us, "{case}: sources");
    }
    assert_eq!(fa.sources.len(), fb.sources.len(), "{case}: source count");
    assert_eq!(fa.masks, fb.masks, "{case}: masks");
    assert_eq!(fa.d, fb.d, "{case}: D");
    assert_eq!(fa.lr, fb.lr, "{case}: L/R");
    assert_eq!(fa.c, fb.c, "{case}: c");
    assert_eq!(fa.output.us, fb.output.us, "{case}: output challenges");
    assert_eq!(fa.output.g, fb.output.g, "{case}: output G");
    assert_eq!(ta.relation, tb.relation, "{case}: relation");
    assert_eq!(
        format!("{:?}", ta.layout),
        format!("{:?}", tb.layout),
        "{case}"
    );
    assert_eq!(ta.shared, tb.shared, "{case}: shared limbs");
    assert_eq!(ta.input_header, tb.input_header, "{case}: input header");
    assert_eq!(ta.fold_header, tb.fold_header, "{case}: fold header");
    assert_eq!(ta.digest, tb.digest, "{case}: digest");
    assert_eq!(ta.challenges, tb.challenges, "{case}: challenges");
    assert_eq!(ta.mu, tb.mu, "{case}: mu");
    assert_eq!(ta.point, tb.point, "{case}: point");
    assert_eq!(ta.zeta, tb.zeta, "{case}: zeta");
    assert_eq!(ta.gamma, tb.gamma, "{case}: gamma");
    assert_eq!(ta.value, tb.value, "{case}: value");
    assert_eq!(ta.b_final, tb.b_final, "{case}: b_final");
    assert_eq!(ta.commitment, tb.commitment, "{case}: C-star");
}

#[test]
fn source_blocks_partition_the_public_generators() {
    fn check<C: PastaCurve>()
    where
        F<C>: Scalar,
    {
        let gens = Gens::<C>::new(1 << 7);
        for k in 1..=7 {
            for claim in claims(&gens, k, 2).iter().take(2) {
                for depth in 0..=k {
                    let blocks = source_blocks(&gens, &claim.us, depth).unwrap();
                    assert_eq!(blocks.len(), 1 << depth);
                    let tail = s_vector(&claim.us[depth..]);
                    for (j, block) in blocks.iter().enumerate() {
                        let bases = &gens.g[j * tail.len()..(j + 1) * tail.len()];
                        let expected = msm_public::<C>(&tail, bases, MemoryBudget::DEFAULT);
                        assert_eq!(C::from(*block), expected.unwrap(), "k {k} depth {depth}");
                    }
                    assert_eq!(
                        recombine::<C>(&blocks, &claim.us),
                        claim.g,
                        "k {k} depth {depth}"
                    );
                }
                assert!(matches!(
                    source_blocks(&gens, &claim.us, k + 1),
                    Err("generator source dimension")
                ));
            }
        }
        assert!(matches!(
            source_blocks(&gens.prefix(4), &[F::<C>::ONE; 3], 1),
            Err("generator source dimension")
        ));
    }
    check::<Pallas>();
    check::<Vesta>();
}

#[test]
fn leading_block_rounds_match_full_width_rounds_exactly() {
    fn check<C: PastaCurve>()
    where
        F<C>: Scalar,
    {
        // A prefix larger than 2^k must leave the claims and the fold unchanged.
        let gens = Gens::<C>::new(1 << 6);
        for k in 1..=6 {
            for pattern in 0..3 {
                let input = claims(&gens, k, pattern);
                let seed = format!("block rounds {k} {pattern}");
                let full = prove_with_rng_at(
                    &gens,
                    [41; 32],
                    input.clone(),
                    0,
                    &mut SpikeRng::new(seed.as_bytes()),
                )
                .unwrap();
                assert!(verify(&gens, [41; 32], &full.0));
                for depth in 1..=k + 1 {
                    let blocked = prove_with_rng_at(
                        &gens,
                        [41; 32],
                        input.clone(),
                        depth,
                        &mut SpikeRng::new(seed.as_bytes()),
                    )
                    .unwrap();
                    assert_same(
                        &full,
                        &blocked,
                        &format!("k {k} pattern {pattern} depth {depth}"),
                    );
                }
            }
        }
        // A fresh mask stream still changes every round message.
        let input = claims(&gens, 4, 0);
        let a = prove_with_rng(&gens, [41; 32], input.clone(), &mut SpikeRng::new(b"A")).unwrap();
        let b = prove_with_rng(&gens, [41; 32], input, &mut SpikeRng::new(b"B")).unwrap();
        assert!(
            a.0.lr
                .iter()
                .zip(&b.0.lr)
                .all(|(x, y)| x.0 != y.0 && x.1 != y.1)
        );
        assert_ne!(a.0.output.g, b.0.output.g);
    }
    check::<Pallas>();
    check::<Vesta>();
}

#[test]
fn block_source_checks_refuse_every_wrong_claim() {
    fn check<C: PastaCurve>()
    where
        F<C>: Scalar,
    {
        let gens = Gens::<C>::new(1 << 4);
        let input = claims(&gens, 4, 0);
        for depth in 0..=4 {
            for slot in 0..SOURCE_COUNT {
                for case in 0..3 {
                    let mut bad = input.clone();
                    match case {
                        0 => bad[slot].g = (C::from(bad[slot].g) + C::from(gens.g[0])).to_affine(),
                        1 => bad[slot].us[depth.min(3)] += F::<C>::ONE,
                        _ => bad[slot].us.push(F::<C>::ONE),
                    }
                    let result = prove_with_rng_at(
                        &gens,
                        [7; 32],
                        bad,
                        depth,
                        &mut SpikeRng::new(b"wrong source"),
                    );
                    let expected = if case == 2 && slot == 0 {
                        "generator source dimension"
                    } else {
                        "wallet predecessor generator claim"
                    };
                    assert!(
                        matches!(result, Err(e) if e == expected),
                        "depth {depth} slot {slot} case {case}"
                    );
                }
            }
        }
        let entropy = ProofEntropy::fresh().unwrap();
        let parents = [input[0].clone(), input[1].clone()];
        let incoming = [input[2].us.as_slice(), &input[3].us[..3]];
        assert!(matches!(
            prove_predecessors(
                &gens,
                [7; 32],
                parents.clone(),
                Some(incoming),
                &entropy,
                b"x"
            ),
            Err("generator source dimension")
        ));
        let mut wrong = parents.clone();
        wrong[1].g = wrong[0].g;
        assert!(matches!(
            prove_predecessors(&gens, [7; 32], wrong, Some(incoming), &entropy, b"x"),
            Err("wallet predecessor generator claim")
        ));
    }
    check::<Pallas>();
    check::<Vesta>();
}
