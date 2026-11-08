//! The v2 decider against the native one: honest accumulators reduce to claims
//! whose batched opening verifies and whose generator claim decides; false
//! copies, lookup inputs outside their tables, false gates, wrong errors and
//! tampered messages do not.

use ff::{Field, PrimeField};
use group::Curve;
use iroha_pasta::{Ep, Fq, msm::MemoryBudget};
use iroha_plonk::{
    cs::{Advice, Column, ConstraintSystem, Instance as InstanceColumn, Rotation, Selector, TableColumn},
    frontend::{Circuit, Error as PlonkError, Layouter, SimpleFloorPlanner, Value, synthesize},
    pcs::{
        ipa::{
            PinnedParams,
            commit::{Secrecy, commit},
        },
        mle as pcs_mle,
    },
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use std::sync::Arc;

use iroha_pasta::Fp;
use iroha_plonk::check::{CheckMode, check_circuit};

use super::{
    DeciderV2Plan, DeciderV2Proof, helper_digest, instance_copy_terms, mle, open,
    plan::{COPY_GROUP, Committed, STEP_RUNS, Source, Steps},
    prove, prove_many, sumcheck, verify, verify_many, verify_opening,
};
use crate::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        decider_v2::{
            assign_accumulator_field, assign_accumulator_field_terms, assign_decider_v2_field_proof, claim_commitments,
            helper_digest as helper_digest_in_circuit, opening_scalars, opening_transcript, verify_decider_v2_field,
        },
        field::{FieldVerifierChips, FieldVerifierConfig},
        transcript as circuit_transcript,
        verify::assign_accumulator,
    },
    transcript::Transcript,
    transcript::s6_limbs,
    relation::FoldShape,
};
use crate::{
    Error,
    commit::{CommitKey, Witness},
    decide::{decide, tests as forgery},
    fold::{Accumulator, Instance, fold, fresh_instance, pow_vector},
    relation::FoldedRelation,
};

const DOMAIN: [u8; 32] = *b"iroha_fold v2 decider test dom!!";
const K: u32 = 5;
const ROWS: usize = 8;

fn random_point(rng: &mut ChaCha20Rng, mu: usize) -> Vec<Fq> {
    (0..mu).map(|_| Fq::random(&mut *rng)).collect()
}

fn evaluate(values: &[Fq], point: &[Fq]) -> Fq {
    pcs_mle::evaluate(values, point).expect("matching length")
}

/// `sum_j eq(rho, j) eq(sigma, (j + k) mod n)` by brute force.
fn shifted_eq_by_definition(rho: &[Fq], sigma: &[Fq], k: i32) -> Fq {
    let (a, b) = (pcs_mle::eq_table(rho), pcs_mle::eq_table(sigma));
    (0..a.len()).fold(Fq::ZERO, |acc, j| acc + a[j] * b[mle::rotate(j, k, a.len())])
}

#[test]
fn multilinear_primitives_match_their_definitions() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let mu = 4;
    let n = 1 << mu;
    let x = random_point(&mut rng, mu);
    let table = pcs_mle::eq_table(&x);
    for (i, w) in table.iter().enumerate() {
        let bits: Vec<Fq> = (0..mu).map(|j| Fq::from(((i >> j) & 1) as u64)).collect();
        assert_eq!(*w, mle::eq(&x, &bits));
    }
    let column: Vec<Fq> = (0..n).map(|_| Fq::random(&mut rng)).collect();
    let mut folded = column.clone();
    for r in &x {
        mle::fold(&mut folded, *r);
    }
    assert_eq!(folded[0], evaluate(&column, &x));
    for u in 0..=n {
        let direct = table[..u].iter().fold(Fq::ZERO, |acc, w| acc + w);
        assert_eq!(mle::prefix_indicator(u, &x), direct, "u = {u}");
    }
    let index: Vec<Fq> = (0..n).map(|i| Fq::from(7 + i as u64)).collect();
    assert_eq!(mle::index(7, &x), evaluate(&index, &x));
    let beta = random_point(&mut rng, mu);
    assert_eq!(mle::pow(&beta, &x), evaluate(&pow_vector(&beta), &x));
    // A column at rotation k, at rho, is sum_m v[m] e[m] with e[m] = eq(rho, m - k),
    // and e at sigma is shifted_eq(rho, sigma, k), for negative and wrapping k too.
    let rho = random_point(&mut rng, mu);
    let sigma = random_point(&mut rng, mu);
    let eq_rho = pcs_mle::eq_table(&rho);
    let n_i32 = i32::try_from(n).expect("small");
    for k in [-n_i32 - 1, -n_i32 + 1, -3, -1, 0, 1, 2, 5, n_i32 - 1, n_i32 + 2] {
        let e: Vec<Fq> = (0..n).map(|m| eq_rho[mle::rotate(m, -k, n)]).collect();
        let weighted = column.iter().zip(&e).fold(Fq::ZERO, |acc, (v, w)| acc + *v * w);
        assert_eq!(evaluate(&mle::rotated(&column, k), &rho), weighted, "k = {k}");
        assert_eq!(evaluate(&e, &sigma), shifted_eq_by_definition(&rho, &sigma, k), "k = {k}");
        assert_eq!(pcs_mle::shifted_eq(&rho, &sigma, k), Some(evaluate(&e, &sigma)), "k = {k}");
        assert_eq!(pcs_mle::shifted_eq_table(&rho, k), Some(e), "k = {k}");
    }
    // A step function's extension, from its changes alone, at every rotation:
    // runs that wrap around, a constant column and a column changing on every row.
    let steps: Vec<Fq> = (0..n).map(|i| Fq::from([5, 5, 0, 0, 0, 9, 9, 9, 9, 9, 2, 2, 2, 5, 5, 5][i])).collect();
    let constant = vec![Fq::from(4); n];
    for values in [&steps, &constant, &column] {
        for k in [-3, 0, 1, 7] {
            let rotated = mle::rotated(values, k);
            assert_eq!(Steps::of(&rotated).eval(&x), evaluate(&rotated, &x), "k = {k}");
        }
    }
    assert_eq!(Steps::of(&steps).changes.len(), 4);
    assert!(Steps::of(&constant).changes.is_empty());
}

/// What every plan keeps: key polynomials pairwise distinct, step tables exactly
/// for keys of at most its step bound of runs (and every one of them evaluated
/// correctly), copy helpers grouping the copy columns in order.
fn assert_plan_invariants(plan: &DeciderV2Plan<Ep>) {
    for (i, a) in plan.key_polys.iter().enumerate() {
        assert!(plan.key_polys[..i].iter().all(|b| b != a), "key {i} repeats an earlier key");
    }
    let runs = plan.key_runs();
    let mut rng = ChaCha20Rng::seed_from_u64(14);
    let x = random_point(&mut rng, plan.mu);
    for (index, source) in plan.sources.iter().enumerate() {
        match *source {
            Source::Steps(key, k) => {
                assert!(runs[key].0 <= plan.step_runs, "a step key has {} runs", runs[key].0);
                let steps = plan.steps[index].as_ref().expect("step table");
                assert_eq!(steps.eval(&x), evaluate(&mle::rotated(&plan.key_polys[key], k), &x));
            }
            Source::Key(key, _) => {
                assert!(runs[key].0 > plan.step_runs, "a committed key has {} runs", runs[key].0);
                assert!(plan.steps[index].is_none());
            }
            _ => assert!(plan.steps[index].is_none()),
        }
    }
    let grouped: Vec<usize> = plan.copies.helpers.iter().flatten().copied().collect();
    assert_eq!(grouped, (0..plan.copies.columns.len()).collect::<Vec<_>>());
    assert!(plan.copies.helpers.iter().all(|group| !group.is_empty() && group.len() <= COPY_GROUP));
    assert_eq!(plan.helper_count(), plan.copies.columns.len().div_ceil(COPY_GROUP));
}

#[test]
fn round_polynomials_interpolate_from_their_sent_values() {
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    let coefficients: Vec<Fq> = (0..6).map(|_| Fq::random(&mut rng)).collect();
    let p = |x: Fq| coefficients.iter().rev().fold(Fq::ZERO, |acc, c| acc * x + c);
    let values: Vec<Fq> = (0..6u64).map(|t| p(Fq::from(t))).collect();
    let x = Fq::random(&mut rng);
    assert_eq!(sumcheck::interpolate(&values, x), p(x));
    let points: Vec<Fq> = sumcheck::points(5);
    let sent: Vec<Fq> = points.iter().map(|t| p(*t)).collect();
    let claim = p(Fq::ZERO) + p(Fq::ONE);
    assert_eq!(sumcheck::next_claim(claim, &sent, x), p(x));
}

#[derive(Clone, Copy, Debug)]
struct MiniConfig {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    d: Column<Advice>,
    instance: Column<InstanceColumn>,
    mul: Selector,
    rot: Selector,
    lookup: Selector,
    table: TableColumn,
}

/// `c = a b`, `d = a_next - a`, `a` in `0..16`, with copies `a[0] = x[0]`,
/// `b[0] = 3` (a constant), `b[1] = a[2]` and `c[7] = x[1]`. A broken witness
/// keeps every gate but breaks `b[1] = a[2]`.
#[derive(Clone, Copy, Debug)]
struct Mini {
    seed: u64,
    broken: bool,
}

impl Mini {
    fn a(self, row: usize) -> u64 {
        (row as u64 * 3 + self.seed) % 16
    }
    fn b(self, row: usize) -> u64 {
        match row {
            0 => 3,
            1 => self.a(2) + u64::from(self.broken),
            _ => (row as u64 + 2) % 16,
        }
    }
    fn instances(self) -> Vec<Vec<Fq>> {
        vec![vec![Fq::from(self.a(0)), Fq::from(self.a(ROWS - 1) * self.b(ROWS - 1))]]
    }
}

// Column names follow the gate equations (a, b, c, d).
#[allow(clippy::many_single_char_names)]
impl Circuit<Fq> for Mini {
    type Config = MiniConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> MiniConfig {
        let [a, b, c, d] = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let instance = meta.instance_column(2);
        for column in [a, b, c] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let (mul, rot) = (meta.selector(), meta.selector());
        let lookup = meta.complex_selector();
        let table = meta.lookup_table_column();
        meta.create_gate("mul", |v| {
            let q = v.query_selector(mul);
            let (a, b, c) = (v.query_advice(a, Rotation::cur()), v.query_advice(b, Rotation::cur()), v.query_advice(c, Rotation::cur()));
            vec![q * (a * b - c)]
        });
        meta.create_gate("rot", |v| {
            let q = v.query_selector(rot);
            let (a, next, d) = (v.query_advice(a, Rotation::cur()), v.query_advice(a, Rotation::next()), v.query_advice(d, Rotation::cur()));
            vec![q * (next - a - d)]
        });
        meta.lookup("range", |v| {
            let q = v.query_selector(lookup);
            vec![(q * v.query_advice(a, Rotation::cur()), table)]
        });
        MiniConfig { a, b, c, d, instance, mul, rot, lookup, table }
    }

    fn synthesize(&self, config: MiniConfig, mut layouter: impl Layouter<Fq>) -> Result<(), PlonkError> {
        layouter.assign_table(
            || "range",
            |mut table| {
                for i in 0..16u64 {
                    table.assign_cell(|| "i", config.table, usize::try_from(i).expect("small"), || Value::known(Fq::from(i)))?;
                }
                Ok(())
            },
        )?;
        let last = layouter.assign_region(
            || "rows",
            |mut region| {
                let mut cells = Vec::new();
                for row in 0..ROWS {
                    config.mul.enable(&mut region, row)?;
                    region.enable_selector(|| "lookup", &config.lookup, row)?;
                    let (a, b) = (self.a(row), self.b(row));
                    let a_cell = if row == 0 {
                        region.assign_advice_from_instance(|| "x0", config.instance, 0, config.a, 0)?.cell()
                    } else {
                        region.assign_advice(config.a, row, Value::known(Fq::from(a)))?.cell()
                    };
                    let b_cell = if row == 0 {
                        region.assign_advice_from_constant(|| "three", config.b, 0, Fq::from(3))?.cell()
                    } else {
                        region.assign_advice(config.b, row, Value::known(Fq::from(b)))?.cell()
                    };
                    let c_cell = region.assign_advice(config.c, row, Value::known(Fq::from(a * b)))?.cell();
                    if row + 1 < ROWS {
                        config.rot.enable(&mut region, row)?;
                        let d = Fq::from(self.a(row + 1)) - Fq::from(a);
                        region.assign_advice(config.d, row, Value::known(d))?;
                    }
                    cells.push((a_cell, b_cell, c_cell));
                }
                region.constrain_equal(cells[1].1, cells[2].0)?;
                Ok(cells[ROWS - 1].2)
            },
        )?;
        layouter.constrain_instance(last, config.instance, 1)
    }
}

struct Setup {
    relation: FoldedRelation<Fq>,
    params: PinnedParams<Ep>,
    key: CommitKey<Ep>,
    plan: DeciderV2Plan<Ep>,
}

fn setup() -> Setup {
    let layout = synthesize(&Mini { seed: 1, broken: false }, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&layout).expect("relation");
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    let key = CommitKey::monomial(params.params());
    let plan = DeciderV2Plan::new(relation.clone(), &key).expect("plan");
    Setup { relation, params, key, plan }
}

fn fresh(s: &Setup, mini: Mini, rng: &mut ChaCha20Rng) -> (Instance<Ep>, Witness<Fq>) {
    let synthesized = synthesize(&mini, K, Some(&mini.instances())).expect("witness");
    let (advice, public) = s.relation.stage_witness(&synthesized).expect("stage witness");
    fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, rng).expect("fresh instance")
}

/// An accumulator of three genuine instances.
fn accumulator(s: &Setup) -> (Accumulator<Ep>, Witness<Fq>) {
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let (first, mut witness) = fresh(s, Mini { seed: 1, broken: false }, &mut rng);
    let mut acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    for seed in 2..4 {
        let (instance, w) = fresh(s, Mini { seed, broken: false }, &mut rng);
        let (next, next_w, _) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &w).expect("fold");
        acc = next;
        witness = next_w;
    }
    decide(&s.relation, &s.key, &acc, &witness).expect("genuine accumulator");
    (acc, witness)
}

/// Whether the v2 decider accepts: the proof verifies, its batched opening
/// verifies and the resulting generator claim decides.
fn decides(s: &Setup, acc: &Accumulator<Ep>, witness: &Witness<Fq>) -> Result<(), Error> {
    let mut proved = prove(&s.plan, &DOMAIN, acc, witness)?;
    let (opening, _) = open(s.params.params(), &mut proved)?;
    let mut verified = verify(&s.plan, &DOMAIN, acc, &proved.proof)?;
    let claim = verify_opening(s.params.params(), &mut verified, &opening)?;
    claim.decide(&s.params, MemoryBudget::DEFAULT).map_err(|_| Error::Rejected("generator claim"))
}

#[test]
fn honest_accumulators_reduce_to_claims_that_open() {
    let s = setup();
    let (sources, keys, helpers, terms) = s.plan.sizes();
    assert!(sources > 0 && keys > 0 && terms > 0);
    assert_eq!(helpers, 2, "copy columns a, b, c and the constants column, two per helper");
    assert_plan_invariants(&s.plan);
    // Selectors, guards and the range table are step keys the verifier
    // evaluates; the copy permutations are committed.
    assert!(s.plan.sources.iter().any(|source| matches!(source, Source::Steps(..))));
    assert_eq!(s.plan.committed.iter().filter(|poly| matches!(poly, Committed::Key(_))).count(), 4);
    // The decider's commitments are the batched opening's: sum_j a_j g_j + rho W.
    let mut rng = ChaCha20Rng::seed_from_u64(7);
    let column: Vec<Fq> = (0..s.relation.n).map(|_| Fq::random(&mut rng)).collect();
    let blind = Fq::random(&mut rng);
    let pcs = commit(s.params.params(), &column, &blind, Secrecy::Secret, MemoryBudget::DEFAULT).expect("commit");
    assert_eq!(s.key.commit(&column, &blind).expect("commit"), pcs.to_affine());
    let (acc, witness) = accumulator(&s);
    let mut proved = prove(&s.plan, &DOMAIN, &acc, &witness).expect("prove");
    let (opening, prover_claim) = open(s.params.params(), &mut proved).expect("open");
    let mut verified = verify(&s.plan, &DOMAIN, &acc, &proved.proof).expect("verify");
    assert_eq!(verified.point, proved.point);
    assert_eq!(verified.claims, proved.claims);
    let claim = verify_opening(s.params.params(), &mut verified, &opening).expect("opening verifies");
    assert_eq!(claim, prover_claim);
    claim.decide(&s.params, MemoryBudget::DEFAULT).expect("generator claim decides");
    // A claimed value other than the polynomial's: the opening does not verify,
    // or its generator claim does not decide.
    let mut lying = verify(&s.plan, &DOMAIN, &acc, &proved.proof).expect("verify");
    lying.claims[0].value += Fq::ONE;
    assert!(
        verify_opening(s.params.params(), &mut lying, &opening)
            .map_or(true, |claim| claim.decide(&s.params, MemoryBudget::DEFAULT).is_err())
    );
    // A fresh instance is an accumulator with e = 0 too.
    let mut rng = ChaCha20Rng::seed_from_u64(4);
    let (instance, w) = fresh(&s, Mini { seed: 9, broken: false }, &mut rng);
    let fresh_acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    decides(&s, &fresh_acc, &w).expect("fresh accumulator");
}

/// Whether `plan`'s decider accepts (as [`decides`] for the setup's plan).
fn decides_with(s: &Setup, plan: &DeciderV2Plan<Ep>, acc: &Accumulator<Ep>, witness: &Witness<Fq>) -> Result<(), Error> {
    let mut proved = prove(plan, &DOMAIN, acc, witness)?;
    let (opening, _) = open(s.params.params(), &mut proved)?;
    let mut verified = verify(plan, &DOMAIN, acc, &proved.proof)?;
    let claim = verify_opening(s.params.params(), &mut verified, &opening)?;
    claim.decide(&s.params, MemoryBudget::DEFAULT).map_err(|_| Error::Rejected("generator claim"))
}

#[test]
fn a_larger_step_bound_opens_fewer_keys_and_still_decides() {
    let s = setup();
    assert_eq!(s.plan.step_runs, STEP_RUNS);
    // At 64 runs the copy permutations of the 32-row domain are step tables too.
    let plan = DeciderV2Plan::with_step_runs(s.relation.clone(), &s.key, 64).expect("plan");
    assert_plan_invariants(&plan);
    let keys = |plan: &DeciderV2Plan<Ep>| plan.committed.iter().filter(|poly| matches!(poly, Committed::Key(_))).count();
    assert_eq!((keys(&s.plan), keys(&plan)), (4, 0));
    let (acc, witness) = accumulator(&s);
    decides_with(&s, &plan, &acc, &witness).expect("the larger bound decides");
    let mut rng = ChaCha20Rng::seed_from_u64(16);
    let (instance, w) = fresh(&s, Mini { seed: 2, broken: true }, &mut rng);
    let broken = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    assert_eq!(decides_with(&s, &plan, &broken, &w), Err(Error::Rejected("zero-check")));
    // The bound is part of the plan: a proof under one does not verify under the other.
    let proved = prove(&plan, &DOMAIN, &acc, &witness).expect("prove");
    assert!(verify(&s.plan, &DOMAIN, &acc, &proved.proof).is_err());
}

#[test]
fn a_false_copy_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    let (instance, w) = fresh(&s, Mini { seed: 2, broken: true }, &mut rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    assert_eq!(decide(&s.relation, &s.key, &acc, &w), Err(Error::Rejected("copy constraint")));
    assert_eq!(decides(&s, &acc, &w), Err(Error::Rejected("zero-check")));
    // Folded into a genuine accumulator, it still never decides.
    let (genuine, gw) = accumulator(&s);
    let (next, next_w, _) = fold(&s.relation, &DOMAIN, &genuine, &gw, &acc.instance, &w).expect("fold");
    assert!(decide(&s.relation, &s.key, &next, &next_w).is_err());
    assert!(decides(&s, &next, &next_w).is_err());
}

#[test]
fn a_lookup_input_outside_its_table_never_decides() {
    let params = PinnedParams::<Ep>::derive(forgery::K).expect("params");
    let stage = forgery::stage_with(CommitKey::monomial(params.params()));
    let plan = DeciderV2Plan::new(stage.relation.clone(), &stage.key).expect("plan");
    assert_plan_invariants(&plan);
    for balance in [forgery::Balance::InactiveReciprocal, forgery::Balance::ReciprocalPastUsable, forgery::Balance::TablePastUsable] {
        let (instance, w) = forgery::forged(&stage, balance);
        let acc = Accumulator::from_fresh(&stage.relation, &forgery::DOMAIN, instance).expect("accumulator");
        let proved = prove(&plan, &forgery::DOMAIN, &acc, &w).expect("prove");
        assert_eq!(
            verify(&plan, &forgery::DOMAIN, &acc, &proved.proof).map(|_| ()),
            Err(Error::Rejected("zero-check")),
            "{balance:?}"
        );
    }
}

#[test]
fn false_gates_wrong_errors_and_tampered_messages_are_rejected() {
    let s = setup();
    let (acc, witness) = accumulator(&s);
    // Another accumulated error.
    let mut wrong = acc.clone();
    wrong.e += Fq::ONE;
    assert_eq!(decides(&s, &wrong, &witness), Err(Error::Rejected("zero-check")));
    // A false gate witness (c on row 4), recommitted honestly and folded in.
    let mut rng = ChaCha20Rng::seed_from_u64(6);
    let (_, mut w) = fresh(&s, Mini { seed: 5, broken: false }, &mut rng);
    w.columns[2][4] += Fq::ONE;
    let public = Mini { seed: 5, broken: false }.instances();
    let advice = w.columns[..s.relation.advice_columns].to_vec();
    let (instance, w) = fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, &mut rng).expect("instance");
    let (next, next_w, _) = fold(&s.relation, &DOMAIN, &acc, &witness, &instance, &w).expect("fold");
    assert!(decide(&s.relation, &s.key, &next, &next_w).is_err());
    assert_eq!(decides(&s, &next, &next_w), Err(Error::Rejected("zero-check")));
    // A witness cell that is not the committed one: the claims do not open.
    let mut bad = witness.clone();
    bad.columns[0][5] += Fq::ONE;
    assert!(decides(&s, &acc, &bad).is_err());
    // Tampered messages.
    let proved = prove(&s.plan, &DOMAIN, &acc, &witness).expect("prove");
    let check = |proof: &super::DeciderV2Proof<Ep>| verify(&s.plan, &DOMAIN, &acc, proof).map(|_| ());
    let mut p = proved.proof.clone();
    p.zero_check[2][1] += Fq::ONE;
    assert_eq!(check(&p), Err(Error::Rejected("zero-check")));
    let mut p = proved.proof.clone();
    p.at_rho[0] += Fq::ONE;
    assert!(check(&p).is_err());
    let mut p = proved.proof.clone();
    p.shift[1][0] += Fq::ONE;
    assert_eq!(check(&p), Err(Error::Rejected("shift reduction")));
    let mut p = proved.proof.clone();
    let last = p.at_sigma.len() - 1;
    p.at_sigma[last] += Fq::ONE;
    assert_eq!(check(&p), Err(Error::Rejected("shift reduction")));
    let mut p = proved.proof;
    p.helpers.swap(0, 1);
    assert!(check(&p).is_err());
}

#[derive(Clone, Copy, Debug)]
struct PairConfig {
    x: Column<Advice>,
    y: Column<Advice>,
    q: Selector,
}

/// `y = x^2` on six rows with the copy `y[0] = x[1]`: another relation of the
/// same domain size as [`Mini`].
#[derive(Clone, Copy, Debug)]
struct Pair {
    seed: u64,
}

impl Pair {
    fn x(self, row: usize) -> u64 {
        match row {
            0 => 2,
            1 => 4,
            _ => (self.seed + row as u64) % 9,
        }
    }
}

impl Circuit<Fq> for Pair {
    type Config = PairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> PairConfig {
        let (x, y) = (meta.advice_column(), meta.advice_column());
        meta.enable_equality(x);
        meta.enable_equality(y);
        let q = meta.selector();
        meta.create_gate("square", |v| {
            let q = v.query_selector(q);
            let (x, y) = (v.query_advice(x, Rotation::cur()), v.query_advice(y, Rotation::cur()));
            vec![q * (x.clone() * x - y)]
        });
        PairConfig { x, y, q }
    }

    fn synthesize(&self, config: PairConfig, mut layouter: impl Layouter<Fq>) -> Result<(), PlonkError> {
        layouter.assign_region(
            || "squares",
            |mut region| {
                let mut cells = Vec::new();
                for row in 0..6 {
                    config.q.enable(&mut region, row)?;
                    let x = self.x(row);
                    let xc = region.assign_advice(config.x, row, Value::known(Fq::from(x)))?.cell();
                    let yc = region.assign_advice(config.y, row, Value::known(Fq::from(x * x)))?.cell();
                    cells.push((xc, yc));
                }
                region.constrain_equal(cells[0].1, cells[1].0)
            },
        )
    }
}

#[test]
fn a_batch_of_accumulators_of_two_relations_decides_with_one_opening() {
    let s = setup();
    let (acc1, w1) = accumulator(&s);
    let mut rng = ChaCha20Rng::seed_from_u64(8);
    let (instance, w2) = fresh(&s, Mini { seed: 11, broken: false }, &mut rng);
    let acc2 = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    let layout = synthesize(&Pair { seed: 1 }, K, None).expect("layout");
    let pair = FoldedRelation::from_synthesized(&layout).expect("relation");
    let pair_plan = DeciderV2Plan::new(pair.clone(), &s.key).expect("plan");
    let none: Vec<Vec<Fq>> = Vec::new();
    let synthesized = synthesize(&Pair { seed: 3 }, K, Some(&none)).expect("witness");
    let (advice, public) = pair.stage_witness(&synthesized).expect("stage witness");
    let (instance, w3) = fresh_instance(&pair, &s.key, &DOMAIN, advice, public, &mut rng).expect("fresh");
    let acc3 = Accumulator::from_fresh(&pair, &DOMAIN, instance).expect("accumulator");
    decide(&pair, &s.key, &acc3, &w3).expect("pair accumulator");

    assert_plan_invariants(&pair_plan);
    let inputs = [(&s.plan, &acc1, &w1), (&s.plan, &acc2, &w2), (&pair_plan, &acc3, &w3)];
    let accs = [(&s.plan, &acc1), (&s.plan, &acc2), (&pair_plan, &acc3)];
    let mut proved = prove_many(&DOMAIN, &inputs).expect("prove");
    let (opening, prover_claim) = open(s.params.params(), &mut proved).expect("open");
    let mut verified = verify_many(&DOMAIN, &accs, &proved.proof).expect("verify");
    assert_eq!(verified.claims, proved.claims);
    // One claim per witness column and helper of every member, and one per
    // distinct key commitment: the second Mini member opens no key of its own.
    let plans = [&s.plan, &s.plan, &pair_plan];
    let mut keys = Vec::new();
    let mut own = 0;
    for plan in plans {
        for poly in &plan.committed {
            match *poly {
                Committed::Key(p) if !keys.contains(&plan.key_commitments[p]) => keys.push(plan.key_commitments[p]),
                Committed::Key(_) => {}
                _ => own += 1,
            }
        }
    }
    assert_eq!(proved.claims.len(), own + keys.len());
    assert!(proved.claims.len() < plans.iter().map(|plan| plan.committed.len()).sum::<usize>());
    assert_eq!(proved.proof.at_sigma.len(), proved.claims.len());
    // Both Mini members read a shared key claim: another value fails the shift
    // reduction.
    let shared = proved.claims.iter().position(|claim| claim.commitment == keys[0]).expect("a key claim");
    let mut tampered = proved.proof.clone();
    tampered.at_sigma[shared] += Fq::ONE;
    assert_eq!(verify_many(&DOMAIN, &accs, &tampered).map(|_| ()), Err(Error::Rejected("shift reduction")));
    let claim = verify_opening(s.params.params(), &mut verified, &opening).expect("opening verifies");
    assert_eq!(claim, prover_claim);
    claim.decide(&s.params, MemoryBudget::DEFAULT).expect("generator claim decides");

    // The verifier names the same members in the same order.
    let swapped = [(&s.plan, &acc2), (&s.plan, &acc1), (&pair_plan, &acc3)];
    assert!(verify_many(&DOMAIN, &swapped, &proved.proof).is_err());
    assert!(verify_many(&DOMAIN, &accs[..2], &proved.proof).is_err());

    // One false member fails the whole batch.
    let mut wrong = acc2.clone();
    wrong.e += Fq::ONE;
    let proved = prove_many(&DOMAIN, &[(&s.plan, &acc1, &w1), (&s.plan, &wrong, &w2), (&pair_plan, &acc3, &w3)])
        .expect("prove");
    let accs = [(&s.plan, &acc1), (&s.plan, &wrong), (&pair_plan, &acc3)];
    assert_eq!(verify_many(&DOMAIN, &accs, &proved.proof).map(|_| ()), Err(Error::Rejected("zero-check")));
    let mut rng = ChaCha20Rng::seed_from_u64(9);
    let (instance, broken) = fresh(&s, Mini { seed: 4, broken: true }, &mut rng);
    let broken_acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    let proved = prove_many(&DOMAIN, &[(&s.plan, &acc1, &w1), (&s.plan, &broken_acc, &broken), (&pair_plan, &acc3, &w3)])
        .expect("prove");
    let accs = [(&s.plan, &acc1), (&s.plan, &broken_acc), (&pair_plan, &acc3)];
    assert_eq!(verify_many(&DOMAIN, &accs, &proved.proof).map(|_| ()), Err(Error::Rejected("zero-check")));

    // An empty batch and unequal domain sizes are refused.
    assert_eq!(prove_many::<Ep>(&DOMAIN, &[]).map(|_| ()), Err(Error::Shape("empty decider batch")));
    let params6 = PinnedParams::<Ep>::derive(forgery::K).expect("params");
    let stage = forgery::stage_with(CommitKey::monomial(params6.params()));
    let plan6 = DeciderV2Plan::new(stage.relation.clone(), &stage.key).expect("plan");
    let (instance, w6) = forgery::forged(&stage, forgery::Balance::InactiveReciprocal);
    let acc6 = Accumulator::from_fresh(&stage.relation, &forgery::DOMAIN, instance).expect("accumulator");
    assert_eq!(
        prove_many(&DOMAIN, &[(&s.plan, &acc1, &w1), (&plan6, &acc6, &w6)]).map(|_| ()),
        Err(Error::Shape("decider batch domains"))
    );
}

/// One member of a field-side batch: its plan, shape, accumulator and imported
/// instance copy term (`None` at key generation, or when not imported).
#[derive(Clone)]
struct FieldMember {
    plan: Arc<DeciderV2Plan<Ep>>,
    shape: FoldShape,
    acc: Option<Accumulator<Ep>>,
    copy_terms: Option<Fq>,
}

/// The field side of the decider over a batch of accumulators: a circuit over
/// the decider's scalar field that exposes `d_F` and the opening scalars `v*`,
/// `b'` for given `alpha` and `u`.
#[derive(Clone)]
struct FieldDeciderCircuit {
    members: Vec<FieldMember>,
    /// Whether the public columns stay out and the instance copy terms are
    /// imported.
    import_terms: bool,
    proof: Option<DeciderV2Proof<Ep>>,
    helpers: Option<Fp>,
    alpha: Option<Fq>,
    u: Option<Vec<Fq>>,
}

#[derive(Clone, Debug)]
struct FieldDeciderConfig {
    field: FieldVerifierConfig<Fq>,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fq> for FieldDeciderCircuit {
    type Config = FieldDeciderConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            members: self.members.iter().map(|m| FieldMember { acc: None, copy_terms: None, ..m.clone() }).collect(),
            import_terms: self.import_terms,
            proof: None,
            helpers: None,
            alpha: None,
            u: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> FieldDeciderConfig {
        let field = FieldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        FieldDeciderConfig { field, instance }
    }

    fn synthesize(&self, config: FieldDeciderConfig, mut layouter: impl Layouter<Fq>) -> Result<(), PlonkError> {
        let mut chips = FieldVerifierChips::new(&config.field);
        chips.load_tables(&mut layouter)?;
        let k = self.members[0].plan.mu;
        let plans: Vec<&DeciderV2Plan<Ep>> = self.members.iter().map(|m| &*m.plan).collect();
        let (digest, value, folded) = layouter.assign_region(
            || "decider v2 field side",
            |mut region| {
                let mut lanes = chips.lanes();
                let mut accs = Vec::with_capacity(self.members.len());
                for m in &self.members {
                    accs.push(if self.import_terms {
                        assign_accumulator_field_terms(&mut lanes, &mut region, &DOMAIN, &m.shape, m.acc.as_ref(), m.copy_terms)?
                    } else {
                        assign_accumulator_field(&mut lanes, &mut region, &DOMAIN, &m.shape, m.acc.as_ref())?
                    });
                }
                let limbs = self.helpers.map(|d| s6_limbs(&d).map(Fq::from_u128));
                let witness = |lanes: &mut crate::circuit::field::FieldLanes<'_, Fq>, region: &mut _, v: Option<Fq>| {
                    lanes.glue.witness(region, v.map_or_else(Value::unknown, Value::known))
                };
                let helpers = [
                    witness(&mut lanes, &mut region, limbs.map(|l| l[0]))?,
                    witness(&mut lanes, &mut region, limbs.map(|l| l[1]))?,
                ];
                let proof = assign_decider_v2_field_proof(&mut lanes, &mut region, &plans, self.proof.as_ref())?;
                let members: Vec<_> = plans.iter().copied().zip(&accs).collect();
                let out = verify_decider_v2_field(&mut lanes, &mut region, &DOMAIN, &members, &helpers, &proof)?;
                let alpha = witness(&mut lanes, &mut region, self.alpha)?;
                let u = (0..k)
                    .map(|j| witness(&mut lanes, &mut region, self.u.as_ref().map(|u| u[j])))
                    .collect::<Result<Vec<_>, _>>()?;
                let (value, folded) = opening_scalars(&mut lanes, &mut region, &out.at_sigma, &out.point, &alpha, &u)?;
                Ok((out.field_digest, value, folded))
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.instance, 0)?;
        layouter.constrain_instance(value.cell(), config.instance, 1)?;
        layouter.constrain_instance(folded.cell(), config.instance, 2)
    }
}

#[test]
#[ignore = "k16 field-side decider circuit; run optimized"]
fn the_field_side_decider_circuit_verifies_exactly_as_the_native_verifier() {
    let s = setup();
    let (acc, witness) = accumulator(&s);
    let proved = prove(&s.plan, &DOMAIN, &acc, &witness).expect("prove");
    let verified = verify(&s.plan, &DOMAIN, &acc, &proved.proof).expect("verify");
    assert_eq!(verified.field_digest, proved.field_digest);
    let helpers = helper_digest::<Ep>(&DOMAIN, &proved.proof.helpers);
    // The opening scalars for some alpha and u, against their native definitions.
    let mut rng = ChaCha20Rng::seed_from_u64(12);
    let alpha = Fq::random(&mut rng);
    let u: Vec<Fq> = (0..s.plan.mu).map(|_| Fq::random(&mut rng)).collect();
    let value = proved.proof.at_sigma.iter().rev().fold(Fq::ZERO, |acc, v| acc * alpha + v);
    let folded = pcs_mle::folded_eq(&verified.point, &u).expect("lengths");
    let public = vec![verified.field_digest, value, folded];
    let plan = Arc::new(DeciderV2Plan::new(s.relation.clone(), &s.key).expect("plan"));
    let shape = s.relation.shape();
    let with_terms = |proof: &DeciderV2Proof<Ep>, helpers: Fp, public: &[Fq], copy_terms: Option<Fq>| {
        let circuit = FieldDeciderCircuit {
            members: vec![FieldMember { plan: Arc::clone(&plan), shape: shape.clone(), acc: Some(acc.clone()), copy_terms }],
            import_terms: copy_terms.is_some(),
            proof: Some(proof.clone()),
            helpers: Some(helpers),
            alpha: Some(alpha),
            u: Some(u.clone()),
        };
        check_circuit(&circuit, 16, &[public.to_vec()], CheckMode::Strict).expect("synthesis").is_satisfied()
    };
    let satisfied = |proof: &DeciderV2Proof<Ep>, helpers: Fp, public: &[Fq]| with_terms(proof, helpers, public, None);
    assert!(satisfied(&proved.proof, helpers, &public));
    // The public columns can stay on the commitment side: the transcript binds
    // them through the digest, and the instance copy term is imported.
    assert!(!plan.reads_public_beyond_copies() && !plan.instance_copy_cells().is_empty());
    let [gamma, eta] = verified.copy_challenges;
    assert_eq!(proved.copy_challenges, verified.copy_challenges);
    let terms = instance_copy_terms(&plan, &acc, gamma, eta).expect("copy terms");
    assert!(with_terms(&proved.proof, helpers, &public, Some(terms)));
    assert!(!with_terms(&proved.proof, helpers, &public, Some(terms + Fq::ONE)));
    let mut other = public.clone();
    other[0] += Fq::ONE;
    assert!(!satisfied(&proved.proof, helpers, &other));
    // Another helper digest changes every challenge, hence d_F.
    assert!(!satisfied(&proved.proof, helpers + Fp::ONE, &public));
    // Tampered field messages: unsatisfied.
    let mut p = proved.proof.clone();
    p.zero_check[3][2] += Fq::ONE;
    assert!(!satisfied(&p, helpers, &public));
    let mut p = proved.proof.clone();
    p.at_rho[1] += Fq::ONE;
    assert!(!satisfied(&p, helpers, &public));
    let mut p = proved.proof.clone();
    p.shift[2][1] += Fq::ONE;
    assert!(!satisfied(&p, helpers, &public));
    let mut p = proved.proof;
    p.at_sigma[0] += Fq::ONE;
    assert!(!satisfied(&p, helpers, &public));
}

#[test]
#[ignore = "k16 field-side decider circuit; run optimized"]
fn the_field_side_decider_circuit_verifies_a_batch_whose_members_share_keys() {
    let s = setup();
    let (acc1, w1) = accumulator(&s);
    let mut rng = ChaCha20Rng::seed_from_u64(15);
    let (instance, w2) = fresh(&s, Mini { seed: 8, broken: false }, &mut rng);
    let acc2 = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    let layout = synthesize(&Pair { seed: 1 }, K, None).expect("layout");
    let pair = FoldedRelation::from_synthesized(&layout).expect("relation");
    let pair_plan = Arc::new(DeciderV2Plan::new(pair.clone(), &s.key).expect("plan"));
    let none: Vec<Vec<Fq>> = Vec::new();
    let synthesized = synthesize(&Pair { seed: 5 }, K, Some(&none)).expect("witness");
    let (advice, public) = pair.stage_witness(&synthesized).expect("stage witness");
    let (instance, w3) = fresh_instance(&pair, &s.key, &DOMAIN, advice, public, &mut rng).expect("fresh");
    let acc3 = Accumulator::from_fresh(&pair, &DOMAIN, instance).expect("accumulator");
    let mini_plan = Arc::new(DeciderV2Plan::new(s.relation.clone(), &s.key).expect("plan"));
    let proved =
        prove_many(&DOMAIN, &[(&*mini_plan, &acc1, &w1), (&*mini_plan, &acc2, &w2), (&*pair_plan, &acc3, &w3)])
            .expect("prove");
    let accs = [(&*mini_plan, &acc1), (&*mini_plan, &acc2), (&*pair_plan, &acc3)];
    let verified = verify_many(&DOMAIN, &accs, &proved.proof).expect("verify");
    let committed: usize = accs.iter().map(|(plan, _)| plan.committed.len()).sum();
    assert!(proved.claims.len() < committed, "the second Mini member shares its key claims");
    let helpers = helper_digest::<Ep>(&DOMAIN, &proved.proof.helpers);
    let alpha = Fq::random(&mut rng);
    let u: Vec<Fq> = (0..s.plan.mu).map(|_| Fq::random(&mut rng)).collect();
    let value = proved.proof.at_sigma.iter().rev().fold(Fq::ZERO, |acc, v| acc * alpha + v);
    let folded = pcs_mle::folded_eq(&verified.point, &u).expect("lengths");
    let public = vec![verified.field_digest, value, folded];
    let member = |plan: &Arc<DeciderV2Plan<Ep>>, relation: &FoldedRelation<Fq>, acc: &Accumulator<Ep>| FieldMember {
        plan: Arc::clone(plan),
        shape: relation.shape(),
        acc: Some(acc.clone()),
        copy_terms: None,
    };
    let members =
        vec![member(&mini_plan, &s.relation, &acc1), member(&mini_plan, &s.relation, &acc2), member(&pair_plan, &pair, &acc3)];
    let satisfied = |proof: &DeciderV2Proof<Ep>, members: &[FieldMember]| {
        let circuit = FieldDeciderCircuit {
            members: members.to_vec(),
            import_terms: false,
            proof: Some(proof.clone()),
            helpers: Some(helpers),
            alpha: Some(alpha),
            u: Some(u.clone()),
        };
        check_circuit(&circuit, 16, &[public.clone()], CheckMode::Strict).expect("synthesis").is_satisfied()
    };
    assert!(satisfied(&proved.proof, &members));
    // A shared key claim's value, read by both Mini members: unsatisfied.
    let key = mini_plan.committed.iter().find_map(|poly| match *poly {
        Committed::Key(p) => Some(mini_plan.key_commitments[p]),
        _ => None,
    });
    let shared = proved.claims.iter().position(|claim| Some(claim.commitment) == key).expect("a shared key claim");
    let mut p = proved.proof.clone();
    p.at_sigma[shared] += Fq::ONE;
    assert!(!satisfied(&p, &members));
    // The members in another order: unsatisfied.
    let swapped = vec![members[1].clone(), members[0].clone(), members[2].clone()];
    assert!(!satisfied(&proved.proof, &swapped));
    // A larger step bound: the copy permutations (about 30 changes each) are
    // evaluated in circuit too.
    let plan64 = Arc::new(DeciderV2Plan::with_step_runs(s.relation.clone(), &s.key, 64).expect("plan"));
    let proved = prove(&plan64, &DOMAIN, &acc1, &w1).expect("prove");
    let verified = verify(&plan64, &DOMAIN, &acc1, &proved.proof).expect("verify");
    let value = proved.proof.at_sigma.iter().rev().fold(Fq::ZERO, |acc, v| acc * alpha + v);
    let folded = pcs_mle::folded_eq(&verified.point, &u).expect("lengths");
    let circuit = FieldDeciderCircuit {
        members: vec![member(&plan64, &s.relation, &acc1)],
        import_terms: false,
        proof: Some(proved.proof.clone()),
        helpers: Some(helper_digest::<Ep>(&DOMAIN, &proved.proof.helpers)),
        alpha: Some(alpha),
        u: Some(u.clone()),
    };
    let public = vec![verified.field_digest, value, folded];
    assert!(check_circuit(&circuit, 16, &[public], CheckMode::Strict).expect("synthesis").is_satisfied());
}

/// The commitment side's decider pieces: the helper digest and the opening
/// transcript seeded with `d_F`, finished here to expose its state.
#[derive(Clone)]
struct GroupDeciderCircuit {
    helpers: Vec<Option<Ep>>,
    field_digest: Option<Fq>,
}

#[derive(Clone, Debug)]
struct GroupDeciderConfig {
    fold: FoldVerifierConfig<Ep>,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fp> for GroupDeciderCircuit {
    type Config = GroupDeciderConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { helpers: vec![None; self.helpers.len()], field_digest: None }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> GroupDeciderConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(2);
        meta.enable_equality(instance);
        GroupDeciderConfig { fold, instance }
    }

    fn synthesize(&self, config: GroupDeciderConfig, mut layouter: impl Layouter<Fp>) -> Result<(), PlonkError> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let (helpers, opening) = layouter.assign_region(
            || "decider v2 group side",
            |mut region| {
                let mut lanes = chips.lanes();
                let points = self
                    .helpers
                    .iter()
                    .map(|p| lanes.assign_point(&mut region, p.map_or_else(Value::unknown, Value::known)))
                    .collect::<Result<Vec<_>, _>>()?;
                let helpers = helper_digest_in_circuit(&mut lanes, &mut region, &DOMAIN, &points)?;
                let digest = lanes.assign_scalar(&mut region, self.field_digest.map_or_else(Value::unknown, Value::known))?;
                opening_transcript(&mut lanes, &mut region, &DOMAIN, &digest)?;
                let opening = circuit_transcript::finish(&mut lanes, &mut region)?;
                Ok((helpers, opening))
            },
        )?;
        layouter.constrain_instance(helpers.cell(), config.instance, 0)?;
        layouter.constrain_instance(opening.cell(), config.instance, 1)
    }
}

#[test]
#[ignore = "k16 group-side decider circuit; run optimized"]
fn the_group_side_decider_pieces_match_the_native_transcripts() {
    let s = setup();
    let (acc, witness) = accumulator(&s);
    let proved = prove(&s.plan, &DOMAIN, &acc, &witness).expect("prove");
    let mut verified = verify(&s.plan, &DOMAIN, &acc, &proved.proof).expect("verify");
    let public = vec![helper_digest::<Ep>(&DOMAIN, &proved.proof.helpers), verified.transcript.digest()];
    let circuit = GroupDeciderCircuit {
        helpers: proved.proof.helpers.iter().map(|p| Some(Ep::from(*p))).collect(),
        field_digest: Some(verified.field_digest),
    };
    assert!(check_circuit(&circuit, 16, &[public.clone()], CheckMode::Strict).expect("synthesis").is_satisfied());
    let other = GroupDeciderCircuit { field_digest: Some(verified.field_digest + Fq::ONE), ..circuit };
    assert!(!check_circuit(&other, 16, &[public], CheckMode::Strict).expect("synthesis").is_satisfied());
}

#[test]
fn decider_claims_enter_a_claim_accumulator_and_restructure() {
    use crate::claims::{Claim, ClaimAccumulator, decide_claims, fold_claim, restructure_prove, restructure_verify};
    let s = setup();
    let generators = PinnedParams::<Ep>::derive(16).expect("params");
    let (acc, witness) = accumulator(&s);
    let mut proved = prove(&s.plan, &DOMAIN, &acc, &witness).expect("prove");
    let (_, first) = open(s.params.params(), &mut proved).expect("open");
    let mut rng = ChaCha20Rng::seed_from_u64(10);
    let (instance, w) = fresh(&s, Mini { seed: 6, broken: false }, &mut rng);
    let fresh_acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    let mut proved = prove(&s.plan, &DOMAIN, &fresh_acc, &w).expect("prove");
    let (_, second) = open(s.params.params(), &mut proved).expect("open");
    // A k5 opening claim is a 16-round claim with 11 leading zero challenges.
    let first = Claim::from_generator_claim(&first).expect("claim");
    let second = Claim::from_generator_claim(&second).expect("claim");
    let g = generators.params().g();
    let (claims, s1) = ClaimAccumulator::from_claim(&DOMAIN, &first);
    decide_claims(g, &claims, &s1).expect("one decider claim decides over the 2^16 generators");
    let (claims, s2, _) = fold_claim(&DOMAIN, &claims, &s1, &second).expect("fold");
    decide_claims(g, &claims, &s2).expect("two folded decider claims decide");
    let (proof, statement) = restructure_prove(&DOMAIN, &claims, &s2).expect("restructure");
    assert_eq!(restructure_verify(&DOMAIN, &claims, &proof), Ok(statement));
    assert_eq!(statement.commitment, claims.g);
}

/// Tag of the test's digest over a commitment list.
const TAG_TEST_COMMITMENTS: u64 = u64::from_le_bytes(*b"testcmts");

/// The commitment side's claim commitments for a batch, digested in claim order.
#[derive(Clone)]
struct CommitmentsCircuit {
    plan: Arc<DeciderV2Plan<Ep>>,
    shape: FoldShape,
    accs: Vec<Option<Accumulator<Ep>>>,
    helpers: Vec<Option<Ep>>,
}

#[derive(Clone, Debug)]
struct CommitmentsConfig {
    fold: FoldVerifierConfig<Ep>,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fp> for CommitmentsCircuit {
    type Config = CommitmentsConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            plan: Arc::clone(&self.plan),
            shape: self.shape.clone(),
            accs: vec![None; self.accs.len()],
            helpers: vec![None; self.helpers.len()],
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> CommitmentsConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(1);
        meta.enable_equality(instance);
        CommitmentsConfig { fold, instance }
    }

    fn synthesize(&self, config: CommitmentsConfig, mut layouter: impl Layouter<Fp>) -> Result<(), PlonkError> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let digest = layouter.assign_region(
            || "claim commitments",
            |mut region| {
                let mut lanes = chips.lanes();
                let accs = self
                    .accs
                    .iter()
                    .map(|acc| assign_accumulator(&mut lanes, &mut region, &self.shape, acc.as_ref()))
                    .collect::<Result<Vec<_>, _>>()?;
                let helpers = self
                    .helpers
                    .iter()
                    .map(|p| lanes.assign_point(&mut region, p.map_or_else(Value::unknown, Value::known)))
                    .collect::<Result<Vec<_>, _>>()?;
                let members: Vec<_> = accs.iter().map(|acc| (&*self.plan, acc)).collect();
                let commitments = claim_commitments(&mut lanes, &mut region, &members, &helpers)?;
                circuit_transcript::start(&mut lanes, TAG_TEST_COMMITMENTS, &DOMAIN)?;
                for point in &commitments {
                    circuit_transcript::absorb_point(&mut lanes, point);
                }
                circuit_transcript::finish(&mut lanes, &mut region)
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.instance, 0)
    }
}

#[test]
#[ignore = "k16 claim-commitment circuit; run optimized"]
fn claim_commitments_follow_the_native_claim_order() {
    let s = setup();
    let (acc1, w1) = accumulator(&s);
    let mut rng = ChaCha20Rng::seed_from_u64(13);
    let (instance, w2) = fresh(&s, Mini { seed: 7, broken: false }, &mut rng);
    let acc2 = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    let proved = prove_many(&DOMAIN, &[(&s.plan, &acc1, &w1), (&s.plan, &acc2, &w2)]).expect("prove");
    let verified = verify_many(&DOMAIN, &[(&s.plan, &acc1), (&s.plan, &acc2)], &proved.proof).expect("verify");
    let mut native = Transcript::<Ep>::new(TAG_TEST_COMMITMENTS, &DOMAIN);
    for claim in &verified.claims {
        native.absorb_point(&claim.commitment);
    }
    let digest = native.digest();
    assert_eq!(s.plan.mu(), K as usize);
    assert_eq!(proved.proof.helpers.len(), 2 * s.plan.helper_count());
    let plan = Arc::new(DeciderV2Plan::new(s.relation.clone(), &s.key).expect("plan"));
    let circuit = CommitmentsCircuit {
        plan,
        shape: s.relation.shape(),
        accs: vec![Some(acc1.clone()), Some(acc2.clone())],
        helpers: proved.proof.helpers.iter().map(|p| Some(Ep::from(*p))).collect(),
    };
    assert!(check_circuit(&circuit, 16, &[vec![digest]], CheckMode::Strict).expect("synthesis").is_satisfied());
    // The members in another order give another claim order.
    let swapped = CommitmentsCircuit { accs: vec![Some(acc2), Some(acc1)], ..circuit };
    assert!(!check_circuit(&swapped, 16, &[vec![digest]], CheckMode::Strict).expect("synthesis").is_satisfied());
}
