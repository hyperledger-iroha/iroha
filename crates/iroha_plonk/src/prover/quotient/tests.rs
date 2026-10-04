//! The compiled evaluator against two independent evaluators: direct tree
//! evaluation of the finalized constraint system (exact values), and the
//! constraint checker over the uncompressed source expressions (which rows
//! fail), on honest, tampered and random assignments.

use std::collections::BTreeSet;

use ff::Field;
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField};
use rand_chacha::ChaCha20Rng;
use rand_core_06::SeedableRng;

use super::*;
use crate::{
    check::{CheckFailure, CheckMode, check},
    cs::{
        AdviceQuery, Expression, ExpressionEvaluator, FixedQuery, InstanceQuery, Selector,
        descriptor::ExprNodeV1,
    },
    frontend::{Circuit, synthesize},
    test_circuits::{Arithmetic, CHOICES, K, Lookups, Permutations, setup},
};

/// Direct evaluation of a finalized expression on one row of evaluation-form
/// columns, rotations wrapping modulo `n`.
struct RowEvaluator<'a, F> {
    fixed: &'a [Vec<F>],
    advice: &'a [Vec<F>],
    instance: &'a [Vec<F>],
    row: usize,
}

impl<F: PastaField> RowEvaluator<'_, F> {
    fn read(&self, columns: &[Vec<F>], column: usize, rotation: i32) -> F {
        let n = i64::try_from(columns[column].len()).expect("small");
        let row = i64::try_from(self.row).expect("small") + i64::from(rotation);
        columns[column][usize::try_from(row.rem_euclid(n)).expect("in range")]
    }
}

impl<F: PastaField> ExpressionEvaluator<F> for RowEvaluator<'_, F> {
    type Output = F;

    fn constant(&mut self, value: &F) -> F {
        *value
    }

    fn selector(&mut self, _selector: Selector) -> F {
        unreachable!("finalized constraint systems have no virtual selectors")
    }

    fn fixed(&mut self, query: FixedQuery) -> F {
        self.read(self.fixed, query.column_index, query.rotation.0)
    }

    fn advice(&mut self, query: AdviceQuery) -> F {
        self.read(self.advice, query.column_index, query.rotation.0)
    }

    fn instance(&mut self, query: InstanceQuery) -> F {
        self.read(self.instance, query.column_index, query.rotation.0)
    }

    fn negated(&mut self, value: F) -> F {
        -value
    }

    fn sum(&mut self, left: F, right: F) -> F {
        left + right
    }

    fn product(&mut self, left: F, right: F) -> F {
        left * right
    }

    fn scaled(&mut self, value: F, factor: &F) -> F {
        value * factor
    }
}

/// Evaluation-form tables of one synthesis run: fixed (with selector
/// columns), advice and zero-padded instance columns.
struct Tables<F> {
    fixed: Vec<Vec<F>>,
    advice: Vec<Vec<F>>,
    instance: Vec<Vec<F>>,
}

fn tables<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    pk: &ProvingKey<C>,
    circuit: &Ci,
    instances: &[Vec<C::ScalarExt>],
) -> Tables<C::ScalarExt> {
    let n = pk.binding().n();
    let synthesized = synthesize(circuit, K, Some(instances)).expect("synthesis");
    Tables {
        fixed: pk.fixed_values().to_vec(),
        advice: synthesized.tables.advice().expect("witness").to_vec(),
        instance: instances
            .iter()
            .map(|column| {
                let mut padded = column.clone();
                padded.resize(n, C::ScalarExt::ZERO);
                padded
            })
            .collect(),
    }
}

/// Compiled gate values equal direct evaluation of the finalized gates, on
/// `tables` and on a random replacement of every column.
fn compiled_matches_direct<C: PastaCurve>(pk: &ProvingKey<C>, tables: &Tables<C::ScalarExt>) {
    let n = pk.binding().n();
    let compiled = CompiledExpressions::compile(pk.binding().descriptor(), true).expect("compile");
    let cs = pk.constraint_system().constraint_system();
    let polys: Vec<&Expression<C::ScalarExt>> = cs
        .gates()
        .iter()
        .flat_map(crate::cs::Gate::polynomials)
        .collect();
    assert_eq!(compiled.gate_count(), polys.len());
    let mut rng = ChaCha20Rng::seed_from_u64(n as u64);
    let random = |columns: &[Vec<C::ScalarExt>], rng: &mut ChaCha20Rng| {
        columns
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|_| C::ScalarExt::random(&mut *rng))
                    .collect()
            })
            .collect::<Vec<Vec<_>>>()
    };
    let randomized = Tables {
        fixed: random(&tables.fixed, &mut rng),
        advice: random(&tables.advice, &mut rng),
        instance: random(&tables.instance, &mut rng),
    };
    for tables in [tables, &randomized] {
        let values = compiled
            .gate_values(&tables.fixed, &tables.advice, &tables.instance, n)
            .expect("gate values");
        for (poly, compiled_values) in polys.iter().zip(&values) {
            for (row, value) in compiled_values.iter().enumerate() {
                let mut direct = RowEvaluator {
                    fixed: &tables.fixed,
                    advice: &tables.advice,
                    instance: &tables.instance,
                    row,
                };
                assert_eq!(*value, poly.evaluate(&mut direct), "row {row}");
            }
        }
    }
}

#[test]
fn the_compiled_evaluator_matches_direct_evaluation() {
    let arithmetic = Arithmetic {
        start: 2,
        rows: 9,
        tamper: Some(3),
    };
    for choice in [CHOICES[0], CHOICES[2]] {
        let fixture = setup::<Ep, _>(&arithmetic, choice);
        let columns = tables(&fixture.pk, &arithmetic, &arithmetic.instances::<Fq>());
        compiled_matches_direct(&fixture.pk, &columns);
    }
    let lookups = Lookups {
        rows: 7,
        tamper: Some(2),
        out_of_range: false,
        offset: 1,
    };
    let fixture = setup::<Eq, _>(&lookups, CHOICES[1]);
    compiled_matches_direct(&fixture.pk, &tables(&fixture.pk, &lookups, &[]));
    let permutations = Permutations {
        rows: 5,
        tamper: None,
    };
    let fixture = setup::<Eq, _>(&permutations, CHOICES[3]);
    let columns = tables(&fixture.pk, &permutations, &permutations.instances::<Fp>());
    compiled_matches_direct(&fixture.pk, &columns);
}

/// The `(gate polynomial, row)` pairs where the compiled values are nonzero
/// equal the gate failures the checker reports on the uncompressed source.
fn compiled_agrees_with_checker<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    pk: &ProvingKey<C>,
    circuit: &Ci,
    instances: &[Vec<C::ScalarExt>],
) -> usize {
    let n = pk.binding().n();
    let synthesized = synthesize(circuit, K, Some(instances)).expect("synthesis");
    let report = check(&synthesized.cs, &synthesized.tables, CheckMode::Strict).expect("check");
    let offsets: Vec<usize> = synthesized
        .cs
        .gates()
        .iter()
        .scan(0, |offset, gate| {
            let start = *offset;
            *offset += gate.polynomials().len();
            Some(start)
        })
        .collect();
    let expected: BTreeSet<(usize, usize)> = report
        .failures()
        .iter()
        .filter_map(|failure| match failure {
            CheckFailure::ConstraintNotSatisfied {
                gate,
                constraint,
                location,
                ..
            } => Some((offsets[*gate] + constraint, location.row)),
            _ => None,
        })
        .collect();
    let tables = tables(pk, circuit, instances);
    let compiled = CompiledExpressions::compile(pk.binding().descriptor(), true).expect("compile");
    let values = compiled
        .gate_values(&tables.fixed, &tables.advice, &tables.instance, n)
        .expect("gate values");
    let found: BTreeSet<(usize, usize)> = values
        .iter()
        .enumerate()
        .flat_map(|(poly, rows)| {
            rows.iter()
                .enumerate()
                .filter(|(_, value)| !bool::from(value.is_zero()))
                .map(move |(row, _)| (poly, row))
        })
        .collect();
    assert_eq!(found, expected);
    found.len()
}

#[test]
fn the_compiled_evaluator_agrees_with_the_constraint_checker() {
    let setup_arithmetic = setup::<Ep, _>(
        &Arithmetic {
            start: 2,
            rows: 9,
            tamper: None,
        },
        CHOICES[0],
    );
    let mut failures = 0;
    for tamper in [None, Some(0), Some(4), Some(8)] {
        let circuit = Arithmetic {
            start: 2,
            rows: 9,
            tamper,
        };
        failures += compiled_agrees_with_checker(
            &setup_arithmetic.pk,
            &circuit,
            &circuit.instances::<Fq>(),
        );
    }
    assert!(failures > 0, "the tampered rows must fail some gate");
    let honest = Lookups {
        rows: 7,
        tamper: None,
        out_of_range: false,
        offset: 0,
    };
    let lookups = setup::<Eq, _>(&honest, CHOICES[2]);
    for tamper in [None, Some(5)] {
        let circuit = Lookups { tamper, ..honest };
        compiled_agrees_with_checker(&lookups.pk, &circuit, &[]);
    }
}

#[test]
fn hash_consing_shares_subexpressions() {
    let setup = setup::<Ep, _>(
        &Lookups {
            rows: 3,
            tamper: None,
            out_of_range: false,
            offset: 0,
        },
        CHOICES[0],
    );
    let descriptor = setup.pk.binding().descriptor();
    let compiled = CompiledExpressions::<Fq>::compile(descriptor, true).expect("compile");
    let lookups_only = CompiledExpressions::<Fq>::compile(descriptor, false).expect("compile");
    assert_eq!(lookups_only.gate_count(), 0);
    assert!(lookups_only.node_count() < compiled.node_count());
    // Duplicating every gate adds roots but no nodes.
    let mut doubled = descriptor.clone();
    doubled.gates = [doubled.gates.clone(), doubled.gates].concat();
    let twice = CompiledExpressions::<Fq>::compile(&doubled, true).expect("compile");
    assert_eq!(twice.node_count(), compiled.node_count());
    assert_eq!(twice.gate_count(), 2 * compiled.gate_count());
    // The two lookups share the selector query and `x`.
    let total_nodes: usize = descriptor
        .lookups
        .iter()
        .flat_map(|lookup| lookup.inputs.iter().chain(&lookup.tables))
        .map(Vec::len)
        .sum();
    assert!(lookups_only.node_count() < total_nodes);
}

#[test]
fn malformed_expressions_do_not_compile() {
    let setup = setup::<Ep, _>(
        &Arithmetic {
            start: 1,
            rows: 3,
            tamper: None,
        },
        CHOICES[0],
    );
    let descriptor = setup.pk.binding().descriptor();
    let malformed = |nodes: Vec<ExprNodeV1>| {
        let mut edited = descriptor.clone();
        edited.gates[0][0] = nodes;
        CompiledExpressions::<Fq>::compile(&edited, true).err()
    };
    let expected = Some(DescriptorError::Invalid(DescriptorRule::Expression));
    assert_eq!(malformed(vec![ExprNodeV1::Sum]), expected);
    assert_eq!(malformed(vec![ExprNodeV1::Advice(99)]), expected);
    assert_eq!(malformed(vec![ExprNodeV1::Constant([0xff; 32])]), expected);
    assert_eq!(
        malformed(vec![ExprNodeV1::Advice(0), ExprNodeV1::Advice(0)]),
        expected
    );
    assert_eq!(malformed(vec![ExprNodeV1::Negated]), expected);
    assert_eq!(malformed(vec![]), expected);
    assert_eq!(rotation_offset(-1, 8), Ok(7));
    assert_eq!(rotation_offset(9, 8), Ok(1));
}

#[test]
fn lookup_compression_matches_direct_folding() {
    let circuit = Lookups {
        rows: 6,
        tamper: Some(1),
        out_of_range: false,
        offset: 3,
    };
    let setup = setup::<Eq, _>(&circuit, CHOICES[3]);
    let tables = tables(&setup.pk, &circuit, &[]);
    let n = setup.pk.binding().n();
    let theta = Fp::from(1_000_003);
    let compiled = CompiledExpressions::<Fp>::compile(setup.pk.binding().descriptor(), false)
        .expect("compile");
    let compressed = compiled
        .compress_lookups(&tables.fixed, &tables.advice, &tables.instance, theta, n)
        .expect("compress");
    let cs = setup.pk.constraint_system().constraint_system();
    assert_eq!(compressed.len(), cs.lookups().len());
    for (lookup, (inputs, table_values)) in cs.lookups().iter().zip(&compressed) {
        for row in 0..n {
            let fold = |expressions: &[Expression<Fp>]| {
                expressions.iter().fold(Fp::ZERO, |acc, expression| {
                    let mut direct = RowEvaluator {
                        fixed: &tables.fixed,
                        advice: &tables.advice,
                        instance: &tables.instance,
                        row,
                    };
                    acc * theta + expression.evaluate(&mut direct)
                })
            };
            assert_eq!(inputs[row], fold(lookup.input_expressions()));
            assert_eq!(table_values[row], fold(lookup.table_expressions()));
        }
    }
    // Columns of the wrong length are refused.
    assert!(
        compiled
            .compress_lookups(&tables.fixed, &vec![vec![Fp::ZERO; 3]; 2], &[], theta, n)
            .is_err()
    );
}
