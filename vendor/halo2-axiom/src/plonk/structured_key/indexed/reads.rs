//! Bounded indexed reads of the original structured frame's directly encoded polynomials.
//!
//! This parser is crate-private and grants no artifact or proof authority. The future
//! consuming key owner must inseparably retain this index and its authenticated reader.
//! TODO: connect that owner through Core and the stored prover; do not pair an index
//! with caller-selected replacement bytes or expose this as generic proof admission.

use super::*;
use std::{
    io::{Seek, SeekFrom},
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

/// An original directly encoded key polynomial, without accepting caller-chosen offsets.
#[derive(Clone, Copy, Debug)]
pub(crate) enum IndexedKeyPolynomialV1 {
    /// One of the original l0, l_last, l_active_row coefficient arrays, in that order.
    MaskCoefficient(usize),
    /// Original fixed-column Lagrange values, including compressed selector columns.
    FixedLagrange(usize),
    /// Original sigma values reconstructed from checked permutation target IDs.
    PermutationLagrange(usize),
}

// Synthetic test boundary only: this models an unwind after successful native decoding.
// It is not a naturally reachable cryptographic failure or an allocator-abort simulation.
#[cfg(test)]
std::thread_local! {
    static COEFFICIENT_TRANSFORM_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Non-secret observations carried by the synthetic transform-boundary panic.
#[cfg(test)]
#[derive(Debug)]
pub(in crate::plonk::structured_key) struct CoefficientTransformBoundaryPanic {
    pub(in crate::plonk::structured_key) rows: usize,
    pub(in crate::plonk::structured_key) nonzero: usize,
}

/// Arm one synthetic boundary panic and always disarm it when this scope exits.
#[cfg(test)]
pub(in crate::plonk::structured_key) fn with_coefficient_transform_panic<T>(
    operation: impl FnOnce() -> T,
) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COEFFICIENT_TRANSFORM_PANIC.with(|armed| armed.set(false));
        }
    }
    assert!(
        !COEFFICIENT_TRANSFORM_PANIC.with(|armed| armed.replace(true)),
        "coefficient transform panic hook is already armed"
    );
    let _reset = Reset;
    operation()
}

#[cfg(test)]
fn coefficient_transform_boundary<F: PrimeField>(values: &[F]) {
    // Disarm before panicking so a caught unwind cannot poison a later parser operation.
    if COEFFICIENT_TRANSFORM_PANIC.with(|armed| armed.replace(false)) {
        std::panic::panic_any(CoefficientTransformBoundaryPanic {
            rows: values.len(),
            nonzero: values.iter().filter(|value| **value != F::ZERO).count(),
        });
    }
}

struct Destination<'a, F: PrimeField> {
    values: &'a mut [F],
    complete: bool,
}
impl<F: PrimeField> Drop for Destination<'_, F> {
    fn drop(&mut self) {
        if !self.complete {
            for value in self.values.iter_mut() {
                // SAFETY: initialized, exclusively borrowed Copy field values admit ZERO.
                unsafe { ptr::write_volatile(value, F::ZERO) };
            }
            compiler_fence(Ordering::SeqCst);
        }
    }
}

fn subrange(range: CheckedRange, offset: u64, bytes: u64, frame: u64) -> io::Result<u64> {
    offset
        .checked_add(bytes)
        .filter(|end| *end <= range.length)
        .ok_or_else(|| invalid("indexed read exceeds polynomial range"))?;
    let at = range
        .offset
        .checked_add(offset)
        .ok_or_else(|| invalid("indexed read offset overflow"))?;
    at.checked_add(bytes)
        .filter(|end| *end <= frame)
        .ok_or_else(|| invalid("indexed read exceeds original frame"))?;
    Ok(at)
}

fn seek<R: Seek>(reader: &mut R, at: u64) -> io::Result<()> {
    if reader.seek(SeekFrom::Start(at))? != at {
        return Err(invalid("indexed source seek returned a different position"));
    }
    Ok(())
}

impl<C: SerdeCurveAffine> IndexedStructuredProvingKeyV1<C>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    /// Fill exactly one coefficient column from this index's original native encoding.
    ///
    /// The caller retains the sole full-column allocation. Fixed and permutation values
    /// use the existing baseline in-place inverse transform; masks already encode coefficients.
    /// The destination remains guarded across both decoding and transformation. Invalid shape,
    /// read/decode failure or unwind clears it, including elements beyond a rejected row count.
    /// Public FFT twiddles and the original index/domain remain additional memory costs.
    /// This does not authenticate a source or establish a stored-polynomial receipt.
    pub(crate) fn copy_coefficient_column<R: Read + Seek>(
        &self,
        reader: &mut R,
        polynomial: IndexedKeyPolynomialV1,
        output: &mut [C::Scalar],
    ) -> io::Result<()> {
        let mut destination = Destination {
            values: output,
            complete: false,
        };
        if destination.values.len() != self.metadata.rows {
            return Err(invalid(
                "indexed coefficient destination has the wrong row count",
            ));
        }
        self.copy_native_interval(reader, polynomial, 0, destination.values)?;
        if !matches!(polynomial, IndexedKeyPolynomialV1::MaskCoefficient(_)) {
            #[cfg(test)]
            coefficient_transform_boundary(destination.values);
            self.vk
                .domain
                .stored_column_transform_in_place(destination.values, true, None);
        }
        destination.complete = true;
        Ok(())
    }

    /// Fill a requested native-basis interval with constant-size decoding scratch.
    ///
    /// Offsets are frame-relative (the supplied source must start at the same frame).
    /// The caller's original authenticated source owns freshness and failure poisoning;
    /// this operation cannot establish source identity. Invalid request, decoding error,
    /// failed read/seek or unwind clears the entire destination. Empty valid intervals
    /// perform no I/O. No coefficient transform or full-column allocation occurs here.
    pub(crate) fn copy_native_interval<R: Read + Seek>(
        &self,
        reader: &mut R,
        polynomial: IndexedKeyPolynomialV1,
        start: usize,
        output: &mut [C::Scalar],
    ) -> io::Result<()> {
        let destination = Destination {
            values: output,
            complete: false,
        };
        let n = self.metadata.rows;
        let end = start
            .checked_add(destination.values.len())
            .filter(|end| *end <= n)
            .ok_or_else(|| invalid("indexed polynomial row range is invalid"))?;
        let width = <C::Scalar as PrimeField>::Repr::default().as_ref().len() as u64;
        let scalar_offset = (start as u64)
            .checked_mul(width)
            .ok_or_else(|| invalid("indexed scalar offset overflow"))?;
        let scalar_bytes = (destination.values.len() as u64)
            .checked_mul(width)
            .ok_or_else(|| invalid("indexed scalar byte count overflow"))?;
        let frame = self.metadata.frame_bytes;
        match polynomial {
            IndexedKeyPolynomialV1::MaskCoefficient(mask) => {
                let range = *self
                    .metadata
                    .masks
                    .get(mask)
                    .ok_or_else(|| invalid("indexed mask is invalid"))?;
                let at = subrange(range, scalar_offset, scalar_bytes, frame)?;
                if start != end {
                    seek(reader, at)?;
                    for value in destination.values.iter_mut() {
                        *value = read_scalar(reader)?;
                    }
                }
            }
            IndexedKeyPolynomialV1::FixedLagrange(column) => {
                let record = self
                    .metadata
                    .fixed
                    .get(column)
                    .ok_or_else(|| invalid("indexed fixed column is invalid"))?;
                match record.mode {
                    CONSTANT => {
                        let at = subrange(record.payload, 0, width, frame)?;
                        if start != end {
                            seek(reader, at)?;
                            destination.values.fill(read_scalar(reader)?);
                        }
                    }
                    BITSET => {
                        let first = start / 8;
                        let last = end.div_ceil(8);
                        let bytes = if start == end { 0 } else { last - first };
                        let at = subrange(record.payload, first as u64, bytes as u64, frame)?;
                        if start != end {
                            seek(reader, at)?;
                            for byte_index in first..last {
                                let mut byte = [0];
                                reader.read_exact(&mut byte)?;
                                if byte_index == n / 8 && n % 8 != 0 && byte[0] >> (n % 8) != 0 {
                                    return Err(invalid("nonzero indexed bitset padding"));
                                }
                                let from = start.max(byte_index * 8);
                                let to = end.min(byte_index * 8 + 8);
                                for row in from..to {
                                    destination.values[row - start] =
                                        if byte[0] >> (row % 8) & 1 == 0 {
                                            C::Scalar::ZERO
                                        } else {
                                            C::Scalar::ONE
                                        };
                                }
                            }
                        }
                    }
                    RAW => {
                        let at = subrange(record.payload, scalar_offset, scalar_bytes, frame)?;
                        if start != end {
                            seek(reader, at)?;
                            for value in destination.values.iter_mut() {
                                *value = read_scalar(reader)?;
                            }
                        }
                    }
                    _ => return Err(invalid("unknown indexed fixed mode")),
                }
            }
            IndexedKeyPolynomialV1::PermutationLagrange(column) => {
                if n == 0 || column >= self.metadata.permutation_columns {
                    return Err(invalid("indexed permutation column is invalid"));
                }
                let cells = n
                    .checked_mul(self.metadata.permutation_columns)
                    .ok_or_else(|| invalid("indexed permutation shape overflow"))?;
                let first = column
                    .checked_mul(n)
                    .and_then(|v| v.checked_add(start))
                    .ok_or_else(|| invalid("indexed permutation offset overflow"))?;
                let offset = (first as u64)
                    .checked_mul(4)
                    .ok_or_else(|| invalid("indexed target offset overflow"))?;
                let bytes = (destination.values.len() as u64)
                    .checked_mul(4)
                    .ok_or_else(|| invalid("indexed target length overflow"))?;
                let at = subrange(self.metadata.permutation_targets, offset, bytes, frame)?;
                if start != end {
                    seek(reader, at)?;
                    let omega = self.vk.domain.get_omega();
                    let mut previous: Option<(usize, C::Scalar)> = None;
                    for value in destination.values.iter_mut() {
                        let mut bytes = [0; 4];
                        reader.read_exact(&mut bytes)?;
                        let target = u32::from_le_bytes(bytes) as usize;
                        if target >= cells {
                            return Err(invalid("indexed permutation target is invalid"));
                        }
                        *value = match previous {
                            Some((prior, label))
                                if prior.checked_add(1) == Some(target)
                                    && prior / n == target / n =>
                            {
                                label * omega
                            }
                            _ => {
                                C::Scalar::DELTA.pow_vartime([(target / n) as u64])
                                    * omega.pow_vartime([(target % n) as u64])
                            }
                        };
                        previous = Some((target, *value));
                    }
                }
            }
        }
        let mut destination = destination;
        destination.complete = true;
        Ok(())
    }
}
