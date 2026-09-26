//! Exact reduced-row codec, ordered physical roots and fail-closed relation tests.

use super::tests::{
    DOMAINS, PARAMETERS, ZeroEvaluator, deep_fixture, fixture_with_parameters, transcript,
};
use super::*;

fn fixture(
    trace_layout: AggregateTraceLayoutV1,
    fri_layout: AggregateFriCommitmentLayoutV1,
) -> (
    AggregateProofLayoutV1,
    AggregateStarkProofV1,
    AggregateStarkParametersV1,
) {
    let parameters = AggregateStarkParametersV1 {
        fri_commitment_layout: fri_layout,
        ..PARAMETERS
    };
    let (_, mut proof, _, _) = fixture_with_parameters(parameters);
    let layout = AggregateProofLayoutV1::new_with_trace_layout_v1(
        parameters,
        vec![
            AggregateTraceGroupLayoutV1 {
                native_trace_log2: 8,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            },
            AggregateTraceGroupLayoutV1 {
                native_trace_log2: 8,
                segment_instances: 1,
                base_width: 2,
                aux_width: 2,
            },
        ],
        trace_layout,
    )
    .unwrap();
    for query in &mut proof.queries {
        let index = u64::from(query.index);
        query.trace_groups[0].base_next.clear();
        query.trace_groups[0].aux_next.clear();
        query.trace_groups.push(AggregateTraceGroupQueryV1 {
            base_current: vec![index + 10, index + 20],
            base_next: Vec::new(),
            aux_current: vec![index + 30, index + 40],
            aux_next: Vec::new(),
        });
    }
    let rows = layout.common_lde_size();
    let indices = composition_opening_indices_v1(&proof.queries, &layout).unwrap();
    proof.trace_groups.clear();
    for physical in 0..layout.trace_commitment_count_v1() {
        let tree = |base| {
            let leaves = (0..rows)
                .map(|index| {
                    let index_field = F(index as u64);
                    let mut values = vec![if base {
                        index_field
                    } else {
                        index_field.mul(F(3))
                    }];
                    let offset = if base { 10 } else { 30 };
                    let second = [F(index as u64 + offset), F(index as u64 + offset + 10)];
                    let group = if trace_layout == AggregateTraceLayoutV1::JoinedCurrent {
                        values.extend(second);
                        JOINED_TRACE_GROUP_MARKER_V1
                    } else {
                        if physical == 1 {
                            values = second.to_vec();
                        }
                        physical
                    };
                    row_leaf_hash_v1(
                        DOMAINS.digest_context,
                        if base {
                            DOMAINS.base_leaf
                        } else {
                            DOMAINS.aux_leaf
                        },
                        group,
                        index,
                        &values,
                    )
                    .unwrap()
                })
                .collect();
            PrivacyOuterMerkleTreeV1::from_leaves(
                leaves,
                DOMAINS.digest_context,
                if base {
                    DOMAINS.base_node
                } else {
                    DOMAINS.aux_node
                },
            )
            .unwrap()
        };
        let base = tree(true);
        let aux = tree(false);
        proof.trace_groups.push(AggregateTraceGroupProofV1 {
            base_root: base.root(),
            aux_root: aux.root(),
            base_frontier: canonical_multiproof_frontier_v1(&base, rows, &indices).unwrap(),
            aux_frontier: canonical_multiproof_frontier_v1(&aux, rows, &indices).unwrap(),
        });
    }
    // Composition/FRI fixture leaves remain authenticated; this fixture does
    // not claim a valid relation or transcript for the substituted trace.
    (layout, proof, parameters)
}

#[test]
fn reduced_trace_codec_authenticates_exact_roots_slices_and_full_deep_payload() {
    for trace_layout in [
        AggregateTraceLayoutV1::GroupedCurrent,
        AggregateTraceLayoutV1::JoinedCurrent,
    ] {
        for fri_layout in [
            AggregateFriCommitmentLayoutV1::Scalar,
            AggregateFriCommitmentLayoutV1::Paired,
        ] {
            let (layout, proof, parameters) = fixture(trace_layout, fri_layout);
            let indices = proof
                .queries
                .iter()
                .map(|query| query.index as usize)
                .collect::<Vec<_>>();
            verify_all_merkle_openings_v1(&proof, parameters, DOMAINS, &layout, &indices).unwrap();
            let deep = deep_fixture(&layout);
            let encoded = encode_proof_with_deep_v1(&proof, &deep, parameters, &layout).unwrap();
            assert_eq!(
                encoded.len(),
                maximum_encoded_proof_with_deep_bytes_v1(parameters, &layout).unwrap()
            );
            assert_eq!(
                decode_proof_with_deep_v1(&encoded, parameters, &layout).unwrap(),
                (proof.clone(), deep.clone())
            );
            let grouped =
                AggregateProofLayoutV1::new(parameters, layout.trace_groups.clone()).unwrap();
            assert_eq!(
                exact_deep_opening_bytes_v1(parameters, &layout).unwrap(),
                exact_deep_opening_bytes_v1(parameters, &grouped).unwrap()
            );
            assert!(decode_proof_with_deep_v1(&encoded, parameters, &grouped).is_err());
            assert_eq!(
                verify_opened_query_relations_v1(
                    &proof,
                    parameters,
                    &layout,
                    &indices,
                    &[],
                    &[],
                    &mut ZeroEvaluator
                ),
                Err(AggregateStarkErrorV1::ConstraintOpening)
            );
            assert_eq!(
                verify_opened_query_relations_with_deep_v1(
                    &proof,
                    &deep,
                    E::ONE,
                    &[],
                    parameters,
                    &layout,
                    &indices,
                    &[],
                    &[],
                    &mut ZeroEvaluator
                ),
                Err(AggregateStarkErrorV1::ConstraintOpening)
            );
            for field in 0..4 {
                let mut changed = proof.clone();
                if field < 2 {
                    changed.queries[0].trace_groups[field].base_current[0] += 1;
                } else {
                    changed.queries[0].trace_groups[field - 2].aux_current[0] += 1;
                }
                assert!(
                    verify_all_merkle_openings_v1(&changed, parameters, DOMAINS, &layout, &indices)
                        .is_err()
                );
            }
            let mut changed = proof.clone();
            changed.queries[0].trace_groups[0].base_next.push(0);
            assert!(encode_proof_with_deep_v1(&changed, &deep, parameters, &layout).is_err());
            let mut omitted = deep.clone();
            omitted.trace_groups[0].base_next.clear();
            assert!(encode_proof_with_deep_v1(&proof, &omitted, parameters, &layout).is_err());
            let mut extra_root = proof.clone();
            extra_root.trace_groups.push(proof.trace_groups[0].clone());
            assert!(encode_proof_with_deep_v1(&extra_root, &deep, parameters, &layout).is_err());
            for length in [0, 7, encoded.len() - 1] {
                assert!(
                    decode_proof_with_deep_v1(&encoded[..length], parameters, &layout).is_err()
                );
            }
            let mut trailing = encoded;
            trailing.push(0);
            assert!(decode_proof_with_deep_v1(&trailing, parameters, &layout).is_err());
        }
    }
}

#[test]
fn trace_layout_is_bound_before_roots_and_joined_scalar_savings_are_exact() {
    let (current, proof, parameters) = fixture(
        AggregateTraceLayoutV1::GroupedCurrent,
        AggregateFriCommitmentLayoutV1::Scalar,
    );
    let joined = AggregateProofLayoutV1::new_with_trace_layout_v1(
        parameters,
        current.trace_groups.clone(),
        AggregateTraceLayoutV1::JoinedCurrent,
    )
    .unwrap();
    let full = AggregateProofLayoutV1::new(parameters, current.trace_groups.clone()).unwrap();
    let state = |layout| {
        let mut transcript = transcript();
        absorb_layout_v1(
            &mut transcript,
            parameters,
            DOMAINS,
            b"layout-separation-test",
            layout,
        )
        .unwrap();
        transcript.state()
    };
    assert_ne!(state(&full), state(&current));
    assert_ne!(state(&current), state(&joined));
    let before = maximum_encoded_proof_with_deep_bytes_v1(parameters, &full).unwrap();
    let middle = maximum_encoded_proof_with_deep_bytes_v1(parameters, &current).unwrap();
    let after = maximum_encoded_proof_with_deep_bytes_v1(parameters, &joined).unwrap();
    let q = parameters.query_count;
    let frontier =
        |opened| maximum_multiproof_frontier_len_v1(full.common_lde_size(), opened).unwrap();
    assert_eq!(
        before - middle,
        q * 6 * 8 + 4 * (frontier(2 * q) - frontier(q)) * PRIVACY_OUTER_DIGEST_BYTES_V1
    );
    assert_eq!(
        middle - after,
        2 * (frontier(q) + 1) * PRIVACY_OUTER_DIGEST_BYTES_V1
    );
    // Scalar FRI continues disclosing both pair members; trace compression
    // must not accidentally halve its frontier accounting.
    let indices = proof
        .queries
        .iter()
        .map(|query| query.index as usize)
        .collect::<Vec<_>>();
    verify_all_merkle_openings_v1(&proof, parameters, DOMAINS, &current, &indices).unwrap();
}
