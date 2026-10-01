//! Fixed rejection custody at the canonical leaf's codec/hash boundaries.
//!
//! No adapter boxes an upstream error or renders an owned diagnostic. This does
//! not fund allocations performed inside an arbitrary caller-supplied serializer;
//! closed native capture must separately qualify its concrete codecs.

use super::LeafError;
use std::io;

/// Exact operation which failed before a retained leaf could be returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum EncodingOperation {
    /// First canonical pass determining exact bytes and length.
    #[error("measure canonical payload")]
    MeasurePayload,
    /// Second canonical pass into the admitted frame owner.
    #[error("write admitted canonical frame")]
    WriteFrame,
    /// Second canonical pass shared by paired value commitments.
    #[error("hash paired canonical value")]
    HashPairedValue,
    /// Direct typed leaf payload commitment.
    #[error("hash typed canonical payload")]
    HashTypedPayload,
}

/// Bounded failure information; formatting occurs only at a caller boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum EncodingReason {
    /// The concrete serializer rejected its input.
    #[error("canonical serializer rejected the input")]
    Codec,
    /// A concrete serializer rejected a statically named semantic value.
    #[error("invalid canonical {0}")]
    InvalidValue(&'static str),
    /// A writer or reader failed, retaining its local I/O category.
    #[error("canonical I/O failure: {0:?}")]
    Io(io::ErrorKind),
    /// A nested codec reported a physical allocation failure, never semantic invalidity.
    #[error("canonical codec could not allocate {bytes} bytes")]
    Allocation {
        /// Original requested byte count reported by the codec.
        bytes: u64,
    },
    /// A serializer violated its own exact child-length contract.
    #[error("canonical codec length mismatch")]
    CodecLength,
    /// Second-pass output differs in length or exceeds its original bound.
    #[error("canonical table payload changed length between bounded passes")]
    ChangedLength,
    /// Equal-length second-pass output differs in bytes.
    #[error("canonical table payload changed between bounded passes")]
    ChangedPayload,
}

/// Copy-sized operation/reason pair with no retained diagnostic allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{operation}: {reason}")]
pub(crate) struct EncodingError {
    operation: EncodingOperation,
    reason: EncodingReason,
}

impl EncodingError {
    pub(super) fn new(operation: EncodingOperation, reason: EncodingReason) -> Self {
        Self { operation, reason }
    }

    pub(super) fn codec(operation: EncodingOperation, error: norito::Error) -> Self {
        let reason = match error {
            norito::Error::Io(error) => EncodingReason::Io(error.kind()),
            norito::Error::AllocationFailed { bytes } => EncodingReason::Allocation { bytes },
            norito::Error::LengthMismatch => EncodingReason::CodecLength,
            norito::Error::InvalidValue { context } => EncodingReason::InvalidValue(context),
            // Detailed owned codec diagnostics are unnecessary for a failed
            // scoped leaf. Drop the original owner without copying/rendering it.
            _ => EncodingReason::Codec,
        };
        Self::new(operation, reason)
    }

    pub(super) fn io(operation: EncodingOperation, error: io::Error) -> Self {
        Self::new(operation, EncodingReason::Io(error.kind()))
    }
}

impl From<EncodingError> for LeafError {
    fn from(error: EncodingError) -> Self {
        Self::Encoding(error)
    }
}
