//! The in-circuit bound batch opening against the native
//! `iroha_plonk::pcs::mle::verify_batch_bound`, on both curves: a transcript
//! binds the point and the claims as a decider does, the opening continues it,
//! and the circuit exposes the claim's `G'_0` and the finished transcript's
//! digest.

use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        mle::{OpeningKey, assign_batch_opening, verify_batch_bound},
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
