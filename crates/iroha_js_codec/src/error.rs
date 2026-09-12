//! Platform-independent error categories retained by the native codec adapters.

use std::fmt;

/// Error classification shared by native platform bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecErrorKind {
    /// A typed account or instruction argument violates its admission contract.
    InvalidArgument,
    /// JSON, Norito decoding, or another codec operation failed.
    Failure,
}

/// Exact diagnostic and classification produced by the canonical codec owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecError {
    kind: CodecErrorKind,
    reason: String,
}

impl CodecError {
    /// Construct an error without changing its native diagnostic text.
    pub fn new(kind: CodecErrorKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }

    /// Construct a generic codec failure.
    pub fn failure(reason: impl Into<String>) -> Self {
        Self::new(CodecErrorKind::Failure, reason)
    }

    /// Return the classification required by the platform error adapter.
    pub fn kind(&self) -> CodecErrorKind {
        self.kind
    }

    /// Borrow the exact native diagnostic.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for CodecError {}

/// Result returned by the shared canonical codec.
pub type CodecResult<T> = Result<T, CodecError>;
