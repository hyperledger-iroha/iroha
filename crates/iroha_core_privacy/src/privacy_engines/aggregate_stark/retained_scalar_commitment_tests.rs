//! Original-coordinate parity, complete cut authentication, and clearing custody.

use super::*;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

thread_local! {
    static CLEARS: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
}
pub(super) fn observe_clear_v1(roots: &[PrivacyOuterDigestV1], root: PrivacyOuterDigestV1) {
    CLEARS.with(|slot| {
        if let Some(clears) = slot.borrow_mut().as_mut() {
            assert_eq!(root, PrivacyOuterDigestV1::default());
            assert!(
                roots
                    .iter()
                    .all(|node| *node == PrivacyOuterDigestV1::default())
            );
            clears.push(roots.len());
        }
    });
}
struct Census;
impl Census {
    fn begin() -> Self {
        CLEARS.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(Vec::new());
        });
        Self
    }
    fn finish(self) -> Vec<usize> {
        CLEARS.with(|slot| slot.borrow_mut().take().unwrap())
    }
}
impl Drop for Census {
    fn drop(&mut self) {
        CLEARS.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
fn context() -> TransparentStarkDigestContextV1 {
    TransparentStarkDigestContextV1::new(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        b"retained-scalar-reference",
    )
}
const NODE: &[u8] = b"retained-scalar-node";
fn digest(index: usize) -> PrivacyOuterDigestV1 {
    let mut bytes = [17; 48];
    bytes[..8].copy_from_slice(&(index as u64).to_be_bytes());
    PrivacyOuterDigestV1::from_bytes(bytes)
}
fn mutate(value: PrivacyOuterDigestV1) -> PrivacyOuterDigestV1 {
    let mut bytes = *value.as_bytes();
    bytes[0] ^= 1;
    PrivacyOuterDigestV1::from_bytes(bytes)
}

#[test]
fn retained_scalar_cuts_preserve_all_frontier_coordinates_and_skip_unselected_leaves() {
    for rows in [1, 32, CUT_ROWS_V1, 4 * CUT_ROWS_V1] {
        let tree =
            PrivacyOuterMerkleTreeV1::from_leaves((0..rows).map(digest).collect(), context(), NODE)
                .unwrap();
        let (original, cut) =
            commit_v1(context(), NODE, rows, &[], true, |index| Ok(digest(index))).unwrap();
        assert_eq!(original.root, tree.root());
        assert_eq!(cut.is_some(), rows >= CUT_ROWS_V1);
        let mut boundary = vec![0, rows - 1];
        if rows > CUT_ROWS_V1 {
            boundary.extend([CUT_ROWS_V1 - 1, CUT_ROWS_V1, CUT_ROWS_V1 + 1]);
        }
        boundary.sort_unstable();
        boundary.dedup();
        for indices in [Vec::new(), vec![0], boundary, (0..rows).collect()] {
            let calls = AtomicUsize::new(0);
            let opened = open_v1(
                context(),
                NODE,
                rows,
                &indices,
                original.root,
                cut.as_ref(),
                |index| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    Ok(digest(index))
                },
            )
            .unwrap();
            assert_eq!(opened.root, original.root);
            let expected_frontier = if indices.is_empty() {
                Vec::new()
            } else {
                canonical_multiproof_frontier_v1(&tree, rows, &indices).unwrap()
            };
            assert_eq!(opened.frontier, expected_frontier);
            let expected = if cut.is_some() {
                indices
                    .iter()
                    .map(|index| index / CUT_ROWS_V1)
                    .collect::<BTreeSet<_>>()
                    .len()
                    * CUT_ROWS_V1
            } else {
                rows
            };
            assert_eq!(calls.load(Ordering::Relaxed), expected);
        }
    }
}

#[test]
fn retained_scalar_replay_rejects_every_cut_mutation_and_selected_source_substitution() {
    let rows = 4 * CUT_ROWS_V1;
    let (original, mut cut) =
        commit_v1(context(), NODE, rows, &[], true, |index| Ok(digest(index))).unwrap();
    let cut = cut.as_mut().unwrap();
    for position in 0..cut.roots.len() {
        let saved = cut.roots[position];
        cut.roots[position] = mutate(saved);
        assert!(
            open_v1(
                context(),
                NODE,
                rows,
                &[0],
                original.root,
                Some(cut),
                |index| Ok(digest(index))
            )
            .is_err()
        );
        // Root-only validation also authenticates every retained subtree.
        assert!(
            open_v1(
                context(),
                NODE,
                rows,
                &[],
                original.root,
                Some(cut),
                |index| Ok(digest(index))
            )
            .is_err()
        );
        cut.roots[position] = saved;
    }
    assert!(
        open_v1(
            context(),
            NODE,
            rows,
            &[0],
            original.root,
            Some(cut),
            |index| Ok(if index == 1 {
                digest(19)
            } else {
                digest(index)
            })
        )
        .is_err()
    );
    assert!(
        open_v1(
            context(),
            b"different-node",
            rows,
            &[0],
            original.root,
            Some(cut),
            |index| Ok(digest(index))
        )
        .is_err()
    );
    assert!(
        open_v1(
            context(),
            NODE,
            rows,
            &[0],
            mutate(original.root),
            Some(cut),
            |index| Ok(digest(index))
        )
        .is_err()
    );
    for indices in [vec![1, 0], vec![1, 1], vec![rows]] {
        assert!(
            open_v1(
                context(),
                NODE,
                rows,
                &indices,
                original.root,
                Some(cut),
                |index| Ok(digest(index))
            )
            .is_err()
        );
    }
    assert!(
        open_v1(
            context(),
            NODE,
            rows,
            &[0],
            original.root,
            None,
            |index| Ok(digest(index))
        )
        .is_err()
    );
    let (small, _) = commit_v1(context(), NODE, 32, &[], true, |index| Ok(digest(index))).unwrap();
    assert!(
        open_v1(
            context(),
            NODE,
            32,
            &[0],
            mutate(small.root),
            None,
            |index| Ok(digest(index))
        )
        .is_err()
    );
}

#[test]
fn retained_scalar_owners_clear_on_success_failure_and_unwind() {
    let census = Census::begin();
    let (_, cut) = commit_v1(context(), NODE, 2 * CUT_ROWS_V1, &[], true, |index| {
        Ok(digest(index))
    })
    .unwrap();
    assert_eq!(
        cut.as_ref().unwrap().allocated_payload_bytes_v1().unwrap(),
        2 * core::mem::size_of::<PrivacyOuterDigestV1>()
    );
    drop(cut);
    assert!(
        commit_v1(context(), NODE, 2 * CUT_ROWS_V1, &[], true, |index| {
            if index == CUT_ROWS_V1 {
                Err(AggregateStarkErrorV1::NonCanonicalField)
            } else {
                Ok(digest(index))
            }
        })
        .is_err()
    );
    let panic = std::panic::catch_unwind(|| {
        let (_, _cut) = commit_v1(context(), NODE, 2 * CUT_ROWS_V1, &[], true, |index| {
            Ok(digest(index))
        })
        .unwrap();
        panic!("test retained custody unwind");
    });
    assert!(panic.is_err());
    assert_eq!(census.finish(), vec![2, 1, 2]);
    assert!(RetainedScalarCutV1::new_v1(CUT_ROWS_V1 - 1).is_err());
    assert!(RetainedScalarCutV1::allocation_bound_v1(0).is_err());
    assert!(RetainedScalarCutV1::allocation_bound_v1(3).is_err());
}

fn parameters(commitment_layout: AggregateFriCommitmentLayoutV1) -> AggregateStarkParametersV1 {
    AggregateStarkParametersV1 {
        security_lanes: 1,
        minimum_trace_log2: 11,
        maximum_trace_log2: 11,
        fri_commitment_layout: commitment_layout,
        ..super::super::tests::PARAMETERS
    }
}
fn layout(parameters: AggregateStarkParametersV1) -> AggregateProofLayoutV1 {
    AggregateProofLayoutV1::new(
        parameters,
        vec![AggregateTraceGroupLayoutV1 {
            native_trace_log2: 11,
            segment_instances: 1,
            base_width: 1,
            aux_width: 1,
        }],
    )
    .unwrap()
}
fn evaluations(layout: &AggregateProofLayoutV1) -> Vec<E> {
    let root = goldilocks_primitive_root_v1(layout.common_lde_log2).unwrap();
    let mut point = F(GOLDILOCKS_GENERATOR_V1);
    (0..layout.common_lde_size())
        .map(|_| {
            let value = E::from_base(F(5).add(F(3).mul(point)).add(F(2).mul(point.mul(point))));
            point = point.mul(root);
            value
        })
        .collect()
}

#[test]
fn retained_composition_and_fri_match_original_roots_transcript_queries_and_frontiers() {
    let domains = &super::super::tests::DOMAINS;
    for mode in [
        AggregateFriCommitmentLayoutV1::Scalar,
        AggregateFriCommitmentLayoutV1::Paired,
    ] {
        let parameters = parameters(mode);
        let layout = layout(parameters);
        let values = evaluations(&layout);
        let indices = (0..parameters.query_count)
            .map(|index| index * 17)
            .collect::<Vec<_>>();
        let chunks = vec![
            values.clone(),
            values.iter().map(|value| value.add(E::ONE)).collect(),
        ];
        let (commitment, cut) = retain_composition_commitment_v1(domains, 0, &chunks).unwrap();
        let tree = composition_tree_v1(domains, 0, &chunks).unwrap();
        assert_eq!(commitment.root, tree.root());
        let opened = open_retained_composition_v1(
            domains,
            0,
            &chunks,
            &indices,
            commitment.root,
            cut.as_ref(),
        )
        .unwrap();
        assert_eq!(
            opened.frontier,
            canonical_multiproof_frontier_v1(&tree, values.len(), &indices).unwrap()
        );
        let mut original_transcript = super::super::tests::transcript();
        let original = build_streaming_fri_lane_v1(
            parameters,
            domains,
            &layout,
            0,
            values.clone(),
            &mut original_transcript,
        )
        .unwrap();
        let mut retained_transcript = super::super::tests::transcript();
        let retained = build_retained_streaming_fri_lane_v1(
            parameters,
            domains,
            &layout,
            0,
            values.clone(),
            &mut retained_transcript,
        )
        .unwrap();
        let mut materialized_transcript = super::super::tests::transcript();
        let materialized = build_fri_lane_v1(
            parameters,
            domains,
            &layout,
            0,
            values.clone(),
            &mut materialized_transcript,
        )
        .unwrap();
        assert_eq!(original.roots, materialized.roots);
        assert_eq!(original.terminal_values, materialized.terminal_values);
        assert_eq!(original_transcript.state(), materialized_transcript.state());
        assert_eq!(original, retained.material);
        assert_eq!(original_transcript.state(), retained_transcript.state());
        assert!(retained.cuts.iter().any(Option::is_some));
        assert!(retained.cuts.iter().any(Option::is_none));
        assert_eq!(
            open_streaming_fri_lane_v1(
                parameters,
                domains,
                &layout,
                0,
                values.clone(),
                &original,
                &indices
            )
            .unwrap(),
            open_retained_streaming_fri_lane_v1(
                parameters,
                domains,
                &layout,
                0,
                values.clone(),
                &retained,
                &indices
            )
            .unwrap()
        );
        let composition = vec![cut];
        let fri = vec![retained];
        assert_eq!(
            oracle_payload_bytes_v1(&composition, &fri).unwrap(),
            oracle_payload_bound_v1(parameters, &layout).unwrap()
        );
        let mut changed = chunks;
        changed[0][values.len() - 1] =
            E::from_raw_coefficients_for_testing([F(u64::MAX), F::ZERO, F::ZERO, F::ZERO]);
        assert_eq!(
            open_retained_composition_v1(
                domains,
                0,
                &changed,
                &indices,
                commitment.root,
                composition[0].as_ref()
            )
            .unwrap_err(),
            AggregateStarkErrorV1::NonCanonicalField
        );
    }
}

#[test]
fn retained_fri_replay_rejects_cut_beta_terminal_and_query_tampering() {
    let domains = &super::super::tests::DOMAINS;
    let parameters = parameters(AggregateFriCommitmentLayoutV1::Paired);
    let layout = layout(parameters);
    let values = evaluations(&layout);
    let indices = (0..parameters.query_count).collect::<Vec<_>>();
    let mut transcript = super::super::tests::transcript();
    let mut retained = build_retained_streaming_fri_lane_v1(
        parameters,
        domains,
        &layout,
        0,
        values.clone(),
        &mut transcript,
    )
    .unwrap();
    let open = |retained: &AggregateRetainedStreamingFriLaneV1| {
        open_retained_streaming_fri_lane_v1(
            parameters,
            domains,
            &layout,
            0,
            values.clone(),
            retained,
            &indices,
        )
    };
    for round in 0..retained.cuts.len() {
        let count = retained.cuts[round]
            .as_ref()
            .map_or(0, |cut| cut.roots.len());
        for index in 0..count {
            let saved = retained.cuts[round].as_ref().unwrap().roots[index];
            retained.cuts[round].as_mut().unwrap().roots[index] = mutate(saved);
            assert!(open(&retained).is_err());
            retained.cuts[round].as_mut().unwrap().roots[index] = saved;
        }
    }
    let beta = retained.material.betas[0];
    retained.material.betas[0] = beta.add(E::ONE);
    assert!(open(&retained).is_err());
    retained.material.betas[0] = beta;
    let terminal = retained.material.terminal_values[0];
    retained.material.terminal_values[0] = terminal.add(E::ONE);
    assert!(open(&retained).is_err());
    retained.material.terminal_values[0] = terminal;
    let removed = retained.cuts.pop().unwrap();
    assert!(open(&retained).is_err());
    retained.cuts.push(removed);
    let mut bad_indices = indices.clone();
    bad_indices[1] = bad_indices[0];
    assert!(
        open_retained_streaming_fri_lane_v1(
            parameters,
            domains,
            &layout,
            0,
            values.clone(),
            &retained,
            &bad_indices
        )
        .is_err()
    );
    let mut changed = values;
    changed[1] = changed[1].add(E::ONE);
    assert!(
        open_retained_streaming_fri_lane_v1(
            parameters, domains, &layout, 0, changed, &retained, &indices
        )
        .is_err()
    );
}
