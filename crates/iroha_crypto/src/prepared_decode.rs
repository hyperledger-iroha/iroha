//! Reusable original-pool destinations for canonical crypto leaf decoding.
//!
//! These owners fund only retained byte backing. The enclosing decoder still
//! owns its frame, active logical limits, scope controls and scratch. Callers
//! must validate protocol-specific algorithm/length geometry before selecting
//! an exact destination. A geometry refusal here is a local planning error, not
//! evidence that the offered bytes are protocol-invalid.

use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer};
use norito::core::{Encoder, Error, SerializePayload};

use crate::{
    ChargedPublicKey, ChargedSignature, PublicKey, PublicKeyAllocationError, PublicKeyCompact,
    SignatureAllocationError, SignaturePayloadError, public_key_decode, signature,
};

/// A canonical leaf failure or a distinct mismatch with its prepared destination.
#[derive(Debug)]
pub enum PreparedCryptoDecodeError {
    /// Original framing, public-key validity or active decoder resource failure.
    Codec(Error),
    /// The authenticated/planned output geometry differs from the offered leaf.
    /// This variant must remain a local destination failure at protocol adapters.
    Geometry {
        /// Exact original allocation capacity selected before decoding.
        expected: usize,
        /// Decoded payload length (including the key algorithm tag for keys).
        offered: usize,
    },
    /// The canonical signature byte validity rule rejected the payload.
    Signature(SignaturePayloadError),
}

impl From<Error> for PreparedCryptoDecodeError {
    fn from(error: Error) -> Self {
        Self::Codec(error)
    }
}
impl std::fmt::Display for PreparedCryptoDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(error) => error.fmt(formatter),
            Self::Geometry { expected, offered } => write!(
                formatter,
                "prepared crypto destination holds {expected} bytes, offered leaf holds {offered}"
            ),
            Self::Signature(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for PreparedCryptoDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Signature(error) => Some(error),
            Self::Geometry { .. } => None,
        }
    }
}

struct InitializedBytes {
    bytes: ChargedBuffer<u8>,
    ready: bool,
}
impl InitializedBytes {
    fn new(mut bytes: ChargedBuffer<u8>) -> Self {
        // Initialization precedes any decode attempt. No unsafe exposure or
        // capacity growth is needed, and even malformed input can overwrite the
        // complete initialized slice while retaining the original allocation.
        for _ in 0..bytes.capacity() {
            bytes.push_reserved(0);
        }
        Self {
            bytes,
            ready: false,
        }
    }
    fn decoded(&self) -> Option<&[u8]> {
        self.ready.then(|| self.bytes.as_slice())
    }
    fn check_length(&self, offered: usize) -> Result<(), PreparedCryptoDecodeError> {
        if offered != self.bytes.capacity() {
            return Err(PreparedCryptoDecodeError::Geometry {
                expected: self.bytes.capacity(),
                offered,
            });
        }
        Ok(())
    }
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let payload = self.decoded().ok_or(Error::InvalidValue {
            context: "unfinished prepared crypto destination",
        })?;
        // This is the canonical ConstVec element encoder, not Vec<u8>'s raw
        // byte specialization. No second protocol type/schema is introduced.
        norito::core::write_element_sequence::<u8, _>(writer, payload.iter())
    }
}

/// Exact, initialized signature storage admitted before a decoding attempt.
///
/// Failed decoding and [`Self::reset`] retain the same backing and charge.
/// Successful decoding establishes codec validity only, not authentication.
/// There is no Clone or mutable/owning byte escape.
pub struct PreparedSignatureDecode(InitializedBytes);

impl PreparedSignatureDecode {
    /// Allocate and initialize the exact offered original charge before decoding.
    ///
    /// # Errors
    /// Returns the unchanged charge on foreign source, layout mismatch/overflow,
    /// or a physical allocator refusal. No admission or refund occurs on error.
    pub fn try_from_charge(
        exact_bytes: usize,
        budget: &AllocationBudget,
        charge: AllocationCharge,
    ) -> Result<Self, (AllocationCharge, SignatureAllocationError)> {
        if !charge.belongs_to(budget) {
            return Err((charge, SignatureAllocationError::ForeignPool));
        }
        let bytes = ChargedBuffer::try_from_charge(exact_bytes, charge)
            .map_err(|(charge, error)| (charge, SignatureAllocationError::Allocation(error)))?;
        Ok(Self(InitializedBytes::new(bytes)))
    }

    /// Decode one complete canonical signature payload into retained storage.
    /// The caller owns the advertised flags and enclosing decode scope.
    ///
    /// # Errors
    /// Preserves the original codec failure, fixed invalid-payload cause, or
    /// local prepared-geometry mismatch. Every error clears validity, including
    /// after a prior successful attempt, without refunding or replacing storage.
    pub fn decode_payload(&mut self, bytes: &[u8]) -> Result<(), PreparedCryptoDecodeError> {
        self.reset();
        let (length, raw_start) = signature::signature_payload_geometry(bytes)?;
        self.0.check_length(length)?;
        signature::decode_signature_payload_elements(
            bytes,
            raw_start,
            self.0.bytes.as_mut_slice(),
        )?;
        signature::validate_signature_payload(self.0.bytes.as_slice())
            .map_err(PreparedCryptoDecodeError::Signature)?;
        self.0.ready = true;
        Ok(())
    }

    /// Forget decoded validity while preserving the exact initialized backing.
    pub fn reset(&mut self) {
        self.0.ready = false;
    }

    /// Borrow validated bytes for the enclosing canonical stream comparison.
    /// This is codec validity only; signature authentication remains mandatory.
    #[must_use]
    pub fn decoded_payload(&self) -> Option<&[u8]> {
        self.0.decoded()
    }

    /// Whether this destination retains the exact original allocation source.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.bytes.belongs_to(budget)
    }

    /// Move the same complete allocation into the canonical immutable signature.
    ///
    /// # Errors
    /// Returns the unchanged destination unless its last decode completed. No
    /// allocation, shrinking, cloning, refund or revalidation occurs on success.
    pub fn finish(self) -> Result<ChargedSignature, Self> {
        if !self.0.ready {
            return Err(self);
        }
        Ok(signature::Signature::bind_payload_allocation(self.0.bytes))
    }
}

impl SerializePayload for PreparedSignatureDecode {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

/// Exact, initialized compact public-key storage admitted before decoding.
///
/// The fixed-stack canonical parser validates the key before copying into this
/// original backing. The stack scratch is bounded by the existing maximum key
/// size; it is not a heap allocation or a claim of enclosing stack admission.
pub struct PreparedPublicKeyDecode(InitializedBytes);

impl PreparedPublicKeyDecode {
    /// Allocate and initialize the exact compact allocation, including its tag.
    ///
    /// # Errors
    /// Returns the unchanged original charge on source/layout/allocator refusal.
    pub fn try_from_charge(
        exact_bytes: usize,
        budget: &AllocationBudget,
        charge: AllocationCharge,
    ) -> Result<Self, (AllocationCharge, PublicKeyAllocationError)> {
        if !charge.belongs_to(budget) {
            return Err((charge, PublicKeyAllocationError::ForeignPool));
        }
        let bytes = ChargedBuffer::try_from_charge(exact_bytes, charge)
            .map_err(|(charge, error)| (charge, PublicKeyAllocationError::Allocation(error)))?;
        Ok(Self(InitializedBytes::new(bytes)))
    }

    /// Decode one complete compact payload with the sole canonical key validator.
    /// The caller owns the advertised flags and enclosing decode scope.
    ///
    /// # Errors
    /// Returns the original codec failure or a distinct local destination shape
    /// failure. The exact backing survives, and stale validity is always cleared.
    pub fn decode_payload(&mut self, bytes: &[u8]) -> Result<(), PreparedCryptoDecodeError> {
        self.reset();
        public_key_decode::with_decoded_compact(bytes, true, |algorithm, payload| {
            self.0.check_length(payload.len() + 1)?;
            // Prepaid physical storage does not relax canonical logical work
            // ceilings. Preserve the ordinary compact owner's second charge
            // after identical tag/point validation and before any copied byte.
            public_key_decode::reserve_compact_decode_backing(payload.len())?;
            let destination = self.0.bytes.as_mut_slice();
            destination[0] = PublicKeyCompact::algorithm_tag(algorithm);
            destination[1..].copy_from_slice(payload);
            Ok::<_, PreparedCryptoDecodeError>(())
        })?;
        self.0.ready = true;
        Ok(())
    }

    /// Forget decoded validity without changing original storage or charge.
    pub fn reset(&mut self) {
        self.0.ready = false;
    }

    /// Borrow validated canonical tag/payload bytes for exact stream comparison.
    #[must_use]
    pub fn decoded_compact(&self) -> Option<&[u8]> {
        self.0.decoded()
    }

    /// Whether this destination retains the exact original allocation source.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.bytes.belongs_to(budget)
    }

    /// Move complete original storage into the immutable canonical public key.
    ///
    /// # Errors
    /// Returns the unchanged destination unless the last decode succeeded. The
    /// fully filled exact buffer cannot shrink while being converted to a Box.
    pub fn finish(self) -> Result<ChargedPublicKey, Self> {
        if !self.0.ready {
            return Err(self);
        }
        Ok(PublicKey::bind_compact_allocation(self.0.bytes))
    }
}
impl SerializePayload for PreparedPublicKeyDecode {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

#[cfg(test)]
mod tests;
