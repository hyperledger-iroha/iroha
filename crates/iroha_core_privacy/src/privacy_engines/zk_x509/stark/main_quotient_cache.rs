//! Registration-local coefficient reuse within the admitted quotient envelope.

use super::super::super::private_table::{
    PrivateTableV1, zeroize_field_rows_v1, zeroize_fields_v1,
};
use super::main_bounded_transform::{MainBoundedTransformPolicyV1, check_completion_v1};
use super::*;
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformBackendV1 as Backend, GoldilocksTransformDirectionV1 as Direction,
    GoldilocksTransformErrorV1 as TransformError, goldilocks_transform_completion_uncertain_v1,
    transform_goldilocks_columns_v1,
};

/// Public geometry fixes each retained kind prefix before private replay.
#[derive(Clone, Copy, Debug)]
pub(super) struct MainQuotientCachePlanV1 {
    pub(super) base_columns: usize,
    pub(super) aux_columns: usize,
    coefficient_count: usize,
    payload_limit: usize,
}

impl MainQuotientCachePlanV1 {
    /// Public work forecast when private coefficient allocations have their
    /// exact admitted capacities. Construction independently deducts actual
    /// capacities before retaining the fixed prefix.
    #[cfg(test)]
    pub(super) fn fixed_prefix_columns_v1(
        self,
        native_rows: usize,
        fixed_width: usize,
        stripes: usize,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let private = self
            .base_columns
            .checked_add(self.aux_columns)
            .and_then(|count| {
                self.coefficient_count
                    .checked_mul(core::mem::size_of::<F>())
                    .and_then(|bytes| bytes.checked_add(core::mem::size_of::<PrivateTableV1<F>>()))
                    .and_then(|bytes| count.checked_mul(bytes))
            })
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<MainQuotientReplayCacheV1>()))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let available = self
            .payload_limit
            .checked_sub(private)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        fixed_prefix_count_v1(native_rows, fixed_width, stripes, available)
    }
    /// Keep expensive P-256 arithmetic auxiliary replay in the same byte budget.
    /// Base columns copy already-retained cells; auxiliary columns reconstruct
    /// running products. Other registrations retain their existing priorities.
    /// Call after all additional-owner reservations, before any private replay.
    pub(super) fn prioritize_registration_v1(
        self,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let segment = registration.segment;
        if self.base_columns > segment.base_width || self.aux_columns > segment.aux_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if segment.adapter != SegmentAdapterIdV1::P256Arithmetic {
            return Ok(self);
        }
        let retained = self
            .base_columns
            .checked_add(self.aux_columns)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let aux_columns = retained.min(segment.aux_width);
        Ok(Self {
            base_columns: retained - aux_columns,
            aux_columns,
            ..self
        })
    }

    /// Charge additional simultaneous owners without increasing the admitted
    /// column count. The owner reapplies its public registration priority after
    /// narrowing. A single-stripe plan keeps its empty cache even with slack.
    pub(super) fn reserve_additional_v1(self, bytes: usize) -> Result<Self, ZkX509StarkErrorV1> {
        let payload_limit = self
            .payload_limit
            .checked_sub(bytes)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let available = payload_limit
            .checked_sub(core::mem::size_of::<MainQuotientReplayCacheV1>())
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let per_column = self
            .coefficient_count
            .checked_mul(core::mem::size_of::<F>())
            .and_then(|value| value.checked_add(core::mem::size_of::<PrivateTableV1<F>>()))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let old_count = self
            .base_columns
            .checked_add(self.aux_columns)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let count = old_count.min(available / per_column);
        let base_columns = count.min(self.base_columns);
        Ok(Self {
            base_columns,
            aux_columns: count - base_columns,
            coefficient_count: self.coefficient_count,
            payload_limit,
        })
    }

    /// Fit an ordered prefix using only public dimensions and admitted bytes.
    pub(super) fn from_budget_v1(
        base_width: usize,
        aux_width: usize,
        coefficient_count: usize,
        stripes: usize,
        budget: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if coefficient_count == 0 || stripes == 0 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let width = base_width
            .checked_add(aux_width)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let per_column = coefficient_count
            .checked_mul(core::mem::size_of::<F>())
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<PrivateTableV1<F>>()))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let available = budget
            .checked_sub(core::mem::size_of::<MainQuotientReplayCacheV1>())
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let retained = if stripes == 1 {
            0
        } else {
            width.min(available / per_column)
        };
        Ok(Self {
            base_columns: retained.min(base_width),
            aux_columns: retained.saturating_sub(base_width),
            coefficient_count,
            payload_limit: budget,
        })
    }
}

fn fixed_prefix_count_v1(
    native_rows: usize,
    fixed_width: usize,
    stripes: usize,
    available: usize,
) -> Result<usize, ZkX509StarkErrorV1> {
    if native_rows == 0
        || !native_rows.is_power_of_two()
        || native_rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
        || stripes == 0
        || !stripes.is_power_of_two()
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let per_column = native_rows
        .checked_mul(core::mem::size_of::<F>())
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<PrivateTableV1<F>>()))
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
    Ok(if stripes == 1 {
        0
    } else {
        fixed_width.min(available / per_column)
    })
}

/// Masked coefficients have clearing owners throughout construction and reuse.
pub(super) struct MainQuotientReplayCacheV1 {
    columns: Vec<PrivateTableV1<F>>,
    base_columns: usize,
    coefficient_count: usize,
    fixed_coefficients: Vec<PrivateTableV1<F>>,
    payload_limit: usize,
}

impl MainQuotientReplayCacheV1 {
    /// Replay bounded batches directly into capacity-checked clearing owners.
    pub(super) fn from_replay_v1(
        plan: MainQuotientCachePlanV1,
        mut replay: impl FnMut(
            MainTraceColumnKindV1,
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let count = plan.base_columns + plan.aux_columns;
        let mut cache = Self {
            columns: Vec::new(),
            base_columns: plan.base_columns,
            coefficient_count: plan.coefficient_count,
            fixed_coefficients: Vec::new(),
            payload_limit: plan.payload_limit,
        };
        cache
            .columns
            .try_reserve_exact(count)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        let mut payload = cache
            .columns
            .capacity()
            .checked_mul(core::mem::size_of::<PrivateTableV1<F>>())
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Self>()))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        if payload > plan.payload_limit {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for (kind, width) in [
            (MainTraceColumnKindV1::Base, plan.base_columns),
            (MainTraceColumnKindV1::Aux, plan.aux_columns),
        ] {
            for first in (0..width).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
                let end = width.min(first + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
                let batch = replay(kind, first..end)?;
                if batch.len() != end - first {
                    return Err(ZkX509StarkErrorV1::InternalInvariant);
                }
                for column in batch {
                    let column = PrivateTableV1::new(column.into_vec_v1(), zeroize_fields_v1);
                    if column.len() != plan.coefficient_count {
                        return Err(ZkX509StarkErrorV1::InternalInvariant);
                    }
                    payload = column
                        .capacity()
                        .checked_mul(core::mem::size_of::<F>())
                        .and_then(|bytes| payload.checked_add(bytes))
                        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
                    if payload > plan.payload_limit {
                        return Err(ZkX509StarkErrorV1::ProofTooLarge);
                    }
                    cache.columns.push(column);
                }
            }
        }
        Ok(cache)
    }

    /// Count both caches by actual capacity under their single registration
    /// allowance. The stored budget and both owner headers are included.
    pub(super) fn allocated_payload_v1(&self) -> Result<usize, ZkX509StarkErrorV1> {
        let headers = self
            .columns
            .capacity()
            .checked_add(self.fixed_coefficients.capacity())
            .and_then(|count| count.checked_mul(core::mem::size_of::<PrivateTableV1<F>>()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Self>()))
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        self.columns
            .iter()
            .chain(&self.fixed_coefficients)
            .try_fold(headers, |total, column| {
                column
                    .capacity()
                    .checked_mul(core::mem::size_of::<F>())
                    .and_then(|bytes| total.checked_add(bytes))
                    .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
            })
    }

    /// Retain a public prefix only after the private cache's actual capacity
    /// has been deducted. No source, replay, runtime or device allowance is
    /// released, and a one-stripe registration never copies fixed coefficients.
    pub(super) fn with_fixed_coefficients_v1(
        mut self,
        fixed: &[Vec<F>],
        native_rows: usize,
        stripes: usize,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if native_rows == 0
            || !native_rows.is_power_of_two()
            || native_rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1
            || stripes == 0
            || !stripes.is_power_of_two()
            || fixed.is_empty()
            || !self.fixed_coefficients.is_empty()
            || fixed.iter().any(|column| {
                column.len() != native_rows
                    || column.iter().any(|value| F::canonical(value.0).is_none())
            })
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let available = self
            .payload_limit
            .checked_sub(self.allocated_payload_v1()?)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let count = fixed_prefix_count_v1(native_rows, fixed.len(), stripes, available)?;
        self.fixed_coefficients
            .try_reserve_exact(count)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if self.fixed_coefficients.capacity() != count
            || self.allocated_payload_v1()? > self.payload_limit
        {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for original in &fixed[..count] {
            let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
            column
                .try_reserve_exact(native_rows)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            if column.capacity() != native_rows
                || self
                    .allocated_payload_v1()?
                    .checked_add(column.capacity() * core::mem::size_of::<F>())
                    .is_none_or(|bytes| bytes > self.payload_limit)
            {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            column.extend_from_slice(original);
            self.fixed_coefficients.push(column);
        }
        Ok(self)
    }

    /// Borrow the immutable public prefix while both caches remain in this owner.
    pub(super) fn fixed_coefficients_v1(&self) -> &[PrivateTableV1<F>] {
        &self.fixed_coefficients
    }

    /// Reuse the prefix and replay only the uncached suffix, in canonical order.
    pub(super) fn evaluate_v1(
        &self,
        kind: MainTraceColumnKindV1,
        width: usize,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        policy: MainBoundedTransformPolicyV1,
        replay: impl FnMut(
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    ) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
        self.evaluate_with_v1(
            kind,
            width,
            stripe,
            policy,
            replay,
            |words, root, direction| {
                transform_goldilocks_columns_v1(
                    words,
                    root,
                    direction,
                    fastpq_prover::ExecutionMode::Auto,
                )
            },
            goldilocks_transform_completion_uncertain_v1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_with_v1(
        &self,
        kind: MainTraceColumnKindV1,
        width: usize,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        policy: MainBoundedTransformPolicyV1,
        mut replay: impl FnMut(
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
        mut transform: impl FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
        mut uncertain: impl FnMut() -> bool,
    ) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
        check_completion_v1(uncertain())?;
        let cached = match kind {
            MainTraceColumnKindV1::Base => &self.columns[..self.base_columns],
            MainTraceColumnKindV1::Aux => &self.columns[self.base_columns..],
        };
        if cached.len() > width {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        let mut output = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1::<Vec<F>>);
        output
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        // The registration charge covers exactly width complete output columns;
        // their headers are reserved by for_quotient_layout_v1. Refuse any excess
        // capacity rather than borrowing the next batch's device allowance.
        if output.capacity() != width {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let mut append = |coefficients: &[&[F]]| -> Result<(), ZkX509StarkErrorV1> {
            check_completion_v1(uncertain())?;
            let start = output.len();
            for _ in coefficients {
                let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
                column
                    .try_reserve_exact(stripe.rows)
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                if column.capacity() != stripe.rows {
                    return Err(ZkX509StarkErrorV1::ProofTooLarge);
                }
                column.resize(stripe.rows, F::ZERO);
                output.push(column.into_vec());
            }
            let batch = &mut output[start..];
            batch
                .par_iter_mut()
                .zip(coefficients.par_iter())
                .try_for_each(|(values, coefficients)| stripe.fold_into_v1(coefficients, values))?;
            policy.forward_with_v1(batch, stripe.root, &mut transform, &mut uncertain)
        };
        // Stack-only borrowed views add no private field matrix or heap batch.
        for batch in cached.chunks(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let mut coefficients: [&[F]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] =
                [&[]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1];
            for (target, source) in coefficients.iter_mut().zip(batch) {
                *target = source;
            }
            append(&coefficients[..batch.len()])?;
        }
        for first in (cached.len()..width).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let end = width.min(first + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
            let batch = replay(first..end)?;
            if batch.len() != end - first
                || batch
                    .iter()
                    .any(|column| column.len() != self.coefficient_count)
            {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            if batch.capacity() != end - first
                || batch
                    .iter()
                    .any(|column| column.0.capacity() != self.coefficient_count)
            {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            let mut coefficients: [&[F]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] =
                [&[]; aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1];
            for (target, source) in coefficients.iter_mut().zip(&batch) {
                *target = source;
            }
            append(&coefficients[..batch.len()])?;
        }
        check_completion_v1(uncertain())?;
        Ok(ZeroizingBaseColumnsV1(output.into_vec()))
    }
}

#[cfg(test)]
#[path = "main_quotient_cache_tests.rs"]
mod tests;
