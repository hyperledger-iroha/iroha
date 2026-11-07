//! The in-circuit claim-accumulator fold against the native one, on both curves:
//! an `Fp` circuit folds Pallas claims (as an A circuit folds the claims of the
//! Pallas Ω it verifies), an `Fq` circuit folds Vesta claims (as a Q circuit
//! folds σ's claim). Each exposes the digest of the folded claim accumulator.

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        claims::{assign_claim, assign_claim_accumulator, assign_claim_proof, claim_accumulator_digest, verify_claim_fold},
    },
    claims::{self, Claim, ClaimAccumulator, ClaimProof, ROUNDS},
};
use iroha_pasta::{Ep, Eq, PastaCurve};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const DOMAIN: [u8; 32] = *b"iroha_fold claim circuit domain!";
const K: u32 = 16;

struct Inputs<C: PastaCurve> {
    acc: ClaimAccumulator<C>,
    claim: Claim<C>,
    proof: ClaimProof<C::ScalarExt>,
}

impl<C: PastaCurve> Clone for Inputs<C> {
    fn clone(&self) -> Self {
        Self { acc: self.acc, claim: self.claim, proof: self.proof }
    }
}

struct ClaimCircuit<C: PastaCurve> {
    inputs: Option<Inputs<C>>,
}

#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    fold: FoldVerifierConfig<C>,
    instance: Column<InstanceColumn>,
}

impl<C: PastaCurve> Circuit<C::Base> for ClaimCircuit<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { inputs: None }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(1);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(&self, config: Config<C>, mut layouter: impl Layouter<C::Base>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let inputs = self.inputs.as_ref();
        let digest = layouter.assign_region(
            || "claim fold",
            |mut region| {
                let mut lanes = chips.lanes();
                let acc = assign_claim_accumulator(&mut lanes, &mut region, inputs.map(|i| &i.acc))?;
                let claim = assign_claim(&mut lanes, &mut region, inputs.map(|i| &i.claim))?;
                let proof = assign_claim_proof(&mut lanes, &mut region, inputs.map(|i| &i.proof))?;
                let folded = verify_claim_fold(&mut lanes, &mut region, &DOMAIN, &acc, &claim, &proof)?;
                claim_accumulator_digest(&mut lanes, &mut region, &DOMAIN, &folded)
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.instance, 0)
    }
}

/// A claim with a random generator point and random challenges (its validity is
/// the decider's concern, not the fold's).
fn random_claim<C: PastaCurve>(rng: &mut ChaCha20Rng) -> Claim<C> {
    Claim {
        g: (C::generator() * C::ScalarExt::random(&mut *rng)).to_affine(),
        u: core::array::from_fn(|_| C::ScalarExt::random(&mut *rng)),
    }
}

fn honest<C: PastaCurve>() -> (Inputs<C>, ClaimAccumulator<C>) {
    let mut rng = ChaCha20Rng::seed_from_u64(41);
    let (acc, s) = ClaimAccumulator::from_claim(&DOMAIN, &random_claim::<C>(&mut rng));
    let second = random_claim::<C>(&mut rng);
    let (acc, s, _) = claims::fold_claim(&DOMAIN, &acc, &s, &second).expect("fold");
    let third = random_claim::<C>(&mut rng);
    let (next, _, proof) = claims::fold_claim(&DOMAIN, &acc, &s, &third).expect("fold");
    (Inputs { acc, claim: third, proof }, next)
}

fn satisfied<C: PastaCurve>(inputs: &Inputs<C>, digest: C::Base) -> bool {
    let circuit = ClaimCircuit { inputs: Some(inputs.clone()) };
    check_circuit(&circuit, K, &[vec![digest]], CheckMode::Strict).expect("synthesis").is_satisfied()
}

fn folds_exactly_as_the_native_verifier<C: PastaCurve>() {
    let (inputs, folded) = honest::<C>();
    assert_eq!(claims::verify_claim_fold(&DOMAIN, &inputs.acc, &inputs.claim, &inputs.proof), folded);
    assert_eq!(ROUNDS, inputs.claim.u.len());
    let digest = folded.digest(&DOMAIN);
    assert!(satisfied(&inputs, digest));
    // Another K coefficient or F coefficient folds to another accumulator.
    let mut bad = inputs.clone();
    bad.proof.k += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&bad, digest));
    let native = claims::verify_claim_fold(&DOMAIN, &bad.acc, &bad.claim, &bad.proof);
    assert!(satisfied(&bad, native.digest(&DOMAIN)));
    let mut bad = inputs.clone();
    bad.proof.f[3] += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&bad, digest));
    // Another claim challenge or accumulated error.
    let mut bad = inputs.clone();
    bad.claim.u[0] += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&bad, digest));
    let mut bad = inputs;
    bad.acc.e += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&bad, digest));
}

#[test]
#[ignore = "k16 claim-fold circuit; run optimized"]
fn an_fp_circuit_folds_pallas_claims_as_the_native_verifier() {
    folds_exactly_as_the_native_verifier::<Ep>();
}

#[test]
#[ignore = "k16 claim-fold circuit; run optimized"]
fn an_fq_circuit_folds_vesta_claims_as_the_native_verifier() {
    folds_exactly_as_the_native_verifier::<Eq>();
}
