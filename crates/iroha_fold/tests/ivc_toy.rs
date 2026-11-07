//! A toy two-curve IVC chain end to end (output `IVC.md`).
//!
//! Q steps are circuits over `Fq` that fold A instances (committed on Vesta);
//! A steps are circuits over `Fp` that fold Q instances (committed on Pallas).
//! Every step checks its passthrough hash against its own previous state and
//! kept accumulator, folds the other side's previous instance (or starts the
//! accumulator), runs a toy business step that reads the other side's bridge,
//! and outputs `[passthrough, h_out, bridge]`. Shapes are fixed first: a step
//! circuit's shape does not depend on the shape it folds.
//!
//! The chain is checked by the native deciders: both kept accumulators decide,
//! the last instance decides as a fresh instance, and the last hash recomputes.
//! A step that lies about its previous state breaks the accumulator folding it.

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Instance, Witness,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, assert_hash_unless, foreign_bounded_word, foreign_hash_word, ivc_hash},
        verify::{accumulator_digest, assign_accumulator, assign_instance, assign_proof, verify_fold_or_start},
    },
    decide::{decide, decide_fresh},
    fold::{FoldProof, fresh_instance},
    ivc::{self, embed, fold_or_start, placeholder_accumulator, placeholder_instance},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::Word;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const K: u32 = 16;
/// Fold-transcript domains of the two relations.
const DOMAIN_Q: [u8; 32] = *b"toy ivc Q relation fold domain!!";
const DOMAIN_A: [u8; 32] = *b"toy ivc A relation fold domain!!";
/// IVC-hash domains of the two sides.
const IVC_Q: [u8; 32] = *b"toy ivc Q side hash domain......";
const IVC_A: [u8; 32] = *b"toy ivc A side hash domain......";
/// The segment base (both sides' first state).
const BASE: u64 = 0x5eed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Q,
    A,
}

impl Side {
    /// Fold domain of the instances this side folds (the other side's relation).
    fn fold_domain(self) -> &'static [u8; 32] {
        match self {
            Self::Q => &DOMAIN_A,
            Self::A => &DOMAIN_Q,
        }
    }
    fn ivc_domain(self) -> &'static [u8; 32] {
        match self {
            Self::Q => &IVC_Q,
            Self::A => &IVC_A,
        }
    }
    /// Step index of the side's first instance.
    fn first(self) -> u64 {
        match self {
            Self::Q => 1,
            Self::A => 2,
        }
    }
}

/// The witness of one step over `C::Base` that folds instances committed on `C`.
struct StepWitness<C: PastaCurve> {
    step: u64,
    state: C::Base,
    stored: C::Base,
    acc: Accumulator<C>,
    fresh: Instance<C>,
    proof: FoldProof<C::ScalarExt>,
    passthrough: C::Base,
}

impl<C: PastaCurve> Clone for StepWitness<C> {
    fn clone(&self) -> Self {
        Self {
            step: self.step,
            state: self.state,
            stored: self.stored,
            acc: self.acc.clone(),
            fresh: self.fresh.clone(),
            proof: self.proof.clone(),
            passthrough: self.passthrough,
        }
    }
}

struct Step<C: PastaCurve> {
    side: Side,
    shape: FoldShape,
    witness: Option<StepWitness<C>>,
}

#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    fold: FoldVerifierConfig<C>,
    instance: Column<InstanceColumn>,
}

impl<C: PastaCurve> Circuit<C::Base> for Step<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { side: self.side, shape: self.shape.clone(), witness: None }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(&self, config: Config<C>, mut layouter: impl Layouter<C::Base>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(|| "step", |mut region| self.layout(&mut chips, &mut region))?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}

impl<C: PastaCurve> Step<C> {
    fn known<T>(&self, f: impl Fn(&StepWitness<C>) -> T) -> Value<T> {
        self.witness.as_ref().map_or_else(Value::unknown, |w| Value::known(f(w)))
    }

    fn layout(&self, chips: &mut FoldVerifierChips<C>, region: &mut Region<'_, C::Base>) -> Result<Vec<Word<C::Base>>, Error> {
        let one = <C::Base as Field>::ONE;
        let w = self.witness.as_ref();
        let step = chips.glue.witness(region, self.known(|w| C::Base::from(w.step)))?;
        let base = chips.glue.constant(region, C::Base::from(BASE))?;
        let state = chips.glue.witness(region, self.known(|w| w.state))?;
        let stored = chips.glue.witness(region, self.known(|w| w.stored))?;
        let pass_witness = chips.glue.witness(region, self.known(|w| w.passthrough))?;
        let acc = assign_accumulator(chips, region, &self.shape, w.map(|w| &w.acc))?;
        let fresh = assign_instance(chips, region, &self.shape, w.map(|w| &w.fresh))?;
        let proof = assign_proof(chips, region, &self.shape, w.map(|w| &w.proof))?;

        // Flags: the side's first instance, an absent accumulator, and (Q only)
        // no predecessor at all.
        let shifted = chips.glue.add_constant(region, &step, -C::Base::from(self.side.first()))?;
        let first = chips.glue.is_zero(region, &shifted)?;
        let absent = chips.glue.is_zero(region, &stored)?;
        let no_predecessor = (self.side == Side::Q).then_some(&first);

        // The first instance starts from the base state.
        let later = chips.glue.not(region, &first)?;
        assert_equal_unless(chips, region, &state, &base, Some(&later))?;
        // The passthrough is this side's previous hash.
        let previous = chips.glue.add_constant(region, &step, -C::Base::from(2))?;
        let expected = ivc_hash(chips, region, self.side.ivc_domain(), &previous, &base, &[state.clone()], &stored)?;
        assert_hash_unless(chips, region, &fresh.public[0][0], &expected, no_predecessor)?;
        // The kept accumulator is the stored one.
        let digest = accumulator_digest(chips, region, self.side.fold_domain(), &acc)?;
        assert_equal_unless(chips, region, &digest, &stored, Some(&absent))?;
        // Fold the other side's previous instance, or start from it.
        let folded = verify_fold_or_start(chips, region, &self.shape, self.side.fold_domain(), &acc, &fresh, &proof, &absent)?;
        let new_digest = accumulator_digest(chips, region, self.side.fold_domain(), &folded)?;
        let from_fresh = foreign_hash_word(chips, region, &fresh.public[0][1])?;
        let bridge = foreign_bounded_word(chips, region, &fresh.public[0][2])?;
        let (out_digest, passthrough) = match no_predecessor {
            Some(bit) => {
                let zero = chips.glue.constant(region, <C::Base as Field>::ZERO)?;
                (
                    chips.glue.select(region, bit, &zero, &new_digest)?,
                    chips.glue.select(region, bit, &pass_witness, &from_fresh)?,
                )
            }
            None => (new_digest, from_fresh),
        };
        // Toy business: Q doubles and adds one; A adds the square of Q's bridge.
        let next = match self.side {
            Side::Q => chips.glue.linear(region, &[(C::Base::from(2), &state)], one)?,
            Side::A => {
                let square = chips.glue.mul(region, &bridge, &bridge)?;
                chips.glue.add(region, &state, &square)?
            }
        };
        let h = ivc_hash(chips, region, self.side.ivc_domain(), &step, &base, &[next], &out_digest)?;
        Ok(vec![passthrough, h.word, step])
    }
}

/// The toy business, natively.
fn business<F: PastaField>(side: Side, state: F, bridge: F) -> F {
    match side {
        Side::Q => state.double() + F::ONE,
        Side::A => state + bridge.square(),
    }
}

/// A fresh instance and its witness.
type Fresh<C> = (Instance<C>, Witness<<C as PastaCurve>::ScalarExt>);
/// A kept accumulator and its witness.
type Kept<C> = (Accumulator<C>, Witness<<C as PastaCurve>::ScalarExt>);

/// The two relations: Q steps (over `Fq`, instances committed on Pallas) and A
/// steps (over `Fp`, instances committed on Vesta).
struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
}

fn relations() -> Relations {
    // Any shape with three public inputs lays out an A step; its own shape does
    // not depend on it, which the second A synthesis checks.
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![3],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_shape = |shape: FoldShape| {
        let layout = synthesize(&Step::<Ep> { side: Side::A, shape, witness: None }, K, None).expect("A layout");
        FoldedRelation::<Fp>::from_synthesized(&layout).expect("A relation")
    };
    let first = a_shape(placeholder).shape();
    let layout = synthesize(&Step::<Eq> { side: Side::Q, shape: first.clone(), witness: None }, K, None).expect("Q layout");
    let q = FoldedRelation::<Fq>::from_synthesized(&layout).expect("Q relation");
    let a = a_shape(q.shape());
    assert_eq!(a.shape(), first, "a step's shape does not depend on the shape it folds");
    Relations {
        key_q: CommitKey::lagrange(PinnedParams::<Ep>::derive(K).expect("params").params()),
        key_a: CommitKey::lagrange(PinnedParams::<Eq>::derive(K).expect("params").params()),
        q,
        a,
    }
}

/// One side's chain state.
struct SideState<C: PastaCurve, K: PastaCurve> {
    /// The side's own state (over `C::Base`).
    state: C::Base,
    /// The accumulator of the other side's instances this side keeps.
    kept: Option<Kept<C>>,
    /// The side's last fresh instance (committed on `K`).
    last: Option<Fresh<K>>,
}

struct Chain {
    rel: Relations,
    q: SideState<Eq, Ep>,
    a: SideState<Ep, Eq>,
    rng: ChaCha20Rng,
    /// A step whose witness lies about its previous state (adds one).
    corrupt: Option<u64>,
    /// Whether every step circuit was satisfied.
    satisfied: Vec<bool>,
}

/// Runs one step of side `side` over `C::Base` folding the other side's last
/// instance (committed on `C`), and returns the step's fresh instance (committed
/// on `K`).
#[allow(clippy::too_many_arguments)]
fn run_step<C: PastaCurve, K: PastaCurve<ScalarExt = C::Base>>(
    side: Side,
    j: u64,
    other: &FoldedRelation<C::ScalarExt>,
    own: &FoldedRelation<C::Base>,
    own_key: &CommitKey<K>,
    own_domain: &[u8; 32],
    me: &mut SideState<C, K>,
    their_last: Option<&Fresh<C>>,
    lie: bool,
    rng: &mut ChaCha20Rng,
) -> (Fresh<K>, bool) {
    let base = C::Base::from(BASE);
    let stored = me.kept.as_ref().map_or(<C::Base as Field>::ZERO, |(acc, _)| acc.digest(side.fold_domain()));
    let state = if j == side.first() { base } else { me.state };
    let state = if lie { state + <C::Base as Field>::ONE } else { state };
    let (fresh, fresh_witness) = match their_last {
        Some((instance, witness)) => (instance.clone(), witness.clone()),
        None => (placeholder_instance::<C>(other), Witness { columns: Vec::new(), blinds: Vec::new() }),
    };
    let (acc, folded, proof) = if their_last.is_some() {
        let (acc, witness, proof) =
            fold_or_start(other, side.fold_domain(), me.kept.as_ref().map(|(a, w)| (a, w)), &fresh, &fresh_witness)
                .expect("fold");
        let input = me.kept.as_ref().map_or_else(|| placeholder_accumulator::<C>(other), |(a, _)| a.clone());
        (input, Some((acc, witness)), proof)
    } else {
        let zero = <C::ScalarExt as Field>::ZERO;
        let proof = FoldProof { f: vec![zero; other.index_bits()], k: vec![zero; other.degree() - 1] };
        (placeholder_accumulator::<C>(other), None, proof)
    };
    let out_digest = folded.as_ref().map_or(<C::Base as Field>::ZERO, |(acc, _)| acc.digest(side.fold_domain()));
    // Q's first instance has no predecessor: its passthrough is A's base hash.
    let passthrough = match their_last {
        Some((instance, _)) => embed::<C::ScalarExt, C::Base>(&instance.public[0][1]),
        None => embed(&ivc::ivc_hash::<Fp>(&IVC_A, 0, Fp::from(BASE), &[Fp::from(BASE)], Fp::ZERO)),
    };
    let bridge = their_last.map_or(<C::Base as Field>::ZERO, |(instance, _)| embed(&instance.public[0][2]));
    let next = business(side, state, bridge);
    let h = ivc::ivc_hash(side.ivc_domain(), j, base, &[next], out_digest);
    let public = vec![vec![passthrough, h, C::Base::from(j)]];
    let circuit = Step::<C> {
        side,
        shape: other.shape(),
        witness: Some(StepWitness { step: j, state, stored, acc, fresh, proof, passthrough }),
    };
    let satisfied = check_circuit(&circuit, K, &public, CheckMode::Strict).expect("synthesis").is_satisfied();
    let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
    let (advice, public) = own.stage_witness(&synthesized).expect("stage witness");
    let instance = fresh_instance(own, own_key, own_domain, advice, public, rng).expect("fresh instance");
    if let Some(kept) = folded {
        me.kept = Some(kept);
    }
    me.state = next;
    (instance, satisfied)
}

impl Chain {
    fn new(corrupt: Option<u64>) -> Self {
        Self {
            rel: relations(),
            q: SideState { state: Fq::from(BASE), kept: None, last: None },
            a: SideState { state: Fp::from(BASE), kept: None, last: None },
            rng: ChaCha20Rng::seed_from_u64(77),
            corrupt,
            satisfied: Vec::new(),
        }
    }

    fn step(&mut self, j: u64) {
        let lie = self.corrupt == Some(j);
        if j % 2 == 1 {
            let their = self.a.last.take();
            let (instance, ok) = run_step::<Eq, Ep>(
                Side::Q, j, &self.rel.a, &self.rel.q, &self.rel.key_q, &DOMAIN_Q, &mut self.q, their.as_ref(), lie, &mut self.rng,
            );
            self.q.last = Some(instance);
            self.satisfied.push(ok);
        } else {
            let their = self.q.last.take();
            let (instance, ok) = run_step::<Ep, Eq>(
                Side::A, j, &self.rel.q, &self.rel.a, &self.rel.key_a, &DOMAIN_A, &mut self.a, their.as_ref(), lie, &mut self.rng,
            );
            self.a.last = Some(instance);
            self.satisfied.push(ok);
        }
    }

    /// Both kept accumulators decide and the last (A) instance decides fresh.
    fn decides(&self) -> bool {
        let (acc_a, w_a) = self.q.kept.as_ref().expect("Q keeps A instances");
        let (acc_q, w_q) = self.a.kept.as_ref().expect("A keeps Q instances");
        let (last, last_w) = self.a.last.as_ref().expect("last A instance");
        decide(&self.rel.a, &self.rel.key_a, acc_a, w_a).is_ok()
            && decide(&self.rel.q, &self.rel.key_q, acc_q, w_q).is_ok()
            && decide_fresh(&self.rel.a, &self.rel.key_a, &DOMAIN_A, last, last_w).is_ok()
    }
}

#[test]
#[ignore = "k16 two-curve IVC chain; run optimized"]
fn a_toy_two_curve_chain_folds_and_decides() {
    let mut chain = Chain::new(None);
    for j in 1..=6 {
        chain.step(j);
    }
    assert!(chain.satisfied.iter().all(|ok| *ok), "every step circuit is satisfied: {:?}", chain.satisfied);
    assert!(chain.decides());
    // The last hash recomputes from A's state and the Q accumulator it keeps.
    let (last, _) = chain.a.last.as_ref().expect("last");
    let (acc_q, _) = chain.a.kept.as_ref().expect("kept");
    let h = ivc::ivc_hash::<Fp>(&IVC_A, 6, Fp::from(BASE), &[chain.a.state], acc_q.digest(&DOMAIN_Q));
    assert_eq!(last.public[0][1], h);
    // A's state is the base plus the squares of Q's bridges (steps 1, 3, 5).
    assert_eq!(chain.a.state, Fp::from(BASE + 1 + 9 + 25));
}

#[test]
#[ignore = "k16 two-curve IVC chain; run optimized"]
fn a_step_lying_about_its_state_breaks_the_folding_accumulator() {
    let mut chain = Chain::new(Some(4));
    for j in 1..=6 {
        chain.step(j);
    }
    assert_eq!(chain.satisfied, vec![true, true, true, false, true, true]);
    // A4 was folded into Q's accumulator of A instances by Q5: it no longer decides.
    let (acc_a, w_a) = chain.q.kept.as_ref().expect("kept");
    assert!(decide(&chain.rel.a, &chain.rel.key_a, acc_a, w_a).is_err());
    assert!(!chain.decides());
}
