//! Fallible, world-state-anchored access to canonical block history.

use std::{num::NonZeroUsize, sync::Arc};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    query::error::CanonicalHistoryError,
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

    fn load(
        self,
        height: NonZeroUsize,
        without_merge_sidecar: bool,
    ) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        let expected_hash = self.expected_hash(height)?;
        if self.kura.is_hash_only_block_height(height) {
            return Err(CanonicalHistoryError::HashOnlyBodyUnavailable {
                height: u64::try_from(height.get())
                    .expect("supported target pointer widths fit a block height into u64"),
                expected_hash,
            });
        }
        let block = if without_merge_sidecar {
            self.kura.get_block_without_merge_sidecar(height)
        } else {
            self.kura.get_block(height)
        };
        authenticate_canonical_block(height, expected_hash, block)
    }

    /// Load and authenticate the canonical block body at `height`.
    ///
    /// # Errors
    ///
    /// Returns a typed availability error for a missing or authenticated
    /// hash-only body, and a typed corruption error when the Kura body
    /// contradicts the committed WSV hash journal or slot.
    pub fn block(self, height: NonZeroUsize) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        self.load(height, false)
    }

    /// Load the canonical body without resolving a merge sidecar.
    ///
    /// This preserves the transaction query's requirement to charge the
    /// compact carrier declaration before decoding its full sidecar.
    pub(crate) fn block_without_merge_sidecar(
        self,
        height: NonZeroUsize,
    ) -> Result<Arc<SignedBlock>, CanonicalHistoryError> {
        self.load(height, true)
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
