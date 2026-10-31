//! Allocation-free counting and canonical lowering of the immutable tableau.

use super::{UpperDigitsError, UpperDigitsOperations};
use crate::prime_power::{UpperDigitInstruction, UpperDigitsProgram};

const BLOCK: usize = 16;
const CACHE: usize = 64;
const REGISTERS: usize = 33;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Input,
    Multiply(usize, usize),
    Add(usize, usize),
    Subtract(usize, usize),
    MultiplyPlain(usize, i64),
    AddPlain(usize, i64),
    Negate(usize),
    // Only compile_program's exact matching tableau subtraction creates this.
    DivideProven(usize),
}

impl Operation {
    pub(super) fn arguments(self) -> ([usize; 2], usize) {
        match self {
            Self::Input => ([0, 0], 0),
            Self::Multiply(a, b) | Self::Add(a, b) | Self::Subtract(a, b) => ([a, b], 2),
            Self::MultiplyPlain(a, _)
            | Self::AddPlain(a, _)
            | Self::Negate(a)
            | Self::DivideProven(a) => ([a, 0], 1),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub(super) operation: Operation,
    pub(super) precision: u32,
    pub(super) depth: usize,
}

#[derive(Clone, Copy)]
enum Value {
    Constant(i64, u32),
    Cipher {
        id: usize,
        precision: u32,
        depth: usize,
    },
}
impl Value {
    fn precision(self) -> u32 {
        match self {
            Self::Constant(_, k) | Self::Cipher { precision: k, .. } => k,
        }
    }
    fn depth(self) -> usize {
        match self {
            Self::Constant(..) => 0,
            Self::Cipher { depth, .. } => depth,
        }
    }
    fn id(self) -> Result<usize, UpperDigitsError> {
        match self {
            Self::Cipher { id, .. } => Ok(id),
            _ => Err(UpperDigitsError::Program),
        }
    }
    fn is_zero(self) -> bool {
        matches!(self, Self::Constant(0, _))
    }
}

pub(super) trait Sink {
    fn emit(&mut self, node: Node) -> Result<usize, UpperDigitsError>;
    fn visit_coefficient(&mut self) -> Result<(), UpperDigitsError>;
}

pub(super) struct Count {
    pub(super) nodes: usize,
    pub(super) visits: u64,
    pub(super) operations: UpperDigitsOperations,
    maximum_visits: u64,
}
impl Count {
    pub(super) fn new(maximum_visits: u64) -> Self {
        Self {
            nodes: 0,
            visits: 0,
            operations: UpperDigitsOperations::default(),
            maximum_visits,
        }
    }
    fn step(&mut self) -> Result<(), UpperDigitsError> {
        self.visits = self
            .visits
            .checked_add(1)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        if self.visits > self.maximum_visits {
            return Err(UpperDigitsError::CompilerBudget);
        }
        Ok(())
    }
}
impl Sink for Count {
    fn emit(&mut self, node: Node) -> Result<usize, UpperDigitsError> {
        self.step()?;
        let id = self.nodes;
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        let counter = match node.operation {
            Operation::Input => return Ok(id),
            Operation::Multiply(..) => &mut self.operations.ciphertext_multiplications,
            Operation::MultiplyPlain(..) => &mut self.operations.plaintext_multiplications,
            Operation::AddPlain(..) => &mut self.operations.plaintext_additions,
            Operation::Add(..) | Operation::Subtract(..) => {
                &mut self.operations.ciphertext_additions
            }
            Operation::Negate(..) => &mut self.operations.negations,
            Operation::DivideProven(..) => &mut self.operations.exact_divisions,
        };
        *counter = counter
            .checked_add(1)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        Ok(id)
    }
    fn visit_coefficient(&mut self) -> Result<(), UpperDigitsError> {
        self.step()
    }
}

pub(super) struct Store {
    pub(super) nodes: Vec<Node>,
    pub(super) limit: usize,
}
impl Sink for Store {
    fn emit(&mut self, node: Node) -> Result<usize, UpperDigitsError> {
        if self.nodes.len() == self.limit {
            return Err(UpperDigitsError::Program);
        }
        let id = self.nodes.len();
        self.nodes.push(node);
        Ok(id)
    }
    fn visit_coefficient(&mut self) -> Result<(), UpperDigitsError> {
        Ok(())
    }
}

struct Powers {
    base: Value,
    entries: [Option<(usize, Value)>; CACHE],
}
struct Compiler<'a, S> {
    program: &'a UpperDigitsProgram,
    sink: S,
}
impl<S: Sink> Compiler<'_, S> {
    fn modulus(&self, precision: u32) -> Result<u64, UpperDigitsError> {
        self.program
            .plan()
            .context()
            .prime()
            .checked_pow(precision)
            .ok_or(UpperDigitsError::SizeOverflow)
    }
    fn constant(&self, value: i64, precision: u32) -> Result<Value, UpperDigitsError> {
        let fixed = crate::constant_time::FixedModulus::derive(self.modulus(precision)?)
            .ok_or(UpperDigitsError::Program)?;
        let word = fixed.canonicalize_i64(value);
        let centered = if word > fixed.modulus / 2 {
            word as i64 - fixed.modulus as i64
        } else {
            word as i64
        };
        Ok(Value::Constant(centered, precision))
    }
    fn emit(
        &mut self,
        operation: Operation,
        precision: u32,
        depth: usize,
    ) -> Result<Value, UpperDigitsError> {
        let id = self.sink.emit(Node {
            operation,
            precision,
            depth,
        })?;
        Ok(Value::Cipher {
            id,
            precision,
            depth,
        })
    }
    fn multiply(&mut self, a: Value, b: Value) -> Result<Value, UpperDigitsError> {
        let k = a.precision();
        if k != b.precision() {
            return Err(UpperDigitsError::Program);
        }
        match (a, b) {
            (Value::Constant(x, _), Value::Constant(y, _)) => {
                self.constant(x.checked_mul(y).ok_or(UpperDigitsError::SizeOverflow)?, k)
            }
            (Value::Constant(c, _), v) | (v, Value::Constant(c, _)) => match c {
                0 => self.constant(0, k),
                1 => Ok(v),
                -1 => self.emit(Operation::Negate(v.id()?), k, v.depth()),
                _ => self.emit(Operation::MultiplyPlain(v.id()?, c), k, v.depth()),
            },
            _ => self.emit(
                Operation::Multiply(a.id()?, b.id()?),
                k,
                a.depth()
                    .max(b.depth())
                    .checked_add(1)
                    .ok_or(UpperDigitsError::SizeOverflow)?,
            ),
        }
    }
    fn add(&mut self, a: Value, b: Value) -> Result<Value, UpperDigitsError> {
        let k = a.precision();
        if k != b.precision() {
            return Err(UpperDigitsError::Program);
        }
        match (a, b) {
            (Value::Constant(x, _), Value::Constant(y, _)) => {
                self.constant(x.checked_add(y).ok_or(UpperDigitsError::SizeOverflow)?, k)
            }
            (Value::Constant(c, _), v) | (v, Value::Constant(c, _)) => {
                if c == 0 {
                    Ok(v)
                } else {
                    self.emit(Operation::AddPlain(v.id()?, c), k, v.depth())
                }
            }
            _ => self.emit(
                Operation::Add(a.id()?, b.id()?),
                k,
                a.depth().max(b.depth()),
            ),
        }
    }
    fn power(&mut self, powers: &mut Powers, exponent: usize) -> Result<Value, UpperDigitsError> {
        if exponent == 0 {
            return self.constant(1, powers.base.precision());
        }
        if exponent == 1 {
            return Ok(powers.base);
        }
        if let Some((_, value)) = powers
            .entries
            .iter()
            .flatten()
            .find(|(n, _)| *n == exponent)
        {
            return Ok(*value);
        }
        let left = self.power(powers, exponent / 2)?;
        let right = self.power(powers, exponent - exponent / 2)?;
        let value = self.multiply(left, right)?;
        let entry = powers
            .entries
            .iter_mut()
            .find(|x| x.is_none())
            .ok_or(UpperDigitsError::SizeOverflow)?;
        *entry = Some((exponent, value));
        Ok(value)
    }
    fn block(
        &mut self,
        powers: &mut Powers,
        coefficients: &[u64],
        block: usize,
    ) -> Result<Value, UpperDigitsError> {
        let k = powers.base.precision();
        let modulus = self.modulus(k)?;
        let mut result = self.constant(0, k)?;
        let start = block
            .checked_mul(BLOCK)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        for offset in 0..BLOCK {
            let index = start
                .checked_add(offset)
                .and_then(|x| x.checked_mul(2))
                .and_then(|x| x.checked_add(1))
                .ok_or(UpperDigitsError::SizeOverflow)?;
            if index >= coefficients.len() {
                break;
            }
            self.sink.visit_coefficient()?;
            let coefficient = coefficients[index];
            if coefficient == 0 {
                continue;
            }
            if coefficient >= modulus {
                return Err(UpperDigitsError::Program);
            }
            let c = if coefficient > modulus / 2 {
                coefficient as i64 - modulus as i64
            } else {
                coefficient as i64
            };
            let power = self.power(powers, offset)?;
            let term = self.multiply(Value::Constant(c, k), power)?;
            result = self.add(result, term)?;
        }
        Ok(result)
    }
    fn blocks(
        &mut self,
        powers: &mut Powers,
        coefficients: &[u64],
        start: usize,
        count: usize,
    ) -> Result<Value, UpperDigitsError> {
        if count == 1 {
            return self.block(powers, coefficients, start);
        }
        let low = 1_usize
            .checked_shl(usize::BITS - (count - 1).leading_zeros() - 1)
            .ok_or(UpperDigitsError::SizeOverflow)?;
        let left = self.blocks(powers, coefficients, start, low)?;
        let right = self.blocks(powers, coefficients, start + low, count - low)?;
        if right.is_zero() {
            return Ok(left);
        }
        let power = self.power(
            powers,
            BLOCK
                .checked_mul(low)
                .ok_or(UpperDigitsError::SizeOverflow)?,
        )?;
        let term = self.multiply(power, right)?;
        self.add(left, term)
    }
    fn lift(&mut self, input: Value, polynomial: usize) -> Result<Value, UpperDigitsError> {
        let program = self.program;
        let polynomial = program
            .polynomials()
            .get(polynomial)
            .ok_or(UpperDigitsError::Program)?;
        let k = input.precision();
        if polynomial.context().precision() != k
            || polynomial.context().prime() != program.plan().context().prime()
        {
            return Err(UpperDigitsError::Program);
        }
        let coefficients = polynomial.coefficients();
        let prime = usize::try_from(program.plan().context().prime())
            .map_err(|_| UpperDigitsError::SizeOverflow)?;
        if coefficients.len() != prime.checked_add(1).ok_or(UpperDigitsError::SizeOverflow)?
            || coefficients.last() != Some(&1)
        {
            return Err(UpperDigitsError::Program);
        }
        for &coefficient in coefficients.iter().step_by(2) {
            self.sink.visit_coefficient()?;
            if coefficient != 0 {
                return Err(UpperDigitsError::Program);
            }
        }
        let square = self.multiply(input, input)?;
        let mut powers = Powers {
            base: square,
            entries: [None; CACHE],
        };
        let blocks = (prime / 2 + 1).div_ceil(BLOCK);
        let body = self.blocks(&mut powers, coefficients, 0, blocks)?;
        self.multiply(input, body)
    }
}

pub(super) fn compile<S: Sink>(
    program: &UpperDigitsProgram,
    sink: S,
) -> Result<(S, usize, usize), UpperDigitsError> {
    let mut compiler = Compiler { program, sink };
    let plan = program.plan();
    let e = plan.context().precision() as usize;
    if e == 0 || e + 2 > REGISTERS || plan.context().modulus() >= 1 << 31 {
        return Err(UpperDigitsError::Program);
    }
    let input = compiler.emit(Operation::Input, e as u32, 0)?;
    let mut registers = [input; REGISTERS];
    let mut instructions = program.instructions().iter();
    for offset in 0..e - plan.retained_digits() as usize {
        let precision = e as u32 - offset as u32;
        let mut source = offset;
        for destination in offset + 1..e {
            let expected = UpperDigitInstruction::Evaluate {
                polynomial: offset,
                input: source,
                output: e,
                precision,
            };
            if instructions.next() != Some(&expected) {
                return Err(UpperDigitsError::Program);
            }
            let lifted = compiler.lift(registers[source], offset)?;
            registers[e] = lifted;
            let expected = UpperDigitInstruction::Subtract {
                lhs: destination,
                rhs: e,
                output: e + 1,
                precision,
            };
            if instructions.next() != Some(&expected)
                || registers[destination].precision() != precision
            {
                return Err(UpperDigitsError::Program);
            }
            let difference = compiler.emit(
                Operation::Subtract(registers[destination].id()?, lifted.id()?),
                precision,
                registers[destination].depth().max(lifted.depth()),
            )?;
            let expected = UpperDigitInstruction::DivideByPrime {
                input: e + 1,
                output: destination,
                precision,
            };
            if instructions.next() != Some(&expected) {
                return Err(UpperDigitsError::Program);
            }
            registers[destination] = compiler.emit(
                Operation::DivideProven(difference.id()?),
                precision - 1,
                difference.depth(),
            )?;
            source = e;
        }
    }
    if instructions.next().is_some() {
        return Err(UpperDigitsError::Program);
    }
    let output = registers[e - 1];
    Ok((compiler.sink, output.id()?, output.depth()))
}
