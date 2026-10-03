//! Canonical section decoding with original local resource refusal preserved.

use crate::error::{ExecutionDeferral, VMError};
use norito::core::{DecodeAttemptErrorKind, DecodeLimits};

pub(super) fn decode<T>(bytes: &[u8], limits: DecodeLimits) -> Result<T, VMError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_for_admission(bytes, limits).map_err(|error| match error.kind() {
        DecodeAttemptErrorKind::Allocator => {
            VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
        }
        DecodeAttemptErrorKind::EnclosingLimit => {
            VMError::ExecutionDeferred(ExecutionDeferral::ActiveMemoryCapacity)
        }
        DecodeAttemptErrorKind::Invalid => VMError::InvalidMetadata,
    })
}

#[cfg(test)]
mod tests;
