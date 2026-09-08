//! Incremental measurement of generic element-sequence payload framing.

use super::{
    DecodeFlagsGuard, Error, SerializePayload, default_encode_flags, encoded_payload_len,
    header_flags, len_prefixed_payload_len_with_flags, packed_sequence_table_len,
    sequential_override_active, validate_header_flags,
};

/// Incrementally measure generic element-sequence payloads under one explicit layout.
///
/// Each successful [`Self::push`] measures the supplied element through one actual
/// [`encoded_payload_len`] call; length hints are never used. Only counts are retained, so copying
/// this value is a constant-size snapshot and [`Self::len`] is constant time. Appending costs the
/// serialization work for the new element plus constant-time checked arithmetic.
///
/// This uses the framing of [`super::write_element_sequence`]: an eight-byte element count followed
/// by a fixed-width offset table and payloads for `PACKED_SEQ`, or individually length-prefixed
/// payloads otherwise. The empty packed sequence includes its initial zero offset. This does **not**
/// measure the raw `Vec<u8>` specialization, which has no element framing. Pushing a `u8` here
/// measures one individually framed byte, as `write_element_sequence::<u8, _>` does.
///
/// The result records observed payload lengths, not element identity, semantic validity, or quota
/// authority. Elements are borrowed only during each push. Later mutation, interior mutability,
/// stateful serializers, or changed ambient inputs can make subsequent serialization differ; this
/// API neither certifies that later bytes match nor writes bytes. A caller claiming the length of a
/// later sequence must independently preserve the values and their serialization behavior.
///
/// Layout flags are validated and frozen. An active [`super::SequentialOverrideGuard`] is accepted
/// only for [`default_encode_flags`], whose layout it preserves. Measurement restores the enclosing layout
/// flags and encode tracking; it does not reset decode payload context, nesting depth or active
/// budgets. Resource charges incurred by an element serializer remain charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequencePayloadLength {
    flags: u8,
    count: usize,
    payload_bytes: usize,
    len: usize,
}

impl SequencePayloadLength {
    /// Measure an empty generic element sequence with validated, fixed layout flags.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedFeature`] for unsupported flags, invalid flag combinations, or
    /// an active sequential override incompatible with `flags`.
    pub fn new(flags: u8) -> Result<Self, Error> {
        validate_header_flags(flags)?;
        ensure_layout_available(flags)?;
        let framing = if flags & header_flags::PACKED_SEQ != 0 {
            packed_sequence_table_len(0)?
        } else {
            0
        };
        Ok(Self {
            flags,
            count: 0,
            payload_bytes: 0,
            len: 8usize.checked_add(framing).ok_or(Error::LengthMismatch)?,
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
    /// Returns [`Error::UnsupportedFeature`] if a sequential override conflicts with the stored
    /// layout; an initial conflict is rejected before invoking the serializer. Otherwise propagates
    /// serialization errors, then reports [`Error::LengthMismatch`] on unrepresentable counts,
    /// lengths or offsets.
    pub fn push(&mut self, value: &dyn SerializePayload) -> Result<(), Error> {
        ensure_layout_available(self.flags)?;
        let measured = {
            let _flags = DecodeFlagsGuard::enter(self.flags);
            encoded_payload_len(value)?
        };
        // A stateful serializer may leave an override guard alive outside this call. Recheck
        // before using prefix helpers, which themselves honor that ambient override.
        ensure_layout_available(self.flags)?;
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
        let count = self.count.checked_add(1).ok_or(Error::LengthMismatch)?;
        u64::try_from(count).map_err(|_| Error::LengthMismatch)?;
        u64::try_from(measured).map_err(|_| Error::LengthMismatch)?;
        let payload_bytes = self
            .payload_bytes
            .checked_add(measured)
            .ok_or(Error::LengthMismatch)?;
        let len = if self.flags & header_flags::PACKED_SEQ != 0 {
            // Each cumulative packed offset must fit its u64 wire representation as well as usize.
            u64::try_from(payload_bytes).map_err(|_| Error::LengthMismatch)?;
            8usize
                .checked_add(packed_sequence_table_len(count)?)
                .and_then(|framing| framing.checked_add(payload_bytes))
                .ok_or(Error::LengthMismatch)?
        } else {
            self.len
                .checked_add(
                    len_prefixed_payload_len_with_flags(measured, self.flags)
                        .ok_or(Error::LengthMismatch)?,
                )
                .ok_or(Error::LengthMismatch)?
        };
        Ok(Self {
            flags: self.flags,
            count,
            payload_bytes,
            len,
        })
    }
}

fn ensure_layout_available(flags: u8) -> Result<(), Error> {
    if sequential_override_active() && flags != default_encode_flags() {
        return Err(Error::UnsupportedFeature("sequence length layout override"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
