//! Clearing evaluated-column ownership for initial joined commitments.

use super::*;
use fastpq_prover::goldilocks_transform::{
    CompletedGoldilocksColumnsV1, prepared_goldilocks_columns_payload_bound_v1,
};
use std::sync::Arc;

mod sealed {
    /// Only the two reviewed clearing owners can enter generic initial replay.
    pub trait Sealed {}
}

/// Complete initial-output ownership, with exact publicly admitted field extent.
pub(crate) trait InitialEvaluationV1: sealed::Sealed + AsRef<[F]> + Send + Sync {
    /// Check actual backing capacity, rather than accepting an oversized owner.
    fn exact_initial_extent_v1(&self, rows: usize) -> bool;
    /// Consume successful outputs, preserving ordinary Drop on every refusal.
    fn erase_completed_v1(self)
    where
        Self: Sized,
    {
        drop(self);
    }
}
impl sealed::Sealed for ZeroizingFieldColumnV1 {}
impl InitialEvaluationV1 for ZeroizingFieldColumnV1 {
    fn exact_initial_extent_v1(&self, rows: usize) -> bool {
        self.len() == rows && self.0.capacity() == rows
    }
}

/// Either an original clearing CPU output, or one slice of completed shared pages.
/// No mutable borrow or device dispatch can be derived from a completed variant.
pub(crate) struct InitialEvaluatedColumnV1 {
    inner: InitialKindV1,
}
enum InitialKindV1 {
    Field(ZeroizingFieldColumnV1),
    Completed {
        owner: Arc<CompletedGoldilocksColumnsV1>,
        index: usize,
    },
}
impl InitialEvaluatedColumnV1 {
    /// Transfer an original clearing field owner without copying field values.
    pub(crate) fn from_field_v1(column: ZeroizingFieldColumnV1) -> Self {
        Self {
            inner: InitialKindV1::Field(column),
        }
    }

    /// Transfer one fully validated owner into ordered immutable column handles.
    pub(crate) fn from_completed_v1(
        owner: CompletedGoldilocksColumnsV1,
    ) -> Result<Vec<Self>, AggregateStarkErrorV1> {
        let rows = owner.rows_v1();
        // This owner's field-cell budget covers whole pages, including short
        // final batches. Smaller domains keep ordinary clearing CPU fields.
        if rows < fastpq_prover::goldilocks_transform::MIN_UNPADDED_GOLDILOCKS_COLUMN_ROWS_V1 {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let columns = owner.columns_v1();
        let admitted = prepared_goldilocks_columns_payload_bound_v1(rows, columns)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
        if owner
            .allocated_payload_bytes_v1()
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            > admitted
        {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(columns)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if result.capacity() != columns {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        let owner = Arc::new(owner);
        for index in 0..columns {
            result.push(Self {
                inner: InitialKindV1::Completed {
                    owner: Arc::clone(&owner),
                    index,
                },
            });
        }
        Ok(result)
    }
}
impl AsRef<[F]> for InitialEvaluatedColumnV1 {
    fn as_ref(&self) -> &[F] {
        match &self.inner {
            InitialKindV1::Field(column) => column,
            InitialKindV1::Completed { owner, index } => {
                crate::privacy_engines::transparent_stark::goldilocks_fields_v1(
                    owner
                        .column_v1(*index)
                        .expect("sealed completed column index"),
                )
            }
        }
    }
}
impl sealed::Sealed for InitialEvaluatedColumnV1 {}
impl InitialEvaluationV1 for InitialEvaluatedColumnV1 {
    fn erase_completed_v1(self) {
        match self.inner {
            InitialKindV1::Field(column) => drop(column),
            InitialKindV1::Completed { owner, .. } => {
                // Exactly one succeeds when all shared column handles use this
                // consumption path, including concurrent final-handle cleanup.
                if let Some(owner) = Arc::into_inner(owner) {
                    owner.erase_completed_v1();
                }
            }
        }
    }

    fn exact_initial_extent_v1(&self, rows: usize) -> bool {
        match &self.inner {
            InitialKindV1::Field(column) => column.exact_initial_extent_v1(rows),
            InitialKindV1::Completed { owner, index } => {
                owner.rows_v1() == rows && *index < owner.columns_v1()
            }
        }
    }
}

/// Every possible live16-column backing header and Arc, descriptor/iterator and
/// borrowed-input scratch. Field pages replace already charged output cells.
/// Charging one entire backing header per column overbounds shared batches.
pub(crate) const fn metadata_payload_v1() -> usize {
    const {
        assert!(size_of::<ZeroizingFieldColumnV1>() <= size_of::<InitialEvaluatedColumnV1>());
    }
    16 * (256
        + size_of::<CompletedGoldilocksColumnsV1>()
        + 2 * size_of::<usize>()
        + size_of::<InitialEvaluatedColumnV1>())
        + 4 * (size_of::<Vec<InitialEvaluatedColumnV1>>()
            + size_of::<std::vec::IntoIter<InitialEvaluatedColumnV1>>())
        + 2 * MASKED_TRACE_LDE_COLUMN_BATCH_V1 * size_of::<InitialEvaluatedColumnV1>()
        + 2 * MASKED_TRACE_LDE_COLUMN_BATCH_V1 * size_of::<ZeroizingFieldColumnV1>()
        + 2 * MASKED_TRACE_LDE_COLUMN_BATCH_V1 * size_of::<&[u64]>()
}

/// Successful callers may parallelize clearing; failure/unwind uses ordinary Drop.
pub(crate) fn drop_completed_v1<C: InitialEvaluationV1>(columns: Vec<C>) {
    if columns.len() > 1
        && columns
            .iter()
            .any(|column| column.as_ref().len() >= 1 << 16)
        && rayon::current_num_threads() > 1
    {
        columns.into_par_iter().for_each(C::erase_completed_v1);
    } else {
        for column in columns {
            column.erase_completed_v1();
        }
    }
}
