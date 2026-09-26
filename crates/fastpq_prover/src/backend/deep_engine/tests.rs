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
        deep_proof::{FriGroup, FriRound, OodAnswers, QuotientOpening, RowOpening, RowValues},
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
                        values: vec![F::ZERO; FRI_ARITIES[round]],
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
fn high_degree_coefficient_chain_checks_every_nonconstant_fold() {
    use crate::backend::{field_pow, mul_mod, polynomial_field::PolynomialField};

    const DEGREE: usize = 16_386;
    const DEGREES: [usize; 6] = [DEGREE, 1_024, 64, 8, 1, 0];
    let geometry = DeepGeometry::new().unwrap();
    let queries = queries(true);
    let (mut proof, _, _) = constant_fixture(&queries);
    let z = F::new([17, 19, 23, 29]).unwrap();
    let next_z = z.mul_base(geometry.trace_generator());
    let lambda = F::new([31, 37, 41, 43]).unwrap();
    let betas = betas();
    // A_0(X)=X^16386, every other trace column is one, and Q0=Q1=0.
    // Only this column contributes: D(X)=(1+lambda*X²)*h(X), where
    // h=(X^16386-U(X))/((X-z)(X-next_z)) and U interpolates the two OOD values.
    proof.ood.current[0] = z.power(DEGREE as u64);
    proof.ood.next[0] = next_z.power(DEGREE as u64);
    for row in &mut proof.rows {
        let mut values = vec![1; COMMITTED_COLUMN_COUNT];
        values[0] = field_pow(geometry.domain().point(row.index as usize), DEGREE as u64);
        row.values = RowValues::new(values).unwrap();
    }
    let composition = DeepComposition::new(
        OodPair::new(z, geometry.trace_generator()).unwrap(),
        &proof.ood.current,
        &proof.ood.next,
        &proof.ood.quotient,
    )
    .unwrap();

    // Independent complete-homogeneous coefficient recurrence for the quotient.
    // No producer division, composition evaluation, FFT or FRI helper supplies
    // these coefficients or the coefficient-folded expected layers.
    let sum = z.add(next_z);
    let product = z.mul(next_z);
    let mut h = vec![F::ZERO; DEGREE - 1];
    h[DEGREE - 2] = F::ONE;
    for degree in (0..DEGREE - 2).rev() {
        h[degree] = sum
            .mul(h[degree + 1])
            .sub(product.mul(h.get(degree + 2).copied().unwrap_or(F::ZERO)));
    }
    let slope = proof.ood.next[0]
        .sub(proof.ood.current[0])
        .mul(next_z.sub(z).inverse().unwrap());
    let intercept = proof.ood.current[0].sub(z.mul(slope));
    assert_eq!(intercept, F::ZERO.sub(product.mul(h[0])));
    assert_eq!(slope, sum.mul(h[0]).sub(product.mul(h[1])));
    let mut initial_coefficients = vec![F::ZERO; DEGREE + 1];
    for (degree, &coefficient) in h.iter().enumerate() {
        initial_coefficients[degree] = initial_coefficients[degree].add(coefficient);
        initial_coefficients[degree + 2] =
            initial_coefficients[degree + 2].add(lambda.mul(coefficient));
    }
    let horner = |coefficients: &[F], x: u64| {
        coefficients
            .iter()
            .rev()
            .fold(F::ZERO, |value, &coefficient| {
                value.mul_base(x).add(coefficient)
            })
    };
    // A closed form avoids a 16K-term Horner evaluation for every initial fiber
    // coordinate. Cross-check it against the independently built coefficients.
    let initial_at = |x: u64| {
        let point = F::from_base(x).unwrap();
        let numerator = F::from_base(field_pow(x, DEGREE as u64))
            .unwrap()
            .sub(intercept.add(slope.mul_base(x)));
        let denominator = point.sub(z).mul(point.sub(next_z));
        numerator
            .mul(denominator.inverse().unwrap())
            .mul(F::ONE.add(lambda.mul_base(mul_mod(x, x))))
    };
    for index in [0, LDE_ROWS / 7, LDE_ROWS - 1] {
        let x = geometry.domain().point(index);
        assert_eq!(initial_at(x), horner(&initial_coefficients, x));
    }
    let mut coefficients = vec![initial_coefficients];
    let mut domain = geometry.domain();
    let mut domains = Vec::with_capacity(5);
    for round in 0..5 {
        domains.push(domain);
        assert_eq!(coefficients[round].len(), DEGREES[round] + 1);
        assert_ne!(coefficients[round][DEGREES[round]], F::ZERO);
        for group in &mut proof.rounds[round].groups {
            for (coordinate, value) in group.values.iter_mut().enumerate() {
                let index = group.index as usize + coordinate * FRI_LENGTHS[round + 1];
                let x = domain.point(index);
                *value = if round == 0 {
                    initial_at(x)
                } else {
                    horner(&coefficients[round], x)
                };
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
    deep_proof::preflight(&proof, &queries).unwrap();
    assert_eq!(
        check_chains(&geometry, &composition, lambda, &betas, &queries, &proof).unwrap(),
        320
    );

    // Query zero passes through group zero in every round. Keep all other rounds
    // fixed so each corruption must be rejected by the actual joined checker.
    assert_eq!(queries[0], 0);
    for round in 0..5 {
        assert_eq!(proof.rounds[round].groups[0].index, 0);
        let correct = proof.rounds[round].groups[0].values.clone();
        for coordinate in 0..FRI_ARITIES[round] {
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
            let x = mul_mod(domains[round].point(coordinate * FRI_LENGTHS[round + 1]), 2);
            *value = if round == 0 {
                initial_at(x)
            } else {
                horner(&coefficients[round], x)
            };
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
        320
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
    let relation = policy_relation();
    let bytes = vec![0; deep_proof::MAX_FRAME_BYTES];
    let exact = VerifyLimits {
        // The enclosing public bundle checks its transition table, before any
        // segment is constructed. This private helper has no transition table.
        max_transitions: 0,
        max_batch_bytes: relation.statement_bytes().len(),
        max_proof_bytes: bytes.len(),
        max_fri_layers: 6,
        max_queries: 64,
        max_query_chunk_values: 2,
        max_query_path_len: 23,
        max_fri_round_values: 16,
        max_air_row_values: COMMITTED_COLUMN_COUNT,
    };
    preflight(&relation, bytes.len(), exact).unwrap();
    type Setter = fn(&mut VerifyLimits, usize);
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
        ("max_queries", 64, |l, v| l.max_queries = v),
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
