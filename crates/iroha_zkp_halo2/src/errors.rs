//! Error types for the Halo2 IPA commitment and proof system.

use thiserror::Error as ThisError;

/// Error type returned by commitment, proof creation, or verification.
#[derive(Debug, ThisError)]
pub enum Error {
    /// Vector lengths do not match expected dimensions.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension encountered.
        actual: usize,
    },

    /// Parameter `n` must be a non-zero power of two.
    #[error("invalid parameter: n must be a non-zero power of two; got n={0}")]
    InvalidN(usize),

    /// Attempted inversion of a zero field element.
    #[error("field inversion of zero")]
    InversionOfZero,

    /// Verification failed.
    #[error("verification failed")]
    VerificationFailed,

    /// Encountered non-canonical field or group encoding.
    #[error("invalid encoding")]
    InvalidEncoding,

    /// Encountered parameters that are neither canonical nor registered.
    #[error("unknown parameter set")]
    UnknownParams,

    /// Generator was invalid (e.g., identity element or duplicate point).
    #[error("invalid {kind} generator at index {index}: {reason}")]
    InvalidGenerator {
        /// Type of generator (`G`, `H`, or `U`).
        kind: &'static str,
        /// Index within the generator vector (0-based; `0` for `U`).
        index: usize,
        /// Human-readable failure reason.
        reason: &'static str,
    },

    /// Backend is not compiled in or otherwise unsupported.
    #[error("unsupported backend: {backend:?}")]
    UnsupportedBackend {
        /// Curve/backend identifier that triggered the error.
        backend: crate::norito_types::ZkCurveId,
    },
}
