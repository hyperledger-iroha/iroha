//! Bounded, fallible advice assignment for an explicitly admitted discard-only producer.
//!
//! This is deliberately not an implementation of the general `Assignment` trait: that trait
//! permits overwrites and escaping references. The existing prover remains its implementation.
//! Each owner retains one 256-row numerator/denominator chunk. Flushing uses one additional
//! 256-field inversion scratch and 256 canonical scalar encodings, regardless of domain size.
//! The backend's buffers, caller copies, field-operation temporaries and process RSS are separate.
//!
//! TODO: Admit complete circuit producers, check configured column uniqueness, and propagate
//! these fallible operations through a stored consuming prover. Finish every phase column's
//! tail in existing column order before drawing any commitment blinds; retain current instance,
//! transcript, challenge and circuit-drop order. No synthesis replay or proof RNG lives here.

use std::{
    fmt, ptr,
    sync::atomic::{Ordering, compiler_fence},
};

use ff::{BatchInverter, PrimeField};
use halo2curves::pasta::{Fp, Fq};

use super::{
    STORED_SCALARS_PER_CHUNK_V1, StoredAdviceErrorV1, StoredAdviceLayoutV1, StoredAdviceSnapshotV1,
    StoredAdviceWriterV1, StoredPastaFieldV1, StoredPolynomialBasisV1,
};
use crate::plonk::Assigned;

mod sealed {
    pub trait Pasta {}
    impl Pasta for halo2curves::pasta::Fp {}
    impl Pasta for halo2curves::pasta::Fq {}
}

/// Exactly the two canonical Pasta scalar fields supported by the confidential store.
///
/// This sealed bound prevents a same-width unrelated field being mislabeled as Pasta.
pub trait StoredAssignmentFieldV1: sealed::Pasta + PrimeField<Repr = [u8; 32]> {
    /// Authenticated scalar-field identity of this concrete field implementation.
    const STORED_FIELD: StoredPastaFieldV1;
}

impl StoredAssignmentFieldV1 for Fp {
    const STORED_FIELD: StoredPastaFieldV1 = StoredPastaFieldV1::Fp;
}
impl StoredAssignmentFieldV1 for Fq {
    const STORED_FIELD: StoredPastaFieldV1 = StoredPastaFieldV1::Fq;
}

/// Coarse errors at the explicit stored-producer boundary; never contains witness values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredAssignmentErrorV1 {
    /// A backend or authenticated-layout operation failed.
    Store(StoredAdviceErrorV1),
    /// An advice assignment was outside the configured usable rows.
    Row,
    /// A duplicate or backward row would violate the admitted write-once contract.
    NonMonotonic,
    /// The producer requested an escaping assigned-value reference.
    ReferenceReturn,
    /// An earlier partial operation failed or unwound; this owner cannot continue.
    Poisoned,
}

impl fmt::Display for StoredAssignmentErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "stored advice assignment: {self:?}")
    }
}
impl std::error::Error for StoredAssignmentErrorV1 {}
impl From<StoredAdviceErrorV1> for StoredAssignmentErrorV1 {
    fn from(error: StoredAdviceErrorV1) -> Self {
        Self::Store(error)
    }
}

// Volatile replacement applies to initialized, owned field slots. The sealed Pasta fields are
// Copy and have no destructor. This clears these slots on success, error and unwind; it cannot
// erase caller copies, earlier moves, arithmetic registers/spills or kernel memory.
struct FieldChunk<F: StoredAssignmentFieldV1>([F; STORED_SCALARS_PER_CHUNK_V1]);

impl<F: StoredAssignmentFieldV1> FieldChunk<F> {
    fn zero() -> Self {
        Self([F::ZERO; STORED_SCALARS_PER_CHUNK_V1])
    }

    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: `value` is an exclusive, aligned reference to a live Copy Pasta field.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

impl<F: StoredAssignmentFieldV1> Drop for FieldChunk<F> {
    fn drop(&mut self) {
        self.clear();
    }
}

struct EncodedChunk([[u8; 32]; STORED_SCALARS_PER_CHUNK_V1]);

impl Drop for EncodedChunk {
    fn drop(&mut self) {
        for scalar in &mut self.0 {
            for byte in scalar {
                // SAFETY: `byte` is an exclusive reference to an initialized byte.
                unsafe { ptr::write_volatile(byte, 0) };
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}

struct Active<F: StoredAssignmentFieldV1, W: StoredAdviceWriterV1> {
    writer: W,
    numerators: FieldChunk<F>,
    denominators: FieldChunk<F>,
    next_row: usize,
    next_chunk: u64,
}

/// One move-only advice column accepting only strictly increasing discard assignments.
///
/// Constructor metadata is compared with the backend's complete immutable layout. Skipped rows
/// evaluate to zero, including an empty column. Assigned rational values retain Halo2's `x/0=0`
/// semantics. No column bank, cell reference cache, transcript or RNG is retained.
///
/// Row/order/reference preflight errors leave the owner usable. Any failure after mutation,
/// including a backend panic caught by the caller, drops the backend writer and clears the owned
/// chunk. No snapshot can then escape from this owner. The trusted backend still owns storage
/// authentication and destruction; this adapter does not turn a malicious backend into one.
pub struct StoredAdviceAssignmentV1<F: StoredAssignmentFieldV1, W: StoredAdviceWriterV1> {
    layout: StoredAdviceLayoutV1,
    usable_rows: usize,
    active: Option<Active<F, W>>,
}

impl<F: StoredAssignmentFieldV1, W: StoredAdviceWriterV1> fmt::Debug
    for StoredAdviceAssignmentV1<F, W>
{
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("StoredAdviceAssignmentV1")
            .field("layout", &self.layout)
            .field("usable_rows", &self.usable_rows)
            .field("poisoned", &self.active.is_none())
            .finish_non_exhaustive()
    }
}

impl<F: StoredAssignmentFieldV1, W: StoredAdviceWriterV1> StoredAdviceAssignmentV1<F, W> {
    /// Admit one exact Lagrange column and its exclusive usable-row end.
    ///
    /// # Errors
    /// Rejects mismatched complete metadata, field, basis or an out-of-domain usable-row end.
    /// Consumes and drops the supplied writer on rejection. The caller owns configuration-wide
    /// uniqueness of column/phase coordinates; a per-column constructor cannot establish it.
    pub fn new(
        writer: W,
        expected: StoredAdviceLayoutV1,
        usable_rows: usize,
    ) -> Result<Self, StoredAssignmentErrorV1> {
        if writer.layout() != expected
            || expected.field() != F::STORED_FIELD
            || expected.basis() != StoredPolynomialBasisV1::Lagrange
        {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        if usable_rows > expected.scalar_count() {
            return Err(StoredAdviceErrorV1::Layout.into());
        }
        Ok(Self {
            layout: expected,
            usable_rows,
            active: Some(Active {
                writer,
                numerators: FieldChunk::zero(),
                denominators: FieldChunk::zero(),
                next_row: 0,
                next_chunk: 0,
            }),
        })
    }

    /// Return the trusted complete immutable identity of this destination.
    pub fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }

    /// Assign a known value without retaining or returning a witness reference.
    ///
    /// # Errors
    /// Row/order errors are retryable preflights. A gap may flush multiple chunks; any failure
    /// during that operation invalidates this complete column, including previously stored slots.
    pub fn assign_discarding_value(
        &mut self,
        row: usize,
        value: Assigned<F>,
    ) -> Result<(), StoredAssignmentErrorV1> {
        let active = self
            .active
            .as_ref()
            .ok_or(StoredAssignmentErrorV1::Poisoned)?;
        if row >= self.usable_rows {
            return Err(StoredAssignmentErrorV1::Row);
        }
        if row < active.next_row {
            return Err(StoredAssignmentErrorV1::NonMonotonic);
        }
        // Detach before mutation. On error or unwind this local owner, its writer and every
        // owned buffer drop; the public owner remains poisoned even if the caller catches panic.
        let mut active = self
            .active
            .take()
            .ok_or(StoredAssignmentErrorV1::Poisoned)?;
        active.fill_zeros_to(row, self.layout)?;
        active.append(value, self.layout)?;
        self.active = Some(active);
        Ok(())
    }

    /// Explicitly refuse the general API's escaping-reference operation.
    ///
    /// # Errors
    /// Always returns `ReferenceReturn` (or `Poisoned` after an earlier operational failure).
    /// The independent return lifetime mirrors the operation being rejected; no reference is
    /// constructed, transmuted, cached, or silently replaced by a discard assignment.
    pub fn assign_returning_reference<'v>(
        &mut self,
        _row: usize,
        _value: Assigned<F>,
    ) -> Result<&'v Assigned<F>, StoredAssignmentErrorV1> {
        if self.active.is_none() {
            Err(StoredAssignmentErrorV1::Poisoned)
        } else {
            Err(StoredAssignmentErrorV1::ReferenceReturn)
        }
    }

    /// Fill remaining usable rows with zero, obtain every tail scalar, then seal this column.
    ///
    /// `tail` is called exactly once for each absolute row in `usable_rows..2^k`, in order.
    /// It is not called for any usable-row gap. For the future prover adapter it must supply
    /// the current phase's ordinary blinding scalars: finish all columns' tails in configured
    /// order before drawing any commitment blinds. This layer never consumes proof RNG itself.
    ///
    /// # Errors
    /// Propagates tail/storage errors. Consumes this owner on every result, rejects backend
    /// identity changes, and never returns a partial or identity-substituted snapshot.
    pub fn finish_with_tail(
        mut self,
        mut tail: impl FnMut(usize) -> Result<F, StoredAssignmentErrorV1>,
    ) -> Result<W::Snapshot, StoredAssignmentErrorV1> {
        let mut active = self
            .active
            .take()
            .ok_or(StoredAssignmentErrorV1::Poisoned)?;
        active.fill_zeros_to(self.usable_rows, self.layout)?;
        while active.next_row < self.layout.scalar_count() {
            let value = tail(active.next_row)?;
            active.append(Assigned::Trivial(value), self.layout)?;
        }
        if active.writer.layout() != self.layout {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        let snapshot = active.writer.seal()?;
        if snapshot.layout() != self.layout {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        Ok(snapshot)
    }
}

impl<F: StoredAssignmentFieldV1, W: StoredAdviceWriterV1> Active<F, W> {
    fn fill_zeros_to(
        &mut self,
        end: usize,
        layout: StoredAdviceLayoutV1,
    ) -> Result<(), StoredAssignmentErrorV1> {
        while self.next_row < end {
            let chunk_end = ((self.next_chunk as usize + 1) * STORED_SCALARS_PER_CHUNK_V1)
                .min(layout.scalar_count());
            // Unassigned slots start at zero and are cleared after every successful flush.
            self.next_row = end.min(chunk_end);
            if self.next_row == chunk_end {
                self.flush(layout)?;
            }
        }
        Ok(())
    }

    fn append(
        &mut self,
        value: Assigned<F>,
        layout: StoredAdviceLayoutV1,
    ) -> Result<(), StoredAssignmentErrorV1> {
        let slot = self.next_row % STORED_SCALARS_PER_CHUNK_V1;
        let (numerator, denominator) = match value {
            Assigned::Zero => (F::ZERO, F::ZERO),
            Assigned::Trivial(value) => (value, F::ONE),
            Assigned::Rational(numerator, denominator) => (numerator, denominator),
        };
        self.numerators.0[slot] = numerator;
        self.denominators.0[slot] = denominator;
        self.next_row += 1;
        if self.next_row % STORED_SCALARS_PER_CHUNK_V1 == 0
            || self.next_row == layout.scalar_count()
        {
            self.flush(layout)?;
        }
        Ok(())
    }

    fn flush(&mut self, layout: StoredAdviceLayoutV1) -> Result<(), StoredAssignmentErrorV1> {
        if self.writer.layout() != layout {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        let count = layout.chunk_scalar_count(self.next_chunk)?;
        let mut scratch = FieldChunk::<F>::zero();
        BatchInverter::invert_with_external_scratch(
            &mut self.denominators.0[..count],
            &mut scratch.0[..count],
        );
        let mut encoded = EncodedChunk([[0; 32]; STORED_SCALARS_PER_CHUNK_V1]);
        for (index, scalar) in encoded.0[..count].iter_mut().enumerate() {
            *scalar = (self.numerators.0[index] * self.denominators.0[index]).to_repr();
        }
        self.writer
            .write_chunk(self.next_chunk, &encoded.0[..count])?;
        self.numerators.clear();
        self.denominators.clear();
        self.next_chunk += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
