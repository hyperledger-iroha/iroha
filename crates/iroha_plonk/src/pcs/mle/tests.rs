//! The multilinear helpers against brute-force definitions, and the batch
//! opening: honest openings verify and decide on both curves, every changed
//! statement or proof field is refused, and a false value only passes the
//! equation with a generator claim that does not decide.

use core::marker::PhantomData;

use ff::{Field, PrimeField};
use group::{Curve, CurveAffine, Group};
use iroha_pasta::{
    Ep, Eq, Fq, PastaAffine, PastaCurve, PastaField,
    msm::{MemoryBudget, msm_naive},
    poseidon::Sponge,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use super::*;
use crate::{
    cs::Rotation,
    pcs::ipa::{
        GeneratorClaim, IpaError, PinnedParams,
        commit::{self, Secrecy},
        fold_scalars,
    },
    transcript::pipa_r::scalar_elements,
};

const BUDGET: MemoryBudget = MemoryBudget::DEFAULT;

/// The folding decider's transcript encodings (iroha_fold `transcript.rs`):
/// scalars as PIPA-R base-field elements, points as `(x, y)`, full challenges
/// mapped by their limbs and 128-bit challenges as the low limb.
#[derive(Clone, Debug)]
struct Transcript<C: PastaCurve> {
    sponge: Sponge<C::Base>,
    marker: PhantomData<C>,
}

impl<C: PastaCurve> Transcript<C> {
    fn new(domain: u64) -> Self {
        let mut sponge = Sponge::new();
        sponge.update(&[C::Base::from(domain)]);
        Self {
            sponge,
            marker: PhantomData,
        }
    }
}

impl<C: PastaCurve> MleTranscript<C> for Transcript<C> {
    fn absorb_u64(&mut self, value: u64) {
        self.sponge.update(&[C::Base::from(value)]);
    }

    fn absorb_scalars(&mut self, values: &[C::ScalarExt]) {
        for value in values {
            self.sponge.update(&scalar_elements::<C>(value));
        }
    }

    fn absorb_point(&mut self, point: &C::AffineExt) {
        self.sponge.update(&[point.x(), point.y()]);
    }

    fn challenge(&mut self) -> C::ScalarExt {
        C::ScalarExt::from_raw_reduced(self.sponge.squeeze().to_canonical_limbs())
    }

    fn challenge_128(&mut self) -> C::ScalarExt {
        let limbs = self.sponge.squeeze().to_canonical_limbs();
        C::ScalarExt::from_u128(u128::from(limbs[0]) | (u128::from(limbs[1]) << 64))
    }
}

fn random<F: Field>(rng: &mut ChaCha20Rng, len: usize) -> Vec<F> {
    (0..len).map(|_| F::random(&mut *rng)).collect()
}

/// The little-endian bits of `row` as a point.
fn bits<F: Field>(row: usize, k: usize) -> Vec<F> {
    (0..k)
        .map(|t| if (row >> t) & 1 == 1 { F::ONE } else { F::ZERO })
        .collect()
}

fn eq_naive<F: Field>(x: &[F], row: usize) -> F {
    x.iter().enumerate().fold(F::ONE, |acc, (t, x)| {
        acc * if (row >> t) & 1 == 1 { *x } else { F::ONE - x }
    })
}

fn inner<F: Field>(a: &[F], b: &[F]) -> F {
    a.iter().zip(b).fold(F::ZERO, |acc, (a, b)| acc + *a * b)
}

fn target(row: usize, rotation: i32, n: usize) -> usize {
    let n = i64::try_from(n).expect("small n");
    usize::try_from((i64::try_from(row).expect("small row") + i64::from(rotation)).rem_euclid(n))
        .expect("in range")
}

#[test]
fn eq_table_is_the_product_formula_and_selects_rows() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    for k in 0..=5 {
        let n = 1 << k;
        let x: Vec<Fq> = random(&mut rng, k);
        let table = eq_table(&x);
        assert_eq!(table.len(), n);
        for (row, value) in table.iter().enumerate() {
            assert_eq!(*value, eq_naive(&x, row), "k {k} row {row}");
        }
        assert_eq!(table.iter().fold(Fq::ZERO, |acc, v| acc + v), Fq::ONE);
        for row in 0..n {
            for (other, value) in eq_table(&bits::<Fq>(row, k)).iter().enumerate() {
                assert_eq!(*value, Fq::from(u64::from(other == row)));
            }
        }
    }
}

#[test]
fn evaluate_is_the_multilinear_extension() {
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    for k in 0..=5 {
        let n = 1 << k;
        let w: Vec<Fq> = random(&mut rng, n);
        let x: Vec<Fq> = random(&mut rng, k);
        assert_eq!(evaluate(&w, &x), Some(inner(&eq_table(&x), &w)));
        for (row, value) in w.iter().enumerate() {
            assert_eq!(evaluate(&w, &bits(row, k)), Some(*value));
        }
        // Affine in each coordinate.
        for t in 0..k {
            let mut low = x.clone();
            let mut high = x.clone();
            low[t] = Fq::ZERO;
            high[t] = Fq::ONE;
            let (low, high) = (evaluate(&w, &low).unwrap(), evaluate(&w, &high).unwrap());
            assert_eq!(evaluate(&w, &x), Some(low + (high - low) * x[t]));
        }
    }
    assert_eq!(evaluate(&[Fq::ONE; 3], &[Fq::ONE]), None);
    assert_eq!(evaluate(&[Fq::ONE; 2], &[]), None);
}

#[test]
fn shifted_eq_is_the_extension_of_rotated_rows() {
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    for k in 0..=5 {
        let n = 1_usize << k;
        let span = 2 * i32::try_from(n).expect("small n") + 1;
        let rho: Vec<Fq> = random(&mut rng, k);
        let sigma: Vec<Fq> = random(&mut rng, k);
        let w: Vec<Fq> = random(&mut rng, n);
        let (rho_eq, sigma_eq) = (eq_table(&rho), eq_table(&sigma));
        let rotations = (-span..=span).chain([
            i32::MIN,
            i32::MIN + 1,
            i32::MAX,
            Rotation::prev().0,
            Rotation::next().0,
        ]);
        for rotation in rotations {
            // The definition, at random points.
            let expected = (0..n).fold(Fq::ZERO, |acc, j| {
                acc + rho_eq[j] * sigma_eq[target(j, rotation, n)]
            });
            assert_eq!(
                shifted_eq(&rho, &sigma, rotation),
                Some(expected),
                "k {k} r {rotation}"
            );
            // At boolean points it is iroha_plonk's row map (j + r) mod n.
            for j in 0..n {
                for i in 0..n {
                    assert_eq!(
                        shifted_eq(&bits::<Fq>(j, k), &bits(i, k), rotation),
                        Some(Fq::from(u64::from(i == target(j, rotation, n)))),
                        "k {k} r {rotation} rows {j} -> {i}"
                    );
                }
            }
            // The decider's claim: the column read at Rotation(r), extended at
            // rho, is sum_y w(y) shifted_eq(rho, y, r).
            let rotated: Vec<Fq> = (0..n).map(|j| w[target(j, rotation, n)]).collect();
            let table = shifted_eq_table(&rho, rotation).expect("small k");
            let by_points = (0..n).fold(Fq::ZERO, |acc, y| {
                acc + w[y] * shifted_eq(&rho, &bits(y, k), rotation).unwrap()
            });
            assert_eq!(evaluate(&rotated, &rho), Some(inner(&w, &table)));
            assert_eq!(by_points, inner(&w, &table));
        }
    }
    assert_eq!(shifted_eq(&[Fq::ONE], &[], 0), None);
    assert_eq!(
        shifted_eq(&[Fq::ONE; MAX_K + 1], &[Fq::ONE; MAX_K + 1], 0),
        None
    );
}

#[test]
fn folded_eq_matches_the_folded_generator_scalars() {
    let mut rng = ChaCha20Rng::seed_from_u64(4);
    for k in 0..=6 {
        let x: Vec<Fq> = random(&mut rng, k);
        let u: Vec<Fq> = random(&mut rng, k);
        let s = fold_scalars(&u, Fq::ONE);
        assert_eq!(folded_eq(&x, &u), Some(inner(&s, &eq_table(&x))), "k {k}");
    }
    assert_eq!(folded_eq(&[Fq::ONE], &[]), None);
}

/// Columns, blinds, a point and the honest claims.
struct Batch<C: PastaCurve> {
    point: Vec<C::ScalarExt>,
    columns: Vec<Vec<C::ScalarExt>>,
    blinds: Vec<C::ScalarExt>,
    claims: Vec<MleClaim<C>>,
}

impl<C: PastaCurve> Batch<C> {
    fn new(
        params: &iroha_pasta::params::ParamsIpa<C>,
        rng: &mut ChaCha20Rng,
        k: usize,
        m: usize,
    ) -> Self {
        let n = 1 << k;
        let point = random(rng, k);
        let columns: Vec<Vec<C::ScalarExt>> = (0..m).map(|_| random(rng, n)).collect();
        let blinds = random(rng, m);
        let claims = columns
            .iter()
            .zip(&blinds)
            .map(|(column, blind)| {
                let mut scalars = column.clone();
                scalars.push(*blind);
                let mut bases = params.g()[..n].to_vec();
                bases.push(params.w());
                MleClaim {
                    commitment: msm_naive::<C>(&scalars, &bases).to_affine(),
                    value: evaluate(column, &point).expect("2^k values"),
                }
            })
            .collect();
        Self {
            point,
            columns,
            blinds,
            claims,
        }
    }

    fn columns(&self) -> Vec<&[C::ScalarExt]> {
        self.columns.iter().map(Vec::as_slice).collect()
    }

    fn open(
        &self,
        params: &iroha_pasta::params::ParamsIpa<C>,
        domain: u64,
    ) -> Result<(BatchOpening<C>, GeneratorClaim<C>, Transcript<C>), IpaError> {
        let mut transcript = Transcript::new(domain);
        open_batch(
            params,
            &mut transcript,
            &self.point,
            &self.claims,
            &self.columns(),
            &self.blinds,
            BUDGET,
        )
        .map(|(opening, claim)| (opening, claim, transcript))
    }
}

fn verify<C: PastaCurve>(
    params: &iroha_pasta::params::ParamsIpa<C>,
    point: &[C::ScalarExt],
    claims: &[MleClaim<C>],
    opening: &BatchOpening<C>,
    domain: u64,
) -> Result<(GeneratorClaim<C>, Transcript<C>), IpaError> {
    let mut transcript = Transcript::new(domain);
    verify_batch(params, &mut transcript, point, claims, opening, BUDGET)
        .map(|claim| (claim, transcript))
}

fn honest<C: PastaCurve>(seed: u64) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let pinned = PinnedParams::<C>::derive(5).expect("params");
    let params = pinned.params();
    for k in [0, 1, 3, 5] {
        for m in [1, 2, 5] {
            let batch = Batch::<C>::new(params, &mut rng, k, m);
            // The crate's monomial-basis commit produces these commitments.
            for ((column, blind), claim) in
                batch.columns.iter().zip(&batch.blinds).zip(&batch.claims)
            {
                let committed = commit::commit(params, column, blind, Secrecy::Secret, BUDGET)
                    .expect("commit")
                    .to_affine();
                assert_eq!(committed, claim.commitment);
            }
            let (opening, claim, mut proved) = batch.open(params, 11).expect("opening");
            assert_eq!(opening.rounds.len(), k);
            let (verified, mut checked) =
                verify(params, &batch.point, &batch.claims, &opening, 11).expect("verifies");
            assert_eq!(verified, claim, "k {k} m {m}");
            assert_eq!(claim.decide(&pinned, BUDGET), Ok(()), "k {k} m {m}");
            // Prover and verifier leave the caller's transcript in one state.
            assert_eq!(proved.challenge(), checked.challenge());
        }
    }
}

#[test]
fn openings_verify_and_their_generator_claims_decide_on_both_curves() {
    honest::<Ep>(5);
    honest::<Eq>(6);
}

#[test]
fn changed_statements_and_proof_fields_are_refused() {
    let mut rng = ChaCha20Rng::seed_from_u64(7);
    let pinned = PinnedParams::<Ep>::derive(4).expect("params");
    let params = pinned.params();
    let batch = Batch::<Ep>::new(params, &mut rng, 4, 3);
    let (opening, _, _) = batch.open(params, 3).expect("opening");
    let refused = |claims: &[MleClaim<Ep>], point: &[Fq], opening: &BatchOpening<Ep>| {
        verify(params, point, claims, opening, 3).is_err()
    };
    assert!(!refused(&batch.claims, &batch.point, &opening));

    for c in 0..3 {
        let mut claims = batch.claims.clone();
        claims[c].value += Fq::ONE;
        assert!(refused(&claims, &batch.point, &opening), "value {c}");
    }
    let mut claims = batch.claims.clone();
    claims.swap(0, 2);
    assert!(refused(&claims, &batch.point, &opening), "claim order");
    let mut claims = batch.claims.clone();
    let first = claims[0].commitment;
    claims[0].commitment = claims[1].commitment;
    claims[1].commitment = first;
    assert!(
        refused(&claims, &batch.point, &opening),
        "swapped commitments"
    );
    // The same column and blind committed on the Lagrange basis.
    let mut claims = batch.claims.clone();
    claims[1].commitment = (msm_naive::<Ep>(&batch.columns[1], params.g_lagrange())
        + params.w() * batch.blinds[1])
        .to_affine();
    assert!(
        refused(&claims, &batch.point, &opening),
        "Lagrange commitment"
    );
    for t in 0..4 {
        let mut point = batch.point.clone();
        point[t] += Fq::ONE;
        assert!(refused(&batch.claims, &point, &opening), "point {t}");
    }

    let w = params.w();
    let mut changed = opening.clone();
    changed.blind += Fq::ONE;
    assert!(refused(&batch.claims, &batch.point, &changed), "blind");
    for round in 0..4 {
        let mut changed = opening.clone();
        changed.rounds[round].0 = (changed.rounds[round].0 + w).to_affine();
        assert!(refused(&batch.claims, &batch.point, &changed), "L {round}");
        let mut changed = opening.clone();
        changed.rounds[round].1 = (changed.rounds[round].1 + w).to_affine();
        assert!(refused(&batch.claims, &batch.point, &changed), "R {round}");
    }
    let mut changed = opening.clone();
    changed.scalar += Fq::ONE;
    assert!(refused(&batch.claims, &batch.point, &changed), "scalar");
    let mut changed = opening.clone();
    changed.folded_generator = (changed.folded_generator + w).to_affine();
    assert!(
        refused(&batch.claims, &batch.point, &changed),
        "folded generator"
    );
    let mut changed = opening.clone();
    changed.rounds.pop();
    assert_eq!(
        verify(params, &batch.point, &batch.claims, &changed, 3).map(|_| ()),
        Err(IpaError::LengthMismatch {
            expected: 4,
            actual: 3
        })
    );
    assert!(
        verify(params, &batch.point, &batch.claims, &opening, 4).is_err(),
        "transcript prefix"
    );
    assert_eq!(
        verify(params, &[Fq::ONE; 5], &batch.claims, &opening, 3).map(|_| ()),
        Err(IpaError::ParamsTooSmall {
            needed: 5,
            available: 4
        })
    );
    assert_eq!(
        verify(params, &batch.point, &[], &opening, 3).map(|_| ()),
        Err(IpaError::LengthMismatch {
            expected: 1,
            actual: 0
        })
    );
}

/// A prover that claims a false value, writes arbitrary rounds and solves the
/// opening equation for `G'_0` passes [`verify_batch`]; its generator claim
/// does not decide. Deciding the claim is what makes the opening sound.
#[test]
fn a_false_value_passes_the_equation_only_with_an_undecidable_generator_claim() {
    let mut rng = ChaCha20Rng::seed_from_u64(8);
    let pinned = PinnedParams::<Ep>::derive(3).expect("params");
    let params = pinned.params();
    let batch = Batch::<Ep>::new(params, &mut rng, 3, 2);
    let mut claims = batch.claims.clone();
    claims[1].value += Fq::ONE;

    let mut transcript = Transcript::<Ep>::new(9);
    transcript.absorb_u64(TAG_BATCH_OPENING);
    transcript.absorb_u64(3);
    transcript.absorb_u64(2);
    transcript.absorb_scalars(&batch.point);
    for claim in &claims {
        transcript.absorb_point(&claim.commitment);
        transcript.absorb_scalars(&[claim.value]);
    }
    let alpha = transcript.challenge_128();
    let blind = batch.blinds[0] + batch.blinds[1] * alpha;
    transcript.absorb_scalars(&[blind]);
    let z = transcript.challenge();
    let mut rounds = Vec::new();
    let mut challenges = Vec::new();
    for _ in 0..3 {
        let l = Ep::random(&mut rng).to_affine();
        let r = Ep::random(&mut rng).to_affine();
        transcript.absorb_point(&l);
        transcript.absorb_point(&r);
        rounds.push((l, r));
        challenges.push(transcript.challenge());
    }
    let scalar = Fq::from(5);
    let value = claims[0].value + claims[1].value * alpha;
    let b = folded_eq(&batch.point, &challenges).expect("k challenges");
    let mut lhs = claims[0].commitment.to_curve() + claims[1].commitment * alpha
        - params.w() * blind
        + params.u() * ((value - scalar * b) * z);
    for ((l, r), u) in rounds.iter().zip(&challenges) {
        lhs += *l * u.invert().unwrap() + *r * u;
    }
    let opening = BatchOpening {
        blind,
        rounds,
        folded_generator: (lhs * scalar.invert().unwrap()).to_affine(),
        scalar,
    };
    let (claim, _) =
        verify(params, &batch.point, &claims, &opening, 9).expect("the solved equation holds");
    assert_eq!(claim.decide(&pinned, BUDGET), Err(IpaError::OpeningFailed));
}

#[test]
fn the_prover_refuses_false_values_and_bad_shapes() {
    let mut rng = ChaCha20Rng::seed_from_u64(10);
    let pinned = PinnedParams::<Ep>::derive(3).expect("params");
    let params = pinned.params();
    let batch = Batch::<Ep>::new(params, &mut rng, 3, 2);
    let open = |point: &[Fq], claims: &[MleClaim<Ep>], columns: &[&[Fq]], blinds: &[Fq]| {
        open_batch(
            params,
            &mut Transcript::<Ep>::new(1),
            point,
            claims,
            columns,
            blinds,
            BUDGET,
        )
        .map(|_| ())
    };
    let columns = batch.columns();
    assert_eq!(
        open(&batch.point, &batch.claims, &columns, &batch.blinds),
        Ok(())
    );
    let mut claims = batch.claims.clone();
    claims[0].value += Fq::ONE;
    assert_eq!(
        open(&batch.point, &claims, &columns, &batch.blinds),
        Err(IpaError::OpeningFailed)
    );
    assert_eq!(
        open(&batch.point, &[], &[], &[]),
        Err(IpaError::LengthMismatch {
            expected: 1,
            actual: 0
        })
    );
    assert_eq!(
        open(&batch.point, &batch.claims, &columns[..1], &batch.blinds),
        Err(IpaError::LengthMismatch {
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(
        open(&batch.point, &batch.claims, &columns, &batch.blinds[..1]),
        Err(IpaError::LengthMismatch {
            expected: 2,
            actual: 1
        })
    );
    let short = [&batch.columns[0][..4], columns[1]];
    assert_eq!(
        open(&batch.point, &batch.claims, &short, &batch.blinds),
        Err(IpaError::LengthMismatch {
            expected: 8,
            actual: 4
        })
    );
    assert_eq!(
        open(&[Fq::ONE; 4], &batch.claims, &columns, &batch.blinds),
        Err(IpaError::ParamsTooSmall {
            needed: 4,
            available: 3
        })
    );
}

/// One transcript call, by kind and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    U64(u64),
    Scalars(usize),
    Point,
    Challenge,
    Challenge128,
}

/// A transcript that records its calls.
struct Recording<C: PastaCurve> {
    inner: Transcript<C>,
    calls: Vec<Call>,
}

impl<C: PastaCurve> MleTranscript<C> for Recording<C> {
    fn absorb_u64(&mut self, value: u64) {
        self.calls.push(Call::U64(value));
        self.inner.absorb_u64(value);
    }

    fn absorb_scalars(&mut self, values: &[C::ScalarExt]) {
        self.calls.push(Call::Scalars(values.len()));
        self.inner.absorb_scalars(values);
    }

    fn absorb_point(&mut self, point: &C::AffineExt) {
        self.calls.push(Call::Point);
        self.inner.absorb_point(point);
    }

    fn challenge(&mut self) -> C::ScalarExt {
        self.calls.push(Call::Challenge);
        self.inner.challenge()
    }

    fn challenge_128(&mut self) -> C::ScalarExt {
        self.calls.push(Call::Challenge128);
        self.inner.challenge_128()
    }
}

/// The exact transcript schedule an in-circuit verifier must replay: the
/// statement (tag, `k`, count, point, every commitment and value) before the
/// batching coefficients, `rho*` before `z`, each round's `L`, `R` before its
/// challenge, then `G'_0` and `c`.
#[test]
fn prover_and_verifier_follow_one_transcript_schedule() {
    let mut rng = ChaCha20Rng::seed_from_u64(12);
    let pinned = PinnedParams::<Eq>::derive(3).expect("params");
    let params = pinned.params();
    let (k, m) = (3, 4);
    let batch = Batch::<Eq>::new(params, &mut rng, k, m);
    let mut expected = vec![
        Call::U64(TAG_BATCH_OPENING),
        Call::U64(3),
        Call::U64(4),
        Call::Scalars(k),
    ];
    for _ in 0..m {
        expected.extend([Call::Point, Call::Scalars(1)]);
    }
    expected.extend([Call::Challenge128; 3]);
    expected.extend([Call::Scalars(1), Call::Challenge]);
    for _ in 0..k {
        expected.extend([Call::Point, Call::Point, Call::Challenge]);
    }
    expected.extend([Call::Point, Call::Scalars(1)]);

    let mut proving = Recording {
        inner: Transcript::<Eq>::new(13),
        calls: Vec::new(),
    };
    let (opening, _) = open_batch(
        params,
        &mut proving,
        &batch.point,
        &batch.claims,
        &batch.columns(),
        &batch.blinds,
        BUDGET,
    )
    .expect("opening");
    let mut verifying = Recording {
        inner: Transcript::<Eq>::new(13),
        calls: Vec::new(),
    };
    let claim = verify_batch(
        params,
        &mut verifying,
        &batch.point,
        &batch.claims,
        &opening,
        BUDGET,
    )
    .expect("verifies");
    assert_eq!(claim.decide(&pinned, BUDGET), Ok(()));
    assert_eq!(proving.calls, expected);
    assert_eq!(verifying.calls, expected);
}
