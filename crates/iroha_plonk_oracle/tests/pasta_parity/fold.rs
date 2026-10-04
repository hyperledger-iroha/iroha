//! The lockstep generator fold against the vendored IPA collapse.
//!
//! Two layers:
//!
//! - Per round: `fold_generators_vartime` (lockstep batch-affine GLV, with
//!   the per-block complete-formula fallback), `fold_generators_with`
//!   (precomputed recoding) and `fold_generators_reference` (complete
//!   formulas only) must leave the lower half
//!   of the generator vector byte-identical to the vendored
//!   `parallel_generator_collapse` (reproduced in
//!   `iroha_plonk_oracle::vendored::generator_collapse`). Inputs cover special
//!   challenges (0, ±1, powers of two, the endomorphism scalar `ZETA` and its
//!   relatives) and adversarial lanes: `lo = ±hi`, `lo = ±u hi`, identity
//!   points, and a single exceptional or identity lane inside a block of
//!   ordinary lanes. Vectors of up to 300 generators run by default; 1,000 to
//!   8,192 generators and full 2,048-lane blocks are ignored release tests,
//!   because the vendored collapse multiplies point by point.
//! - Whole proofs: real vendored IPA opening proofs (`create_proof` with the
//!   Blake2b transcript, verified by `verify_proof`) supply the round
//!   challenges. Folding the native generators with them must give the
//!   vendored verifier's `G'_0` (`GuardIPA::compute_g`), the multi-round
//!   vendored collapse must agree, and the native `G'_0` must make the vendored
//!   verifier accept (`GuardIPA::use_g` then `MSM::check`). k = 1..=10 by
//!   default, k = 11..=16 as ignored release tests.

use halo2_axiom::{
    arithmetic::eval_polynomial,
    halo2curves::{
        ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
        group::{Curve, Group, GroupEncoding, prime::PrimeCurveAffine},
    },
    poly::{
        EvaluationDomain,
        commitment::{Blind, MSM, Params, ParamsProver},
        ipa::commitment::{ParamsIPA, create_proof, verify_proof},
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, Transcript, TranscriptReadBuffer,
        TranscriptWriterBuffer,
    },
};
use iroha_pasta::fold::{
    FoldChallenge, fold_generators_reference, fold_generators_vartime, fold_generators_with,
};
use iroha_plonk_oracle::{
    convert::{
        CurveBridge, NativeAffine, Pallas, VendoredCurve, Vesta, native_affines, native_scalar,
        native_scalars, vendored_affine,
    },
    pools::same_on_each_pool,
    vendored::{Recording, generator_collapse},
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use crate::data_rng;

/// Compressed encodings of a point slice.
fn encodings<P: GroupEncoding<Repr = [u8; 32]>>(points: &[P]) -> Vec<[u8; 32]> {
    points.iter().map(GroupEncoding::to_bytes).collect()
}

/// Asserts equal encodings, naming the first differing lane.
fn assert_lanes(label: &str, expected: &[[u8; 32]], actual: &[[u8; 32]]) {
    assert_eq!(expected.len(), actual.len(), "{label}: lane count");
    if let Some(lane) = expected.iter().zip(actual).position(|(e, a)| e != a) {
        panic!("{label}: lane {lane} differs from the vendored collapse");
    }
}

/// One fold round natively (three entry points) and through the vendored
/// collapse, in every pool.
fn fold_round<B: CurveBridge>(label: &str, generators: &[B::Vendored], u: B::VScalar) {
    let native_generators = native_affines::<B>(generators);
    let native_u = native_scalar::<B>(&u);
    let half = generators.len() / 2;
    same_on_each_pool(label, |threads| {
        let label = format!("{label} at {threads} threads");
        let mut vendored = generators.to_vec();
        generator_collapse(&mut vendored, u);
        let expected = encodings(&vendored);

        let mut native = native_generators.clone();
        let returned = fold_generators_vartime::<B::Native>(&mut native, &native_u);
        assert_eq!(returned, half, "{label}: returned half");
        assert_lanes(
            &format!("{label} vartime"),
            &expected[..half],
            &encodings(&native[..half]),
        );
        assert_eq!(
            encodings(&native[half..]),
            expected[half..],
            "{label}: the upper half is left for truncation"
        );

        let challenge = FoldChallenge::new::<B::Native>(&native_u).expect("GLV split");
        assert_eq!(challenge.scalar(), native_u);
        let mut precomputed = native_generators.clone();
        fold_generators_with::<B::Native>(&mut precomputed, &challenge);
        assert_lanes(
            &format!("{label} precomputed"),
            &expected[..half],
            &encodings(&precomputed[..half]),
        );

        let mut reference = native_generators.clone();
        fold_generators_reference::<B::Native>(&mut reference, &native_u);
        assert_lanes(
            &format!("{label} reference"),
            &expected[..half],
            &encodings(&reference[..half]),
        );
        expected
    });
}

/// Named challenges covering the GLV and wNAF edge cases.
fn special_challenges<B: CurveBridge>(rng: &mut ChaCha20Rng) -> Vec<(String, B::VScalar)> {
    let two = B::VScalar::from(2);
    let zeta = B::VScalar::ZETA;
    let mut out = vec![
        ("0".to_owned(), B::VScalar::ZERO),
        ("1".to_owned(), B::VScalar::ONE),
        ("-1".to_owned(), -B::VScalar::ONE),
        ("2".to_owned(), two),
        ("3".to_owned(), B::VScalar::from(3)),
        ("15".to_owned(), B::VScalar::from(15)),
        ("1/2".to_owned(), B::VScalar::TWO_INV),
        ("zeta".to_owned(), zeta),
        ("zeta^2".to_owned(), zeta.square()),
        ("-zeta".to_owned(), -zeta),
        ("1+zeta".to_owned(), B::VScalar::ONE + zeta),
        ("2^127".to_owned(), two.pow_vartime([127])),
        (
            "2^128-1".to_owned(),
            two.pow_vartime([128]) - B::VScalar::ONE,
        ),
        ("2^128".to_owned(), two.pow_vartime([128])),
        ("2^254".to_owned(), two.pow_vartime([254])),
    ];
    for index in 0..4 {
        out.push((format!("random{index}"), B::VScalar::random(&mut *rng)));
    }
    out
}

/// Distinct random generators (a random arithmetic progression).
fn random_generators<B: CurveBridge>(n: usize, rng: &mut ChaCha20Rng) -> Vec<B::Vendored> {
    let step = VendoredCurve::<B>::random(&mut *rng);
    let mut current = VendoredCurve::<B>::random(&mut *rng);
    let projective: Vec<VendoredCurve<B>> = (0..n)
        .map(|_| {
            current += step;
            current
        })
        .collect();
    let mut affine = vec![B::Vendored::identity(); n];
    VendoredCurve::<B>::batch_normalize(&projective, &mut affine);
    affine
}

/// Generators whose lanes cycle through every exceptional shape for `u`.
fn adversarial_generators<B: CurveBridge>(
    half: usize,
    u: B::VScalar,
    rng: &mut ChaCha20Rng,
) -> Vec<B::Vendored> {
    let mut hi = random_generators::<B>(half, rng);
    let random = random_generators::<B>(half, rng);
    let mut lo = Vec::with_capacity(half);
    for lane in 0..half {
        let h = hi[lane];
        lo.push(match lane % 8 {
            0 => h,
            1 => -h,
            2 => (h * u).to_affine(),
            3 => (-(h * u)).to_affine(),
            4 => B::Vendored::identity(),
            5 => {
                hi[lane] = B::Vendored::identity();
                random[lane]
            }
            6 => {
                hi[lane] = B::Vendored::identity();
                B::Vendored::identity()
            }
            _ => random[lane],
        });
    }
    lo.extend(hi);
    lo
}

/// Every special challenge on random generators of each size in `sizes`.
fn random_rounds<B: CurveBridge>(sizes: &[usize]) {
    let mut rng = data_rng(&format!("fold random {}", B::NAME));
    let challenges = special_challenges::<B>(&mut rng);
    for &n in sizes {
        let generators = random_generators::<B>(n, &mut rng);
        for (name, u) in &challenges {
            fold_round::<B>(
                &format!("{} random n={n} u={name}", B::NAME),
                &generators,
                *u,
            );
        }
    }
}

#[test]
fn random_rounds_match_the_vendored_collapse() {
    let sizes = [1, 2, 3, 4, 5, 9, 64, 65, 127, 128, 300];
    random_rounds::<Vesta>(&sizes);
    random_rounds::<Pallas>(&sizes);
}

#[test]
#[ignore = "folds of 1,000..=8,192 generators on both curves; run in release"]
fn random_rounds_match_the_vendored_collapse_large() {
    let sizes = [1000, 4096, 4098, 8192];
    random_rounds::<Vesta>(&sizes);
    random_rounds::<Pallas>(&sizes);
}

/// Exceptional lanes mixed into blocks of `halves` lanes, and one exceptional
/// lane (then one identity lane) at the middle of `single` ordinary lanes.
fn adversarial_rounds<B: CurveBridge>(halves: &[usize], single: usize) {
    let mut rng = data_rng(&format!("fold adversarial {}", B::NAME));
    let challenges = special_challenges::<B>(&mut rng);
    for &half in halves {
        for (name, u) in &challenges {
            let generators = adversarial_generators::<B>(half, *u, &mut rng);
            fold_round::<B>(
                &format!("{} adversarial half={half} u={name}", B::NAME),
                &generators,
                *u,
            );
        }
    }
    for (name, u) in challenges
        .iter()
        .filter(|(name, _)| name.starts_with("random"))
    {
        let mut generators = random_generators::<B>(2 * single, &mut rng);
        let lane = single / 2;
        generators[lane] = (generators[single + lane] * *u).to_affine();
        fold_round::<B>(
            &format!("{} single exceptional lane of {single} u={name}", B::NAME),
            &generators,
            *u,
        );
        generators[lane] = B::Vendored::identity();
        fold_round::<B>(
            &format!("{} single identity lane of {single} u={name}", B::NAME),
            &generators,
            *u,
        );
    }
}

#[test]
fn adversarial_rounds_match_the_vendored_collapse() {
    // 200 lanes form one block at 1 thread and several 64-lane blocks at 4
    // and 7 threads.
    adversarial_rounds::<Vesta>(&[8, 67], 200);
    adversarial_rounds::<Pallas>(&[8, 67], 200);
}

#[test]
#[ignore = "adversarial folds with full 2,048-lane blocks on both curves; run in release"]
fn adversarial_rounds_match_the_vendored_collapse_large() {
    adversarial_rounds::<Vesta>(&[2048, 3000], 3000);
    adversarial_rounds::<Pallas>(&[2048, 3000], 3000);
}

/// Absorbs the opening claim (commitment, point, value) as the vendored IPA
/// callers do before `create_proof` and `verify_proof`.
fn absorb<C, T>(transcript: &mut T, commitment: C, point: C::Scalar, value: C::Scalar)
where
    C: halo2_axiom::arithmetic::CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    T: Transcript<C, Challenge255<C>>,
{
    transcript
        .common_point(commitment)
        .expect("absorb commitment");
    transcript.common_scalar(point).expect("absorb point");
    transcript.common_scalar(value).expect("absorb value");
}

/// A real vendored IPA opening at size `k`, and the native fold over its
/// round challenges.
fn ipa_fold<B: CurveBridge>(k: u32, seed: u8)
where
    B::VScalar: FromUniformBytes<64>,
{
    let label = format!("{} ipa k{k} seed{seed}", B::NAME);
    let params = ParamsIPA::<B::Vendored>::new(k);
    let n = 1_usize << k;
    let mut rng = data_rng(&label);
    let domain = EvaluationDomain::<B::VScalar>::new(1, k);
    let polynomial = domain.coeff_from_vec((0..n).map(|_| B::VScalar::random(&mut rng)).collect());
    let blind = Blind(B::VScalar::random(&mut rng));
    let point = B::VScalar::random(&mut rng);
    let value = eval_polynomial(&polynomial, point);
    let commitment = params.commit(&polynomial, blind).to_affine();

    let mut writer = Recording::new(Blake2bWrite::<_, B::Vendored, Challenge255<_>>::init(
        Vec::new(),
    ));
    absorb(&mut writer, commitment, point, value);
    create_proof(
        &params,
        ChaCha20Rng::from_seed([seed; 32]),
        &mut writer,
        &polynomial,
        blind,
        point,
    )
    .expect("vendored IPA proof");
    let challenges = writer.challenges();
    let k_usize = usize::try_from(k).expect("small k");
    assert_eq!(
        challenges.len(),
        2 + k_usize,
        "{label}: xi, z and one challenge per round"
    );
    let proof = writer.into_inner().finalize();

    let mut reader = Recording::new(Blake2bRead::<_, B::Vendored, Challenge255<_>>::init(
        proof.as_slice(),
    ));
    absorb(&mut reader, commitment, point, value);
    let mut msm = params.empty_msm();
    msm.append_term(B::VScalar::ONE, commitment.into());
    let guard = verify_proof(&params, msm, &mut reader, point, value).expect("vendored verify");
    assert_eq!(
        reader.challenges(),
        challenges,
        "{label}: verifier replays the challenges"
    );
    let vendored_g0 = guard.compute_g().to_bytes();

    let rounds = &challenges[2..];
    let native_rounds = native_scalars::<B>(rounds);
    let native_g = native_affines::<B>(params.get_g());
    let native_g0 = same_on_each_pool(&label, |threads| {
        let mut native = native_g.clone();
        let mut len = n;
        for u in &native_rounds {
            len = fold_generators_vartime::<B::Native>(&mut native[..len], u);
        }
        assert_eq!(len, 1, "{label}: folded to one generator");
        let mut vendored = params.get_g().to_vec();
        let mut vendored_len = n;
        for u in rounds {
            generator_collapse(&mut vendored[..vendored_len], *u);
            vendored_len /= 2;
        }
        assert_eq!(
            vendored[0].to_bytes(),
            vendored_g0,
            "{label}: the vendored collapse folds to compute_g"
        );
        assert_eq!(
            native[0].to_bytes(),
            vendored_g0,
            "{label}: native G'_0 at {threads} threads"
        );
        native[0]
    });

    let (msm, accumulator) = guard.clone().use_g(vendored_affine::<B>(&native_g0));
    assert!(
        msm.check(),
        "{label}: the vendored verifier accepts the native G'_0"
    );
    assert_eq!(accumulator.g.to_bytes(), vendored_g0);
    assert_eq!(accumulator.u_packed, rounds, "{label}: packed challenges");
    assert!(
        guard.use_challenges().check(),
        "{label}: the proof verifies"
    );

    // A wrong folded generator is rejected.
    let wrong: NativeAffine<B> = (native_g0.to_curve().double()).to_affine();
    let mut msm = params.empty_msm();
    msm.append_term(B::VScalar::ONE, commitment.into());
    let mut reader = Blake2bRead::<_, B::Vendored, Challenge255<_>>::init(proof.as_slice());
    absorb(&mut reader, commitment, point, value);
    let guard = verify_proof(&params, msm, &mut reader, point, value).expect("vendored verify");
    let (msm, _) = guard.use_g(vendored_affine::<B>(&wrong));
    assert!(!msm.check(), "{label}: a wrong G'_0 is rejected");
}

#[test]
fn ipa_folds_match_vendored_proofs_k1_to_k10() {
    for k in 1..=10 {
        for seed in [7, 61] {
            ipa_fold::<Vesta>(k, seed);
            ipa_fold::<Pallas>(k, seed);
        }
    }
}

#[test]
#[ignore = "k = 11..=16 IPA proofs on both curves; run in release"]
fn ipa_folds_match_vendored_proofs_k11_to_k16() {
    for k in 11..=16 {
        ipa_fold::<Vesta>(k, 7);
        ipa_fold::<Pallas>(k, 7);
    }
}

#[test]
fn adversarial_generators_have_the_intended_shapes() {
    let mut rng = data_rng("fold shapes");
    let u = <Vesta as CurveBridge>::VScalar::from(5);
    let g = adversarial_generators::<Vesta>(8, u, &mut rng);
    let (lo, hi) = g.split_at(8);
    assert_eq!(lo[0], hi[0]);
    assert_eq!(lo[1], -hi[1]);
    assert_eq!(lo[2], (hi[2] * u).to_affine());
    assert_eq!(lo[3], (-(hi[3] * u)).to_affine());
    assert!(bool::from(lo[4].is_identity()));
    assert!(bool::from(hi[5].is_identity()) && !bool::from(lo[5].is_identity()));
    assert!(bool::from(hi[6].is_identity()) && bool::from(lo[6].is_identity()));
    assert_eq!(
        encodings(&[<Vesta as CurveBridge>::Vendored::identity()]),
        vec![[0; 32]]
    );
    let challenges = special_challenges::<Pallas>(&mut rng);
    assert!(challenges.iter().any(|(name, value)| name == "zeta"
        && *value == <Pallas as CurveBridge>::VScalar::ZETA));
    assert_eq!(
        challenges[0].1.to_repr(),
        <Pallas as CurveBridge>::VScalar::ZERO.to_repr()
    );
}

#[test]
#[should_panic(expected = "lane 1 differs")]
fn assert_lanes_names_the_first_difference() {
    assert_lanes("test", &[[0; 32], [1; 32]], &[[0; 32], [2; 32]]);
}
