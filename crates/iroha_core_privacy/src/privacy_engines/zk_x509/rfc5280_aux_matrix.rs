//! Parallel RFC auxiliary groups into the already admitted final owner.
//!
//! Each disjoint group follows its original complete ordered recurrence and
//! terminal checks. Group errors are selected by public order after every
//! worker joins; failure or unwind clears the complete destination matrix.

use super::*;
use rayon::prelude::*;

const WIDTH: usize = ZK_X509_RFC5280_STARK_AUX_WIDTH_V1;
const GROUPS: usize = WIDTH.div_ceil(BATCH);

/// Charge every public group's complete bounded replay scratch and borrowed
/// headers even when fewer workers execute concurrently. Outputs already belong
/// to the caller; no worker allocates another private matrix or field column.
pub(super) const fn scratch_payload_bytes_v1() -> usize {
    GROUPS
        * (inverse_window::scratch_payload_bytes_v1()
            + core::mem::size_of::<&mut [&mut [F]]>()
            + core::mem::size_of::<Option<(usize, ZkX509Rfc5280StarkErrorV1)>>()
            + 4 * core::mem::size_of::<usize>())
        + core::mem::size_of::<[&mut [F]; WIDTH]>()
        + core::mem::size_of::<OutputGuardV1<'static, 'static>>()
}

pub(super) fn fill_matrix_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    der: &ZkX509DerStarkChallengesV1,
    challenges: &ZkX509Rfc5280StarkChallengesV1,
    outputs: &mut [&mut [F]],
    centers: &ZkX509ShaUnionCentersV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    der.validate()?;
    challenges.validate()?;
    centers.validate_v1()?;
    fill_groups_v1(
        ZK_X509_RFC5280_STARK_TRACE_SIZE_V1,
        outputs,
        |first, targets| super::fill_columns_v1(material, der, challenges, first, targets, centers),
    )
}

fn fill_groups_v1(
    rows: usize,
    outputs: &mut [&mut [F]],
    replay: impl Fn(usize, &mut [&mut [F]]) -> Result<(), ZkX509Rfc5280StarkErrorV1> + Sync,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if rows == 0
        || rows > ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
        || outputs.len() != WIDTH
        || outputs.iter().any(|output| output.len() != rows)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let mut output = OutputGuardV1 {
        outputs,
        committed: false,
    };
    // There is no fail-fast cancellation: all groups finish their original
    // checks, then the earliest public group error is selected deterministically.
    // Rayon also joins running groups before propagating an unwind to the guard.
    let failed = output
        .outputs
        .par_chunks_mut(BATCH)
        .enumerate()
        .map(|(group, targets)| {
            replay(group * BATCH, targets)
                .err()
                .map(|error| (group, error))
        })
        .reduce(
            || None,
            |left, right| match (left, right) {
                (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
                (left, right) => left.or(right),
            },
        );
    if let Some((_, error)) = failed {
        return Err(error);
    }
    output.committed = true;
    Ok(())
}

#[cfg(test)]
fn fill_with_v1(
    rows: usize,
    outputs: &mut [&mut [F]],
    der: &ZkX509DerStarkChallengesV1,
    challenges: &ZkX509Rfc5280StarkChallengesV1,
    centers: &ZkX509ShaUnionCentersV1,
    row_at: impl Fn(usize, usize) -> Result<RowContextV1, ZkX509Rfc5280StarkErrorV1> + Sync,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    der.validate()?;
    challenges.validate()?;
    centers.validate_v1()?;
    fill_groups_v1(rows, outputs, |first, targets| {
        inverse_window::fill_with_v1(rows, first, targets, der, challenges, centers, |index| {
            row_at(first / BATCH, index)
        })
    })
}

#[cfg(test)]
#[path = "rfc5280_aux_matrix_tests.rs"]
mod tests;
