//! `LogUp` sums count exactly the cells the relation constrains: each `h_s` on
//! its slot's cells, `g` on the usable rows. A forger who leaves a lookup input
//! outside its table (`m = g = 0`, every reciprocal cell genuine) and balances
//! the sum with a cell no constraint touches is rejected by the native deciders
//! and by the decider circuit alike.

use std::sync::Arc;

use ff::Field;
use iroha_pasta::{Ep, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Advice, Column, ConstraintSystem, Rotation, Selector, TableColumn},
    frontend::{Circuit, Error as PlonkError, Layouter, SimpleFloorPlanner, Value, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use super::{decide, decide_fresh};
use crate::{
    Error,
    commit::{CommitKey, Witness},
    decider::{Decider, DeciderPlan, decider_instances, decider_witness},
    fold::{Accumulator, Instance, fresh_instance},
    logup,
    program::Point,
    relation::FoldedRelation,
};

const K: u32 = 6;
const DOMAIN: [u8; 32] = *b"iroha_fold LogUp support domain!";
/// Rows with an active lookup; the other usable rows hold no reciprocal cell
/// (but one, which keeps the zero tuple checked).
const LOOKUPS: usize = 4;
const NON_MEMBER: u64 = 1000;

#[derive(Clone, Copy, Debug)]
struct Config {
    a: Column<Advice>,
    q: Selector,
    table: TableColumn,
}

/// `q * a` in `0..16`, with `q` set on the first [`LOOKUPS`] rows only.
#[derive(Clone, Copy, Debug)]
struct Sparse;

impl Circuit<Fq> for Sparse {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> Config {
        let a = meta.advice_column();
        let q = meta.complex_selector();
        let table = meta.lookup_table_column();
        meta.lookup("range", |v| {
            let q = v.query_selector(q);
            let a = v.query_advice(a, Rotation::cur());
            vec![(q * a, table)]
        });
        Config { a, q, table }
    }

    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fq>) -> Result<(), PlonkError> {
        layouter.assign_table(
            || "range",
            |mut table| {
                for i in 0..16u64 {
                    let row = usize::try_from(i).expect("small");
                    table.assign_cell(|| "i", config.table, row, || Value::known(Fq::from(i)))?;
                }
                Ok(())
            },
        )?;
        layouter.assign_region(
            || "rows",
            |mut region| {
                for row in 0..LOOKUPS {
                    region.enable_selector(|| "q", &config.q, row)?;
                    let value = u64::try_from(row).expect("small") + 2;
                    region.assign_advice(config.a, row, Value::known(Fq::from(value)))?;
                }
                Ok(())
            },
        )
    }
}

struct Stage {
    relation: FoldedRelation<Fq>,
    key: CommitKey<Ep>,
    advice: Vec<Vec<Fq>>,
    public: Vec<Vec<Fq>>,
}

fn stage() -> Stage {
    let layout = synthesize(&Sparse, K, None).expect("layout");
    let relation = FoldedRelation::from_synthesized(&layout).expect("relation");
    let none: Vec<Vec<Fq>> = Vec::new();
    let synthesized = synthesize(&Sparse, K, Some(&none)).expect("witness");
    let (advice, public) = relation.stage_witness(&synthesized).expect("stage witness");
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    Stage { relation, key: CommitKey::lagrange(params.params()), advice, public }
}

/// Where a forger puts the value that balances the `LogUp` sum.
#[derive(Clone, Copy, Debug)]
enum Balance {
    /// A reciprocal cell on a usable row where no member is active.
    InactiveReciprocal,
    /// A reciprocal cell past the usable rows.
    ReciprocalPastUsable,
    /// A table cell past the usable rows.
    TablePastUsable,
}

/// A fresh instance with its own challenges whose lookup input on row 1 is not
/// in the table: `m = g = 0`, every reciprocal cell genuine, and one cell no
/// constraint touches balancing the sum.
fn forged(stage: &Stage, balance: Balance) -> (Instance<Ep>, Witness<Fq>) {
    let relation = &stage.relation;
    let group = &relation.groups[0];
    let mut rng = ChaCha20Rng::seed_from_u64(77);
    let mut columns = stage.advice.clone();
    columns[0][1] = Fq::from(NON_MEMBER);
    columns.resize(relation.columns(), vec![Fq::ZERO; relation.n]);
    let blinds: Vec<Fq> = (0..relation.columns()).map(|_| Fq::random(&mut rng)).collect();
    let commit = |column: usize, columns: &[Vec<Fq>]| {
        stage.key.commit(&columns[column], &blinds[column]).expect("commit")
    };
    let mut commitments: Vec<_> = (0..relation.columns()).map(|column| commit(column, &columns)).collect();
    // Round one draws theta and r from the advice and m commitments only.
    let probe = Instance::<Ep> {
        commitments: commitments.clone(),
        public: stage.public.clone(),
        theta: Vec::new(),
        r: Vec::new(),
    };
    let (theta, challenges, _) = probe.transcript(relation, &DOMAIN).expect("transcript");
    let tables = relation.tables();
    let point = Point { advice: &columns[..relation.advice_columns], instance: &stage.public };
    let (mut reciprocals, mut table) =
        logup::reciprocals(group, &tables, point, &columns[group.m_column()], &theta, challenges[0])
            .expect("reciprocals");
    assert!(table.iter().all(|v| bool::from(v.is_zero())), "no table row is used");
    let total: Fq = reciprocals.iter().flatten().sum();
    let usable = relation.usable;
    match balance {
        Balance::InactiveReciprocal => {
            let row = (0..usable)
                .find(|row| group.slots[0].iter().all(|cell| cell.row as usize != *row))
                .expect("an inactive usable row");
            reciprocals[0][row] -= total;
        }
        Balance::ReciprocalPastUsable => reciprocals[0][usable] -= total,
        Balance::TablePastUsable => table[usable] += total,
    }
    for (slot, column) in reciprocals.into_iter().enumerate() {
        columns[group.h_column(slot)] = column;
    }
    columns[group.g_column()] = table;
    for column in (group.m_column() + 1)..=group.g_column() {
        commitments[column] = commit(column, &columns);
    }
    let instance = Instance { commitments, public: stage.public.clone(), theta, r: challenges };
    assert!(instance.has_fresh_challenges(relation, &DOMAIN));
    (instance, Witness { columns, blinds })
}

fn circuit_satisfied(relation: &FoldedRelation<Fq>, acc: &Accumulator<Ep>, witness: &Witness<Fq>) -> bool {
    let plan = Arc::new(DeciderPlan::new(relation.clone()).expect("plan"));
    let instances = decider_instances(&plan, acc);
    let circuit = Decider::new(Arc::clone(&plan), decider_witness(&plan, acc, witness));
    check_circuit(&circuit, K, &instances, CheckMode::Strict).expect("synthesis").is_satisfied()
}

#[test]
fn honest_sparse_lookups_decide() {
    let s = stage();
    let mut rng = ChaCha20Rng::seed_from_u64(76);
    let (instance, witness) =
        fresh_instance(&s.relation, &s.key, &DOMAIN, s.advice.clone(), s.public.clone(), &mut rng).expect("fresh");
    decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness).expect("honest instance");
    let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
    decide(&s.relation, &s.key, &acc, &witness).expect("honest accumulator");
    assert!(circuit_satisfied(&s.relation, &acc, &witness));
}

#[test]
fn no_unconstrained_cell_balances_a_lookup_input_outside_its_table() {
    let s = stage();
    for balance in [Balance::InactiveReciprocal, Balance::ReciprocalPastUsable, Balance::TablePastUsable] {
        let (instance, witness) = forged(&s, balance);
        assert_eq!(
            decide_fresh(&s.relation, &s.key, &DOMAIN, &instance, &witness),
            Err(Error::Rejected("LogUp sum")),
            "{balance:?}"
        );
        let acc = Accumulator::from_fresh(&s.relation, &DOMAIN, instance).expect("accumulator");
        assert_eq!(decide(&s.relation, &s.key, &acc, &witness), Err(Error::Rejected("LogUp sum")), "{balance:?}");
        assert!(!circuit_satisfied(&s.relation, &acc, &witness), "{balance:?}");
    }
}
