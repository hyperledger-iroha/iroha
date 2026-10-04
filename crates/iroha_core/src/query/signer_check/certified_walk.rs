//! Exact borrowed-view custody for native signer proof blocks.
//!
//! A certified block proves its header/result under authenticated authority, but does
//! not by itself identify the State/Kura source that supplied its local certificate.
//! Only this walk constructs a source-bound block. Borrowing the original view prevents
//! its identity from being reused while any block is retained; identity is never serialized.
//!
//! Local native refusals keep their original retry owner; only completed rejections map
//! into a purpose's semantic finality error.

use super::{Error, StateView};
use crate::execution_attempt::ExecutionAttemptError;
use crate::sumeragi::certified_chain::{CertifiedBlock, CertifiedChain};
use iroha_data_model::{
    query::error::QueryExecutionFail, sumeragi::finality::NativeFinalityLimits,
};
use std::{cell::RefCell, num::NonZeroUsize};

const LIMITS: NativeFinalityLimits = NativeFinalityLimits {
    block_bytes: iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_BYTES,
    journal_bytes: 64 * 1024 * 1024,
    block_count: 2 * super::MAX_NATIVE_CHECK_HISTORY_BLOCKS_V1 as usize + 3,
    allocated_bytes: 256 * 1024 * 1024,
};

/// Retain one finite cumulative decoder owner around the complete purpose operation.
/// Nested callers retain every narrower original allowance; no frame replenishes it.
pub(crate) fn with_native_check_read_limits<T>(consume: impl FnOnce() -> T) -> T {
    norito::core::with_decode_limits_scope(
        LIMITS
            .decode_limits()
            .expect("fixed native Check limits are valid"),
        consume,
    )
}

struct SourceAllowance {
    frames: u64,
    bytes: u64,
    failed: bool,
}
impl SourceAllowance {
    fn admit(
        &mut self,
        frames: u64,
        bytes: u64,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<QueryExecutionFail>> {
        let remaining = self
            .frames
            .checked_sub(frames)
            .zip(self.bytes.checked_sub(bytes));
        if self.failed || bytes > LIMITS.block_bytes as u64 || remaining.is_none() {
            self.failed = true;
            return Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
            ));
        }
        (self.frames, self.bytes) = remaining.unwrap();
        Ok(())
    }
}

/// One native reader tied to the exact immutable view used for proof rows.
/// Purpose owners retain [`with_native_check_read_limits`] around construction, every
/// iterator step and their complete proof relations. Source allowances belong to this reader
/// and cannot be refreshed by starting another interval on it.
pub(crate) struct SignerCertifiedWalkV1<'view, 'state> {
    view: &'view StateView<'state>,
    chain: CertifiedChain<'view, StateView<'state>>,
    allowance: RefCell<SourceAllowance>,
}
impl<'view, 'state> SignerCertifiedWalkV1<'view, 'state> {
    pub(crate) fn new(
        view: &'view StateView<'state>,
    ) -> Result<Self, ExecutionAttemptError<Error>> {
        let mut allowance = SourceAllowance {
            frames: LIMITS.block_count as u64,
            bytes: LIMITS.journal_bytes as u64,
            failed: false,
        };
        let chain = CertifiedChain::new_with_source_admission(view, |frames, bytes| {
            allowance.admit(frames, bytes)
        })
        .map_err(|error| error.map_rejection(|_| Error::Finality))?;
        Ok(Self {
            view,
            chain,
            allowance: RefCell::new(allowance),
        })
    }

    /// Every receipt is produced by this reader's actual checked walk, never supplied by a caller.
    pub(crate) fn walk(
        &self,
        start: u64,
        end: u64,
    ) -> impl Iterator<
        Item = Result<SignerCertifiedBlockV1<'view, 'state>, ExecutionAttemptError<Error>>,
    > + '_ {
        let interval = super::check_history_span_v1(start, end).and_then(|()| {
            usize::try_from(start)
                .ok()
                .and_then(NonZeroUsize::new)
                .zip(usize::try_from(end).ok().and_then(NonZeroUsize::new))
                .ok_or(Error::Finality)
        });
        let mut failed = false;
        let mut invalid = interval.is_err();
        let mut walk = interval.ok().map(|(start, end)| {
            self.chain.walk_from_execution(start, end, |frames, bytes| {
                self.allowance.borrow_mut().admit(frames, bytes)
            })
        });
        std::iter::from_fn(move || {
            if failed {
                return None;
            }
            if invalid {
                invalid = false;
                failed = true;
                return Some(Err(ExecutionAttemptError::Rejected(Error::Finality)));
            }
            // Move the receipt directly into its source-bound owner. Chained result maps
            // retain additional full decoded receipt temporaries on debug native stacks.
            match walk.as_mut()?.next()? {
                Ok(block) => Some(Ok(SignerCertifiedBlockV1 {
                    view: self.view,
                    block,
                })),
                Err(error) => {
                    failed = true;
                    Some(Err(error.map_rejection(|_| Error::Finality)))
                }
            }
        })
    }
}

/// Actual certified receipt with its original borrowed source; no caller-facing constructor.
pub(crate) struct SignerCertifiedBlockV1<'view, 'state> {
    view: &'view StateView<'state>,
    block: CertifiedBlock,
}
impl SignerCertifiedBlockV1<'_, '_> {
    pub(crate) fn height(&self) -> u64 {
        self.block.height()
    }

    /// Reference identity is checked before any native proof predicate may consume the receipt.
    /// A fresh view, even of the same State and bytes, is a different proof source.
    pub(crate) fn in_view(&self, view: &StateView<'_>) -> Result<&CertifiedBlock, Error> {
        // Erase only the type's lifetime parameter for address comparison. Both original
        // references remain borrowed and live; neither address is stored or serialized.
        if core::ptr::eq(
            core::ptr::from_ref(self.view).cast::<()>(),
            core::ptr::from_ref(view).cast::<()>(),
        ) {
            Ok(&self.block)
        } else {
            Err(Error::Finality)
        }
    }
}
