//! Compiled expressions and their row evaluators.
//!
//! An `iroha_plonk` [`Expression`] is compiled once into a postfix [`Program`]. A
//! program is evaluated either at one witness ([`Program::eval`]) or at several
//! points of the line `acc + X (new - acc)` in one walk ([`Program::eval_line`]),
//! which is what the `ProtoGalaxy` prover needs for `G(2..d)`. Rotations wrap modulo
//! the domain size and an instance cell beyond its column's length reads zero, as
//! in the `iroha_plonk` checker.

use iroha_pasta::PastaField;
use iroha_plonk::cs::{
    AdviceQuery, Expression, ExpressionEvaluator, FixedQuery, InstanceQuery, Selector,
};

/// Most line points one walk evaluates; the supported folded degree is
/// `MAX_POINTS + 1` (points `X = 2..=d`).
pub const MAX_POINTS: usize = 8;

#[derive(Clone, Debug)]
pub(crate) enum Op<F> {
    Const(F),
    Selector(u32),
    Fixed(u32, i32),
    Advice(u32, i32),
    Instance(u32, i32),
    Neg,
    Add,
    Mul,
    Scale(F),
}

/// A postfix program for one expression.
#[derive(Clone, Debug)]
pub struct Program<F> {
    ops: Vec<Op<F>>,
}

struct Compiler<F> {
    ops: Vec<Op<F>>,
}

fn index(value: usize) -> u32 {
    u32::try_from(value).expect("column and selector indices fit u32")
}

impl<F: Copy> ExpressionEvaluator<F> for Compiler<F> {
    type Output = ();
    fn constant(&mut self, value: &F) {
        self.ops.push(Op::Const(*value));
    }
    fn selector(&mut self, selector: Selector) {
        self.ops.push(Op::Selector(index(selector.index())));
    }
    fn fixed(&mut self, query: FixedQuery) {
        self.ops.push(Op::Fixed(index(query.column_index), query.rotation.0));
    }
    fn advice(&mut self, query: AdviceQuery) {
        self.ops.push(Op::Advice(index(query.column_index), query.rotation.0));
    }
    fn instance(&mut self, query: InstanceQuery) {
        self.ops
            .push(Op::Instance(index(query.column_index), query.rotation.0));
    }
    fn negated(&mut self, (): ()) {
        self.ops.push(Op::Neg);
    }
    fn sum(&mut self, (): (), (): ()) {
        self.ops.push(Op::Add);
    }
    fn product(&mut self, (): (), (): ()) {
        self.ops.push(Op::Mul);
    }
    fn scaled(&mut self, (): (), factor: &F) {
        self.ops.push(Op::Scale(*factor));
    }
}

/// Degree in the folded variables: advice and instance queries count one;
/// fixed columns, selectors and constants are constants of the relation.
#[must_use]
pub fn folded_degree<F>(expression: &Expression<F>) -> usize {
    match expression {
        Expression::Constant(_) | Expression::Selector(_) | Expression::Fixed(_) => 0,
        Expression::Advice(_) | Expression::Instance(_) => 1,
        Expression::Negated(inner) | Expression::Scaled(inner, _) => folded_degree(inner),
        Expression::Sum(left, right) => folded_degree(left).max(folded_degree(right)),
        Expression::Product(left, right) => folded_degree(left) + folded_degree(right),
    }
}

fn factors<F: Clone>(expression: &Expression<F>, out: &mut Vec<Expression<F>>) {
    match expression {
        Expression::Product(left, right) => {
            factors(left, out);
            factors(right, out);
        }
        other => out.push(other.clone()),
    }
}

fn product<F: Clone>(parts: Vec<Expression<F>>, one: F) -> Expression<F> {
    parts
        .into_iter()
        .reduce(|acc, part| Expression::Product(Box::new(acc), Box::new(part)))
        .unwrap_or(Expression::Constant(one))
}

/// Splits `expression` into its constant guard (the product of top-level factors
/// that read only fixed columns, selectors and constants) and the remaining body,
/// so that `expression = guard * body` and rows where the guard vanishes need no
/// witness evaluation.
pub(crate) fn split_guard<F: PastaField>(expression: &Expression<F>) -> (Program<F>, Program<F>, bool) {
    let mut parts = Vec::new();
    factors(expression, &mut parts);
    let (guard, body): (Vec<_>, Vec<_>) = parts.into_iter().partition(|part| folded_degree(part) == 0);
    let trivial = guard.is_empty();
    (
        Program::compile(&product(guard, F::ONE)),
        Program::compile(&product(body, F::ONE)),
        trivial,
    )
}

/// Fixed columns and selector activations of a relation.
#[derive(Clone, Copy)]
pub(crate) struct Tables<'a, F> {
    pub(crate) fixed: &'a [Vec<F>],
    pub(crate) selectors: &'a [Vec<bool>],
    pub(crate) n: usize,
}

/// One witness point: advice columns and instance columns.
#[derive(Clone, Copy)]
pub(crate) struct Point<'a, F> {
    pub(crate) advice: &'a [Vec<F>],
    pub(crate) instance: &'a [Vec<F>],
}

#[inline]
fn at(row: usize, rotation: i32, n: usize) -> usize {
    let n = i64::try_from(n).expect("domain fits i64");
    let row = i64::try_from(row).expect("row fits i64");
    usize::try_from((row + i64::from(rotation)).rem_euclid(n)).expect("nonnegative")
}

#[inline]
fn instance_cell<F: PastaField>(instance: &[Vec<F>], column: u32, row: usize) -> F {
    instance
        .get(column as usize)
        .and_then(|values| values.get(row))
        .copied()
        .unwrap_or(F::ZERO)
}

impl<F: PastaField> Program<F> {
    /// Compiles `expression`.
    #[must_use]
    pub fn compile(expression: &Expression<F>) -> Self {
        let mut compiler = Compiler { ops: Vec::new() };
        expression.evaluate(&mut compiler);
        Self { ops: compiler.ops }
    }

    /// Whether the program is the constant one.
    pub(crate) fn is_one(&self) -> bool {
        matches!(self.ops.as_slice(), [Op::Const(value)] if *value == F::ONE)
    }

    /// Evaluates at one witness point and row.
    pub(crate) fn eval(
        &self,
        tables: &Tables<'_, F>,
        point: Point<'_, F>,
        row: usize,
        stack: &mut Vec<F>,
    ) -> F {
        stack.clear();
        for op in &self.ops {
            match op {
                Op::Const(value) => stack.push(*value),
                Op::Selector(s) => stack.push(if tables.selectors[*s as usize][row] {
                    F::ONE
                } else {
                    F::ZERO
                }),
                Op::Fixed(c, r) => stack.push(tables.fixed[*c as usize][at(row, *r, tables.n)]),
                Op::Advice(c, r) => stack.push(point.advice[*c as usize][at(row, *r, tables.n)]),
                Op::Instance(c, r) => {
                    stack.push(instance_cell(point.instance, *c, at(row, *r, tables.n)));
                }
                Op::Neg => {
                    let v = stack.pop().expect("operand");
                    stack.push(-v);
                }
                Op::Add => {
                    let b = stack.pop().expect("operand");
                    let a = stack.pop().expect("operand");
                    stack.push(a + b);
                }
                Op::Mul => {
                    let b = stack.pop().expect("operand");
                    let a = stack.pop().expect("operand");
                    stack.push(a * b);
                }
                Op::Scale(factor) => {
                    let v = stack.pop().expect("operand");
                    stack.push(v * factor);
                }
            }
        }
        stack.pop().expect("program result")
    }

    /// Evaluates at the points `acc + x (new - acc)` for every `x` in `xs`.
    pub(crate) fn eval_line(
        &self,
        tables: &Tables<'_, F>,
        acc: Point<'_, F>,
        new: Point<'_, F>,
        xs: &[F],
        row: usize,
        stack: &mut Vec<[F; MAX_POINTS]>,
    ) -> [F; MAX_POINTS] {
        let m = xs.len();
        let line = |a: F, b: F| {
            let mut out = [F::ZERO; MAX_POINTS];
            let d = b - a;
            for (o, x) in out.iter_mut().zip(xs) {
                *o = a + *x * d;
            }
            out
        };
        stack.clear();
        for op in &self.ops {
            match op {
                Op::Const(value) => stack.push([*value; MAX_POINTS]),
                Op::Selector(s) => {
                    let v = if tables.selectors[*s as usize][row] { F::ONE } else { F::ZERO };
                    stack.push([v; MAX_POINTS]);
                }
                Op::Fixed(c, r) => {
                    stack.push([tables.fixed[*c as usize][at(row, *r, tables.n)]; MAX_POINTS]);
                }
                Op::Advice(c, r) => {
                    let cell = at(row, *r, tables.n);
                    stack.push(line(acc.advice[*c as usize][cell], new.advice[*c as usize][cell]));
                }
                Op::Instance(c, r) => {
                    let cell = at(row, *r, tables.n);
                    stack.push(line(
                        instance_cell(acc.instance, *c, cell),
                        instance_cell(new.instance, *c, cell),
                    ));
                }
                Op::Neg => {
                    let mut v = stack.pop().expect("operand");
                    for x in v.iter_mut().take(m) {
                        *x = -*x;
                    }
                    stack.push(v);
                }
                Op::Add => {
                    let b = stack.pop().expect("operand");
                    let mut a = stack.pop().expect("operand");
                    for i in 0..m {
                        a[i] += b[i];
                    }
                    stack.push(a);
                }
                Op::Mul => {
                    let b = stack.pop().expect("operand");
                    let mut a = stack.pop().expect("operand");
                    for i in 0..m {
                        a[i] *= b[i];
                    }
                    stack.push(a);
                }
                Op::Scale(factor) => {
                    let mut v = stack.pop().expect("operand");
                    for x in v.iter_mut().take(m) {
                        *x *= factor;
                    }
                    stack.push(v);
                }
            }
        }
        stack.pop().expect("program result")
    }
}
