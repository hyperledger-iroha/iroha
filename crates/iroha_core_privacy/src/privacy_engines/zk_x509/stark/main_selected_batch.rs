//! Eight-column selected DIF evaluation with shared public twiddles.
//!
//! Interleaved rows contain eight independent field values. The complete private
//! scratch occupies exactly the same eight full columns as the scalar evaluator;
//! the first compact output temporarily holds the public destination order.
//! The caller selects only measured public geometry. Scalar pruning remains an
//! independent arithmetic oracle and handles other CPU fallback shapes.

use super::*;

const WIDTH_V1: usize = 8;
const MAX_SELECTED_V1: usize = 272;
const PAIRS_PER_TASK_V1: usize = 1 << 12;
const PARALLEL_PAIRS_V1: usize = 1 << 16;
const PARALLEL_SUBTREE_ROWS_V1: usize = 1 << 15;
const CLEAR_CHUNK_FIELDS_V1: usize = 1 << 18;

/// Clear every live field through the ordinary volatile field eraser. Parallel
/// clearing keeps one interleaved owner from serializing eight column drops.
fn erase_batch_v1(fields: &mut [F]) {
    #[cfg(test)]
    let observation = tests::before_clear_v1(fields);
    let clear = |fields: &mut [F]| {
        for value in fields {
            value.zeroize_v1();
        }
    };
    if fields.len() >= CLEAR_CHUNK_FIELDS_V1 && rayon::current_num_threads() > 1 {
        fields.par_chunks_mut(CLEAR_CHUNK_FIELDS_V1).for_each(clear);
    } else {
        clear(fields);
    }
    #[cfg(test)]
    tests::after_clear_v1(fields, observation);
}

/// Return exactly eight compact columns without a second private field matrix.
pub(super) fn evaluate_batch_v1(
    coefficients: &[Column],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
) -> Result<Vec<Column>, AggregateStarkErrorV1> {
    evaluate_batch_with_v1(
        coefficients,
        native_log,
        common_log,
        selected,
        |_, _| Ok(()),
    )
}

fn evaluate_batch_with_v1(
    coefficients: &[Column],
    native_log: u8,
    common_log: u8,
    selected: &[usize],
    mut observe: impl FnMut(bool, &[F]) -> Result<(), AggregateStarkErrorV1>,
) -> Result<Vec<Column>, AggregateStarkErrorV1> {
    let rows = 1usize
        .checked_shl(u32::from(common_log))
        .filter(|&rows| rows <= 1 << 22)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let native_rows = 1usize
        .checked_shl(u32::from(native_log))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let root =
        goldilocks_primitive_root_v1(common_log).map_err(aggregate::map_transparent_error_v1)?;
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    if rows <= native_rows
        || coefficients.len() != WIDTH_V1
        || coefficients
            .iter()
            .any(|column| column.is_empty() || column.len() > rows)
        || selected.is_empty()
        || selected.len() > MAX_SELECTED_V1
        || selected.last().is_none_or(|&row| row >= rows)
        || selected.windows(2).any(|pair| pair[0] >= pair[1])
        || shift.pow(rows as u128) == F::ONE
        || shift.pow(native_rows as u128) == F::ONE
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if coefficients
        .iter()
        .flat_map(|column| column.iter())
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    // Drop may clear a full interleaved matrix in parallel, even when a short
    // prefix and one selected row never schedule parallel arithmetic. Complete
    // any fallible Rayon initialization before allocating/writing that matrix,
    // so unwinding cannot first initialize its cleanup thread pool.
    let _initialized_workers = rayon::current_num_threads();
    let cells = rows
        .checked_mul(WIDTH_V1)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let prefix = coefficients
        .iter()
        .map(|column| column.len())
        .max()
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let mut scratch = PrivateTableV1::new(Vec::new(), erase_batch_v1);
    scratch
        .try_reserve_exact(cells)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    if scratch.capacity() != cells {
        return Err(AggregateStarkErrorV1::AllocationFailure);
    }
    // These are the same compact output allocations returned by the scalar
    // route. Reuse the first one for sorted public coordinates until gathering.
    let mut outputs: [PrivateTableV1<F>; WIDTH_V1] =
        core::array::from_fn(|_| PrivateTableV1::new(Vec::new(), zeroize_fields_v1));
    for output in &mut outputs {
        output
            .try_reserve_exact(selected.len())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if output.capacity() != selected.len() {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        output.resize(selected.len(), F::ZERO);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(WIDTH_V1)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    if result.capacity() != WIDTH_V1 {
        return Err(AggregateStarkErrorV1::AllocationFailure);
    }
    let reversed = |row: usize| row.reverse_bits() >> (usize::BITS - u32::from(common_log));
    for (value, &row) in outputs[0].iter_mut().zip(selected) {
        *value = F(reversed(row) as u64);
    }
    outputs[0].sort_unstable_by_key(|value| value.0);
    scratch.resize(cells, F::ZERO);
    let scale = |first: usize, values: &mut [F]| {
        let mut power = shift.pow(first as u128);
        for (offset, row) in values.as_chunks_mut::<WIDTH_V1>().0.iter_mut().enumerate() {
            let degree = first + offset;
            for (destination, column) in row.iter_mut().zip(coefficients) {
                // Unequal public lengths retain the original zero suffix.
                *destination = column.get(degree).map_or(F::ZERO, |value| value.mul(power));
            }
            power = power.mul(shift);
        }
    };
    if prefix >= PARALLEL_PAIRS_V1 && rayon::current_num_threads() > 1 {
        scratch[..prefix * WIDTH_V1]
            .par_chunks_mut(PAIRS_PER_TASK_V1 * WIDTH_V1)
            .enumerate()
            .for_each(|(chunk, values)| scale(chunk * PAIRS_PER_TASK_V1, values));
    } else {
        scale(0, &mut scratch[..prefix * WIDTH_V1]);
    }
    observe(false, &scratch)?;
    prune_batch_v1(&mut scratch, 0, &outputs[0], root, prefix);
    observe(true, &scratch)?;
    for (column, output) in outputs.iter_mut().enumerate() {
        for (destination, &row) in output.iter_mut().zip(selected) {
            *destination = scratch[reversed(row) * WIDTH_V1 + column];
        }
    }
    result.extend(outputs.map(|output| Column::from_vec_v1(output.into_vec())));
    Ok(result)
}

/// Each split borrows disjoint interleaved row ranges. Only public selected
/// destinations and coefficient extents decide which butterflies are needed.
fn prune_batch_v1(values: &mut [F], offset: usize, destinations: &[F], root: F, prefix: usize) {
    let rows = values.len() / WIDTH_V1;
    debug_assert!(values.len().is_multiple_of(WIDTH_V1));
    debug_assert!(prefix > 0 && prefix <= rows);
    if destinations.is_empty() || rows == 1 {
        return;
    }
    let half = rows / 2;
    let split = destinations.partition_point(|index| index.0 < (offset + half) as u64);
    let (even_destinations, odd_destinations) = destinations.split_at(split);
    let even_needed = !even_destinations.is_empty();
    let odd_needed = !odd_destinations.is_empty();
    let (even, odd) = values.split_at_mut(half * WIDTH_V1);
    let pairs = prefix.min(half);
    let apply = |first: usize, even: &mut [F], odd: &mut [F]| {
        let mut twiddle = if odd_needed {
            root.pow(first as u128)
        } else {
            F::ONE
        };
        for (even, odd) in even
            .as_chunks_mut::<WIDTH_V1>()
            .0
            .iter_mut()
            .zip(odd.chunks_exact_mut(WIDTH_V1))
        {
            for (even, odd) in even.iter_mut().zip(odd) {
                let left = *even;
                let right = *odd;
                if even_needed {
                    *even = left.add(right);
                }
                if odd_needed {
                    *odd = left.sub(right).mul(twiddle);
                }
            }
            if odd_needed {
                twiddle = twiddle.mul(root);
            }
        }
    };
    let cells = pairs * WIDTH_V1;
    if pairs >= PARALLEL_PAIRS_V1 && rayon::current_num_threads() > 1 {
        even[..cells]
            .par_chunks_mut(PAIRS_PER_TASK_V1 * WIDTH_V1)
            .zip(odd[..cells].par_chunks_mut(PAIRS_PER_TASK_V1 * WIDTH_V1))
            .enumerate()
            .for_each(|(chunk, (even, odd))| apply(chunk * PAIRS_PER_TASK_V1, even, odd));
    } else {
        apply(0, &mut even[..cells], &mut odd[..cells]);
    }
    let child_root = root.mul(root);
    if even_needed
        && odd_needed
        && rows >= PARALLEL_SUBTREE_ROWS_V1
        && rayon::current_num_threads() > 1
    {
        rayon::join(
            || prune_batch_v1(even, offset, even_destinations, child_root, pairs),
            || prune_batch_v1(odd, offset + half, odd_destinations, child_root, pairs),
        );
    } else {
        if even_needed {
            prune_batch_v1(even, offset, even_destinations, child_root, pairs);
        }
        if odd_needed {
            prune_batch_v1(odd, offset + half, odd_destinations, child_root, pairs);
        }
    }
}

#[cfg(test)]
#[path = "main_selected_batch_tests.rs"]
mod tests;

#[cfg(test)]
pub(super) fn observe_clear_v1<T>(
    operation: impl FnOnce() -> T,
) -> (
    T,
    Vec<crate::privacy_engines::zk_x509::private_table::inspection::ErasureObservationV1>,
) {
    tests::observe_batch_clear_v1(operation)
}
