//! Shared helpers of the step-relation tests: the M7 relation-check cases,
//! the relation shapes, strict constraint checks, in-circuit digests, key
//! caches, deterministic prover randomness and the forged-witness builder.

#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    convert::Infallible,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use ff::Field;
use iroha_kagemusha_proof::{
    Mutation, PrefixMode, ProofFormat, RelationShape, ShapePolicy, SigmaCircuit, SigmaProver,
    SigmaShape, StateLayout, StepDigests, StepRelation, StepWitness, select_shape,
};
use iroha_pasta::{Eq, Fp, PastaCurve, poseidon::PoseidonField};
use iroha_plonk::{
    ProverRandomness, Witness,
    check::{CheckFailure, CheckMode, CheckReport, check, check_circuit},
    cs::{ConstraintSystem, Expression},
    frontend::{Assembly, SingleChipLayouter, configure, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

/// The M7 relation checks: each step relation accepts an honest witness and
/// rejects an overdraft, a newer Request policy epoch, an accepted time below
/// the floor and a `u128` overflow.
pub const RELATION_CASES: [(StepRelation, Mutation, bool); 6] = [
    (StepRelation::Send, Mutation::None, true),
    (StepRelation::Send, Mutation::Overdraft, false),
    (StepRelation::Send, Mutation::StaleEpoch, false),
    (StepRelation::Send, Mutation::EarlyTime, false),
    (StepRelation::Receive, Mutation::None, true),
    (StepRelation::Receive, Mutation::Overflow, false),
];

/// The witness seed of the relation checks.
pub const CHECK_SEED: u64 = 0x4d37;

/// Every relation shape: both prefix modes, both layouts, both steps (the
/// M7 backends x layouts x steps grid).
pub fn relation_shapes() -> Vec<RelationShape> {
    let mut shapes = Vec::new();
    for prefix in [PrefixMode::Folded, PrefixMode::Absorbed] {
        for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
            for step in [StepRelation::Send, StepRelation::Receive] {
                shapes.push(RelationShape::new(step, layout, prefix));
            }
        }
    }
    shapes
}

/// The shape a policy selects for `relation` (on Vesta).
pub fn shape(relation: RelationShape, policy: &ShapePolicy) -> SigmaShape {
    select_shape::<Eq>(relation, policy)
        .unwrap_or_else(|error| panic!("{}: {error}", relation.label()))
        .shape
}

/// The smallest shape that fits `relation`.
pub fn smallest_shape(relation: RelationShape) -> SigmaShape {
    shape(relation, &ShapePolicy::smallest_k())
}

/// The shape meeting the 3.5 KB budget.
pub fn budget_shape(relation: RelationShape) -> SigmaShape {
    shape(relation, &ShapePolicy::default())
}

/// The strict checker report of `witness` under `shape`, with the public
/// outputs the circuit computes.
pub fn check_witness<F: PoseidonField>(
    shape: &SigmaShape,
    witness: &StepWitness<F>,
) -> CheckReport<F> {
    let public = witness.evaluate(shape.params.relation().layout).public();
    let circuit = SigmaCircuit::new(shape.params, witness.clone());
    check_circuit(&circuit, shape.k, &[public.instance()], CheckMode::Strict)
        .unwrap_or_else(|error| panic!("synthesis: {error}"))
}

/// The digests the circuit computes for `witness` (a synthesis with known
/// values; every hash is also compared with the native reference inside).
pub fn in_circuit_digests<F: PoseidonField>(
    shape: &SigmaShape,
    witness: &StepWitness<F>,
) -> StepDigests<F> {
    let public = witness.evaluate(shape.params.relation().layout).public();
    let circuit = SigmaCircuit::new(shape.params, witness.clone());
    let (cs, config) = configure(&circuit).expect("configure");
    let instances = [public.instance()];
    let mut assembly = Assembly::new(&cs, shape.k, Some(&instances)).expect("assembly");
    let mut layouter = SingleChipLayouter::new(&mut assembly, cs.constants().to_vec());
    let output = circuit
        .lay_out(config, &mut layouter)
        .unwrap_or_else(|error| panic!("lay out: {error}"));
    let mut digests = None;
    let _ = output.digests.map(|value| digests = Some(value));
    digests.expect("known digests")
}

/// A deterministic recovery stream (tests only: the derivation is public).
pub fn recovery(seed: u8) -> ProverRandomness<'static> {
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

/// Vesta parameters, derived once per `k`.
pub fn vesta_params(k: u32) -> PinnedParams<Eq> {
    static CACHE: OnceLock<Mutex<BTreeMap<u32, PinnedParams<Eq>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut cache = cache.lock().expect("params cache");
    cache
        .entry(k)
        .or_insert_with(|| PinnedParams::derive(k).expect("params"))
        .clone()
}

/// Keys of `shape` on Vesta in the KAGEMUSHA step format.
pub fn vesta_prover(shape: SigmaShape) -> SigmaProver<Eq> {
    SigmaProver::keygen_with_params(shape, ProofFormat::default(), vesta_params(shape.k))
        .unwrap_or_else(|error| panic!("keygen: {error}"))
}

/// The selectors `(q_step, q_limb, q_shift)` of the running-sum chip: the
/// two selectors of the limb lookup's input (in order) and the selector of
/// the shifted-top-limb gate.
pub fn range_selectors<F: PoseidonField>(cs: &ConstraintSystem<F>) -> (usize, usize, usize) {
    fn collect<F>(expression: &Expression<F>, found: &mut Vec<usize>) {
        match expression {
            Expression::Selector(selector) => found.push(selector.index()),
            Expression::Negated(inner) | Expression::Scaled(inner, _) => collect(inner, found),
            Expression::Sum(left, right) | Expression::Product(left, right) => {
                collect(left, found);
                collect(right, found);
            }
            _ => {}
        }
    }
    let lookup = cs.lookups().first().expect("the running-sum lookup");
    let mut found = Vec::new();
    collect(&lookup.input_expressions()[0], &mut found);
    let shift = cs
        .gates()
        .iter()
        .find(|gate| gate.name() == "running-sum shifted top limb")
        .and_then(|gate| gate.queried_selectors().first())
        .expect("the shift gate")
        .index();
    (found[0], found[1], shift)
}

/// The column of the running-sum chip's advice queries in the limb lookup.
fn range_column<F: PoseidonField>(cs: &ConstraintSystem<F>) -> usize {
    fn first_advice<F>(expression: &Expression<F>) -> Option<usize> {
        match expression {
            Expression::Advice(query) => Some(query.column_index),
            Expression::Negated(inner) | Expression::Scaled(inner, _) => first_advice(inner),
            Expression::Sum(left, right) | Expression::Product(left, right) => {
                first_advice(left).or_else(|| first_advice(right))
            }
            _ => None,
        }
    }
    let lookup = cs.lookups().first().expect("the running-sum lookup");
    first_advice(&lookup.input_expressions()[0]).expect("an advice query")
}

/// The rows of the range checks that contain a failing lookup row.
fn failing_checks(
    selectors: &[Vec<bool>],
    (q_step, q_limb, q_shift): (usize, usize, usize),
    failing_rows: &[usize],
) -> Vec<(usize, usize)> {
    let on = |selector: usize, row: usize| {
        selectors
            .get(selector)
            .and_then(|rows| rows.get(row))
            .copied()
            .unwrap_or(false)
    };
    let mut checks = Vec::new();
    for &row in failing_rows {
        // A shifted top-limb row follows its check's top-limb row.
        let top_guess = if on(q_shift, row) { row - 1 } else { row };
        let mut start = top_guess;
        while start > 0 && on(q_step, start - 1) {
            start -= 1;
        }
        let mut top = start;
        while on(q_step, top) {
            top += 1;
        }
        assert!(on(q_limb, top), "row {top} closes a range check");
        let last = if on(q_shift, top + 1) { top + 1 } else { top };
        if !checks.contains(&(start, last)) {
            checks.push((start, last));
        }
    }
    checks
}

/// A forged witness, the public outputs it claims, and the strict checker
/// failures of the unrepaired witness.
pub type Forged<F> = (Witness<F>, Vec<F>, Vec<CheckFailure<F>>);

/// A forged witness for a relation-violating `witness`: its synthesized
/// advice with every range check that fails the limb lookup zeroed, so all
/// lookups pass and the engine produces a proof. The zeroed `z_0` breaks the
/// copy from the checked value (the only way around the range check), so a
/// sound verifier must reject the proof. Returns the witness, the public
/// outputs the forger claims and the failures the strict checker reported
/// before the repair.
pub fn forged_witness<C: PastaCurve>(
    prover: &SigmaProver<C>,
    witness: &StepWitness<C::ScalarExt>,
) -> Forged<C::ScalarExt>
where
    C::ScalarExt: PoseidonField,
{
    let shape = prover.shape();
    let public = witness.evaluate(shape.params.relation().layout).public();
    let instances = vec![public.instance()];
    let circuit = prover.circuit(witness).expect("circuit");
    let mut synthesized = synthesize(&circuit, shape.k, Some(&instances)).expect("synthesis");
    let report = check(&synthesized.cs, &synthesized.tables, CheckMode::Strict).expect("check");
    let failures = report.failures().to_vec();
    let failing_rows: Vec<usize> = failures
        .iter()
        .map(|failure| match failure {
            CheckFailure::LookupInputMissing { location, .. } => location.row,
            other => panic!("only range checks may fail: {other}"),
        })
        .collect();
    let selectors = range_selectors(&synthesized.cs);
    let column = range_column(&synthesized.cs);
    let checks = failing_checks(synthesized.tables.selectors(), selectors, &failing_rows);
    assert!(!checks.is_empty(), "a mutated witness fails a range check");
    let mut advice = synthesized.tables.take_advice().expect("advice");
    for (start, last) in checks {
        for value in &mut advice[column][start..=last] {
            *value = C::ScalarExt::ZERO;
        }
    }
    let forged = Witness::from_columns(prover.proving_key(), advice, instances.clone())
        .expect("forged witness shape");
    (forged, public.instance(), failures)
}

/// The repository root.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A one-line label of a case.
pub fn case_label(relation: RelationShape, mutation: Mutation) -> String {
    format!("{} {mutation:?}", relation.label())
}

/// A Vesta witness of the relation-check grid.
pub fn check_witness_of(step: StepRelation, mutation: Mutation) -> StepWitness<Fp> {
    iroha_kagemusha_proof::sample_witness(CHECK_SEED, step, mutation)
}
