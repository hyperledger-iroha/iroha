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
fn checked_public_blocks_require_the_exact_generator_claim_on_both_curves() {
    fn check<C: PastaCurve>() {
        let gens = Gens::<C>::new(1 << 7);
        for k in 0..=7 {
            let challenges = (0..k)
                .map(|i| scalar::<F<C>>(9 + i as u64))
                .collect::<Vec<_>>();
            for us in [challenges, vec![F::<C>::ZERO; k]] {
                let g = msm_public::<C>(&s_vector(&us), &gens.g[..1 << k], MemoryBudget::DEFAULT)
                    .unwrap()
                    .to_affine();
                let claim = Claim { g, us };
                let checked = VerifiedBlocks::decide(&gens, &claim.us, claim.g).unwrap();
                let blocks = checked.for_claim(&claim, k).unwrap();
                assert_eq!(blocks.len(), 1 << BLOCK_DEPTH.min(k));
                if k > 0 {
                    let private = source_blocks(&gens, &claim.us, BLOCK_DEPTH.min(k)).unwrap();
                    assert_eq!(*blocks, *private);
                    let mut changed = claim.clone();
                    changed.us[k / 2] += F::<C>::ONE;
                    assert!(checked.for_claim(&changed, k).is_err());
                }
                let different = Claim {
                    us: claim.us.clone(),
                    g: (C::from(claim.g) + C::from(gens.h)).to_affine(),
                };
                assert!(checked.for_claim(&different, k).is_err());
                assert!(checked.for_claim(&claim, k + 1).is_err());
                assert!(VerifiedBlocks::decide(&gens, &claim.us, different.g).is_err());
            }
        }
        assert!(VerifiedBlocks::decide(&gens, &[F::<C>::ONE; 8], gens.g[0]).is_err());
        assert!(
            VerifiedBlocks::decide(&gens, &[F::<C>::ONE; usize::BITS as usize], gens.g[0]).is_err()
        );
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

#[test]
fn verified_predecessors_match_the_derived_fold_exactly() {
    fn check<C: PastaCurve>()
    where
        F<C>: Scalar,
    {
        let all = Gens::<C>::new(1 << 6);
        let entropy = ProofEntropy::fresh().unwrap();
        let (relation, purpose) = ([9; 32], b"claimed predecessors".as_slice());
        for k in 1..=6 {
            let gens = all.prefix(1 << k);
            let depth = BLOCK_DEPTH.min(k);
            for pattern in [0, 2] {
                let case = format!("k {k} pattern {pattern}");
                let input = claims(&gens, k, pattern);
                let parents = [input[0].clone(), input[1].clone()];
                let incoming = [input[2].clone(), input[3].clone()];
                let derive = |incoming: &[Claim<C>; 2]| {
                    prove_predecessors(
                        &gens,
                        relation,
                        parents.clone(),
                        Some([&incoming[0].us, &incoming[1].us]),
                        &entropy,
                        purpose,
                    )
                };
                let derived = derive(&incoming).unwrap();
                // True claims close the batched rounds, with the derived fold's bits.
                let batched = prove_rounds(
                    &gens,
                    relation,
                    input.clone(),
                    None,
                    depth,
                    &mut entropy.stream(purpose),
                )
                .expect("true claims close");
                assert_same(&derived, &batched, &case);
                let claimed = fold_verified_points(
                    &gens,
                    relation,
                    parents.clone(),
                    incoming.clone(),
                    &entropy,
                    purpose,
                )
                .unwrap();
                assert_same(&derived, &claimed, &case);
            }
        }
    }
    check::<Pallas>();
    check::<Vesta>();
}

/// Opus review: every false source through the verified entry point. The
/// batched rounds alone refuse each false claim; a false parent is then
/// refused exactly as the stock path refuses it, and a false incoming claim
/// never reaches the fold: the result is the stock derived fold, byte for byte,
/// from the same entropy.
#[test]
fn verified_fold_handles_each_false_claim_like_stock() {
    fn check<C: PastaCurve>()
    where
        F<C>: Scalar,
    {
        let all = Gens::<C>::new(1 << 6);
        let entropy = ProofEntropy::fresh().unwrap();
        let (relation, purpose) = ([9; 32], b"claimed predecessors".as_slice());
        for k in 1..=6 {
            let gens = all.prefix(1 << k);
            let depth = BLOCK_DEPTH.min(k);
            for pattern in [0, 2] {
                let input = claims(&gens, k, pattern);
                for bad in 0..SOURCE_COUNT {
                    let case = format!("k {k} pattern {pattern} false source {bad}");
                    let mut wrong = input.clone();
                    wrong[bad].g = (C::from(wrong[bad].g) + C::from(gens.g[0])).to_affine();
                    assert!(
                        prove_rounds(
                            &gens,
                            relation,
                            wrong.clone(),
                            None,
                            depth,
                            &mut entropy.stream(purpose),
                        )
                        .is_none(),
                        "{case}: the batched rounds closed a false claim"
                    );
                    let parents = [wrong[0].clone(), wrong[1].clone()];
                    let incoming = [wrong[2].clone(), wrong[3].clone()];
                    let verified = fold_verified_points(
                        &gens,
                        relation,
                        parents.clone(),
                        incoming.clone(),
                        &entropy,
                        purpose,
                    );
                    let stock = prove_predecessors(
                        &gens,
                        relation,
                        parents,
                        Some([&incoming[0].us, &incoming[1].us]),
                        &entropy,
                        purpose,
                    );
                    match (verified, stock) {
                        (Err(a), Err(b)) => {
                            assert!(bad < 2, "{case}: an incoming claim refused the fold");
                            assert_eq!(a, b, "{case}: refusal");
                        }
                        (Ok(a), Ok(b)) => {
                            assert!(bad >= 2, "{case}: a false parent folded");
                            assert_same(&a, &b, &case);
                            assert!(a.0.sources[bad].g == input[bad].g, "{case}: derived point");
                        }
                        _ => panic!("{case}: the verified and stock paths disagree"),
                    }
                }
            }
        }
    }
    check::<Pallas>();
    check::<Vesta>();
}

/// Process CPU seconds (user + system, every thread) for the review timings.
fn process_cpu_seconds() -> f64 {
    #[cfg(target_os = "macos")]
    type Usec = i32;
    #[cfg(not(target_os = "macos"))]
    type Usec = i64;
    #[repr(C)]
    struct TimeVal {
        sec: i64,
        usec: Usec,
    }
    #[repr(C)]
    struct RUsage {
        user: TimeVal,
        system: TimeVal,
        rest: [i64; 14],
    }
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut RUsage) -> i32;
    }
    let mut usage = std::mem::MaybeUninit::<RUsage>::zeroed();
    // SAFETY: getrusage(RUSAGE_SELF) writes one complete struct rusage.
    assert_eq!(unsafe { getrusage(0, usage.as_mut_ptr()) }, 0);
    // SAFETY: zero-initialized and then fully written by getrusage.
    let usage = unsafe { usage.assume_init() };
    let seconds = |t: &TimeVal| t.sec as f64 + t.usec as f64 * 1e-6;
    seconds(&usage.user) + seconds(&usage.system)
}

/// Wall and process CPU seconds of `f`.
fn measured<T>(f: impl FnOnce() -> T) -> (T, f64, f64) {
    let (wall, cpu) = (std::time::Instant::now(), process_cpu_seconds());
    let out = f();
    (
        out,
        wall.elapsed().as_secs_f64(),
        process_cpu_seconds() - cpu,
    )
}

/// Opus review: the production widths (2^18 on the primary curve, 2^17 on the
/// secondary). True claims close the batched rounds themselves (no fallback),
/// and the verified fold equals the stock derived fold from the same entropy.
/// Prints wall and process CPU seconds, stock and verified alternating
/// (stock, verified, verified, stock).
#[test]
#[ignore = "production-width generator folds; run in a build window"]
fn verified_fold_matches_stock_at_production_width() {
    fn check<C: PastaCurve>(k: usize, curve: &str)
    where
        F<C>: Scalar,
    {
        let gens = Gens::<C>::new(1 << k);
        let entropy = ProofEntropy::fresh().unwrap();
        let (relation, purpose) = ([9; 32], b"production width".as_slice());
        let input = claims(&gens, k, 0);
        let parents = [input[0].clone(), input[1].clone()];
        let incoming = [input[2].clone(), input[3].clone()];
        let stock = || {
            prove_predecessors(
                &gens,
                relation,
                parents.clone(),
                Some([&incoming[0].us, &incoming[1].us]),
                &entropy,
                purpose,
            )
            .unwrap()
        };
        let verified = || {
            fold_verified_points(
                &gens,
                relation,
                parents.clone(),
                incoming.clone(),
                &entropy,
                purpose,
            )
            .unwrap()
        };
        let case = format!("{curve} 2^{k}");
        let (s1, s1_wall, s1_cpu) = measured(stock);
        let (v1, v1_wall, v1_cpu) = measured(verified);
        let (v2, v2_wall, v2_cpu) = measured(verified);
        let (s2, s2_wall, s2_cpu) = measured(stock);
        let batched = prove_rounds(
            &gens,
            relation,
            input,
            None,
            BLOCK_DEPTH.min(k),
            &mut entropy.stream(purpose),
        )
        .expect("true claims close the batched rounds");
        for (other, label) in [
            (&v1, "verified 1"),
            (&v2, "verified 2"),
            (&s2, "stock 2"),
            (&batched, "batched"),
        ] {
            assert_same(&s1, other, &format!("{case} {label}"));
        }
        eprintln!(
            "opus-review fold {case}: stock wall {s1_wall:.3} {s2_wall:.3} s cpu {s1_cpu:.3} {s2_cpu:.3} s; verified wall {v1_wall:.3} {v2_wall:.3} s cpu {v1_cpu:.3} {v2_cpu:.3} s"
        );
    }
    check::<Vesta>(18, "primary (Vesta)");
    check::<Pallas>(17, "secondary (Pallas)");
}

/// Opus review: every fold round challenge is a 126-bit Halo endoscalar
/// `a zeta + b` with integer coefficients in `[2^63 + 1, 3 2^63 - 1]`, and
/// `a zeta + b = 0` would force `a^2 - ab + b^2 = 0` modulo the field, which the
/// integer bound `0 < a^2 - ab + b^2 < 2^130` excludes on both Pasta fields. So a
/// round challenge is never zero and the zero test in `prove_rounds` cannot fire.
#[test]
fn fold_round_challenges_are_nonzero_endoscalars() {
    fn field<S: PrimeField + ff::WithSmallOrderMulGroup<3>>() {
        assert!(
            S::ZETA != S::ONE && S::ZETA * S::ZETA + S::ZETA + S::ONE == S::ZERO,
            "order 3"
        );
        assert!(S::NUM_BITS >= 254, "Pasta modulus width");
        let (lo, hi) = ((1u128 << 63) + 1, 3 * (1u128 << 63) - 1);
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut patterns: Vec<Vec<bool>> = vec![
            vec![true; crate::fs::CHALLENGE_BITS],
            vec![false; crate::fs::CHALLENGE_BITS],
            (0..crate::fs::CHALLENGE_BITS).map(|i| i % 2 == 1).collect(),
            (0..crate::fs::CHALLENGE_BITS).map(|i| i % 2 == 0).collect(),
        ];
        for _ in 0..256 {
            patterns.push(
                (0..crate::fs::CHALLENGE_BITS)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        state & 1 == 1
                    })
                    .collect(),
            );
        }
        for bits in &patterns {
            let (a, b, sa, sb) = crate::fs::endo_coefficients(bits);
            assert!(sa == 1 && sb == 1, "positive coefficients");
            assert!(
                (lo..=hi).contains(&a) && (lo..=hi).contains(&b),
                "coefficient range"
            );
            // a^2 - ab + b^2 <= max(a, b)^2 < 9 2^126 < 2^130.
            assert!(a.max(b) < 3 * (1u128 << 63));
            assert!(crate::fs::endo_scalar::<S>(bits) != S::ZERO);
        }
        // The extremes are attained: all (sign, endo) pairs set gives a = 3 2^63 - 1.
        assert_eq!(crate::fs::endo_coefficients(&patterns[0]).0, hi);
    }
    field::<F<Vesta>>();
    field::<F<Pallas>>();
}
