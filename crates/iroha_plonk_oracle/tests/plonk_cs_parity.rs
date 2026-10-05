//! Constraint-system, selector-compression and layout parity of `iroha_plonk`
//! with the vendored halo2-axiom (stage ENGINE-1, tasks T8/T9).
//!
//! - `cs_parity_*`: seeded random constraint systems are configured through
//!   both APIs with the same calls (mixing immediately interned
//!   `VirtualCells::query_*` queries with deferred `Column::query_cell` ones,
//!   gates with simple and complex selectors, `lookup` and `lookup_any`,
//!   equality and constants). Degrees, blinding factors, the three query
//!   tables, the permutation columns, every gate and lookup expression node for
//!   node, and then, after `compress_selectors` and
//!   `directly_convert_selectors_to_fixed` on seeded activations, the selector
//!   columns, the new fixed queries and the substituted expressions must be
//!   identical.
//! - `layout_parity_*`: one circuit with tables, constants, copies,
//!   `copy_advice`, instance copies, rational fixed values and selectors is
//!   synthesized by the vendored `MockProver` (`SimpleFloorPlanner`) and by the
//!   native `SimpleFloorPlanner`; the fixed columns (selector columns
//!   included), their assigned flags and the copy permutation must be equal.
//!
//! Run: `cargo test -p iroha_plonk_oracle --test plonk_cs_parity`.

use std::marker::PhantomData;

use halo2_axiom::{
    circuit::{Layouter as VLayouter, SimpleFloorPlanner as VSimpleFloorPlanner, Value as VValue},
    dev::{CellValue, MockProver},
    halo2curves::ff::{Field, FromUniformBytes, PrimeField},
    plonk::{
        Advice as VAdvice, Any as VAny, Assigned as VAssigned, Circuit as VCircuit,
        Column as VColumn, ConstraintSystem as VCs, Error as VError, Expression as VExpr,
        Fixed as VFixed, Instance as VInstance, Selector as VSelector, TableColumn as VTableColumn,
        VirtualCells as VCells,
    },
    poly::Rotation as VRotation,
};
use iroha_plonk::{
    cs::{
        Advice, AdviceQuery, Any, Column, ConstraintSystem, Expression, Fixed, FixedQuery,
        Instance, InstanceQuery, Rotation, Selector, TableColumn, VirtualCells,
    },
    frontend::{Assigned, Circuit, Error, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_oracle::convert::{CurveBridge, NativeScalar, Pallas, Vesta, native_scalar};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use rayon::iter::ParallelIterator;

/// A leaf of a seeded expression recipe.
#[derive(Clone, Debug)]
enum Leaf {
    /// Advice column, rotation, and whether it is queried through
    /// `VirtualCells` (interned immediately) or `Column::query_cell`.
    Advice(usize, i32, bool),
    /// Fixed column, rotation, immediate.
    Fixed(usize, i32, bool),
    /// Instance column, rotation, immediate.
    Instance(usize, i32, bool),
    /// A complex selector.
    Complex(usize),
    /// A small constant.
    Constant(u64),
}

/// A seeded expression recipe, built identically on both sides.
#[derive(Clone, Debug)]
enum Recipe {
    Leaf(Leaf),
    Neg(Box<Recipe>),
    Sum(Box<Recipe>, Box<Recipe>),
    Product(Box<Recipe>, Box<Recipe>),
    Scale(Box<Recipe>, u64),
}

/// The table side of a lookup pair.
#[derive(Clone, Debug)]
enum TableSide {
    /// A lookup table column (`lookup`).
    Column(usize),
    /// An expression (`lookup_any`).
    Expression(Recipe),
}

/// One configure-time call.
#[derive(Clone, Debug)]
enum Op {
    /// A gate: an optional simple selector multiplied onto each body.
    Gate(Option<usize>, Vec<Recipe>),
    /// A lookup; `true` uses `lookup_any`.
    Lookup(bool, Vec<(Recipe, TableSide)>),
    /// `enable_equality` on a column kind (0 advice, 1 fixed, 2 instance).
    Equality(u8, usize),
    /// `enable_constant` on a fixed column.
    Constant(usize),
    /// `set_minimum_degree`.
    MinimumDegree(usize),
}

/// A seeded constraint system.
#[derive(Clone, Debug)]
struct CsRecipe {
    advice: usize,
    fixed: usize,
    tables: usize,
    instance_lengths: Vec<usize>,
    simple: Vec<bool>,
    ops: Vec<Op>,
}

fn pick(rng: &mut ChaCha20Rng, bound: usize) -> usize {
    usize::try_from(rng.next_u32()).expect("u32 fits usize") % bound
}

fn rotation(rng: &mut ChaCha20Rng) -> i32 {
    i32::try_from(pick(rng, 5)).expect("small") - 2
}

fn leaf(rng: &mut ChaCha20Rng, recipe: &CsRecipe, complex: &[usize]) -> Leaf {
    let immediate = rng.next_u32().is_multiple_of(2);
    match pick(rng, 6) {
        0 | 1 => Leaf::Advice(pick(rng, recipe.advice), rotation(rng), immediate),
        2 => Leaf::Fixed(pick(rng, recipe.fixed), rotation(rng), immediate),
        3 if !recipe.instance_lengths.is_empty() => Leaf::Instance(
            pick(rng, recipe.instance_lengths.len()),
            rotation(rng),
            immediate,
        ),
        4 if !complex.is_empty() => Leaf::Complex(complex[pick(rng, complex.len())]),
        _ => Leaf::Constant(u64::from(rng.next_u32() % 7)),
    }
}

fn expression(rng: &mut ChaCha20Rng, recipe: &CsRecipe, complex: &[usize], depth: u32) -> Recipe {
    if depth == 0 || rng.next_u32().is_multiple_of(3) {
        return Recipe::Leaf(leaf(rng, recipe, complex));
    }
    let kind = pick(rng, 4);
    let left = Box::new(expression(rng, recipe, complex, depth - 1));
    match kind {
        0 => Recipe::Neg(left),
        1 => Recipe::Sum(left, Box::new(expression(rng, recipe, complex, depth - 1))),
        2 => Recipe::Product(left, Box::new(expression(rng, recipe, complex, depth - 1))),
        _ => Recipe::Scale(left, 2 + u64::from(rng.next_u32() % 5)),
    }
}

/// A seeded random constraint system.
fn random_recipe(rng: &mut ChaCha20Rng) -> CsRecipe {
    let mut recipe = CsRecipe {
        advice: 1 + pick(rng, 4),
        fixed: 1 + pick(rng, 3),
        tables: 1 + pick(rng, 2),
        instance_lengths: (0..pick(rng, 3)).map(|_| 1 + pick(rng, 3)).collect(),
        simple: (0..pick(rng, 9))
            .map(|_| !rng.next_u32().is_multiple_of(4))
            .collect(),
        ops: Vec::new(),
    };
    let complex: Vec<usize> = recipe
        .simple
        .iter()
        .enumerate()
        .filter_map(|(index, simple)| (!simple).then_some(index))
        .collect();
    let simple: Vec<usize> = recipe
        .simple
        .iter()
        .enumerate()
        .filter_map(|(index, simple)| simple.then_some(index))
        .collect();
    for _ in 0..(2 + pick(rng, 6)) {
        let op = match pick(rng, 8) {
            0..=3 => {
                let selector = (!simple.is_empty() && !rng.next_u32().is_multiple_of(4))
                    .then(|| simple[pick(rng, simple.len())]);
                let polys = (0..=pick(rng, 3))
                    .map(|_| expression(rng, &recipe, &complex, 3))
                    .collect();
                Op::Gate(selector, polys)
            }
            4 => {
                let any = rng.next_u32().is_multiple_of(2);
                let pairs = (0..=pick(rng, 2))
                    .map(|_| {
                        let input = expression(rng, &recipe, &complex, 2);
                        let table = if any {
                            TableSide::Expression(expression(rng, &recipe, &complex, 1))
                        } else {
                            TableSide::Column(pick(rng, recipe.tables))
                        };
                        (input, table)
                    })
                    .collect();
                Op::Lookup(any, pairs)
            }
            5 => {
                let kind = u8::try_from(pick(rng, 3)).expect("small");
                let bound = match kind {
                    0 => recipe.advice,
                    1 => recipe.fixed,
                    _ => recipe.instance_lengths.len(),
                };
                if bound == 0 {
                    Op::MinimumDegree(3)
                } else {
                    Op::Equality(kind, pick(rng, bound))
                }
            }
            6 => Op::Constant(pick(rng, recipe.fixed)),
            _ => Op::MinimumDegree(3 + pick(rng, 4)),
        };
        recipe.ops.push(op);
    }
    recipe
}

/// The vendored columns of a recipe.
struct VendoredColumns {
    advice: Vec<VColumn<VAdvice>>,
    fixed: Vec<VColumn<VFixed>>,
    tables: Vec<VTableColumn>,
    instance: Vec<VColumn<VInstance>>,
    selectors: Vec<VSelector>,
}

fn vendored_leaf<F: PrimeField>(
    leaf: &Leaf,
    cells: &mut VCells<'_, F>,
    columns: &VendoredColumns,
) -> VExpr<F> {
    match leaf {
        Leaf::Advice(index, rotation, true) => {
            cells.query_advice(columns.advice[*index], VRotation(*rotation))
        }
        Leaf::Advice(index, rotation, false) => {
            columns.advice[*index].query_cell(VRotation(*rotation))
        }
        Leaf::Fixed(index, rotation, true) => {
            cells.query_fixed(columns.fixed[*index], VRotation(*rotation))
        }
        Leaf::Fixed(index, rotation, false) => {
            columns.fixed[*index].query_cell(VRotation(*rotation))
        }
        Leaf::Instance(index, rotation, true) => {
            cells.query_instance(columns.instance[*index], VRotation(*rotation))
        }
        Leaf::Instance(index, rotation, false) => {
            columns.instance[*index].query_cell(VRotation(*rotation))
        }
        Leaf::Complex(index) => cells.query_selector(columns.selectors[*index]),
        Leaf::Constant(value) => VExpr::Constant(F::from(*value)),
    }
}

fn vendored_expression<F: PrimeField>(
    recipe: &Recipe,
    cells: &mut VCells<'_, F>,
    columns: &VendoredColumns,
) -> VExpr<F> {
    match recipe {
        Recipe::Leaf(leaf) => vendored_leaf(leaf, cells, columns),
        Recipe::Neg(inner) => -vendored_expression(inner, cells, columns),
        Recipe::Sum(left, right) => {
            let left = vendored_expression(left, cells, columns);
            left + vendored_expression(right, cells, columns)
        }
        Recipe::Product(left, right) => {
            let left = vendored_expression(left, cells, columns);
            left * vendored_expression(right, cells, columns)
        }
        Recipe::Scale(inner, factor) => {
            vendored_expression(inner, cells, columns) * F::from(*factor)
        }
    }
}

fn configure_vendored<F: PrimeField>(recipe: &CsRecipe) -> VCs<F> {
    let mut cs = VCs::<F>::default();
    let columns = VendoredColumns {
        advice: (0..recipe.advice).map(|_| cs.advice_column()).collect(),
        fixed: (0..recipe.fixed).map(|_| cs.fixed_column()).collect(),
        tables: (0..recipe.tables)
            .map(|_| cs.lookup_table_column())
            .collect(),
        instance: recipe
            .instance_lengths
            .iter()
            .map(|_| cs.instance_column())
            .collect(),
        selectors: recipe
            .simple
            .iter()
            .map(|simple| {
                if *simple {
                    cs.selector()
                } else {
                    cs.complex_selector()
                }
            })
            .collect(),
    };
    for (index, op) in recipe.ops.iter().enumerate() {
        match op {
            Op::Gate(selector, polys) => cs.create_gate(format!("gate {index}"), |cells| {
                polys
                    .iter()
                    .map(|body| match selector {
                        Some(s) => {
                            let s = cells.query_selector(columns.selectors[*s]);
                            s * vendored_expression(body, cells, &columns)
                        }
                        None => vendored_expression(body, cells, &columns),
                    })
                    .collect::<Vec<_>>()
            }),
            Op::Lookup(false, pairs) => {
                cs.lookup(format!("lookup {index}"), |cells| {
                    pairs
                        .iter()
                        .map(|(input, table)| {
                            let TableSide::Column(table) = table else {
                                unreachable!("lookup pairs use table columns");
                            };
                            (
                                vendored_expression(input, cells, &columns),
                                columns.tables[*table],
                            )
                        })
                        .collect()
                });
            }
            Op::Lookup(true, pairs) => {
                cs.lookup_any(format!("lookup {index}"), |cells| {
                    pairs
                        .iter()
                        .map(|(input, table)| {
                            let TableSide::Expression(table) = table else {
                                unreachable!("lookup_any pairs use expressions");
                            };
                            let input = vendored_expression(input, cells, &columns);
                            (input, vendored_expression(table, cells, &columns))
                        })
                        .collect()
                });
            }
            Op::Equality(0, column) => cs.enable_equality(columns.advice[*column]),
            Op::Equality(1, column) => cs.enable_equality(columns.fixed[*column]),
            Op::Equality(_, column) => cs.enable_equality(columns.instance[*column]),
            Op::Constant(column) => cs.enable_constant(columns.fixed[*column]),
            Op::MinimumDegree(degree) => cs.set_minimum_degree(*degree),
        }
    }
    cs
}

/// The native columns of a recipe.
struct NativeColumns {
    advice: Vec<Column<Advice>>,
    fixed: Vec<Column<Fixed>>,
    tables: Vec<TableColumn>,
    instance: Vec<Column<Instance>>,
    selectors: Vec<Selector>,
}

fn native_leaf<F: iroha_pasta::PastaField>(
    leaf: &Leaf,
    cells: &mut VirtualCells<'_, F>,
    columns: &NativeColumns,
) -> Expression<F> {
    match leaf {
        Leaf::Advice(index, rotation, true) => {
            cells.query_advice(columns.advice[*index], Rotation(*rotation))
        }
        Leaf::Advice(index, rotation, false) => {
            columns.advice[*index].query_cell(Rotation(*rotation))
        }
        Leaf::Fixed(index, rotation, true) => {
            cells.query_fixed(columns.fixed[*index], Rotation(*rotation))
        }
        Leaf::Fixed(index, rotation, false) => {
            columns.fixed[*index].query_cell(Rotation(*rotation))
        }
        Leaf::Instance(index, rotation, true) => {
            cells.query_instance(columns.instance[*index], Rotation(*rotation))
        }
        Leaf::Instance(index, rotation, false) => {
            columns.instance[*index].query_cell(Rotation(*rotation))
        }
        Leaf::Complex(index) => cells.query_selector(columns.selectors[*index]),
        Leaf::Constant(value) => Expression::Constant(F::from(*value)),
    }
}

fn native_expression<F: iroha_pasta::PastaField>(
    recipe: &Recipe,
    cells: &mut VirtualCells<'_, F>,
    columns: &NativeColumns,
) -> Expression<F> {
    match recipe {
        Recipe::Leaf(leaf) => native_leaf(leaf, cells, columns),
        Recipe::Neg(inner) => -native_expression(inner, cells, columns),
        Recipe::Sum(left, right) => {
            let left = native_expression(left, cells, columns);
            left + native_expression(right, cells, columns)
        }
        Recipe::Product(left, right) => {
            let left = native_expression(left, cells, columns);
            left * native_expression(right, cells, columns)
        }
        Recipe::Scale(inner, factor) => native_expression(inner, cells, columns) * F::from(*factor),
    }
}

fn configure_native<F: iroha_pasta::PastaField>(recipe: &CsRecipe) -> ConstraintSystem<F> {
    let mut cs = ConstraintSystem::<F>::new();
    let columns = NativeColumns {
        advice: (0..recipe.advice).map(|_| cs.advice_column()).collect(),
        fixed: (0..recipe.fixed).map(|_| cs.fixed_column()).collect(),
        tables: (0..recipe.tables)
            .map(|_| cs.lookup_table_column())
            .collect(),
        instance: recipe
            .instance_lengths
            .iter()
            .map(|length| cs.instance_column(*length))
            .collect(),
        selectors: recipe
            .simple
            .iter()
            .map(|simple| {
                if *simple {
                    cs.selector()
                } else {
                    cs.complex_selector()
                }
            })
            .collect(),
    };
    for (index, op) in recipe.ops.iter().enumerate() {
        match op {
            Op::Gate(selector, polys) => cs.create_gate(format!("gate {index}"), |cells| {
                polys
                    .iter()
                    .map(|body| match selector {
                        Some(s) => {
                            let s = cells.query_selector(columns.selectors[*s]);
                            s * native_expression(body, cells, &columns)
                        }
                        None => native_expression(body, cells, &columns),
                    })
                    .collect::<Vec<_>>()
            }),
            Op::Lookup(false, pairs) => {
                cs.lookup(format!("lookup {index}"), |cells| {
                    pairs
                        .iter()
                        .map(|(input, table)| {
                            let TableSide::Column(table) = table else {
                                unreachable!("lookup pairs use table columns");
                            };
                            (
                                native_expression(input, cells, &columns),
                                columns.tables[*table],
                            )
                        })
                        .collect()
                });
            }
            Op::Lookup(true, pairs) => {
                cs.lookup_any(format!("lookup {index}"), |cells| {
                    pairs
                        .iter()
                        .map(|(input, table)| {
                            let TableSide::Expression(table) = table else {
                                unreachable!("lookup_any pairs use expressions");
                            };
                            let input = native_expression(input, cells, &columns);
                            (input, native_expression(table, cells, &columns))
                        })
                        .collect()
                });
            }
            Op::Equality(0, column) => cs.enable_equality(columns.advice[*column]),
            Op::Equality(1, column) => cs.enable_equality(columns.fixed[*column]),
            Op::Equality(_, column) => cs.enable_equality(columns.instance[*column]),
            Op::Constant(column) => cs.enable_constant(columns.fixed[*column]),
            Op::MinimumDegree(degree) => cs.set_minimum_degree(*degree),
        }
    }
    cs
}

/// A vendored expression as a native one (constants re-encoded).
fn convert<B: CurveBridge>(expression: &VExpr<B::VScalar>) -> Expression<NativeScalar<B>> {
    match expression {
        VExpr::Constant(value) => Expression::Constant(native_scalar::<B>(value)),
        VExpr::Selector(selector) => {
            Expression::Selector(Selector::new(selector.index(), selector.is_simple()))
        }
        VExpr::Fixed(query) => Expression::Fixed(FixedQuery {
            column_index: query.column_index(),
            rotation: Rotation(query.rotation().0),
        }),
        VExpr::Advice(query) => Expression::Advice(AdviceQuery {
            column_index: query.column_index(),
            rotation: Rotation(query.rotation().0),
        }),
        VExpr::Instance(query) => Expression::Instance(InstanceQuery {
            column_index: query.column_index(),
            rotation: Rotation(query.rotation().0),
        }),
        VExpr::Challenge(_) => unreachable!("PIPA-v1 has no challenges"),
        VExpr::Negated(inner) => Expression::Negated(Box::new(convert::<B>(inner))),
        VExpr::Sum(left, right) => {
            Expression::Sum(Box::new(convert::<B>(left)), Box::new(convert::<B>(right)))
        }
        VExpr::Product(left, right) => {
            Expression::Product(Box::new(convert::<B>(left)), Box::new(convert::<B>(right)))
        }
        VExpr::Scaled(inner, factor) => {
            Expression::Scaled(Box::new(convert::<B>(inner)), native_scalar::<B>(factor))
        }
    }
}

fn any_kind(column: &VColumn<VAny>) -> (u8, usize) {
    let kind = match column.column_type() {
        VAny::Advice(_) => 0,
        VAny::Fixed => 1,
        VAny::Instance => 2,
    };
    (kind, column.index())
}

fn native_kind(column: &Column<Any>) -> (u8, usize) {
    let kind = match column.column_type() {
        Any::Advice => 0,
        Any::Fixed => 1,
        Any::Instance => 2,
    };
    (kind, column.index())
}

/// Asserts that the two constraint systems are identical in every part the
/// descriptor, keys and proofs depend on.
fn assert_same_cs<B: CurveBridge>(
    vendored: &VCs<B::VScalar>,
    native: &ConstraintSystem<NativeScalar<B>>,
    context: &str,
) {
    assert_eq!(vendored.degree(), native.degree(), "{context}: degree");
    assert_eq!(
        vendored.blinding_factors(),
        native.blinding_factors(),
        "{context}: blinding factors"
    );
    assert_eq!(vendored.minimum_rows(), native.minimum_rows(), "{context}");
    assert_eq!(
        vendored.num_fixed_columns(),
        native.num_fixed_columns(),
        "{context}: fixed columns"
    );
    assert_eq!(vendored.num_advice_columns(), native.num_advice_columns());
    assert_eq!(
        vendored.num_instance_columns(),
        native.num_instance_columns()
    );
    assert_eq!(vendored.num_selectors(), native.num_selectors());
    assert_eq!(
        vendored
            .advice_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        native
            .advice_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        "{context}: advice queries"
    );
    assert_eq!(
        vendored
            .fixed_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        native
            .fixed_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        "{context}: fixed queries"
    );
    assert_eq!(
        vendored
            .instance_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        native
            .instance_queries()
            .iter()
            .map(|(c, r)| (c.index(), r.0))
            .collect::<Vec<_>>(),
        "{context}: instance queries"
    );
    assert_eq!(
        vendored
            .permutation()
            .get_columns()
            .iter()
            .map(any_kind)
            .collect::<Vec<_>>(),
        native
            .permutation()
            .columns()
            .iter()
            .map(native_kind)
            .collect::<Vec<_>>(),
        "{context}: permutation columns"
    );
    assert_eq!(
        vendored
            .constants()
            .iter()
            .map(VColumn::index)
            .collect::<Vec<_>>(),
        native
            .constants()
            .iter()
            .map(Column::index)
            .collect::<Vec<_>>(),
        "{context}: constants"
    );
    assert_eq!(vendored.gates().len(), native.gates().len(), "{context}");
    for (index, (v, n)) in vendored.gates().iter().zip(native.gates()).enumerate() {
        assert_eq!(v.name(), n.name());
        let converted: Vec<_> = v.polynomials().iter().map(convert::<B>).collect();
        assert_eq!(converted, n.polynomials(), "{context}: gate {index}");
    }
    assert_eq!(
        vendored.lookups().len(),
        native.lookups().len(),
        "{context}"
    );
    for (index, (v, n)) in vendored.lookups().iter().zip(native.lookups()).enumerate() {
        let inputs: Vec<_> = v.input_expressions().iter().map(convert::<B>).collect();
        let tables: Vec<_> = v.table_expressions().iter().map(convert::<B>).collect();
        assert_eq!(
            inputs,
            n.input_expressions(),
            "{context}: lookup {index} inputs"
        );
        assert_eq!(
            tables,
            n.table_expressions(),
            "{context}: lookup {index} tables"
        );
    }
}

/// Seeded activations: a mix of disjoint slots (which compress together) and
/// random overlaps.
fn activations(rng: &mut ChaCha20Rng, selectors: usize, n: usize) -> Vec<Vec<bool>> {
    (0..selectors)
        .map(|selector| match pick(rng, 6) {
            0 => vec![false; n],
            1..=3 => (0..n)
                .map(|row| row % selectors.max(1) == selector)
                .collect(),
            4 => (0..n).map(|_| rng.next_u32().is_multiple_of(8)).collect(),
            _ => (0..n).map(|_| rng.next_u32().is_multiple_of(2)).collect(),
        })
        .collect()
}

fn cs_parity<B: CurveBridge>(seed: u8, cases: usize) {
    let mut rng = ChaCha20Rng::from_seed([seed; 32]);
    let n = 32;
    // Coverage counters: the suite must exercise real merges, lookups of both
    // kinds and multi-member combinations, not only trivial plans.
    let (mut merged, mut lookups, mut lookup_any) = (0, 0, 0);
    for case in 0..cases {
        let recipe = random_recipe(&mut rng);
        let context = format!("{} case {case}: {recipe:?}", B::NAME);
        let vendored = configure_vendored::<B::VScalar>(&recipe);
        let native = configure_native::<NativeScalar<B>>(&recipe);
        assert!(native.check().is_ok(), "{context}");
        assert_same_cs::<B>(&vendored, &native, &context);
        assert_eq!(
            vendored.selector_degrees_for_parity(),
            native.selector_degrees(),
            "{context}: selector degrees"
        );

        let rows = activations(&mut rng, recipe.simple.len(), n);
        let (compressed, vendored_columns) = vendored.clone().compress_selectors(rows.clone());
        let finalized = native.clone().compress_selectors(&rows).expect("compress");
        assert_same_cs::<B>(&compressed, finalized.constraint_system(), &context);
        if finalized.selector_columns().len() < recipe.simple.len() {
            merged += 1;
        }
        for op in &recipe.ops {
            match op {
                Op::Lookup(false, _) => lookups += 1,
                Op::Lookup(true, _) => lookup_any += 1,
                _ => {}
            }
        }
        let converted: Vec<Vec<_>> = vendored_columns
            .iter()
            .map(|column| column.iter().map(native_scalar::<B>).collect())
            .collect();
        assert_eq!(
            converted,
            finalized.selector_columns(),
            "{context}: compressed selector columns"
        );

        let (direct, vendored_columns) = vendored.directly_convert_selectors_to_fixed(rows.clone());
        let finalized = native
            .directly_convert_selectors_to_fixed(&rows)
            .expect("direct");
        assert_same_cs::<B>(&direct, finalized.constraint_system(), &context);
        let converted: Vec<Vec<_>> = vendored_columns
            .iter()
            .map(|column| column.iter().map(native_scalar::<B>).collect())
            .collect();
        assert_eq!(
            converted,
            finalized.selector_columns(),
            "{context}: direct columns"
        );
    }
    assert!(
        merged * 10 > cases && lookups > 10 && lookup_any > 10,
        "coverage: {merged} merged plans, {lookups} lookups, {lookup_any} lookup_any"
    );
}

/// `selector_degrees` is crate-private in the vendored code; recompute it from
/// the public gates exactly as `ConstraintSystem::selector_degrees` does.
trait SelectorDegrees {
    fn selector_degrees_for_parity(&self) -> Vec<usize>;
}

impl<F: Field> SelectorDegrees for VCs<F> {
    fn selector_degrees_for_parity(&self) -> Vec<usize> {
        fn simple<F: Field>(expression: &VExpr<F>) -> Option<usize> {
            match expression {
                VExpr::Selector(selector) => selector.is_simple().then(|| selector.index()),
                VExpr::Negated(inner) | VExpr::Scaled(inner, _) => simple(inner),
                VExpr::Sum(left, right) | VExpr::Product(left, right) => {
                    simple(left).or_else(|| simple(right))
                }
                _ => None,
            }
        }
        let mut degrees = vec![0; self.num_selectors()];
        for poly in self
            .gates()
            .iter()
            .flat_map(halo2_axiom::plonk::Gate::polynomials)
        {
            if let Some(selector) = simple(poly) {
                degrees[selector] = degrees[selector].max(poly.degree());
            }
        }
        degrees
    }
}

#[test]
fn cs_parity_vesta() {
    cs_parity::<Vesta>(0x31, 600);
}

/// Twelve simple selectors gating degree 1-4 bodies and two complex ones,
/// with disjoint and overlapping activations: multi-member combinations,
/// exclusions and the degree budget all occur.
#[test]
fn cs_parity_dense_selectors() {
    let advice = |index: usize, rotation: i32| Recipe::Leaf(Leaf::Advice(index, rotation, true));
    let power = |degree: usize| {
        (1..degree).fold(advice(0, 0), |acc, i| {
            Recipe::Product(Box::new(acc), Box::new(advice(i % 3, 0)))
        })
    };
    let mut recipe = CsRecipe {
        advice: 3,
        fixed: 1,
        tables: 1,
        instance_lengths: vec![1],
        simple: vec![true; 12],
        ops: Vec::new(),
    };
    recipe.simple.extend([false, false]);
    for selector in 0..12 {
        recipe
            .ops
            .push(Op::Gate(Some(selector), vec![power(1 + selector % 4)]));
    }
    recipe.ops.push(Op::Lookup(
        false,
        vec![(
            Recipe::Product(
                Box::new(Recipe::Leaf(Leaf::Complex(12))),
                Box::new(advice(1, 0)),
            ),
            TableSide::Column(0),
        )],
    ));
    recipe.ops.push(Op::Gate(
        None,
        vec![Recipe::Product(
            Box::new(Recipe::Leaf(Leaf::Complex(13))),
            Box::new(advice(2, 1)),
        )],
    ));
    let n = 32;
    for pattern in 0..4 {
        let rows: Vec<Vec<bool>> = (0..14)
            .map(|selector| match pattern {
                0 => (0..n).map(|row| row % 14 == selector).collect(),
                1 => (0..n).map(|row| row % 7 == selector % 7).collect(),
                2 => (0..n)
                    .map(|row| row < 16 && row % 4 == selector % 4)
                    .collect(),
                _ => vec![false; n],
            })
            .collect();
        let vendored = configure_vendored::<<Vesta as CurveBridge>::VScalar>(&recipe);
        let native = configure_native::<NativeScalar<Vesta>>(&recipe);
        let context = format!("dense pattern {pattern}");
        assert_same_cs::<Vesta>(&vendored, &native, &context);
        let (compressed, columns) = vendored.compress_selectors(rows.clone());
        let finalized = native.compress_selectors(&rows).expect("compress");
        assert_same_cs::<Vesta>(&compressed, finalized.constraint_system(), &context);
        let converted: Vec<Vec<_>> = columns
            .iter()
            .map(|column| column.iter().map(native_scalar::<Vesta>).collect())
            .collect();
        assert_eq!(converted, finalized.selector_columns(), "{context}");
        assert!(
            finalized
                .selector_plan()
                .entries
                .iter()
                .any(|entry| entry.root >= 2),
            "{context}: some combination has several members"
        );
    }
}

#[test]
fn cs_parity_pallas() {
    cs_parity::<Pallas>(0x32, 600);
}

/// Columns of the layout circuit (vendored side).
#[derive(Clone, Copy)]
struct VLayoutConfig {
    a: VColumn<VAdvice>,
    b: VColumn<VAdvice>,
    c: VColumn<VAdvice>,
    f: VColumn<VFixed>,
    table: VTableColumn,
    instance: VColumn<VInstance>,
    s0: VSelector,
    s1: VSelector,
    q: VSelector,
}

/// Layout circuit, vendored side.
#[derive(Clone, Default)]
struct VLayout<F>(PhantomData<F>);

impl<F: PrimeField> VCircuit<F> for VLayout<F> {
    type Config = VLayoutConfig;
    type FloorPlanner = VSimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self(PhantomData)
    }

    fn configure(meta: &mut VCs<F>) -> VLayoutConfig {
        let lhs = meta.advice_column();
        let rhs = meta.advice_column();
        let out = meta.advice_column();
        let step = meta.fixed_column();
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let table = meta.lookup_table_column();
        let instance = meta.instance_column();
        for column in [lhs, rhs, out] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let s0 = meta.selector();
        let s1 = meta.selector();
        let range = meta.complex_selector();
        meta.create_gate("mul", |cells| {
            let enabled = cells.query_selector(s0);
            let left = cells.query_advice(lhs, VRotation::cur());
            let right = cells.query_advice(rhs, VRotation::cur());
            let product = cells.query_advice(out, VRotation::cur());
            vec![enabled * (left * right - product)]
        });
        meta.create_gate("step", |cells| {
            let enabled = cells.query_selector(s1);
            let next = cells.query_advice(lhs, VRotation::next());
            let current = cells.query_advice(lhs, VRotation::cur());
            let increment = cells.query_fixed(step, VRotation::cur());
            vec![enabled * (next - current - increment)]
        });
        meta.lookup("range", |cells| {
            let enabled = cells.query_selector(range);
            let value = cells.query_advice(rhs, VRotation::cur());
            vec![(enabled * value, table)]
        });
        VLayoutConfig {
            a: lhs,
            b: rhs,
            c: out,
            f: step,
            table,
            instance,
            s0,
            s1,
            q: range,
        }
    }

    fn synthesize(
        &self,
        config: VLayoutConfig,
        mut layouter: impl VLayouter<F>,
    ) -> Result<(), VError> {
        layouter.assign_table(
            || "range",
            |mut table| {
                for value in 0..12_u64 {
                    let offset = usize::try_from(value).expect("small");
                    table.assign_cell(
                        || "v",
                        config.table,
                        offset,
                        || VValue::known(F::from(value)),
                    )?;
                }
                Ok(())
            },
        )?;
        let last = layouter.assign_region(
            || "first",
            |mut region| {
                let mut previous = region
                    .assign_advice_from_constant(|| "two", config.a, 0, F::from(2))?
                    .cell();
                for row in 0..8 {
                    if row % 2 == 0 {
                        config.s0.enable(&mut region, row)?;
                    } else {
                        config.s1.enable(&mut region, row)?;
                    }
                    if row % 3 == 0 {
                        config.q.enable(&mut region, row)?;
                    }
                    let a =
                        region.assign_advice(config.a, row + 1, VValue::known(F::from(row as u64)));
                    region.constrain_equal(previous, a.cell());
                    let b = region.assign_advice(config.b, row, VValue::known(F::from(3)));
                    let c = region.assign_advice(config.c, row, VValue::known(F::from(5)));
                    if row % 4 == 1 {
                        let copied = b.copy_advice(&mut region, config.c, row + 10);
                        region.constrain_equal(copied.cell(), c.cell());
                    }
                    region.assign_fixed(
                        config.f,
                        row,
                        VAssigned::Rational(F::from(row as u64 + 1), F::from(3)),
                    );
                    previous = c.cell();
                }
                region.assign_advice_from_instance(|| "pub", config.instance, 1, config.b, 20)?;
                Ok(previous)
            },
        )?;
        layouter.assign_region(
            || "second",
            |mut region| {
                region.assign_advice_from_constant(|| "seven", config.c, 21, F::from(7))?;
                region.assign_advice_from_constant(|| "two again", config.b, 22, F::from(2))?;
                Ok(())
            },
        )?;
        layouter.constrain_instance(last, config.instance, 0);
        Ok(())
    }
}

/// Columns of the layout circuit (native side).
#[derive(Clone, Copy)]
struct LayoutConfig {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Advice>,
    f: Column<Fixed>,
    table: TableColumn,
    instance: Column<Instance>,
    s0: Selector,
    s1: Selector,
    q: Selector,
}

/// Layout circuit, native side: the same calls in the same order.
#[derive(Clone, Default)]
struct Layout<F>(PhantomData<F>);

impl<F: iroha_pasta::PastaField> Circuit<F> for Layout<F> {
    type Config = LayoutConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self(PhantomData)
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> LayoutConfig {
        let lhs = meta.advice_column();
        let rhs = meta.advice_column();
        let out = meta.advice_column();
        let step = meta.fixed_column();
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let table = meta.lookup_table_column();
        let instance = meta.instance_column(2);
        for column in [lhs, rhs, out] {
            meta.enable_equality(column);
        }
        meta.enable_equality(instance);
        let s0 = meta.selector();
        let s1 = meta.selector();
        let range = meta.complex_selector();
        meta.create_gate("mul", |cells| {
            let enabled = cells.query_selector(s0);
            let left = cells.query_advice(lhs, Rotation::cur());
            let right = cells.query_advice(rhs, Rotation::cur());
            let product = cells.query_advice(out, Rotation::cur());
            vec![enabled * (left * right - product)]
        });
        meta.create_gate("step", |cells| {
            let enabled = cells.query_selector(s1);
            let next = cells.query_advice(lhs, Rotation::next());
            let current = cells.query_advice(lhs, Rotation::cur());
            let increment = cells.query_fixed(step, Rotation::cur());
            vec![enabled * (next - current - increment)]
        });
        meta.lookup("range", |cells| {
            let enabled = cells.query_selector(range);
            let value = cells.query_advice(rhs, Rotation::cur());
            vec![(enabled * value, table)]
        });
        LayoutConfig {
            a: lhs,
            b: rhs,
            c: out,
            f: step,
            table,
            instance,
            s0,
            s1,
            q: range,
        }
    }

    fn synthesize(
        &self,
        config: LayoutConfig,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_table(
            || "range",
            |mut table| {
                for value in 0..12_u64 {
                    let offset = usize::try_from(value).map_err(|_| Error::Synthesis)?;
                    table.assign_cell(
                        || "v",
                        config.table,
                        offset,
                        || Value::known(F::from(value)),
                    )?;
                }
                Ok(())
            },
        )?;
        let last = layouter.assign_region(
            || "first",
            |mut region| {
                let mut previous = region
                    .assign_advice_from_constant(|| "two", config.a, 0, F::from(2))?
                    .cell();
                for row in 0..8 {
                    if row % 2 == 0 {
                        config.s0.enable(&mut region, row)?;
                    } else {
                        config.s1.enable(&mut region, row)?;
                    }
                    if row % 3 == 0 {
                        config.q.enable(&mut region, row)?;
                    }
                    let a = region.assign_advice(
                        config.a,
                        row + 1,
                        Value::known(F::from(row as u64)),
                    )?;
                    region.constrain_equal(previous, a.cell())?;
                    let b = region.assign_advice(config.b, row, Value::known(F::from(3)))?;
                    let c = region.assign_advice(config.c, row, Value::known(F::from(5)))?;
                    if row % 4 == 1 {
                        let copied = b.copy_advice(&mut region, config.c, row + 10)?;
                        region.constrain_equal(copied.cell(), c.cell())?;
                    }
                    region.assign_fixed(
                        config.f,
                        row,
                        Assigned::Rational(F::from(row as u64 + 1), F::from(3)),
                    )?;
                    previous = c.cell();
                }
                region.assign_advice_from_instance(|| "pub", config.instance, 1, config.b, 20)?;
                Ok(previous)
            },
        )?;
        layouter.assign_region(
            || "second",
            |mut region| {
                region.assign_advice_from_constant(|| "seven", config.c, 21, F::from(7))?;
                region.assign_advice_from_constant(|| "two again", config.b, 22, F::from(2))?;
                Ok(())
            },
        )?;
        layouter.constrain_instance(last, config.instance, 0)
    }
}

fn layout_parity<B: CurveBridge>()
where
    B::VScalar: FromUniformBytes<64> + Ord,
{
    let k = 5;
    let public_vendored = vec![B::VScalar::from(5), B::VScalar::from(9)];
    let prover = MockProver::run(k, &VLayout::<B::VScalar>::default(), vec![public_vendored])
        .expect("vendored MockProver");
    let public: Vec<NativeScalar<B>> = vec![NativeScalar::<B>::from(5), NativeScalar::<B>::from(9)];
    let synthesized = synthesize(&Layout::<NativeScalar<B>>::default(), k, Some(&[public]))
        .expect("native synthesis");
    let tables = &synthesized.tables;
    let finalized = synthesized
        .cs
        .clone()
        .compress_selectors(tables.selectors())
        .expect("compress");

    // Fixed columns, selector columns appended (MockProver compresses).
    let vendored_fixed = prover.fixed();
    let mut native_fixed: Vec<Vec<Option<NativeScalar<B>>>> = tables
        .fixed()
        .iter()
        .zip(tables.fixed_assigned())
        .map(|(values, assigned)| {
            values
                .iter()
                .zip(assigned)
                .map(|(value, assigned)| assigned.then_some(*value))
                .collect()
        })
        .collect();
    native_fixed.extend(
        finalized
            .selector_columns()
            .iter()
            .map(|column| column.iter().copied().map(Some).collect()),
    );
    let converted: Vec<Vec<Option<NativeScalar<B>>>> = vendored_fixed
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|cell| match cell {
                    CellValue::Unassigned => None,
                    CellValue::Assigned(value) => Some(native_scalar::<B>(value)),
                    CellValue::Poison(_) => panic!("fixed cells are never poison"),
                })
                .collect()
        })
        .collect();
    assert_eq!(converted, native_fixed, "{}: fixed columns", B::NAME);

    // The copy permutation.
    let vendored_mapping: Vec<Vec<(usize, usize)>> = prover
        .permutation()
        .mapping()
        .map(ParallelIterator::collect::<Vec<_>>)
        .collect();
    let permutation = tables.permutation();
    let native_mapping: Vec<Vec<(usize, usize)>> = (0..permutation.columns().len())
        .map(|column| {
            (0..permutation.rows())
                .map(|row| permutation.mapping(column, row).expect("in range"))
                .collect()
        })
        .collect();
    assert_eq!(vendored_mapping, native_mapping, "{}: permutation", B::NAME);
    assert!(!permutation.is_identity());
}

#[test]
fn layout_parity_vesta() {
    layout_parity::<Vesta>();
}

#[test]
fn layout_parity_pallas() {
    layout_parity::<Pallas>();
}
