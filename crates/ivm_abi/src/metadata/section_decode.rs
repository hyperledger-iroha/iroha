//! Canonical section decoding with original local resource refusal preserved.

use crate::error::VMError;
use crate::error::{VmDeferral, VmTrap};
use norito::core::DecodeLimits;

pub(super) fn decode<T>(bytes: &[u8], limits: DecodeLimits) -> Result<T, VMError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_for_admission(bytes, limits).map_err(|error| {
        match error.into_local_refusal() {
            Ok(original) => VMError::Deferred(VmDeferral::DecodeDeferred(original)),
            Err(_) => VMError::Trap(VmTrap::InvalidMetadata),
        }
    })
}

#[cfg(test)]
mod tests;
