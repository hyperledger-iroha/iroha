//! Internal chunk-only access shared by bounded expression and graph evaluators.
//!
//! This trait exposes neither a snapshot handle nor a whole-column operation. It does not
//! provide transactional ownership: complete-advice callers must enclose the entire evaluator,
//! including preflight and its result consumer, in their own consuming session guard.

use super::{StoredPolynomialErrorV1, StoredPolynomialLayoutV1, StoredPolynomialSnapshotV1};

/// Borrowed backend paired with independently supplied exact trusted metadata.
pub(crate) struct StoredAdviceInputV1<'a, S> {
    pub(crate) expected: StoredPolynomialLayoutV1,
    pub(crate) snapshot: &'a mut S,
}

/// Serial, bounded encoded reads with exact global-column metadata checks.
pub(crate) trait StoredAdviceChunkSourceV1 {
    /// Number of retained global advice columns, including columns unused by this evaluator.
    fn column_count(&self) -> usize;

    /// Check independently supplied metadata against both the receipt and its live snapshot.
    fn validate_layout(
        &mut self,
        expected: StoredPolynomialLayoutV1,
    ) -> Result<(), StoredPolynomialErrorV1>;

    /// Read one encoded chunk without permitting a backend or borrowed chunk to escape.
    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1>;
}

/// Adapter retaining the original raw-snapshot helper contract for existing consumers.
///
/// Borrowed snapshots cannot provide whole-proof ownership. In particular, an arithmetic or
/// final consumer failure clears evaluator scratch but does not destroy this borrowed bank.
pub(crate) struct StoredAdviceSliceReaderV1<'a, 'snapshot, S> {
    inputs: &'a mut [StoredAdviceInputV1<'snapshot, S>],
}

impl<'a, 'snapshot, S> StoredAdviceSliceReaderV1<'a, 'snapshot, S> {
    /// Borrow a raw input slice without allocating per-column proxies or witness memory.
    pub(crate) fn new(inputs: &'a mut [StoredAdviceInputV1<'snapshot, S>]) -> Self {
        Self { inputs }
    }
}

impl<S: StoredPolynomialSnapshotV1> StoredAdviceChunkSourceV1
    for StoredAdviceSliceReaderV1<'_, '_, S>
{
    fn column_count(&self) -> usize {
        self.inputs.len()
    }

    fn validate_layout(
        &mut self,
        expected: StoredPolynomialLayoutV1,
    ) -> Result<(), StoredPolynomialErrorV1> {
        let input = self
            .inputs
            .get(expected.advice_coordinates()?.0 as usize)
            .ok_or(StoredPolynomialErrorV1::Context)?;
        if input.expected != expected || input.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        Ok(())
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        let input = self
            .inputs
            .get_mut(expected.advice_coordinates()?.0 as usize)
            .ok_or(StoredPolynomialErrorV1::Context)?;
        // The shared evaluator checks exact receipt/live identities immediately before and
        // after each complete leaf read, preserving the former snapshot-slice behavior.
        input.snapshot.with_chunk(expected, chunk, consume)
    }
}
