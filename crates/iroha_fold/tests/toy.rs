//! Folding a small circuit with every relation feature: selectors, a fixed
//! coefficient, a rotation, a degree-three gate, a tuple lookup into a fixed
//! table, two single-column lookups sharing one table (shared reciprocal cells), a
//! dynamic advice table, copies to an instance cell and to a constant.

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation, Witness,
    decide::{decide, decide_fresh},
    fold::{fold, fresh_instance, verify_fold},
};
use iroha_pasta::{Ep, Fq};
use iroha_plonk::{
    cs::{
        Advice, Column, ConstraintSystem, Expression, Fixed, Instance, Rotation, Selector,
        TableColumn,
    },
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const K: u32 = 7;
const ROWS: usize = 24;
const DOMAIN: [u8; 32] = *b"iroha_fold toy relation domain!!";

#[derive(Clone, Copy, Debug)]
struct Config {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    d: Column<Advice>,
    e: Column<Advice>,
    cubes: Column<Advice>,
    coeff: Column<Fixed>,
    instance: Column<Instance>,
    mul: Selector,
    add: Selector,
    rot: Selector,
    cube: Selector,
    lookup: Selector,
    values: TableColumn,
    squares: TableColumn,
}

/// Row `i` holds `x_i, x_i^2, x_i^3`, `d_i = b_{i+1} - b_i`, `e_i = a_i + 5 b_i`.
#[derive(Clone, Copy, Debug)]
struct Toy {
    seed: u64,
}

impl Toy {
    fn x(self, row: usize) -> u64 {
        if row == 0 {
            3
        } else {
            (row as u64 * 5 + self.seed) % 15
        }
    }
    fn instances(self) -> Vec<Vec<Fq>> {
        let last = self.x(ROWS - 1);
        vec![vec![Fq::from(self.x(0)), Fq::from(last * last * last)]]
    }
}

// Column names follow the gate equations (a, b, c, d, e).
#[allow(clippy::many_single_char_names)]
impl Circuit<Fq> for Toy {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> Config {
        let [a, b, c, d, e, cubes] = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let coeff = meta.fixed_column();
        let instance = meta.instance_column(2);
        for column in [a, b, c] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let (mul, add, rot, cube) = (
            meta.selector(),
            meta.selector(),
            meta.selector(),
            meta.selector(),
        );
        let lookup = meta.complex_selector();
        let values = meta.lookup_table_column();
        let squares = meta.lookup_table_column();
        meta.create_gate("mul", |v| {
            let s = v.query_selector(mul);
            let (a, b, c) = (
                v.query_advice(a, Rotation::cur()),
                v.query_advice(b, Rotation::cur()),
                v.query_advice(c, Rotation::cur()),
            );
            vec![s * (a * b - c)]
        });
        meta.create_gate("add", |v| {
            let s = v.query_selector(add);
            let k = v.query_fixed(coeff, Rotation::cur());
            let (a, b, e) = (
                v.query_advice(a, Rotation::cur()),
                v.query_advice(b, Rotation::cur()),
                v.query_advice(e, Rotation::cur()),
            );
            vec![s * (a + k * b - e)]
        });
        meta.create_gate("rot", |v| {
            let s = v.query_selector(rot);
            let (b, next, d) = (
                v.query_advice(b, Rotation::cur()),
                v.query_advice(b, Rotation::next()),
                v.query_advice(d, Rotation::cur()),
            );
            vec![s * (next - b - d)]
        });
        meta.create_gate("cube", |v| {
            let s = v.query_selector(cube);
            let (a, c) = (
                v.query_advice(a, Rotation::cur()),
                v.query_advice(c, Rotation::cur()),
            );
            vec![s * (c - a.clone() * a.clone() * a)]
        });
        meta.lookup("pairs", |v| {
            let s = v.query_selector(lookup);
            let (a, b) = (
                v.query_advice(a, Rotation::cur()),
                v.query_advice(b, Rotation::cur()),
            );
            vec![(s.clone() * a, values), (s * b, squares)]
        });
        meta.lookup_any("range a", |v| {
            let s = v.query_selector(lookup);
            let a = v.query_advice(a, Rotation::cur());
            vec![(
                s * (a + Expression::Constant(Fq::ONE)),
                v.query_fixed(values.inner(), Rotation::cur()),
            )]
        });
        meta.lookup_any("range d", |v| {
            let s = v.query_selector(lookup);
            let a = v.query_advice(a, Rotation::cur());
            vec![(
                s * (a * Expression::Constant(Fq::ONE)),
                v.query_fixed(values.inner(), Rotation::cur()),
            )]
        });
        meta.lookup_any("cubes", |v| {
            let s = v.query_selector(lookup);
            let c = v.query_advice(c, Rotation::cur());
            vec![(s * c, v.query_advice(cubes, Rotation::cur()))]
        });
        Config {
            a,
            b,
            c,
            d,
            e,
            cubes,
            coeff,
            instance,
            mul,
            add,
            rot,
            cube,
            lookup,
            values,
            squares,
        }
    }

    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        layouter.assign_table(
            || "squares",
            |mut table| {
                for i in 0..16u64 {
                    let row = usize::try_from(i).expect("small");
                    table.assign_cell(|| "i", config.values, row, || Value::known(Fq::from(i)))?;
                    table.assign_cell(
                        || "i^2",
                        config.squares,
                        row,
                        || Value::known(Fq::from(i * i)),
                    )?;
                }
                Ok(())
            },
        )?;
        let last = layouter.assign_region(
            || "rows",
            |mut region| {
                let mut last = None;
                for row in 0..ROWS {
                    let x = self.x(row);
                    let (b, c) = (x * x, x * x * x);
                    config.mul.enable(&mut region, row)?;
                    config.add.enable(&mut region, row)?;
                    config.cube.enable(&mut region, row)?;
                    region.enable_selector(|| "lookup", &config.lookup, row)?;
                    region.assign_fixed(config.coeff, row, Fq::from(5))?;
                    if row == 0 {
                        region.assign_advice_from_instance(
                            || "x0",
                            config.instance,
                            0,
                            config.a,
                            0,
                        )?;
                        region.assign_advice_from_constant(|| "nine", config.b, 0, Fq::from(9))?;
                    } else {
                        region.assign_advice(config.a, row, Value::known(Fq::from(x)))?;
                        region.assign_advice(config.b, row, Value::known(Fq::from(b)))?;
                    }
                    last = Some(
                        region
                            .assign_advice(config.c, row, Value::known(Fq::from(c)))?
                            .cell(),
                    );
                    region.assign_advice(config.e, row, Value::known(Fq::from(x + 5 * b)))?;
                    if row + 1 < ROWS {
                        config.rot.enable(&mut region, row)?;
                        let next = self.x(row + 1);
                        let d = Fq::from(next * next) - Fq::from(b);
                        region.assign_advice(config.d, row, Value::known(d))?;
                    }
                }
                for i in 0..16u64 {
                    region.assign_advice(
                        config.cubes,
                        usize::try_from(i).expect("small"),
                        Value::known(Fq::from(i * i * i)),
                    )?;
                }
                last.ok_or(Error::Synthesis)
            },
        )?;
        layouter.constrain_instance(last, config.instance, 1)
    }
}

struct Setup {
    relation: FoldedRelation<Fq>,
    key: CommitKey<Ep>,
}

fn setup() -> Setup {
    let synthesized = synthesize(&Toy { seed: 1 }, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&synthesized).expect("relation");
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    Setup {
        relation,
        key: CommitKey::lagrange(params.params()),
    }
}

fn fresh(
    setup: &Setup,
    seed: u64,
    rng: &mut ChaCha20Rng,
) -> (iroha_fold::Instance<Ep>, Witness<Fq>) {
    let toy = Toy { seed };
    let synthesized = synthesize(&toy, K, Some(&toy.instances())).expect("witness");
    let (advice, public) = setup
        .relation
        .stage_witness(&synthesized)
        .expect("stage witness");
    fresh_instance(&setup.relation, &setup.key, &DOMAIN, advice, public, rng)
        .expect("fresh instance")
}

#[test]
fn relation_shape() {
    let s = setup();
    assert_eq!(s.relation.n(), 1 << K);
    // Gates are degree 3; shared single-column cells (two range inputs) are
    // degree 1 + 2; the tuple lookup is 1 + 2.
    assert_eq!(s.relation.degree(), 3);
    // Three groups: pairs (tuple), the two range lookups (shared), cubes (dynamic).
    assert_eq!(s.relation.groups(), 3);
    assert_eq!(s.relation.theta_len(), 1);
}

#[test]
fn fresh_instances_decide() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    for seed in 1..4 {
        let (instance, witness) = fresh(&s, seed, &mut rng);
        decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness)
            .expect("genuine fresh instance");
    }
}

#[test]
fn folds_decide_and_verifier_matches_prover() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    let (first, mut witness) = fresh(&s, 1, &mut rng);
    let mut acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    decide(&s.relation, &s.key, &acc, &witness).expect("initial accumulator");
    for seed in 2..8 {
        let (instance, fresh_witness) = fresh(&s, seed, &mut rng);
        let (next, next_witness, proof) = fold(
            &s.relation,
            &DOMAIN,
            &acc,
            &witness,
            &instance,
            &fresh_witness,
        )
        .expect("fold");
        let verified = verify_fold(&s.relation, &DOMAIN, &acc, &instance, &proof).expect("verify");
        assert_eq!(
            verified, next,
            "native verifier recomputes the prover's accumulator"
        );
        decide(&s.relation, &s.key, &next, &next_witness).expect("folded accumulator decides");
        acc = next;
        witness = next_witness;
    }
}

/// Folds `bad` into a valid accumulator and returns whether the result decides.
fn folded_decides(
    s: &Setup,
    bad: (iroha_fold::Instance<Ep>, Witness<Fq>),
    rng: &mut ChaCha20Rng,
) -> bool {
    let (first, witness) = fresh(s, 1, rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    let (instance, fresh_witness) = bad;
    match fold(
        &s.relation,
        &DOMAIN,
        &acc,
        &witness,
        &instance,
        &fresh_witness,
    ) {
        Ok((next, next_witness, _)) => decide(&s.relation, &s.key, &next, &next_witness).is_ok(),
        Err(_) => false,
    }
}

#[test]
fn a_false_gate_witness_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let (_, mut witness) = fresh(&s, 2, &mut rng);
    // Break the mul gate on row 4 (column e is outside every lookup) and recommit
    // honestly: the instance stays well formed, the relation is false.
    witness.columns[4][4] += Fq::ONE;
    let toy = Toy { seed: 2 };
    let public = toy.instances();
    let (instance, witness) = fresh_instance(
        &s.relation,
        &s.key,
        &DOMAIN,
        witness.columns[..6].to_vec(),
        public,
        &mut rng,
    )
    .expect("instance");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).is_err());
    assert!(!folded_decides(&s, (instance, witness), &mut rng));
}

#[test]
fn a_false_public_input_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(4);
    let toy = Toy { seed: 3 };
    let synthesized = synthesize(&toy, K, Some(&toy.instances())).expect("witness");
    let (advice, mut public) = s
        .relation
        .stage_witness(&synthesized)
        .expect("stage witness");
    public[0][1] += Fq::ONE;
    let bad =
        fresh_instance(&s.relation, &s.key, &DOMAIN, advice, public, &mut rng).expect("instance");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &bad.0, &bad.1).is_err());
    assert!(!folded_decides(&s, bad, &mut rng));
}

#[test]
fn a_false_reciprocal_never_decides() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    let (mut instance, mut witness) = fresh(&s, 4, &mut rng);
    // Change one reciprocal of the shared range group after its challenges and
    // recommit it: the round-one data, hence the challenges, are unchanged.
    // Columns: six advice, then per group m, h_0, g: pairs 6..=8, the shared range
    // group 9..=11, cubes 12..=14. Column 10 is the range group's h_0.
    assert_eq!(s.relation.columns(), 15);
    let column = 10;
    let row = witness.columns[column]
        .iter()
        .position(|v| !bool::from(v.is_zero()))
        .expect("nonzero cell");
    witness.columns[column][row] += Fq::ONE;
    instance.commitments[column] = s
        .key
        .commit(&witness.columns[column], &witness.blinds[column])
        .expect("commit");
    assert!(decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).is_err());
    assert!(!folded_decides(&s, (instance, witness), &mut rng));
}

#[test]
fn tampered_messages_or_challenges_are_rejected() {
    let s = setup();
    let mut rng = ChaCha20Rng::seed_from_u64(6);
    let (first, witness) = fresh(&s, 1, &mut rng);
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, first).expect("accumulator");
    let (instance, fresh_witness) = fresh(&s, 5, &mut rng);
    let (next, next_witness, proof) = fold(
        &s.relation,
        &DOMAIN,
        &acc,
        &witness,
        &instance,
        &fresh_witness,
    )
    .expect("fold");
    // A changed F or K coefficient changes the verifier's accumulator; the honest
    // witness no longer decides it.
    for which in 0..2 {
        let mut bad = proof.clone();
        if which == 0 {
            bad.f[0] += Fq::ONE
        } else {
            bad.k[0] += Fq::ONE
        }
        let verified = verify_fold(&s.relation, &DOMAIN, &acc, &instance, &bad).expect("verify");
        assert_ne!(verified, next);
        assert!(decide(&s.relation, &s.key, &verified, &next_witness).is_err());
    }
    // Foreign LogUp challenges are refused before folding.
    let mut foreign = instance.clone();
    foreign.r[0] += Fq::ONE;
    assert!(verify_fold(&s.relation, &DOMAIN, &acc, &foreign, &proof).is_err());
    assert!(
        fold(
            &s.relation,
            &DOMAIN,
            &acc,
            &witness,
            &foreign,
            &fresh_witness
        )
        .is_err()
    );
    // Another domain derives other challenges, so the fold does not verify there.
    let other = *b"another relation domain of tests";
    assert!(verify_fold(&s.relation, &other, &acc, &instance, &proof).is_err());
}
