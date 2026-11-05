//! Bounded-integer circuit builder for game step functions.
//!
//! A game describes one step (one tick) of its deterministic simulation with a [`Builder`]. Every
//! value is an integer with a statically known interval far below `p/2`, so field arithmetic never
//! wraps. The builder records, for each committed column, both its defining constraints and a
//! *hint* that computes its value, so the same description yields the AIR and the witness generator.
//!
//! One trace row holds the state before a step plus every intermediate of that step. Rows after the
//! last step repeat the final state (the public `active` selector freezes transitions). Range checks
//! are LogUp lookups of `L = log2(rows)`-bit limbs into the row-index column.
//!
//! Gadget soundness summary (every new column is fully determined by its constraints):
//! - products and selects: one degree-2 equation;
//! - comparisons: a Boolean bit plus a range-checked slack that is negative unless the bit is right;
//! - divisions by constants: quotient and remainder are both range-checked, which forces exact
//!   integer division (`c*q = a - r` with small `q`, `r`);
//! - one-hot selectors: Booleans summing to one whose weighted sum is the index;
//! - nonzero assertions: an inverse witness.

use std::collections::BTreeMap;
use std::fmt;

/// A value available when a row is evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Var {
    /// Committed main column at the current row.
    Main(u32),
    /// Committed main column at the next row.
    Next(u32),
    /// Public (verifier-computed) column at the current row.
    Public(u32),
}

/// A linear combination `constant + sum coeff * var` with integer coefficients.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lin {
    /// Constant term.
    pub constant: i64,
    /// Sorted terms with nonzero coefficients.
    pub terms: Vec<(Var, i64)>,
}

impl Lin {
    fn from_terms(constant: i64, terms: BTreeMap<Var, i128>) -> Self {
        Self {
            constant,
            terms: terms
                .into_iter()
                .filter(|(_, c)| *c != 0)
                .map(|(v, c)| (v, i64::try_from(c).expect("coefficient overflow")))
                .collect(),
        }
    }

    /// A single variable.
    pub fn var(var: Var) -> Self {
        Self {
            constant: 0,
            terms: vec![(var, 1)],
        }
    }

    /// A constant.
    pub fn constant(value: i64) -> Self {
        Self {
            constant: value,
            terms: Vec::new(),
        }
    }

    fn combine(&self, self_scale: i64, other: &Self, other_scale: i64) -> Self {
        let mut terms: BTreeMap<Var, i128> = BTreeMap::new();
        for (v, c) in &self.terms {
            *terms.entry(*v).or_default() += i128::from(*c) * i128::from(self_scale);
        }
        for (v, c) in &other.terms {
            *terms.entry(*v).or_default() += i128::from(*c) * i128::from(other_scale);
        }
        let constant = i128::from(self.constant) * i128::from(self_scale)
            + i128::from(other.constant) * i128::from(other_scale);
        Self::from_terms(i64::try_from(constant).expect("constant overflow"), terms)
    }

    /// Replaces every `Main(c)` with `Next(c)`.
    #[must_use]
    pub fn to_next(&self) -> Self {
        Self {
            constant: self.constant,
            terms: self
                .terms
                .iter()
                .map(|(v, c)| match v {
                    Var::Main(i) => (Var::Next(*i), *c),
                    other => (*other, *c),
                })
                .collect(),
        }
    }

    /// Evaluates with an integer lookup.
    pub fn eval_i(&self, value: impl Fn(Var) -> i128) -> i128 {
        self.terms
            .iter()
            .fold(i128::from(self.constant), |acc, (v, c)| {
                acc + i128::from(*c) * value(*v)
            })
    }

    /// Whether the expression is constant.
    pub fn is_constant(&self) -> bool {
        self.terms.is_empty()
    }
}

/// A user-facing bounded integer expression: a linear combination with a proven interval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    lin: Lin,
    lo: i128,
    hi: i128,
    boolean: bool,
}

/// Largest magnitude any intermediate integer may reach (far below `p/2 ~ 2^63`).
pub const MAX_MAGNITUDE: i128 = 1 << 60;

impl Expr {
    fn new(lin: Lin, lo: i128, hi: i128, boolean: bool) -> Self {
        assert!(lo <= hi, "empty interval");
        assert!(
            lo.abs() <= MAX_MAGNITUDE && hi.abs() <= MAX_MAGNITUDE,
            "interval [{lo}, {hi}] exceeds the safe integer range"
        );
        Self {
            lin,
            lo,
            hi,
            boolean,
        }
    }

    /// The constant `value`.
    pub fn constant(value: i64) -> Self {
        Self::new(
            Lin::constant(value),
            i128::from(value),
            i128::from(value),
            value == 0 || value == 1,
        )
    }

    /// The linear form.
    pub fn lin(&self) -> &Lin {
        &self.lin
    }

    /// Inclusive lower bound.
    pub fn lo(&self) -> i128 {
        self.lo
    }

    /// Inclusive upper bound.
    pub fn hi(&self) -> i128 {
        self.hi
    }

    /// Whether the value is known to be 0 or 1.
    pub fn is_boolean(&self) -> bool {
        self.boolean
    }

    /// The constant value, when the expression has no variables.
    pub fn as_constant(&self) -> Option<i64> {
        self.lin.is_constant().then_some(self.lin.constant)
    }

    /// `self + other`.
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self::new(
            self.lin.combine(1, &other.lin, 1),
            self.lo + other.lo,
            self.hi + other.hi,
            false,
        )
    }

    /// `self - other`.
    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        Self::new(
            self.lin.combine(1, &other.lin, -1),
            self.lo - other.hi,
            self.hi - other.lo,
            false,
        )
    }

    /// `self * k` for a constant `k`.
    #[must_use]
    pub fn scale(&self, k: i64) -> Self {
        let k128 = i128::from(k);
        let (a, b) = (self.lo * k128, self.hi * k128);
        Self::new(
            self.lin.combine(k, &Lin::default(), 0),
            a.min(b),
            a.max(b),
            k == 1 && self.boolean,
        )
    }

    /// `self + c` for a constant `c`.
    #[must_use]
    pub fn offset(&self, c: i64) -> Self {
        self.add(&Self::constant(c))
    }

    /// `-self`.
    #[must_use]
    pub fn neg(&self) -> Self {
        self.scale(-1)
    }

    /// `1 - self` for a Boolean.
    ///
    /// # Panics
    /// Panics unless the expression is Boolean.
    #[must_use]
    pub fn not(&self) -> Self {
        assert!(self.boolean, "not() requires a Boolean");
        let mut out = Self::constant(1).sub(self);
        out.boolean = true;
        out
    }
}

/// A product term `coeff * left * right` of a quadratic expression.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Product {
    /// Integer coefficient.
    pub coeff: i64,
    /// First factor.
    pub left: Lin,
    /// Second factor.
    pub right: Lin,
}

/// `lin + sum products`, at most quadratic in the row values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Quad {
    /// Linear part.
    pub lin: Lin,
    /// Product terms.
    pub products: Vec<Product>,
}

impl Quad {
    fn linear(lin: Lin) -> Self {
        Self {
            lin,
            products: Vec::new(),
        }
    }

    /// Evaluates with an integer lookup.
    pub fn eval_i(&self, value: impl Fn(Var) -> i128 + Copy) -> i128 {
        self.products.iter().fold(self.lin.eval_i(value), |acc, p| {
            acc + i128::from(p.coeff) * p.left.eval_i(value) * p.right.eval_i(value)
        })
    }
}

/// A polynomial identity over a row: `lin + sum products [* gate]` must vanish.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Constraint {
    /// The quadratic body.
    pub body: Quad,
    /// Optional linear factor multiplying the whole body (raises the degree by one).
    pub gate: Option<Lin>,
    /// Whether the identity holds on every row except the last (`true`) or on every row.
    pub transition: bool,
    /// Human-readable origin for diagnostics.
    pub label: String,
}

impl Constraint {
    /// Polynomial degree in the trace columns.
    pub fn degree(&self) -> u32 {
        let body = if self.body.products.is_empty() {
            u32::from(!self.body.lin.is_constant())
        } else {
            2
        };
        body + u32::from(self.gate.is_some())
    }
}

/// How the prover computes a column's value from earlier columns of the same row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hint {
    /// The state register with this index.
    State(u32),
    /// The integer value of a quadratic expression.
    Quad(Quad),
    /// `1` if `a < b`, else `0`.
    LessThan(Lin, Lin),
    /// Bits `[shift, shift + bits)` of a nonnegative quadratic expression.
    Limb {
        /// The decomposed value.
        value: Quad,
        /// Starting bit.
        shift: u32,
        /// Width.
        bits: u32,
    },
    /// `a / c` truncated toward zero.
    DivTrunc(Lin, i64),
    /// `floor(a / c)`.
    DivFloor(Lin, i64),
    /// `1` if `a == k`, else `0`.
    Equals(Lin, i64),
    /// `1` if `floor(a / divisor) == k`, else `0`.
    QuotientEquals {
        /// Dividend.
        value: Lin,
        /// Positive divisor.
        divisor: i64,
        /// Selected quotient.
        k: i64,
    },
    /// Field inverse of a quadratic expression, or zero.
    Inverse(Quad),
}

/// A committed main column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// How the prover computes it.
    pub hint: Hint,
    /// Diagnostics label.
    pub label: String,
}

/// A state register carried from one row to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Register {
    /// Its main column.
    pub column: u32,
    /// Value at the next row, as a linear form of this row.
    pub next: Lin,
    /// Inclusive lower bound.
    pub lo: i64,
    /// Inclusive upper bound.
    pub hi: i64,
    /// Diagnostics label.
    pub label: String,
}

/// A public column declared by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicColumn {
    /// Inclusive lower bound of every value.
    pub lo: i64,
    /// Inclusive upper bound of every value.
    pub hi: i64,
    /// Diagnostics label.
    pub label: String,
}

/// Engine-provided public column: `1` on rows `< steps`.
pub const PUBLIC_ACTIVE: u32 = 0;
/// Engine-provided public column: `1` on row 0.
pub const PUBLIC_FIRST: u32 = 1;
/// Engine-provided public column: `1` on row `steps`.
pub const PUBLIC_LAST: u32 = 2;
/// Engine-provided public column: the row index (the step counter).
pub const PUBLIC_ROW: u32 = 3;
/// Engine-provided public column: `1` on the row of the statement's checkpoint, if any.
pub const PUBLIC_CHECKPOINT: u32 = 4;
/// Number of engine-provided public columns.
pub const ENGINE_PUBLIC_COLUMNS: u32 = 5;

/// A compiled circuit: the AIR plus the witness program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circuit {
    /// Game-chosen name and semantics version, bound into the circuit identity.
    pub name: String,
    /// `log2` of the trace height.
    pub log_rows: u32,
    /// Range-check limb width: lookups must lie in `[0, 2^range_bits)`.
    pub range_bits: u32,
    /// Committed main columns.
    pub columns: Vec<Column>,
    /// Public columns (engine columns first).
    pub public: Vec<PublicColumn>,
    /// State registers.
    pub registers: Vec<Register>,
    /// Polynomial identities (state transitions included).
    pub constraints: Vec<Constraint>,
    /// Values that must lie in `[0, 2^range_bits)`.
    pub lookups: Vec<Lin>,
    /// Lookups combined per LogUp helper column.
    pub lookups_per_helper: usize,
    /// Interval facts the application asserted from its own constraints. The honest prover re-checks them
    /// on every row, but the AIR does not enforce them: they are trusted compiler assertions.
    pub assumptions: Vec<Assumption>,
}

/// A prover-checked interval fact `lo <= value <= hi`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assumption {
    /// The value.
    pub value: Lin,
    /// Inclusive lower bound.
    pub lo: i128,
    /// Inclusive upper bound.
    pub hi: i128,
    /// Diagnostics label.
    pub label: String,
}

impl Circuit {
    /// Trace height.
    pub const fn rows(&self) -> usize {
        1 << self.log_rows
    }

    /// Maximum steps a statement may claim (the last row holds the final state).
    pub const fn max_steps(&self) -> usize {
        self.rows() - 1
    }

    /// Number of LogUp helper columns.
    pub fn helper_count(&self) -> usize {
        self.lookups.len().div_ceil(self.lookups_per_helper)
    }

    /// Main columns referenced at the next row.
    pub fn next_columns(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .constraints
            .iter()
            .flat_map(|c| {
                c.body
                    .lin
                    .terms
                    .iter()
                    .chain(
                        c.body
                            .products
                            .iter()
                            .flat_map(|p| p.left.terms.iter().chain(&p.right.terms)),
                    )
                    .chain(c.gate.iter().flat_map(|g| g.terms.iter()))
                    .filter_map(|(v, _)| match v {
                        Var::Next(i) => Some(*i),
                        _ => None,
                    })
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Maximum constraint degree, including the LogUp helper identities.
    pub fn max_degree(&self) -> u32 {
        let helper_degree = u32::try_from(self.lookups_per_helper).expect("small") + 1;
        self.constraints
            .iter()
            .map(Constraint::degree)
            .max()
            .unwrap_or(1)
            .max(if self.lookups.is_empty() {
                2
            } else {
                helper_degree
            })
            .max(2)
    }

    /// Canonical bytes of everything the verifier relies on.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let push_u64 = |out: &mut Vec<u8>, v: u64| out.extend_from_slice(&v.to_le_bytes());
        let push_i64 = |out: &mut Vec<u8>, v: i64| out.extend_from_slice(&v.to_le_bytes());
        let push_str = |out: &mut Vec<u8>, s: &str| {
            out.extend_from_slice(&(s.len() as u64).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        };
        let push_var = |out: &mut Vec<u8>, v: Var| match v {
            Var::Main(i) => {
                out.push(0);
                out.extend_from_slice(&i.to_le_bytes());
            }
            Var::Next(i) => {
                out.push(1);
                out.extend_from_slice(&i.to_le_bytes());
            }
            Var::Public(i) => {
                out.push(2);
                out.extend_from_slice(&i.to_le_bytes());
            }
        };
        let push_lin = |out: &mut Vec<u8>, lin: &Lin| {
            push_i64(out, lin.constant);
            push_u64(out, lin.terms.len() as u64);
            for (v, c) in &lin.terms {
                push_var(out, *v);
                push_i64(out, *c);
            }
        };
        push_str(&mut out, "iroha_execution_stark circuit v1");
        push_str(&mut out, &self.name);
        push_u64(&mut out, u64::from(self.log_rows));
        push_u64(&mut out, u64::from(self.range_bits));
        push_u64(&mut out, self.columns.len() as u64);
        push_u64(&mut out, self.public.len() as u64);
        for p in &self.public {
            push_i64(&mut out, p.lo);
            push_i64(&mut out, p.hi);
        }
        push_u64(&mut out, self.registers.len() as u64);
        for r in &self.registers {
            push_u64(&mut out, u64::from(r.column));
            push_lin(&mut out, &r.next);
            push_i64(&mut out, r.lo);
            push_i64(&mut out, r.hi);
        }
        push_u64(&mut out, self.constraints.len() as u64);
        for c in &self.constraints {
            out.push(u8::from(c.transition));
            push_lin(&mut out, &c.body.lin);
            push_u64(&mut out, c.body.products.len() as u64);
            for p in &c.body.products {
                push_i64(&mut out, p.coeff);
                push_lin(&mut out, &p.left);
                push_lin(&mut out, &p.right);
            }
            match &c.gate {
                Some(g) => {
                    out.push(1);
                    push_lin(&mut out, g);
                }
                None => out.push(0),
            }
        }
        push_u64(&mut out, self.lookups.len() as u64);
        for l in &self.lookups {
            push_lin(&mut out, l);
        }
        push_u64(&mut out, self.lookups_per_helper as u64);
        out
    }
}

impl fmt::Display for Circuit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: 2^{} rows, {} main columns, {} public, {} registers, {} constraints, {} lookups ({} helpers)",
            self.name,
            self.log_rows,
            self.columns.len(),
            self.public.len(),
            self.registers.len(),
            self.constraints.len(),
            self.lookups.len(),
            self.helper_count()
        )
    }
}

/// One-hot selector over a contiguous integer range.
#[derive(Clone, Debug)]
pub struct OneHot {
    first: i64,
    bits: Vec<Expr>,
}

impl OneHot {
    /// `sum k * bit_k`, the selected index (free).
    pub fn index(&self) -> Expr {
        let mut out = Expr::constant(0);
        for (offset, bit) in self.bits.iter().enumerate() {
            let k = self.first + i64::try_from(offset).expect("small selector");
            if k != 0 {
                out = out.add(&bit.scale(k));
            }
        }
        out.lo = i128::from(self.first);
        out.hi =
            i128::from(self.first) + i128::try_from(self.bits.len()).expect("small selector") - 1;
        out.boolean = false;
        out
    }

    /// Selector bit for `value`.
    pub fn bit(&self, value: i64) -> &Expr {
        &self.bits[usize::try_from(value - self.first).expect("value in range")]
    }

    /// `table[index - first]` as a linear expression (free).
    ///
    /// # Panics
    /// Panics if the table length differs from the selector range.
    pub fn select_table(&self, table: &[i64]) -> Expr {
        assert_eq!(table.len(), self.bits.len(), "table size mismatch");
        let mut out = Expr::constant(0);
        for (bit, value) in self.bits.iter().zip(table) {
            if *value != 0 {
                out = out.add(&bit.scale(*value));
            }
        }
        let (lo, hi) = (
            table.iter().copied().min().unwrap_or(0),
            table.iter().copied().max().unwrap_or(0),
        );
        out.lo = i128::from(lo);
        out.hi = i128::from(hi);
        out
    }

    /// The sum of the bits whose table entry satisfies `predicate` (a Boolean, free).
    pub fn indicator(&self, predicate: impl Fn(i64) -> bool) -> Expr {
        let mut out = Expr::constant(0);
        for (offset, bit) in self.bits.iter().enumerate() {
            if predicate(self.first + i64::try_from(offset).expect("small selector")) {
                out = out.add(bit);
            }
        }
        out.lo = out.lo.max(0);
        out.hi = out.hi.min(1);
        out.boolean = true;
        out
    }
}

/// Records a step function as columns, constraints, lookups and hints.
#[derive(Debug)]
pub struct Builder {
    name: String,
    log_rows: u32,
    range_bits: u32,
    columns: Vec<Column>,
    public: Vec<PublicColumn>,
    registers: Vec<Register>,
    pending_next: Vec<Option<Lin>>,
    constraints: Vec<Constraint>,
    lookups: Vec<Lin>,
    lookups_per_helper: usize,
    assumptions: Vec<Assumption>,
}

fn bit_length(value: i128) -> u32 {
    debug_assert!(value >= 0);
    128 - value.leading_zeros()
}

impl Builder {
    /// Starts a circuit with `2^log_rows` trace rows.
    ///
    /// # Panics
    /// Panics unless `8 <= log_rows <= 22`.
    pub fn new(name: impl Into<String>, log_rows: u32) -> Self {
        assert!(
            (8..=22).contains(&log_rows),
            "trace height must be 2^8..2^22"
        );
        let public = ["active", "first", "last", "row", "checkpoint"]
            .iter()
            .map(|label| PublicColumn {
                lo: 0,
                hi: if *label == "row" {
                    (1i64 << log_rows) - 1
                } else {
                    1
                },
                label: (*label).to_owned(),
            })
            .collect();
        Self {
            name: name.into(),
            log_rows,
            range_bits: 16,
            columns: Vec::new(),
            public,
            registers: Vec::new(),
            pending_next: Vec::new(),
            constraints: Vec::new(),
            lookups: Vec::new(),
            lookups_per_helper: 2,
            assumptions: Vec::new(),
        }
    }

    /// Sets how many lookups share one LogUp helper column (constraint degree = this + 1).
    ///
    /// # Panics
    /// Panics unless `1 <= count <= 4`.
    pub fn set_lookups_per_helper(&mut self, count: usize) {
        assert!((1..=4).contains(&count));
        self.lookups_per_helper = count;
    }

    /// Bits per range-check limb.
    pub const fn limb_bits(&self) -> u32 {
        self.range_bits
    }

    /// Sets the range-table width (8..=16 bits) before any gadget is added. The verifier sums the
    /// `2^bits`-entry table side itself, so small circuits can use a smaller table.
    ///
    /// # Panics
    /// Panics if gadgets were already added or `bits` is outside 8..=16.
    pub fn set_range_bits(&mut self, bits: u32) {
        assert!((8..=16).contains(&bits));
        assert!(
            self.lookups.is_empty(),
            "set the range width before adding gadgets"
        );
        self.range_bits = bits;
    }

    /// The engine's `active` selector (1 while steps are being applied).
    pub fn active(&self) -> Expr {
        Expr::new(Lin::var(Var::Public(PUBLIC_ACTIVE)), 0, 1, true)
    }

    /// The row index, i.e. the step number of this row.
    pub fn step_index(&self) -> Expr {
        Expr::new(
            Lin::var(Var::Public(PUBLIC_ROW)),
            0,
            (1i128 << self.log_rows) - 1,
            false,
        )
    }

    /// Declares a public column with values in `[lo, hi]`.
    pub fn public(&mut self, label: impl Into<String>, lo: i64, hi: i64) -> Expr {
        let index = u32::try_from(self.public.len()).expect("small");
        self.public.push(PublicColumn {
            lo,
            hi,
            label: label.into(),
        });
        Expr::new(
            Lin::var(Var::Public(index)),
            i128::from(lo),
            i128::from(hi),
            lo == 0 && hi == 1,
        )
    }

    fn new_column(&mut self, hint: Hint, label: impl Into<String>) -> u32 {
        let index = u32::try_from(self.columns.len()).expect("small");
        self.columns.push(Column {
            hint,
            label: label.into(),
        });
        index
    }

    fn constrain(&mut self, body: Quad, gate: Option<Lin>, label: impl Into<String>) {
        self.constraints.push(Constraint {
            body,
            gate,
            transition: false,
            label: label.into(),
        });
    }

    /// Declares a state register with values in `[lo, hi]`; returns its current value.
    pub fn register(&mut self, label: impl Into<String>, lo: i64, hi: i64) -> Expr {
        let label = label.into();
        let index = u32::try_from(self.registers.len()).expect("small");
        let column = self.new_column(Hint::State(index), label.clone());
        self.registers.push(Register {
            column,
            next: Lin::default(),
            lo,
            hi,
            label,
        });
        self.pending_next.push(None);
        Expr::new(
            Lin::var(Var::Main(column)),
            i128::from(lo),
            i128::from(hi),
            lo == 0 && hi == 1,
        )
    }

    /// Sets the value a register takes after this step.
    ///
    /// # Panics
    /// Panics if `register` is not a register, was already set, or `next` can leave its interval.
    pub fn set_next(&mut self, register: &Expr, next: &Expr) {
        let column = match register.lin.terms.as_slice() {
            [(Var::Main(c), 1)] if register.lin.constant == 0 => *c,
            _ => panic!("set_next expects a register"),
        };
        let index = self
            .registers
            .iter()
            .position(|r| r.column == column)
            .expect("set_next expects a register");
        assert!(self.pending_next[index].is_none(), "register set twice");
        let reg = &self.registers[index];
        assert!(
            next.lo >= i128::from(reg.lo) && next.hi <= i128::from(reg.hi),
            "next value of {} may leave [{}, {}]: [{}, {}]",
            reg.label,
            reg.lo,
            reg.hi,
            next.lo,
            next.hi
        );
        self.pending_next[index] = Some(next.lin.clone());
    }

    /// Commits an expression to a fresh column (useful to cut expression size).
    pub fn materialize(&mut self, a: &Expr, label: &str) -> Expr {
        if a.lin.terms.len() == 1 && a.lin.constant == 0 && a.lin.terms[0].1 == 1 {
            if let Var::Main(_) = a.lin.terms[0].0 {
                return a.clone();
            }
        }
        let column = self.new_column(Hint::Quad(Quad::linear(a.lin.clone())), label);
        self.constrain(
            Quad::linear(Lin::var(Var::Main(column)).combine(1, &a.lin, -1)),
            None,
            label,
        );
        Expr::new(Lin::var(Var::Main(column)), a.lo, a.hi, a.boolean)
    }

    fn quad_column(&mut self, body: Quad, lo: i128, hi: i128, boolean: bool, label: &str) -> Expr {
        let column = self.new_column(Hint::Quad(body.clone()), label);
        let mut constraint = body;
        constraint.lin = constraint.lin.combine(1, &Lin::var(Var::Main(column)), -1);
        self.constrain(constraint, None, label);
        Expr::new(Lin::var(Var::Main(column)), lo, hi, boolean)
    }

    /// `a * b` (one column).
    pub fn mul(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        if let Some(k) = a.as_constant() {
            return b.scale(k);
        }
        if let Some(k) = b.as_constant() {
            return a.scale(k);
        }
        let candidates = [a.lo * b.lo, a.lo * b.hi, a.hi * b.lo, a.hi * b.hi];
        let lo = *candidates.iter().min().expect("4");
        let hi = *candidates.iter().max().expect("4");
        let body = Quad {
            lin: Lin::default(),
            products: vec![Product {
                coeff: 1,
                left: a.lin.clone(),
                right: b.lin.clone(),
            }],
        };
        self.quad_column(body, lo, hi, a.boolean && b.boolean, label)
    }

    /// `cond ? a : b` (one column unless free).
    ///
    /// # Panics
    /// Panics unless `cond` is Boolean.
    pub fn select(&mut self, cond: &Expr, a: &Expr, b: &Expr, label: &str) -> Expr {
        assert!(cond.boolean, "select condition must be Boolean");
        if let Some(c) = cond.as_constant() {
            return if c == 1 { a.clone() } else { b.clone() };
        }
        if a == b {
            return a.clone();
        }
        let lo = a.lo.min(b.lo);
        let hi = a.hi.max(b.hi);
        let diff = a.sub(b);
        if let Some(k) = diff.as_constant() {
            // b + k * cond is linear.
            let mut out = b.add(&cond.scale(k));
            out.lo = lo;
            out.hi = hi;
            out.boolean = a.boolean && b.boolean;
            return out;
        }
        let body = Quad {
            lin: b.lin.clone(),
            products: vec![Product {
                coeff: 1,
                left: cond.lin.clone(),
                right: diff.lin,
            }],
        };
        self.quad_column(body, lo, hi, a.boolean && b.boolean, label)
    }

    /// Boolean AND.
    pub fn and(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        assert!(a.boolean && b.boolean, "and() requires Booleans");
        self.mul(a, b, label)
    }

    /// Boolean OR.
    pub fn or(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        assert!(a.boolean && b.boolean, "or() requires Booleans");
        let both = self.and(a, b, label);
        let mut out = a.add(b).sub(&both);
        out.lo = 0;
        out.hi = 1;
        out.boolean = true;
        out
    }

    /// Asserts `0 <= value <= max` for a value already known to be at most `max` when nonnegative.
    ///
    /// The value is decomposed into `ceil(bits(max)/L)` full `L`-bit limbs, one lookup each. A limb
    /// sum is below `2^62`, far from `p`, so this is an exact nonnegativity check: a negative value
    /// would be a field element near `p` and cannot be written that way.
    fn nonneg_quad(&mut self, value: &Quad, max: i128, label: &str) {
        assert!(max >= 0, "{label}: negative bound");
        let limb_bits = self.limb_bits();
        let bits = bit_length(max).max(1);
        let limbs = bits.div_ceil(limb_bits);
        assert!(limbs * limb_bits <= 62, "{label}: range check too wide");
        if max == 0 {
            self.constrain(value.clone(), None, format!("{label}: zero"));
            return;
        }
        if value.products.is_empty() && limbs == 1 {
            self.lookups.push(value.lin.clone());
            return;
        }
        let mut reconstruction = value.clone();
        for limb in 0..limbs {
            let shift = limb * limb_bits;
            let column = self.new_column(
                Hint::Limb {
                    value: value.clone(),
                    shift,
                    bits: limb_bits,
                },
                format!("{label}: limb {limb}"),
            );
            self.lookups.push(Lin::var(Var::Main(column)));
            reconstruction.lin =
                reconstruction
                    .lin
                    .combine(1, &Lin::var(Var::Main(column)), -(1i64 << shift));
        }
        self.constrain(reconstruction, None, format!("{label}: reconstruction"));
    }

    fn negate_quad(value: &Quad, plus: i64) -> Quad {
        Quad {
            lin: Lin::constant(plus).combine(1, &value.lin, -1),
            products: value
                .products
                .iter()
                .map(|p| Product {
                    coeff: -p.coeff,
                    left: p.left.clone(),
                    right: p.right.clone(),
                })
                .collect(),
        }
    }

    /// Asserts `0 <= value < 2^bits`.
    pub fn range_check(&mut self, value: &Expr, bits: u32, label: &str) {
        let bound = (1i64 << bits) - 1;
        self.range_check_bounded(value, bound, label);
    }

    /// Asserts `0 <= value <= bound`.
    pub fn range_check_bounded(&mut self, value: &Expr, bound: i64, label: &str) {
        self.range_check_quad_bounded(
            &Quad::linear(value.lin.clone()),
            value.lo,
            value.hi,
            bound,
            label,
        );
    }

    /// Asserts `0 <= value <= bound` for a quadratic value with known interval `[lo, hi]`; sides that
    /// the interval already guarantees are skipped.
    fn range_check_quad_bounded(
        &mut self,
        value: &Quad,
        lo: i128,
        hi: i128,
        bound: i64,
        label: &str,
    ) {
        assert!(bound >= 0);
        let bound128 = i128::from(bound);
        if lo < 0 || hi > bound128 {
            if lo < 0 {
                self.nonneg_quad(value, hi.min(bound128).max(0), &format!("{label}: lower"));
            }
            if hi > bound128 {
                let upper = Self::negate_quad(value, bound);
                self.nonneg_quad(
                    &upper,
                    (bound128 - lo.max(0)).max(0),
                    &format!("{label}: upper"),
                );
            }
        } else if lo == hi && lo >= 0 && hi <= bound128 {
            // Constant within range: nothing to check.
        } else {
            // The interval already proves the bound.
        }
    }

    /// Enforces `lo <= value <= hi` and returns the value with that tighter interval.
    ///
    /// Use it where the step's semantics guarantee a bound that interval arithmetic cannot see
    /// (for example `energy - 25` on the branch where `energy >= 25`); the check is a real
    /// constraint, so a wrong assumption makes proving fail instead of weakening soundness.
    pub fn narrow(&mut self, value: &Expr, lo: i64, hi: i64, label: &str) -> Expr {
        let (lo128, hi128) = (i128::from(lo), i128::from(hi));
        if value.lo >= lo128 && value.hi <= hi128 {
            return value.clone();
        }
        let shifted = value.offset(-lo);
        self.range_check_quad_bounded(
            &Quad::linear(shifted.lin.clone()),
            shifted.lo,
            shifted.hi,
            hi - lo,
            label,
        );
        let mut out = value.clone();
        out.lo = lo128.max(value.lo);
        out.hi = hi128.min(value.hi);
        out
    }

    /// `a < b` as a Boolean.
    pub fn lt(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        if a.hi < b.lo {
            return Expr::constant(1);
        }
        if a.lo >= b.hi {
            return Expr::constant(0);
        }
        let column = self.new_column(Hint::LessThan(a.lin.clone(), b.lin.clone()), label);
        let bit = Var::Main(column);
        // bit * (bit - 1) = 0
        self.constrain(
            Quad {
                lin: Lin::default(),
                products: vec![Product {
                    coeff: 1,
                    left: Lin::var(bit),
                    right: Lin::var(bit).combine(1, &Lin::constant(-1), 1),
                }],
            },
            None,
            format!("{label}: boolean"),
        );
        // u = (a - b) + 2^K * bit lies in [0, 2^K) exactly when bit = [a < b], for any K (a multiple
        // of the limb width) with 2^K > |a - b|: a wrong bit makes u negative or at least 2^K.
        let limb_bits = self.limb_bits();
        let magnitude = (a.hi - b.lo).max(b.hi - a.lo);
        let k = bit_length(magnitude).max(1).div_ceil(limb_bits) * limb_bits;
        assert!(k <= 60, "{label}: comparison too wide");
        let u = a
            .lin
            .combine(1, &b.lin, -1)
            .combine(1, &Lin::var(bit), 1i64 << k);
        self.nonneg_quad(
            &Quad::linear(u),
            (1i128 << k) - 1,
            &format!("{label}: offset"),
        );
        Expr::new(Lin::var(bit), 0, 1, true)
    }

    /// `a <= b`.
    pub fn le(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        self.lt(a, &b.offset(1), label)
    }

    /// `a > b`.
    pub fn gt(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        self.lt(b, a, label)
    }

    /// `a >= b`.
    pub fn ge(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        self.lt(a, b, label).not()
    }

    /// `min(a, b)`.
    pub fn min(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        let a_lt_b = self.lt(a, b, &format!("{label}: cmp"));
        let mut out = self.select(&a_lt_b, a, b, label);
        out.lo = a.lo.min(b.lo);
        out.hi = a.hi.min(b.hi);
        out
    }

    /// `max(a, b)`.
    pub fn max(&mut self, a: &Expr, b: &Expr, label: &str) -> Expr {
        let a_lt_b = self.lt(a, b, &format!("{label}: cmp"));
        let mut out = self.select(&a_lt_b, b, a, label);
        out.lo = a.lo.max(b.lo);
        out.hi = a.hi.max(b.hi);
        out
    }

    /// `clamp(a, lo, hi)` for expressions `lo <= hi`.
    pub fn clamp(&mut self, a: &Expr, lo: &Expr, hi: &Expr, label: &str) -> Expr {
        let below = self.lt(a, lo, &format!("{label}: below"));
        let above = self.lt(hi, a, &format!("{label}: above"));
        // Exactly one of below, above, inside holds because lo <= hi.
        let inside = Expr::constant(1).sub(&below).sub(&above);
        let mut body = Quad::default();
        for (bit, value) in [(&below, lo), (&above, hi), (&inside, a)] {
            if let Some(k) = value.as_constant() {
                body.lin = body.lin.combine(1, &bit.lin, k);
            } else if let Some(k) = bit.as_constant() {
                body.lin = body.lin.combine(1, &value.lin, k);
            } else {
                body.products.push(Product {
                    coeff: 1,
                    left: bit.lin.clone(),
                    right: value.lin.clone(),
                });
            }
        }
        let out_lo = a.lo.max(lo.lo).min(hi.lo);
        let out_hi = a.hi.min(hi.hi).max(lo.hi);
        if body.products.is_empty() {
            return Expr::new(body.lin, out_lo, out_hi, false);
        }
        self.quad_column(body, out_lo, out_hi, false, label)
    }

    /// `|a|`.
    pub fn abs(&mut self, a: &Expr, label: &str) -> Expr {
        if a.lo >= 0 {
            return a.clone();
        }
        if a.hi <= 0 {
            return a.neg();
        }
        let negative = self.lt(a, &Expr::constant(0), &format!("{label}: sign"));
        let mut out = self.select(&negative, &a.neg(), a, label);
        out.lo = 0;
        out.hi = a.lo.abs().max(a.hi.abs());
        out
    }

    fn quotient_range(lo: i128, hi: i128, divisor: i64, trunc: bool) -> (i128, i128) {
        let c = i128::from(divisor);
        let f = |x: i128| {
            if trunc { x / c } else { x.div_euclid(c) }
        };
        (f(lo), f(hi))
    }

    /// Enforces `0 <= value <= bound` on both sides regardless of any assumed interval.
    fn bounded_forced(&mut self, value: &Quad, bound: i64, label: &str) {
        let bound128 = i128::from(bound);
        self.nonneg_quad(value, bound128, &format!("{label}: lower"));
        self.nonneg_quad(
            &Self::negate_quad(value, bound),
            bound128,
            &format!("{label}: upper"),
        );
    }

    fn checked_quotient(
        &mut self,
        hint: Hint,
        lo: i128,
        hi: i128,
        divisor: i64,
        label: &str,
    ) -> Expr {
        let column = self.new_column(hint, label);
        // q - lo >= 0 with a small limb budget makes q a small integer; the remainder identity then
        // forces the exact quotient, which also implies q <= hi.
        let shifted = Lin::var(Var::Main(column)).combine(
            1,
            &Lin::constant(i64::try_from(-lo).expect("bounded")),
            1,
        );
        self.nonneg_quad(
            &Quad::linear(shifted),
            hi - lo,
            &format!("{label}: quotient"),
        );
        let limb_budget =
            1i128 << (bit_length(hi - lo).max(1).div_ceil(self.limb_bits()) * self.limb_bits());
        assert!(
            (lo.abs() + limb_budget) * i128::from(divisor) < MAX_MAGNITUDE,
            "{label}: quotient check too wide for exact division"
        );
        Expr::new(Lin::var(Var::Main(column)), lo, hi, false)
    }

    /// `floor(a / divisor)` for a positive constant divisor.
    ///
    /// # Panics
    /// Panics unless `divisor > 0`.
    pub fn div_floor(&mut self, a: &Expr, divisor: i64, label: &str) -> Expr {
        assert!(divisor > 0);
        if divisor == 1 {
            return a.clone();
        }
        let (lo, hi) = Self::quotient_range(a.lo, a.hi, divisor, false);
        if lo == hi {
            return Expr::constant(i64::try_from(lo).expect("bounded"));
        }
        let q = self.checked_quotient(
            Hint::DivFloor(a.lin.clone(), divisor),
            lo,
            hi,
            divisor,
            label,
        );
        let remainder = a.sub(&q.scale(divisor));
        if divisor == 2 {
            // r * (r - 1) = 0 is cheaper than two lookups.
            self.constrain(
                Quad {
                    lin: Lin::default(),
                    products: vec![Product {
                        coeff: 1,
                        left: remainder.lin.clone(),
                        right: remainder.lin.combine(1, &Lin::constant(-1), 1),
                    }],
                },
                None,
                format!("{label}: remainder bit"),
            );
        } else {
            self.bounded_forced(
                &Quad::linear(remainder.lin),
                divisor - 1,
                &format!("{label}: remainder"),
            );
        }
        q
    }

    /// `a mod divisor` in `[0, divisor)` (Euclidean), with its quotient.
    pub fn div_rem_floor(&mut self, a: &Expr, divisor: i64, label: &str) -> (Expr, Expr) {
        let q = self.div_floor(a, divisor, label);
        let mut r = a.sub(&q.scale(divisor));
        r.lo = 0;
        r.hi = i128::from(divisor - 1);
        (q, r)
    }

    /// `a / divisor` truncated toward zero, for a positive constant divisor.
    ///
    /// # Panics
    /// Panics unless `divisor > 0`.
    pub fn div_trunc(&mut self, a: &Expr, divisor: i64, label: &str) -> Expr {
        assert!(divisor > 0);
        if divisor == 1 {
            return a.clone();
        }
        if a.lo >= 0 {
            return self.div_floor(a, divisor, label);
        }
        let (lo, hi) = Self::quotient_range(a.lo, a.hi, divisor, true);
        if lo == hi {
            return Expr::constant(i64::try_from(lo).expect("bounded"));
        }
        let q = self.checked_quotient(
            Hint::DivTrunc(a.lin.clone(), divisor),
            lo,
            hi,
            divisor,
            label,
        );
        let remainder = a.sub(&q.scale(divisor));
        if a.hi <= 0 {
            self.bounded_forced(
                &Quad::linear(remainder.neg().lin),
                divisor - 1,
                &format!("{label}: remainder"),
            );
            return q;
        }
        let negative = self.lt(a, &Expr::constant(0), &format!("{label}: sign"));
        // |remainder| = remainder * (1 - 2 * negative).
        let magnitude = Quad {
            lin: remainder.lin.clone(),
            products: vec![Product {
                coeff: -2,
                left: negative.lin.clone(),
                right: remainder.lin,
            }],
        };
        self.bounded_forced(&magnitude, divisor - 1, &format!("{label}: remainder"));
        q
    }

    /// `a / divisor` truncated toward zero when the caller already knows the dividend's sign:
    /// `negative` must equal `a < 0` whenever `a != 0` (for example the sign of a public factor of a
    /// product with a nonnegative value). Saves the sign comparison.
    ///
    /// # Panics
    /// Panics unless `divisor > 0` and `negative` is Boolean.
    pub fn div_trunc_with_sign(
        &mut self,
        a: &Expr,
        divisor: i64,
        negative: &Expr,
        label: &str,
    ) -> Expr {
        assert!(divisor > 0 && negative.boolean);
        if divisor == 1 {
            return a.clone();
        }
        if a.lo >= 0 || a.hi <= 0 {
            return self.div_trunc(a, divisor, label);
        }
        let (lo, hi) = Self::quotient_range(a.lo, a.hi, divisor, true);
        let q = self.checked_quotient(
            Hint::DivTrunc(a.lin.clone(), divisor),
            lo,
            hi,
            divisor,
            label,
        );
        let remainder = a.sub(&q.scale(divisor));
        let magnitude = Quad {
            lin: remainder.lin.clone(),
            products: vec![Product {
                coeff: -2,
                left: negative.lin.clone(),
                right: remainder.lin,
            }],
        };
        self.bounded_forced(&magnitude, divisor - 1, &format!("{label}: remainder"));
        q
    }

    /// A one-hot selector for `index` over `first..=last`, with the index constrained to the range.
    pub fn one_hot(&mut self, index: &Expr, first: i64, last: i64, label: &str) -> OneHot {
        assert!(first <= last);
        assert!(
            index.lo >= i128::from(first) && index.hi <= i128::from(last),
            "one-hot index interval exceeds the selector range"
        );
        let mut bits = Vec::new();
        let mut sum = Lin::default();
        let mut weighted = Lin::default();
        for value in first..=last {
            let column = self.new_column(
                Hint::Equals(index.lin.clone(), value),
                format!("{label}: ={value}"),
            );
            let bit = Var::Main(column);
            self.constrain(
                Quad {
                    lin: Lin::default(),
                    products: vec![Product {
                        coeff: 1,
                        left: Lin::var(bit),
                        right: Lin::var(bit).combine(1, &Lin::constant(-1), 1),
                    }],
                },
                None,
                format!("{label}: boolean {value}"),
            );
            sum = sum.combine(1, &Lin::var(bit), 1);
            weighted = weighted.combine(1, &Lin::var(bit), value);
            bits.push(Expr::new(Lin::var(bit), 0, 1, true));
        }
        self.constrain(
            Quad::linear(sum.combine(1, &Lin::constant(-1), 1)),
            None,
            format!("{label}: one"),
        );
        self.constrain(
            Quad::linear(weighted.combine(1, &index.lin, -1)),
            None,
            format!("{label}: index"),
        );
        OneHot { first, bits }
    }

    /// Asserts `value != 0` on rows where the Boolean `gate` is one.
    pub fn assert_nonzero_when(&mut self, gate: &Expr, value: &Expr, label: &str) {
        assert!(gate.boolean, "gate must be Boolean");
        let gated_value = Quad {
            lin: Lin::default(),
            products: vec![Product {
                coeff: 1,
                left: gate.lin.clone(),
                right: value.lin.clone(),
            }],
        };
        let column = self.new_column(Hint::Inverse(gated_value), format!("{label}: inverse"));
        // gate * (value * inv - 1) = 0
        self.constrain(
            Quad {
                lin: Lin::constant(-1),
                products: vec![Product {
                    coeff: 1,
                    left: value.lin.clone(),
                    right: Lin::var(Var::Main(column)),
                }],
            },
            Some(gate.lin.clone()),
            label,
        );
    }

    /// Asserts `value == 0` on rows where the Boolean `gate` is one (every row when `gate` is 1).
    pub fn assert_zero_when(&mut self, gate: &Expr, value: &Expr, label: &str) {
        assert!(gate.boolean, "gate must be Boolean");
        if gate.as_constant() == Some(1) {
            self.constrain(Quad::linear(value.lin.clone()), None, label);
        } else {
            self.constrain(
                Quad {
                    lin: Lin::default(),
                    products: vec![Product {
                        coeff: 1,
                        left: gate.lin.clone(),
                        right: value.lin.clone(),
                    }],
                },
                None,
                label,
            );
        }
    }

    /// Finishes the circuit, adding state transitions and the LogUp multiplicity column.
    ///
    /// # Panics
    /// Panics if a register has no next value.
    pub fn finish(mut self) -> Circuit {
        for (index, next) in self.pending_next.iter().enumerate() {
            let next = next.clone().unwrap_or_else(|| {
                panic!("register {} has no next value", self.registers[index].label)
            });
            self.registers[index].next = next;
        }
        for register in &self.registers {
            let current = Lin::var(Var::Main(register.column));
            let next = Lin::var(Var::Next(register.column));
            // next - cur - active * (step - cur) = 0
            self.constraints.push(Constraint {
                body: Quad {
                    lin: next.combine(1, &current, -1),
                    products: vec![Product {
                        coeff: -1,
                        left: Lin::var(Var::Public(PUBLIC_ACTIVE)),
                        right: register.next.combine(1, &current, -1),
                    }],
                },
                gate: None,
                transition: true,
                label: format!("{}: transition", register.label),
            });
        }
        Circuit {
            name: self.name,
            log_rows: self.log_rows,
            range_bits: self.range_bits,
            columns: self.columns,
            public: self.public,
            registers: self.registers,
            constraints: self.constraints,
            lookups: self.lookups,
            lookups_per_helper: self.lookups_per_helper,
            assumptions: self.assumptions,
        }
    }

    /// Trusted assertion that `value` lies in `[lo, hi]` because of constraints already present (for
    /// example a remainder check that pins a quotient); returns it with that interval.
    ///
    /// The AIR does not enforce the fact. Later gadgets size and may skip checks using it, so a
    /// false assertion can make the circuit accept wrong executions. Use [`Self::narrow`] for an
    /// enforced bound. The honest prover re-checks every assertion on every row, which catches
    /// mistakes on tested inputs, not adversarial ones.
    pub fn assume_interval(&mut self, value: &Expr, lo: i64, hi: i64, label: &str) -> Expr {
        let (lo, hi) = (i128::from(lo), i128::from(hi));
        if value.lo >= lo && value.hi <= hi {
            return value.clone();
        }
        self.assumptions.push(Assumption {
            value: value.lin.clone(),
            lo,
            hi,
            label: label.to_owned(),
        });
        let mut out = value.clone();
        out.lo = lo.max(value.lo);
        out.hi = hi.min(value.hi);
        out.boolean = out.lo >= 0 && out.hi <= 1 && value.boolean;
        out
    }

    /// Trusted assertion like [`Self::assume_interval`] with `[0, 1]`, marking the value Boolean
    /// (for example the sum of two mutually exclusive comparison bits).
    pub fn assume_boolean(&mut self, value: &Expr, label: &str) -> Expr {
        let mut out = self.assume_interval(value, 0, 1, label);
        if value.lo < 0 || value.hi > 1 {
            out.lo = 0;
            out.hi = 1;
        }
        out.boolean = true;
        out
    }

    /// Enforces `0 <= value <= bound` on both sides, whatever interval the value carries.
    pub fn range_check_bounded_forced(&mut self, value: &Expr, bound: i64, label: &str) {
        self.bounded_forced(&Quad::linear(value.lin.clone()), bound, label);
    }

    /// One-hot bits for `floor(value / divisor)` over `first..=last` that are Boolean and sum to
    /// one, plus the exact remainder check `0 <= value - divisor * index < divisor`.
    pub fn one_hot_quotient(
        &mut self,
        value: &Expr,
        divisor: i64,
        first: i64,
        last: i64,
        label: &str,
    ) -> OneHot {
        let hot = self.one_hot_quotient_unchecked(value, divisor, first, last, label);
        let remainder = value.sub(&hot.index().scale(divisor));
        self.range_check_bounded_forced(&remainder, divisor - 1, &format!("{label}: remainder"));
        hot
    }

    /// [`Self::one_hot_quotient`] without the remainder check. The caller must constrain the index
    /// some other way (for example through a nested quotient whose remainder check implies it).
    pub fn one_hot_quotient_unchecked(
        &mut self,
        value: &Expr,
        divisor: i64,
        first: i64,
        last: i64,
        label: &str,
    ) -> OneHot {
        assert!(divisor > 0 && first <= last);
        let mut bits = Vec::new();
        let mut sum = Lin::default();
        for k in first..=last {
            let column = self.new_column(
                Hint::QuotientEquals {
                    value: value.lin.clone(),
                    divisor,
                    k,
                },
                format!("{label}: ={k}"),
            );
            let bit = Var::Main(column);
            self.constrain(
                Quad {
                    lin: Lin::default(),
                    products: vec![Product {
                        coeff: 1,
                        left: Lin::var(bit),
                        right: Lin::var(bit).combine(1, &Lin::constant(-1), 1),
                    }],
                },
                None,
                format!("{label}: boolean {k}"),
            );
            sum = sum.combine(1, &Lin::var(bit), 1);
            bits.push(Expr::new(Lin::var(bit), 0, 1, true));
        }
        self.constrain(
            Quad::linear(sum.combine(1, &Lin::constant(-1), 1)),
            None,
            format!("{label}: one"),
        );
        OneHot { first, bits }
    }
}
