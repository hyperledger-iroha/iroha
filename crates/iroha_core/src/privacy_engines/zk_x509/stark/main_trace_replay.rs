//! Original-mask replay from closed immutable MAIN witness owners.

use super::super::super::private_table::{PrivateTableV1, zeroize_fields_v1};
#[cfg(test)]
use super::super::super::prover_observation::{PhaseTimerV1, PhaseV1};
use super::*;
use crate::privacy_engines::transparent_stark::{ReplayableTraceMaskV1, sample_trace_mask_v1};

#[cfg(test)]
use crate::privacy_engines::transparent_stark::masked_trace_coefficients_with_mask_v1;

/// The only long-lived per-column randomness; coefficients clear on drop.
pub(in super::super) struct MainTraceMaskGroupV1 {
    native_log: u8,
    common_log: u8,
    masks: Vec<ReplayableTraceMaskV1>,
}

impl MainTraceMaskGroupV1 {
    pub(in super::super) fn sample_v1<R: TryRngCore>(
        native_log: u8,
        common_log: u8,
        width: usize,
        rng: &mut R,
        mut source: impl FnMut(usize) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let native_rows = 1_usize
            .checked_shl(u32::from(native_log))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let common_rows = 1_usize
            .checked_shl(u32::from(common_log))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if native_log >= common_log
            || width == 0
            || width > usize::from(u16::MAX)
            || native_rows
                .checked_add(MASK_DEGREE)
                .is_none_or(|highest| highest >= common_rows)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for column in 0..width {
            // Preserve the original source-before-entropy order and reject bad
            // native columns before sampling their masks. No native column survives.
            #[cfg(test)]
            let source_timer = PhaseTimerV1::start_v1(PhaseV1::SampleSourceColumns);
            let native = source(column)?;
            #[cfg(test)]
            source_timer.complete_v1();
            if native.len() != native_rows
                || native.iter().any(|value| F::canonical(value.0).is_none())
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            #[cfg(test)]
            let mask_timer = PhaseTimerV1::start_v1(PhaseV1::SampleMaskDraws);
            masks.push(sample_trace_mask_v1(MASK_DEGREE, rng).map_err(map_transparent_error_v1)?);
            #[cfg(test)]
            mask_timer.complete_v1();
        }
        Ok(Self {
            native_log,
            common_log,
            masks,
        })
    }

    /// Reuse bounded source batches while the original sampler preserves the
    /// exact per-column validation and entropy draw sequence when source
    /// construction succeeds. A failed batch construction can reject before
    /// the corresponding scalar source would have been reached; no proof is
    /// published in either case. The pending iterator owns at most eight
    /// clearing columns on error and unwind too.
    pub(in super::super) fn sample_batched_v1<R: TryRngCore>(
        native_log: u8,
        common_log: u8,
        width: usize,
        rng: &mut R,
        mut source: impl FnMut(
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let mut pending = Vec::new().into_iter();
        let mut next_column = 0;
        Self::sample_v1(native_log, common_log, width, rng, |column| {
            if column != next_column {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            if pending.len() == 0 {
                let end = column
                    .checked_add(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
                    .min(width);
                let batch = source(column..end)?;
                if batch.len() != end - column {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                pending = batch.into_iter();
            }
            next_column += 1;
            pending.next().ok_or(ZkX509StarkErrorV1::InternalInvariant)
        })
    }

    #[cfg(test)]
    fn replay_v1(
        &self,
        column: usize,
        native: &[F],
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let mask = self
            .masks
            .get(column)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        masked_trace_coefficients_with_mask_v1(native, self.native_log, mask.coefficients())
            .map(ZeroizingMainTraceColumnV1)
            .map_err(map_transparent_error_v1)
    }

    /// Construct sources serially, then interpolate only the bounded resident
    /// batch in parallel. Mask application replaces one allocation at a time;
    /// the original random coefficients and column order are unchanged.
    fn replay_batch_v1(
        &self,
        columns: core::ops::Range<usize>,
        mut source: impl FnMut(usize) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1>,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1> {
        let width = columns
            .end
            .checked_sub(columns.start)
            .filter(|&width| width != 0 && width <= aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let masks = self
            .masks
            .get(columns.clone())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let native_rows = 1_usize
            .checked_shl(u32::from(self.native_log))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let root =
            goldilocks_primitive_root_v1(self.native_log).map_err(map_transparent_error_v1)?;
        if masks.iter().any(|mask| {
            mask.coefficients().len() != MASK_DEGREE + 1
                || mask
                    .coefficients()
                    .iter()
                    .any(|value| F::canonical(value.0).is_none())
        }) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let coefficient_count = native_rows
            .checked_add(MASK_DEGREE + 1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut batch = Vec::new();
        batch
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for column in columns {
            // Never overlap source construction scratch: only the completed
            // native column transfers to this bounded batch.
            let native = PrivateTableV1::new(source(column)?.into_vec_v1(), zeroize_fields_v1);
            if native.len() != native_rows
                || native.iter().any(|value| F::canonical(value.0).is_none())
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            batch.push(native);
        }
        batch.par_iter_mut().try_for_each(|native| {
            crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(native, root)
                .map_err(map_transparent_error_v1)
        })?;
        for (native, mask) in batch.iter_mut().zip(masks) {
            let mut coefficients = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
            coefficients
                .try_reserve_exact(coefficient_count)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            coefficients.extend_from_slice(native);
            coefficients.resize(coefficient_count, F::ZERO);
            // T(X) + r(X)(X^n - 1), including masks longer than native n.
            for (degree, &random) in mask.coefficients().iter().enumerate() {
                coefficients[degree] = coefficients[degree].sub(random);
                coefficients[native_rows + degree] = coefficients[native_rows + degree].add(random);
            }
            *native = coefficients;
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for coefficients in batch {
            output.push(ZeroizingMainTraceColumnV1(coefficients.into_vec()));
        }
        Ok(output)
    }
}

/// Six exact groups retain explicit original masks, never all native coefficients.
/// Witnesses remain borrowed from the closed phase owners below. Each replay
/// reconstructs the same polynomial and final commitment replay checks its root.
pub(in super::super) struct MainTracePolynomialSetV1 {
    groups: [MainTraceMaskGroupV1; FULL_PROFILE_TRACE_GROUPS_V1],
}

impl MainTracePolynomialSetV1 {
    pub(in super::super) fn from_ordered_v1(
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        groups: Vec<MainTraceMaskGroupV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let set = Self {
            groups: groups
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?,
        };
        set.validate_v1(layout, kind)?;
        Ok(set)
    }

    pub(super) fn validate_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        for (masks, group) in self.groups.iter().zip(&layout.trace_groups) {
            let width = match kind {
                MainTraceColumnKindV1::Base => group.base_width,
                MainTraceColumnKindV1::Aux => group.aux_width,
            };
            if masks.masks.len() != width
                || masks.native_log != group.native_trace_log2
                || masks.common_log != layout.common_lde_log2
                || masks
                    .masks
                    .iter()
                    .any(|mask| mask.coefficients().len() != MASK_DEGREE + 1)
            {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn joined_plan_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<aggregate::joined_trace::JoinedTraceCommitmentPlanV1, ZkX509StarkErrorV1> {
        self.validate_v1(layout, kind)?;
        aggregate::joined_trace::JoinedTraceCommitmentPlanV1::new_v1(
            layout.parameters_v1(),
            &layout.as_shared()?,
            match kind {
                MainTraceColumnKindV1::Base => {
                    aggregate::joined_trace::JoinedTraceColumnKindV1::Base
                }
                MainTraceColumnKindV1::Aux => aggregate::joined_trace::JoinedTraceColumnKindV1::Aux,
            },
        )
        .map_err(map_aggregate_error_v1)
    }

    pub(super) fn replay_columns_coefficients_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group_index: usize,
        columns: core::ops::Range<usize>,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1> {
        let masks = self
            .groups
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let mut next_column = columns.start;
        let mut native = None;
        masks.replay_batch_v1(columns.clone(), |column| {
            if column != next_column {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            if native.is_none() {
                native = Some(
                    sources
                        .native_columns_v1(layout, kind, group_index, columns.clone())?
                        .into_iter(),
                );
            }
            next_column += 1;
            native
                .as_mut()
                .and_then(Iterator::next)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)
        })
    }

    pub(super) fn commit_joined_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        indices: &[usize],
        assembly_payload: usize,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<aggregate::StreamingRowCommitmentResultV1, ZkX509StarkErrorV1> {
        let plan = self.joined_plan_v1(layout, kind)?;
        // The same joined plan creates the leaf framing for retained and replayed
        // columns. At most eight coefficient/evaluation columns coexist.
        let mut source_error = None;
        let mut pending = Vec::new().into_iter();
        let mut pending_next = None;
        let mut evaluator =
            main_transform::MainTraceCosetEvaluatorV1::new_v1(layout, assembly_payload)?;
        let result = plan.commit_replayed_v1(
            AGGREGATE_DOMAINS_V1,
            indices,
            |group, column| {
                let replay = (|| {
                    if pending.len() == 0 {
                        if column % aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1 != 0 {
                            return Err(ZkX509StarkErrorV1::InternalInvariant);
                        }
                        let width = self
                            .groups
                            .get(group)
                            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
                            .masks
                            .len();
                        let end = width.min(column + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
                        pending = self
                            .replay_columns_coefficients_v1(
                                layout,
                                kind,
                                group,
                                column..end,
                                sources,
                            )?
                            .into_iter();
                        pending_next = Some((group, column));
                    }
                    if pending_next != Some((group, column)) {
                        return Err(ZkX509StarkErrorV1::InternalInvariant);
                    }
                    pending_next = Some((group, column + 1));
                    pending.next().ok_or(ZkX509StarkErrorV1::InternalInvariant)
                })();
                replay
                    .map(|coefficients| coefficients.into_vec_v1())
                    .map_err(|error| {
                        source_error = Some(error);
                        AggregateStarkErrorV1::InvalidLayout
                    })
            },
            |columns, native, common| evaluator.evaluate_v1(columns, native, common),
        );
        tracing::debug!(target: "zk_x509::transform", actual = ?evaluator.receipt_v1(),
            "completed MAIN joined commitment transform dispatches");
        if fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1() {
            return Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain);
        }
        match (result, source_error) {
            (Ok(result), None) => Ok(result),
            (Err(_), Some(error)) => Err(error),
            (Err(error), None) => Err(map_aggregate_error_v1(error)),
            (Ok(_), Some(_)) => Err(ZkX509StarkErrorV1::InternalInvariant),
        }
    }

    pub(super) fn deep_group_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group: usize,
        point: E,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<(Vec<E>, Vec<E>), ZkX509StarkErrorV1> {
        self.validate_v1(layout, kind)?;
        let masks = self
            .groups
            .get(group)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let next = point.mul_base(
            goldilocks_primitive_root_v1(masks.native_log).map_err(map_transparent_error_v1)?,
        );
        let mut current_values = ZeroizingExtensionColumnV1(Vec::new());
        let mut next_values = ZeroizingExtensionColumnV1(Vec::new());
        current_values
            .0
            .try_reserve_exact(masks.masks.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        next_values
            .0
            .try_reserve_exact(masks.masks.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        let powers = main_deep_replay::MainDeepPointPowersV1::new_v1(
            [point, next],
            (1_usize << masks.native_log) + MASK_DEGREE + 1,
        )?;
        for first in (0..masks.masks.len()).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let end = masks
                .masks
                .len()
                .min(first + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
            let coefficients =
                self.replay_columns_coefficients_v1(layout, kind, group, first..end, sources)?;
            let values = coefficients
                .par_iter()
                .map(|column| powers.evaluate_v1(column))
                .collect::<Result<Vec<_>, _>>()?;
            for [current, next] in values {
                current_values.0.push(current);
                next_values.0.push(next);
            }
        }
        Ok((
            core::mem::take(&mut current_values.0),
            core::mem::take(&mut next_values.0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn native(log: u8, column: usize) -> ZeroizingMainTraceColumnV1 {
        ZeroizingMainTraceColumnV1(
            (0..1_usize << log)
                .map(|row| F((row * row + 17 * column + 3) as u64))
                .collect(),
        )
    }

    #[test]
    fn mask_only_replay_matches_retained_coefficients_entropy_and_independent_native_evaluation() {
        for native_log in [3, 8, 11] {
            let common_log = 13;
            let mut replay_rng = StdRng::from_seed([native_log; 32]);
            let mut retained_rng = StdRng::from_seed([native_log; 32]);
            let group = MainTraceMaskGroupV1::sample_v1(
                native_log,
                common_log,
                3,
                &mut replay_rng,
                |column| Ok(native(native_log, column)),
            )
            .unwrap();
            let retained = aggregate::MaskedTracePolynomialSetV1::sample_columns_v1(
                native_log,
                common_log,
                3,
                MASK_DEGREE,
                &mut retained_rng,
                |column| Ok(native(native_log, column).into_vec_v1()),
            )
            .unwrap();
            assert_eq!(replay_rng.next_u64(), retained_rng.next_u64());
            let root = goldilocks_primitive_root_v1(native_log).unwrap();
            for column in 0..3 {
                let values = native(native_log, column);
                let coefficients = group.replay_v1(column, &values).unwrap();
                assert_eq!(
                    &*coefficients,
                    retained.column_coefficients_v1(column).unwrap()
                );
                assert_eq!(&*group.replay_v1(column, &values).unwrap(), &*coefficients);
                // The mask vanishes at each independently selected native point,
                // including when its degree exceeds the smallest native subgroup.
                for row in [0, 1, (1_usize << native_log) - 1] {
                    let point = root.pow(row as u128);
                    let actual = coefficients
                        .iter()
                        .rev()
                        .fold(F::ZERO, |value, &coefficient| {
                            value.mul(point).add(coefficient)
                        });
                    assert_eq!(actual, values[row]);
                }
            }
        }
    }

    #[test]
    fn bounded_parallel_interpolation_preserves_original_masks_order_and_polynomials() {
        for native_log in [3, 11] {
            let mut rng = StdRng::from_seed([native_log + 91; 32]);
            let group = MainTraceMaskGroupV1::sample_v1(native_log, 13, 10, &mut rng, |column| {
                Ok(native(native_log, column))
            })
            .unwrap();
            for width in [1, 3, 8] {
                let range = 2..2 + width;
                let expected = range
                    .clone()
                    .map(|column| {
                        group
                            .replay_v1(column, &native(native_log, column))
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                for workers in [1, 4] {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(workers)
                        .build()
                        .unwrap();
                    let actual = pool.install(|| {
                        let caller = std::thread::current().id();
                        let mut order = Vec::new();
                        let result = group
                            .replay_batch_v1(range.clone(), |column| {
                                assert_eq!(std::thread::current().id(), caller);
                                order.push(column);
                                Ok(native(native_log, column))
                            })
                            .unwrap();
                        assert_eq!(order, range.clone().collect::<Vec<_>>());
                        result
                    });
                    assert_eq!(actual, expected);
                    let root = goldilocks_primitive_root_v1(native_log).unwrap();
                    for (offset, coefficients) in actual.iter().enumerate() {
                        let values = native(native_log, offset + 2);
                        for row in [0, 1, values.len() / 2, values.len() - 1] {
                            let point = root.pow(row as u128);
                            let value = coefficients
                                .iter()
                                .rev()
                                .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient));
                            assert_eq!(value, values[row]);
                        }
                        if native_log == 3 {
                            // Independent quadratic-time inverse DFT and exact
                            // coefficient convolution; no FFT helper is used.
                            let n = values.len();
                            let inverse_n = F(n as u64).inv().unwrap();
                            let inverse_root = root.inv().unwrap();
                            let mut reference = vec![F::ZERO; n + MASK_DEGREE + 1];
                            for (degree, target) in reference[..n].iter_mut().enumerate() {
                                *target = values
                                    .iter()
                                    .enumerate()
                                    .fold(F::ZERO, |sum, (row, &value)| {
                                        sum.add(value.mul(inverse_root.pow((degree * row) as u128)))
                                    })
                                    .mul(inverse_n);
                            }
                            for (degree, &mask) in
                                group.masks[offset + 2].coefficients().iter().enumerate()
                            {
                                reference[degree] = reference[degree].sub(mask);
                                reference[n + degree] = reference[n + degree].add(mask);
                            }
                            assert_eq!(&**coefficients, reference);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn bounded_interpolation_clears_partial_sources_on_error_unwind_and_replacement() {
        use super::super::super::super::private_table::inspection;
        let mut rng = StdRng::from_seed([119; 32]);
        let group =
            MainTraceMaskGroupV1::sample_v1(3, 13, 8, &mut rng, |column| Ok(native(3, column)))
                .unwrap();
        for range in [0..0, 0..9, 9..10] {
            let mut calls = 0;
            assert!(
                group
                    .replay_batch_v1(range, |_| {
                        calls += 1;
                        Ok(native(3, 0))
                    })
                    .is_err()
            );
            assert_eq!(calls, 0);
        }
        let assert_cleared = |observations: Vec<inspection::ErasureObservationV1>, cells: usize| {
            assert_eq!(
                observations.iter().map(|value| value.cells).sum::<usize>(),
                cells
            );
            assert!(
                observations
                    .iter()
                    .map(|value| value.nonzero_before)
                    .sum::<usize>()
                    > 0
            );
            assert!(observations.iter().all(|value| value.nonzero_after == 0));
        };
        for unwind in [false, true] {
            let (result, observations) = inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    group.replay_batch_v1(0..8, |column| {
                        if column == 2 {
                            assert!(!unwind, "injected source unwind");
                            return Err(ZkX509StarkErrorV1::InternalInvariant);
                        }
                        Ok(native(3, column))
                    })
                }))
            });
            assert!(match result {
                Ok(result) => result.is_err(),
                Err(_) => unwind,
            });
            assert_cleared(observations, 16);
        }
        let (result, observations) = inspection::observe_v1(|| {
            group
                .replay_batch_v1(0..8, |column| Ok(native(3, column)))
                .unwrap()
        });
        assert_cleared(observations, 64);
        assert_eq!(result.len(), 8);
        let (result, observations) = inspection::observe_v1(|| {
            group.replay_batch_v1(0..8, |column| {
                if column == 7 {
                    Ok(ZeroizingMainTraceColumnV1(vec![F::ONE; 7]))
                } else {
                    Ok(native(3, column))
                }
            })
        });
        assert!(result.is_err());
        assert_cleared(observations, 63);
    }

    #[test]
    fn mask_replay_rejects_invalid_shapes_and_preserves_entropy_on_source_failure() {
        let mut rng = StdRng::from_seed([71; 32]);
        let mut unchanged = rng.clone();
        let mut calls = 0;
        assert!(
            MainTraceMaskGroupV1::sample_v1(3, 3, 1, &mut rng, |_| {
                calls += 1;
                Ok(native(3, 0))
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert_eq!(rng.next_u64(), unchanged.next_u64());
        for invalid in [
            vec![F::ONE; 7],
            vec![F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1); 8],
        ] {
            let mut rng = StdRng::from_seed([72; 32]);
            let mut unchanged = rng.clone();
            assert!(
                MainTraceMaskGroupV1::sample_v1(3, 11, 1, &mut rng, |_| Ok(
                    ZeroizingMainTraceColumnV1(invalid.clone())
                ))
                .is_err()
            );
            assert_eq!(rng.next_u64(), unchanged.next_u64());
        }
        let mut rng = StdRng::from_seed([73; 32]);
        let group =
            MainTraceMaskGroupV1::sample_v1(3, 11, 1, &mut rng, |_| Ok(native(3, 0))).unwrap();
        assert!(group.replay_v1(1, &native(3, 0)).is_err());
        assert!(group.replay_v1(0, &[F::ONE; 7]).is_err());
        let mut coefficients = group.replay_v1(0, &native(3, 0)).unwrap();
        coefficients.zeroize_private_v1();
        assert!(coefficients.is_empty());
        assert!(core::mem::needs_drop::<MainTraceMaskGroupV1>());
        assert!(core::mem::needs_drop::<ReplayableTraceMaskV1>());
    }
}

/// Immutable closed source roles for deterministic replay of original columns.
pub(super) enum MainTraceReplaySourcesV1<'phase, 'assembly> {
    Base {
        assembly: &'assembly ZkX509MainTraceAssemblyV1,
        sha: &'phase [ZkX509ShaBatchSegmentBaseSourceV1<'assembly>; ZK_X509_SHA_SEGMENT_COUNT_V1],
        p256: &'phase P256MainBaseSourceV1,
        projection: &'phase MainProjectionTraceGroupSourceV1<'assembly>,
        io: &'phase MainIoTraceGroupSourceV1<'assembly>,
    },
    Bound {
        log19: &'phase MainLog19BoundTraceGroupSourceV1<'assembly>,
        projection: &'phase MainProjectionTraceGroupSourceV1<'assembly>,
        io: &'phase MainIoTraceGroupSourceV1<'assembly>,
    },
}

impl MainTraceReplaySourcesV1<'_, '_> {
    /// Extract each SHA run and bound arithmetic auxiliary run once, preserving
    /// public group/registration order and the closed base/bound phase.
    pub(super) fn native_columns_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group: usize,
        columns: core::ops::Range<usize>,
    ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1> {
        let width = columns
            .end
            .checked_sub(columns.start)
            .filter(|&width| width > 0 && width <= aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if matches!(self, Self::Base { .. }) && matches!(kind, MainTraceColumnKindV1::Aux) {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        let mut first = columns.start;
        while first < columns.end {
            let (registration, local) =
                registered_main_group_column_v1(layout, group, kind, first)?;
            if registration.segment.adapter == SegmentAdapterIdV1::P256Arithmetic
                && matches!(kind, MainTraceColumnKindV1::Aux)
                && let Self::Bound { log19, .. } = self
            {
                let end = columns.end.min(registration.aux_end()?);
                if end <= first {
                    return Err(ZkX509StarkErrorV1::ProfileMismatch);
                }
                let binding = log19.p256_binding_v1(registration)?;
                let mut batch = (first..end)
                    .map(|_| zeroed_main_trace_column_v1(registration.segment.trace_size()))
                    .collect::<Result<Vec<_>, _>>()?;
                {
                    let count = batch.len();
                    let mut targets: [&mut [F]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] =
                        core::array::from_fn(|_| -> &mut [F] { &mut [] });
                    for (target, column) in targets.iter_mut().zip(batch.iter_mut()) {
                        *target = &mut **column;
                    }
                    log19.p256.fill_arithmetic_aux_columns_v1(
                        binding.p256,
                        local,
                        &mut targets[..count],
                    )?;
                }
                output.extend(batch);
                first = end;
                continue;
            }
            if registration.segment.adapter != SegmentAdapterIdV1::Sha256CallBus {
                output.push(self.native_column_v1(layout, kind, registration, local)?);
                first += 1;
                continue;
            }
            let end = columns.end.min(match kind {
                MainTraceColumnKindV1::Base => registration.base_end()?,
                MainTraceColumnKindV1::Aux => registration.aux_end()?,
            });
            if end <= first {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let segment = usize::from(registration.segment.instance);
            let mut batch = (first..end)
                .map(|_| zeroed_main_trace_column_v1(registration.segment.trace_size()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut targets = batch
                .iter_mut()
                .map(|column| &mut **column)
                .collect::<Vec<_>>();
            match self {
                Self::Base { sha, .. } => sha
                    .get(segment)
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
                    .fill_base_columns_v1(segment, local, &mut targets)
                    .map_err(map_main_sha_source_error_v1)?,
                Self::Bound { log19, .. } => {
                    let source = log19
                        .sha_aux
                        .get(segment)
                        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
                    match kind {
                        MainTraceColumnKindV1::Base => source
                            .replay_base_columns_v1(segment, local, &mut targets)
                            .map_err(map_main_sha_source_error_v1)?,
                        MainTraceColumnKindV1::Aux => {
                            source
                                .fill_aux_columns_with_air_terminals_v1(
                                    segment,
                                    local,
                                    &mut targets,
                                )
                                .map_err(map_main_sha_source_error_v1)?;
                        }
                    }
                }
            }
            drop(targets);
            output.extend(batch);
            first = end;
        }
        if output.len() != width {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(output)
    }

    fn native_column_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        registration: RegisteredSegmentLayoutV1,
        column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let get = |source: &dyn MainTraceGroupSourceV1| match kind {
            MainTraceColumnKindV1::Base => source.native_base_column_v1(registration, column),
            MainTraceColumnKindV1::Aux => source.native_aux_column_v1(registration, column),
        };
        match self {
            Self::Base {
                assembly,
                sha,
                p256,
                projection,
                io,
            } => {
                if matches!(kind, MainTraceColumnKindV1::Aux) {
                    return Err(ZkX509StarkErrorV1::TranscriptMismatch);
                }
                match registration.trace_group {
                    0 => get(&MainP256Log5TraceGroupSourceV1::for_base_v1(layout, p256)?),
                    1 => get(&MainP256ScalarTraceGroupSourceV1::for_base_v1(
                        layout, p256,
                    )?),
                    2 => get(*projection),
                    3 => get(&MainP256Log16TraceGroupSourceV1::for_base_v1(layout, p256)?),
                    4 => get(*io),
                    5 => get(&MainLog19BaseTraceGroupSourceV1::for_main_v1(
                        layout, assembly, sha, p256,
                    )?),
                    _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
                }
            }
            Self::Bound {
                log19,
                projection,
                io,
            } => match registration.trace_group {
                0 => get(&MainP256Log5TraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                1 => get(&MainP256ScalarTraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                2 => get(*projection),
                3 => get(&MainP256Log16TraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                4 => get(*io),
                5 => get(*log19),
                _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
            },
        }
    }
}

#[cfg(test)]
#[path = "main_mask_sampling_tests.rs"]
mod mask_sampling_tests;
