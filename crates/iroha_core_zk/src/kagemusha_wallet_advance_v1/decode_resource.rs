//! Actual monetary-original decode boundaries, before any invalid/custody-loss projection.
//! The retained reason is diagnostic only; it never grants proof or selection authority.

use super::{KagemushaWalletDecodeResourceV1, KagemushaWalletProviderErrorV1 as Error};
use iroha_data_model::kagemusha::KagemushaWalletValidationErrorV1;

fn refusal(error: norito::core::DecodeAttemptError, field: &'static str) -> Error {
    use norito::core::DecodeAttemptErrorKind as Kind;
    match error.kind() {
        Kind::Allocator => Error::DecodeResource(KagemushaWalletDecodeResourceV1::Allocator),
        Kind::EnclosingLimit => {
            Error::DecodeResource(KagemushaWalletDecodeResourceV1::EnclosingLimit)
        }
        Kind::Invalid => Error::Invalid { field },
    }
}

/// Observe this actual model read before its nested Codec error is projected.
pub(crate) fn read_model<T>(
    read: impl FnOnce() -> Result<T, KagemushaWalletValidationErrorV1>,
    field: &'static str,
) -> Result<T, Error> {
    let completed = norito::core::classify_decode_attempt(|| match read() {
        Err(KagemushaWalletValidationErrorV1::Codec(error)) => Err(error),
        result => Ok(result),
    });
    match completed {
        Err(error) => Err(refusal(error, field)),
        Ok(result) => result.map_err(|_| Error::Invalid { field }),
    }
}

pub(super) fn read_canonical<T>(bytes: &[u8], maximum: usize) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > maximum {
        return Err(Error::Invalid {
            field: "envelope size",
        });
    }
    norito::core::classify_decode_attempt(|| {
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
    })
    .map_err(|error| refusal(error, "envelope"))
}

/// Project completed malformed originals only. A local unfinished read must stop recovery.
pub(super) fn completed<T>(result: Result<T, Error>) -> Result<Option<T>, Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(Error::Invalid { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
