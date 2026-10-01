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
    block_hashes: &'a dyn super::BlockHashRead,
    tip: Option<super::NativeExecutionTip>,
}

impl<'a> CanonicalHistorySource<'a> {
    pub(super) fn new(
        kura: &'a Kura,
        block_hashes: &'a dyn super::BlockHashRead,
        tip: Option<super::NativeExecutionTip>,
    ) -> Self {
        Self {
            kura,
            block_hashes,
            tip,
        }
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
        if self.kura.is_canonical_body_missing(height) {
            return Err(CanonicalHistoryError::BodyUnavailable {
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

    /// Load one canonical body after the caller admits its exact durable frame.
    /// Metadata and cache presence cannot authorize unpaid body I/O or decoding.
    /// The caller retains one allocation scope across this and subsequent reads.
    pub(crate) fn block_with_admission(
        self,
        height: NonZeroUsize,
        mut before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Arc<SignedBlock>, QueryExecutionFail> {
        let expected = self
            .expected_hash(height)
            .map_err(QueryExecutionFail::CanonicalHistory)?;
        let height_u64 = height.get() as u64;
        let missing = || {
            QueryExecutionFail::CanonicalHistory(CanonicalHistoryError::BodyUnavailable {
                height: height_u64,
                expected_hash: expected,
            })
        };
        if self.kura.is_canonical_body_missing(height) {
            return Err(missing());
        }
        let storage_error = |error: crate::kura::Error| match error {
            crate::kura::Error::NoritoFrame(error) if error.is_decode_resource_limit() => {
                QueryExecutionFail::GasBudgetExceeded
            }
            crate::kura::Error::VersionedCodec(error) if error.is_decode_resource_limit() => {
                QueryExecutionFail::GasBudgetExceeded
            }
            error => QueryExecutionFail::Conversion(error.to_string()),
        };
        let source = self
            .kura
            .native_frame_read(height_u64, expected)
            .map_err(storage_error)?
            .ok_or_else(missing)?;
        let wire_len = source.wire_len();
        before_read(1, wire_len)?;
        let bytes = source
            .read(wire_len)
            .map_err(storage_error)?
            .ok_or_else(missing)?;
        let block =
            iroha_data_model::block::decode_framed_signed_block(&bytes).map_err(|error| {
                if error.is_decode_resource_limit() {
                    QueryExecutionFail::GasBudgetExceeded
                } else {
                    QueryExecutionFail::Conversion(error.to_string())
                }
            })?;
        authenticate_canonical_block(height, expected, Some(Arc::new(block)))
            .map_err(QueryExecutionFail::CanonicalHistory)
    }

    /// Read execution identity through the original State tip, without inspecting
    /// any local QC. Every source frame is admitted before its bytes are read.
    /// Parent core hash, parent R and Iroha parent hash jointly authenticate the
    /// reverse walk. The finite captured tip bounds its number of source frames.
    pub(crate) fn executed_receipt(
        self,
        height: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<crate::sumeragi::certified_chain::CommittedBlock, QueryExecutionFail> {
        let mut receipt = None;
        self.visit_executed_backwards(height, height, before_read, |value| {
            receipt = Some(value);
            Ok(())
        })?;
        receipt.ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "requested execution was absent from its authenticated interval".into(),
            )
        })
    }

    /// Visit one inclusive execution interval in descending height order after a single
    /// authenticated walk from this source's original native tip. Every physical source
    /// is charged before reading; only authenticated receipts in the selected interval
    /// reach the visitor. No local certificate is parsed or trusted.
    pub(crate) fn visit_executed_backwards(
        self,
        first: NonZeroUsize,
        last: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
        mut visit: impl FnMut(
            crate::sumeragi::certified_chain::CommittedBlock,
        ) -> Result<(), QueryExecutionFail>,
    ) -> Result<(), QueryExecutionFail> {
        self.visit_executed_backwards_until(first, last, before_read, |value| {
            visit(value).map(|()| core::ops::ControlFlow::Continue(()))
        })
        .map(drop)
    }

    /// Authenticate one ancestry walk, stopping before older source I/O when requested.
    ///
    /// Returns whether the selected interval was exhausted. Skipped ancestors still
    /// consume admitted source work and bytes before reading; every visited receipt
    /// is bound to this original State tip through its exact parent identities.
    pub(crate) fn visit_executed_backwards_until(
        self,
        first: NonZeroUsize,
        last: NonZeroUsize,
        mut before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
        mut visit: impl FnMut(
            crate::sumeragi::certified_chain::CommittedBlock,
        ) -> Result<core::ops::ControlFlow<()>, QueryExecutionFail>,
    ) -> Result<bool, QueryExecutionFail> {
        if first > last {
            return Err(QueryExecutionFail::Conversion(
                "native execution interval is reversed".into(),
            ));
        }
        self.expected_hash(first)
            .and_then(|_| self.expected_hash(last))
            .map_err(QueryExecutionFail::CanonicalHistory)?;
        let invalid = |message: String| QueryExecutionFail::Conversion(message);
        let tip = self
            .tip
            .ok_or_else(|| invalid("State has no authenticated native execution tip".into()))?;
        if usize::try_from(tip.height()).ok() != Some(self.height()) {
            return Err(invalid(
                "native execution tip differs from the captured State history cut".into(),
            ));
        }
        let mut expected_iroha = tip.iroha_hash();
        let mut expected_core = tip.core_hash();
        let mut expected_result = tip.result();
        let target =
            u64::try_from(first.get()).map_err(|_| QueryExecutionFail::GasBudgetExceeded)?;
        let selected_last =
            u64::try_from(last.get()).map_err(|_| QueryExecutionFail::GasBudgetExceeded)?;
        for source_height in (target..=tip.height()).rev() {
            let index = usize::try_from(source_height)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
            let journal_hash = self
                .expected_hash(index)
                .map_err(QueryExecutionFail::CanonicalHistory)?;
            if journal_hash != expected_iroha {
                return Err(invalid(format!(
                    "native execution parent contradicts State hash at {source_height}"
                )));
            }
            let block = self.block_with_admission(index, &mut before_read)?;
            let receipt = crate::sumeragi::certified_chain::read_frame(block, source_height)
                .map_err(|error| invalid(error.to_string()))?;
            if receipt.core_hash() != expected_core || receipt.result() != expected_result {
                return Err(invalid(format!(
                    "native header or R differs from authenticated execution ancestry at {source_height}"
                )));
            }
            if source_height > target {
                let header = receipt
                    .header()
                    .ok_or_else(|| invalid("genesis cannot precede the requested height".into()))?;
                expected_core = header.parent_hash;
                expected_result = header.parent_result;
                expected_iroha =
                    receipt.block().header().prev_block_hash().ok_or_else(|| {
                        invalid("native successor omits Iroha parent hash".into())
                    })?;
            }
            if source_height <= selected_last {
                if visit(receipt)?.is_break() {
                    return Ok(false);
                }
            }
            if source_height == target {
                return Ok(true);
            }
        }
        Err(invalid(
            "requested execution lies beyond the original native tip".into(),
        ))
    }

    /// Load exact original execution bytes; the callback admits actual source
    /// count and bytes, including intermediate parents and a subsequently failed read.
    pub(crate) fn executed_block(
        self,
        height: NonZeroUsize,
        before_read: impl FnMut(u64, u64) -> Result<(), QueryExecutionFail>,
    ) -> Result<Arc<SignedBlock>, QueryExecutionFail> {
        self.executed_receipt(height, before_read)
            .map(|receipt| Arc::clone(receipt.block()))
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
