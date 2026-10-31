//! Bounded native inverse and quotient forward publication into existing field slices.
//!
//! The final field owners keep their pointers and capacities. Production Metal
//! owns only its existing bounded shared staging allocation; it validates the
//! completed result before publishing. Test word callbacks use the former
//! adapter as an independent fault-injection and arithmetic oracle.

use super::main_bounded_transform::MainBoundedTransformPolicyV1;
use super::*;
use crate::privacy_engines::transparent_stark::{goldilocks_words_mut_v1, goldilocks_words_v1};
#[cfg(test)]
use fastpq_prover::goldilocks_transform::GoldilocksTransformBackendV1 as Backend;
use fastpq_prover::goldilocks_transform::{
    GoldilocksTransformDirectionV1 as Direction, GoldilocksTransformErrorV1 as TransformError,
    MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1, PreparedGoldilocksColumnsOutcomeV1 as Outcome,
    PreparedGoldilocksColumnsV1, metal_goldilocks_forward_extra_payload_v1,
    metal_goldilocks_transform_extra_payload_v1, prepare_goldilocks_columns_v1,
    prepare_goldilocks_forward_columns_v1, prepared_goldilocks_columns_payload_bound_v1,
    prepared_goldilocks_forward_payload_bound_v1,
};

/// The closed native interpolation seam; production has one exact implementation.
pub(super) trait MainNativeTransformV1 {
    fn inverse_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1>;
}

/// Select the production prepared transaction and unchanged deterministic CPU fallback.
pub(super) struct ExactNativeTransformV1;

/// CPU selection is decided before publication by public admission or Auto policy.
pub(super) enum MainNativePreparedV1 {
    Published,
    Cpu,
}

impl MainNativeTransformV1 for ExactNativeTransformV1 {
    fn inverse_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        policy.inverse_native_exact_v1(columns, root, uncertain)
    }
}

/// Preserve existing word-closure tests without a second production path.
#[cfg(test)]
pub(super) struct WordInjectionV1<T>(pub(super) T);

#[cfg(test)]
impl<T> MainNativeTransformV1 for WordInjectionV1<T>
where
    T: FnMut(&mut [Vec<u64>], u64, Direction) -> Result<Backend, TransformError>,
{
    fn inverse_v1(
        &mut self,
        policy: MainBoundedTransformPolicyV1,
        columns: &mut [Vec<F>],
        root: F,
        uncertain: &mut dyn FnMut() -> bool,
    ) -> Result<(), ZkX509StarkErrorV1> {
        policy.inverse_with_v1(columns, root, &mut self.0, uncertain)
    }
}

/// Charge the complete token and both bounded borrowed-view arrays, including
/// scalar/header workspace, in addition to the original Metal allowance.
pub(super) const fn metadata_payload_v1() -> usize {
    metadata_for_columns_v1::<{ aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1 }>()
}

pub(super) const fn forward_metadata_payload_v1() -> usize {
    metadata_for_columns_v1::<MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1>()
}

const fn metadata_for_columns_v1<const MAX: usize>() -> usize {
    core::mem::size_of::<PreparedGoldilocksColumnsV1>()
        + core::mem::size_of::<Outcome>()
        + core::mem::size_of::<[&[u64]; MAX]>()
        + core::mem::size_of::<[&mut [u64]; MAX]>()
        + 16 * core::mem::size_of::<usize>()
}

/// Return CPU when public metadata admission or the shared Auto policy requires it.
/// An error never publishes any part of this native batch.
pub(super) fn prepare_and_publish_v1(
    columns: &mut [Vec<F>],
    root: F,
    available: usize,
    uncertain: &mut impl FnMut() -> bool,
) -> Result<MainNativePreparedV1, ZkX509StarkErrorV1> {
    prepare_and_publish_with_v1(columns, root, available, uncertain, |views, root| {
        prepare_goldilocks_columns_v1(
            views,
            root,
            Direction::Inverse,
            fastpq_prover::ExecutionMode::Auto,
        )
    })
}

fn prepare_and_publish_with_v1(
    columns: &mut [Vec<F>],
    root: F,
    available: usize,
    uncertain: &mut impl FnMut() -> bool,
    prepare: impl FnOnce(&[&[u64]], u64) -> Result<Outcome, TransformError>,
) -> Result<MainNativePreparedV1, ZkX509StarkErrorV1> {
    prepare_with_limit_v1::<{ aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1 }>(
        columns,
        root,
        available,
        uncertain,
        required_payload_v1,
        prepared_goldilocks_columns_payload_bound_v1,
        prepare,
    )
}

/// The forward-only contract borrows up to sixteen already charged final columns.
pub(super) fn prepare_forward_and_publish_v1(
    columns: &mut [Vec<F>],
    root: F,
    available: usize,
    uncertain: &mut impl FnMut() -> bool,
) -> Result<MainNativePreparedV1, ZkX509StarkErrorV1> {
    prepare_forward_and_publish_with_v1(columns, root, available, uncertain, |views, root| {
        prepare_goldilocks_forward_columns_v1(views, root, fastpq_prover::ExecutionMode::Auto)
    })
}

fn prepare_forward_and_publish_with_v1(
    columns: &mut [Vec<F>],
    root: F,
    available: usize,
    uncertain: &mut impl FnMut() -> bool,
    prepare: impl FnOnce(&[&[u64]], u64) -> Result<Outcome, TransformError>,
) -> Result<MainNativePreparedV1, ZkX509StarkErrorV1> {
    prepare_with_limit_v1::<MAX_GOLDILOCKS_PREPARED_FORWARD_COLUMNS_V1>(
        columns,
        root,
        available,
        uncertain,
        required_forward_payload_v1,
        prepared_goldilocks_forward_payload_bound_v1,
        prepare,
    )
}

fn prepare_with_limit_v1<const MAX: usize>(
    columns: &mut [Vec<F>],
    root: F,
    available: usize,
    uncertain: &mut impl FnMut() -> bool,
    required: fn(usize, usize) -> Result<usize, ZkX509StarkErrorV1>,
    token_bound: fn(usize, usize) -> Result<usize, TransformError>,
    prepare: impl FnOnce(&[&[u64]], u64) -> Result<Outcome, TransformError>,
) -> Result<MainNativePreparedV1, ZkX509StarkErrorV1> {
    main_bounded_transform::check_completion_v1(uncertain())?;
    let rows = columns.first().map_or(0, Vec::len);
    let count = columns.len();
    if count == 0 || count > MAX || columns.iter().any(|column| column.len() != rows) {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    if required(rows, count)? > available {
        // Tiny exact-budget word policies can admit their old word matrix but
        // not both fixed view arrays. Keep their deterministic CPU fallback;
        // never enlarge the original width or allocate an uncharged token.
        return Ok(MainNativePreparedV1::Cpu);
    }
    let token_limit = token_bound(rows, count).map_err(map_error_v1)?;
    let prepared = {
        let mut views: [&[u64]; MAX] = [&[]; MAX];
        for (view, column) in views.iter_mut().zip(columns.iter()) {
            *view = goldilocks_words_v1(column);
        }
        prepare(&views[..count], root.0)
    };
    // The shared Auto result has already inspected sticky/returned uncertainty.
    // Preserve the caller's additional completion seam before any publication
    // or CPU fallback can begin, while the prepared token still owns its pages.
    main_bounded_transform::check_completion_v1(uncertain())?;
    let token = match prepared.map_err(map_error_v1)? {
        Outcome::Cpu => return Ok(MainNativePreparedV1::Cpu),
        Outcome::Metal(token) => token,
    };
    if token.rows_v1() != rows
        || token.columns_v1() != count
        || token.allocated_payload_bytes_v1().map_err(map_error_v1)? > token_limit
    {
        return Err(ZkX509StarkErrorV1::ProofTooLarge);
    }
    let mut destinations: [&mut [u64]; MAX] = core::array::from_fn(|_| -> &mut [u64] { &mut [] });
    for (target, column) in destinations.iter_mut().zip(columns.iter_mut()) {
        *target = goldilocks_words_mut_v1(column);
    }
    // The consuming publish performs all remaining status/shape/canonicality
    // refusals before its first write, then clears/releases shared backing. No
    // mask replacement or later source callback can overlap that token owner.
    token
        .publish_into_v1(&mut destinations[..count])
        // This token type can publish only Metal. No new fallible check runs
        // after it has copied into the existing destination allocations.
        .map(|_| MainNativePreparedV1::Published)
        .map_err(map_error_v1)
}

pub(super) fn required_payload_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, ZkX509StarkErrorV1> {
    if rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    metal_goldilocks_transform_extra_payload_v1(rows, columns)
        .map_err(map_error_v1)?
        .checked_add(metadata_payload_v1())
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
}

pub(super) fn required_forward_payload_v1(
    rows: usize,
    columns: usize,
) -> Result<usize, ZkX509StarkErrorV1> {
    if rows > 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    metal_goldilocks_forward_extra_payload_v1(rows, columns)
        .map_err(map_error_v1)?
        .checked_add(forward_metadata_payload_v1())
        .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
}

#[cfg(test)]
#[path = "main_native_transform_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "main_forward_transform_tests.rs"]
mod forward_tests;

fn map_error_v1(error: TransformError) -> ZkX509StarkErrorV1 {
    match error {
        TransformError::CompletionUncertain => ZkX509StarkErrorV1::AcceleratorCompletionUncertain,
        TransformError::NonCanonicalInput | TransformError::NonCanonicalOutput => {
            ZkX509StarkErrorV1::NonCanonicalField
        }
        _ => ZkX509StarkErrorV1::ProfileMismatch,
    }
}
