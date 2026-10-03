//! Joined bounded-opening checks without a full-domain witness or prover tree.

use std::collections::BTreeMap;

use super::*;
use crate::{
    backend::{
        compact_protocol::FixedAir,
        compact_public_columns::COMMITTED_COLUMN_COUNT,
        compact_transfer_air::CompactTransferAir,
        deep_composition::OodPair,
        deep_geometry::LDE_ROWS,
        deep_proof::{
            FriGroup, FriRound, FriValues, OodAnswers, QuotientMaskOpening, RowOpening, RowValues,
        },
    },
    gadgets::compact_smt_air::{PublicStatement, PublicUpdate},
};

fn queries(spread: bool) -> Vec<usize> {
    let mut values = if spread {
        (0usize..128)
            .filter(|i| i.count_ones() % 2 == 1)
            .chain((0usize..128).filter(|i| i.count_ones() % 2 == 0))
            .take(QUERY_COUNT)
            .map(|index| (index | index << 7 | index << 14 | index << 21) & (LDE_ROWS - 1))
            .collect::<Vec<_>>()
    } else {
        (0..QUERY_COUNT).collect()
    };
    values.sort_unstable();
    values
}
// Algebra fixtures deliberately contain unauthenticated roots; this helper has
// no access to VerifiedDeepProof and is absent from every normal build. The full
// commitment test below invokes the normal authentication path independently.
fn check_chains(
    geometry: &DeepGeometry,
    composition: &DeepComposition,
    lambda: F,
    betas: &[F; 5],
    queries: &[usize],
    proof: &DeepProof,
) -> Result<usize> {
    let plans = OpeningPlans::new(queries)?;
    let binding = Context::new(b"algebra-only compressed fibers").unwrap();
    let (checks, _, _) = super::walk_chains(
        geometry,
        composition,
        lambda,
        betas,
        queries,
        proof,
        &binding,
        &plans,
        |round, _, digests| {
            assert_eq!(digests.len(), plans.round_indices[round].len());
            Ok(0)
        },
    )?;
    Ok(checks)
}
fn transmitted_coordinate(plans: &OpeningPlans, round: usize, index: usize, wire: usize) -> usize {
    let omitted = plans.omitted_coordinate(round, index).unwrap();
    if wire < omitted { wire } else { wire + 1 }
}

/// Narrow a fixture position or level; every fixture value stays below `LDE_ROWS`.
fn narrow_u32(value: usize) -> u32 {
    u32::try_from(value).expect("fixture position fits u32")
}

fn constant_fixture(queries: &[usize]) -> (DeepProof, OpeningPlans, DeepComposition) {
    let plans = OpeningPlans::new(queries).unwrap();
    let digest = WireDigest::from_bytes([0x37; 32]);
    let ood = OodAnswers {
        current: vec![F::ONE; COMMITTED_COLUMN_COUNT],
        next: vec![F::ONE; COMMITTED_COLUMN_COUNT],
        quotient: vec![F::ZERO; 2],
    };
    let composition = DeepComposition::new(
        OodPair::new(
            F::new([17, 19, 23, 29]).unwrap(),
            DeepGeometry::new().unwrap().trace_generator(),
        )
        .unwrap(),
        &ood.current,
        &ood.next,
        &ood.quotient,
    )
    .unwrap();
    let proof = DeepProof {
        row_root: digest,
        quotient_root: digest,
        fri_roots: vec![digest; 6],
        ood,
        rows: queries
            .iter()
            .map(|&index| RowOpening {
                index: narrow_u32(index),
                values: RowValues::new(vec![1; COMMITTED_COLUMN_COUNT]).unwrap(),
            })
            .collect(),
        quotients: queries
            .iter()
            .map(|&index| QuotientMaskOpening {
                index: narrow_u32(index),
                low: F::ZERO,
                high: F::ZERO,
                composition_mask: F::ZERO,
            })
            .collect(),
        row_siblings: vec![digest; plans.initial.work().siblings],
        quotient_siblings: vec![digest; plans.initial.work().siblings],
        rounds: plans
            .round_indices
            .iter()
            .enumerate()
            .map(|(round, indices)| FriRound {
                groups: indices
                    .iter()
                    .map(|&index| FriGroup {
                        index: narrow_u32(index),
                        values: FriValues::omit(
                            &vec![F::ZERO; FRI_ARITIES[round]],
                            plans.omitted_coordinate(round, index).unwrap(),
                        )
                        .unwrap(),
                    })
                    .collect(),
                siblings: vec![digest; plans.rounds[round].work().siblings],
            })
            .collect(),
        terminal: vec![F::ZERO; 128],
    };
    deep_proof::preflight(&proof, queries).unwrap();
    (proof, plans, composition)
}

fn betas() -> [F; 5] {
    core::array::from_fn(|index| F::new([index as u64 + 31, 37, 41, 43]).unwrap())
}

#[test]
fn all_initial_indices_follow_their_exact_strided_fibers() {
    let geometry = DeepGeometry::new().unwrap();
    for spread in [false, true] {
        let queries = queries(spread);
        let (proof, _, composition) = constant_fixture(&queries);
        assert_eq!(
            check_chains(
                &geometry,
                &composition,
                F::new([17, 19, 23, 29]).unwrap(),
                &betas(),
                &queries,
                &proof
            )
            .unwrap(),
            QUERY_COUNT * 5
        );
    }
}

#[test]
fn nonconstant_coefficient_chain_checks_fiber_coordinates_cosets_and_challenges() {
    use crate::backend::{field_pow, polynomial_field::PolynomialField};
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, _) = constant_fixture(&queries);
    let z = F::new([17, 19, 23, 29]).unwrap();
    let next_z = z.mul_base(geometry.trace_generator());
    let lambda = F::new([31, 37, 41, 43]).unwrap();
    // A_0(X)=X²; all other trace columns are one, and Q0=Q1=0.
    // With zero R its DEEP composition is lambda+lambda²*X², independent of z.
    proof.ood.current[0] = z.mul(z);
    proof.ood.next[0] = next_z.mul(next_z);
    for row in &mut proof.rows {
        let mut values = vec![1; COMMITTED_COLUMN_COUNT];
        values[0] = field_pow(geometry.domain().point(row.index as usize), 2);
        row.values = RowValues::new(values).unwrap();
    }
    let composition = DeepComposition::new(
        OodPair::new(z, geometry.trace_generator()).unwrap(),
        &proof.ood.current,
        &proof.ood.next,
        &proof.ood.quotient,
    )
    .unwrap();
    let betas = betas();
    let mut coefficients = vec![lambda, F::ZERO, lambda.mul(lambda)];
    let mut domain = geometry.domain();
    let plans = OpeningPlans::new(
        &proof
            .rows
            .iter()
            .map(|r| r.index as usize)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for round in 0..5 {
        for group in &mut proof.rounds[round].groups {
            for (position, value) in group.values.iter_mut().enumerate() {
                let index = group.index as usize
                    + transmitted_coordinate(&plans, round, group.index as usize, position)
                        * FRI_LENGTHS[round + 1];
                let x = domain.point(index);
                *value = coefficients
                    .iter()
                    .rev()
                    .fold(F::ZERO, |sum, &coefficient| {
                        sum.mul_base(x).add(coefficient)
                    });
            }
        }
        // Independent coefficient decomposition; no folding/interpolation helper.
        coefficients = coefficients
            .chunks(FRI_ARITIES[round])
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(F::ZERO, |sum, (power, &coefficient)| {
                        sum.add(coefficient.mul(betas[round].power(power as u64)))
                    })
            })
            .collect();
        domain = domain.folded(FRI_ARITIES[round]);
    }
    assert_eq!(coefficients.len(), 1);
    proof.terminal.fill(coefficients[0]);
    deep_proof::preflight(&proof, &queries).unwrap();
    assert_eq!(
        check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).unwrap(),
        QUERY_COUNT * 5
    );
    proof.rounds[0].groups[0].values.swap(0, 1);
    assert!(check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).is_err());
    proof.rounds[0].groups[0].values.swap(0, 1);
    let mut wrong_betas = betas;
    wrong_betas[0] = wrong_betas[0].add(F::ONE);
    assert!(
        check_chains(
            &geometry,
            &composition,
            lambda,
            &wrong_betas,
            &queries,
            &proof
        )
        .is_err()
    );
    assert!(
        check_chains(
            &geometry,
            &composition,
            lambda.add(F::ONE),
            &betas,
            &queries,
            &proof
        )
        .is_err()
    );
}

#[test]
fn independent_mask_highest_degree_flows_through_every_fold_and_full_terminal() {
    use crate::backend::polynomial_field::PolynomialField;
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, composition) = constant_fixture(&queries);
    let mask = [
        (0, F::new([3, 5, 7, 11]).unwrap()),
        (1, F::new([13, 17, 19, 23]).unwrap()),
        (FRI_DEGREES[0] - 1, F::new([29, 31, 37, 41]).unwrap()),
    ];
    let evaluate = |coefficients: &BTreeMap<usize, F>, x: u64| {
        coefficients
            .iter()
            .fold(F::ZERO, |value, (&degree, &coefficient)| {
                value.add(coefficient.mul(F::embed_base(x).power(degree as u64)))
            })
    };
    let mut coefficients = BTreeMap::from(mask);
    for opening in &mut proof.quotients {
        opening.composition_mask = evaluate(
            &coefficients,
            geometry.domain().point(opening.index as usize),
        );
    }
    // Constant trace / zero quotient has H_lambda=0. The candidate FRI input
    // is exactly R, including its highest permitted coefficient at 2N-1.
    let betas = betas();
    let mut domain = geometry.domain();
    let plans = OpeningPlans::new(&queries).unwrap();
    for (round, &arity) in FRI_ARITIES.iter().enumerate() {
        for group in &mut proof.rounds[round].groups {
            for (coordinate, value) in group.values.iter_mut().enumerate() {
                *value = evaluate(
                    &coefficients,
                    domain.point(
                        group.index as usize
                            + transmitted_coordinate(
                                &plans,
                                round,
                                group.index as usize,
                                coordinate,
                            ) * FRI_LENGTHS[round + 1],
                    ),
                );
            }
        }
        let mut folded = BTreeMap::new();
        for (&degree, &value) in &coefficients {
            let entry = folded.entry(degree / arity).or_insert(F::ZERO);
            *entry = entry.add(value.mul(betas[round].power((degree % arity) as u64)));
        }
        coefficients = folded;
        domain = domain.folded(arity);
    }
    assert_eq!(coefficients.len(), 2);
    assert_ne!(coefficients[&1], F::ZERO);
    for (index, value) in proof.terminal.iter_mut().enumerate() {
        *value = evaluate(&coefficients, domain.point(index));
    }
    deep_proof::preflight(&proof, &queries).unwrap();
    assert_eq!(
        check_chains(&geometry, &composition, F::ONE, &betas, &queries, &proof).unwrap(),
        QUERY_COUNT * 5
    );
    proof.quotients[0].composition_mask = proof.quotients[0].composition_mask.add(F::ONE);
    assert!(check_chains(&geometry, &composition, F::ONE, &betas, &queries, &proof).is_err());
}

/// Degree of the single non-constant trace column in the high-degree chain fixture.
const HIGH_DEGREE: usize = 16_386;

/// Evaluate coefficients, lowest degree first, at one base-field point.
fn horner(coefficients: &[F], x: u64) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul_base(x).add(coefficient)
        })
}

/// Closed form `lambda*(1+lambda*X²)*h(X)` of the high-degree initial DEEP layer.
#[derive(Clone, Copy)]
struct InitialLayer {
    z: F,
    next_z: F,
    lambda: F,
    slope: F,
    intercept: F,
}

impl InitialLayer {
    /// A closed form avoids a 16K-term Horner evaluation for every fiber coordinate.
    fn at(self, x: u64) -> F {
        use crate::backend::{field_pow, mul_mod, polynomial_field::PolynomialField};

        let point = F::from_base(x).unwrap();
        let numerator = F::from_base(field_pow(x, HIGH_DEGREE as u64))
            .unwrap()
            .sub(self.intercept.add(self.slope.mul_base(x)));
        let denominator = point.sub(self.z).mul(point.sub(self.next_z));
        numerator
            .mul(denominator.inverse().unwrap())
            .mul(F::ONE.add(self.lambda.mul_base(mul_mod(x, x))))
            .mul(self.lambda)
    }
}

/// Evaluate one FRI layer polynomial, using the closed form for round zero.
fn layer_value(initial: InitialLayer, coefficients: &[Vec<F>], round: usize, x: u64) -> F {
    if round == 0 {
        initial.at(x)
    } else {
        horner(&coefficients[round], x)
    }
}

/// Independent complete-homogeneous coefficient recurrence for the quotient.
///
/// No producer division, composition evaluation, FFT or FRI helper supplies
/// these coefficients or the coefficient-folded expected layers.
fn high_degree_initial_layer(ood: [F; 2], z: F, next_z: F, lambda: F) -> (InitialLayer, Vec<F>) {
    use crate::backend::polynomial_field::PolynomialField;

    let [current, next] = ood;
    let sum = z.add(next_z);
    let product = z.mul(next_z);
    let mut h = vec![F::ZERO; HIGH_DEGREE - 1];
    h[HIGH_DEGREE - 2] = F::ONE;
    for degree in (0..HIGH_DEGREE - 2).rev() {
        h[degree] = sum
            .mul(h[degree + 1])
            .sub(product.mul(h.get(degree + 2).copied().unwrap_or(F::ZERO)));
    }
    let slope = next.sub(current).mul(next_z.sub(z).inverse().unwrap());
    let intercept = current.sub(z.mul(slope));
    assert_eq!(intercept, F::ZERO.sub(product.mul(h[0])));
    assert_eq!(slope, sum.mul(h[0]).sub(product.mul(h[1])));
    let mut initial_coefficients = vec![F::ZERO; HIGH_DEGREE + 1];
    for (degree, &coefficient) in h.iter().enumerate() {
        initial_coefficients[degree] = initial_coefficients[degree].add(lambda.mul(coefficient));
        initial_coefficients[degree + 2] =
            initial_coefficients[degree + 2].add(lambda.mul(lambda).mul(coefficient));
    }
    let initial = InitialLayer {
        z,
        next_z,
        lambda,
        slope,
        intercept,
    };
    (initial, initial_coefficients)
}

/// Fill every opened fiber and the terminal from coefficient-folded layers.
///
/// Returns each round's layer coefficients and evaluation domain.
fn fill_high_degree_rounds(
    proof: &mut DeepProof,
    geometry: &DeepGeometry,
    initial: InitialLayer,
    initial_coefficients: Vec<F>,
    betas: &[F; 5],
) -> (Vec<Vec<F>>, Vec<crate::backend::FriDomain>) {
    use crate::backend::polynomial_field::PolynomialField;

    const DEGREES: [usize; 6] = [HIGH_DEGREE, 1_024, 64, 8, 1, 0];
    let mut coefficients = vec![initial_coefficients];
    let mut domain = geometry.domain();
    let mut domains = Vec::with_capacity(5);
    let plans = OpeningPlans::new(
        &proof
            .rows
            .iter()
            .map(|r| r.index as usize)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for round in 0..5 {
        domains.push(domain);
        assert_eq!(coefficients[round].len(), DEGREES[round] + 1);
        assert_ne!(coefficients[round][DEGREES[round]], F::ZERO);
        for group in &mut proof.rounds[round].groups {
            for (coordinate, value) in group.values.iter_mut().enumerate() {
                let index = group.index as usize
                    + transmitted_coordinate(&plans, round, group.index as usize, coordinate)
                        * FRI_LENGTHS[round + 1];
                *value = layer_value(initial, &coefficients, round, domain.point(index));
            }
        }
        let powers: Vec<_> = (0..FRI_ARITIES[round])
            .map(|power| betas[round].power(power as u64))
            .collect();
        let next = coefficients[round]
            .chunks(FRI_ARITIES[round])
            .map(|chunk| {
                chunk
                    .iter()
                    .zip(&powers)
                    .fold(F::ZERO, |value, (&coefficient, &power)| {
                        value.add(coefficient.mul(power))
                    })
            })
            .collect();
        coefficients.push(next);
        domain = domain.folded(FRI_ARITIES[round]);
    }
    assert_eq!(coefficients[5].len(), DEGREES[5] + 1);
    proof.terminal.fill(coefficients[5][0]);
    (coefficients, domains)
}

#[test]
fn high_degree_coefficient_chain_checks_every_nonconstant_fold() {
    use crate::backend::{field_pow, mul_mod, polynomial_field::PolynomialField};

    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, _) = constant_fixture(&queries);
    let z = F::new([17, 19, 23, 29]).unwrap();
    let next_z = z.mul_base(geometry.trace_generator());
    let lambda = F::new([31, 37, 41, 43]).unwrap();
    let betas = betas();
    // A_0(X)=X^16386, every other trace column is one, and Q0=Q1=0.
    // Only this column contributes with R=0: D(X)=lambda*(1+lambda*X²)*h(X), where
    // h=(X^16386-U(X))/((X-z)(X-next_z)) and U interpolates the two OOD values.
    proof.ood.current[0] = z.power(HIGH_DEGREE as u64);
    proof.ood.next[0] = next_z.power(HIGH_DEGREE as u64);
    for row in &mut proof.rows {
        let mut values = vec![1; COMMITTED_COLUMN_COUNT];
        values[0] = field_pow(
            geometry.domain().point(row.index as usize),
            HIGH_DEGREE as u64,
        );
        row.values = RowValues::new(values).unwrap();
    }
    let composition = DeepComposition::new(
        OodPair::new(z, geometry.trace_generator()).unwrap(),
        &proof.ood.current,
        &proof.ood.next,
        &proof.ood.quotient,
    )
    .unwrap();
    let (initial, initial_coefficients) =
        high_degree_initial_layer([proof.ood.current[0], proof.ood.next[0]], z, next_z, lambda);
    // Cross-check the closed form against the independently built coefficients.
    for index in [0, LDE_ROWS / 7, LDE_ROWS - 1] {
        let x = geometry.domain().point(index);
        assert_eq!(initial.at(x), horner(&initial_coefficients, x));
    }
    let (coefficients, domains) =
        fill_high_degree_rounds(&mut proof, &geometry, initial, initial_coefficients, &betas);
    deep_proof::preflight(&proof, &queries).unwrap();
    assert_eq!(
        check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).unwrap(),
        QUERY_COUNT * 5
    );

    // Query zero passes through group zero in every round. Keep all other rounds
    // fixed so each corruption must be rejected by the actual joined checker.
    // The parity-first maximum set includes zero among the added even words.
    assert_eq!(queries[0], 0);
    let plans = OpeningPlans::new(&queries).unwrap();
    for round in 0..5 {
        assert_eq!(proof.rounds[round].groups[0].index, 0);
        let correct = proof.rounds[round].groups[0].values.clone();
        for coordinate in 0..FRI_ARITIES[round] - 1 {
            proof.rounds[round].groups[0].values[coordinate] = correct[coordinate].add(F::ONE);
            assert!(
                check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).is_err(),
                "round={round}, coordinate={coordinate}"
            );
            proof.rounds[round].groups[0].values[coordinate] = correct[coordinate];
        }
        assert_ne!(correct[0], correct[1], "round={round}");
        proof.rounds[round].groups[0].values.swap(0, 1);
        assert!(
            check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).is_err(),
            "round={round}, reversed coordinates"
        );
        proof.rounds[round].groups[0].values.swap(0, 1);

        for (coordinate, value) in proof.rounds[round].groups[0].values.iter_mut().enumerate() {
            // Evaluate the same polynomial on a different coset, preserving the
            // within-fiber root orientation and every coefficient and challenge.
            let x = mul_mod(
                domains[round].point(
                    transmitted_coordinate(&plans, round, 0, coordinate) * FRI_LENGTHS[round + 1],
                ),
                2,
            );
            *value = layer_value(initial, &coefficients, round, x);
        }
        assert_ne!(
            proof.rounds[round].groups[0].values, correct,
            "round={round}"
        );
        assert!(
            check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).is_err(),
            "round={round}, wrong coset"
        );
        proof.rounds[round].groups[0].values = correct;

        let mut wrong_betas = betas;
        wrong_betas[round] = wrong_betas[round].add(F::ONE);
        assert!(
            check_chains(
                &geometry,
                &composition,
                lambda,
                &wrong_betas,
                &queries,
                &proof
            )
            .is_err(),
            "round={round}, wrong beta"
        );
    }
    assert_eq!(
        check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).unwrap(),
        QUERY_COUNT * 5
    );
}

#[test]
fn changed_composition_and_every_fiber_coordinate_fail_linkage() {
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, composition) = constant_fixture(&queries);
    let lambda = F::new([17, 19, 23, 29]).unwrap();
    proof.quotients[0].composition_mask = F::ONE;
    assert!(check_chains(&geometry, &composition, lambda, &betas(), &queries, &proof).is_err());
    proof.quotients[0].composition_mask = F::ZERO;
    for (round, &arity) in FRI_ARITIES.iter().enumerate() {
        for coordinate in 0..arity - 1 {
            proof.rounds[round].groups[0].values[coordinate] = F::ONE;
            assert!(
                check_chains(&geometry, &composition, lambda, &betas(), &queries, &proof).is_err(),
                "round={round}, coordinate={coordinate}"
            );
            proof.rounds[round].groups[0].values[coordinate] = F::ZERO;
        }
    }
    for half in 0..2 {
        if half == 0 {
            proof.quotients[0].low = F::ONE;
        } else {
            proof.quotients[0].high = F::ONE;
        }
        assert!(check_chains(&geometry, &composition, lambda, &betas(), &queries, &proof).is_err());
        proof.quotients[0].low = F::ZERO;
        proof.quotients[0].high = F::ZERO;
    }
    let mut changed = vec![1; COMMITTED_COLUMN_COUNT];
    changed[0] = 2;
    proof.rows[0].values = RowValues::new(changed).unwrap();
    assert!(check_chains(&geometry, &composition, lambda, &betas(), &queries, &proof).is_err());
}

#[test]
fn every_terminal_value_is_checked_even_when_not_queried() {
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(false);
    let (mut proof, _, composition) = constant_fixture(&queries);
    for index in 0..128 {
        proof.terminal[index] = F::ONE;
        assert!(
            check_chains(&geometry, &composition, F::ONE, &betas(), &queries, &proof).is_err(),
            "terminal={index}"
        );
        proof.terminal[index] = F::ZERO;
    }
    proof.terminal.fill(F::ONE);
    assert!(check_chains(&geometry, &composition, F::ONE, &betas(), &queries, &proof).is_err());
}

#[test]
fn full_terminal_accepts_a_linear_polynomial_on_the_folded_coset_only() {
    let mut domain = DeepGeometry::new().unwrap().domain();
    for arity in FRI_ARITIES {
        domain = domain.folded(arity);
    }
    let intercept = F::new([17, 19, 23, 29]).unwrap();
    let slope = F::new([31, 37, 41, 43]).unwrap();
    let mut terminal: Vec<_> = (0..FRI_LENGTHS[5])
        .map(|index| intercept.add(slope.mul_base(domain.point(index))))
        .collect();
    check_terminal_degree(domain, &terminal).unwrap();
    terminal[0] = terminal[0].add(F::ONE);
    assert!(check_terminal_degree(domain, &terminal).is_err());
    terminal[0] = terminal[0].sub(F::ONE);
    terminal[127] = terminal[127].add(F::ONE);
    assert!(check_terminal_degree(domain, &terminal).is_err());
    terminal[127] = terminal[127].sub(F::ONE);
    // An index-affine vector is not a degree-one polynomial in the actual
    // coset points. The inverse-folded domain, not vector position, governs it.
    let index_linear: Vec<_> = (0..FRI_LENGTHS[5])
        .map(|index| intercept.add(slope.mul_base(index as u64)))
        .collect();
    assert!(check_terminal_degree(domain, &index_linear).is_err());
    let quadratic: Vec<_> = (0..FRI_LENGTHS[5])
        .map(|index| {
            let x = F::from_base(domain.point(index)).unwrap();
            intercept.add(slope.mul(x.mul(x)))
        })
        .collect();
    assert!(check_terminal_degree(domain, &quadratic).is_err());
}

// Independent sparse-tree reduction: fill the explicitly planned frontier into
// ordered maps, then reduce complete pairs. This creates authentication controls,
// not a complete Fiat-Shamir proof or a witness for the transfer AIR.
fn root_from_frontier(
    binding: &Context,
    oracle: Oracle,
    leaves: usize,
    indices: &[usize],
    plan: &MultiproofPlan,
    digests: &[Digest],
    siblings: &[WireDigest],
) -> WireDigest {
    let mut current: BTreeMap<usize, Digest> = indices
        .iter()
        .copied()
        .zip(digests.iter().copied())
        .collect();
    if leaves == 1 {
        return binding
            .hash_parent(oracle, 1, 0, digests[0], digests[0])
            .unwrap()
            .into();
    }
    for level in 0..leaves.ilog2() as usize {
        for (position, digest) in plan.sibling_positions().iter().zip(siblings) {
            if position.level == level {
                assert!(current.insert(position.index, digest.as_fastpq()).is_none());
            }
        }
        let items: Vec<_> = current.into_iter().collect();
        assert_eq!(items.len() % 2, 0);
        current = items
            .chunks_exact(2)
            .map(|pair| {
                assert_eq!(pair[0].0 ^ 1, pair[1].0);
                let index = pair[0].0 / 2;
                (
                    index,
                    binding
                        .hash_parent(
                            oracle,
                            narrow_u32(level + 1),
                            narrow_u32(index),
                            pair[0].1,
                            pair[1].1,
                        )
                        .unwrap(),
                )
            })
            .collect();
    }
    assert_eq!(current.len(), 1);
    current[&0].into()
}

#[test]
fn all_commitments_authenticate_under_their_exact_statement_and_role() {
    let binding = Context::new(b"independent authentication-only fixture").unwrap();
    let queries = queries(true);
    let (mut proof, plans, composition) = constant_fixture(&queries);
    let row_bytes: Vec<_> = (0..COMMITTED_COLUMN_COUNT)
        .flat_map(|_| 1_u64.to_le_bytes())
        .collect();
    let leaves = queries
        .iter()
        .map(|&index| {
            binding
                .hash_leaf(Oracle::Row, narrow_u32(index), &row_bytes)
                .unwrap()
        })
        .collect::<Vec<_>>();
    proof.row_root = root_from_frontier(
        &binding,
        Oracle::Row,
        LDE_ROWS,
        &queries,
        &plans.initial,
        &leaves,
        &proof.row_siblings,
    );
    let leaves = queries
        .iter()
        .map(|&index| {
            binding
                .hash_leaf(Oracle::QuotientAndMask, narrow_u32(index), &[0; 96])
                .unwrap()
        })
        .collect::<Vec<_>>();
    proof.quotient_root = root_from_frontier(
        &binding,
        Oracle::QuotientAndMask,
        LDE_ROWS,
        &queries,
        &plans.initial,
        &leaves,
        &proof.quotient_siblings,
    );
    for round in 0..5 {
        let oracle = Oracle::Fri(u8::try_from(round).expect("five FRI rounds fit u8"));
        let leaves = plans.round_indices[round]
            .iter()
            .map(|&index| {
                binding
                    .hash_leaf(oracle, narrow_u32(index), &vec![0; FRI_ARITIES[round] * 32])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        proof.fri_roots[round] = root_from_frontier(
            &binding,
            oracle,
            FRI_LENGTHS[round + 1],
            &plans.round_indices[round],
            &plans.rounds[round],
            &leaves,
            &proof.rounds[round].siblings,
        );
    }
    let terminal = binding.hash_leaf(Oracle::Terminal, 0, &[0; 4096]).unwrap();
    proof.fri_roots[5] = root_from_frontier(
        &binding,
        Oracle::Terminal,
        1,
        &[0],
        &plans.terminal,
        &[terminal],
        &[],
    );
    let (leaves, parents) = authenticate(&binding, &proof, &plans).unwrap();
    assert_eq!(leaves, 2 * QUERY_COUNT + 1);
    assert_eq!(parents, 2 * plans.initial.work().parent_hashes + 1);
    let (checks, fri_leaves, fri_parents) = super::check_chains(
        &DeepGeometry::new().unwrap(),
        &composition,
        F::ONE,
        &betas(),
        &queries,
        &proof,
        &binding,
        &plans,
    )
    .unwrap();
    assert_eq!(checks, QUERY_COUNT * 5);
    assert_eq!(fri_leaves, QUERY_COUNT * 5);
    assert_eq!(
        fri_parents,
        plans
            .rounds
            .iter()
            .map(|p| p.work().parent_hashes)
            .sum::<usize>()
    );
    for round in 0..5 {
        let saved = proof.fri_roots[round];
        proof.fri_roots[round] = WireDigest::from_bytes([0xff; 32]);
        assert!(
            super::check_chains(
                &DeepGeometry::new().unwrap(),
                &composition,
                F::ONE,
                &betas(),
                &queries,
                &proof,
                &binding,
                &plans
            )
            .is_err()
        );
        proof.fri_roots[round] = saved;
    }
    proof.quotients[0].composition_mask = F::ONE;
    assert!(authenticate(&binding, &proof, &plans).is_err());
    proof.quotients[0].composition_mask = F::ZERO;
    let changed = Context::new(b"another caller statement").unwrap();
    assert!(authenticate(&changed, &proof, &plans).is_err());
    let original = proof.row_root;
    proof.row_root = proof.quotient_root;
    assert!(authenticate(&binding, &proof, &plans).is_err());
    proof.row_root = original;
    proof.fri_roots[5] = terminal.into();
    assert!(authenticate(&binding, &proof, &plans).is_err());
}

#[test]
fn raw_verifier_rejects_invalid_frames_and_enforces_the_caller_byte_cap() {
    let mut digest = [0; 8];
    digest[7] = 1 << 24;
    let statement = PublicStatement {
        updates: [PublicUpdate {
            old_leaf: digest,
            new_leaf: digest,
            path: 0,
        }; 2],
        old_root: digest,
        new_root: digest,
    };
    let relation = CompactTransferAir::new(&statement, Some(b"caller statement")).unwrap();
    assert!(verify(&relation, &[], 512 * 1024).is_err());
    assert!(matches!(
        verify(&relation, &[0; 1], 0),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    let (proof, _, _) = constant_fixture(&queries(true));
    let bytes = norito::encode_canonical(&proof).unwrap();
    assert!(matches!(
        verify(&relation, &bytes, bytes.len() - 1),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    // A shaped carrier with arbitrary roots/answers has no acceptance authority.
    assert!(verify(&relation, &bytes, bytes.len()).is_err());
}

#[test]
fn transcript_field_messages_require_the_exact_complete_dimension() {
    let binding = Context::new(b"field message shape fixture").unwrap();
    let mut transcript = Transcript::new(binding.clone());
    assert!(fields(&mut transcript, 1).is_err());
    let mut transcript = Transcript::new(binding);
    assert_eq!(transcript.challenge().unwrap(), Message::Dummy);
    transcript
        .commit_root(Oracle::Row, Digest::default())
        .unwrap();
    assert_eq!(
        fields(&mut transcript, CONSTRAINTS).unwrap().len(),
        CONSTRAINTS
    );
    assert!(fields(&mut transcript, 1).is_err());
    assert!(matches!(
        binding_error(BindingError::Phase),
        Error::InvalidTraceShape { .. }
    ));
}

fn policy_relation() -> CompactTransferAir {
    let mut digest = [0; 8];
    digest[7] = 1 << 24;
    CompactTransferAir::new(
        &PublicStatement {
            updates: [PublicUpdate {
                old_leaf: digest,
                new_leaf: digest,
                path: 0,
            }; 2],
            old_root: digest,
            new_root: digest,
        },
        Some(b"complete committed verifier policy"),
    )
    .unwrap()
}

#[test]
fn committed_policy_checks_every_segment_dimension_before_decoding() {
    type Setter = fn(&mut VerifyLimits, usize);
    let relation = policy_relation();
    let bytes = vec![0; deep_proof::MAX_FRAME_BYTES];
    let exact = VerifyLimits {
        // The enclosing public bundle checks its transition table, before any
        // segment is constructed. This private helper has no transition table.
        max_transitions: 0,
        max_batch_bytes: relation.statement_bytes().len(),
        max_proof_bytes: bytes.len(),
        max_fri_layers: 6,
        max_queries: QUERY_COUNT,
        max_query_chunk_values: 2,
        max_query_path_len: 23,
        max_fri_round_values: 16,
        max_air_row_values: COMMITTED_COLUMN_COUNT,
    };
    preflight(&relation, bytes.len(), exact).unwrap();
    let dimensions: [(&str, usize, Setter); 8] = [
        (
            "max_compact_statement_bytes",
            exact.max_batch_bytes,
            |l, v| l.max_batch_bytes = v,
        ),
        ("max_proof_bytes", exact.max_proof_bytes, |l, v| {
            l.max_proof_bytes = v
        }),
        ("max_fri_layers", 6, |l, v| l.max_fri_layers = v),
        ("max_queries", QUERY_COUNT, |l, v| l.max_queries = v),
        ("max_query_chunk_values", 2, |l, v| {
            l.max_query_chunk_values = v
        }),
        ("max_query_path_len", 23, |l, v| l.max_query_path_len = v),
        ("max_fri_round_values", 16, |l, v| {
            l.max_fri_round_values = v
        }),
        ("max_air_row_values", COMMITTED_COLUMN_COUNT, |l, v| {
            l.max_air_row_values = v
        }),
    ];
    for (name, required, set) in dimensions {
        let mut limited = exact;
        set(&mut limited, required - 1);
        assert!(matches!(
            verify_committed(&relation, &bytes, limited, deep_proof::MAX_ALLOCATION_CHARGES),
            Err(Error::VerifierLimitExceeded { limit, actual, max })
                if limit == name && actual == required && max == required - 1
        ));
    }
    // Inclusive geometry does not grant a success result to malformed bytes.
    assert!(
        verify_committed(&relation, &bytes, exact, deep_proof::MAX_ALLOCATION_CHARGES).is_err()
    );
    assert!(matches!(
        preflight(
            &relation,
            deep_proof::MAX_FRAME_BYTES + 1,
            VerifyLimits {
                max_proof_bytes: usize::MAX,
                ..exact
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            max: deep_proof::MAX_FRAME_BYTES,
            ..
        })
    ));
}

#[test]
fn committed_result_is_unavailable_for_decoded_but_unauthenticated_carriers() {
    let relation = policy_relation();
    let (proof, _, _) = constant_fixture(&queries(true));
    let bytes = norito::encode_canonical(&proof).unwrap();
    let limits = VerifyLimits {
        max_proof_bytes: bytes.len(),
        ..VerifyLimits::default()
    };
    preflight(&relation, bytes.len(), limits).unwrap();
    assert!(deep_proof::decode(&bytes, bytes.len()).is_ok());
    assert!(verify_committed(&relation, &bytes, limits, 0).is_err());
    assert!(
        verify_committed(
            &relation,
            &bytes,
            limits,
            deep_proof::MAX_ALLOCATION_CHARGES
        )
        .is_err()
    );
}

#[test]
fn producer_preflight_rejects_the_fixed_statement_envelope_before_private_work() {
    let reference = policy_relation();
    let mut digest = [0; 8];
    digest[7] = 1 << 24;
    let context = vec![0; 240 * 1024];
    let oversized = CompactTransferAir::new(
        &PublicStatement {
            updates: [PublicUpdate {
                old_leaf: digest,
                new_leaf: digest,
                path: 0,
            }; 2],
            old_root: digest,
            new_root: digest,
        },
        Some(&context),
    )
    .unwrap();
    assert!(oversized.statement_bytes().len() > context.len());
    let policy = VerifyLimits {
        max_batch_bytes: oversized.statement_bytes().len(),
        ..VerifyLimits::default()
    };
    preflight(&reference, deep_proof::MAX_FRAME_BYTES, policy).unwrap();
    assert!(matches!(
        preflight(&oversized, deep_proof::MAX_FRAME_BYTES, policy),
        Err(Error::InvalidTraceShape { .. })
    ));
}

#[test]
fn shared_fibers_check_every_incoming_coordinate_before_authentication_or_deduplication() {
    let mut queries = (0..QUERY_COUNT - 1).collect::<Vec<_>>();
    queries.push(FRI_LENGTHS[1]);
    let (mut proof, plans, composition) = constant_fixture(&queries);
    let geometry = DeepGeometry::new().unwrap();
    let binding = Context::new(b"shared incoming edge regression").unwrap();
    assert_eq!(plans.round_indices[0].len(), QUERY_COUNT - 1);
    assert_eq!(plans.omitted_coordinate(0, 0).unwrap(), 0);
    let mut visited = 0;
    let (checks, _, _) = super::walk_chains(
        &geometry,
        &composition,
        F::ONE,
        &[F::ZERO; 5],
        &queries,
        &proof,
        &binding,
        &plans,
        |_, _, _| {
            visited += 1;
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(visited, 5);
    assert_eq!(checks, QUERY_COUNT + 4 * (QUERY_COUNT - 1));
    // Index L/16 enters coordinate one of the same fiber as index zero.
    // That coordinate remains on wire, and beta=0 cannot excuse its equality.
    proof.rounds[0].groups[0].values[0] = F::ONE;
    visited = 0;
    assert!(
        super::walk_chains(
            &geometry,
            &composition,
            F::ONE,
            &[F::ZERO; 5],
            &queries,
            &proof,
            &binding,
            &plans,
            |_, _, _| {
                visited += 1;
                Ok(0)
            }
        )
        .is_err()
    );
    assert_eq!(visited, 0);
}

/// Exact authenticated algebra fixture; never constructs a verifier success token.
fn retirement_authentication_fixture() -> (
    Context,
    Vec<usize>,
    DeepProof,
    OpeningPlans,
    DeepComposition,
) {
    let binding = Context::new(b"independent authentication-only fixture").unwrap();
    let queries = queries(true);
    let (mut proof, plans, composition) = constant_fixture(&queries);
    let row_bytes: Vec<_> = (0..COMMITTED_COLUMN_COUNT)
        .flat_map(|_| 1_u64.to_le_bytes())
        .collect();
    let leaves = queries
        .iter()
        .map(|&index| {
            binding
                .hash_leaf(Oracle::Row, narrow_u32(index), &row_bytes)
                .unwrap()
        })
        .collect::<Vec<_>>();
    proof.row_root = root_from_frontier(
        &binding,
        Oracle::Row,
        LDE_ROWS,
        &queries,
        &plans.initial,
        &leaves,
        &proof.row_siblings,
    );
    let leaves = queries
        .iter()
        .map(|&index| {
            binding
                .hash_leaf(Oracle::QuotientAndMask, narrow_u32(index), &[0; 96])
                .unwrap()
        })
        .collect::<Vec<_>>();
    proof.quotient_root = root_from_frontier(
        &binding,
        Oracle::QuotientAndMask,
        LDE_ROWS,
        &queries,
        &plans.initial,
        &leaves,
        &proof.quotient_siblings,
    );
    for round in 0..5 {
        let oracle = Oracle::Fri(u8::try_from(round).expect("five FRI rounds fit u8"));
        let leaves = plans.round_indices[round]
            .iter()
            .map(|&index| {
                binding
                    .hash_leaf(oracle, narrow_u32(index), &vec![0; FRI_ARITIES[round] * 32])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        proof.fri_roots[round] = root_from_frontier(
            &binding,
            oracle,
            FRI_LENGTHS[round + 1],
            &plans.round_indices[round],
            &plans.rounds[round],
            &leaves,
            &proof.rounds[round].siblings,
        );
    }
    let terminal = binding.hash_leaf(Oracle::Terminal, 0, &[0; 4096]).unwrap();
    proof.fri_roots[5] = root_from_frontier(
        &binding,
        Oracle::Terminal,
        1,
        &[0],
        &plans.terminal,
        &[terminal],
        &[],
    );
    (binding, queries, proof, plans, composition)
}

#[test]
fn every_current_root_frontier_byte_and_extension_lane_is_authenticated() {
    fn changed(value: WireDigest, byte: usize) -> WireDigest {
        let mut bytes = value.into_bytes();
        bytes[byte] ^= 1;
        WireDigest::from_bytes(bytes)
    }
    fn frontier(proof: &mut DeepProof, which: usize) -> &mut Vec<WireDigest> {
        match which {
            0 => &mut proof.row_siblings,
            1 => &mut proof.quotient_siblings,
            _ => &mut proof.rounds[which - 2].siblings,
        }
    }
    let (binding, queries, mut proof, plans, composition) = retirement_authentication_fixture();
    let check = |proof: &DeepProof| {
        authenticate(&binding, proof, &plans)?;
        super::check_chains(
            &DeepGeometry::new()?,
            &composition,
            F::ONE,
            &betas(),
            &queries,
            proof,
            &binding,
            &plans,
        )?;
        Ok::<_, Error>(())
    };
    check(&proof).unwrap();
    for root in 0..8 {
        let saved = match root {
            0 => proof.row_root,
            1 => proof.quotient_root,
            _ => proof.fri_roots[root - 2],
        };
        for byte in 0..32 {
            let value = changed(saved, byte);
            match root {
                0 => proof.row_root = value,
                1 => proof.quotient_root = value,
                _ => proof.fri_roots[root - 2] = value,
            }
            assert!(check(&proof).is_err(), "root={root}, byte={byte}");
        }
        match root {
            0 => proof.row_root = saved,
            1 => proof.quotient_root = saved,
            _ => proof.fri_roots[root - 2] = saved,
        }
    }
    for which in 0..7 {
        let count = frontier(&mut proof, which).len();
        assert!(count > 0);
        for index in [0, count / 2, count - 1] {
            let saved = frontier(&mut proof, which)[index];
            for byte in 0..32 {
                frontier(&mut proof, which)[index] = changed(saved, byte);
                assert!(
                    check(&proof).is_err(),
                    "frontier={which}, sibling={index}, byte={byte}"
                );
            }
            frontier(&mut proof, which)[index] = saved;
        }
    }
    let increment = |value: F, lane: usize| {
        let mut words = value.coefficients();
        words[lane] = crate::backend::add_mod(words[lane], 1);
        F::new(words).unwrap()
    };
    for half in 0..3 {
        let saved = match half {
            0 => proof.quotients[0].low,
            1 => proof.quotients[0].high,
            _ => proof.quotients[0].composition_mask,
        };
        for lane in 0..4 {
            let value = increment(saved, lane);
            match half {
                0 => proof.quotients[0].low = value,
                1 => proof.quotients[0].high = value,
                _ => proof.quotients[0].composition_mask = value,
            }
            assert!(check(&proof).is_err(), "quotient/mask={half}, lane={lane}");
        }
        match half {
            0 => proof.quotients[0].low = saved,
            1 => proof.quotients[0].high = saved,
            _ => proof.quotients[0].composition_mask = saved,
        }
    }
    for (round, &arity) in FRI_ARITIES.iter().enumerate() {
        for coordinate in 0..arity - 1 {
            let saved = proof.rounds[round].groups[0].values[coordinate];
            for lane in 0..4 {
                proof.rounds[round].groups[0].values[coordinate] = increment(saved, lane);
                assert!(
                    check(&proof).is_err(),
                    "round={round}, coordinate={coordinate}, lane={lane}"
                );
            }
            proof.rounds[round].groups[0].values[coordinate] = saved;
        }
    }
    for index in 0..128 {
        let saved = proof.terminal[index];
        for lane in 0..4 {
            proof.terminal[index] = increment(saved, lane);
            assert!(check(&proof).is_err(), "terminal={index}, lane={lane}");
        }
        proof.terminal[index] = saved;
    }
    check(&proof).unwrap();
}
