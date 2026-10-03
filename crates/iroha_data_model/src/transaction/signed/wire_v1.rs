//! UNLINKED DRAFT: one counted fixed-V1 payload for ordinary and admitted destinations.
//!
//! This is the existing version byte followed by its default-layout bare Norito payload, not
//! a header-framed transaction or a native external-entrypoint frame. A plan borrows the original
//! value and measures a real serialization pass. Its count is only an output extent: it neither
//! admits nor funds allocations made inside a concrete transaction's serializer.
//!
//! TODO: Link together with the inherent API and replace encode_default_layout_versioned in
//! signed.rs for BOTH SignedTransaction and TransactionEntrypoint. Their ordinary Vec encoder,
//! EncodeVersioned implementation and canonical decode re-encoding must use this same primitive.
//! Audit and fund nested serializer scratch before any original-State custody producer is live.

use norito::core::{DecodeFlagsGuard, Encoder, Error, SerializePayload};
use std::io::Write;

/// Borrowed, measured complete V1 transaction or entrypoint wire.
///
/// Construction is private to the canonical model. A plan supplies byte encoding only, never
/// authority, signature verification, finality, resource admission or permission to submit.
pub struct WireV1Plan<'a> {
    payload: VersionedPayloadV1<'a>,
    length: usize,
}

impl<'a> WireV1Plan<'a> {
    pub(super) fn new(value: &'a dyn SerializePayload) -> Result<Self, Error> {
        let payload = VersionedPayloadV1(value);
        let _layout = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let length = norito::core::encoded_payload_len(&payload)?;
        Ok(Self { payload, length })
    }

    /// Exact output extent, including the sole V1 version byte.
    #[must_use]
    pub const fn wire_length(&self) -> usize {
        self.length
    }

    /// Write this original value within the measured extent under the fixed V1 layout.
    ///
    /// An oversized serializer cannot write beyond the extent. A shorter pass is rejected too.
    /// Destination or serializer errors can leave an incomplete prefix; discard that attempt's
    /// destination on error. This call neither flushes nor publishes a destination.
    ///
    /// # Errors
    /// Returns the original codec/destination error, or LengthMismatch for a changed extent.
    pub fn write_to(&self, output: &mut impl Write) -> Result<(), Error> {
        let _layout = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::core::serialize_to_writer_exact(&self.payload, output, self.length)
    }

    /// Materialize the ordinary uncharged API's vector through the same exact writer.
    ///
    /// This allocation is deliberately not represented as an original-State charged allocation.
    /// Callers needing admitted storage supply their real destination to write_to instead.
    pub(super) fn into_vec(self) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.length)
            .map_err(|_| Error::AllocationFailed {
                bytes: self.length as u64,
            })?;
        self.write_to(&mut bytes)?;
        Ok(bytes)
    }
}

// This sole payload adapter is traversed by both the counter and the real writer. Norito's
// native counting destination avoids re-traversing already measured nested field bytes.
struct VersionedPayloadV1<'a>(&'a dyn SerializePayload);

impl SerializePayload for VersionedPayloadV1<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&[1])?;
        self.0.serialize(writer)
    }
}

#[cfg(test)]
#[path = "wire_v1_tests.rs"]
mod tests;
