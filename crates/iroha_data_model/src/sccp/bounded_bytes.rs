//! Bounded opaque byte wrappers shared by SCCP v1 proofs, advances and evidence.
//!
//! Proofs, light-client advances and equivocation evidence are headered Norito frames of
//! per-chain structures that only `iroha_sccp::light_client` decodes, so the data model stays
//! independent of chain formats. Each wrapper enforces `1..=MAX` bytes at every construction
//! and decode boundary: its `new` constructor, the binary codec (through the Norito `validate`
//! hook, which also covers slice decoding) and JSON; violations are
//! [`SccpBoundedBytesError`]s.

/// A bounded SCCP byte wrapper was empty or too long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SccpBoundedBytesError {
    /// The wrapper holds no bytes.
    #[error("{kind} is empty")]
    Empty {
        /// Wrapper kind.
        kind: &'static str,
    },
    /// The wrapper exceeds its maximum length.
    #[error("{kind} has {len} bytes; at most {max} are allowed")]
    TooLong {
        /// Wrapper kind.
        kind: &'static str,
        /// Actual length.
        len: usize,
        /// Maximum length.
        max: usize,
    },
}

/// Check that `len` is within `1..=max` for a wrapper of `kind`.
///
/// # Errors
///
/// Returns [`SccpBoundedBytesError`] for an empty or over-long wrapper.
pub const fn check_bounded_len(
    kind: &'static str,
    len: usize,
    max: usize,
) -> Result<(), SccpBoundedBytesError> {
    if len == 0 {
        Err(SccpBoundedBytesError::Empty { kind })
    } else if len > max {
        Err(SccpBoundedBytesError::TooLong { kind, len, max })
    } else {
        Ok(())
    }
}

/// Implement the bounded-wrapper API for a struct `{ bytes: Vec<u8> }` that derives the Norito
/// codecs with `#[norito(validate = "Self::checked")]` and JSON serialization only.
macro_rules! impl_sccp_bounded_bytes {
    ($ty:ident, $max:expr, $kind:literal) => {
        impl $ty {
            /// Maximum byte length.
            pub const MAX_BYTES: usize = $max;

            /// Wrap `bytes`, which must hold `1..=MAX_BYTES` bytes.
            ///
            /// # Errors
            ///
            /// Returns [`SccpBoundedBytesError`](crate::sccp::bounded_bytes::SccpBoundedBytesError)
            /// for an empty or over-long input.
            pub fn new(
                bytes: Vec<u8>,
            ) -> Result<Self, crate::sccp::bounded_bytes::SccpBoundedBytesError> {
                crate::sccp::bounded_bytes::check_bounded_len($kind, bytes.len(), $max)?;
                Ok(Self { bytes })
            }

            /// Return the wrapped bytes.
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                &self.bytes
            }

            /// Return the wrapped bytes by value.
            #[must_use]
            pub fn into_bytes(self) -> Vec<u8> {
                self.bytes
            }

            /// Return the number of wrapped bytes.
            #[must_use]
            pub fn len(&self) -> usize {
                self.bytes.len()
            }

            /// Return whether the wrapper is empty (never true for a constructed value).
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.bytes.is_empty()
            }

            /// Norito decode hook: reject values outside `1..=MAX_BYTES`.
            fn checked(self) -> Result<Self, norito::Error> {
                crate::sccp::bounded_bytes::check_bounded_len($kind, self.bytes.len(), $max)
                    .map_err(|error| norito::Error::Message(error.to_string()))?;
                Ok(self)
            }
        }

        impl norito::json::JsonDeserialize for $ty {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                #[derive(crate::DeriveJsonDeserialize)]
                #[norito(deny_unknown_fields, no_fast_from_json)]
                struct Fields {
                    #[norito(json = "crate::json_helpers::base64_vec")]
                    bytes: Vec<u8>,
                }
                let fields = <Fields as norito::json::JsonDeserialize>::json_deserialize(parser)?;
                Self::new(fields.bytes)
                    .map_err(|error| norito::json::Error::Message(error.to_string()))
            }
        }
    };
}
pub(crate) use impl_sccp_bounded_bytes;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_check_is_one_through_max() {
        assert_eq!(
            check_bounded_len("proof", 0, 4),
            Err(SccpBoundedBytesError::Empty { kind: "proof" })
        );
        assert_eq!(check_bounded_len("proof", 1, 4), Ok(()));
        assert_eq!(check_bounded_len("proof", 4, 4), Ok(()));
        let error = check_bounded_len("proof", 5, 4).expect_err("too long");
        assert_eq!(
            error,
            SccpBoundedBytesError::TooLong {
                kind: "proof",
                len: 5,
                max: 4
            }
        );
        assert_eq!(
            error.to_string(),
            "proof has 5 bytes; at most 4 are allowed"
        );
    }
}
