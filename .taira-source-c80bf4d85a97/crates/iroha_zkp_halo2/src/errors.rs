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

    /// Wire component version is not supported by this verifier.
    #[error("unsupported {component} version: {version}")]
    UnsupportedVersion {
        /// Wire component being decoded.
        component: &'static str,
        /// Version encountered in the payload.
        version: u16,
    },

    /// Curve/backend identifiers in a payload do not match.
    #[error("curve mismatch: expected {expected:?}, got {actual:?}")]
    CurveMismatch {
        /// Expected curve/backend identifier.
        expected: crate::norito_types::ZkCurveId,
        /// Actual curve/backend identifier.
        actual: crate::norito_types::ZkCurveId,
    },

    /// Proof shape is inconsistent with the parameter domain.
    #[error("invalid proof shape for {reason}: expected {expected}, got {actual}")]
    InvalidProofShape {
        /// Shape property that failed validation.
        reason: &'static str,
        /// Expected value.
        expected: usize,
        /// Actual value.
        actual: usize,
    },

    /// Envelope exceeded a configured verification limit.
    #[error("envelope limit exceeded for {limit}: max {max}, got {actual}")]
    EnvelopeLimitExceeded {
        /// Limit that rejected the payload.
        limit: &'static str,
        /// Configured maximum.
        max: usize,
        /// Actual value.
        actual: usize,
    },

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
