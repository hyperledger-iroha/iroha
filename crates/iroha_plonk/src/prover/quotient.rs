//! The quotient polynomial on exact cosets, and the compiled evaluator.
//!
//! # The compiled evaluator
//!
//! [`CompiledExpressions`] compiles the postfix gate and lookup expressions
//! of the descriptor (the bytes that are hashed, spec section 4) into one
//! expression DAG: every node is hash-consed, so a subexpression shared by
//! several gates or lookups, or repeated inside one, is evaluated once per
//! row. Nodes are stored in topological order and evaluated into a scratch
//! vector, one row at a time. A query reads its column at `(row + rotation)
//! mod n` of whatever domain the columns are given on: the base domain for
//! the lookup compression, a quotient coset for `h`.
//!
//! It is the third evaluator of the same constraints, independent of the
//! constraint checker (a naive interpreter of the uncompressed source
//! expressions) and of the verifier (an explicit-stack interpreter of the
//! descriptor expressions on evaluations); the tests compare them.
//!
//! # The quotient
//!
//! On each of the `d - 1` quotient cosets ([`crate::keys::QuotientDomain`])
//! the numerator is folded over the constraints in the spec section 2 order
//! with `y`:
//!
//! 1. every gate polynomial;
//! 2. the permutation: `l_0 (1 - z_0)`, `l_last (z_l^2 - z_l)`, the set links
//!    `l_0 (z_s - z_{s-1}(omega^{-(b+1)} X))` and, per set,
//!    `l_active (z_s(omega X) prod (v + beta sigma + gamma) - z_s prod (v +
//!    beta DELTA^j X + gamma))`;
//! 3. per lookup the five halo2 constraints.
//!
//! The numerator is multiplied by `(s_c^n - 1)^-1`, the constant value of
//! `1 / (X^n - 1)` on coset `c`, and the `d - 1` coset vectors are recombined
//! into the `(d - 1) n` coefficients of `h` with a small Vandermonde solve.
//! For a satisfying witness this is the vendored `h` exactly.
//!
//! Rows are independent, so the rows of a coset are split across the
//! caller's Rayon pool; every row's arithmetic is the same at any pool size.

use std::collections::BTreeMap;

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField};
use rayon::prelude::*;

use super::{Challenges, ProverError};
use crate::{
    cs::{
        CircuitDescriptorV1, DescriptorError, DescriptorRule,
        descriptor::{ColumnKindV1, ExprNodeV1, ExprV1, MAX_EXPRESSION_STACK, QueryV1},
    },
    keys::{CosetPolynomial, KeyError, ProvingKey},
    protocol::{Protocol, ProtocolError},
    transcript::decode_scalar,
};

/// Rows per parallel task; results do not depend on it.
const ROWS_PER_TASK: usize = 1 << 8;

/// One node of the compiled DAG; operands index earlier nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Node<F> {
    Constant(F),
    Fixed(u32),
    Advice(u32),
    Instance(u32),
    Negated(u32),
    Sum(u32, u32),
    Product(u32, u32),
    Scaled(u32, F),
}

/// The roots of one lookup's input and table expressions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CompiledLookup {
    inputs: Vec<u32>,
    tables: Vec<u32>,
}

/// The gate and lookup expressions of a descriptor compiled into one
/// hash-consed DAG (see the module documentation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledExpressions<F> {
    nodes: Vec<Node<F>>,
    gates: Vec<u32>,
    lookups: Vec<CompiledLookup>,
    fixed_queries: Vec<QueryV1>,
    advice_queries: Vec<QueryV1>,
    instance_queries: Vec<QueryV1>,
}

/// Hash-consing state of a compilation.
struct Builder<'a, F> {
    nodes: Vec<Node<F>>,
    index: BTreeMap<Node<F>, u32>,
    descriptor: &'a CircuitDescriptorV1,
}

/// The descriptor rule-6 error.
fn malformed() -> DescriptorError {
    DescriptorError::Invalid(DescriptorRule::Expression)
}

impl<F: PastaField> Builder<'_, F> {
    /// The index of `node`, adding it if new.
    fn intern(&mut self, node: Node<F>) -> Result<u32, DescriptorError> {
        if let Some(index) = self.index.get(&node) {
            return Ok(*index);
        }
        let index = u32::try_from(self.nodes.len()).map_err(|_| malformed())?;
        self.nodes.push(node);
        self.index.insert(node, index);
        Ok(index)
    }

    /// A query leaf after checking its index.
    fn query(&mut self, index: u32, table: usize, node: Node<F>) -> Result<u32, DescriptorError> {
        if usize::try_from(index).map_or(true, |index| index >= table) {
            return Err(malformed());
        }
        self.intern(node)
    }

    /// Compiles one postfix expression and returns its root.
    fn expression(&mut self, expression: &ExprV1) -> Result<u32, DescriptorError> {
        let mut stack: Vec<u32> = Vec::new();
        for node in expression {
            let index = match *node {
                ExprNodeV1::Constant(bytes) => {
                    let value = decode_scalar::<F>(&bytes).map_err(|_| malformed())?;
                    self.intern(Node::Constant(value))?
                }
                ExprNodeV1::Fixed(index) => {
                    let table = self.descriptor.fixed_queries.len();
                    self.query(index, table, Node::Fixed(index))?
                }
                ExprNodeV1::Advice(index) => {
                    let table = self.descriptor.advice_queries.len();
                    self.query(index, table, Node::Advice(index))?
                }
                ExprNodeV1::Instance(index) => {
                    let table = self.descriptor.instance_queries.len();
                    self.query(index, table, Node::Instance(index))?
                }
                ExprNodeV1::Negated => {
                    let operand = stack.pop().ok_or_else(malformed)?;
                    self.intern(Node::Negated(operand))?
                }
                ExprNodeV1::Scaled(bytes) => {
                    let factor = decode_scalar::<F>(&bytes).map_err(|_| malformed())?;
                    let operand = stack.pop().ok_or_else(malformed)?;
                    self.intern(Node::Scaled(operand, factor))?
                }
                ExprNodeV1::Sum | ExprNodeV1::Product => {
                    let right = stack.pop().ok_or_else(malformed)?;
                    let left = stack.pop().ok_or_else(malformed)?;
                    self.intern(if matches!(node, ExprNodeV1::Sum) {
                        Node::Sum(left, right)
                    } else {
                        Node::Product(left, right)
                    })?
                }
            };
            stack.push(index);
            if stack.len() > MAX_EXPRESSION_STACK {
                return Err(malformed());
            }
        }
        match stack.as_slice() {
            [root] => Ok(*root),
            _ => Err(malformed()),
        }
    }
}

/// Borrows every column as a slice.
fn slices<F>(columns: &[Vec<F>]) -> Vec<&[F]> {
    columns.iter().map(Vec::as_slice).collect()
}

/// `rotation mod n` as a row offset.
fn rotation_offset(rotation: i32, n: usize) -> Result<usize, ProtocolError> {
    let n = i64::try_from(n).map_err(|_| ProtocolError::Overflow)?;
    usize::try_from(i64::from(rotation).rem_euclid(n)).map_err(|_| ProtocolError::Overflow)
}

/// Columns bound to the query tables: per query, its column slice and its
/// rotation offset.
struct BoundColumns<'a, F> {
    fixed: Vec<(&'a [F], usize)>,
    advice: Vec<(&'a [F], usize)>,
    instance: Vec<(&'a [F], usize)>,
    mask: usize,
}

/// Binds one query table to its columns, checking every length.
fn bind_table<'a, F>(
    queries: &[QueryV1],
    columns: &[&'a [F]],
    n: usize,
) -> Result<Vec<(&'a [F], usize)>, ProverError> {
    queries
        .iter()
        .map(|query| {
            let column = usize::try_from(query.column)
                .ok()
                .and_then(|column| columns.get(column))
                .filter(|column| column.len() == n)
                .ok_or(ProverError::Key(KeyError::CosetIndex))?;
            Ok((*column, rotation_offset(query.rotation, n)?))
        })
        .collect()
}

impl<F: PastaField> CompiledExpressions<F> {
    /// Compiles the lookup expressions of `descriptor`, and its gate
    /// polynomials when `include_gates` is set.
    ///
    /// # Errors
    ///
    /// Rule 6 ([`DescriptorRule::Expression`]) for a malformed expression;
    /// validated descriptors always compile.
    pub fn compile(
        descriptor: &CircuitDescriptorV1,
        include_gates: bool,
    ) -> Result<Self, DescriptorError> {
        let mut builder = Builder {
            nodes: Vec::new(),
            index: BTreeMap::new(),
            descriptor,
        };
        let mut gates = Vec::new();
        if include_gates {
            for poly in descriptor.gates.iter().flatten() {
                gates.push(builder.expression(poly)?);
            }
        }
        let lookups = descriptor
            .lookups
            .iter()
            .map(|lookup| {
                Ok(CompiledLookup {
                    inputs: lookup
                        .inputs
                        .iter()
                        .map(|input| builder.expression(input))
                        .collect::<Result<_, DescriptorError>>()?,
                    tables: lookup
                        .tables
                        .iter()
                        .map(|table| builder.expression(table))
                        .collect::<Result<_, DescriptorError>>()?,
                })
            })
            .collect::<Result<Vec<_>, DescriptorError>>()?;
        Ok(Self {
            nodes: builder.nodes,
            gates,
            lookups,
            fixed_queries: descriptor.fixed_queries.clone(),
            advice_queries: descriptor.advice_queries.clone(),
            instance_queries: descriptor.instance_queries.clone(),
        })
    }

    /// The number of distinct DAG nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The number of compiled gate polynomials.
    #[must_use]
    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    /// Binds every query to its column (all `n` rows long).
    fn bind<'a>(
        &self,
        fixed: &[&'a [F]],
        advice: &[&'a [F]],
        instance: &[&'a [F]],
        n: usize,
    ) -> Result<BoundColumns<'a, F>, ProverError> {
        if !n.is_power_of_two() {
            return Err(ProverError::Protocol(ProtocolError::Overflow));
        }
        Ok(BoundColumns {
            fixed: bind_table(&self.fixed_queries, fixed, n)?,
            advice: bind_table(&self.advice_queries, advice, n)?,
            instance: bind_table(&self.instance_queries, instance, n)?,
            mask: n - 1,
        })
    }

    /// Evaluates every node at `row` into `scratch` (`node_count` long).
    fn evaluate_row(&self, columns: &BoundColumns<'_, F>, row: usize, scratch: &mut [F]) {
        let read = |(values, offset): (&[F], usize)| values[(row + offset) & columns.mask];
        for (index, node) in self.nodes.iter().enumerate() {
            scratch[index] = match *node {
                Node::Constant(value) => value,
                Node::Fixed(query) => read(columns.fixed[query as usize]),
                Node::Advice(query) => read(columns.advice[query as usize]),
                Node::Instance(query) => read(columns.instance[query as usize]),
                Node::Negated(operand) => -scratch[operand as usize],
                Node::Sum(left, right) => scratch[left as usize] + scratch[right as usize],
                Node::Product(left, right) => scratch[left as usize] * scratch[right as usize],
                Node::Scaled(operand, factor) => scratch[operand as usize] * factor,
            };
        }
    }

    /// `fold(acc * theta + value)` over `roots`.
    fn compress(roots: &[u32], scratch: &[F], theta: F) -> F {
        roots
            .iter()
            .fold(F::ZERO, |acc, root| acc * theta + scratch[*root as usize])
    }

    /// Evaluates `width` outputs per row on the base domain, row-major.
    fn rows_major(
        &self,
        columns: &BoundColumns<'_, F>,
        n: usize,
        width: usize,
        emit: impl Fn(&[F], &mut [F]) + Sync,
    ) -> Vec<F> {
        let mut out = vec![F::ZERO; n * width];
        if width == 0 {
            return out;
        }
        out.par_chunks_mut(width * ROWS_PER_TASK)
            .enumerate()
            .for_each(|(task, chunk)| {
                let mut scratch = vec![F::ZERO; self.nodes.len()];
                for (offset, row_out) in chunk.chunks_mut(width).enumerate() {
                    self.evaluate_row(columns, task * ROWS_PER_TASK + offset, &mut scratch);
                    emit(&scratch, row_out);
                }
            });
        out
    }

    /// The compressed input `A` and table `S` of every lookup on every row of
    /// the base domain (`theta`-folded, rotations wrapping modulo `n`).
    ///
    /// # Errors
    ///
    /// [`ProverError::Key`] when a column is missing or not `n` rows long.
    pub fn compress_lookups(
        &self,
        fixed: &[Vec<F>],
        advice: &[Vec<F>],
        instance: &[Vec<F>],
        theta: F,
        n: usize,
    ) -> Result<Vec<(Vec<F>, Vec<F>)>, ProverError> {
        let (fixed, advice, instance) = (slices(fixed), slices(advice), slices(instance));
        let columns = self.bind(&fixed, &advice, &instance, n)?;
        let width = self.lookups.len() * 2;
        let out = self.rows_major(&columns, n, width, |scratch, row_out| {
            for (lookup, pair) in self.lookups.iter().zip(row_out.chunks_mut(2)) {
                pair[0] = Self::compress(&lookup.inputs, scratch, theta);
                pair[1] = Self::compress(&lookup.tables, scratch, theta);
            }
        });
        Ok((0..self.lookups.len())
            .map(|lookup| {
                let column = |side: usize| -> Vec<F> {
                    (0..n).map(|row| out[row * width + 2 * lookup + side]).collect()
                };
                (column(0), column(1))
            })
            .collect())
    }

    /// The value of every compiled gate polynomial on every row of the base
    /// domain (rotations wrapping modulo `n`), one vector per polynomial in
    /// descriptor order. Differential tests compare it with the constraint
    /// checker and with direct evaluation of the finalized constraint system.
    ///
    /// # Errors
    ///
    /// [`ProverError::Key`] when a column is missing or not `n` rows long.
    pub fn gate_values(
        &self,
        fixed: &[Vec<F>],
        advice: &[Vec<F>],
        instance: &[Vec<F>],
        n: usize,
    ) -> Result<Vec<Vec<F>>, ProverError> {
        let (fixed, advice, instance) = (slices(fixed), slices(advice), slices(instance));
        let columns = self.bind(&fixed, &advice, &instance, n)?;
        let width = self.gates.len();
        let out = self.rows_major(&columns, n, width, |scratch, row_out| {
            for (root, value) in self.gates.iter().zip(row_out.iter_mut()) {
                *value = scratch[*root as usize];
            }
        });
        Ok((0..width)
            .map(|gate| (0..n).map(|row| out[row * width + gate]).collect())
            .collect())
    }
}

/// One lookup's committed polynomials in coefficient form.
pub(super) struct LookupPolys<'a, F> {
    /// The product `z`.
    pub(super) product: &'a [F],
    /// The permuted input `A'`.
    pub(super) input: &'a [F],
    /// The permuted table `S'`.
    pub(super) table: &'a [F],
}

/// The witness polynomials the quotient reads, in coefficient form.
pub(super) struct QuotientInputs<'a, F> {
    /// The advice columns.
    pub(super) advice: &'a [Vec<F>],
    /// The instance columns.
    pub(super) instance: &'a [Vec<F>],
    /// The permutation products `z_s`.
    pub(super) permutation_products: Vec<&'a [F]>,
    /// The lookups.
    pub(super) lookups: Vec<LookupPolys<'a, F>>,
}

/// The coset values of one lookup: `z`, `A'`, `S'`.
type LookupCoset<F> = [Vec<F>; 3];

/// Computes the `(d - 1) n` coefficients of `h` (see the module
/// documentation).
///
/// # Errors
///
/// [`ProverError::Key`] for a missing polynomial or coset,
/// [`ProverError::Fft`] on a transform failure.
pub(super) fn evaluate<C: PastaCurve>(
    pk: &ProvingKey<C>,
    protocol: &Protocol,
    compiled: &CompiledExpressions<C::ScalarExt>,
    inputs: &QuotientInputs<'_, C::ScalarExt>,
    challenges: Challenges<C::ScalarExt>,
) -> Result<Vec<C::ScalarExt>, ProverError> {
    let shape = protocol.shape();
    let n = shape.n;
    let domain = pk.domain();
    let quotient = pk.quotient_domain();
    let omega = domain.omega();
    let mask = n - 1;
    let last_offset = rotation_offset(shape.last_rotation()?, n)?;
    let Challenges {
        theta,
        beta,
        gamma,
        y,
    } = challenges;
    let one = C::ScalarExt::ONE;
    let delta = <C::ScalarExt as ff::PrimeField>::DELTA;
    let mut cosets = Vec::with_capacity(shape.quotient_pieces);
    for coset in 0..shape.quotient_pieces {
        let shift = quotient.shift(coset).ok_or(KeyError::CosetIndex)?;
        let to_coset = |coeffs: &[C::ScalarExt]| quotient.evaluate(domain, coeffs, coset);
        let fixed = (0..shape.num_fixed)
            .map(|i| pk.coset_values(CosetPolynomial::Fixed(i), coset))
            .collect::<Result<Vec<_>, _>>()?;
        let sigma = (0..shape.permutation_columns)
            .map(|j| pk.coset_values(CosetPolynomial::Permutation(j), coset))
            .collect::<Result<Vec<_>, _>>()?;
        let l0 = pk.coset_values(CosetPolynomial::L0, coset)?;
        let l_last = pk.coset_values(CosetPolynomial::LLast, coset)?;
        let l_active = pk.coset_values(CosetPolynomial::LActive, coset)?;
        let advice = inputs
            .advice
            .par_iter()
            .map(|poly| to_coset(poly))
            .collect::<Result<Vec<_>, _>>()?;
        let instance = inputs
            .instance
            .par_iter()
            .map(|poly| to_coset(poly))
            .collect::<Result<Vec<_>, _>>()?;
        let products = inputs
            .permutation_products
            .par_iter()
            .map(|poly| to_coset(poly))
            .collect::<Result<Vec<_>, _>>()?;
        let lookups = inputs
            .lookups
            .iter()
            .map(|lookup| -> Result<LookupCoset<C::ScalarExt>, KeyError> {
                Ok([
                    to_coset(lookup.product)?,
                    to_coset(lookup.input)?,
                    to_coset(lookup.table)?,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fixed_refs: Vec<&[C::ScalarExt]> = fixed.iter().map(AsRef::as_ref).collect();
        let advice_refs: Vec<&[C::ScalarExt]> = advice.iter().map(Vec::as_slice).collect();
        let instance_refs: Vec<&[C::ScalarExt]> = instance.iter().map(Vec::as_slice).collect();
        let sigma_refs: Vec<&[C::ScalarExt]> = sigma.iter().map(AsRef::as_ref).collect();
        let bound = compiled.bind(&fixed_refs, &advice_refs, &instance_refs, n)?;
        let permutation_columns = protocol
            .permutation_columns()
            .iter()
            .map(|column| {
                let table = match column.kind {
                    ColumnKindV1::Advice => &advice_refs,
                    ColumnKindV1::Fixed => &fixed_refs,
                    ColumnKindV1::Instance => &instance_refs,
                };
                table
                    .get(column.column)
                    .copied()
                    .filter(|values| values.len() == n)
                    .ok_or(KeyError::CosetIndex)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if sigma_refs.iter().any(|values| values.len() != n)
            || products.iter().any(|values| values.len() != n)
            || [&*l0, &*l_last, &*l_active]
                .iter()
                .any(|values| values.len() != n)
        {
            return Err(KeyError::CosetIndex.into());
        }
        let (l0, l_last, l_active) = (&*l0, &*l_last, &*l_active);
        let mut values = vec![C::ScalarExt::ZERO; n];
        values
            .par_chunks_mut(ROWS_PER_TASK)
            .enumerate()
            .for_each(|(task, out)| {
                let start = task * ROWS_PER_TASK;
                let mut scratch = vec![C::ScalarExt::ZERO; compiled.nodes.len()];
                let mut x_row = shift * omega.pow_vartime([start as u64]);
                for (offset, out) in out.iter_mut().enumerate() {
                    let row = start + offset;
                    let r_next = (row + 1) & mask;
                    let r_prev = (row + mask) & mask;
                    compiled.evaluate_row(&bound, row, &mut scratch);
                    let mut value = C::ScalarExt::ZERO;
                    for root in &compiled.gates {
                        value = value * y + scratch[*root as usize];
                    }
                    if let (Some(first), Some(last)) = (products.first(), products.last()) {
                        let r_last = (row + last_offset) & mask;
                        value = value * y + (one - first[row]) * l0[row];
                        value = value * y + (last[row].square() - last[row]) * l_last[row];
                        for pair in products.windows(2) {
                            value = value * y + (pair[1][row] - pair[0][r_last]) * l0[row];
                        }
                        let mut current_delta = beta * x_row;
                        for ((set, set_columns), set_sigma) in products
                            .iter()
                            .zip(permutation_columns.chunks(shape.chunk_len))
                            .zip(sigma_refs.chunks(shape.chunk_len))
                        {
                            let mut left = set[r_next];
                            for (column, sigma) in set_columns.iter().zip(set_sigma) {
                                left *= column[row] + beta * sigma[row] + gamma;
                            }
                            let mut right = set[row];
                            for column in set_columns {
                                right *= column[row] + current_delta + gamma;
                                current_delta *= delta;
                            }
                            value = value * y + (left - right) * l_active[row];
                        }
                    }
                    for (lookup, [product, input, table]) in compiled.lookups.iter().zip(&lookups) {
                        let compressed_input =
                            CompiledExpressions::compress(&lookup.inputs, &scratch, theta);
                        let compressed_table =
                            CompiledExpressions::compress(&lookup.tables, &scratch, theta);
                        let table_value = (compressed_input + beta) * (compressed_table + gamma);
                        let a_minus_s = input[row] - table[row];
                        value = value * y + (one - product[row]) * l0[row];
                        value =
                            value * y + (product[row].square() - product[row]) * l_last[row];
                        value = value * y
                            + (product[r_next] * (input[row] + beta) * (table[row] + gamma)
                                - product[row] * table_value)
                                * l_active[row];
                        value = value * y + a_minus_s * l0[row];
                        value = value * y + a_minus_s * (input[row] - input[r_prev]) * l_active[row];
                    }
                    *out = value;
                    x_row *= omega;
                }
            });
        let inverse = quotient
            .vanishing_inverse(coset)
            .ok_or(KeyError::CosetIndex)?;
        values.par_iter_mut().for_each(|value| *value *= inverse);
        cosets.push(values);
    }
    Ok(quotient.recombine(domain, cosets)?)
}

#[cfg(test)]
mod tests;
