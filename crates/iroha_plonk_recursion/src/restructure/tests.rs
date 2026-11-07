//! Restructured claims verify, decide and join a PIPA-AS fold on both curves;
//! false statements and changed statements, coins and bodies are refused; a
//! false value passes the equation only with a claim that does not decide.

use super::*;
use crate::{FoldPurpose, create_fold};
use ff::Field;
use group::Curve;
use iroha_pasta::{Ep, Eq, Fq, PastaCurve, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    pcs::{
        ipa::{
            PinnedParams,
            commit::{self, Secrecy},
        },
        mle::evaluate,
    },
    transcript::TranscriptWriter,
};

/// Public deterministic data for algebraic regressions, never producer entropy.
fn fixture_tape<C: PastaCurve>(start: u64) -> FoldTape<C> {
    let mut value = start;
    FoldTape::try_fill::<()>(|coin| {
        *coin = C::ScalarExt::from(value);
        value += 1;
        Ok(())
    })
    .expect("public fixture tape")
}

/// SplitMix64: public pseudo-random test data (this crate has no RNG
/// dependency), never producer entropy.
struct Data(u64);

impl Data {
    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn scalar<F: PastaField>(&mut self) -> F {
        F::from_raw_reduced(core::array::from_fn(|_| self.word()))
    }

    fn point<C: PastaCurve>(&mut self) -> C::AffineExt {
        (C::generator() * self.scalar::<C::ScalarExt>()).to_affine()
    }
}

/// A random relaxed tensor vector, its unblinded commitment, a point and the
/// value of its multilinear extension there.
struct Case<C: PastaCurve> {
    params: PinnedParams<C>,
    witness: Vec<C::ScalarExt>,
    statement: RestructureStatement<C>,
}

fn case<C: PastaCurve>(seed: u64) -> Case<C> {
    let params = PinnedParams::<C>::derive(K_U32).expect("pinned params");
    let mut data = Data(seed);
    let witness: Vec<C::ScalarExt> = (0..GENERATORS).map(|_| data.scalar()).collect();
    let point: [C::ScalarExt; K] = core::array::from_fn(|_| data.scalar());
    let commitment = commit::commit(
        params.params(),
        &witness,
        &C::ScalarExt::ZERO,
        Secrecy::Secret,
        MemoryBudget::DEFAULT,
    )
    .expect("commit")
    .to_affine();
    let value = evaluate(&witness, &point).expect("2^16 entries");
    Case {
        params,
        witness,
        statement: RestructureStatement {
            commitment,
            point,
            value,
        },
    }
}

fn honest<C: PastaCurve>(seed: u64)
where
    C::Base: PoseidonField,
{
    let case = case::<C>(seed);
    let config = FoldConfig::default();
    let (proof, output) = create_restructure(
        &case.params,
        &case.statement,
        &case.witness,
        fixture_tape(seed),
        &config,
    )
    .expect("restructure");
    assert_eq!(
        verify_restructure(&case.params, &case.statement, &proof, &config),
        Ok(output.clone())
    );
    output
        .decide(&case.params, config.kernel_budget)
        .expect("the structured claim decides");
    // The structured claim joins a PIPA-AS fold like any other claim.
    let purpose = if C::CURVE_ID == "pallas" {
        FoldPurpose::AInitial
    } else {
        FoldPurpose::SigmaBatch
    };
    let (_, folded) = create_fold(
        &case.params,
        &[output.as_input()],
        fixture_tape(seed + 100),
        purpose,
        &config,
    )
    .expect("fold");
    folded
        .decide(&case.params, config.kernel_budget)
        .expect("the fold decides");
}

#[test]
fn restructured_claims_verify_decide_and_fold_on_both_curves() {
    honest::<Ep>(1);
    honest::<Eq>(2);
}

#[test]
fn false_statements_and_changed_fields_are_refused() {
    let case = case::<Ep>(3);
    let config = FoldConfig::default();
    let shifted =
        |point: &<Ep as PastaCurve>::AffineExt| (*point + case.params.params().w()).to_affine();

    // A statement the witness does not have is caught before returning.
    let mut wrong = case.statement.clone();
    wrong.value += Fq::ONE;
    assert_eq!(
        create_restructure(
            &case.params,
            &wrong,
            &case.witness,
            fixture_tape(9),
            &config
        )
        .map(|_| ()),
        Err(Error::FoldEquation)
    );
    let mut wrong = case.statement.clone();
    wrong.commitment = shifted(&wrong.commitment);
    assert_eq!(
        create_restructure(
            &case.params,
            &wrong,
            &case.witness,
            fixture_tape(9),
            &config
        )
        .map(|_| ()),
        Err(Error::FoldEquation)
    );
    assert_eq!(
        create_restructure(
            &case.params,
            &case.statement,
            &case.witness[1..],
            fixture_tape(9),
            &config
        )
        .map(|_| ()),
        Err(Error::Length {
            expected: GENERATORS,
            actual: GENERATORS - 1
        })
    );

    let (proof, _) = create_restructure(
        &case.params,
        &case.statement,
        &case.witness,
        fixture_tape(9),
        &config,
    )
    .expect("restructure");
    let refused = |statement: &RestructureStatement<Ep>, proof: &FoldWitness<Ep>| {
        verify_restructure(&case.params, statement, proof, &config).is_err()
    };
    assert!(!refused(&case.statement, &proof));
    let mut changed = case.statement.clone();
    changed.value += Fq::ONE;
    assert!(refused(&changed, &proof), "value");
    let mut changed = case.statement.clone();
    changed.commitment = shifted(&changed.commitment);
    assert!(refused(&changed, &proof), "commitment");
    for t in [0, 7, K - 1] {
        let mut changed = case.statement.clone();
        changed.point[t] += Fq::ONE;
        assert!(refused(&changed, &proof), "point {t}");
    }

    // One bit of c, rho_D, the first and the last mask: still canonical, and
    // refused. One bit of D, an L, an R or G': refused or no longer a point.
    let bytes = proof.to_private_bytes();
    let tampered = |offset: usize| {
        let mut tampered = bytes.0.to_vec();
        tampered[offset] ^= 1;
        FoldWitness::<Ep>::from_private_bytes(&tampered)
    };
    let scalars = [
        32 * (2 * K + 1),
        FOLD_BODY_BYTES,
        FOLD_BODY_BYTES + 32,
        FOLD_WITNESS_BYTES - 32,
    ];
    for offset in scalars {
        let changed = tampered(offset).expect("a canonical scalar");
        assert!(refused(&case.statement, &changed), "offset {offset}");
    }
    for offset in [0, 32, 64, 32 * 2 * K, 32 * (2 * K + 2)] {
        if let Ok(changed) = tampered(offset) {
            assert!(refused(&case.statement, &changed), "offset {offset}");
        }
    }
}

/// A prover that states a false value, binds it honestly in `D`, writes random
/// rounds and solves the equation for `G'` passes [`verify_restructure`]; the
/// returned claim does not decide, which is what makes the opening sound.
#[test]
fn a_false_value_passes_the_equation_only_with_a_claim_that_does_not_decide() {
    let case = case::<Ep>(4);
    let config = FoldConfig::default();
    let params = &case.params;
    let mut statement = case.statement.clone();
    statement.value += Fq::ONE;
    let tape = fixture_tape::<Ep>(11);
    let mut data = Data(5);

    let digest = input_digest(params, &statement, &tape);
    let d = opening(params, digest, *tape.rho()).to_affine();
    let mut writer = TranscriptWriter::<Ep, _>::new(BasePoseidonHash::with_domain(
        iroha_plonk::contract::PIPA_AS_FOLD_DOMAIN,
    ));
    prelude(&mut writer, params).expect("context");
    writer.write_point(&d).expect("D");
    let zeta = writer.squeeze_challenge();
    let mut rounds = Vec::new();
    let mut challenges = [Fq::ZERO; K];
    for challenge in &mut challenges {
        let left = data.point::<Ep>();
        let right = data.point::<Ep>();
        writer.write_point(&left).expect("L");
        writer.write_point(&right).expect("R");
        *challenge = nonzero_challenge(writer.squeeze_challenge());
        rounds.push((left, right));
    }
    let scalar = Fq::from(3);
    let folded = folded_eq(&statement.point, &challenges).expect("K challenges");
    let mut gamma = Fq::ZERO;
    let mut lhs = statement.commitment.to_curve()
        + params.params().u() * ((statement.value - scalar * folded) * zeta);
    for (round, ((left, right), u)) in rounds.iter().zip(&challenges).enumerate() {
        let inverse = u.invert().unwrap();
        lhs += *left * inverse + *right * u;
        gamma += tape.masks()[2 * round] * inverse + tape.masks()[2 * round + 1] * u;
    }
    lhs -= params.params().w() * gamma;
    let generator = (lhs * scalar.invert().unwrap()).to_affine();
    writer.write_scalar(&scalar);
    writer.append_unabsorbed_generator(&generator);
    let mut bytes = writer.finish();
    bytes.extend_from_slice(&tape.to_private_bytes());
    let proof = FoldWitness::<Ep>::from_private_bytes(&bytes).expect("canonical witness");

    let claim =
        verify_restructure(params, &statement, &proof, &config).expect("the solved equation holds");
    assert!(claim.decide(params, config.kernel_budget).is_err());
}
