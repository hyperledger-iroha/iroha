//! The v2 decider against the native one: honest accumulators reduce to claims
//! whose batched opening verifies and whose generator claim decides; false
//! copies, lookup inputs outside their tables, false gates, wrong errors and
//! tampered messages do not.

use ff::Field;
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

use super::{DeciderV2Plan, mle, open, prove, prove_many, sumcheck, verify, verify_many, verify_opening};
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
    assert_eq!(helpers, 4, "copy columns a, b, c and the constants column");
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

    let inputs = [(&s.plan, &acc1, &w1), (&s.plan, &acc2, &w2), (&pair_plan, &acc3, &w3)];
    let accs = [(&s.plan, &acc1), (&s.plan, &acc2), (&pair_plan, &acc3)];
    let mut proved = prove_many(&DOMAIN, &inputs).expect("prove");
    let (opening, prover_claim) = open(s.params.params(), &mut proved).expect("open");
    let mut verified = verify_many(&DOMAIN, &accs, &proved.proof).expect("verify");
    assert_eq!(verified.claims, proved.claims);
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
