#![allow(clippy::too_many_arguments)]

use crate::multicore;
use crate::plonk::{Any, ProvingKey, lookup, permutation};
use crate::poly::Basis;
use crate::{
    arithmetic::{CurveAffine, parallelize},
    poly::{Coeff, ExtendedLagrangeCoeff, LagrangeCoeff, Polynomial, Rotation},
};
#[cfg(feature = "profile")]
use ark_std::{end_timer, start_timer};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use multicore::{IntoParallelIterator, ParallelIterator};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use super::{ConstraintSystem, Expression};

/// Return the index in the polynomial of size `isize` after rotation `rot`.
fn get_rotation_idx(idx: usize, rot: i32, rot_scale: i32, isize: i32) -> usize {
    (((idx as i32) + (rot * rot_scale)).rem_euclid(isize)) as usize
}

fn store_extended_lagrange_part<F: Field>(
    extended: &mut Polynomial<F, ExtendedLagrangeCoeff>,
    part: &Polynomial<F, LagrangeCoeff>,
    part_index: usize,
    num_parts: usize,
) {
    assert!(part_index < num_parts);
    assert_eq!(extended.len(), part.len() * num_parts);
    for (row, value) in part.iter().enumerate() {
        extended[row * num_parts + part_index] = *value;
    }
}

/// Value used in a calculation
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd)]
pub enum ValueSource {
    /// This is a constant value
    Constant(usize),
    /// This is an intermediate value
    Intermediate(usize),
    /// This is a fixed column
    Fixed(usize, usize),
    /// This is an advice (witness) column
    Advice(usize, usize),
    /// This is an instance (external) column
    Instance(usize, usize),
    /// This is a challenge
    Challenge(usize),
    /// beta
    Beta(),
    /// gamma
    Gamma(),
    /// theta
    Theta(),
    /// y
    Y(),
    /// Previous value
    PreviousValue(),
}

impl Default for ValueSource {
    fn default() -> Self {
        ValueSource::Constant(0)
    }
}

impl ValueSource {
    /// Get the value for this source
    pub fn get<F: Field, B: Basis>(
        &self,
        rotations: &[usize],
        constants: &[F],
        intermediates: &[F],
        fixed_values: &[Polynomial<F, B>],
        advice_values: &[Polynomial<F, B>],
        instance_values: &[Polynomial<F, B>],
        challenges: &[F],
        beta: &F,
        gamma: &F,
        theta: &F,
        y: &F,
        previous_value: &F,
    ) -> F {
        match self {
            ValueSource::Constant(idx) => constants[*idx],
            ValueSource::Intermediate(idx) => intermediates[*idx],
            ValueSource::Fixed(column_index, rotation) => {
                fixed_values[*column_index][rotations[*rotation]]
            }
            ValueSource::Advice(column_index, rotation) => {
                advice_values[*column_index][rotations[*rotation]]
            }
            ValueSource::Instance(column_index, rotation) => {
                instance_values[*column_index][rotations[*rotation]]
            }
            ValueSource::Challenge(index) => challenges[*index],
            ValueSource::Beta() => *beta,
            ValueSource::Gamma() => *gamma,
            ValueSource::Theta() => *theta,
            ValueSource::Y() => *y,
            ValueSource::PreviousValue() => *previous_value,
        }
    }
}

/// Calculation
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Calculation {
    /// This is an addition
    Add(ValueSource, ValueSource),
    /// This is a subtraction
    Sub(ValueSource, ValueSource),
    /// This is a product
    Mul(ValueSource, ValueSource),
    /// This is a square
    Square(ValueSource),
    /// This is a double
    Double(ValueSource),
    /// This is a negation
    Negate(ValueSource),
    /// This is Horner's rule: `val = a; val = val * c + b[]`
    Horner(ValueSource, Vec<ValueSource>, ValueSource),
    /// This is a simple assignment
    Store(ValueSource),
}

impl Calculation {
    fn for_each_source(&self, mut visit: impl FnMut(ValueSource)) {
        match self {
            Calculation::Add(a, b) | Calculation::Sub(a, b) | Calculation::Mul(a, b) => {
                visit(*a);
                visit(*b);
            }
            Calculation::Square(value)
            | Calculation::Double(value)
            | Calculation::Negate(value)
            | Calculation::Store(value) => visit(*value),
            Calculation::Horner(start, parts, factor) => {
                visit(*start);
                for part in parts {
                    visit(*part);
                }
                visit(*factor);
            }
        }
    }

    fn remap_intermediates(&mut self, target_slots: &[usize]) {
        let remap = |source: &mut ValueSource| {
            if let ValueSource::Intermediate(target) = source {
                *target = target_slots[*target];
            }
        };
        match self {
            Calculation::Add(a, b) | Calculation::Sub(a, b) | Calculation::Mul(a, b) => {
                remap(a);
                remap(b);
            }
            Calculation::Square(value)
            | Calculation::Double(value)
            | Calculation::Negate(value)
            | Calculation::Store(value) => remap(value),
            Calculation::Horner(start, parts, factor) => {
                remap(start);
                for part in parts {
                    remap(part);
                }
                remap(factor);
            }
        }
    }

    /// Get the resulting value of this calculation
    pub fn evaluate<F: Field, B: Basis>(
        &self,
        rotations: &[usize],
        constants: &[F],
        intermediates: &[F],
        fixed_values: &[Polynomial<F, B>],
        advice_values: &[Polynomial<F, B>],
        instance_values: &[Polynomial<F, B>],
        challenges: &[F],
        beta: &F,
        gamma: &F,
        theta: &F,
        y: &F,
        previous_value: &F,
    ) -> F {
        let get_value = |value: &ValueSource| {
            value.get(
                rotations,
                constants,
                intermediates,
                fixed_values,
                advice_values,
                instance_values,
                challenges,
                beta,
                gamma,
                theta,
                y,
                previous_value,
            )
        };
        match self {
            Calculation::Add(a, b) => get_value(a) + get_value(b),
            Calculation::Sub(a, b) => get_value(a) - get_value(b),
            Calculation::Mul(a, b) => get_value(a) * get_value(b),
            Calculation::Square(v) => get_value(v).square(),
            Calculation::Double(v) => get_value(v).double(),
            Calculation::Negate(v) => -get_value(v),
            Calculation::Horner(start_value, parts, factor) => {
                let factor = get_value(factor);
                let mut value = get_value(start_value);
                for part in parts.iter() {
                    value = value * factor + get_value(part);
                }
                value
            }
            Calculation::Store(v) => get_value(v),
        }
    }
}

/// Evaluator
#[derive(Clone, Default, Debug)]
pub struct Evaluator<C: CurveAffine> {
    ///  Custom gates evalution
    pub custom_gates: GraphEvaluator<C>,
    ///  Lookups evalution
    pub lookups: Vec<GraphEvaluator<C>>,
}

/// GraphEvaluator
#[derive(Clone, Debug)]
pub struct GraphEvaluator<C: CurveAffine> {
    /// Constants
    pub constants: Vec<C::ScalarExt>,
    /// Rotations
    pub rotations: Vec<i32>,
    /// Calculations
    pub calculations: Vec<CalculationInfo>,
    /// Number of intermediates
    pub num_intermediates: usize,
    /// Construction-only index for exact calculation reuse. This is discarded
    /// once the graph has been built so a proving key does not retain a second
    /// copy of every calculation.
    calculation_indices: Option<CalculationIndex>,
}

/// Collision-safe construction index that does not retain a second copy of
/// each (potentially variable-sized) `Calculation` as a hash-table key.
#[derive(Clone, Debug, Default)]
struct CalculationIndex {
    heads: HashMap<u64, usize>,
    previous_same_hash: Vec<usize>,
}

impl CalculationIndex {
    fn hash(calculation: &Calculation) -> u64 {
        let mut hasher = DefaultHasher::new();
        calculation.hash(&mut hasher);
        hasher.finish()
    }

    fn find(
        &self,
        hash: u64,
        calculation: &Calculation,
        calculations: &[CalculationInfo],
    ) -> Option<usize> {
        let mut candidate = self.heads.get(&hash).copied();
        while let Some(index) = candidate {
            if calculations[index].calculation == *calculation {
                return Some(calculations[index].target);
            }
            candidate = match self.previous_same_hash[index] {
                usize::MAX => None,
                previous => Some(previous),
            };
        }
        None
    }

    fn insert(&mut self, hash: u64, index: usize) {
        assert_eq!(self.previous_same_hash.len(), index);
        let previous = self.heads.insert(hash, index).unwrap_or(usize::MAX);
        self.previous_same_hash.push(previous);
    }
}

// Evaluator scratch is replicated once for every concurrent row chunk. Keep
// that replication bounded independently of the global Rayon worker count;
// large recursive verifier graphs can otherwise multiply one very large
// intermediate vector by every host core.
const CUSTOM_GATE_SCRATCH_BUDGET_BYTES: usize = 256 * 1024 * 1024;

/// EvaluationData
#[derive(Default, Debug)]
pub struct EvaluationData<C: CurveAffine> {
    /// Intermediates
    pub intermediates: Vec<C::ScalarExt>,
    /// Rotations
    pub rotations: Vec<usize>,
}

/// CaluclationInfo
#[derive(Clone, Debug)]
pub struct CalculationInfo {
    /// Calculation
    pub calculation: Calculation,
    /// Target
    pub target: usize,
}

impl<C: CurveAffine> Evaluator<C> {
    /// Creates a new evaluation structure
    pub fn new(cs: &ConstraintSystem<C::ScalarExt>) -> Self {
        let mut ev = Evaluator::default();

        // Custom gates
        let mut parts = Vec::new();
        for gate in cs.gates.iter() {
            parts.extend(
                gate.polynomials()
                    .iter()
                    .map(|poly| ev.custom_gates.add_expression(poly)),
            );
        }
        ev.custom_gates.add_calculation(Calculation::Horner(
            ValueSource::PreviousValue(),
            parts,
            ValueSource::Y(),
        ));
        ev.custom_gates.finish_building();

        // Lookups
        for lookup in cs.lookups.iter() {
            let mut graph = GraphEvaluator::default();

            let mut evaluate_lc = |expressions: &Vec<Expression<_>>| {
                let parts = expressions
                    .iter()
                    .map(|expr| graph.add_expression(expr))
                    .collect();
                graph.add_calculation(Calculation::Horner(
                    ValueSource::Constant(0),
                    parts,
                    ValueSource::Theta(),
                ))
            };

            // Input coset
            let compressed_input_coset = evaluate_lc(&lookup.input_expressions);
            // table coset
            let compressed_table_coset = evaluate_lc(&lookup.table_expressions);
            // z(\omega X) (a'(X) + \beta) (s'(X) + \gamma)
            let right_gamma = graph.add_calculation(Calculation::Add(
                compressed_table_coset,
                ValueSource::Gamma(),
            ));
            let lc = graph.add_calculation(Calculation::Add(
                compressed_input_coset,
                ValueSource::Beta(),
            ));
            graph.add_calculation(Calculation::Mul(lc, right_gamma));

            graph.finish_building();
            ev.lookups.push(graph);
        }

        ev
    }

    /// Evaluate h poly
    pub(in crate::plonk) fn evaluate_h(
        &self,
        pk: &ProvingKey<C>,
        advice_polys: &[&[Polynomial<C::ScalarExt, Coeff>]],
        instance_polys: &[&[Polynomial<C::ScalarExt, Coeff>]],
        challenges: &[C::ScalarExt],
        y: C::ScalarExt,
        beta: C::ScalarExt,
        gamma: C::ScalarExt,
        theta: C::ScalarExt,
        lookups: &[Vec<lookup::prover::Committed<C>>],
        permutations: &[permutation::prover::Committed<C>],
        stream_permutation_cosets: bool,
    ) -> Polynomial<C::ScalarExt, ExtendedLagrangeCoeff> {
        let domain = &pk.vk.domain;
        let size = 1 << domain.k() as usize;
        let rot_scale = 1;
        let extended_omega = domain.get_extended_omega();
        let omega = domain.get_omega();
        let isize = size as i32;
        let one = C::ScalarExt::ONE;
        let p = &pk.vk.cs.permutation;
        let num_parts = domain.extended_len() >> domain.k();

        // Calculate the quotient polynomial for each part
        let mut current_extended_omega = one;
        let mut extended_values = domain.empty_extended();
        (0..num_parts).for_each(|part_index| {
            #[cfg(feature = "profile")]
            let fixed_timer = start_timer!(|| "Fixed coeff_to_extended_part");
            let fixed: Vec<Polynomial<C::ScalarExt, LagrangeCoeff>> = (&pk.fixed_polys)
                .into_par_iter()
                .map(|p| domain.coeff_to_extended_part(p.clone(), current_extended_omega))
                .collect();
            let fixed = &fixed[..];
            let l0 = domain.coeff_to_extended_part(pk.l0.clone(), current_extended_omega);
            let l_last = domain.coeff_to_extended_part(pk.l_last.clone(), current_extended_omega);
            let l_active_row =
                domain.coeff_to_extended_part(pk.l_active_row.clone(), current_extended_omega);
            #[cfg(feature = "profile")]
            end_timer!(fixed_timer);

            #[cfg(feature = "profile")]
            let advice_timer = start_timer!(|| "Advice coeff_to_extended_part");
            // Calculate the advice and instance cosets
            let advice: Vec<Vec<Polynomial<C::Scalar, LagrangeCoeff>>> = advice_polys
                .into_par_iter()
                .map(|advice_polys| {
                    advice_polys
                        .iter()
                        .map(|poly| {
                            domain.coeff_to_extended_part(poly.clone(), current_extended_omega)
                        })
                        .collect()
                })
                .collect();
            #[cfg(feature = "profile")]
            end_timer!(advice_timer);
            #[cfg(feature = "profile")]
            let instance_timer = start_timer!(|| "Instance coeff_to_extended_part");
            let instance: Vec<Vec<Polynomial<C::Scalar, LagrangeCoeff>>> = instance_polys
                .into_par_iter()
                .map(|instance_polys| {
                    instance_polys
                        .iter()
                        .map(|poly| {
                            domain.coeff_to_extended_part(poly.clone(), current_extended_omega)
                        })
                        .collect()
                })
                .collect();
            #[cfg(feature = "profile")]
            end_timer!(instance_timer);

            let mut values = domain.empty_lagrange();

            // Core expression evaluations
            let num_threads = multicore::current_num_threads();
            for (((advice, instance), lookups), permutation) in advice
                .iter()
                .zip(instance.iter())
                .zip(lookups.iter())
                .zip(permutations.iter())
            {
                #[cfg(feature = "profile")]
                let timer = start_timer!(|| "Custom gates");
                // Custom gates
                multicore::scope(|scope| {
                    let worker_count = self
                        .custom_gates
                        .memory_bounded_worker_count(size, num_threads);
                    let chunk_size = size.div_ceil(worker_count);
                    for (thread_idx, values) in values.chunks_mut(chunk_size).enumerate() {
                        let start = thread_idx * chunk_size;
                        scope.spawn(move |_| {
                            let mut eval_data = self.custom_gates.instance();
                            for (i, value) in values.iter_mut().enumerate() {
                                let idx = start + i;
                                *value = self.custom_gates.evaluate(
                                    &mut eval_data,
                                    fixed,
                                    advice,
                                    instance,
                                    challenges,
                                    &beta,
                                    &gamma,
                                    &theta,
                                    &y,
                                    value,
                                    idx,
                                    rot_scale,
                                    isize,
                                );
                            }
                        });
                    }
                });
                #[cfg(feature = "profile")]
                end_timer!(timer);

                #[cfg(feature = "profile")]
                let timer = start_timer!(|| "Permutations");
                // Permutations
                let sets = &permutation.sets;
                if !sets.is_empty() {
                    let blinding_factors = pk.vk.cs.blinding_factors();
                    let last_rotation = Rotation(-((blinding_factors + 1) as i32));
                    let chunk_len = pk.vk.cs.degree() - 2;
                    let delta_start = beta * &C::Scalar::ZETA;

                    let permutation_product_cosets: Vec<Polynomial<C::ScalarExt, LagrangeCoeff>> =
                        sets.into_par_iter()
                            .map(|set| {
                                domain.coeff_to_extended_part(
                                    set.permutation_product_poly.clone(),
                                    current_extended_omega,
                                )
                            })
                            .collect();
                    if stream_permutation_cosets {
                        let first_set_permutation_product_coset =
                            permutation_product_cosets.first().unwrap();
                        let last_set_permutation_product_coset =
                            permutation_product_cosets.last().unwrap();

                        // These boundary and set-link constraints only use product
                        // cosets. Complete them in their original Horner order before
                        // allocating any sigma cosets.
                        parallelize(&mut values, |values, start| {
                            for (i, value) in values.iter_mut().enumerate() {
                                let idx = start + i;
                                let r_last =
                                    get_rotation_idx(idx, last_rotation.0, rot_scale, isize);

                                // Enforce only for the first set.
                                // l_0(X) * (1 - z_0(X)) = 0
                                *value = *value * y
                                    + ((one - first_set_permutation_product_coset[idx]) * l0[idx]);
                                // Enforce only for the last set.
                                // l_last(X) * (z_l(X)^2 - z_l(X)) = 0
                                *value = *value * y
                                    + ((last_set_permutation_product_coset[idx]
                                        * last_set_permutation_product_coset[idx]
                                        - last_set_permutation_product_coset[idx])
                                        * l_last[idx]);
                                // Except for the first set, enforce.
                                // l_0(X) * (z_i(X) - z_{i-1}(\omega^(last) X)) = 0
                                for (set_idx, permutation_product_coset) in
                                    permutation_product_cosets.iter().enumerate()
                                {
                                    if set_idx != 0 {
                                        *value = *value * y
                                            + ((permutation_product_coset[idx]
                                                - permutation_product_cosets[set_idx - 1][r_last])
                                                * l0[idx]);
                                    }
                                }
                            }
                        });

                        // One degree-sized sigma chunk is the final input needed for
                        // each set relation. Transform and consume one chunk at a time
                        // rather than retaining one n-element coset for every equality
                        // column. The compact KAGEMUSHA shape therefore keeps two sigma
                        // cosets live here instead of eleven.
                        let mut set_delta_start = delta_start;
                        for ((columns, permutation_product_coset), permutation_polynomial_chunk) in
                            p.columns
                                .chunks(chunk_len)
                                .zip(permutation_product_cosets.iter())
                                .zip(pk.permutation.polys.chunks(chunk_len))
                        {
                            let permutation_coset_chunk: Vec<
                                Polynomial<C::ScalarExt, LagrangeCoeff>,
                            > = permutation_polynomial_chunk
                                .into_par_iter()
                                .map(|polynomial| {
                                    domain.coeff_to_extended_part(
                                        polynomial.clone(),
                                        current_extended_omega,
                                    )
                                })
                                .collect();
                            let current_set_delta_start = set_delta_start;

                            // Processing complete sets serially preserves every row's
                            // Horner sequence while bounding transient sigma storage.
                            parallelize(&mut values, |values, start| {
                                let mut beta_term =
                                    current_extended_omega * omega.pow_vartime([start as u64]);
                                for (i, value) in values.iter_mut().enumerate() {
                                    let idx = start + i;
                                    let r_next = get_rotation_idx(idx, 1, rot_scale, isize);
                                    let mut left = permutation_product_coset[r_next];
                                    for (values, permutation) in columns
                                        .iter()
                                        .map(|&column| match column.column_type() {
                                            Any::Advice(_) => &advice[column.index()],
                                            Any::Fixed => &fixed[column.index()],
                                            Any::Instance => &instance[column.index()],
                                        })
                                        .zip(permutation_coset_chunk.iter())
                                    {
                                        left *= values[idx] + beta * permutation[idx] + gamma;
                                    }

                                    let mut right = permutation_product_coset[idx];
                                    let mut current_delta = current_set_delta_start * beta_term;
                                    for values in
                                        columns.iter().map(|&column| match column.column_type() {
                                            Any::Advice(_) => &advice[column.index()],
                                            Any::Fixed => &fixed[column.index()],
                                            Any::Instance => &instance[column.index()],
                                        })
                                    {
                                        right *= values[idx] + current_delta + gamma;
                                        current_delta *= &C::Scalar::DELTA;
                                    }

                                    *value = *value * y + ((left - right) * l_active_row[idx]);
                                    beta_term *= &omega;
                                }
                            });

                            for _ in columns {
                                set_delta_start *= &C::Scalar::DELTA;
                            }
                        }
                    } else {
                        let permutation_cosets: Vec<Polynomial<C::ScalarExt, LagrangeCoeff>> =
                            (&pk.permutation.polys)
                                .into_par_iter()
                                .map(|p| {
                                    domain.coeff_to_extended_part(p.clone(), current_extended_omega)
                                })
                                .collect();

                        let first_set_permutation_product_coset =
                            permutation_product_cosets.first().unwrap();
                        let last_set_permutation_product_coset =
                            permutation_product_cosets.last().unwrap();

                        // Permutation constraints
                        parallelize(&mut values, |values, start| {
                            let mut beta_term =
                                current_extended_omega * omega.pow_vartime([start as u64]);
                            for (i, value) in values.iter_mut().enumerate() {
                                let idx = start + i;
                                let r_next = get_rotation_idx(idx, 1, rot_scale, isize);
                                let r_last =
                                    get_rotation_idx(idx, last_rotation.0, rot_scale, isize);

                                // Enforce only for the first set.
                                // l_0(X) * (1 - z_0(X)) = 0
                                *value = *value * y
                                    + ((one - first_set_permutation_product_coset[idx]) * l0[idx]);
                                // Enforce only for the last set.
                                // l_last(X) * (z_l(X)^2 - z_l(X)) = 0
                                *value = *value * y
                                    + ((last_set_permutation_product_coset[idx]
                                        * last_set_permutation_product_coset[idx]
                                        - last_set_permutation_product_coset[idx])
                                        * l_last[idx]);
                                // Except for the first set, enforce.
                                // l_0(X) * (z_i(X) - z_{i-1}(\omega^(last) X)) = 0
                                for (set_idx, permutation_product_coset) in
                                    permutation_product_cosets.iter().enumerate()
                                {
                                    if set_idx != 0 {
                                        *value = *value * y
                                            + ((permutation_product_coset[idx]
                                                - permutation_product_cosets[set_idx - 1][r_last])
                                                * l0[idx]);
                                    }
                                }
                                // And for all the sets we enforce:
                                // (1 - (l_last(X) + l_blind(X))) * (
                                //   z_i(\omega X) \prod_j (p(X) + \beta s_j(X) + \gamma)
                                // - z_i(X) \prod_j (p(X) + \delta^j \beta X + \gamma)
                                // )
                                let mut current_delta = delta_start * beta_term;
                                for (
                                    (columns, permutation_product_coset),
                                    permutation_coset_chunk,
                                ) in p
                                    .columns
                                    .chunks(chunk_len)
                                    .zip(permutation_product_cosets.iter())
                                    .zip(permutation_cosets.chunks(chunk_len))
                                {
                                    let mut left = permutation_product_coset[r_next];
                                    for (values, permutation) in columns
                                        .iter()
                                        .map(|&column| match column.column_type() {
                                            Any::Advice(_) => &advice[column.index()],
                                            Any::Fixed => &fixed[column.index()],
                                            Any::Instance => &instance[column.index()],
                                        })
                                        .zip(permutation_coset_chunk.iter())
                                    {
                                        left *= values[idx] + beta * permutation[idx] + gamma;
                                    }

                                    let mut right = permutation_product_coset[idx];
                                    for values in
                                        columns.iter().map(|&column| match column.column_type() {
                                            Any::Advice(_) => &advice[column.index()],
                                            Any::Fixed => &fixed[column.index()],
                                            Any::Instance => &instance[column.index()],
                                        })
                                    {
                                        right *= values[idx] + current_delta + gamma;
                                        current_delta *= &C::Scalar::DELTA;
                                    }

                                    *value = *value * y + ((left - right) * l_active_row[idx]);
                                }
                                beta_term *= &omega;
                            }
                        });
                    }
                }
                #[cfg(feature = "profile")]
                end_timer!(timer);

                #[cfg(feature = "profile")]
                let timer = start_timer!(|| "Lookups");
                // Lookups
                for (n, lookup) in lookups.iter().enumerate() {
                    // Polynomials required for this lookup.
                    // Calculated here so these only have to be kept in memory for the short time
                    // they are actually needed.
                    let product_coset = pk.vk.domain.coeff_to_extended_part(
                        lookup.product_poly.clone(),
                        current_extended_omega,
                    );
                    let permuted_input_coset = pk.vk.domain.coeff_to_extended_part(
                        lookup.permuted_input_poly.clone(),
                        current_extended_omega,
                    );
                    let permuted_table_coset = pk.vk.domain.coeff_to_extended_part(
                        lookup.permuted_table_poly.clone(),
                        current_extended_omega,
                    );

                    // Lookup constraints
                    parallelize(&mut values, |values, start| {
                        let lookup_evaluator = &self.lookups[n];
                        let mut eval_data = lookup_evaluator.instance();
                        for (i, value) in values.iter_mut().enumerate() {
                            let idx = start + i;

                            let table_value = lookup_evaluator.evaluate(
                                &mut eval_data,
                                fixed,
                                advice,
                                instance,
                                challenges,
                                &beta,
                                &gamma,
                                &theta,
                                &y,
                                &C::ScalarExt::ZERO,
                                idx,
                                rot_scale,
                                isize,
                            );

                            let r_next = get_rotation_idx(idx, 1, rot_scale, isize);
                            let r_prev = get_rotation_idx(idx, -1, rot_scale, isize);

                            let a_minus_s = permuted_input_coset[idx] - permuted_table_coset[idx];
                            // l_0(X) * (1 - z(X)) = 0
                            *value = *value * y + ((one - product_coset[idx]) * l0[idx]);
                            // l_last(X) * (z(X)^2 - z(X)) = 0
                            *value = *value * y
                                + ((product_coset[idx] * product_coset[idx] - product_coset[idx])
                                    * l_last[idx]);
                            // (1 - (l_last(X) + l_blind(X))) * (
                            //   z(\omega X) (a'(X) + \beta) (s'(X) + \gamma)
                            //   - z(X) (\theta^{m-1} a_0(X) + ... + a_{m-1}(X) + \beta)
                            //          (\theta^{m-1} s_0(X) + ... + s_{m-1}(X) + \gamma)
                            // ) = 0
                            *value = *value * y
                                + ((product_coset[r_next]
                                    * (permuted_input_coset[idx] + beta)
                                    * (permuted_table_coset[idx] + gamma)
                                    - product_coset[idx] * table_value)
                                    * l_active_row[idx]);
                            // Check that the first values in the permuted input expression and permuted
                            // fixed expression are the same.
                            // l_0(X) * (a'(X) - s'(X)) = 0
                            *value = *value * y + (a_minus_s * l0[idx]);
                            // Check that each value in the permuted lookup input expression is either
                            // equal to the value above it, or the value at the same index in the
                            // permuted table expression.
                            // (1 - (l_last + l_blind)) * (a′(X) − s′(X))⋅(a′(X) − a′(\omega^{-1} X)) = 0
                            *value = *value * y
                                + (a_minus_s
                                    * (permuted_input_coset[idx] - permuted_input_coset[r_prev])
                                    * l_active_row[idx]);
                        }
                    });
                }
                #[cfg(feature = "profile")]
                end_timer!(timer);
            }
            current_extended_omega *= extended_omega;
            store_extended_lagrange_part(&mut extended_values, &values, part_index, num_parts);
        });

        extended_values
    }
}

#[test]
fn streamed_extended_parts_match_transposed_reference() {
    use crate::poly::EvaluationDomain;
    use halo2curves::bn256::Fr;

    let domain = EvaluationDomain::<Fr>::new(4, 3);
    let num_parts = domain.extended_len() >> domain.k();
    let parts = (0..num_parts)
        .map(|part_index| {
            domain.lagrange_from_vec(
                (0..domain.get_n() as usize)
                    .map(|row| Fr::from((row * num_parts + part_index) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let expected = domain.extended_from_lagrange_vec(parts.clone());
    let mut streamed = domain.empty_extended();
    for (part_index, part) in parts.iter().enumerate() {
        store_extended_lagrange_part(&mut streamed, part, part_index, num_parts);
    }

    assert_eq!(streamed.values, expected.values);
}

impl<C: CurveAffine> Default for GraphEvaluator<C> {
    fn default() -> Self {
        Self {
            // Fixed positions to allow easy access
            constants: vec![
                C::ScalarExt::ZERO,
                C::ScalarExt::ONE,
                C::ScalarExt::from(2u64),
            ],
            rotations: Vec::new(),
            calculations: Vec::new(),
            num_intermediates: 0,
            calculation_indices: Some(CalculationIndex::default()),
        }
    }
}

impl<C: CurveAffine> GraphEvaluator<C> {
    /// Adds a rotation
    fn add_rotation(&mut self, rotation: &Rotation) -> usize {
        let position = self.rotations.iter().position(|&c| c == rotation.0);
        match position {
            Some(pos) => pos,
            None => {
                self.rotations.push(rotation.0);
                self.rotations.len() - 1
            }
        }
    }

    /// Adds a constant
    fn add_constant(&mut self, constant: &C::ScalarExt) -> ValueSource {
        let position = self.constants.iter().position(|&c| c == *constant);
        ValueSource::Constant(match position {
            Some(pos) => pos,
            None => {
                self.constants.push(*constant);
                self.constants.len() - 1
            }
        })
    }

    /// Adds a calculation, reusing the first identical calculation already in
    /// the graph. The hash index changes only lookup complexity; the ordered
    /// `calculations` vector and assigned targets remain first-occurrence order.
    fn add_calculation(&mut self, calculation: Calculation) -> ValueSource {
        let hash = CalculationIndex::hash(&calculation);
        let target = self
            .calculation_indices
            .as_ref()
            .expect("cannot extend a finalized evaluation graph")
            .find(hash, &calculation, &self.calculations);
        if let Some(target) = target {
            return ValueSource::Intermediate(target);
        }

        let target = self.num_intermediates;
        self.calculations.push(CalculationInfo {
            calculation,
            target,
        });
        self.calculation_indices
            .as_mut()
            .expect("construction index remains present while extending a graph")
            .insert(hash, target);
        self.num_intermediates += 1;
        ValueSource::Intermediate(target)
    }

    /// Discard the construction index before retaining this graph in a proving
    /// key, then reuse scratch slots after their final read. Calculations are
    /// appended in dependency order, so remapping targets cannot change the
    /// arithmetic or Horner order.
    fn finish_building(&mut self) {
        self.calculation_indices = None;
        self.compact_intermediate_slots();
    }

    fn compact_intermediate_slots(&mut self) {
        let calculation_count = self.calculations.len();
        if calculation_count == 0 {
            self.num_intermediates = 0;
            return;
        }

        // Before compaction every newly inserted calculation receives the next
        // target, making these target IDs a topological numbering.
        let mut last_use = (0..calculation_count).collect::<Vec<_>>();
        for (position, calculation) in self.calculations.iter().enumerate() {
            assert_eq!(
                calculation.target, position,
                "evaluation graph targets must be topological before compaction"
            );
            calculation.calculation.for_each_source(|source| {
                if let ValueSource::Intermediate(target) = source {
                    assert!(
                        target < position,
                        "evaluation graph calculations may only read earlier targets"
                    );
                    last_use[target] = position;
                }
            });
        }
        // `evaluate` returns the final calculation's target after the loop.
        let output_target = self
            .calculations
            .last()
            .expect("non-empty graph has an output calculation")
            .target;
        last_use[output_target] = calculation_count;

        let mut target_slots = vec![usize::MAX; calculation_count];
        let mut free_slots = Vec::new();
        let mut released_targets = Vec::new();
        let mut slot_count = 0usize;

        for position in 0..calculation_count {
            let old_target = self.calculations[position].target;
            let has_successor = position + 1 < calculation_count;
            released_targets.clear();
            if has_successor {
                self.calculations[position]
                    .calculation
                    .for_each_source(|source| {
                        if let ValueSource::Intermediate(target) = source {
                            if last_use[target] == position {
                                released_targets.push(target);
                            }
                        }
                    });
            }
            released_targets.sort_unstable();
            released_targets.dedup();

            self.calculations[position]
                .calculation
                .remap_intermediates(&target_slots);

            // Never alias a calculation's destination with one of its current
            // inputs. Even a final-use source becomes available only to the next
            // calculation, keeping the evaluator's read-then-write behavior
            // explicit and independent of Rust assignment evaluation details.
            let slot = free_slots.pop().unwrap_or_else(|| {
                let slot = slot_count;
                slot_count += 1;
                slot
            });
            target_slots[old_target] = slot;
            self.calculations[position].target = slot;

            for &target in &released_targets {
                let slot = target_slots[target];
                assert_ne!(slot, usize::MAX, "source target must already have a slot");
                free_slots.push(slot);
            }

            // Simplification can leave a calculation with no consumer. Its slot
            // is available immediately after the calculation writes it.
            if has_successor && last_use[old_target] == position {
                free_slots.push(slot);
            }
        }

        self.num_intermediates = slot_count;
    }

    fn scratch_bytes(&self) -> usize {
        self.num_intermediates
            .saturating_mul(std::mem::size_of::<C::ScalarExt>())
            .saturating_add(
                self.rotations
                    .len()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
    }

    fn memory_bounded_worker_count(&self, rows: usize, available_workers: usize) -> usize {
        let available_workers = available_workers.max(1).min(rows.max(1));
        let scratch_bytes = self.scratch_bytes();
        if scratch_bytes == 0 {
            return available_workers;
        }

        available_workers.min((CUSTOM_GATE_SCRATCH_BUDGET_BYTES / scratch_bytes).max(1))
    }

    /// Generates an optimized evaluation for the expression
    fn add_expression(&mut self, expr: &Expression<C::ScalarExt>) -> ValueSource {
        match expr {
            Expression::Constant(scalar) => self.add_constant(scalar),
            Expression::Selector(_selector) => unreachable!(),
            Expression::Fixed(query) => {
                let rot_idx = self.add_rotation(&query.rotation);
                self.add_calculation(Calculation::Store(ValueSource::Fixed(
                    query.column_index,
                    rot_idx,
                )))
            }
            Expression::Advice(query) => {
                let rot_idx = self.add_rotation(&query.rotation);
                self.add_calculation(Calculation::Store(ValueSource::Advice(
                    query.column_index,
                    rot_idx,
                )))
            }
            Expression::Instance(query) => {
                let rot_idx = self.add_rotation(&query.rotation);
                self.add_calculation(Calculation::Store(ValueSource::Instance(
                    query.column_index,
                    rot_idx,
                )))
            }
            Expression::Challenge(challenge) => self.add_calculation(Calculation::Store(
                ValueSource::Challenge(challenge.index()),
            )),
            Expression::Negated(a) => match **a {
                Expression::Constant(scalar) => self.add_constant(&-scalar),
                _ => {
                    let result_a = self.add_expression(a);
                    match result_a {
                        ValueSource::Constant(0) => result_a,
                        _ => self.add_calculation(Calculation::Negate(result_a)),
                    }
                }
            },
            Expression::Sum(a, b) => {
                // Undo subtraction stored as a + (-b) in expressions
                match &**b {
                    Expression::Negated(b_int) => {
                        let result_a = self.add_expression(a);
                        let result_b = self.add_expression(b_int);
                        if result_a == ValueSource::Constant(0) {
                            self.add_calculation(Calculation::Negate(result_b))
                        } else if result_b == ValueSource::Constant(0) {
                            result_a
                        } else {
                            self.add_calculation(Calculation::Sub(result_a, result_b))
                        }
                    }
                    _ => {
                        let result_a = self.add_expression(a);
                        let result_b = self.add_expression(b);
                        if result_a == ValueSource::Constant(0) {
                            result_b
                        } else if result_b == ValueSource::Constant(0) {
                            result_a
                        } else if result_a <= result_b {
                            self.add_calculation(Calculation::Add(result_a, result_b))
                        } else {
                            self.add_calculation(Calculation::Add(result_b, result_a))
                        }
                    }
                }
            }
            Expression::Product(a, b) => {
                let result_a = self.add_expression(a);
                let result_b = self.add_expression(b);
                if result_a == ValueSource::Constant(0) || result_b == ValueSource::Constant(0) {
                    ValueSource::Constant(0)
                } else if result_a == ValueSource::Constant(1) {
                    result_b
                } else if result_b == ValueSource::Constant(1) {
                    result_a
                } else if result_a == ValueSource::Constant(2) {
                    self.add_calculation(Calculation::Double(result_b))
                } else if result_b == ValueSource::Constant(2) {
                    self.add_calculation(Calculation::Double(result_a))
                } else if result_a == result_b {
                    self.add_calculation(Calculation::Square(result_a))
                } else if result_a <= result_b {
                    self.add_calculation(Calculation::Mul(result_a, result_b))
                } else {
                    self.add_calculation(Calculation::Mul(result_b, result_a))
                }
            }
            Expression::Scaled(a, f) => {
                if *f == C::ScalarExt::ZERO {
                    ValueSource::Constant(0)
                } else if *f == C::ScalarExt::ONE {
                    self.add_expression(a)
                } else {
                    let cst = self.add_constant(f);
                    let result_a = self.add_expression(a);
                    self.add_calculation(Calculation::Mul(result_a, cst))
                }
            }
        }
    }

    /// Creates a new evaluation structure
    pub fn instance(&self) -> EvaluationData<C> {
        EvaluationData {
            intermediates: vec![C::ScalarExt::ZERO; self.num_intermediates],
            rotations: vec![0usize; self.rotations.len()],
        }
    }

    pub fn evaluate<B: Basis>(
        &self,
        data: &mut EvaluationData<C>,
        fixed: &[Polynomial<C::ScalarExt, B>],
        advice: &[Polynomial<C::ScalarExt, B>],
        instance: &[Polynomial<C::ScalarExt, B>],
        challenges: &[C::ScalarExt],
        beta: &C::ScalarExt,
        gamma: &C::ScalarExt,
        theta: &C::ScalarExt,
        y: &C::ScalarExt,
        previous_value: &C::ScalarExt,
        idx: usize,
        rot_scale: i32,
        isize: i32,
    ) -> C::ScalarExt {
        // All rotation index values
        for (rot_idx, rot) in self.rotations.iter().enumerate() {
            data.rotations[rot_idx] = get_rotation_idx(idx, *rot, rot_scale, isize);
        }

        // All calculations, with cached intermediate results
        for calc in self.calculations.iter() {
            data.intermediates[calc.target] = calc.calculation.evaluate(
                &data.rotations,
                &self.constants,
                &data.intermediates,
                fixed,
                advice,
                instance,
                challenges,
                beta,
                gamma,
                theta,
                y,
                previous_value,
            );
        }

        // Return the result of the last calculation (if any)
        if let Some(calc) = self.calculations.last() {
            data.intermediates[calc.target]
        } else {
            C::ScalarExt::ZERO
        }
    }
}

#[cfg(test)]
mod graph_evaluator_tests {
    use super::{
        CUSTOM_GATE_SCRATCH_BUDGET_BYTES, Calculation, CalculationIndex, CalculationInfo,
        GraphEvaluator, ValueSource,
    };
    use crate::arithmetic::CurveAffine;
    use crate::halo2curves::pasta::EqAffine;
    use crate::poly::{Coeff, Polynomial};
    use ff::Field;

    #[test]
    fn calculation_hash_index_preserves_first_occurrence_order() {
        let mut graph = GraphEvaluator::<EqAffine>::default();
        let load = Calculation::Store(ValueSource::Advice(7, 0));
        let add = Calculation::Add(ValueSource::Intermediate(0), ValueSource::Constant(1));

        assert_eq!(
            graph.add_calculation(load.clone()),
            ValueSource::Intermediate(0)
        );
        assert_eq!(
            graph.add_calculation(add.clone()),
            ValueSource::Intermediate(1)
        );
        assert_eq!(
            graph.add_calculation(load.clone()),
            ValueSource::Intermediate(0)
        );
        assert_eq!(
            graph.add_calculation(add.clone()),
            ValueSource::Intermediate(1)
        );

        assert_eq!(graph.num_intermediates, 2);
        assert_eq!(graph.calculations.len(), 2);
        assert_eq!(graph.calculations[0].target, 0);
        assert_eq!(graph.calculations[0].calculation, load);
        assert_eq!(graph.calculations[1].target, 1);
        assert_eq!(graph.calculations[1].calculation, add);

        graph.finish_building();
        assert!(graph.calculation_indices.is_none());
    }

    #[test]
    fn calculation_hash_index_resolves_hash_collisions_by_exact_equality() {
        let first = Calculation::Store(ValueSource::Advice(3, 0));
        let second = Calculation::Store(ValueSource::Advice(4, 0));
        let calculations = vec![
            CalculationInfo {
                calculation: first.clone(),
                target: 0,
            },
            CalculationInfo {
                calculation: second.clone(),
                target: 1,
            },
        ];
        let mut index = CalculationIndex::default();
        index.insert(7, 0);
        index.insert(7, 1);

        assert_eq!(index.find(7, &first, &calculations), Some(0));
        assert_eq!(index.find(7, &second, &calculations), Some(1));
        assert_eq!(
            index.find(
                7,
                &Calculation::Store(ValueSource::Advice(5, 0)),
                &calculations,
            ),
            None
        );
    }

    #[test]
    fn custom_gate_workers_are_bounded_by_total_scratch() {
        let mut graph = GraphEvaluator::<EqAffine>::default();
        let scalar_bytes = std::mem::size_of::<<EqAffine as CurveAffine>::ScalarExt>();
        graph.num_intermediates = CUSTOM_GATE_SCRATCH_BUDGET_BYTES / scalar_bytes + 1;

        assert_eq!(graph.memory_bounded_worker_count(1 << 16, 16), 1);
        assert_eq!(graph.memory_bounded_worker_count(1, 16), 1);

        graph.num_intermediates = 1;
        assert_eq!(graph.memory_bounded_worker_count(1 << 16, 16), 16);
    }

    #[test]
    fn intermediate_slot_reuse_preserves_evaluation_with_horner_parts() {
        type Scalar = <EqAffine as CurveAffine>::ScalarExt;

        let mut graph = GraphEvaluator::<EqAffine>::default();
        let first = graph.add_calculation(Calculation::Store(ValueSource::Constant(1)));
        let doubled = graph.add_calculation(Calculation::Double(first));
        let sum = graph.add_calculation(Calculation::Add(first, doubled));
        let product = graph.add_calculation(Calculation::Mul(sum, doubled));
        graph.add_calculation(Calculation::Horner(
            ValueSource::PreviousValue(),
            vec![sum, product, first],
            ValueSource::Y(),
        ));

        let empty_polynomials = Vec::<Polynomial<Scalar, Coeff>>::new();
        let empty_challenges = Vec::<Scalar>::new();
        let zero = Scalar::ZERO;
        let y = Scalar::from(3);
        let previous = Scalar::from(5);
        let mut before_data = graph.instance();
        let before = graph.evaluate(
            &mut before_data,
            &empty_polynomials,
            &empty_polynomials,
            &empty_polynomials,
            &empty_challenges,
            &zero,
            &zero,
            &zero,
            &y,
            &previous,
            0,
            1,
            1,
        );
        let uncompressed_slots = graph.num_intermediates;

        graph.finish_building();
        let mut after_data = graph.instance();
        let after = graph.evaluate(
            &mut after_data,
            &empty_polynomials,
            &empty_polynomials,
            &empty_polynomials,
            &empty_challenges,
            &zero,
            &zero,
            &zero,
            &y,
            &previous,
            0,
            1,
            1,
        );

        assert_eq!(before, after);
        assert!(graph.num_intermediates < uncompressed_slots);
    }
}

/// Simple evaluation of an expression
pub fn evaluate<F: Field, B: Basis>(
    expression: &Expression<F>,
    size: usize,
    rot_scale: i32,
    fixed: &[Polynomial<F, B>],
    advice: &[Polynomial<F, B>],
    instance: &[Polynomial<F, B>],
    challenges: &[F],
) -> Vec<F> {
    let mut values = vec![F::ZERO; size];
    let isize = size as i32;
    parallelize(&mut values, |values, start| {
        for (i, value) in values.iter_mut().enumerate() {
            let idx = start + i;
            *value = expression.evaluate(
                &|scalar| scalar,
                &|_| panic!("virtual selectors are removed during optimization"),
                &|query| {
                    fixed[query.column_index]
                        [get_rotation_idx(idx, query.rotation.0, rot_scale, isize)]
                },
                &|query| {
                    advice[query.column_index]
                        [get_rotation_idx(idx, query.rotation.0, rot_scale, isize)]
                },
                &|query| {
                    instance[query.column_index]
                        [get_rotation_idx(idx, query.rotation.0, rot_scale, isize)]
                },
                &|challenge| challenges[challenge.index()],
                &|a| -a,
                &|a, b| a + &b,
                &|a, b| a * b,
                &|a, scalar| a * scalar,
            );
        }
    });
    values
}
