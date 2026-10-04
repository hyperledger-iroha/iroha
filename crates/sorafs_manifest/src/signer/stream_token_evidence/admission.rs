//! Original codec provenance and the exact privately retained observation attempt.

use super::SignerStreamTokenEvidenceErrorV1;
use std::fmt;

/// A completed evidence rejection or an original canonical codec refusal.
///
/// The codec owner establishes origin before its scopes retire. No numeric limit or
/// reconstructed wire-shaped error can become an enclosing admission refusal.
#[derive(Debug)]
pub enum SignerStreamTokenEvidenceAdmissionErrorV1 {
    /// A completed semantic rejection; it must not trigger a replacement observation.
    Rejected(SignerStreamTokenEvidenceErrorV1),
    /// Original canonical codec error, including its admission provenance.
    Codec(norito::core::DecodeAttemptError),
    /// Original bounded canonical output failure, without a fabricated decode scope.
    Encoding(norito::core::BoundedEncodeError),
    /// Actual domain-prefixed output reservation failed before either slice was copied.
    Allocation(std::collections::TryReserveError),
}

impl SignerStreamTokenEvidenceAdmissionErrorV1 {
    /// Whether only local allocation or an original enclosing budget blocked this attempt.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Codec(error) => matches!(
                error.kind(),
                norito::core::DecodeAttemptErrorKind::Allocator
                    | norito::core::DecodeAttemptErrorKind::EnclosingLimit
            ),
            Self::Encoding(
                norito::core::BoundedEncodeError::AllocationFailed { .. }
                | norito::core::BoundedEncodeError::Serialization(norito::Error::AllocationFailed {
                    ..
                }),
            )
            | Self::Allocation(_) => true,
            Self::Rejected(_) | Self::Encoding(_) => false,
        }
    }

    /// Completed payload-free rejection, without discarding the retained original codec error.
    #[must_use]
    pub fn rejection(&self) -> Option<SignerStreamTokenEvidenceErrorV1> {
        match self {
            Self::Rejected(error) => Some(*error),
            Self::Codec(error) if error.kind() == norito::core::DecodeAttemptErrorKind::Invalid => {
                Some(SignerStreamTokenEvidenceErrorV1::InvalidDocument)
            }
            Self::Codec(_) => None,
            Self::Encoding(_) if !self.is_retryable() => {
                Some(SignerStreamTokenEvidenceErrorV1::InvalidDocument)
            }
            Self::Encoding(_) | Self::Allocation(_) => None,
        }
    }
}

impl From<SignerStreamTokenEvidenceErrorV1> for SignerStreamTokenEvidenceAdmissionErrorV1 {
    fn from(error: SignerStreamTokenEvidenceErrorV1) -> Self {
        Self::Rejected(error)
    }
}

impl fmt::Display for SignerStreamTokenEvidenceAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(error) => error.fmt(formatter),
            Self::Codec(_) | Self::Encoding(_) | Self::Allocation(_) => {
                formatter.write_str("stream-token evidence canonical admission failed")
            }
        }
    }
}
impl std::error::Error for SignerStreamTokenEvidenceAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Rejected(error) => error,
            Self::Codec(error) => error,
            Self::Encoding(error) => error,
            Self::Allocation(error) => error,
        })
    }
}
