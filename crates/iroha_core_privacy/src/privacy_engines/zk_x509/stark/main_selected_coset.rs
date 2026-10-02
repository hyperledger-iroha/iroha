//! Output-pruned exact-root coset FFT with one clearing full-domain scratch.
//!
//! At each radix-two DIF split the even outputs transform `a[j]+a[j+n/2]`,
//! and the odd outputs transform `(a[j]-a[j+n/2])*root^j`, at root squared.
//! Only public selected indices select branches. Traversal is bit-reversed;
//! final gathers restore the caller's strictly increasing global-row order.

use super::*;

const PARALLEL_VALUES_V1: usize = 1 << 14;
const PAIRS_PER_TASK_V1: usize = 1 << 10;

/// Evaluate unchanged masked coefficients at the selected canonical coset rows.
/// Full input validation precedes private writes; output/index storage is reused
/// so the only field allocations are `rows` scratch plus `selected.len()` output.
pub(super) fn evaluate_v1(
    coefficients: &[F],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
) -> Result<Column, AggregateStarkErrorV1> {
    evaluate_with_v1(coefficients, native_log, common_log, selected, |_| {})
}

fn evaluate_with_v1(
    coefficients: &[F],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
    after_scale: impl FnOnce(&[F]),
) -> Result<Column, AggregateStarkErrorV1> {
    let rows = 1usize
        .checked_shl(u32::from(common_log))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let native_rows = 1usize
        .checked_shl(u32::from(native_log))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let root =
        goldilocks_primitive_root_v1(common_log).map_err(aggregate::map_transparent_error_v1)?;
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    if rows <= native_rows
        || coefficients.is_empty()
        || coefficients.len() > rows
        || selected.is_empty()
        || selected.last().is_none_or(|&row| row >= rows)
        || selected.windows(2).any(|pair| pair[0] >= pair[1])
        || shift.pow(rows as u128) == F::ONE
        || shift.pow(native_rows as u128) == F::ONE
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if coefficients
        .iter()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let mut scratch = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    scratch
        .try_reserve_exact(rows)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut output = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    output
        .try_reserve_exact(selected.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    if scratch.capacity() != rows || output.capacity() != selected.len() {
        return Err(AggregateStarkErrorV1::AllocationFailure);
    }
    // Reuse the compact output allocation for public destination coordinates.
    // Every coordinate is below rows<=2^32, hence exactly represented by F.
    // No private value is written to this allocation until pruning is complete.
    let reversed = |row: usize| row.reverse_bits() >> (usize::BITS - u32::from(common_log));
    output.extend(selected.iter().map(|&row| F(reversed(row) as u64)));
    output.sort_unstable_by_key(|value| value.0);
    scratch.resize(rows, F::ZERO);
    let mut power = F::ONE;
    for (destination, &coefficient) in scratch.iter_mut().zip(coefficients) {
        *destination = coefficient.mul(power);
        power = power.mul(shift);
    }
    after_scale(&scratch);
    prune_v1(&mut scratch, 0, &output, root);
    for (destination, &row) in output.iter_mut().zip(selected) {
        *destination = scratch[reversed(row)];
    }
    Ok(Column::from_vec_v1(output.into_vec()))
}

/// Infallible public-geometry recursion over disjoint slices of the same scratch.
fn prune_v1(values: &mut [F], offset: usize, destinations: &[F], root: F) {
    if destinations.is_empty() || values.len() == 1 {
        return;
    }
    let half = values.len() / 2;
    let split = destinations.partition_point(|index| (index.0 as usize) < offset + half);
    let (even_destinations, odd_destinations) = destinations.split_at(split);
    let even_needed = !even_destinations.is_empty();
    let odd_needed = !odd_destinations.is_empty();
    let (even, odd) = values.split_at_mut(half);
    let apply = |first: usize, even: &mut [F], odd: &mut [F]| {
        let mut twiddle = if odd_needed {
            root.pow(first as u128)
        } else {
            F::ONE
        };
        for (even, odd) in even.iter_mut().zip(odd) {
            // Read both original cells before either side can be overwritten.
            let left = *even;
            let right = *odd;
            if even_needed {
                *even = left.add(right);
            }
            if odd_needed {
                *odd = left.sub(right).mul(twiddle);
                twiddle = twiddle.mul(root);
            }
        }
    };
    if half * 2 >= PARALLEL_VALUES_V1 && rayon::current_num_threads() > 1 {
        even.par_chunks_mut(PAIRS_PER_TASK_V1)
            .zip(odd.par_chunks_mut(PAIRS_PER_TASK_V1))
            .enumerate()
            .for_each(|(chunk, (even, odd))| apply(chunk * PAIRS_PER_TASK_V1, even, odd));
    } else {
        apply(0, even, odd);
    }
    let child_root = root.mul(root);
    if even_needed && odd_needed && half * 2 >= PARALLEL_VALUES_V1 {
        rayon::join(
            || prune_v1(even, offset, even_destinations, child_root),
            || prune_v1(odd, offset + half, odd_destinations, child_root),
        );
    } else {
        if even_needed {
            prune_v1(even, offset, even_destinations, child_root);
        }
        if odd_needed {
            prune_v1(odd, offset + half, odd_destinations, child_root);
        }
    }
}

#[cfg(test)]
#[path = "main_selected_coset_tests.rs"]
mod tests;

/// Diagnostic alternative for public low-degree selected-coordinate geometry.
///
/// TODO: use same-fixture native timing to select a public dispatch threshold.
/// This remains test-only; the production pruned transform is unchanged.
/// Output chunks contain 128 public points, avoiding recursive FFT task wakes;
/// no branch or iteration count depends on a private coefficient value.
#[cfg(test)]
fn evaluate_horner_v1(
    coefficients: &[F],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
) -> Result<Column, AggregateStarkErrorV1> {
    evaluate_horner_with_v1(coefficients, native_log, common_log, selected, |_| {})
}

#[cfg(test)]
fn evaluate_horner_with_v1(
    coefficients: &[F],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
    after_evaluate: impl FnOnce(&[F]),
) -> Result<Column, AggregateStarkErrorV1> {
    let rows = 1usize
        .checked_shl(u32::from(common_log))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let native_rows = 1usize
        .checked_shl(u32::from(native_log))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let root =
        goldilocks_primitive_root_v1(common_log).map_err(aggregate::map_transparent_error_v1)?;
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    if rows <= native_rows
        || coefficients.is_empty()
        || coefficients.len() > rows
        || selected.is_empty()
        || selected.last().is_none_or(|&row| row >= rows)
        || selected.windows(2).any(|pair| pair[0] >= pair[1])
        || shift.pow(rows as u128) == F::ONE
        || shift.pow(native_rows as u128) == F::ONE
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if coefficients
        .iter()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let mut output = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    output
        .try_reserve_exact(selected.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    if output.capacity() != selected.len() {
        return Err(AggregateStarkErrorV1::AllocationFailure);
    }
    output.resize(selected.len(), F::ZERO);
    let evaluate = |indices: &[usize], values: &mut [F]| {
        for (&index, value) in indices.iter().zip(values) {
            let point = shift.mul(root.pow(index as u128));
            *value = coefficients
                .iter()
                .rev()
                .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient));
        }
    };
    const POINTS_PER_TASK_V1: usize = 128;
    if selected.len() >= POINTS_PER_TASK_V1 && rayon::current_num_threads() > 1 {
        output
            .par_chunks_mut(POINTS_PER_TASK_V1)
            .zip(selected.par_chunks(POINTS_PER_TASK_V1))
            .for_each(|(values, indices)| evaluate(indices, values));
    } else {
        evaluate(selected, &mut output);
    }
    after_evaluate(&output);
    Ok(Column::from_vec_v1(output.into_vec()))
}
