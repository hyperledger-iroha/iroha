//! The in-circuit fold verifier against the native one, on both curves: an `Fp`
//! circuit folds toy instances over `Fq` committed on Pallas (as an A circuit
//! folds Q instances), and an `Fq` circuit folds toy instances over `Fp`
//! committed on Vesta (as a Q circuit folds A instances). Each exposes the digest
//! of the folded accumulator and the fresh instance's digest.

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Instance, Witness,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        verify::{
            accumulator_digest, assign_accumulator, assign_instance, assign_proof,
            fresh_transcript, verify_fold,
        },
    },
    fold::{self, FoldProof, fresh_instance},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, PastaCurve};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::{K, Toy};

const DOMAIN: [u8; 32] = *b"iroha_fold toy relation domain!!";
const CIRCUIT_K: u32 = 16;

struct Inputs<C: PastaCurve> {
    acc: Accumulator<C>,
    fresh: Instance<C>,
    proof: FoldProof<C::ScalarExt>,
}

impl<C: PastaCurve> Clone for Inputs<C> {
    fn clone(&self) -> Self {
        Self {
            acc: self.acc.clone(),
            fresh: self.fresh.clone(),
            proof: self.proof.clone(),
        }
    }
}

/// Verifies one fold in circuit; public: `[digest of the folded accumulator,
/// digest of the fresh instance]`.
struct FoldCircuit<C: PastaCurve> {
    shape: FoldShape,
    inputs: Option<Inputs<C>>,
}

#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    fold: FoldVerifierConfig<C>,
    instance: Column<InstanceColumn>,
}

impl<C: PastaCurve> Circuit<C::Base> for FoldCircuit<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            shape: self.shape.clone(),
            inputs: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(2);
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
        let (folded, fresh) = layouter.assign_region(
            || "fold",
            |mut region| {
                let mut lanes = chips.lanes();
                let acc = assign_accumulator(
                    &mut lanes,
                    &mut region,
                    &self.shape,
                    inputs.map(|i| &i.acc),
                )?;
                let fresh = assign_instance(
                    &mut lanes,
                    &mut region,
                    &self.shape,
                    inputs.map(|i| &i.fresh),
                )?;
                let proof = assign_proof(
                    &mut lanes,
                    &mut region,
                    &self.shape,
                    inputs.map(|i| &i.proof),
                )?;
                let fresh_digest =
                    fresh_transcript(&mut lanes, &mut region, &self.shape, &DOMAIN, &fresh)?.digest;
                let folded = verify_fold(
                    &mut lanes,
                    &mut region,
                    &self.shape,
                    &DOMAIN,
                    &acc,
                    &fresh,
                    &proof,
                )?;
                let digest = accumulator_digest(&mut lanes, &mut region, &DOMAIN, &folded)?;
                Ok((digest, fresh_digest))
            },
        )?;
        layouter.constrain_instance(folded.cell(), config.instance, 0)?;
        layouter.constrain_instance(fresh.cell(), config.instance, 1)
    }
}

struct Setup<C: PastaCurve> {
    relation: FoldedRelation<C::ScalarExt>,
    key: CommitKey<C>,
}

fn setup<C: PastaCurve>() -> Setup<C> {
    let synthesized = synthesize(&Toy { seed: 1 }, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&synthesized).expect("relation");
    let params = PinnedParams::<C>::derive(K).expect("params");
    Setup {
        relation,
        key: CommitKey::lagrange(params.params()),
    }
}

fn fresh<C: PastaCurve>(
    s: &Setup<C>,
    seed: u64,
    rng: &mut ChaCha20Rng,
) -> (Instance<C>, Witness<C::ScalarExt>) {
    let toy = Toy { seed };
    let synthesized = synthesize(&toy, K, Some(&toy.instances::<C::ScalarExt>())).expect("witness");
    let (advice, public) = s
        .relation
        .stage_witness(&synthesized)
        .expect("stage witness");
    fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, rng).expect("fresh instance")
}

/// A folded accumulator (two instances), a third fresh instance and its fold.
fn honest_fold<C: PastaCurve>(s: &Setup<C>) -> (Inputs<C>, Accumulator<C>) {
    let mut rng = ChaCha20Rng::seed_from_u64(9);
    let (first, w1) = fresh(s, 1, &mut rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    let (second, w2) = fresh(s, 2, &mut rng);
    let (acc, w, _) = fold::fold(&s.relation, &DOMAIN, &acc, &w1, &second, &w2).expect("fold");
    let (third, w3) = fresh(s, 3, &mut rng);
    let (next, _, proof) = fold::fold(&s.relation, &DOMAIN, &acc, &w, &third, &w3).expect("fold");
    (
        Inputs {
            acc,
            fresh: third,
            proof,
        },
        next,
    )
}

fn public_of<C: PastaCurve>(
    s: &Setup<C>,
    inputs: &Inputs<C>,
    folded: &Accumulator<C>,
) -> Vec<Vec<C::Base>> {
    let fresh_digest = inputs
        .fresh
        .fresh_digest(&s.relation, &DOMAIN)
        .expect("fresh");
    vec![vec![folded.digest(&DOMAIN), fresh_digest]]
}

fn satisfied<C: PastaCurve>(s: &Setup<C>, inputs: &Inputs<C>, public: &[Vec<C::Base>]) -> bool {
    let circuit = FoldCircuit {
        shape: s.relation.shape(),
        inputs: Some(inputs.clone()),
    };
    check_circuit(&circuit, CIRCUIT_K, public, CheckMode::Strict)
        .expect("synthesis")
        .is_satisfied()
}

fn folds_exactly_as_the_native_verifier<C: PastaCurve>() {
    let s = setup::<C>();
    let (inputs, folded) = honest_fold(&s);
    assert_eq!(
        fold::verify_fold(
            &s.relation,
            &DOMAIN,
            &inputs.acc,
            &inputs.fresh,
            &inputs.proof
        )
        .expect("native"),
        folded
    );
    let public = public_of(&s, &inputs, &folded);
    assert!(satisfied(&s, &inputs, &public));

    // A changed fold message folds to another accumulator: the honest digest no
    // longer matches, and the circuit's digest is the native verifier's for the
    // changed message.
    for which in 0..2 {
        let mut bad = inputs.clone();
        if which == 0 {
            bad.proof.f[0] += <C::ScalarExt as Field>::ONE;
        } else {
            bad.proof.k[0] += <C::ScalarExt as Field>::ONE;
        }
        assert!(!satisfied(&s, &bad, &public));
        let native = fold::verify_fold(&s.relation, &DOMAIN, &bad.acc, &bad.fresh, &bad.proof)
            .expect("native");
        assert!(satisfied(&s, &bad, &public_of(&s, &bad, &native)));
    }

    // A changed accumulator value or public input changes digests and challenges.
    let mut bad = inputs.clone();
    bad.acc.e += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&s, &bad, &public));
    let mut bad = inputs.clone();
    bad.fresh.public[0][0] += <C::ScalarExt as Field>::ONE;
    assert!(!satisfied(&s, &bad, &public));
    // A changed commitment of the fresh instance (another point) changes its digest.
    let mut bad = inputs;
    bad.fresh.commitments[0] = bad.fresh.commitments[1];
    assert!(!satisfied(&s, &bad, &public));
}

#[test]
#[ignore = "k16 fold-verifier circuit; run optimized"]
fn an_fp_circuit_folds_pallas_instances_as_the_native_verifier() {
    folds_exactly_as_the_native_verifier::<Ep>();
}

#[test]
#[ignore = "k16 fold-verifier circuit; run optimized"]
fn an_fq_circuit_folds_vesta_instances_as_the_native_verifier() {
    folds_exactly_as_the_native_verifier::<Eq>();
}
