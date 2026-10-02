//! Full-tree parity, original-coordinate rejection and clearing custody.

use super::*;
use std::cell::RefCell;

thread_local! {
    static CUT_CLEARS: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
}
pub(super) fn observe_cut_clear_v1(roots: &[PrivacyOuterDigestV1]) {
    CUT_CLEARS.with(|slot| {
        if let Some(sizes) = slot.borrow_mut().as_mut() {
            assert!(
                roots
                    .iter()
                    .all(|root| *root == PrivacyOuterDigestV1::default())
            );
            sizes.push(roots.len());
        }
    });
}
fn context() -> TransparentStarkDigestContextV1 {
    TransparentStarkDigestContextV1::new(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        b"retained-cut-reference",
    )
}
const LEAF: &[u8] = b"retained-cut-leaf";
const NODE: &[u8] = b"retained-cut-node";
fn columns(rows: usize) -> Vec<Vec<F>> {
    (0..9)
        .map(|column| {
            (0..rows)
                .map(|row| F((column * 10000 + row * 17 + 1) as u64))
                .collect()
        })
        .collect()
}
fn full(columns: &[Vec<F>], indices: &[usize]) -> StreamingRowCommitmentV1 {
    let mut out = StreamingRowCommitmentV1::new(
        context(),
        LEAF,
        NODE,
        usize::from(u16::MAX),
        columns[0].len(),
        columns.len(),
        indices,
    )
    .unwrap();
    for batch in columns.chunks(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
        out.absorb_columns_v1(batch).unwrap();
    }
    out
}
fn selected<'a>(
    columns: &[Vec<F>],
    indices: &[usize],
    cut: &'a RetainedMerkleCutV1,
) -> SelectedRowCommitmentV1<'a> {
    let mut out = SelectedRowCommitmentV1::new_v1(
        context(),
        LEAF,
        NODE,
        usize::from(u16::MAX),
        columns[0].len(),
        columns.len(),
        indices,
        cut,
    )
    .unwrap();
    for batch in columns.chunks(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
        out.absorb_columns_v1(batch).unwrap();
    }
    out
}

#[test]
fn every_singleton_boundary_shared_cut_and_full_set_matches_original_tree_and_frontier() {
    for rows in [16, 32, 64, 128] {
        let values = columns(rows);
        let (initial, cut) = full(&values, &[]).finish_retaining_cut_v1().unwrap();
        assert_eq!(cut.roots.len(), rows / CUT_ROWS_V1);
        assert_eq!(initial, full(&values, &[]).finish().unwrap());
        let mut selections = (0..rows).map(|row| vec![row]).collect::<Vec<_>>();
        selections.extend([vec![0, 1, 15], vec![0, rows - 1], (0..rows).collect()]);
        if rows > 16 {
            selections.push(vec![14, 15, 16, 17]);
        }
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            for indices in &selections {
                let expected = full(&values, indices).finish().unwrap();
                let actual = pool
                    .install(|| selected(&values, indices, &cut).finish_v1())
                    .unwrap();
                assert_eq!(
                    actual, expected,
                    "rows={rows} indices={indices:?} workers={workers}"
                );
            }
        }
    }
}

#[test]
fn corrupted_selected_or_unselected_original_cut_and_original_root_fail_closed() {
    let values = columns(64);
    for mutation in 0..4 {
        let (_, mut cut) = full(&values, &[]).finish_retaining_cut_v1().unwrap();
        match mutation {
            0 => cut.roots[0] = PrivacyOuterDigestV1::default(),
            1 => cut.roots[3] = PrivacyOuterDigestV1::default(),
            2 => cut.roots.swap(0, 1),
            3 => cut.root = PrivacyOuterDigestV1::default(),
            _ => unreachable!(),
        }
        assert!(selected(&values, &[0, 1], &cut).finish_v1().is_err());
    }
}

#[test]
fn changed_selected_source_global_index_group_width_and_phase_roles_fail_closed() {
    let values = columns(64);
    let (_, cut) = full(&values, &[]).finish_retaining_cut_v1().unwrap();
    for mutation in 0..6 {
        let mut changed = values.clone();
        let (leaf, node, group, width) = match mutation {
            0 => {
                changed[0][17] = changed[0][17].add(F::ONE);
                (LEAF, NODE, usize::from(u16::MAX), 9)
            }
            1 => (
                b"wrong-phase-leaf".as_slice(),
                NODE,
                usize::from(u16::MAX),
                9,
            ),
            2 => (
                LEAF,
                b"wrong-phase-node".as_slice(),
                usize::from(u16::MAX),
                9,
            ),
            3 => (LEAF, NODE, 0, 9),
            4 => (LEAF, NODE, usize::from(u16::MAX), 8),
            5 => {
                changed.iter_mut().for_each(|column| column.swap(17, 1));
                (LEAF, NODE, usize::from(u16::MAX), 9)
            }
            _ => unreachable!(),
        };
        let mut out =
            SelectedRowCommitmentV1::new_v1(context(), leaf, node, group, 64, width, &[17], &cut)
                .unwrap();
        for batch in changed[..width].chunks(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            out.absorb_columns_v1(batch).unwrap();
        }
        assert!(out.finish_v1().is_err(), "mutation={mutation}");
    }
}

#[test]
fn malformed_query_order_incomplete_width_and_noncanonical_unselected_values_are_rejected() {
    let mut values = columns(64);
    let (_, cut) = full(&values, &[]).finish_retaining_cut_v1().unwrap();
    for indices in [vec![], vec![1, 1], vec![2, 1], vec![64]] {
        assert!(
            SelectedRowCommitmentV1::new_v1(context(), LEAF, NODE, 0, 64, 9, &indices, &cut)
                .is_err()
        );
    }
    let mut out = SelectedRowCommitmentV1::new_v1(
        context(),
        LEAF,
        NODE,
        usize::from(u16::MAX),
        64,
        9,
        &[0],
        &cut,
    )
    .unwrap();
    out.absorb_columns_v1(&values[..8]).unwrap();
    assert!(out.finish_v1().is_err());
    values[0][63] = F(u64::MAX);
    let mut out = SelectedRowCommitmentV1::new_v1(
        context(),
        LEAF,
        NODE,
        usize::from(u16::MAX),
        64,
        9,
        &[0],
        &cut,
    )
    .unwrap();
    assert_eq!(
        out.absorb_columns_v1(&values[..8]),
        Err(AggregateStarkErrorV1::NonCanonicalField)
    );
    for rows in [0, 1, 8, 17, usize::MAX] {
        assert!(RetainedMerkleCutV1::payload_bound_v1(rows).is_err());
    }
    assert_eq!(
        RetainedMerkleCutV1::payload_bound_v1(64).unwrap(),
        4 * core::mem::size_of::<PrivacyOuterDigestV1>()
            + core::mem::size_of::<RetainedMerkleCutV1>()
    );
}

#[test]
fn partial_complete_and_unwound_cuts_erase_original_backings() {
    CUT_CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
    {
        let mut partial = RetainedMerkleCutV1::new_v1(64).unwrap();
        partial
            .capture_v1(0, &[PrivacyOuterDigestV1::from_bytes([11; 48])])
            .unwrap();
        assert!(
            partial
                .capture_v1(0, &[PrivacyOuterDigestV1::default()])
                .is_err()
        );
        assert!(
            partial
                .bind_root_v1(PrivacyOuterDigestV1::default())
                .is_err()
        );
    }
    assert!(
        std::panic::catch_unwind(|| {
            let values = columns(64);
            let (_, _cut) = full(&values, &[]).finish_retaining_cut_v1().unwrap();
            panic!("injected cut unwind");
        })
        .is_err()
    );
    let observed = CUT_CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
    assert_eq!(observed, vec![1, 4]);
}
