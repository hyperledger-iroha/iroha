//! Single-owner RFC masked coefficients retained from the original first commitment.

use super::super::super::private_table::{ClearingVecV1, PrivateTableV1, zeroize_fields_v1};
use super::*;

/// Exact public registration slice; no witness-derived cache size or selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlanV1 {
    group: usize,
    start: usize,
    end: usize,
    coefficients: usize,
    base: bool,
}
impl PlanV1 {
    fn new_v1(
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::Rfc5280, 0)?;
        let base = matches!(kind, MainTraceColumnKindV1::Base);
        let (start, end) = if base {
            (registration.base_start, registration.base_end()?)
        } else {
            (registration.aux_start, registration.aux_end()?)
        };
        let coefficients = registration
            .segment
            .trace_size()
            .checked_add(MASK_DEGREE + 1)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        Ok(Self {
            group: registration.trace_group,
            start,
            end,
            coefficients,
            base,
        })
    }
    fn width_v1(self) -> usize {
        self.end - self.start
    }
    fn payload_v1(self) -> Result<usize, ZkX509StarkErrorV1> {
        self.coefficients
            .checked_mul(core::mem::size_of::<F>())
            .and_then(|n| n.checked_add(core::mem::size_of::<PrivateTableV1<F>>()))
            .and_then(|n| n.checked_mul(self.width_v1()))
            .and_then(|n| n.checked_add(core::mem::size_of::<MainRetainedRfcV1>()))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
    }
    fn contains_v1(self, group: usize, column: usize) -> bool {
        group == self.group && (self.start..self.end).contains(&column)
    }
}

/// The original coefficient arrays move here; only bounded working copies leave.
/// All production insertion is private to the original first-commit constructor.
pub(super) struct MainRetainedRfcV1 {
    proof_instance: ZkX509ProofInstanceV1,
    plan: PlanV1,
    columns: ClearingVecV1<PrivateTableV1<F>>,
    native_prepared: bool,
    retained: usize,
}
impl MainRetainedRfcV1 {
    /// Additional split receivers and copy/iterator metadata, beyond the original
    /// five-array native replay ledger. Values keep their existing bounded batch charge.
    pub(super) const fn replay_metadata_v1() -> usize {
        2 * aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
            * core::mem::size_of::<ZeroizingMainTraceColumnV1>()
            + 2 * core::mem::size_of::<Vec<ZeroizingMainTraceColumnV1>>()
            + 2 * core::mem::size_of::<std::vec::IntoIter<ZeroizingMainTraceColumnV1>>()
            + 2 * core::mem::size_of::<PrivateTableV1<F>>()
            + 2 * core::mem::size_of::<ZeroizingMainTraceColumnV1>()
    }
    /// Small original-mask test geometry has no production constructor path.
    #[cfg(test)]
    pub(super) fn for_original_mask_test_v1(group: usize, width: usize, native_log: u8) -> Self {
        Self::from_plan_v1(
            TEST_PROOF_INSTANCE_V1,
            PlanV1 {
                group,
                start: 0,
                end: width,
                coefficients: (1usize << native_log) + MASK_DEGREE + 1,
                base: true,
            },
        )
        .unwrap()
    }
    pub(super) fn forecast_all_v1(
        layout: &AggregateProofLayoutV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        PlanV1::new_v1(layout, MainTraceColumnKindV1::Base)?
            .payload_v1()?
            .checked_add(PlanV1::new_v1(layout, MainTraceColumnKindV1::Aux)?.payload_v1()?)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
    }
    pub(super) fn new_v1(
        proof_instance: ZkX509ProofInstanceV1,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        Self::from_plan_v1(proof_instance, PlanV1::new_v1(layout, kind)?)
    }
    fn from_plan_v1(
        proof_instance: ZkX509ProofInstanceV1,
        plan: PlanV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        plan.payload_v1()?;
        let columns = ClearingVecV1::try_with_capacity_v1(plan.width_v1())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if columns.capacity_v1() != plan.width_v1() {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        Ok(Self {
            proof_instance,
            plan,
            columns,
            native_prepared: false,
            retained: 0,
        })
    }
    pub(super) fn validate_v1(
        &self,
        proof_instance: ZkX509ProofInstanceV1,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if self.proof_instance != proof_instance || self.plan != PlanV1::new_v1(layout, kind)? {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        self.validate_complete_v1()
    }
    fn validate_complete_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        if self.retained != self.plan.width_v1()
            || self.columns.len() != self.plan.width_v1()
            || self.columns.capacity_v1() != self.plan.width_v1()
            || self.columns.iter().any(|column| {
                column.len() != self.plan.coefficients
                    || column.capacity() != self.plan.coefficients
            })
            || self.allocated_payload_v1() != self.plan.payload_v1()?
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }
    pub(super) fn allocated_payload_v1(&self) -> usize {
        self.columns.iter().fold(
            core::mem::size_of::<Self>().saturating_add(self.columns.allocated_bytes_v1()),
            |n, column| {
                n.saturating_add(column.capacity().saturating_mul(core::mem::size_of::<F>()))
            },
        )
    }
    /// Scratch headers used while the final RFC allocation is populated with
    /// native rows. The field allocations are already in forecast_all_v1.
    pub(super) const fn native_preparation_scratch_v1() -> usize {
        core::mem::size_of::<ClearingVecV1<PrivateTableV1<F>>>()
            + core::mem::size_of::<[&mut [F]; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]>()
    }

    /// Populate this final owner only at the first original RFC request. No RNG
    /// is used here; the existing sampler retains its original column order.
    pub(super) fn prepare_native_if_needed_v1(
        &mut self,
        group: usize,
        range: core::ops::Range<usize>,
        fill: impl FnOnce(&mut [&mut [F]]) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        if group != self.plan.group
            || range.end <= self.plan.start
            || range.start >= self.plan.end
            || self.native_prepared
        {
            return Ok(());
        }
        if self.retained != 0
            || !self.columns.is_empty()
            || self.columns.capacity_v1() != self.plan.width_v1()
            || self.plan.width_v1() > ZK_X509_RFC5280_STARK_BASE_WIDTH_V1
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let native_rows = self
            .plan
            .coefficients
            .checked_sub(MASK_DEGREE + 1)
            .filter(|rows| rows.is_power_of_two())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        // Move the already charged header allocation into a transactional owner.
        // On error/unwind every constructed native column clears immediately;
        // self remains incomplete and cannot expose a retained polynomial.
        let mut columns = core::mem::replace(
            &mut self.columns,
            ClearingVecV1::try_with_capacity_v1(0)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?,
        );
        for _ in 0..self.plan.width_v1() {
            let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
            column
                .try_reserve_exact(self.plan.coefficients)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            if column.capacity() != self.plan.coefficients {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            // Initialize the full charged allocation so its mask suffix also
            // has clearing coverage before it is replaced by the original mask.
            column.resize(self.plan.coefficients, F::ZERO);
            columns.try_push_v1(column).map_err(|rejected| {
                drop(rejected);
                ZkX509StarkErrorV1::ProofTooLarge
            })?;
        }
        {
            let mut targets: [&mut [F]; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1] =
                core::array::from_fn(|_| -> &mut [F] { &mut [] });
            for (target, column) in targets.iter_mut().zip(columns.iter_mut()) {
                *target = &mut column[..native_rows];
            }
            fill(&mut targets[..self.plan.width_v1()])?;
        }
        if columns
            .iter()
            .flat_map(|column| column.iter())
            .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        self.columns = columns;
        self.native_prepared = true;
        Ok(())
    }

    /// Bounded native working copies leave the preallocated final owner. This
    /// route is available only before that original column has been masked.
    pub(super) fn copy_native_columns_v1(
        &self,
        group: usize,
        range: core::ops::Range<usize>,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1> {
        let width = range
            .end
            .checked_sub(range.start)
            .filter(|&width| width > 0 && width <= aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if !self.native_prepared
            || self.columns.len() != self.plan.width_v1()
            || !self.plan.contains_v1(group, range.start)
            || range.end > self.plan.end
            || range.start < self.plan.start + self.retained
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let native_rows = self.plan.coefficients - (MASK_DEGREE + 1);
        let mut output = Vec::new();
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if output.capacity() != width {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for column in range {
            output.push(copy_v1(
                &self.columns[column - self.plan.start][..native_rows],
            )?);
        }
        Ok(output)
    }
    /// Preserve the exact complete batch returned after original source/entropy work.
    pub(super) fn retain_batch_v1(
        &mut self,
        group: usize,
        range: core::ops::Range<usize>,
        batch: &mut [ZeroizingMainTraceColumnV1],
    ) -> Result<(), ZkX509StarkErrorV1> {
        if range.end.checked_sub(range.start) != Some(batch.len())
            || batch.is_empty()
            || batch.len() > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        for (index, column) in range.zip(batch) {
            if !self.plan.contains_v1(group, index) {
                continue;
            }
            if index != self.plan.start + self.retained
                || column.len() != self.plan.coefficients
                || column.0.capacity() != self.plan.coefficients
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            // Allocate under a clearing owner before copying private coefficients.
            // On failure the caller still owns the untouched original column.
            let working = copy_v1(column)?;
            let original = core::mem::replace(column, working);
            let original = PrivateTableV1::new(original.into_vec_v1(), zeroize_fields_v1);
            if self.native_prepared {
                // The outgoing native slot already occupies this final-owner
                // allowance; its clearing destructor runs after the replacement.
                self.columns[self.retained] = original;
            } else {
                self.columns.try_push_v1(original).map_err(|rejected| {
                    drop(rejected);
                    ZkX509StarkErrorV1::ProofTooLarge
                })?;
            }
            self.retained += 1;
        }
        Ok(())
    }
    /// Public maximal run beginning at `first`, bounded by the requested original batch.
    pub(super) fn run_v1(
        &self,
        group: usize,
        first: usize,
        end: usize,
    ) -> Result<(usize, bool), ZkX509StarkErrorV1> {
        if first >= end || end - first > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if group != self.plan.group || first >= self.plan.end {
            return Ok((end, false));
        }
        if first < self.plan.start {
            return Ok((end.min(self.plan.start), false));
        }
        Ok((end.min(self.plan.end), true))
    }
    /// Borrow one immutable original; public range classification has already
    /// selected the source. No coefficient copy or fresh source work occurs.
    pub(super) fn column_v1(
        &self,
        group: usize,
        column: usize,
    ) -> Result<&[F], ZkX509StarkErrorV1> {
        self.validate_complete_v1()?;
        if !self.plan.contains_v1(group, column) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(&self.columns[column - self.plan.start])
    }
    /// Borrow exactly the original registration's complete coefficient owners.
    /// Callers form bounded stack views; no private allocation changes custody.
    pub(super) fn registration_columns_v1(
        &self,
        group: usize,
        range: core::ops::Range<usize>,
    ) -> Result<&[PrivateTableV1<F>], ZkX509StarkErrorV1> {
        self.validate_complete_v1()?;
        if group != self.plan.group || range != (self.plan.start..self.plan.end) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(self.columns.as_slice_v1())
    }
    pub(super) fn group_width_v1(&self, group: usize) -> usize {
        if group == self.plan.group {
            self.plan.width_v1()
        } else {
            0
        }
    }
    pub(super) fn copy_columns_v1(
        &self,
        group: usize,
        range: core::ops::Range<usize>,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1> {
        self.validate_complete_v1()?;
        let width = range
            .end
            .checked_sub(range.start)
            .filter(|&n| n > 0 && n <= aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if !self.plan.contains_v1(group, range.start) || range.end > self.plan.end {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if output.capacity() != width {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for index in range {
            output.push(copy_v1(&self.columns[index - self.plan.start])?);
        }
        Ok(output)
    }
}
fn copy_v1(column: &[F]) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
    let mut copy = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    copy.try_reserve_exact(column.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    if copy.capacity() != column.len() {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    copy.extend_from_slice(column);
    Ok(ZeroizingMainTraceColumnV1(copy.into_vec()))
}

#[cfg(test)]
mod tests {
    use super::super::main_bounded_transform::MainBoundedTransformPolicyV1;
    use super::*;
    fn fixture() -> MainRetainedRfcV1 {
        MainRetainedRfcV1::from_plan_v1(
            TEST_PROOF_INSTANCE_V1,
            PlanV1 {
                group: 5,
                start: 4,
                end: 7,
                coefficients: 32,
                base: true,
            },
        )
        .unwrap()
    }
    fn column(value: u64) -> ZeroizingMainTraceColumnV1 {
        ZeroizingMainTraceColumnV1(vec![F(value); 32])
    }
    #[test]
    fn complete_registration_borrow_preserves_original_addresses_and_rejects_subsets() {
        let mut retained = fixture();
        assert!(retained.registration_columns_v1(5, 4..7).is_err());
        let mut batch = vec![column(3), column(5), column(7)];
        let pointers = batch
            .iter()
            .map(|column| column.as_ptr())
            .collect::<Vec<_>>();
        retained.retain_batch_v1(5, 4..7, &mut batch).unwrap();
        let originals = retained.registration_columns_v1(5, 4..7).unwrap();
        assert_eq!(
            originals
                .iter()
                .map(|column| column.as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
        assert_eq!(originals[1][0], F(5));
        for (group, range) in [(4, 4..7), (5, 3..7), (5, 4..6), (5, 4..8), (5, 7..4)] {
            assert!(retained.registration_columns_v1(group, range).is_err());
        }
        retained.columns[0].pop();
        assert!(retained.registration_columns_v1(5, 4..7).is_err());
    }

    #[test]
    fn original_transfer_and_copies_retain_all_coefficients_without_aliasing() {
        let mut retained = fixture();
        let mut batch = vec![column(3), column(5), column(7), column(11)];
        let pointers: Vec<_> = batch.iter().map(|x| x.as_ptr()).collect();
        retained.retain_batch_v1(5, 3..7, &mut batch).unwrap();
        retained.validate_complete_v1().unwrap();
        assert_eq!(batch[0].as_ptr(), pointers[0]);
        for i in 0..3 {
            assert_eq!(retained.columns[i].as_ptr(), pointers[i + 1]);
            assert_ne!(batch[i + 1].as_ptr(), pointers[i + 1]);
            assert_eq!(retained.columns[i].as_slice(), &*batch[i + 1]);
        }
        // Exercise the same pre-copy reservation primitive as the production
        // split receiver, at its exact limit and one byte below it.
        let rows = 256;
        let admitted = rows * core::mem::size_of::<u64>()
            + core::mem::size_of::<Vec<u64>>()
            + core::mem::size_of::<PrivateTableV1<Vec<u64>>>()
            + fastpq_prover::goldilocks_transform::metal_goldilocks_transform_extra_payload_v1(
                rows, 1,
            )
            .unwrap();
        let metadata = MainRetainedRfcV1::replay_metadata_v1();
        assert!(admitted >= metadata);
        for shortage in [0, 1] {
            let policy = MainBoundedTransformPolicyV1::for_test_v1(rows, 1)
                .reserve_additional_v1(admitted - metadata + shortage)
                .unwrap();
            let mut copied_after_reservation = false;
            let outcome = policy
                .reserve_additional_v1(metadata)
                .and_then(|remaining| {
                    assert!(remaining.reserve_additional_v1(1).is_err());
                    copied_after_reservation = true;
                    retained.copy_columns_v1(5, 4..7)
                });
            assert_eq!(outcome.is_ok(), shortage == 0);
            assert_eq!(copied_after_reservation, shortage == 0);
            if let Ok(columns) = outcome {
                assert_eq!(columns, batch[1..]);
            }
        }
        let copied = retained.copy_columns_v1(5, 4..7).unwrap();
        assert_eq!(copied, batch[1..]);
        batch[1][31] = F(99);
        assert_eq!(retained.columns[0][31], F(5));
        assert_eq!(copied[0][31], F(5));
        assert_eq!(
            retained.allocated_payload_v1(),
            retained.plan.payload_v1().unwrap()
        );
    }
    #[test]
    fn exact_public_registration_and_all_crossing_batches_preserve_column_order() {
        let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
        for (kind, start, end) in [
            (MainTraceColumnKindV1::Base, 76, 361),
            (MainTraceColumnKindV1::Aux, 196, 476),
        ] {
            let cache = MainRetainedRfcV1::new_v1(TEST_PROOF_INSTANCE_V1, &layout, kind).unwrap();
            assert_eq!(
                (
                    cache.plan.group,
                    cache.plan.start,
                    cache.plan.end,
                    cache.plan.coefficients
                ),
                (5, start, end, 526_104)
            );
            let group = layout.trace_groups[5];
            let width = if matches!(kind, MainTraceColumnKindV1::Base) {
                group.base_width
            } else {
                group.aux_width
            };
            let mut seen = Vec::new();
            let mut cached = Vec::new();
            for first in (0..width).step_by(8) {
                let end_batch = (first + 8).min(width);
                let mut cursor = first;
                while cursor < end_batch {
                    let (next, hit) = cache.run_v1(5, cursor, end_batch).unwrap();
                    assert!(next > cursor);
                    for column in cursor..next {
                        seen.push(column);
                        if hit {
                            cached.push(column);
                        }
                    }
                    cursor = next;
                }
            }
            assert_eq!(seen, (0..width).collect::<Vec<_>>());
            assert_eq!(cached, (start..end).collect::<Vec<_>>());
            assert_eq!(
                cache.validate_v1(TEST_PROOF_INSTANCE_V1, &layout, kind),
                Err(ZkX509StarkErrorV1::ProfileMismatch)
            );
            let wrong_kind = if matches!(kind, MainTraceColumnKindV1::Base) {
                MainTraceColumnKindV1::Aux
            } else {
                MainTraceColumnKindV1::Base
            };
            assert_eq!(
                cache.validate_v1(TEST_PROOF_INSTANCE_V1, &layout, wrong_kind),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            );
            assert_eq!(
                cache.validate_v1(ZkX509ProofInstanceV1::new_v1([0x6b; 32]), &layout, kind),
                Err(ZkX509StarkErrorV1::TranscriptMismatch)
            );
        }
    }
    #[test]
    fn partial_missing_reordered_oversized_and_duplicate_columns_are_refused() {
        let mut cache = fixture();
        assert!(cache.copy_columns_v1(5, 4..5).is_err());
        assert!(cache.retain_batch_v1(5, 5..6, &mut [column(1)]).is_err());
        let mut too_short = ZeroizingMainTraceColumnV1(vec![F::ONE; 31]);
        assert!(
            cache
                .retain_batch_v1(5, 4..5, core::slice::from_mut(&mut too_short))
                .is_err()
        );
        cache.retain_batch_v1(5, 4..5, &mut [column(2)]).unwrap();
        assert!(cache.validate_complete_v1().is_err());
        assert!(cache.retain_batch_v1(5, 4..5, &mut [column(2)]).is_err());
        cache
            .retain_batch_v1(5, 5..7, &mut [column(3), column(4)])
            .unwrap();
        cache.validate_complete_v1().unwrap();
        for range in [3..5, 6..8, 4..4, 4..13] {
            assert!(cache.copy_columns_v1(5, range).is_err());
        }
        assert!(cache.copy_columns_v1(4, 4..5).is_err());
        let mut overflow = cache.plan;
        overflow.coefficients = usize::MAX;
        assert!(overflow.payload_v1().is_err());
    }
    #[test]
    fn retained_and_working_allocations_clear_on_success_error_and_unwind() {
        use super::super::super::super::private_table::{allocation_inspection, inspection};
        for mode in 0..3 {
            let ((result, fields), allocations) = allocation_inspection::observe_v1(|| {
                inspection::observe_v1(|| {
                    std::panic::catch_unwind(|| {
                        let mut retained = fixture();
                        let mut batch = vec![column(3), column(5), column(7)];
                        if mode == 0 {
                            retained.retain_batch_v1(5, 4..7, &mut batch).unwrap();
                            retained.validate_complete_v1().unwrap();
                            Ok(())
                        } else {
                            retained.retain_batch_v1(5, 4..5, &mut batch[..1]).unwrap();
                            assert!(mode != 2, "injected retained owner unwind");
                            Err(())
                        }
                    })
                })
            });
            match mode {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(result.unwrap().is_err()),
                _ => assert!(result.is_err()),
            }
            let expected = (if mode == 0 { 6 } else { 4 }) * 32;
            assert_eq!(
                fields.iter().map(|x| x.nonzero_before).sum::<usize>(),
                expected
            );
            assert!(fields.iter().all(|x| x.nonzero_after == 0));
            assert_eq!(allocations.len(), 1);
            assert_eq!(
                allocations[0].bytes,
                3 * core::mem::size_of::<PrivateTableV1<F>>()
            );
            assert_eq!(allocations[0].nonzero_after, 0);
        }
    }

    #[test]
    fn native_preparation_uses_final_capacity_and_original_coefficients_replace_each_slot() {
        let mut retained = MainRetainedRfcV1::for_original_mask_test_v1(5, 3, 4);
        assert!(retained.copy_native_columns_v1(5, 0..1).is_err());
        retained
            .prepare_native_if_needed_v1(4, 0..3, |_| panic!("wrong group read source"))
            .unwrap();
        assert!(!retained.native_prepared);
        retained
            .prepare_native_if_needed_v1(5, 0..3, |targets| {
                for (column, target) in targets.iter_mut().enumerate() {
                    target.fill(F((column + 1) as u64));
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            retained.allocated_payload_v1(),
            retained.plan.payload_v1().unwrap()
        );
        assert!(retained.column_v1(5, 0).is_err());
        for column in retained.columns.iter() {
            assert_eq!(column.len(), retained.plan.coefficients);
            assert_eq!(column.capacity(), retained.plan.coefficients);
            assert!(column[16..].iter().all(|value| *value == F::ZERO));
        }
        let native = retained.copy_native_columns_v1(5, 0..3).unwrap();
        for (column, copy) in native.iter().enumerate() {
            assert_eq!(copy.0.capacity(), 16);
            assert_eq!(&**copy, &[F((column + 1) as u64); 16]);
        }
        retained
            .prepare_native_if_needed_v1(5, 0..3, |_| panic!("prepared source replayed"))
            .unwrap();
        let coefficients = retained.plan.coefficients;
        let mut originals = (0..3)
            .map(|column| ZeroizingMainTraceColumnV1(vec![F((column + 7) as u64); coefficients]))
            .collect::<Vec<_>>();
        let pointers = originals
            .iter()
            .map(|column| column.as_ptr())
            .collect::<Vec<_>>();
        retained
            .retain_batch_v1(5, 0..1, &mut originals[..1])
            .unwrap();
        assert!(retained.copy_native_columns_v1(5, 0..1).is_err());
        assert!(retained.copy_native_columns_v1(5, 1..3).is_ok());
        assert!(retained.copy_columns_v1(5, 0..1).is_err());
        retained
            .retain_batch_v1(5, 1..3, &mut originals[1..])
            .unwrap();
        retained.validate_complete_v1().unwrap();
        for (index, pointer) in pointers.into_iter().enumerate() {
            assert_eq!(retained.columns[index].as_ptr(), pointer);
            assert_ne!(originals[index].as_ptr(), pointer);
        }
        assert_eq!(
            retained.allocated_payload_v1(),
            retained.plan.payload_v1().unwrap()
        );
        assert_eq!(retained.copy_columns_v1(5, 0..3).unwrap(), originals);
    }

    #[test]
    fn native_preparation_clears_complete_allocations_on_error_unwind_and_noncanonical_input() {
        use super::super::super::super::private_table::{allocation_inspection, inspection};
        for mode in 0..3 {
            let ((result, fields), allocations) = allocation_inspection::observe_v1(|| {
                inspection::observe_v1(|| {
                    let mut retained = MainRetainedRfcV1::for_original_mask_test_v1(5, 3, 4);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        retained.prepare_native_if_needed_v1(5, 0..3, |targets| {
                            for column in targets.iter_mut() {
                                column.fill(F(13));
                            }
                            if mode == 0 {
                                return Err(ZkX509StarkErrorV1::ProfileMismatch);
                            }
                            if mode == 1 {
                                panic!("injected native preparation unwind");
                            }
                            targets[2][15] = F(u64::MAX);
                            Ok(())
                        })
                    }));
                    assert!(!retained.native_prepared);
                    assert_eq!(retained.retained, 0);
                    assert!(retained.columns.is_empty());
                    assert_eq!(retained.columns.capacity_v1(), 0);
                    assert!(retained.column_v1(5, 0).is_err());
                    assert!(retained.copy_native_columns_v1(5, 0..1).is_err());
                    result
                })
            });
            assert!(result.map_or(mode == 1, |outcome| outcome.is_err()));
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.nonzero_before)
                    .sum::<usize>(),
                48
            );
            assert!(fields.iter().all(|field| field.nonzero_after == 0));
            assert_eq!(allocations.len(), 2);
            assert_eq!(
                allocations[0].bytes,
                3 * core::mem::size_of::<PrivateTableV1<F>>()
            );
            assert_eq!(allocations[1].bytes, 0);
            assert!(
                allocations
                    .iter()
                    .all(|allocation| allocation.nonzero_after == 0)
            );
        }
    }
}
