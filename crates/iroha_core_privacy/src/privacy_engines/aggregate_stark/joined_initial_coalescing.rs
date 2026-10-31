//! Initial-only coalescing of two completed eight-column evaluation owners.

use super::super::initial_evaluated_column::{InitialEvaluationV1, drop_completed_v1};
use super::*;

const COLUMNS: usize = 2 * MASKED_TRACE_LDE_COLUMN_BATCH_V1;
const ROWS_PER_PACKING_TASK: usize = 1024;

/// Additional simultaneous output and metadata beyond the ordinary replay batch.
pub(super) fn extra_payload_v1(rows: usize) -> Result<usize, AggregateStarkErrorV1> {
    if rows == 0 || !rows.is_power_of_two() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let additional_output = rows
        .checked_mul(MASKED_TRACE_LDE_COLUMN_BATCH_V1)
        .and_then(|cells| cells.checked_mul(core::mem::size_of::<F>()));
    let metadata = core::mem::size_of::<InitialCoalescerV1<ZeroizingFieldColumnV1>>()
        + COLUMNS * core::mem::size_of::<ZeroizingFieldColumnV1>()
        + 3 * core::mem::size_of::<Vec<ZeroizingFieldColumnV1>>();
    // Conservatively charge all 256-byte paired row-packing tasks, independently of
    // worker count, although only active workers hold their stack scratch.
    additional_output
        .and_then(|bytes| {
            rows.div_ceil(ROWS_PER_PACKING_TASK)
                .checked_mul(256)
                .and_then(|packing| bytes.checked_add(packing))
        })
        .and_then(|bytes| bytes.checked_add(metadata))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Contains at most eight pending outputs between source callbacks. A second
/// completed batch moves into the same descriptor list before atomic absorption.
pub(super) struct InitialCoalescerV1<C = ZeroizingFieldColumnV1> {
    rows: usize,
    pending: Vec<C>,
    failed: bool,
}

impl<C: InitialEvaluationV1> InitialCoalescerV1<C> {
    pub(super) fn new_v1(rows: usize) -> Result<Self, AggregateStarkErrorV1> {
        extra_payload_v1(rows)?;
        Ok(Self {
            rows,
            pending: Vec::new(),
            failed: false,
        })
    }

    pub(super) fn push_v1(
        &mut self,
        evaluations: Vec<C>,
        mut absorb: impl FnMut(&[C]) -> Result<(), AggregateStarkErrorV1>,
    ) -> Result<(), AggregateStarkErrorV1> {
        // Local custody makes every refusal/unwind immediately erase previously
        // retained outputs as well as this newly transferred batch.
        let mut pending = core::mem::take(&mut self.pending);
        let previously_failed = self.failed;
        self.failed = true;
        if previously_failed
            || evaluations.is_empty()
            || evaluations.len() > MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || pending.len() > MASKED_TRACE_LDE_COLUMN_BATCH_V1
            || evaluations
                .iter()
                .any(|column| !column.exact_initial_extent_v1(self.rows))
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        // Preserve the original callback boundary: a late-invalid word in the
        // first eight outputs must refuse before another source is invoked.
        // The initial streaming builder keeps its independent preflight too.
        let invalid = |column: &C| !canonical_field_column_v1(column.as_ref());
        if if self.rows >= 1 << 22 {
            evaluations.par_iter().any(invalid)
        } else {
            evaluations.iter().any(invalid)
        } {
            return Err(AggregateStarkErrorV1::NonCanonicalField);
        }
        pending
            .try_reserve_exact(COLUMNS - pending.len())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if pending.capacity() != COLUMNS {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        // Moves each clearing output owner without reading or copying any private field.
        pending.extend(evaluations);
        if pending.len() == COLUMNS {
            absorb(&pending)?;
            drop_completed_v1(pending);
        } else {
            self.pending = pending;
        }
        self.failed = false;
        Ok(())
    }

    /// Flush a public group boundary, including an odd or short final batch.
    pub(super) fn flush_v1(
        &mut self,
        mut absorb: impl FnMut(&[C]) -> Result<(), AggregateStarkErrorV1>,
    ) -> Result<(), AggregateStarkErrorV1> {
        let pending = core::mem::take(&mut self.pending);
        let previously_failed = self.failed;
        self.failed = true;
        if previously_failed {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if !pending.is_empty() {
            absorb(&pending)?;
            drop_completed_v1(pending);
        }
        self.failed = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::aggregate_stark::field_column_erasure;

    fn columns(first: usize, count: usize, rows: usize) -> Vec<ZeroizingFieldColumnV1> {
        (first..first + count)
            .map(|column| {
                let mut values = Vec::new();
                values.try_reserve_exact(rows).unwrap();
                assert_eq!(values.capacity(), rows);
                values.extend((0..rows).map(|row| F((1 + column * rows + row) as u64)));
                ZeroizingFieldColumnV1(values)
            })
            .collect()
    }

    #[test]
    fn initial_coalescing_moves_exact_owners_in_order_and_flushes_odd_groups() {
        let mut owner = InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(4).unwrap();
        let mut seen = Vec::new();
        let mut batches = Vec::new();
        let mut expected = Vec::new();
        for (start, width) in [(0, 25), (25, 17), (42, 3)] {
            for first in (start..start + width).step_by(8) {
                let values = columns(first, 8.min(start + width - first), 4);
                expected.extend(values.iter().map(|column| column.as_ptr() as usize));
                owner
                    .push_v1(values, |values| {
                        batches.push(values.len());
                        seen.extend(values.iter().map(|column| column.as_ptr() as usize));
                        Ok(())
                    })
                    .unwrap();
            }
            owner
                .flush_v1(|values| {
                    batches.push(values.len());
                    seen.extend(values.iter().map(|column| column.as_ptr() as usize));
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(seen, expected);
        assert_eq!(batches, [16, 9, 16, 1, 3]);
        assert!(owner.pending.is_empty());
    }

    #[test]
    fn initial_coalescing_preserves_complete_streaming_root_and_opened_rows() {
        const ROWS: usize = 32;
        const WIDTH: usize = 45;
        let context = TransparentStarkDigestContextV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            b"coalescing-test-v1",
        );
        let make = || {
            StreamingRowCommitmentV1::new(
                context,
                b"coalescing-leaf",
                b"coalescing-node",
                JOINED_TRACE_GROUP_MARKER_V1,
                ROWS,
                WIDTH,
                &[0, ROWS - 1],
            )
            .unwrap()
        };
        let mut original = make();
        let mut combined = make();
        let mut owner = InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(ROWS).unwrap();
        for (start, width) in [(0, 25), (25, 17), (42, 3)] {
            for first in (start..start + width).step_by(8) {
                let count = 8.min(start + width - first);
                original
                    .absorb_columns_v1(&columns(first, count, ROWS))
                    .unwrap();
                owner
                    .push_v1(columns(first, count, ROWS), |values| {
                        combined.absorb_initial_columns_v1(values)
                    })
                    .unwrap();
            }
            owner
                .flush_v1(|values| combined.absorb_initial_columns_v1(values))
                .unwrap();
        }
        assert_eq!(original.finish().unwrap(), combined.finish().unwrap());
    }

    #[test]
    fn initial_stream_refuses_oversized_or_noncanonical_batches_before_any_hash_mutation() {
        const ROWS: usize = 4;
        let context = TransparentStarkDigestContextV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            b"coalescing-refusal-v1",
        );
        let make = || {
            StreamingRowCommitmentV1::new(
                context,
                b"coalescing-leaf",
                b"coalescing-node",
                JOINED_TRACE_GROUP_MARKER_V1,
                ROWS,
                17,
                &[],
            )
            .unwrap()
        };
        let mut checked = make();
        assert_eq!(
            checked.absorb_initial_columns_v1(&columns(0, 17, ROWS)),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        let mut invalid = columns(0, 16, ROWS);
        invalid[15].0[ROWS - 1] =
            F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            checked.absorb_initial_columns_v1(&invalid),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );
        assert_eq!(checked.received_columns, 0);
        assert!(!checked.failed);
        checked
            .absorb_initial_columns_v1(&columns(0, 16, ROWS))
            .unwrap();
        checked
            .absorb_initial_columns_v1(&columns(16, 1, ROWS))
            .unwrap();
        let mut original = make();
        for first in [0, 8, 16] {
            original
                .absorb_columns_v1(&columns(first, 8.min(17 - first), ROWS))
                .unwrap();
        }
        assert_eq!(checked.finish().unwrap(), original.finish().unwrap());
    }

    #[test]
    fn initial_coalescing_clears_pending_and_incoming_owners_on_refusal_and_unwind() {
        const ROWS: usize = 4;
        for fault in 0..4 {
            let (result, erased) = field_column_erasure::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let mut owner =
                        InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(ROWS).unwrap();
                    owner
                        .push_v1(columns(0, 8, ROWS), |_| unreachable!())
                        .unwrap();
                    match fault {
                        0 => owner.push_v1(Vec::new(), |_| unreachable!()),
                        1 => owner.push_v1(columns(8, 9, ROWS), |_| unreachable!()),
                        2 => owner.push_v1(columns(8, 8, ROWS), |_| {
                            Err(AggregateStarkErrorV1::InternalInvariant)
                        }),
                        _ => {
                            panic!("next source callback unwinds with first output batch retained");
                        }
                    }
                })
            });
            if fault < 3 {
                assert!(result.unwrap().is_err());
            } else {
                assert!(result.is_err());
            }
            let columns = [8, 17, 16, 8][fault];
            assert_eq!(
                erased.iter().map(|entry| entry.cells).sum::<usize>(),
                columns * ROWS
            );
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
        let (result, erased) = field_column_erasure::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut owner = InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(ROWS).unwrap();
                owner
                    .push_v1(columns(0, 3, ROWS), |_| unreachable!())
                    .unwrap();
                owner.flush_v1(|_| panic!("hash stream unwinds during short group flush"))
            })
        });
        assert!(result.is_err());
        assert_eq!(
            erased.iter().map(|entry| entry.cells).sum::<usize>(),
            3 * ROWS
        );
        assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
    }

    #[test]
    fn initial_coalescing_refuses_invalid_extent_and_overflow() {
        for rows in [0, 3, 1usize << (usize::BITS - 1)] {
            assert!(InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(rows).is_err());
        }
        let rows = 1usize << 22;
        let output = MASKED_TRACE_LDE_COLUMN_BATCH_V1 * rows * core::mem::size_of::<F>();
        let packing = rows.div_ceil(ROWS_PER_PACKING_TASK) * 256;
        let metadata = core::mem::size_of::<InitialCoalescerV1<ZeroizingFieldColumnV1>>()
            + COLUMNS * core::mem::size_of::<ZeroizingFieldColumnV1>()
            + 3 * core::mem::size_of::<Vec<ZeroizingFieldColumnV1>>();
        assert_eq!(packing, 1 << 20);
        assert_eq!(extra_payload_v1(rows).unwrap(), output + packing + metadata);
        assert!(packing + metadata < 2 << 20);
    }

    #[test]
    fn initial_coalescing_empty_flush_is_idle_and_every_refusal_is_terminal() {
        let mut owner = InitialCoalescerV1::<ZeroizingFieldColumnV1>::new_v1(4).unwrap();
        assert!(owner.pending.is_empty());
        owner
            .flush_v1(|_| panic!("empty flush must not hash"))
            .unwrap();
        owner
            .flush_v1(|_| panic!("repeated empty flush must not hash"))
            .unwrap();
        assert!(!owner.failed);
        let mut invalid = columns(0, 8, 4);
        invalid[7].0[3] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            owner.push_v1(invalid, |_| panic!("invalid first batch must not hash")),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );
        assert!(owner.pending.is_empty());
        assert!(owner.failed);
        assert_eq!(
            owner.flush_v1(|_| panic!("poisoned flush must not hash")),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        let (result, erased) = field_column_erasure::observe_v1(|| {
            owner.push_v1(columns(0, 8, 4), |_| panic!("poisoned push must not hash"))
        });
        assert_eq!(result, Err(AggregateStarkErrorV1::InvalidLayout));
        assert_eq!(erased.iter().map(|entry| entry.cells).sum::<usize>(), 8 * 4);
        assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
    }
}
