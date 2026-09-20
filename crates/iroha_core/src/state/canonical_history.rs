//! Fallible, world-state-anchored access to canonical block history.

use std::{num::NonZeroUsize, sync::Arc};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    query::error::{CanonicalHistoryError, QueryExecutionFail},
};
use iroha_logger::prelude::*;

use crate::kura::Kura;

pub(super) fn authenticate_canonical_block(
    height: NonZeroUsize,
    expected: HashOf<BlockHeader>,
    block: Option<Arc<SignedBlock>>,
) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
    let height_u64 = u64::try_from(height.get())
        .expect("supported target pointer widths always fit a block height into u64");
    let block = block.ok_or(CanonicalHistoryError::BodyUnavailable {
        height: height_u64,
        expected_hash: expected,
    })?;
    let actual = block.hash();
    if actual != expected {
        return Err(CanonicalHistoryError::BlockHashMismatch {
            height: height_u64,
            expected_hash: expected,
            actual_hash: actual,
        });
    }
    if block.header().height().get() != height_u64 {
        return Err(CanonicalHistoryError::BlockHeightMismatch {
            height: height_u64,
            actual_height: block.header().height().get(),
        });
    }
    Ok(block)
}

pub(super) fn committed_block_from_kura(
    kura: &Kura,
    height: NonZeroUsize,
    expected: HashOf<BlockHeader>,
) -> Option<Arc<SignedBlock>> {
    authenticate_canonical_block(height, expected, kura.get_block(height))
        .inspect_err(|error| warn!(%error, "rejecting non-canonical Kura block body"))
        .ok()
}

/// Immutable, WSV-anchored source of canonical committed block bodies.
///
/// Every load authenticates both the header hash and one-based header height.
/// An authenticated hash-only snapshot entry is reported as an explicit body
/// availability failure and is never omitted from iteration.
#[derive(Clone, Copy)]
pub struct CanonicalHistorySource<'a> {
    kura: &'a Kura,
    block_hashes: &'a [HashOf<BlockHeader>],
}

impl<'a> CanonicalHistorySource<'a> {
    pub(super) fn new(kura: &'a Kura, block_hashes: &'a [HashOf<BlockHeader>]) -> Self {
        Self { kura, block_hashes }
    }

    /// Return the committed height captured by this immutable source.
    #[must_use]
    pub fn height(self) -> usize {
        self.block_hashes.len()
    }

    /// Resolve a committed header hash from the immutable WSV journal.
    #[must_use]
    pub fn block_height_by_hash(self, hash: HashOf<BlockHeader>) -> Option<NonZeroUsize> {
        self.block_hashes
            .iter()
            .position(|candidate| *candidate == hash)
            .and_then(|index| index.checked_add(1))
            .and_then(NonZeroUsize::new)
    }

    fn expected_hash(
        self,
        height: NonZeroUsize,
    ) -> Result<HashOf<BlockHeader>, CanonicalHistoryError> {
        self.block_hashes
            .get(height.get() - 1)
            .copied()
            .ok_or_else(|| CanonicalHistoryError::HeightOutsideSnapshot {
                height: u64::try_from(height.get())
                    .expect("supported target pointer widths fit a block height into u64"),
                committed_height: u64::try_from(self.height())
                    .expect("supported target pointer widths fit a block height into u64"),
            })
    }

    fn load(self, height: NonZeroUsize) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        let expected_hash = self.expected_hash(height)?;
        if self.kura.is_hash_only_block_height(height) {
            return Err(CanonicalHistoryError::HashOnlyBodyUnavailable {
                height: u64::try_from(height.get())
                    .expect("supported target pointer widths fit a block height into u64"),
                expected_hash,
            });
        }
        let block = self.kura.get_block(height);
        authenticate_canonical_block(height, expected_hash, block)
    }

    /// Load a body whose header hash and height agree with this snapshot.
    ///
    /// Output readers must use `executed_block` to bind attached outputs to
    /// exact finalized bytes; the proposal header does not commit those bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed availability error for a missing or authenticated
    /// hash-only body, and a typed corruption error when the Kura body
    /// contradicts the committed WSV hash journal or slot.
    pub fn block(self, height: NonZeroUsize) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        self.load(height)
    }

    /// Load exact published execution bytes after the caller admits their durable size.
    ///
    /// Header identity alone does not authenticate attached outputs. This path
    /// requires Kura's verified finality commitment and rechecks the admitted
    /// length under its storage guards before allocating or decoding the body.
    /// The admission callback runs once, including for a later failed read.
    pub(crate) fn executed_block(
        self,
        height: NonZeroUsize,
        before_read: impl FnOnce(u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Arc<SignedBlock>, QueryExecutionFail> {
        let expected_hash = self
            .expected_hash(height)
            .map_err(QueryExecutionFail::CanonicalHistory)?;
        let height_u64 =
            u64::try_from(height.get()).map_err(|_| QueryExecutionFail::GasBudgetExceeded)?;
        if self.kura.is_hash_only_block_height(height) {
            return Err(QueryExecutionFail::CanonicalHistory(
                CanonicalHistoryError::HashOnlyBodyUnavailable {
                    height: height_u64,
                    expected_hash,
                },
            ));
        }
        let storage_error = |error: crate::kura::Error| {
            QueryExecutionFail::Conversion(format!(
                "canonical executed body at height {height_u64} failed storage authentication: {error}"
            ))
        };
        let (durable_height, wire_len) = self
            .kura
            .durable_block_payload_len_by_hash(expected_hash)
            .map_err(storage_error)?
            .ok_or(QueryExecutionFail::CanonicalHistory(
                CanonicalHistoryError::BodyUnavailable {
                    height: height_u64,
                    expected_hash,
                },
            ))?;
        if durable_height != height_u64 {
            return Err(QueryExecutionFail::CanonicalHistory(
                CanonicalHistoryError::BlockHeightMismatch {
                    height: height_u64,
                    actual_height: durable_height,
                },
            ));
        }
        before_read(wire_len)?;
        let block = self
            .kura
            .read_block_body_with_wire_bound(height, expected_hash, wire_len)
            .map_err(storage_error)?;
        authenticate_canonical_block(height, expected_hash, block)
            .map_err(QueryExecutionFail::CanonicalHistory)
    }

    /// Exercise the actual executed-body admission boundary with a durable test journal.
    #[cfg(test)]
    pub(crate) fn read_executed_for_testing(
        kura: &'a Kura,
        hashes: &'a [HashOf<BlockHeader>],
        height: NonZeroUsize,
        before_read: impl FnOnce(u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Arc<SignedBlock>, QueryExecutionFail> {
        Self::new(kura, hashes).executed_block(height, before_read)
    }

    /// Iterate every committed slot from `start` through this source's tip.
    ///
    /// The cursor stops after its first error so callers cannot accidentally
    /// resume beyond an unavailable or corrupt canonical slot.
    #[must_use]
    pub fn cursor(self, start: NonZeroUsize) -> CanonicalHistoryCursor<'a> {
        CanonicalHistoryCursor {
            source: self,
            front: start.get(),
            back_inclusive: self.height(),
            done: start.get() > self.height(),
        }
    }
}

/// Fallible double-ended cursor over canonical committed block bodies.
pub struct CanonicalHistoryCursor<'a> {
    source: CanonicalHistorySource<'a>,
    front: usize,
    back_inclusive: usize,
    done: bool,
}

impl CanonicalHistoryCursor<'_> {
    fn stop_on_error(
        &mut self,
        result: Result<Arc<SignedBlock>, CanonicalHistoryError>,
    ) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        if result.is_err() {
            self.done = true;
        }
        result
    }
}

impl Iterator for CanonicalHistoryCursor<'_> {
    type Item = Result<Arc<SignedBlock>, CanonicalHistoryError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let height = NonZeroUsize::new(self.front)
            .expect("a canonical history cursor starts at a non-zero height");
        if self.front == self.back_inclusive {
            self.done = true;
        } else {
            self.front += 1;
        }
        let result = self.source.block(height);
        Some(self.stop_on_error(result))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.done {
            0
        } else {
            self.back_inclusive
                .saturating_sub(self.front)
                .saturating_add(1)
        };
        (0, Some(remaining))
    }
}

impl DoubleEndedIterator for CanonicalHistoryCursor<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let height = NonZeroUsize::new(self.back_inclusive)
            .expect("a canonical history cursor ends at a non-zero height");
        if self.front == self.back_inclusive {
            self.done = true;
        } else {
            self.back_inclusive -= 1;
        }
        let result = self.source.block(height);
        Some(self.stop_on_error(result))
    }
}

impl std::iter::FusedIterator for CanonicalHistoryCursor<'_> {}
