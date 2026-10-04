//! Caller-prepared spans using the same canonical element-sequence parser.

use super::*;

/// Original byte failure or an unchanged caller-owned scratch planning mismatch.
#[derive(Debug, thiserror::Error)]
pub enum SequenceDestinationError {
    /// Original canonical parsing or active logical-budget failure.
    #[error(transparent)]
    Codec(#[from] Error),
    /// The prepared bank is smaller than the already-admitted input shape.
    /// This is local destination geometry, never intrinsic protocol invalidity.
    #[error("prepared sequence holds {available} slots but needs {required}")]
    Storage {
        /// Number of initialized slots selected before the attempt.
        available: usize,
        /// Original declared count, after canonical sequence-limit admission.
        required: usize,
    },
}

/// Complete element spans borrowed from the original input and prepared scratch.
///
/// The entire original sequence framing is checked before an element is decoded,
/// preserving the ordinary owned planner's late-framing-error precedence. No
/// span Vec is allocated and no scratch escapes its exclusive preparation borrow.
pub struct PreparedElementSequence<'input, 'scratch> {
    bytes: &'input [u8],
    spans: &'scratch [SequenceSpan],
    used: usize,
}
impl PreparedElementSequence<'_, '_> {
    /// Original number of canonically framed elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }
    /// Whether the admitted sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
    /// Exact original sequence prefix length; outer boundaries reject trailing bytes.
    #[must_use]
    pub fn used(&self) -> usize {
        self.used
    }

    /// Visit canonical element fields using the same owning leaf relationship.
    ///
    /// This is the generic element layout even for `u8`, as used by `ConstVec`;
    /// `Vec<u8>`'s raw layout uses [`decode_raw_byte_sequence_into`]. Each caller
    /// uses [`CanonicalField::with_payload`] and prepared leaf/record storage.
    /// Nominal sequence storage remains charged to the active logical wire-work
    /// limits exactly as in the owning Vec decoder. This is not physical pool
    /// admission: the original owner must already hold all element destinations.
    ///
    /// # Errors
    /// Returns the original codec or caller-owned destination cause. A prefix may
    /// have been written on error; the enclosing retained owner resets validity
    /// without replacing any destination backing before reuse.
    pub fn decode_elements<T, E>(
        &self,
        mut visit: impl FnMut(usize, CanonicalField<'_, T>) -> Result<(), DecodeIntoError<E>>,
    ) -> Result<(), DecodeIntoError<E>>
    where
        T: for<'de> DeserializePayload<'de> + SerializePayload,
    {
        reserve_decode_sequence_storage::<T>(self.spans.len())?;
        for (index, span) in self.spans.iter().enumerate() {
            let element = span.get(self.bytes)?;
            record_slice_access(element, span.len());
            visit(
                index,
                field_destination::canonical_field_from_slice(element),
            )?;
        }
        Ok(())
    }
}

/// Plan an element sequence into initialized scratch using its sole scalar walker.
///
/// This uses the same count admission, advertised flags, minimum framing check,
/// logical span-storage charge, span order and complete-prefix consumption as
/// [`plan_binary_sequence`]. The initialized scratch may be larger than the
/// actual count; unused slots are never read or included in the resulting view.
/// No allocation, resize, raw lifetime conversion or heap fallback occurs.
///
/// # Errors
/// Preserves the original codec/resource failure. A short prepared destination
/// is a distinct local planning error. Partial span writes do not create a plan;
/// the unchanged scratch can be reused after any error.
pub fn prepare_element_sequence<'input, 'scratch>(
    bytes: &'input [u8],
    scratch: &'scratch mut [SequenceSpan],
) -> Result<PreparedElementSequence<'input, 'scratch>, SequenceDestinationError> {
    let (count, _) = read_seq_len_slice(bytes)?;
    let flags = effective_decode_flags().unwrap_or_else(default_encode_flags);
    validate_header_flags(flags)?;
    validate_binary_sequence_reservation(bytes, flags, count)?;
    if count > scratch.len() {
        return Err(SequenceDestinationError::Storage {
            available: scratch.len(),
            required: count,
        });
    }
    reserve_decode_sequence_storage::<SequenceSpan>(count)?;
    let mut index = 0;
    let used = byte_sequence::visit_binary_sequence_with_count(bytes, flags, count, |span| {
        scratch[index] = span;
        index += 1;
        Ok(())
    })?;
    note_payload_access(bytes, used);
    Ok(PreparedElementSequence {
        bytes,
        spans: &scratch[..count],
        used,
    })
}

/// Fill initialized storage from the canonical raw `Vec<u8>` payload layout.
///
/// Count/field/resource ordering and nominal retained-byte accounting match the
/// owning raw-byte specialization. The same borrowed slice decoder owns framing;
/// no element-sequence interpretation or alternate codec is attempted.
///
/// # Errors
/// Returns original codec/resource failure or distinct local destination geometry.
/// Failure retains all backing. Successful output is exactly `destination[..count]`.
pub fn decode_raw_byte_sequence_into(
    bytes: &[u8],
    destination: &mut [u8],
) -> Result<(usize, usize), SequenceDestinationError> {
    let (source, used) = <&[u8] as DecodeFromSlice>::decode_from_slice(bytes)?;
    if source.len() > destination.len() {
        return Err(SequenceDestinationError::Storage {
            available: destination.len(),
            required: source.len(),
        });
    }
    reserve_decode_sequence_storage::<u8>(source.len())?;
    destination[..source.len()].copy_from_slice(source);
    Ok((source.len(), used))
}
