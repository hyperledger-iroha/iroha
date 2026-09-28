//! Registration-local coefficient reuse within the admitted quotient envelope.

use super::super::super::private_table::{PrivateTableV1, zeroize_fields_v1};
use super::*;

/// Public geometry fixes the retained base-then-auxiliary prefix before replay.
#[derive(Clone, Copy, Debug)]
pub(super) struct MainQuotientCachePlanV1 {
    pub(super) base_columns: usize,
    pub(super) aux_columns: usize,
    coefficient_count: usize,
    payload_limit: usize,
}

impl MainQuotientCachePlanV1 {
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

/// Masked coefficients have clearing owners throughout construction and reuse.
pub(super) struct MainQuotientReplayCacheV1 {
    columns: Vec<PrivateTableV1<F>>,
    base_columns: usize,
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

    /// Reuse the prefix and replay only the uncached suffix, in canonical order.
    pub(super) fn evaluate_v1(
        &self,
        kind: MainTraceColumnKindV1,
        width: usize,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
        mut replay: impl FnMut(
            core::ops::Range<usize>,
        ) -> Result<Vec<ZeroizingMainTraceColumnV1>, ZkX509StarkErrorV1>,
    ) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
        let cached = match kind {
            MainTraceColumnKindV1::Base => &self.columns[..self.base_columns],
            MainTraceColumnKindV1::Aux => &self.columns[self.base_columns..],
        };
        if cached.len() > width {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        let mut output = ZeroizingBaseColumnsV1(Vec::new());
        output
            .0
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for batch in cached.chunks(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let values = batch
                .par_iter()
                .map(|column| stripe.evaluate_v1(column))
                .collect::<Result<Vec<_>, _>>()?;
            for column in values {
                output.0.push(column.into_vec_v1());
            }
        }
        for first in (cached.len()..width).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let end = width.min(first + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1);
            let batch = replay(first..end)?;
            if batch.len() != end - first {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            let values = batch
                .par_iter()
                .map(|column| stripe.evaluate_v1(column))
                .collect::<Result<Vec<_>, _>>()?;
            for column in values {
                output.0.push(column.into_vec_v1());
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "main_quotient_cache_tests.rs"]
mod tests;
