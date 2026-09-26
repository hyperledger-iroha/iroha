//! Joined bounded-opening checks without a full-domain witness or prover tree.

use std::collections::BTreeMap;

use super::*;
use crate::{
    backend::{
        compact_public_columns::COMMITTED_COLUMN_COUNT,
        deep_composition::OodPair,
        deep_geometry::LDE_ROWS,
        deep_proof::{
            FriGroup, FriRound, FriValues, OodAnswers, QuotientOpening, RowOpening, RowValues,
        },
    },
    gadgets::compact_smt_air::{PublicStatement, PublicUpdate},
};

fn queries(spread: bool) -> Vec<usize> {
    let mut result: Vec<_> = (0..QUERY_COUNT)
        .map(|index| {
            if spread {
                (index | index << 6 | index << 12 | index << 18) & (LDE_ROWS - 1)
            } else {
                index
            }
        })
        .collect();
    result.sort_unstable();
    result
}

fn constant_fixture(queries: &[usize]) -> (DeepProof, OpeningPlans, DeepComposition) {
    let plans = OpeningPlans::new(queries).unwrap();
    let digest = WireDigest::new([1, 2, 3, 5, 7, 11]).unwrap();
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
                index: index as u32,
                values: RowValues::new(vec![1; COMMITTED_COLUMN_COUNT]).unwrap(),
            })
            .collect(),
        quotients: queries
            .iter()
            .map(|&index| QuotientOpening {
                index: index as u32,
                low: F::ZERO,
                high: F::ZERO,
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
                        index: index as u32,
                        values: FriValues::new(vec![F::ZERO; FRI_ARITIES[round]]).unwrap(),
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
            320
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
    // Its DEEP composition is exactly 1+lambda*X², independent of z.
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
    let mut coefficients = vec![F::ONE, F::ZERO, lambda];
    let mut domain = geometry.domain();
    for round in 0..5 {
        for group in &mut proof.rounds[round].groups {
            for (position, value) in group.values.iter_mut().enumerate() {
                let index = group.index as usize + position * FRI_LENGTHS[round + 1];
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
        320
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
fn changed_composition_and_every_fiber_coordinate_fail_linkage() {
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, composition) = constant_fixture(&queries);
    let lambda = F::new([17, 19, 23, 29]).unwrap();
    for round in 0..5 {
        for coordinate in 0..FRI_ARITIES[round] {
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
                            (level + 1) as u32,
                            index as u32,
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
    let (mut proof, plans, _) = constant_fixture(&queries);
    let row_bytes: Vec<_> = (0..COMMITTED_COLUMN_COUNT)
        .flat_map(|_| 1_u64.to_le_bytes())
        .collect();
    let leaves = queries
        .iter()
        .map(|&index| {
            binding
                .hash_leaf(Oracle::Row, index as u32, &row_bytes)
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
                .hash_leaf(Oracle::QuotientPair, index as u32, &[0; 64])
                .unwrap()
        })
        .collect::<Vec<_>>();
    proof.quotient_root = root_from_frontier(
        &binding,
        Oracle::QuotientPair,
        LDE_ROWS,
        &queries,
        &plans.initial,
        &leaves,
        &proof.quotient_siblings,
    );
    for round in 0..5 {
        let oracle = Oracle::Fri(round as u8);
        let leaves = plans.round_indices[round]
            .iter()
            .map(|&index| {
                binding
                    .hash_leaf(oracle, index as u32, &vec![0; FRI_ARITIES[round] * 32])
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
    assert_eq!(leaves, 449);
    assert_eq!(parents, 4666);
    assert_eq!(leaves + parents + 10, 5125);
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
