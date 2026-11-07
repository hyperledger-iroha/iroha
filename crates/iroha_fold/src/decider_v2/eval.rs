//! Polynomials over the decider's tables.
//!
//! A [`Poly`] is a postfix program whose leaves are tables (multilinear
//! extensions, degree one in every variable), per-accumulator scalars and
//! constants. The prover evaluates it along one sumcheck pair at the round
//! points `0, 2, 3, ..., d` ([`Poly::eval_pair`]); the verifier evaluates it once
//! from the claimed table values ([`Poly::eval`]).

use iroha_pasta::PastaField;

/// Largest per-variable degree of a decider term.
pub(crate) const MAX_DEGREE: usize = 15;
/// Values per evaluated line: the points `0, 2, ..., d` of a round polynomial.
pub(crate) const LINE: usize = MAX_DEGREE;

/// One postfix operation.
#[derive(Clone, Debug)]
pub(crate) enum Op<F> {
    Const(F),
    /// A table by index.
    Table(u32),
    /// A per-accumulator scalar by index.
    Scalar(u32),
    Neg,
    Add,
    Mul,
    Scale(F),
}

/// A polynomial over tables, scalars and constants.
#[derive(Clone, Debug)]
pub(crate) struct Poly<F> {
    ops: Vec<Op<F>>,
}

fn id(value: usize) -> u32 {
    u32::try_from(value).expect("table and scalar indices fit u32")
}

impl<F: PastaField> Poly<F> {
    pub(crate) fn constant(value: F) -> Self {
        Self { ops: vec![Op::Const(value)] }
    }

    pub(crate) fn table(index: usize) -> Self {
        Self { ops: vec![Op::Table(id(index))] }
    }

    pub(crate) fn scalar(index: usize) -> Self {
        Self { ops: vec![Op::Scalar(id(index))] }
    }

    fn binary(mut self, other: Self, op: Op<F>) -> Self {
        self.ops.extend(other.ops);
        self.ops.push(op);
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn add(self, other: Self) -> Self {
        self.binary(other, Op::Add)
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn mul(self, other: Self) -> Self {
        self.binary(other, Op::Mul)
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn neg(mut self) -> Self {
        self.ops.push(Op::Neg);
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn sub(self, other: Self) -> Self {
        self.add(other.neg())
    }

    pub(crate) fn scale(mut self, factor: F) -> Self {
        self.ops.push(Op::Scale(factor));
        self
    }

    /// Per-variable degree: a table counts one, scalars and constants zero.
    pub(crate) fn degree(&self) -> usize {
        let mut stack: Vec<usize> = Vec::new();
        for op in &self.ops {
            match op {
                Op::Const(_) | Op::Scalar(_) => stack.push(0),
                Op::Table(_) => stack.push(1),
                Op::Neg | Op::Scale(_) => {}
                Op::Add => {
                    let b = stack.pop().expect("operand");
                    let a = stack.pop().expect("operand");
                    stack.push(a.max(b));
                }
                Op::Mul => {
                    let b = stack.pop().expect("operand");
                    let a = stack.pop().expect("operand");
                    stack.push(a + b);
                }
            }
        }
        stack.pop().unwrap_or(0)
    }

    /// Evaluates at one point from table values and scalars.
    pub(crate) fn eval(&self, table: &impl Fn(usize) -> F, scalars: &[F]) -> F {
        let mut stack: Vec<F> = Vec::with_capacity(16);
        for op in &self.ops {
            match op {
                Op::Const(value) => stack.push(*value),
                Op::Table(t) => stack.push(table(*t as usize)),
                Op::Scalar(s) => stack.push(scalars[*s as usize]),
                Op::Neg => {
                    let v = stack.pop().expect("operand");
                    stack.push(-v);
                }
                Op::Scale(factor) => {
                    let v = stack.pop().expect("operand");
                    stack.push(v * factor);
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
            }
        }
        stack.pop().unwrap_or(F::ZERO)
    }

    /// Evaluates along pair `pair` (rows `2 pair` and `2 pair + 1` of every
    /// table) at the `m` round points `0, 2, 3, ..., m`: a table reads
    /// `T[2p] + t (T[2p+1] - T[2p])`.
    pub(crate) fn eval_pair(
        &self,
        tables: &[Vec<F>],
        pair: usize,
        m: usize,
        scalars: &[F],
        stack: &mut Vec<[F; LINE]>,
    ) -> [F; LINE] {
        stack.clear();
        for op in &self.ops {
            match op {
                Op::Const(value) => stack.push([*value; LINE]),
                Op::Scalar(s) => stack.push([scalars[*s as usize]; LINE]),
                Op::Table(t) => {
                    let table = &tables[*t as usize];
                    let (low, high) = (table[2 * pair], table[2 * pair + 1]);
                    let step = high - low;
                    let mut line = [F::ZERO; LINE];
                    line[0] = low;
                    if m > 1 {
                        let mut value = high + step;
                        line[1] = value;
                        for slot in line.iter_mut().take(m).skip(2) {
                            value += step;
                            *slot = value;
                        }
                    }
                    stack.push(line);
                }
                Op::Neg => {
                    let v = stack.last_mut().expect("operand");
                    for x in v.iter_mut().take(m) {
                        *x = -*x;
                    }
                }
                Op::Scale(factor) => {
                    let v = stack.last_mut().expect("operand");
                    for x in v.iter_mut().take(m) {
                        *x *= factor;
                    }
                }
                Op::Add => {
                    let b = stack.pop().expect("operand");
                    let a = stack.last_mut().expect("operand");
                    for (x, y) in a.iter_mut().zip(b).take(m) {
                        *x += y;
                    }
                }
                Op::Mul => {
                    let b = stack.pop().expect("operand");
                    let a = stack.last_mut().expect("operand");
                    for (x, y) in a.iter_mut().zip(b).take(m) {
                        *x *= y;
                    }
                }
            }
        }
        stack.pop().unwrap_or([F::ZERO; LINE])
    }
}
