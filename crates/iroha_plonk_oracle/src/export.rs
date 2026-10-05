//! Export of vendored halo2-axiom circuits into the `iroha_plonk` IR (task
//! T15).
//!
//! A vendored circuit is exported in two passes that mirror what the vendored
//! key generator and prover observe:
//!
//! 1. Key-generation pass ([`export_circuit`] on `circuit.without_witnesses()`,
//!    as the vendored `keygen_vk` runs it): a capturing implementation of the
//!    vendored `Assignment` trait records every fixed value, every selector
//!    activation and every copy constraint in call order. It returns unknown
//!    advice values, as the vendored keygen `Assembly` does.
//! 2. Witness pass (on the circuit itself, with the instance values): the
//!    capture records every advice value, as the vendored `WitnessCollection`
//!    does, and answers instance queries from the instance values.
//!
//! The configure-time vendored `ConstraintSystem` (selectors not yet
//! compressed) is replayed into a native [`ConstraintSystem`]
//! ([`export_constraint_system`]): the same columns, selectors, gates (node
//! for node), lookups, equality columns, constants and minimum degree, with
//! the vendored query tables reproduced in their exact interning order.
//! Native key generation then compresses the selectors itself, so the
//! vendored compressed system (`VerifyingKey::cs`) serves as a check of the
//! native compression ([`compare_constraint_systems`]), and the copy
//! constraints are replayed in call order into a native
//! [`PermutationAssembly`], whose union-find merges cycles exactly as the
//! vendored one does.
//!
//! The vendored `transcript_repr` (the hash of the vendored `Debug`
//! rendering) is exported with [`vendored_transcript_repr`]; oracle-mode
//! proving and verification inject it.
//!
//! Only single-phase circuits are exported: PIPA-v1 has no challenges and no
//! later-phase advice. Every failure is a typed [`ExportError`]; the export
//! is stricter than the vendored backends, which panic on misuse.
//!
//! The vendored `Assignment::assign_advice` returns a reference whose lifetime
//! is not tied to the backend; the vendored backends satisfy it with `unsafe`
//! pointer casts. This crate forbids `unsafe`, so the witness pass stores
//! each assigned value in a leaked per-column arena of `n` write-once slots,
//! allocated when the column is first assigned (one allocation per column,
//! not one per cell), and leaks a separate box only for a cell that is
//! assigned again. Test infrastructure only: the leak is bounded by `n` times
//! the advice columns per export, and the golden tests export each `(family,
//! curve, k)` once per process (`tests/vendored_goldens/cases.rs` caches the
//! setups).

use core::fmt;
use std::sync::OnceLock;

use halo2_axiom::{
    SerdeFormat,
    circuit::Value as VValue,
    halo2curves::ff::Field,
    plonk::{
        Advice as VAdvice, Any as VAny, Assigned as VAssigned, Assignment, Challenge,
        Circuit as VCircuit, Column as VColumn, ConstraintSystem as VCs, Error as VError,
        Expression as VExpr, Fixed as VFixed, FloorPlanner, Gate as VGate, Instance as VInstance,
        Selector as VSelector, VerifyingKey as VVerifyingKey,
    },
};
use iroha_plonk::{
    cs::{
        Advice, AdviceQuery, Any, Column, Constraint, ConstraintSystem, CsError, Expression, Fixed,
        FixedQuery, Instance, InstanceModeV1, InstanceQuery, PermutationAssembly, PermutationError,
        ProofSuffixV1, Rotation, Selector, TranscriptV1, VirtualCells,
    },
    keys::{KeyError, KeygenConfig, ProvingKey, keygen_from_tables},
    pcs::ipa::PinnedParams,
    prover::{ProverError, Witness},
};

use crate::convert::{CurveBridge, NativeScalar, native_scalar};

/// A vendored circuit could not be exported.
#[derive(Debug)]
pub enum ExportError {
    /// The vendored floor planner or circuit failed.
    Synthesis(VError),
    /// The circuit uses challenges or later-phase advice columns, which
    /// PIPA-v1 does not have.
    Phases,
    /// An expression queries a challenge.
    Challenge,
    /// `2^k` does not fit, or the domain is smaller than the circuit needs.
    Rows {
        /// The domain size exponent.
        k: u32,
    },
    /// The number of instance columns differs from the constraint system's.
    InstanceColumns {
        /// The constraint system's count.
        expected: usize,
        /// The supplied count.
        found: usize,
    },
    /// A cell lies outside the usable rows.
    Row {
        /// What was assigned (`"fixed"`, `"copy"`, `"advice"`).
        what: &'static str,
        /// The row.
        row: usize,
        /// The number of usable rows.
        usable_rows: usize,
    },
    /// A column index is outside the constraint system.
    Column {
        /// The column kind.
        what: &'static str,
        /// The index.
        index: usize,
    },
    /// The witness pass assigned an unknown advice value.
    UnknownAdvice {
        /// The advice column.
        column: usize,
        /// The row.
        row: usize,
    },
    /// The native constraint system rejected the replay.
    ConstraintSystem(CsError),
    /// The replayed native query tables differ from the vendored ones.
    QueryOrder,
    /// A copy constraint could not be replayed natively.
    Permutation(PermutationError),
    /// Native key generation failed.
    Key(KeyError),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Synthesis(error) => write!(f, "vendored synthesis: {error}"),
            Self::Phases => f.write_str("multi-phase circuits have no PIPA-v1 export"),
            Self::Challenge => f.write_str("an expression queries a challenge"),
            Self::Rows { k } => write!(f, "k = {k} has too few rows for the circuit"),
            Self::InstanceColumns { expected, found } => {
                write!(
                    f,
                    "{found} instance columns supplied, {expected} configured"
                )
            }
            Self::Row {
                what,
                row,
                usable_rows,
            } => write!(
                f,
                "{what} at row {row}, outside the {usable_rows} usable rows"
            ),
            Self::Column { what, index } => write!(f, "{what} column {index} does not exist"),
            Self::UnknownAdvice { column, row } => {
                write!(f, "advice column {column} row {row} has no witness value")
            }
            Self::ConstraintSystem(error) => write!(f, "native constraint system: {error}"),
            Self::QueryOrder => f.write_str("the native query tables differ from the vendored"),
            Self::Permutation(error) => write!(f, "copy constraint: {error}"),
            Self::Key(error) => write!(f, "native key generation: {error}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<CsError> for ExportError {
    fn from(error: CsError) -> Self {
        Self::ConstraintSystem(error)
    }
}

impl From<PermutationError> for ExportError {
    fn from(error: PermutationError) -> Self {
        Self::Permutation(error)
    }
}

impl From<KeyError> for ExportError {
    fn from(error: KeyError) -> Self {
        Self::Key(error)
    }
}

/// One copy constraint `left == right`, as `(column, row)` cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyConstraint {
    /// The left cell.
    pub left: (Column<Any>, usize),
    /// The right cell.
    pub right: (Column<Any>, usize),
}

/// The native column of a vendored column of any kind.
///
/// # Errors
///
/// [`ExportError::Phases`] for a later-phase advice column.
pub fn export_column(column: &VColumn<VAny>) -> Result<Column<Any>, ExportError> {
    let kind = match column.column_type() {
        VAny::Advice(advice) if advice.phase() == 0 => Any::Advice,
        VAny::Advice(_) => return Err(ExportError::Phases),
        VAny::Fixed => Any::Fixed,
        VAny::Instance => Any::Instance,
    };
    Ok(Column::new(column.index(), kind))
}

/// The native expression of a vendored expression: the same tree, node for
/// node, with constants re-encoded.
///
/// # Errors
///
/// [`ExportError::Challenge`] for a challenge query and
/// [`ExportError::Phases`] for a later-phase advice query.
pub fn export_expression<B: CurveBridge>(
    expression: &VExpr<B::VScalar>,
) -> Result<Expression<NativeScalar<B>>, ExportError> {
    let boxed = |inner: &VExpr<B::VScalar>| export_expression::<B>(inner).map(Box::new);
    Ok(match expression {
        VExpr::Constant(value) => Expression::Constant(native_scalar::<B>(value)),
        VExpr::Selector(selector) => {
            Expression::Selector(Selector::new(selector.index(), selector.is_simple()))
        }
        VExpr::Fixed(query) => Expression::Fixed(FixedQuery {
            column_index: query.column_index(),
            rotation: Rotation(query.rotation().0),
        }),
        VExpr::Advice(query) => {
            if query.phase() != 0 {
                return Err(ExportError::Phases);
            }
            Expression::Advice(AdviceQuery {
                column_index: query.column_index(),
                rotation: Rotation(query.rotation().0),
            })
        }
        VExpr::Instance(query) => Expression::Instance(InstanceQuery {
            column_index: query.column_index(),
            rotation: Rotation(query.rotation().0),
        }),
        VExpr::Challenge(_) => return Err(ExportError::Challenge),
        VExpr::Negated(inner) => Expression::Negated(boxed(inner)?),
        VExpr::Sum(left, right) => Expression::Sum(boxed(left)?, boxed(right)?),
        VExpr::Product(left, right) => Expression::Product(boxed(left)?, boxed(right)?),
        VExpr::Scaled(inner, factor) => {
            Expression::Scaled(boxed(inner)?, native_scalar::<B>(factor))
        }
    })
}

/// Whether each selector of `cs` is simple. A selector that no expression
/// reads is reported as complex; compression treats an unread selector the
/// same either way (its gate degree is 0).
fn selector_kinds<F: Field>(cs: &VCs<F>) -> Vec<bool> {
    fn visit<F: Field>(expression: &VExpr<F>, simple: &mut [bool]) {
        match expression {
            VExpr::Selector(selector) => {
                if let Some(slot) = simple.get_mut(selector.index()) {
                    *slot |= selector.is_simple();
                }
            }
            VExpr::Negated(inner) | VExpr::Scaled(inner, _) => visit(inner, simple),
            VExpr::Sum(left, right) | VExpr::Product(left, right) => {
                visit(left, simple);
                visit(right, simple);
            }
            _ => {}
        }
    }
    let mut simple = vec![false; cs.num_selectors()];
    for poly in cs.gates().iter().flat_map(VGate::polynomials) {
        visit(poly, &mut simple);
    }
    for lookup in cs.lookups() {
        for expression in lookup
            .input_expressions()
            .iter()
            .chain(lookup.table_expressions())
        {
            visit(expression, &mut simple);
        }
    }
    simple
}

/// Interns every vendored query, in the vendored table order, through
/// immediate `VirtualCells` queries.
fn intern_vendored_queries<F: Field, G: iroha_pasta::PastaField>(
    cells: &mut VirtualCells<'_, G>,
    vendored: &VCs<F>,
) {
    for (column, rotation) in vendored.fixed_queries() {
        cells.query_fixed(Column::new(column.index(), Fixed), Rotation(rotation.0));
    }
    for (column, rotation) in vendored.advice_queries() {
        cells.query_advice(Column::new(column.index(), Advice), Rotation(rotation.0));
    }
    for (column, rotation) in vendored.instance_queries() {
        cells.query_instance(Column::new(column.index(), Instance), Rotation(rotation.0));
    }
}

/// The `(column, rotation)` pairs of a query table.
fn query_pairs<C: Copy, R: Copy>(
    queries: &[(C, R)],
    index: impl Fn(C) -> usize,
    rotation: impl Fn(R) -> i32,
) -> Vec<(usize, i32)> {
    queries
        .iter()
        .map(|(column, at)| (index(*column), rotation(*at)))
        .collect()
}

/// Replays a configure-time vendored constraint system into the native
/// builder.
///
/// Columns, selectors (simple or complex as the expressions read them),
/// gates, lookups, equality columns (in `enable_equality` order), constants
/// and the minimum degree are reproduced; `instance_lengths` gives the exact
/// length of every instance column (PIPA-v1 fixes them, the vendored system
/// does not). The vendored query tables depend on the order of the original
/// `configure` calls, which the finished system no longer records, so every
/// vendored query is interned first, in vendored order, through immediate
/// queries in the first gate (or, without gates, the first lookup). That
/// gate's diagnostic cell list therefore names every query; nothing a
/// descriptor, key or proof depends on changes. Lookups are replayed as
/// `lookup_any` with the vendored table expressions.
///
/// # Errors
///
/// [`ExportError::Phases`], [`ExportError::Challenge`],
/// [`ExportError::InstanceColumns`], [`ExportError::ConstraintSystem`] when
/// the native builder records an error, and [`ExportError::QueryOrder`] when
/// the replayed query tables differ from the vendored ones.
pub fn export_constraint_system<B: CurveBridge>(
    vendored: &VCs<B::VScalar>,
    instance_lengths: &[usize],
) -> Result<ConstraintSystem<NativeScalar<B>>, ExportError> {
    if vendored.num_challenges() != 0 || vendored.advice_column_phase().iter().any(|p| *p != 0) {
        return Err(ExportError::Phases);
    }
    if instance_lengths.len() != vendored.num_instance_columns() {
        return Err(ExportError::InstanceColumns {
            expected: vendored.num_instance_columns(),
            found: instance_lengths.len(),
        });
    }
    let mut cs = ConstraintSystem::<NativeScalar<B>>::new();
    for _ in 0..vendored.num_fixed_columns() {
        cs.fixed_column();
    }
    for _ in 0..vendored.num_advice_columns() {
        cs.advice_column();
    }
    for &length in instance_lengths {
        cs.instance_column(length);
    }
    for simple in selector_kinds(vendored) {
        if simple {
            cs.selector();
        } else {
            cs.complex_selector();
        }
    }

    let mut interned = false;
    for gate in vendored.gates() {
        let constraints = gate
            .polynomials()
            .iter()
            .enumerate()
            .map(|(index, poly)| {
                export_expression::<B>(poly)
                    .map(|poly| Constraint::from((gate.constraint_name(index).to_owned(), poly)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = !interned;
        cs.create_gate(gate.name(), |cells| {
            if first {
                intern_vendored_queries(cells, vendored);
            }
            constraints
        });
        interned = true;
    }
    for lookup in vendored.lookups() {
        let pairs = lookup
            .input_expressions()
            .iter()
            .zip(lookup.table_expressions())
            .map(|(input, table)| {
                Ok((
                    export_expression::<B>(input)?,
                    export_expression::<B>(table)?,
                ))
            })
            .collect::<Result<Vec<_>, ExportError>>()?;
        let first = !interned;
        cs.lookup_any(lookup.name(), |cells| {
            if first {
                intern_vendored_queries(cells, vendored);
            }
            pairs
        });
        interned = true;
    }
    for column in vendored.permutation().get_columns() {
        cs.enable_equality(export_column(&column)?);
    }
    for column in vendored.constants() {
        cs.enable_constant(Column::new(column.index(), Fixed));
    }
    // `degree = max(3, lookups, gates, minimum)`: the expressions are equal,
    // so a larger vendored degree can only come from `set_minimum_degree`.
    if vendored.degree() > cs.degree() {
        cs.set_minimum_degree(vendored.degree());
    }
    cs.check()?;

    let same_tables = query_pairs(vendored.fixed_queries(), |c| c.index(), |r| r.0)
        == query_pairs(cs.fixed_queries(), |c| c.index(), |r| r.0)
        && query_pairs(vendored.advice_queries(), |c| c.index(), |r| r.0)
            == query_pairs(cs.advice_queries(), |c| c.index(), |r| r.0)
        && query_pairs(vendored.instance_queries(), |c| c.index(), |r| r.0)
            == query_pairs(cs.instance_queries(), |c| c.index(), |r| r.0);
    if !same_tables {
        return Err(ExportError::QueryOrder);
    }
    Ok(cs)
}

/// The first part in which two constraint systems differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CsMismatch {
    /// The part (`"degree"`, `"advice queries"`, `"gate"`, ...).
    pub part: &'static str,
    /// The gate or lookup index, for per-item parts.
    pub index: Option<usize>,
}

impl fmt::Display for CsMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.index {
            Some(index) => write!(f, "{} {index} differs", self.part),
            None => write!(f, "{} differs", self.part),
        }
    }
}

impl std::error::Error for CsMismatch {}

/// Compares a vendored and a native constraint system in every part that
/// descriptors, keys and proofs depend on: degree, blinding factors, minimum
/// rows, column and selector counts, the three query tables, the equality
/// columns, the constants, every gate (names and polynomials, node for node)
/// and every lookup.
///
/// # Errors
///
/// The first [`CsMismatch`]. An expression the export rejects (a challenge or
/// a later phase) is reported as a mismatch of its gate or lookup.
pub fn compare_constraint_systems<B: CurveBridge>(
    vendored: &VCs<B::VScalar>,
    native: &ConstraintSystem<NativeScalar<B>>,
) -> Result<(), CsMismatch> {
    let check = |equal: bool, part: &'static str, index: Option<usize>| {
        if equal {
            Ok(())
        } else {
            Err(CsMismatch { part, index })
        }
    };
    check(vendored.degree() == native.degree(), "degree", None)?;
    check(
        vendored.blinding_factors() == native.blinding_factors(),
        "blinding factors",
        None,
    )?;
    check(
        vendored.minimum_rows() == native.minimum_rows(),
        "minimum rows",
        None,
    )?;
    check(
        vendored.num_fixed_columns() == native.num_fixed_columns(),
        "fixed columns",
        None,
    )?;
    check(
        vendored.num_advice_columns() == native.num_advice_columns(),
        "advice columns",
        None,
    )?;
    check(
        vendored.num_instance_columns() == native.num_instance_columns(),
        "instance columns",
        None,
    )?;
    check(
        vendored.num_selectors() == native.num_selectors(),
        "selectors",
        None,
    )?;
    check(
        query_pairs(vendored.fixed_queries(), |c| c.index(), |r| r.0)
            == query_pairs(native.fixed_queries(), |c| c.index(), |r| r.0),
        "fixed queries",
        None,
    )?;
    check(
        query_pairs(vendored.advice_queries(), |c| c.index(), |r| r.0)
            == query_pairs(native.advice_queries(), |c| c.index(), |r| r.0),
        "advice queries",
        None,
    )?;
    check(
        query_pairs(vendored.instance_queries(), |c| c.index(), |r| r.0)
            == query_pairs(native.instance_queries(), |c| c.index(), |r| r.0),
        "instance queries",
        None,
    )?;
    let permutation = vendored
        .permutation()
        .get_columns()
        .iter()
        .map(|column| export_column(column).ok())
        .collect::<Vec<_>>();
    check(
        permutation
            == native
                .permutation()
                .columns()
                .iter()
                .copied()
                .map(Some)
                .collect::<Vec<_>>(),
        "equality columns",
        None,
    )?;
    check(
        vendored
            .constants()
            .iter()
            .map(VColumn::index)
            .eq(native.constants().iter().map(Column::index)),
        "constants",
        None,
    )?;
    check(
        vendored.gates().len() == native.gates().len(),
        "gate count",
        None,
    )?;
    for (index, (v, n)) in vendored.gates().iter().zip(native.gates()).enumerate() {
        let polys = v
            .polynomials()
            .iter()
            .map(|poly| export_expression::<B>(poly).ok())
            .collect::<Vec<_>>();
        let names_equal = v.name() == n.name()
            && (0..v.polynomials().len()).all(|i| v.constraint_name(i) == n.constraint_name(i));
        let polys_equal = polys
            == n.polynomials()
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<_>>();
        check(names_equal && polys_equal, "gate", Some(index))?;
    }
    check(
        vendored.lookups().len() == native.lookups().len(),
        "lookup count",
        None,
    )?;
    for (index, (v, n)) in vendored.lookups().iter().zip(native.lookups()).enumerate() {
        let convert = |expressions: &[VExpr<B::VScalar>]| {
            expressions
                .iter()
                .map(|e| export_expression::<B>(e).ok())
                .collect::<Vec<_>>()
        };
        let wrap = |expressions: &[Expression<NativeScalar<B>>]| {
            expressions.iter().cloned().map(Some).collect::<Vec<_>>()
        };
        check(
            convert(v.input_expressions()) == wrap(n.input_expressions())
                && convert(v.table_expressions()) == wrap(n.table_expressions()),
            "lookup",
            Some(index),
        )?;
    }
    Ok(())
}

/// What a capture records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    /// Fixed values, selectors and copies (the vendored keygen `Assembly`).
    Keygen,
    /// Advice values (the vendored prover's `WitnessCollection`).
    Witness,
}

/// A leaked column of write-once slots for assigned advice values.
type Arena<F> = &'static [OnceLock<VAssigned<F>>];

/// A recording implementation of the vendored `Assignment` trait.
struct Capture<'a, F: Field + 'static> {
    pass: Pass,
    k: u32,
    n: usize,
    usable_rows: usize,
    fixed: Vec<Vec<F>>,
    selectors: Vec<Vec<bool>>,
    copies: Vec<CopyConstraint>,
    advice: Vec<Vec<F>>,
    /// Per advice column, the arena that backs the returned references.
    arenas: Vec<Option<Arena<F>>>,
    instances: &'a [Vec<F>],
    error: Option<ExportError>,
}

impl<'a, F: Field + 'static> Capture<'a, F> {
    /// An empty capture for `cs` at `n = 2^k` rows.
    fn new(pass: Pass, k: u32, n: usize, cs: &VCs<F>, instances: &'a [Vec<F>]) -> Option<Self> {
        let usable_rows = n.checked_sub(cs.blinding_factors().checked_add(1)?)?;
        let (fixed, selectors, advice) = match pass {
            Pass::Keygen => (
                vec![vec![F::ZERO; n]; cs.num_fixed_columns()],
                vec![vec![false; n]; cs.num_selectors()],
                Vec::new(),
            ),
            Pass::Witness => (
                Vec::new(),
                Vec::new(),
                vec![vec![F::ZERO; n]; cs.num_advice_columns()],
            ),
        };
        let arenas = vec![None; advice.len()];
        Some(Self {
            pass,
            k,
            n,
            usable_rows,
            fixed,
            selectors,
            copies: Vec::new(),
            advice,
            arenas,
            instances,
            error: None,
        })
    }

    /// A reference to `value` that outlives the capture (see the module
    /// documentation): the cell's slot in its column arena, or a separate
    /// leaked box when the slot is taken (a reassigned cell) or the cell is
    /// out of range (already reported by [`Capture::record_advice`]).
    fn store(&mut self, column: usize, row: usize, value: VAssigned<F>) -> &'static VAssigned<F> {
        let n = self.n;
        let arena = self.arenas.get_mut(column).map(|arena| {
            *arena.get_or_insert_with(|| {
                let slots: Vec<OnceLock<VAssigned<F>>> = (0..n).map(|_| OnceLock::new()).collect();
                Box::leak(slots.into_boxed_slice())
            })
        });
        match arena.and_then(|slots| slots.get(row)) {
            Some(slot) if slot.get().is_none() => slot.get_or_init(|| value),
            _ => Box::leak(Box::new(value)),
        }
    }

    /// Records the first error.
    fn fail(&mut self, error: ExportError) {
        if self.error.is_none() {
            self.error = Some(error);
        }
    }

    /// The vendored "not enough rows" error.
    const fn not_enough_rows(&self) -> VError {
        VError::NotEnoughRowsAvailable { current_k: self.k }
    }

    /// Records an advice value in the witness pass; returns it when known.
    fn record_advice(
        &mut self,
        column: VColumn<VAdvice>,
        row: usize,
        to: VValue<VAssigned<F>>,
    ) -> Option<VAssigned<F>> {
        if self.pass != Pass::Witness {
            return None;
        }
        if column.column_type().phase() != 0 {
            self.fail(ExportError::Phases);
            return None;
        }
        let mut known = None;
        to.map(|value| known = Some(value));
        let Some(value) = known else {
            self.fail(ExportError::UnknownAdvice {
                column: column.index(),
                row,
            });
            return None;
        };
        if row >= self.usable_rows {
            self.fail(ExportError::Row {
                what: "advice",
                row,
                usable_rows: self.usable_rows,
            });
            return None;
        }
        match self
            .advice
            .get_mut(column.index())
            .and_then(|cells| cells.get_mut(row))
        {
            Some(cell) => *cell = value.evaluate(),
            None => self.fail(ExportError::Column {
                what: "advice",
                index: column.index(),
            }),
        }
        Some(value)
    }
}

impl<F: Field + 'static> Assignment<F> for Capture<'_, F> {
    fn enter_region<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn annotate_column<A, AR>(&mut self, _: A, _: VColumn<VAny>)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
    }

    fn exit_region(&mut self) {}

    fn enable_selector<A, AR>(
        &mut self,
        _: A,
        selector: &VSelector,
        row: usize,
    ) -> Result<(), VError>
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        if row >= self.usable_rows {
            return Err(self.not_enough_rows());
        }
        if self.pass == Pass::Keygen {
            let cell = self
                .selectors
                .get_mut(selector.index())
                .and_then(|rows| rows.get_mut(row))
                .ok_or(VError::BoundsFailure)?;
            *cell = true;
        }
        Ok(())
    }

    fn query_instance(&self, column: VColumn<VInstance>, row: usize) -> Result<VValue<F>, VError> {
        if row >= self.usable_rows {
            return Err(self.not_enough_rows());
        }
        match self.pass {
            Pass::Keygen => Ok(VValue::unknown()),
            Pass::Witness => self
                .instances
                .get(column.index())
                .and_then(|values| values.get(row))
                .map(|value| VValue::known(*value))
                .ok_or(VError::BoundsFailure),
        }
    }

    fn assign_advice<'v>(
        &mut self,
        column: VColumn<VAdvice>,
        row: usize,
        to: VValue<VAssigned<F>>,
    ) -> VValue<&'v VAssigned<F>> {
        // See the module documentation: the returned reference outlives
        // `self`, which only leaked storage provides without `unsafe`.
        self.record_advice(column, row, to)
            .map_or_else(VValue::unknown, |value| {
                let stored: &'v VAssigned<F> = self.store(column.index(), row, value);
                VValue::known(stored)
            })
    }

    fn assign_advice_discarding_value(
        &mut self,
        column: VColumn<VAdvice>,
        row: usize,
        to: VValue<VAssigned<F>>,
    ) {
        let _ = self.record_advice(column, row, to);
    }

    fn assign_fixed(&mut self, column: VColumn<VFixed>, row: usize, to: VAssigned<F>) {
        if self.pass != Pass::Keygen {
            return;
        }
        if row >= self.usable_rows {
            self.fail(ExportError::Row {
                what: "fixed",
                row,
                usable_rows: self.usable_rows,
            });
            return;
        }
        match self
            .fixed
            .get_mut(column.index())
            .and_then(|cells| cells.get_mut(row))
        {
            Some(cell) => *cell = to.evaluate(),
            None => self.fail(ExportError::Column {
                what: "fixed",
                index: column.index(),
            }),
        }
    }

    fn copy(
        &mut self,
        left_column: VColumn<VAny>,
        left_row: usize,
        right_column: VColumn<VAny>,
        right_row: usize,
    ) {
        if self.pass != Pass::Keygen {
            return;
        }
        for row in [left_row, right_row] {
            if row >= self.usable_rows {
                self.fail(ExportError::Row {
                    what: "copy",
                    row,
                    usable_rows: self.usable_rows,
                });
                return;
            }
        }
        match (export_column(&left_column), export_column(&right_column)) {
            (Ok(left), Ok(right)) => self.copies.push(CopyConstraint {
                left: (left, left_row),
                right: (right, right_row),
            }),
            (Err(error), _) | (_, Err(error)) => self.fail(error),
        }
    }

    fn fill_from_row(
        &mut self,
        column: VColumn<VFixed>,
        from_row: usize,
        to: VValue<VAssigned<F>>,
    ) -> Result<(), VError> {
        if self.pass != Pass::Keygen {
            return Ok(());
        }
        if from_row >= self.usable_rows {
            return Err(self.not_enough_rows());
        }
        let mut known = None;
        to.map(|value| known = Some(value));
        let filler = known.ok_or(VError::Synthesis)?.evaluate();
        let usable_rows = self.usable_rows;
        let cells = self
            .fixed
            .get_mut(column.index())
            .ok_or(VError::BoundsFailure)?;
        for cell in cells.iter_mut().take(usable_rows).skip(from_row) {
            *cell = filler;
        }
        Ok(())
    }

    fn get_challenge(&self, _: Challenge) -> VValue<F> {
        VValue::unknown()
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self, _: Option<String>) {}
}

/// Runs the vendored floor planner of `circuit` into a capture.
fn capture<'a, F: Field + 'static, C: VCircuit<F>>(
    pass: Pass,
    k: u32,
    circuit: &C,
    cs: &VCs<F>,
    config: C::Config,
    instances: &'a [Vec<F>],
) -> Result<Capture<'a, F>, ExportError> {
    let n = 1_usize.checked_shl(k).ok_or(ExportError::Rows { k })?;
    if n < cs.minimum_rows() {
        return Err(ExportError::Rows { k });
    }
    let mut capture = Capture::new(pass, k, n, cs, instances).ok_or(ExportError::Rows { k })?;
    C::FloorPlanner::synthesize(&mut capture, circuit, config, cs.constants().clone())
        .map_err(ExportError::Synthesis)?;
    let error = capture.error.take();
    error.map_or(Ok(capture), Err)
}

/// The configure-time vendored constraint system of `circuit` and its
/// configuration, as the vendored key generator and prover build them.
pub fn configure_vendored<F: Field, C: VCircuit<F>>(circuit: &C) -> (VCs<F>, C::Config) {
    let mut cs = VCs::default();
    let config = C::configure_with_params(&mut cs, circuit.params());
    (cs, config)
}

/// A vendored circuit exported into the native IR: everything native key
/// generation and proving take.
#[derive(Clone, Debug)]
pub struct ExportedCircuit<B: CurveBridge> {
    k: u32,
    cs: ConstraintSystem<NativeScalar<B>>,
    fixed: Vec<Vec<NativeScalar<B>>>,
    selectors: Vec<Vec<bool>>,
    copies: Vec<CopyConstraint>,
    advice: Vec<Vec<NativeScalar<B>>>,
    instances: Vec<Vec<NativeScalar<B>>>,
}

/// Exports `circuit` at `n = 2^k` rows with the instance columns
/// `instances` (their lengths become the descriptor's exact lengths): the
/// configure-time constraint system ([`export_constraint_system`]), the
/// key-generation tables from `circuit.without_witnesses()` and the advice
/// table from `circuit`.
///
/// # Errors
///
/// [`ExportError`] when the circuit cannot be configured, synthesized or
/// replayed natively.
pub fn export_circuit<B: CurveBridge, C: VCircuit<B::VScalar>>(
    k: u32,
    circuit: &C,
    instances: &[Vec<B::VScalar>],
) -> Result<ExportedCircuit<B>, ExportError> {
    let (cs, config) = configure_vendored::<B::VScalar, C>(circuit);
    let lengths = instances.iter().map(Vec::len).collect::<Vec<_>>();
    let native_cs = export_constraint_system::<B>(&cs, &lengths)?;
    let keygen = capture(
        Pass::Keygen,
        k,
        &circuit.without_witnesses(),
        &cs,
        config.clone(),
        &[],
    )?;
    let witness = capture(Pass::Witness, k, circuit, &cs, config, instances)?;
    let convert = |columns: Vec<Vec<B::VScalar>>| {
        columns
            .iter()
            .map(|column| column.iter().map(native_scalar::<B>).collect())
            .collect::<Vec<Vec<_>>>()
    };
    Ok(ExportedCircuit {
        k,
        cs: native_cs,
        fixed: convert(keygen.fixed),
        selectors: keygen.selectors,
        copies: keygen.copies,
        advice: convert(witness.advice),
        instances: convert(instances.to_vec()),
    })
}

impl<B: CurveBridge> ExportedCircuit<B> {
    /// `log2` of the domain size.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// The native configure-time constraint system (selectors not yet
    /// compressed).
    pub fn constraint_system(&self) -> &ConstraintSystem<NativeScalar<B>> {
        &self.cs
    }

    /// The configure-time fixed columns, `n` rows each (unassigned cells are
    /// zero).
    pub fn fixed(&self) -> &[Vec<NativeScalar<B>>] {
        &self.fixed
    }

    /// The selector activations, `n` rows each.
    pub fn selectors(&self) -> &[Vec<bool>] {
        &self.selectors
    }

    /// The copy constraints in call order.
    pub fn copies(&self) -> &[CopyConstraint] {
        &self.copies
    }

    /// The advice columns, `n` rows each (unassigned cells and the rows from
    /// the first blinding row on are zero).
    pub fn advice(&self) -> &[Vec<NativeScalar<B>>] {
        &self.advice
    }

    /// The instance columns.
    pub fn instances(&self) -> &[Vec<NativeScalar<B>>] {
        &self.instances
    }

    /// The copy constraints replayed in call order into a native
    /// permutation assembly.
    ///
    /// # Errors
    ///
    /// [`ExportError::Permutation`] when a copy names a column without
    /// equality, and [`ExportError::Rows`] when `2^k` does not fit.
    pub fn permutation(&self) -> Result<PermutationAssembly, ExportError> {
        let n = 1_usize
            .checked_shl(self.k)
            .ok_or(ExportError::Rows { k: self.k })?;
        let mut assembly = PermutationAssembly::new(n, self.cs.permutation())?;
        for copy in &self.copies {
            assembly.copy(copy.left.0, copy.left.1, copy.right.0, copy.right.1)?;
        }
        Ok(assembly)
    }

    /// Native key generation from the exported tables
    /// ([`keygen_from_tables`]).
    ///
    /// # Errors
    ///
    /// The errors of [`ExportedCircuit::permutation`] and
    /// [`ExportError::Key`].
    pub fn keygen(
        &self,
        params: &PinnedParams<B::Native>,
        config: &KeygenConfig,
    ) -> Result<ProvingKey<B::Native>, ExportError> {
        let permutation = self.permutation()?;
        Ok(keygen_from_tables(
            params,
            self.cs.clone(),
            self.fixed.clone(),
            self.selectors.clone(),
            &permutation,
            config,
        )?)
    }

    /// The exported advice and instance columns as a native witness for
    /// `pk` ([`Witness::from_columns`]).
    ///
    /// # Errors
    ///
    /// The shape errors of [`Witness::from_columns`].
    pub fn witness(
        &self,
        pk: &ProvingKey<B::Native>,
    ) -> Result<Witness<NativeScalar<B>>, ProverError> {
        Witness::from_columns(pk, self.advice.clone(), self.instances.clone())
    }
}

/// The vendored `transcript_repr` of `vk` (the hash of its `Debug`
/// rendering) as a native scalar, for oracle-mode proving and verification.
pub fn vendored_transcript_repr<B: CurveBridge>(
    vk: &VVerifyingKey<B::Vendored>,
) -> NativeScalar<B> {
    native_scalar::<B>(&vk.transcript_repr())
}

/// The vendored `0x02` verifying-key bytes (`Processed` format), which the
/// native key must reproduce.
pub fn vendored_vk_bytes<B: CurveBridge>(vk: &VVerifyingKey<B::Vendored>) -> Vec<u8> {
    vk.to_bytes(SerdeFormat::Processed)
}

/// Offset of the compress flag in the `0x02` layout (after the version byte
/// and the little-endian `u32` `k`).
const VK_COMPRESS_OFFSET: usize = 5;

/// Whether the vendored key compressed its selectors. The halo2-axiom
/// `keygen_vk` default is off; `keygen_vk_custom` chooses. The flag is read
/// from the encoded key, as the vendored type does not expose it.
pub fn vendored_compress_selectors<B: CurveBridge>(vk: &VVerifyingKey<B::Vendored>) -> bool {
    vendored_vk_bytes::<B>(vk).get(VK_COMPRESS_OFFSET) == Some(&1)
}

/// The native key-generation configuration that reproduces the vendored key
/// and the proofs of one vendored proving path: Committed instances (the
/// vendored `ProverIPA` default), the vendored selector-compression choice,
/// and the path's transcript and suffix:
///
/// - `(Blake2bChallenge255, None)`: the vendored `Blake2bWrite` path of the
///   halo2-axiom goldens;
/// - `(KagemushaPoseidonRp57, FoldedGenerator)`: the KAGEMUSHA path of
///   `iroha_core_zk` (snark-verifier `PoseidonTranscript`, then the folded
///   generator appended).
///
/// The verifying-key bytes do not depend on the transcript or the suffix.
pub fn vendored_keygen_config<B: CurveBridge>(
    vk: &VVerifyingKey<B::Vendored>,
    transcript: TranscriptV1,
    proof_suffix: ProofSuffixV1,
) -> KeygenConfig {
    let mut config = KeygenConfig::new(transcript);
    config.instance_mode = InstanceModeV1::Committed;
    config.proof_suffix = proof_suffix;
    config.compress_selectors = vendored_compress_selectors::<B>(vk);
    config
}

#[cfg(test)]
mod tests;
