//! Incremental measurement of generic element-sequence payload framing.

use super::{
    DecodeFlagsGuard, Error, SerializePayload, encoded_payload_len,
    len_prefixed_payload_len_with_flags, seq_len_prefix_len_with_flags, validate_layout_code,
};

/// Incrementally measure generic element-sequence payloads under one explicit layout.
///
/// Each successful [`Self::push`] measures the supplied element through one actual
/// [`encoded_payload_len`] call; length hints are never used. Only counts are retained, so copying
/// this value is a constant-size snapshot and [`Self::len`] is constant time. Appending costs the
/// serialization work for the new element plus constant-time checked arithmetic.
///
/// This uses the framing of [`super::write_element_sequence`]: an element count followed by
/// individually length-prefixed payloads, with the count and each prefix sized by the stored layout
/// code (fixed-width `u64` or varints; the dense layout's count is a varint). This does **not**
/// measure raw byte sequences, which have no element framing: `Vec<u8>` in every layout, and every
/// sequence of `u8` in the dense layout. In minor-0 layouts, pushing a `u8` measures one
/// individually framed byte, as `write_element_sequence::<u8, _>` does there.
///
/// The result records observed payload lengths, not element identity, semantic validity, or quota
/// authority. Elements are borrowed only during each push. Later mutation, interior mutability,
/// stateful serializers, or changed ambient inputs can make subsequent serialization differ; this
/// API neither certifies that later bytes match nor writes bytes. A caller claiming the length of a
/// later sequence must independently preserve the values and their serialization behavior.
///
/// Layout flags are validated and frozen. Measurement restores the enclosing layout flags and
/// encode tracking; it does not reset decode payload context, nesting depth or active budgets.
/// Resource charges incurred by an element serializer remain charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequencePayloadLength {
    flags: u8,
    count: usize,
    len: usize,
    /// Framed element bytes, excluding the count.
    elements: usize,
}

impl SequencePayloadLength {
    /// Measure an empty generic element sequence (its count alone) under a validated, fixed
    /// layout code.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedFeature`] unless `flags` is a minor-0 flag set or the dense
    /// layout code.
    pub fn new(flags: u8) -> Result<Self, Error> {
        validate_layout_code(flags)?;
        Ok(Self {
            flags,
            count: 0,
            len: seq_len_prefix_len_with_flags(0, flags),
            elements: 0,
        })
    }

    /// Measure one supplied element and append its observed payload length atomically.
    ///
    /// Does not revisit any previously measured element or trust length hints. The accumulator is
    /// unchanged on error. Side effects inside an element serializer, including resource charges,
    /// cannot be rolled back by this measurement API.
    ///
    /// # Errors
    ///
    /// Propagates serialization errors, then reports [`Error::LengthMismatch`] on unrepresentable
    /// counts or lengths.
    pub fn push(&mut self, value: &dyn SerializePayload) -> Result<(), Error> {
        let measured = {
            let _flags = DecodeFlagsGuard::enter(self.flags);
            encoded_payload_len(value)?
        };
        let next = self.checked_append(measured)?;
        *self = next;
        Ok(())
    }

    /// Return the measured sequence payload length, including its count and element framing.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Return the number of successfully measured elements.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Return whether no elements have been measured.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Return the validated layout flags fixed at construction.
    #[must_use]
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    // Arithmetic only; deliberately private so callers cannot supply fabricated measured lengths.
    fn checked_append(&self, measured: usize) -> Result<Self, Error> {
        let count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::LengthMismatch)?;
        u64::try_from(count).map_err(|_| Error::LengthMismatch)?;
        u64::try_from(measured).map_err(|_| Error::LengthMismatch)?;
        let elements = self
            .elements
            .checked_add(
                len_prefixed_payload_len_with_flags(measured, self.flags)
                    .ok_or_else(|| Error::LengthMismatch)?,
            )
            .ok_or_else(|| Error::LengthMismatch)?;
        let len = elements
            .checked_add(seq_len_prefix_len_with_flags(count, self.flags))
            .ok_or_else(|| Error::LengthMismatch)?;
        Ok(Self {
            flags: self.flags,
            count,
            len,
            elements,
        })
    }
}

#[cfg(test)]
mod tests;
