//! The in-circuit bound batch opening against the native
//! `iroha_plonk::pcs::mle::verify_batch_bound`, on both curves: a transcript
//! binds the point and the claims as a decider does, the opening continues it,
//! and the circuit exposes the claim's `G'_0` and the finished transcript's
//! digest.

use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        mle::{OpeningKey, assign_batch_opening, verify_batch_bound, verify_batch_bound_group},
        transcript as ct,
    },
    transcript::Transcript,
};
use iroha_pasta::{Ep, Eq, PastaAffine, PastaCurve, msm::MemoryBudget};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
    pcs::{
        ipa::{
            PinnedParams,
            commit::{self, Secrecy},
        },
        mle::{self, BatchOpening, MleClaim},
    },
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const DOMAIN: [u8; 32] = *b"iroha_fold mle opening circuit!!";
const TAG: u64 = u64::from_le_bytes(*b"testmle1");
const CIRCUIT_K: u32 = 16;
const BUDGET: MemoryBudget = MemoryBudget::DEFAULT;

struct Inputs<C: PastaCurve> {
    point: Vec<C::ScalarExt>,
    claims: Vec<MleClaim<C>>,
    opening: BatchOpening<C>,
}

impl<C: PastaCurve> Clone for Inputs<C> {
    fn clone(&self) -> Self {
        Self {
            point: self.point.clone(),
            claims: self.claims.clone(),
            opening: self.opening.clone(),
        }
    }
}

struct OpeningCircuit<C: PastaCurve> {
    key: OpeningKey<C>,
    k: usize,
    m: usize,
    inputs: Option<Inputs<C>>,
}

#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    fold: FoldVerifierConfig<C>,
    instance: Column<InstanceColumn>,
}

fn value_of<T: Clone>(value: Option<&T>) -> Value<T> {
    value.map_or_else(Value::unknown, |v| Value::known(v.clone()))
}

impl<C: PastaCurve> Circuit<C::Base> for OpeningCircuit<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            key: self.key.clone(),
            k: self.k,
            m: self.m,
            inputs: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(
        &self,
        config: Config<C>,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let inputs = self.inputs.as_ref();
        let outputs = layouter.assign_region(
            || "bound opening",
            |mut region| {
                let mut lanes = chips.lanes();
                let mut point = Vec::with_capacity(self.k);
                for i in 0..self.k {
                    point.push(
                        lanes.assign_scalar(&mut region, value_of(inputs.map(|x| &x.point[i])))?,
                    );
                }
                let mut claims = Vec::with_capacity(self.m);
                for c in 0..self.m {
                    let commitment = lanes.assign_point(
                        &mut region,
                        value_of(inputs.map(|x| C::from(x.claims[c].commitment)).as_ref()),
                    )?;
                    let value = lanes
                        .assign_scalar(&mut region, value_of(inputs.map(|x| &x.claims[c].value)))?;
                    claims.push((commitment, value));
                }
                let opening = assign_batch_opening(
                    &mut lanes,
                    &mut region,
                    self.k,
                    inputs.map(|x| &x.opening),
                )?;
                // The caller's transcript binds the point and every claim.
                ct::start(&mut lanes, TAG, &DOMAIN)?;
                ct::absorb_scalars(&mut lanes, &mut region, &point)?;
                for (commitment, value) in &claims {
                    ct::absorb_point(&mut lanes, commitment);
                    ct::absorb_scalar(&mut lanes, &mut region, value)?;
                }
                let claim = verify_batch_bound(
                    &mut lanes,
                    &mut region,
                    &self.key,
                    &point,
                    &claims,
                    &opening,
                )?;
                let digest = ct::finish(&mut lanes, &mut region)?;
                Ok([
                    claim.folded_generator.x().cell(),
                    claim.folded_generator.y().cell(),
                    digest.cell(),
                ])
            },
        )?;
        for (row, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, row)?;
        }
        Ok(())
    }
}

/// The native transcript the circuit mirrors: the caller's binding.
fn bound<C: PastaCurve>(inputs: &Inputs<C>) -> Transcript<C> {
    let mut transcript = Transcript::<C>::new(TAG, &DOMAIN);
    transcript.absorb_scalars(&inputs.point);
    for claim in &inputs.claims {
        transcript.absorb_point(&claim.commitment);
        transcript.absorb_scalars(&[claim.value]);
    }
    transcript
}

/// An honest bound opening of `m` random columns at a random point, and the
/// public instances the native verifier implies.
fn honest<C: PastaCurve>(
    seed: u64,
    k: usize,
    m: usize,
) -> (PinnedParams<C>, Inputs<C>, Vec<C::Base>) {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let params = PinnedParams::<C>::derive(u32::try_from(k).expect("small k")).expect("params");
    let n = 1 << k;
    let point: Vec<C::ScalarExt> = (0..k)
        .map(|_| <C::ScalarExt as ff::Field>::random(&mut rng))
        .collect();
    let columns: Vec<Vec<C::ScalarExt>> = (0..m)
        .map(|_| {
            (0..n)
                .map(|_| <C::ScalarExt as ff::Field>::random(&mut rng))
                .collect()
        })
        .collect();
    let blinds: Vec<C::ScalarExt> = (0..m)
        .map(|_| <C::ScalarExt as ff::Field>::random(&mut rng))
        .collect();
    let claims: Vec<MleClaim<C>> = columns
        .iter()
        .zip(&blinds)
        .map(|(column, blind)| MleClaim {
            commitment: commit::commit(params.params(), column, blind, Secrecy::Secret, BUDGET)
                .expect("commit")
                .to_affine(),
            value: mle::evaluate(column, &point).expect("2^k values"),
        })
        .collect();
    let mut prover = Inputs {
        point,
        claims,
        opening: dummy_opening::<C>(k),
    };
    let mut transcript = bound(&prover);
    let slices: Vec<&[C::ScalarExt]> = columns.iter().map(Vec::as_slice).collect();
    let (opening, _) = mle::open_batch_bound(
        params.params(),
        &mut transcript,
        &prover.point,
        &prover.claims,
        &slices,
        &blinds,
        BUDGET,
    )
    .expect("opening");
    prover.opening = opening;
    let instances = instances(&params, &prover).expect("honest opening verifies");
    (params, prover, instances)
}

fn dummy_opening<C: PastaCurve>(k: usize) -> BatchOpening<C> {
    let g = C::generator().to_affine();
    BatchOpening {
        blind: <C::ScalarExt as ff::Field>::ZERO,
        rounds: vec![(g, g); k],
        folded_generator: g,
        scalar: <C::ScalarExt as ff::Field>::ZERO,
    }
}

/// `[G'_0.x, G'_0.y, digest]` from the native bound verifier, which must
/// accept and whose claim must decide.
fn instances<C: PastaCurve>(params: &PinnedParams<C>, inputs: &Inputs<C>) -> Option<Vec<C::Base>> {
    let mut transcript = bound(inputs);
    let claim = mle::verify_batch_bound(
        params.params(),
        &mut transcript,
        &inputs.point,
        &inputs.claims,
        &inputs.opening,
        BUDGET,
    )
    .ok()?;
    claim.decide(params, BUDGET).ok()?;
    let generator = claim.g();
    Some(vec![generator.x(), generator.y(), transcript.digest()])
}

fn satisfied<C: PastaCurve>(
    params: &PinnedParams<C>,
    inputs: &Inputs<C>,
    instances: &[C::Base],
) -> bool {
    let circuit = OpeningCircuit {
        key: OpeningKey::new(params.params()),
        k: inputs.point.len(),
        m: inputs.claims.len(),
        inputs: Some(inputs.clone()),
    };
    check_circuit(
        &circuit,
        CIRCUIT_K,
        &[instances.to_vec()],
        CheckMode::Strict,
    )
    .expect("synthesis")
    .is_satisfied()
}

fn verifies_exactly_as_the_native_verifier<C: PastaCurve>(seed: u64) {
    let (params, inputs, expected) = honest::<C>(seed, 3, 3);
    assert!(satisfied(&params, &inputs, &expected), "honest opening");
    let mut other = expected.clone();
    other[2] += <C::Base as ff::Field>::ONE;
    assert!(!satisfied(&params, &inputs, &other), "another digest");
    // A changed value is bound by the caller's transcript: refused natively
    // and unsatisfiable.
    let mut bad = inputs.clone();
    bad.claims[1].value += <C::ScalarExt as ff::Field>::ONE;
    assert!(instances(&params, &bad).is_none());
    assert!(!satisfied(&params, &bad, &expected), "value");
    let mut bad = inputs.clone();
    bad.opening.scalar += <C::ScalarExt as ff::Field>::ONE;
    assert!(instances(&params, &bad).is_none());
    assert!(!satisfied(&params, &bad, &expected), "scalar");
    let mut bad = inputs.clone();
    bad.opening.rounds[0].0 = (bad.opening.rounds[0].0 + C::generator().to_affine()).to_affine();
    assert!(instances(&params, &bad).is_none());
    assert!(!satisfied(&params, &bad, &expected), "L_0");
    let mut bad = inputs;
    bad.opening.blind += <C::ScalarExt as ff::Field>::ONE;
    assert!(instances(&params, &bad).is_none());
    assert!(!satisfied(&params, &bad, &expected), "blind");
}

#[test]
#[ignore = "k16 opening circuit; run optimized"]
fn an_fp_circuit_verifies_pallas_bound_openings_as_the_native_verifier() {
    verifies_exactly_as_the_native_verifier::<Ep>(51);
}

#[test]
#[ignore = "k16 opening circuit; run optimized"]
fn an_fq_circuit_verifies_vesta_bound_openings_as_the_native_verifier() {
    verifies_exactly_as_the_native_verifier::<Eq>(52);
}

/// The group side of a split decider opening: `v*` and `b'` arrive from the
/// field side (here as private scalars); the circuit exposes `G'_0`, the
/// finished digest and the exported low limb of `alpha`.
struct GroupCircuit<C: PastaCurve> {
    key: OpeningKey<C>,
    k: usize,
    m: usize,
    inputs: Option<(Inputs<C>, C::ScalarExt, C::ScalarExt)>,
}

impl<C: PastaCurve> Circuit<C::Base> for GroupCircuit<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            key: self.key.clone(),
            k: self.k,
            m: self.m,
            inputs: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(5);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(
        &self,
        config: Config<C>,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let inputs = self.inputs.as_ref();
        let outputs = layouter.assign_region(
            || "group opening",
            |mut region| {
                let mut lanes = chips.lanes();
                let mut point = Vec::with_capacity(self.k);
                for i in 0..self.k {
                    point.push(
                        lanes
                            .assign_scalar(&mut region, value_of(inputs.map(|x| &x.0.point[i])))?,
                    );
                }
                let mut claims = Vec::with_capacity(self.m);
                for c in 0..self.m {
                    let commitment = lanes.assign_point(
                        &mut region,
                        value_of(inputs.map(|x| C::from(x.0.claims[c].commitment)).as_ref()),
                    )?;
                    let value = lanes.assign_scalar(
                        &mut region,
                        value_of(inputs.map(|x| &x.0.claims[c].value)),
                    )?;
                    claims.push((commitment, value));
                }
                let opening = assign_batch_opening(
                    &mut lanes,
                    &mut region,
                    self.k,
                    inputs.map(|x| &x.0.opening),
                )?;
                let value = lanes.assign_scalar(&mut region, value_of(inputs.map(|x| &x.1)))?;
                let folded = lanes.assign_scalar(&mut region, value_of(inputs.map(|x| &x.2)))?;
                // The caller's transcript is bound as in the single-circuit test.
                ct::start(&mut lanes, TAG, &DOMAIN)?;
                ct::absorb_scalars(&mut lanes, &mut region, &point)?;
                for (commitment, claim_value) in &claims {
                    ct::absorb_point(&mut lanes, commitment);
                    ct::absorb_scalar(&mut lanes, &mut region, claim_value)?;
                }
                let commitments: Vec<_> = claims
                    .iter()
                    .map(|(commitment, _)| commitment.clone())
                    .collect();
                let group = verify_batch_bound_group(
                    &mut lanes,
                    &mut region,
                    &self.key,
                    self.k,
                    &commitments,
                    &value,
                    &folded,
                    &opening,
                )?;
                let digest = ct::finish(&mut lanes, &mut region)?;
                let alpha = group.alpha.ok_or(Error::Synthesis)?;
                Ok([
                    group.claim.folded_generator.x().cell(),
                    group.claim.folded_generator.y().cell(),
                    digest.cell(),
                    alpha.scalar.s6.lo().word().cell(),
                    alpha.scalar.s6.hi().word().cell(),
                ])
            },
        )?;
        for (row, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, row)?;
        }
        Ok(())
    }
}

/// The field side's values natively: `alpha` (replayed), `v* = sum alpha^c v_c`
/// and `b' = folded_eq(sigma, u)` with the native verifier's `u`.
fn field_side<C: PastaCurve>(
    params: &PinnedParams<C>,
    inputs: &Inputs<C>,
) -> (C::ScalarExt, C::ScalarExt, C::ScalarExt) {
    let mut replay = bound(inputs);
    replay.absorb_u64(mle::TAG_BATCH_OPENING);
    replay.absorb_u64(inputs.point.len() as u64);
    replay.absorb_u64(inputs.claims.len() as u64);
    let limbs =
        <C::ScalarExt as iroha_pasta::PastaField>::to_canonical_limbs(&replay.challenge_128());
    let alpha =
        mle::endoscalar::<C::ScalarExt>(u128::from(limbs[0]) | (u128::from(limbs[1]) << 64));
    let value = inputs
        .claims
        .iter()
        .rev()
        .fold(<C::ScalarExt as ff::Field>::ZERO, |acc, claim| {
            acc * alpha + claim.value
        });
    let mut transcript = bound(inputs);
    let claim = mle::verify_batch_bound(
        params.params(),
        &mut transcript,
        &inputs.point,
        &inputs.claims,
        &inputs.opening,
        BUDGET,
    )
    .expect("honest opening");
    let folded = mle::folded_eq(&inputs.point, claim.challenges()).expect("k challenges");
    (alpha, value, folded)
}

fn group_side_verifies_as_the_native_verifier<C: PastaCurve>(seed: u64) {
    let (params, inputs, mut expected) = honest::<C>(seed, 3, 3);
    let (alpha, value, folded) = field_side(&params, &inputs);
    let alpha_limbs = <C::ScalarExt as iroha_pasta::PastaField>::to_canonical_limbs(&alpha);
    for pair in alpha_limbs.chunks(2) {
        expected.push(<C::Base as ff::PrimeField>::from_u128(
            u128::from(pair[0]) | (u128::from(pair[1]) << 64),
        ));
    }
    let satisfied = |value: C::ScalarExt, folded: C::ScalarExt, public: &[C::Base]| {
        let circuit = GroupCircuit {
            key: OpeningKey::new(params.params()),
            k: inputs.point.len(),
            m: inputs.claims.len(),
            inputs: Some((inputs.clone(), value, folded)),
        };
        check_circuit(&circuit, CIRCUIT_K, &[public.to_vec()], CheckMode::Strict)
            .expect("synthesis")
            .is_satisfied()
    };
    assert!(
        satisfied(value, folded, &expected),
        "honest field-side values"
    );
    let one = <C::ScalarExt as ff::Field>::ONE;
    assert!(!satisfied(value + one, folded, &expected), "wrong v*");
    assert!(!satisfied(value, folded + one, &expected), "wrong b'");
    let mut other = expected.clone();
    other[3] += <C::Base as ff::Field>::ONE;
    assert!(!satisfied(value, folded, &other), "another exported alpha");
}

#[test]
#[ignore = "k16 opening circuit; run optimized"]
fn an_fp_circuit_runs_the_group_side_of_a_split_opening() {
    group_side_verifies_as_the_native_verifier::<Ep>(53);
}

#[test]
#[ignore = "k16 opening circuit; run optimized"]
fn an_fq_circuit_runs_the_group_side_of_a_split_opening() {
    group_side_verifies_as_the_native_verifier::<Eq>(54);
}
