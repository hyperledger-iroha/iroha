use super::super::{
    ChallengeBeta, ChallengeGamma, ChallengeTheta, ChallengeX, Error, ProvingKey,
    circuit::Expression,
};
use super::Argument;
use crate::multicore::{self, IntoParallelIterator};
#[cfg(feature = "multicore")]
use crate::multicore::{
    IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator, ParallelSliceMut,
};
use crate::plonk::evaluation::evaluate;
use crate::{
    arithmetic::{CurveAffine, eval_polynomial, parallelize},
    poly::{
        Coeff, EvaluationDomain, LagrangeCoeff, Polynomial, ProverQuery, Rotation,
        commitment::{Blind, Params},
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
#[cfg(feature = "profile")]
use ark_std::{end_timer, start_timer};
use ff::{PrimeField, WithSmallOrderMulGroup};
use group::{
    Curve,
    ff::{BatchInvert, Field},
};
use rand_core::RngCore;

use std::collections::HashMap;
use std::hash::Hash;
use std::{
    collections::BTreeMap,
    iter,
    ops::{Mul, MulAssign},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScalarKey<F: PrimeField>(F);

impl<F: PrimeField> Hash for ScalarKey<F> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_repr().as_ref().hash(state);
    }
}

#[derive(Debug)]
pub(in crate::plonk) struct Permuted<C: CurveAffine> {
    compressed_input_expression: Polynomial<C::Scalar, LagrangeCoeff>,
    permuted_input_expression: Polynomial<C::Scalar, LagrangeCoeff>,
    permuted_input_poly: Polynomial<C::Scalar, Coeff>,
    permuted_input_blind: Blind<C::Scalar>,
    compressed_table_expression: Polynomial<C::Scalar, LagrangeCoeff>,
    permuted_table_expression: Polynomial<C::Scalar, LagrangeCoeff>,
    permuted_table_poly: Polynomial<C::Scalar, Coeff>,
    permuted_table_blind: Blind<C::Scalar>,
}

#[derive(Debug)]
pub(in crate::plonk) struct Committed<C: CurveAffine> {
    pub(in crate::plonk) permuted_input_poly: Polynomial<C::Scalar, Coeff>,
    permuted_input_blind: Blind<C::Scalar>,
    pub(in crate::plonk) permuted_table_poly: Polynomial<C::Scalar, Coeff>,
    permuted_table_blind: Blind<C::Scalar>,
    pub(in crate::plonk) product_poly: Polynomial<C::Scalar, Coeff>,
    product_blind: Blind<C::Scalar>,
}

pub(in crate::plonk) struct Evaluated<C: CurveAffine> {
    constructed: Committed<C>,
}

impl<F: WithSmallOrderMulGroup<3>> Argument<F> {
    /// Given a Lookup with input expressions [A_0, A_1, ..., A_{m-1}] and table expressions
    /// [S_0, S_1, ..., S_{m-1}], this method
    /// - constructs A_compressed = \theta^{m-1} A_0 + theta^{m-2} A_1 + ... + \theta A_{m-2} + A_{m-1}
    ///   and S_compressed = \theta^{m-1} S_0 + theta^{m-2} S_1 + ... + \theta S_{m-2} + S_{m-1},
    /// - permutes A_compressed and S_compressed using permute_expression_pair() helper,
    ///   obtaining A' and S', and
    /// - constructs Permuted<C> struct using permuted_input_value = A', and
    ///   permuted_table_expression = S'.
    /// The Permuted<C> struct is used to update the Lookup, and is then returned.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::plonk) fn commit_permuted<
        'a,
        'params: 'a,
        C,
        P: Params<'params, C>,
        E: EncodedChallenge<C>,
        R: RngCore,
        T: TranscriptWrite<C, E>,
    >(
        &self,
        pk: &ProvingKey<C>,
        params: &P,
        domain: &EvaluationDomain<C::Scalar>,
        theta: ChallengeTheta<C>,
        advice_values: &'a [Polynomial<C::Scalar, LagrangeCoeff>],
        fixed_values: &'a [Polynomial<C::Scalar, LagrangeCoeff>],
        instance_values: &'a [Polynomial<C::Scalar, LagrangeCoeff>],
        challenges: &'a [C::Scalar],
        mut rng: R,
        transcript: &mut T,
    ) -> Result<Permuted<C>, Error>
    where
        C: CurveAffine<ScalarExt = F>,
        C::Curve: Mul<F, Output = C::Curve> + MulAssign<F>,
    {
        // Closure to get values of expressions and compress them
        let compress_expressions = |expressions: &[Expression<C::Scalar>]| {
            let compressed_expression = expressions
                .iter()
                .map(|expression| {
                    pk.vk.domain.lagrange_from_vec(evaluate(
                        expression,
                        params.n() as usize,
                        1,
                        fixed_values,
                        advice_values,
                        instance_values,
                        challenges,
                    ))
                })
                .fold(domain.empty_lagrange(), |acc, expression| {
                    acc * *theta + &expression
                });
            compressed_expression
        };

        // Get values of input expressions involved in the lookup and compress them
        let compressed_input_expression = compress_expressions(&self.input_expressions);

        // Get values of table expressions involved in the lookup and compress them
        let compressed_table_expression = compress_expressions(&self.table_expressions);

        // Permute compressed (InputExpression, TableExpression) pair
        let (permuted_input_expression, permuted_table_expression) = permute_expression_pair(
            pk,
            params,
            domain,
            &mut rng,
            &compressed_input_expression,
            &compressed_table_expression,
        )?;

        // Closure to construct commitment to vector of values
        let mut commit_values = |values: &Polynomial<C::Scalar, LagrangeCoeff>| {
            let poly = pk.vk.domain.lagrange_to_coeff(values.clone());
            let blind = Blind(C::Scalar::random(&mut rng));
            let commitment = params.commit_lagrange(values, blind).to_affine();
            (poly, blind, commitment)
        };

        // Commit to permuted input expression
        let (permuted_input_poly, permuted_input_blind, permuted_input_commitment) =
            commit_values(&permuted_input_expression);

        // Commit to permuted table expression
        let (permuted_table_poly, permuted_table_blind, permuted_table_commitment) =
            commit_values(&permuted_table_expression);

        // Hash permuted input commitment
        transcript.write_point(permuted_input_commitment)?;

        // Hash permuted table commitment
        transcript.write_point(permuted_table_commitment)?;

        Ok(Permuted {
            compressed_input_expression,
            permuted_input_expression,
            permuted_input_poly,
            permuted_input_blind,
            compressed_table_expression,
            permuted_table_expression,
            permuted_table_poly,
            permuted_table_blind,
        })
    }
}

impl<C: CurveAffine> Permuted<C> {
    /// Given a Lookup with input expressions, table expressions, and the permuted
    /// input expression and permuted table expression, this method constructs the
    /// grand product polynomial over the lookup. The grand product polynomial
    /// is used to populate the Product<C> struct. The Product<C> struct is
    /// added to the Lookup and finally returned by the method.
    pub(in crate::plonk) fn commit_product<
        'params,
        P: Params<'params, C>,
        E: EncodedChallenge<C>,
        R: RngCore,
        T: TranscriptWrite<C, E>,
    >(
        self,
        pk: &ProvingKey<C>,
        params: &P,
        beta: ChallengeBeta<C>,
        gamma: ChallengeGamma<C>,
        mut rng: R,
        transcript: &mut T,
    ) -> Result<Committed<C>, Error> {
        let blinding_factors = pk.vk.cs.blinding_factors();
        // Goal is to compute the products of fractions
        //
        // Numerator: (\theta^{m-1} a_0(\omega^i) + \theta^{m-2} a_1(\omega^i) + ... + \theta a_{m-2}(\omega^i) + a_{m-1}(\omega^i) + \beta)
        //            * (\theta^{m-1} s_0(\omega^i) + \theta^{m-2} s_1(\omega^i) + ... + \theta s_{m-2}(\omega^i) + s_{m-1}(\omega^i) + \gamma)
        // Denominator: (a'(\omega^i) + \beta) (s'(\omega^i) + \gamma)
        //
        // where a_j(X) is the jth input expression in this lookup,
        // where a'(X) is the compression of the permuted input expressions,
        // s_j(X) is the jth table expression in this lookup,
        // s'(X) is the compression of the permuted table expressions,
        // and i is the ith row of the expression.
        let mut lookup_product = vec![C::Scalar::ZERO; params.n() as usize];
        // Denominator uses the permuted input expression and permuted table expression
        parallelize(&mut lookup_product, |lookup_product, start| {
            for ((lookup_product, permuted_input_value), permuted_table_value) in lookup_product
                .iter_mut()
                .zip(self.permuted_input_expression[start..].iter())
                .zip(self.permuted_table_expression[start..].iter())
            {
                *lookup_product = (*beta + permuted_input_value) * &(*gamma + permuted_table_value);
            }
        });

        // Batch invert to obtain the denominators for the lookup product
        // polynomials
        lookup_product.iter_mut().batch_invert();

        // Finish the computation of the entire fraction by computing the numerators
        // (\theta^{m-1} a_0(\omega^i) + \theta^{m-2} a_1(\omega^i) + ... + \theta a_{m-2}(\omega^i) + a_{m-1}(\omega^i) + \beta)
        // * (\theta^{m-1} s_0(\omega^i) + \theta^{m-2} s_1(\omega^i) + ... + \theta s_{m-2}(\omega^i) + s_{m-1}(\omega^i) + \gamma)
        parallelize(&mut lookup_product, |product, start| {
            for (i, product) in product.iter_mut().enumerate() {
                let i = i + start;

                *product *= &(self.compressed_input_expression[i] + &*beta);
                *product *= &(self.compressed_table_expression[i] + &*gamma);
            }
        });

        // The product vector is a vector of products of fractions of the form
        //
        // Numerator: (\theta^{m-1} a_0(\omega^i) + \theta^{m-2} a_1(\omega^i) + ... + \theta a_{m-2}(\omega^i) + a_{m-1}(\omega^i) + \beta)
        //            * (\theta^{m-1} s_0(\omega^i) + \theta^{m-2} s_1(\omega^i) + ... + \theta s_{m-2}(\omega^i) + s_{m-1}(\omega^i) + \gamma)
        // Denominator: (a'(\omega^i) + \beta) (s'(\omega^i) + \gamma)
        //
        // where there are m input expressions and m table expressions,
        // a_j(\omega^i) is the jth input expression in this lookup,
        // a'j(\omega^i) is the permuted input expression,
        // s_j(\omega^i) is the jth table expression in this lookup,
        // s'(\omega^i) is the permuted table expression,
        // and i is the ith row of the expression.

        // Compute the evaluations of the lookup product polynomial
        // over our domain, starting with z[0] = 1
        let z = iter::once(C::Scalar::ONE)
            .chain(lookup_product)
            .scan(C::Scalar::ONE, |state, cur| {
                *state *= &cur;
                Some(*state)
            })
            // Take all rows including the "last" row which should
            // be a boolean (and ideally 1, else soundness is broken)
            .take(params.n() as usize - blinding_factors)
            // Chain random blinding factors.
            .chain((0..blinding_factors).map(|_| C::Scalar::random(&mut rng)))
            .collect::<Vec<_>>();
        assert_eq!(z.len(), params.n() as usize);
        let z = pk.vk.domain.lagrange_from_vec(z);

        #[cfg(feature = "sanity-checks")]
        // This test works only with intermediate representations in this method.
        // It can be used for debugging purposes.
        {
            // While in Lagrange basis, check that product is correctly constructed
            let u = (params.n() as usize) - (blinding_factors + 1);

            // l_0(X) * (1 - z(X)) = 0
            assert_eq!(z[0], C::Scalar::ONE);

            // z(\omega X) (a'(X) + \beta) (s'(X) + \gamma)
            // - z(X) (\theta^{m-1} a_0(X) + ... + a_{m-1}(X) + \beta) (\theta^{m-1} s_0(X) + ... + s_{m-1}(X) + \gamma)
            for i in 0..u {
                let mut left = z[i + 1];
                let permuted_input_value = &self.permuted_input_expression[i];

                let permuted_table_value = &self.permuted_table_expression[i];

                left *= &(*beta + permuted_input_value);
                left *= &(*gamma + permuted_table_value);

                let mut right = z[i];
                let mut input_term = self.compressed_input_expression[i];
                let mut table_term = self.compressed_table_expression[i];

                input_term += &(*beta);
                table_term += &(*gamma);
                right *= &(input_term * &table_term);

                assert_eq!(left, right);
            }

            // l_last(X) * (z(X)^2 - z(X)) = 0
            // Assertion will fail only when soundness is broken, in which
            // case this z[u] value will be zero. (bad!)
            assert_eq!(z[u], C::Scalar::ONE);
        }

        let product_blind = Blind(C::Scalar::random(rng));
        let product_commitment = params.commit_lagrange(&z, product_blind).to_affine();
        let z = pk.vk.domain.lagrange_to_coeff(z);

        // Hash product commitment
        transcript.write_point(product_commitment)?;

        Ok(Committed::<C> {
            permuted_input_poly: self.permuted_input_poly,
            permuted_input_blind: self.permuted_input_blind,
            permuted_table_poly: self.permuted_table_poly,
            permuted_table_blind: self.permuted_table_blind,
            product_poly: z,
            product_blind,
        })
    }
}

impl<C: CurveAffine> Committed<C> {
    pub(in crate::plonk) fn evaluate<E: EncodedChallenge<C>, T: TranscriptWrite<C, E>>(
        self,
        pk: &ProvingKey<C>,
        x: ChallengeX<C>,
        transcript: &mut T,
    ) -> Result<Evaluated<C>, Error> {
        let domain = &pk.vk.domain;
        let x_inv = domain.rotate_omega(*x, Rotation::prev());
        let x_next = domain.rotate_omega(*x, Rotation::next());

        let product_eval = eval_polynomial(&self.product_poly, *x);
        let product_next_eval = eval_polynomial(&self.product_poly, x_next);
        let permuted_input_eval = eval_polynomial(&self.permuted_input_poly, *x);
        let permuted_input_inv_eval = eval_polynomial(&self.permuted_input_poly, x_inv);
        let permuted_table_eval = eval_polynomial(&self.permuted_table_poly, *x);

        // Hash each advice evaluation
        for eval in iter::empty()
            .chain(Some(product_eval))
            .chain(Some(product_next_eval))
            .chain(Some(permuted_input_eval))
            .chain(Some(permuted_input_inv_eval))
            .chain(Some(permuted_table_eval))
        {
            transcript.write_scalar(eval)?;
        }

        Ok(Evaluated { constructed: self })
    }
}

impl<C: CurveAffine> Evaluated<C> {
    pub(in crate::plonk) fn open<'a>(
        &'a self,
        pk: &'a ProvingKey<C>,
        x: ChallengeX<C>,
    ) -> impl Iterator<Item = ProverQuery<'a, C>> + Clone {
        let x_inv = pk.vk.domain.rotate_omega(*x, Rotation::prev());
        let x_next = pk.vk.domain.rotate_omega(*x, Rotation::next());

        iter::empty()
            // Open lookup product commitments at x
            .chain(Some(ProverQuery {
                point: *x,
                poly: &self.constructed.product_poly,
                blind: self.constructed.product_blind,
            }))
            // Open lookup input commitments at x
            .chain(Some(ProverQuery {
                point: *x,
                poly: &self.constructed.permuted_input_poly,
                blind: self.constructed.permuted_input_blind,
            }))
            // Open lookup table commitments at x
            .chain(Some(ProverQuery {
                point: *x,
                poly: &self.constructed.permuted_table_poly,
                blind: self.constructed.permuted_table_blind,
            }))
            // Open lookup input commitments at x_inv
            .chain(Some(ProverQuery {
                point: x_inv,
                poly: &self.constructed.permuted_input_poly,
                blind: self.constructed.permuted_input_blind,
            }))
            // Open lookup product commitments at x_next
            .chain(Some(ProverQuery {
                point: x_next,
                poly: &self.constructed.product_poly,
                blind: self.constructed.product_blind,
            }))
    }
}

type ExpressionPair<F> = (Polynomial<F, LagrangeCoeff>, Polynomial<F, LagrangeCoeff>);

/// Order lookup groups canonically before assigning their output rows. Hash-table
/// seeds and parallel merge order must not become unseeded proof randomness.
fn canonical_input_unique_ranges<F: PrimeField + Ord>(
    input_uniques: &HashMap<ScalarKey<F>, usize>,
) -> Vec<(F, std::ops::Range<usize>)> {
    let mut groups = input_uniques
        .iter()
        .map(|(&ScalarKey(value), &count)| (value, 0..count))
        .collect::<Vec<_>>();
    #[cfg(feature = "multicore")]
    groups.par_sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    #[cfg(not(feature = "multicore"))]
    groups.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    let mut offset = 0;
    for (_, range) in &mut groups {
        let count = range.end;
        *range = offset..offset + count;
        offset += count;
    }
    groups
}

/// Given a vector of input values A and a vector of table values S,
/// this method permutes A and S to produce A' and S', such that:
/// - like values in A' are vertically adjacent to each other; and
/// - the first row in a sequence of like values in A' is the row
///   that has the corresponding value in S'.
/// This method returns (A', S') if no errors are encountered.
fn permute_expression_pair<'params, C: CurveAffine, P: Params<'params, C>, R: RngCore>(
    pk: &ProvingKey<C>,
    params: &P,
    domain: &EvaluationDomain<C::Scalar>,
    mut rng: R,
    input_expression: &Polynomial<C::Scalar, LagrangeCoeff>,
    table_expression: &Polynomial<C::Scalar, LagrangeCoeff>,
) -> Result<ExpressionPair<C::Scalar>, Error> {
    let num_threads = multicore::current_num_threads();
    // heuristic on when multi-threading isn't worth it
    // for now it seems like multi-threading is often worth it
    /*if params.n() < (num_threads as u64) << 10 {
        return permute_expression_pair_seq::<_, _, _, ZK>(
            pk,
            params,
            domain,
            rng,
            input_expression,
            table_expression,
        );
    }*/
    let usable_rows = params.n() as usize - (pk.vk.cs.blinding_factors() + 1);

    let input_expression = &input_expression[0..usable_rows];

    // Sort input lookup expression values
    #[cfg(feature = "profile")]
    let input_time = start_timer!(|| "permute_par input hashmap (cpu par)");
    // count input_expression unique values using a HashMap, using rayon parallel fold+reduce
    let capacity = usable_rows / num_threads + 1;

    #[cfg(feature = "multicore")]
    let input_uniques: HashMap<ScalarKey<C::Scalar>, usize> = input_expression
        .par_iter()
        .fold(
            || HashMap::with_capacity(capacity),
            |mut acc, coeff| {
                *acc.entry(ScalarKey(*coeff)).or_insert(0) += 1;
                acc
            },
        )
        .reduce_with(|mut m1, m2| {
            m2.into_iter().for_each(|(key, v)| {
                *m1.entry(key).or_insert(0) += v;
            });
            m1
        })
        .unwrap();
    #[cfg(not(feature = "multicore"))]
    let input_uniques: HashMap<ScalarKey<C::Scalar>, usize> =
        input_expression
            .iter()
            .fold(HashMap::with_capacity(capacity), |mut acc, coeff| {
                *acc.entry(ScalarKey(*coeff)).or_insert(0) += 1;
                acc
            });
    #[cfg(feature = "profile")]
    end_timer!(input_time);

    #[cfg(feature = "profile")]
    let timer = start_timer!(|| "permute_par input unique ranges (cpu par)");

    let input_unique_ranges = canonical_input_unique_ranges(&input_uniques);
    #[cfg(feature = "profile")]
    end_timer!(timer);

    #[cfg(feature = "profile")]
    let to_vec_time = start_timer!(|| "to_vec");
    let mut sorted_table_coeffs = table_expression[0..usable_rows].to_vec();
    #[cfg(feature = "profile")]
    end_timer!(to_vec_time);
    #[cfg(feature = "profile")]
    let sort_table_time = start_timer!(|| "permute_par sort table");
    #[cfg(feature = "multicore")]
    sorted_table_coeffs.par_sort();
    #[cfg(not(feature = "multicore"))]
    sorted_table_coeffs.sort();
    #[cfg(feature = "profile")]
    end_timer!(sort_table_time);

    #[cfg(feature = "profile")]
    let timer = start_timer!(|| "leftover table coeffs (cpu par)");

    let leftover_table_coeffs: Vec<C::Scalar> = sorted_table_coeffs
        .as_slice()
        .into_par_iter()
        .enumerate()
        .filter_map(|(i, coeff)| {
            ((i != 0 && coeff == &sorted_table_coeffs[i - 1])
                || !input_uniques.contains_key(&ScalarKey(*coeff)))
            .then_some(*coeff)
        })
        .collect();
    #[cfg(feature = "profile")]
    end_timer!(timer);

    let (mut permuted_input_expression, mut permuted_table_coeffs): (Vec<_>, Vec<_>) =
        input_unique_ranges
            .into_par_iter()
            .enumerate()
            .flat_map(|(i, (coeff, range))| {
                // subtract off the number of rows in table rows that correspond to input uniques
                let leftover_range_start = range.start - i;
                let leftover_range_end = range.end - i - 1;
                [(coeff, coeff)].into_par_iter().chain(
                    leftover_table_coeffs[leftover_range_start..leftover_range_end]
                        .into_par_iter()
                        .map(move |leftover_table_coeff| (coeff, *leftover_table_coeff)),
                )
            })
            .unzip();
    permuted_input_expression.resize_with(params.n() as usize, || C::Scalar::random(&mut rng));
    permuted_table_coeffs.resize_with(params.n() as usize, || C::Scalar::random(&mut rng));

    Ok((
        domain.lagrange_from_vec(permuted_input_expression),
        domain.lagrange_from_vec(permuted_table_coeffs),
    ))
}

/// Given a vector of input values A and a vector of table values S,
/// this method permutes A and S to produce A' and S', such that:
/// - like values in A' are vertically adjacent to each other; and
/// - the first row in a sequence of like values in A' is the row
///   that has the corresponding value in S'.
/// This method returns (A', S') if no errors are encountered.
#[allow(dead_code)]
fn permute_expression_pair_seq<'params, C: CurveAffine, P: Params<'params, C>, R: RngCore>(
    pk: &ProvingKey<C>,
    params: &P,
    domain: &EvaluationDomain<C::Scalar>,
    mut rng: R,
    input_expression: &Polynomial<C::Scalar, LagrangeCoeff>,
    table_expression: &Polynomial<C::Scalar, LagrangeCoeff>,
) -> Result<ExpressionPair<C::Scalar>, Error> {
    let blinding_factors = pk.vk.cs.blinding_factors();
    let usable_rows = params.n() as usize - (blinding_factors + 1);

    let mut permuted_input_expression: Vec<C::Scalar> = input_expression.to_vec();
    permuted_input_expression.truncate(usable_rows);

    // Sort input lookup expression values
    #[cfg(feature = "multicore")]
    permuted_input_expression.par_sort();
    #[cfg(not(feature = "multicore"))]
    permuted_input_expression.sort();

    // A BTreeMap of each unique element in the table expression and its count
    let mut leftover_table_map: BTreeMap<C::Scalar, u32> = table_expression
        .iter()
        .take(usable_rows)
        .fold(BTreeMap::new(), |mut acc, coeff| {
            *acc.entry(*coeff).or_insert(0) += 1;
            acc
        });
    let mut permuted_table_coeffs = vec![C::Scalar::ZERO; usable_rows];

    let mut repeated_input_rows = permuted_input_expression
        .iter()
        .zip(permuted_table_coeffs.iter_mut())
        .enumerate()
        .filter_map(|(row, (input_value, table_value))| {
            // If this is the first occurrence of `input_value` in the input expression
            if row == 0 || *input_value != permuted_input_expression[row - 1] {
                *table_value = *input_value;
                // Remove one instance of input_value from leftover_table_map
                if let Some(count) = leftover_table_map.get_mut(input_value) {
                    assert!(*count > 0);
                    *count -= 1;
                    None
                } else {
                    // Return error if input_value not found
                    panic!("{:?}", Error::ConstraintSystemFailure);
                    // Some(Err(Error::ConstraintSystemFailure))
                }
            // If input value is repeated
            } else {
                Some(row)
            }
        })
        .collect::<Vec<_>>();

    // Populate permuted table at unfilled rows with leftover table elements
    for (coeff, count) in leftover_table_map.iter() {
        for _ in 0..*count {
            permuted_table_coeffs[repeated_input_rows.pop().unwrap()] = *coeff;
        }
    }
    assert!(repeated_input_rows.is_empty());

    permuted_input_expression
        .extend((0..(blinding_factors + 1)).map(|_| C::Scalar::random(&mut rng)));
    permuted_table_coeffs.extend((0..(blinding_factors + 1)).map(|_| C::Scalar::random(&mut rng)));
    assert_eq!(permuted_input_expression.len(), params.n() as usize);
    assert_eq!(permuted_table_coeffs.len(), params.n() as usize);

    #[cfg(feature = "sanity-checks")]
    {
        let mut last = None;
        for (a, b) in permuted_input_expression
            .iter()
            .zip(permuted_table_coeffs.iter())
            .take(usable_rows)
        {
            if *a != *b {
                assert_eq!(*a, last.unwrap());
            }
            last = Some(*a);
        }
    }

    Ok((
        domain.lagrange_from_vec(permuted_input_expression),
        domain.lagrange_from_vec(permuted_table_coeffs),
    ))
}

#[cfg(test)]
#[path = "deterministic_recovery_tests.rs"]
mod deterministic_recovery_tests;

/// Test-only differential access to the active ordinary permutation, with guaranteed membership.
#[cfg(test)]
pub(crate) fn stored_sort_ordinary_oracle<C: CurveAffine>(
    pk: &ProvingKey<C>,
    params: &crate::poly::ipa::commitment::ParamsIPA<C>,
    values: Vec<C::Scalar>,
) -> Vec<C::Scalar> {
    use rand_core::SeedableRng;
    let polynomial = pk.vk.domain.lagrange_from_vec(values);
    let (input, _) = permute_expression_pair(
        pk,
        params,
        &pk.vk.domain,
        rand_chacha::ChaCha20Rng::from_seed([73; 32]),
        &polynomial,
        &polynomial,
    )
    .unwrap();
    input.to_vec()
}

/// Test-only differential access to the active ordinary pair construction for valid membership.
#[cfg(test)]
pub(crate) fn stored_membership_ordinary_oracle<C: CurveAffine>(
    pk: &ProvingKey<C>,
    params: &crate::poly::ipa::commitment::ParamsIPA<C>,
    input: Vec<C::Scalar>,
    table: Vec<C::Scalar>,
) -> (Vec<C::Scalar>, Vec<C::Scalar>) {
    use rand_core::SeedableRng;
    let input = pk.vk.domain.lagrange_from_vec(input);
    let table = pk.vk.domain.lagrange_from_vec(table);
    let (input, table) = permute_expression_pair(
        pk,
        params,
        &pk.vk.domain,
        rand_chacha::ChaCha20Rng::from_seed([79; 32]),
        &input,
        &table,
    )
    .unwrap();
    (input.to_vec(), table.to_vec())
}

/// Test-only concrete result from the active ordinary lookup commitment implementation.
#[cfg(test)]
pub(crate) struct StoredPermutedOrdinaryOracleV1<C: CurveAffine> {
    pub(crate) input_lagrange: Vec<C::Scalar>,
    pub(crate) table_lagrange: Vec<C::Scalar>,
    pub(crate) input_coefficient: Vec<C::Scalar>,
    pub(crate) table_coefficient: Vec<C::Scalar>,
    pub(crate) input_blind: Blind<C::Scalar>,
    pub(crate) table_blind: Blind<C::Scalar>,
}

/// Invoke the actual ordinary argument, including its tails, conversions, blinds and writes.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn stored_permuted_ordinary_oracle<C, R, T, E>(
    pk: &ProvingKey<C>,
    params: &crate::poly::ipa::commitment::ParamsIPA<C>,
    lookup_index: usize,
    theta: ChallengeTheta<C>,
    advice: &[Polynomial<C::Scalar, LagrangeCoeff>],
    instances: &[Polynomial<C::Scalar, LagrangeCoeff>],
    challenges: &[C::Scalar],
    rng: &mut R,
    transcript: &mut T,
) -> Result<StoredPermutedOrdinaryOracleV1<C>, Error>
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3>,
    C::Curve: Mul<C::Scalar, Output = C::Curve> + MulAssign<C::Scalar>,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    let result = pk.vk.cs.lookups[lookup_index].commit_permuted(
        pk,
        params,
        &pk.vk.domain,
        theta,
        advice,
        &pk.fixed_values,
        instances,
        challenges,
        rng,
        transcript,
    )?;
    Ok(StoredPermutedOrdinaryOracleV1 {
        input_lagrange: result.permuted_input_expression.to_vec(),
        table_lagrange: result.permuted_table_expression.to_vec(),
        input_coefficient: result.permuted_input_poly.to_vec(),
        table_coefficient: result.permuted_table_poly.to_vec(),
        input_blind: result.permuted_input_blind,
        table_blind: result.permuted_table_blind,
    })
}

/// Test-only values from a real ordinary lookup product, including its original pair blinds.
#[cfg(test)]
pub(crate) struct StoredLookupProductOrdinaryOracleV1<C: CurveAffine> {
    pub(crate) input_coefficient: Vec<C::Scalar>,
    pub(crate) table_coefficient: Vec<C::Scalar>,
    pub(crate) product_coefficient: Vec<C::Scalar>,
    pub(crate) input_blind: Blind<C::Scalar>,
    pub(crate) table_blind: Blind<C::Scalar>,
    pub(crate) product_blind: Blind<C::Scalar>,
}
/// Test-only complete ordinary trajectory from immediately after theta through all products.
#[cfg(test)]
pub(crate) struct StoredProductsOrdinaryOracleV1<C: CurveAffine> {
    pub(crate) beta: C::Scalar,
    pub(crate) gamma: C::Scalar,
    pub(crate) pairs: Vec<StoredPermutedOrdinaryOracleV1<C>>,
    pub(crate) permutations:
        Vec<crate::plonk::permutation::prover::StoredCopyProductOrdinaryOracleV1<C>>,
    pub(crate) lookups: Vec<StoredLookupProductOrdinaryOracleV1<C>>,
}
/// Invoke the actual ordinary lookup-permutation, challenge and product routines in order.
/// The caller supplies the genuine cloned RNG and transcript prefix immediately after theta;
/// no reseed, manufactured Permuted owner or independently sampled beta/gamma is substituted.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn stored_products_ordinary_oracle<C, R, T, E>(
    pk: &ProvingKey<C>,
    params: &crate::poly::ipa::commitment::ParamsIPA<C>,
    theta: ChallengeTheta<C>,
    advice: &[Polynomial<C::Scalar, LagrangeCoeff>],
    instances: &[Polynomial<C::Scalar, LagrangeCoeff>],
    challenges: &[C::Scalar],
    rng: &mut R,
    transcript: &mut T,
) -> Result<StoredProductsOrdinaryOracleV1<C>, Error>
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3>,
    C::Curve: Mul<C::Scalar, Output = C::Curve> + MulAssign<C::Scalar>,
    R: RngCore,
    T: TranscriptWrite<C, E>,
    E: EncodedChallenge<C>,
{
    let mut actual = Vec::new();
    let mut pairs = Vec::new();
    for argument in &pk.vk.cs.lookups {
        let pair = argument.commit_permuted(
            pk,
            params,
            &pk.vk.domain,
            theta,
            advice,
            &pk.fixed_values,
            instances,
            challenges,
            &mut *rng,
            transcript,
        )?;
        pairs.push(StoredPermutedOrdinaryOracleV1 {
            input_lagrange: pair.permuted_input_expression.to_vec(),
            table_lagrange: pair.permuted_table_expression.to_vec(),
            input_coefficient: pair.permuted_input_poly.to_vec(),
            table_coefficient: pair.permuted_table_poly.to_vec(),
            input_blind: pair.permuted_input_blind,
            table_blind: pair.permuted_table_blind,
        });
        actual.push(pair);
    }
    let beta: ChallengeBeta<C> = transcript.squeeze_challenge_scalar();
    let gamma: ChallengeGamma<C> = transcript.squeeze_challenge_scalar();
    let permutations = pk
        .vk
        .cs
        .permutation
        .commit(
            params,
            pk,
            &pk.permutation,
            advice,
            &pk.fixed_values,
            instances,
            beta,
            gamma,
            &mut *rng,
            transcript,
        )?
        .stored_product_oracle_values();
    let mut lookups = Vec::new();
    for pair in actual {
        let product = pair.commit_product(pk, params, beta, gamma, &mut *rng, transcript)?;
        lookups.push(StoredLookupProductOrdinaryOracleV1 {
            input_coefficient: product.permuted_input_poly.to_vec(),
            table_coefficient: product.permuted_table_poly.to_vec(),
            product_coefficient: product.product_poly.to_vec(),
            input_blind: product.permuted_input_blind,
            table_blind: product.permuted_table_blind,
            product_blind: product.product_blind,
        });
    }
    Ok(StoredProductsOrdinaryOracleV1 {
        beta: *beta,
        gamma: *gamma,
        pairs,
        permutations,
        lookups,
    })
}

/// Actual ordinary commitments and undivided numerator from the genuine pre-pair trajectory.
#[cfg(test)]
pub(crate) struct StoredQuotientOrdinaryOracleV1<C: CurveAffine> {
    pub(crate) products: StoredProductsOrdinaryOracleV1<C>,
    pub(crate) vanishing: crate::plonk::vanishing::StoredVanishingOrdinaryOracleV1<C>,
    pub(crate) numerator: Vec<C::Scalar>,
    pub(crate) borrowed_numerator: Vec<C::Scalar>,
    pub(crate) advice_coefficient: Vec<Vec<C::Scalar>>,
    pub(crate) instance_coefficient: Vec<Vec<C::Scalar>>,
}

/// Continue the actual pair/product/vanishing routines and both ordinary evaluator owners.
///
/// The caller supplies its cloned original RNG and transcript immediately after theta. Actual
/// Committed values survive through evaluation; no product owner is reconstructed from extracted
/// scalars. The two evaluator calls consume no proof randomness or transcript operations. Their
/// flattened outputs use the ordinary row-major extended-domain layout, before quotient division.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn stored_quotient_ordinary_oracle<C, R, T, E>(
    pk: &ProvingKey<C>,
    params: &crate::poly::ipa::commitment::ParamsIPA<C>,
    theta: ChallengeTheta<C>,
    advice: &[Polynomial<C::Scalar, LagrangeCoeff>],
    instances: &[Polynomial<C::Scalar, LagrangeCoeff>],
    challenges: &[C::Scalar],
    rng: &mut R,
    transcript: &mut T,
) -> Result<StoredQuotientOrdinaryOracleV1<C>, Error>
where
    C: CurveAffine,
    C::Scalar: WithSmallOrderMulGroup<3>,
    C::Curve: Mul<C::Scalar, Output = C::Curve> + MulAssign<C::Scalar>,
    R: RngCore,
    T: TranscriptWrite<C, E>,
    E: EncodedChallenge<C>,
{
    let mut actual_pairs = Vec::new();
    let mut pairs = Vec::new();
    for argument in &pk.vk.cs.lookups {
        let pair = argument.commit_permuted(
            pk,
            params,
            &pk.vk.domain,
            theta,
            advice,
            &pk.fixed_values,
            instances,
            challenges,
            &mut *rng,
            transcript,
        )?;
        pairs.push(StoredPermutedOrdinaryOracleV1 {
            input_lagrange: pair.permuted_input_expression.to_vec(),
            table_lagrange: pair.permuted_table_expression.to_vec(),
            input_coefficient: pair.permuted_input_poly.to_vec(),
            table_coefficient: pair.permuted_table_poly.to_vec(),
            input_blind: pair.permuted_input_blind,
            table_blind: pair.permuted_table_blind,
        });
        actual_pairs.push(pair);
    }
    let beta: ChallengeBeta<C> = transcript.squeeze_challenge_scalar();
    let gamma: ChallengeGamma<C> = transcript.squeeze_challenge_scalar();
    let actual_permutations = vec![pk.vk.cs.permutation.commit(
        params,
        pk,
        &pk.permutation,
        advice,
        &pk.fixed_values,
        instances,
        beta,
        gamma,
        &mut *rng,
        transcript,
    )?];
    let mut actual_lookups = Vec::new();
    for pair in actual_pairs {
        actual_lookups.push(pair.commit_product(pk, params, beta, gamma, &mut *rng, transcript)?);
    }
    let actual_lookups = vec![actual_lookups];
    let vanishing = crate::plonk::vanishing::stored_vanishing_ordinary_oracle(
        params,
        &pk.vk.domain,
        &mut *rng,
        transcript,
    )?;
    let advice_coefficient = advice
        .iter()
        .map(|polynomial| pk.vk.domain.lagrange_to_coeff(polynomial.clone()))
        .collect::<Vec<_>>();
    let instance_coefficient = instances
        .iter()
        .map(|polynomial| pk.vk.domain.lagrange_to_coeff(polynomial.clone()))
        .collect::<Vec<_>>();
    // Exercise the actual borrowed evaluator's dense-sigma branch, and the actual consuming
    // evaluator's streamed-sigma branch, with the same immutable argument commitments.
    let borrowed_numerator = pk.ev.evaluate_h(
        pk,
        &[advice_coefficient.as_slice()],
        &[instance_coefficient.as_slice()],
        challenges,
        vanishing.y,
        *beta,
        *gamma,
        *theta,
        &actual_lookups,
        &actual_permutations,
        false,
    );
    let (numerator, restored_advice) = pk.ev.evaluate_h_consuming_advice(
        pk,
        vec![advice_coefficient],
        &[instance_coefficient.as_slice()],
        challenges,
        vanishing.y,
        *beta,
        *gamma,
        *theta,
        &actual_lookups,
        &actual_permutations,
        true,
    );
    let permutations = actual_permutations
        .into_iter()
        .next()
        .expect("one actual ordinary proof instance")
        .stored_product_oracle_values();
    let lookups = actual_lookups
        .into_iter()
        .next()
        .expect("one actual ordinary proof instance")
        .into_iter()
        .map(|product| StoredLookupProductOrdinaryOracleV1 {
            input_coefficient: product.permuted_input_poly.to_vec(),
            table_coefficient: product.permuted_table_poly.to_vec(),
            product_coefficient: product.product_poly.to_vec(),
            input_blind: product.permuted_input_blind,
            table_blind: product.permuted_table_blind,
            product_blind: product.product_blind,
        })
        .collect();
    Ok(StoredQuotientOrdinaryOracleV1 {
        products: StoredProductsOrdinaryOracleV1 {
            beta: *beta,
            gamma: *gamma,
            pairs,
            permutations,
            lookups,
        },
        vanishing,
        numerator: numerator.to_vec(),
        borrowed_numerator: borrowed_numerator.to_vec(),
        advice_coefficient: restored_advice
            .into_iter()
            .next()
            .expect("one restored ordinary proof instance")
            .into_iter()
            .map(|polynomial| polynomial.to_vec())
            .collect(),
        instance_coefficient: instance_coefficient
            .into_iter()
            .map(|polynomial| polynomial.to_vec())
            .collect(),
    })
}
