//! Reusable bounded-integer arithmetic AIR compiler.
//!
//! Every witness node has an explicit low-degree equation. Comparisons and divisions are
//! constrained with Boolean decompositions; evaluating host-language branches is never a verifier
//! constraint. All integer intervals are strictly smaller than the Goldilocks modulus, excluding
//! modular aliases. The compiled relation is fixed before Fiat–Shamir challenges are sampled.

use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;

#[derive(Clone, Copy, Debug)]
pub(super) enum Source {
    Constant(i64),
    Column(usize),
    Fixed(usize),
    Next(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Value {
    pub(super) source: Source,
    pub(super) low: i64,
    pub(super) high: i64,
}
impl Value {
    pub(super) const fn constant(value: i64) -> Self {
        Self {
            source: Source::Constant(value),
            low: value,
            high: value,
        }
    }
    pub(super) const fn fixed(index: usize, low: i64, high: i64) -> Self {
        Self {
            source: Source::Fixed(index),
            low,
            high,
        }
    }
}

#[derive(Clone, Debug)]
enum Operation {
    Input,
    Add(Value, Value),
    Sub(Value, Value),
    Mul(Value, Value),
    Select(Value, Value, Value),
    Bit(Value, u32),
    Digit(Value, u32),
    EqualConstant(Value, i64),
    Quotient(Value, i64),
    Remainder(Value, i64),
}

#[derive(Clone, Debug)]
enum Constraint {
    Equation(Value, Value),
    Add(Value, Value, Value),
    Sub(Value, Value, Value),
    Mul(Value, Value, Value),
    Select(Value, Value, Value, Value),
    Boolean(Value),
    RadixFour(Value),
    Decompose(Value, Vec<Value>),
    GatedEquation(Value, Value, Value),
}

/// A closed arithmetic graph and its exact polynomial constraints.
#[derive(Clone, Debug, Default)]
pub(super) struct IntegerAirV1 {
    operations: Vec<Operation>,
    constraints: Vec<Constraint>,
}

pub(super) fn field(value: i64) -> F {
    if value >= 0 {
        F(value as u64)
    } else {
        F::ZERO.sub(F(value.unsigned_abs()))
    }
}

impl IntegerAirV1 {
    fn push(&mut self, operation: Operation, low: i64, high: i64) -> Value {
        assert!(
            low <= high && low > -(1_i64 << 50) && high < (1_i64 << 50),
            "closed AIR integer bound"
        );
        let value = Value {
            source: Source::Column(self.operations.len()),
            low,
            high,
        };
        self.operations.push(operation);
        value
    }
    pub(super) fn input(&mut self, low: i64, high: i64) -> Value {
        self.push(Operation::Input, low, high)
    }
    pub(super) fn width(&self) -> usize {
        self.operations.len()
    }
    pub(super) fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
    pub(super) fn add(&mut self, a: Value, b: Value) -> Value {
        let out = self.push(Operation::Add(a, b), a.low + b.low, a.high + b.high);
        self.constraints.push(Constraint::Add(out, a, b));
        out
    }
    pub(super) fn sub(&mut self, a: Value, b: Value) -> Value {
        let out = self.push(Operation::Sub(a, b), a.low - b.high, a.high - b.low);
        self.constraints.push(Constraint::Sub(out, a, b));
        out
    }
    pub(super) fn mul(&mut self, a: Value, b: Value) -> Value {
        let products = [
            a.low * b.low,
            a.low * b.high,
            a.high * b.low,
            a.high * b.high,
        ];
        let out = self.push(
            Operation::Mul(a, b),
            *products.iter().min().expect("four"),
            *products.iter().max().expect("four"),
        );
        self.constraints.push(Constraint::Mul(out, a, b));
        out
    }
    pub(super) fn add_constant(&mut self, a: Value, b: i64) -> Value {
        self.add(a, Value::constant(b))
    }
    pub(super) fn sub_constant(&mut self, a: Value, b: i64) -> Value {
        self.sub(a, Value::constant(b))
    }
    pub(super) fn mul_constant(&mut self, a: Value, b: i64) -> Value {
        self.mul(a, Value::constant(b))
    }
    pub(super) fn not(&mut self, a: Value) -> Value {
        self.sub(Value::constant(1), a)
    }
    pub(super) fn and(&mut self, a: Value, b: Value) -> Value {
        self.mul(a, b)
    }
    pub(super) fn or(&mut self, a: Value, b: Value) -> Value {
        let product = self.mul(a, b);
        let sum = self.add(a, b);
        let mut result = self.sub(sum, product);
        result.low = 0;
        result.high = 1;
        result
    }
    pub(super) fn select(&mut self, condition: Value, yes: Value, no: Value) -> Value {
        assert!(condition.low >= 0 && condition.high <= 1);
        let out = self.push(
            Operation::Select(condition, yes, no),
            yes.low.min(no.low),
            yes.high.max(no.high),
        );
        self.constraints
            .push(Constraint::Select(out, condition, yes, no));
        out
    }
    fn bits(&mut self, value: Value, bits: u32) -> Vec<Value> {
        assert!(value.low >= 0 && bits < 50 && value.high < (1_i64 << bits));
        let mut outputs = vec![Value::constant(0); bits as usize];
        let mut bit = 0;
        while bit + 2 <= bits - 1 {
            let out = self.push(Operation::Digit(value, bit), 0, 3);
            self.constraints.push(Constraint::RadixFour(out));
            outputs[bit as usize] = out;
            bit += 2;
        }
        while bit < bits {
            let out = self.push(Operation::Bit(value, bit), 0, 1);
            self.constraints.push(Constraint::Boolean(out));
            outputs[bit as usize] = out;
            bit += 1;
        }
        self.constraints
            .push(Constraint::Decompose(value, outputs.clone()));
        outputs
    }
    /// A verifier-fixed small lookup, proved by a complete Boolean one-hot selection.
    pub(super) fn lookup(&mut self, value: Value, table: &[i64]) -> Value {
        assert!(!table.is_empty() && table.len() < 256);
        let mut total = Value::constant(0);
        let mut index = Value::constant(0);
        let mut result = Value::constant(0);
        for (i, entry) in table.iter().enumerate() {
            let selected = self.push(Operation::EqualConstant(value, i as i64), 0, 1);
            self.constraints.push(Constraint::Boolean(selected));
            total = self.add(total, selected);
            let weighted = self.mul_constant(selected, i as i64);
            index = self.add(index, weighted);
            if *entry != 0 {
                let contribution = self.mul_constant(selected, *entry);
                result = self.add(result, contribution);
            }
        }
        self.equate(total, Value::constant(1));
        self.equate(index, value);
        result.low = *table.iter().min().expect("nonempty");
        result.high = *table.iter().max().expect("nonempty");
        result
    }
    fn bit_width(maximum: i64) -> u32 {
        (64 - maximum.max(1).leading_zeros()).max(1)
    }
    /// Constrain a < b without interpreting field residues as signed host integers.
    pub(super) fn less(&mut self, a: Value, b: Value) -> Value {
        let maximum = (a.low - b.high)
            .unsigned_abs()
            .max((a.high - b.low).unsigned_abs());
        let bits = 64 - maximum.max(1).leading_zeros();
        let offset = 1_i64 << bits;
        let delta = self.sub(a, b);
        let shifted = self.add_constant(delta, offset);
        let decomposition = self.bits(shifted, bits + 1);
        self.not(decomposition[bits as usize])
    }
    pub(super) fn minimum(&mut self, a: Value, b: Value) -> Value {
        let less = self.less(a, b);
        let mut out = self.select(less, a, b);
        out.low = a.low.min(b.low);
        out.high = a.high.min(b.high);
        out
    }
    pub(super) fn maximum(&mut self, a: Value, b: Value) -> Value {
        let less = self.less(a, b);
        let mut out = self.select(less, b, a);
        out.low = a.low.max(b.low);
        out.high = a.high.max(b.high);
        out
    }
    pub(super) fn clamp(&mut self, value: Value, low: i64, high: i64) -> Value {
        let bottom = self.maximum(value, Value::constant(low));
        self.minimum(bottom, Value::constant(high))
    }
    pub(super) fn absolute(&mut self, value: Value) -> Value {
        let negative = self.less(value, Value::constant(0));
        let negated = self.mul_constant(value, -1);
        let mut out = self.select(negative, negated, value);
        out.low = 0;
        out.high = value.low.abs().max(value.high.abs());
        out
    }
    /// Exact unsigned Euclidean quotient and remainder, both range constrained.
    pub(super) fn divmod_unsigned(&mut self, value: Value, divisor: i64) -> (Value, Value) {
        assert!(value.low >= 0 && divisor > 0);
        let quotient = self.push(Operation::Quotient(value, divisor), 0, value.high / divisor);
        let remainder = self.push(Operation::Remainder(value, divisor), 0, divisor - 1);
        self.bits(quotient, Self::bit_width(quotient.high));
        self.bits(remainder, Self::bit_width(remainder.high));
        let remainder_in_range = self.less(remainder, Value::constant(divisor));
        self.equate(remainder_in_range, Value::constant(1));
        let product = self.mul_constant(quotient, divisor);
        let reconstructed = self.add(product, remainder);
        self.equate(value, reconstructed);
        (quotient, remainder)
    }
    /// Signed division with exactly Rust/JavaScript truncation toward zero.
    pub(super) fn divide(&mut self, value: Value, divisor: i64) -> Value {
        if value.low >= 0 {
            return self.divmod_unsigned(value, divisor).0;
        }
        let negative = self.less(value, Value::constant(0));
        let absolute = self.absolute(value);
        let quotient = self.divmod_unsigned(absolute, divisor).0;
        let negated = self.mul_constant(quotient, -1);
        self.select(negative, negated, quotient)
    }
    pub(super) fn equate(&mut self, a: Value, b: Value) {
        self.constraints.push(Constraint::Equation(a, b));
    }
    pub(super) fn gated_equate(&mut self, gate: Value, a: Value, b: Value) {
        self.constraints.push(Constraint::GatedEquation(gate, a, b));
    }
    pub(super) fn next(value: Value) -> Value {
        let Source::Column(index) = value.source else {
            panic!("next requires a trace column")
        };
        Value {
            source: Source::Next(index),
            ..value
        }
    }
    /// Generate one row from input state columns and public fixed values.
    pub(super) fn witness(&self, inputs: &[i64], fixed: &[i64]) -> Vec<i64> {
        let mut row = Vec::with_capacity(self.width());
        let mut input = inputs.iter();
        for operation in &self.operations {
            let read = |value: Value| match value.source {
                Source::Constant(v) => v,
                Source::Column(i) => row[i],
                Source::Fixed(i) => fixed[i],
                Source::Next(_) => panic!("next witness operand"),
            };
            let result = match *operation {
                Operation::Input => *input.next().expect("complete input row"),
                Operation::Add(a, b) => read(a) + read(b),
                Operation::Sub(a, b) => read(a) - read(b),
                Operation::Mul(a, b) => read(a) * read(b),
                Operation::Select(c, a, b) => {
                    if read(c) == 1 {
                        read(a)
                    } else {
                        read(b)
                    }
                }
                Operation::Bit(a, bit) => (read(a) >> bit) & 1,
                Operation::Digit(a, bit) => (read(a) >> bit) & 3,
                Operation::EqualConstant(a, constant) => i64::from(read(a) == constant),
                Operation::Quotient(a, d) => read(a) / d,
                Operation::Remainder(a, d) => read(a) % d,
            };
            row.push(result);
        }
        assert!(input.next().is_none(), "exact input row");
        row
    }
    /// Evaluate polynomial residues on arbitrary field points, including extension lifting samples.
    pub(super) fn residues(&self, current: &[F], next: &[F], fixed: &[F]) -> Vec<F> {
        let read = |v: Value| match v.source {
            Source::Constant(x) => field(x),
            Source::Column(i) => current[i],
            Source::Fixed(i) => fixed[i],
            Source::Next(i) => next[i],
        };
        self.constraints
            .iter()
            .map(|constraint| match constraint {
                Constraint::Equation(a, b) => read(*a).sub(read(*b)),
                Constraint::Add(out, a, b) => read(*out).sub(read(*a).add(read(*b))),
                Constraint::Sub(out, a, b) => read(*out).sub(read(*a).sub(read(*b))),
                Constraint::Mul(out, a, b) => read(*out).sub(read(*a).mul(read(*b))),
                Constraint::Select(out, c, a, b) => {
                    read(*out).sub(read(*b).add(read(*c).mul(read(*a).sub(read(*b)))))
                }
                Constraint::Boolean(a) => read(*a).mul(read(*a).sub(F::ONE)),
                Constraint::RadixFour(a) => {
                    let value = read(*a);
                    value
                        .mul(value.sub(F::ONE))
                        .mul(value.sub(field(2)))
                        .mul(value.sub(field(3)))
                }
                Constraint::Decompose(a, bits) => {
                    read(*a).sub(bits.iter().enumerate().fold(F::ZERO, |sum, (i, bit)| {
                        sum.add(read(*bit).mul(field(1_i64 << i)))
                    }))
                }
                Constraint::GatedEquation(gate, a, b) => read(*gate).mul(read(*a).sub(read(*b))),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_division_and_comparison_have_polynomial_constraints() {
        let mut air = IntegerAirV1::default();
        let a = air.input(-500, 500);
        let b = air.input(-500, 500);
        let less = air.less(a, b);
        let quotient = air.divide(a, 8);
        for av in [-499, -9, -1, 0, 1, 9, 499] {
            for bv in [-499, 0, 499] {
                let row = air.witness(&[av, bv], &[]);
                let fields = row.iter().copied().map(field).collect::<Vec<_>>();
                assert!(
                    air.residues(&fields, &fields, &[])
                        .iter()
                        .all(|r| *r == F::ZERO)
                );
                let Source::Column(li) = less.source else {
                    panic!("column")
                };
                let Source::Column(qi) = quotient.source else {
                    panic!("column")
                };
                assert_eq!(row[li], i64::from(av < bv));
                assert_eq!(row[qi], av / 8);
                let mut corrupt = fields.clone();
                corrupt[qi] = corrupt[qi].add(F::ONE);
                assert!(
                    air.residues(&corrupt, &corrupt, &[])
                        .iter()
                        .any(|r| *r != F::ZERO)
                );
            }
        }
    }
}
