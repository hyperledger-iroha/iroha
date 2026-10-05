//! Borrowed canonical signature framing without an unfunded payload allocation.

use crate::PreparedCryptoDecodeError;

use super::{
    Signature, ncore, signature_payload_geometry, validate_signature_payload,
    validate_signature_payload_observation, visit_signature_payload_elements,
};

/// Original canonical failure or a local change to the borrowed source's layout.
#[derive(Debug, thiserror::Error)]
pub enum BorrowedSignaturePayloadError {
    /// Original codec/resource, fixed signature-payload or destination-geometry cause.
    #[error(transparent)]
    Decode(#[from] PreparedCryptoDecodeError),
    /// The caller changed the advertised layout after this source was checked.
    /// This is a local source constraint, never evidence of invalid signature bytes.
    #[error("borrowed signature layout changed from {expected:#04x} to {actual:#04x}")]
    LayoutChanged {
        /// Original effective flags at the complete source walk.
        expected: u8,
        /// Effective flags at the attempted destination fill.
        actual: u8,
    },
}
impl From<ncore::Error> for BorrowedSignaturePayloadError {
    fn from(error: ncore::Error) -> Self {
        Self::Decode(PreparedCryptoDecodeError::Codec(error))
    }
}

/// Original complete encoded signature payload and its exact byte count.
///
/// The bytes are the canonical count plus individually framed byte elements,
/// not a contiguous decoded signature. This private-field view borrows its
/// original immutable source; it creates no allocation or signature authority.
/// The caller must retain that source's actual custody, advertised flags and
/// enclosing field/depth/protocol checks. No complete-frame claim is made here.
pub struct BorrowedSignaturePayload<'a> {
    encoded: &'a [u8],
    count: usize,
    flags: u8,
}

impl Signature {
    /// Check one complete canonical payload while borrowing its original framing.
    ///
    /// This uses the same count charge and sole byte walk as ordinary decoding.
    /// Empty/all-zero rejection occurs after complete framing, with the same
    /// fixed cause as the existing prepared decoder; no algorithm or exact
    /// signature length is inferred. Every source inspection remains charged
    /// to the active logical limits, independently of physical custody.
    ///
    /// # Errors
    /// Returns original framing/resource or fixed payload-validity causes.
    /// Trailing input is rejected as in the ordinary signature payload decoder.
    pub fn borrow_canonical_payload(
        bytes: &[u8],
    ) -> Result<BorrowedSignaturePayload<'_>, BorrowedSignaturePayloadError> {
        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        let (count, offset) = signature_payload_geometry(bytes)?;
        let mut has_nonzero = false;
        visit_signature_payload_elements(bytes, offset, count, |_, byte| {
            has_nonzero |= byte != 0;
        })?;
        // The archived owner records complete access before fixed payload
        // validation. Preserve that relationship for a prepared field visitor.
        ncore::note_payload_access(bytes, bytes.len());
        validate_signature_payload_observation(count, has_nonzero)
            .map_err(PreparedCryptoDecodeError::Signature)?;
        Ok(BorrowedSignaturePayload {
            encoded: bytes,
            count,
            flags,
        })
    }
}

impl<'a> BorrowedSignaturePayload<'a> {
    /// Borrow the exact original count and individually framed byte elements.
    #[must_use]
    pub fn encoded_payload(&self) -> &'a [u8] {
        self.encoded
    }
    /// Exact number of decoded payload bytes, without a fixed algorithm width.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }
    /// Whether the canonical payload is empty; successful construction rejects emptiness.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Exact consumed encoded extent, including count and element framing.
    #[must_use]
    pub fn used(&self) -> usize {
        self.encoded.len()
    }
    /// Fill an initialized exact-size destination through the sole canonical byte walk.
    ///
    /// No allocation, capacity growth, owned decode, clone or authentication
    /// occurs. The caller admits all physical destinations before this call.
    /// Repeated fills retain original source/flags and repeat the canonical
    /// logical count/framing work; prepaid storage does not erase those limits.
    /// A late codec refusal can leave a written prefix, so enclosing owners must
    /// publish validity only after success, just as the existing prepared owner.
    ///
    /// # Errors
    /// Returns a distinct local layout change or original codec/resource/fixed
    /// payload cause. A different initialized length is local Geometry. Every
    /// failure retains caller storage and does not refund its original charge.
    pub fn fill_into(
        &self,
        destination: &mut [u8],
    ) -> Result<usize, BorrowedSignaturePayloadError> {
        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        if flags != self.flags {
            return Err(BorrowedSignaturePayloadError::LayoutChanged {
                expected: self.flags,
                actual: flags,
            });
        }
        let (count, offset) = signature_payload_geometry(self.encoded)?;
        if destination.len() != count {
            return Err(PreparedCryptoDecodeError::Geometry {
                expected: destination.len(),
                offered: count,
            }
            .into());
        }
        super::decode_signature_payload_elements(self.encoded, offset, destination)?;
        ncore::note_payload_access(self.encoded, self.encoded.len());
        validate_signature_payload(destination).map_err(PreparedCryptoDecodeError::Signature)?;
        Ok(self.encoded.len())
    }
}

#[cfg(test)]
mod tests;
