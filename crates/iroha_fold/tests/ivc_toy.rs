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
    CommitKey, FoldedRelation,
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{self, Fresh, Kept, embed, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::step::{BASE, DOMAIN_A, DOMAIN_Q, IVC_A, K, Side, Step, business};

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
/// on `K`) and whether its circuit was satisfied.
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
    let chain = side.chain();
    let base = C::Base::from(BASE);
    let state = if j == chain.first_step { base } else { me.state };
    let state = if lie { state + <C::Base as Field>::ONE } else { state };
    // Q's first instance has no predecessor: its passthrough is A's base hash.
    let start = embed(&ivc::ivc_hash::<Fp>(&IVC_A, 0, Fp::from(BASE), &[Fp::from(BASE)], Fp::ZERO));
    let prepared = prepare_step(&chain, j, other, me.kept.as_ref(), their_last, vec![state], start).expect("prepare");
    let next = business(side, state, prepared.bridge[0]);
    let public = vec![step_public(&chain, j, base, prepared.passthrough, &[next], prepared.digest, &[C::Base::from(j)])];
    let circuit = Step::<C> { side, shape: other.shape(), witness: Some(prepared.witness) };
    let satisfied = check_circuit(&circuit, K, &public, CheckMode::Strict).expect("synthesis").is_satisfied();
    let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
    let (advice, public) = own.stage_witness(&synthesized).expect("stage witness");
    let instance = fresh_instance(own, own_key, own_domain, advice, public, rng).expect("fresh instance");
    me.kept = prepared.kept;
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
