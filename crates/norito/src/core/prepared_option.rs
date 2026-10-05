//! One optional-payload framing kernel for owning and prepared destinations.

use super::*;

/// Inspect the original option prefix without decoding or allocating its child.
/// The caller keeps its existing invalid-tag diagnostic and child decoder.
pub(super) fn option_payload_prefix(
    bytes: &[u8],
    invalid_tag: fn(u8) -> Error,
) -> Result<(Option<&[u8]>, usize), Error> {
    let tag = *bytes.first().ok_or(Error::LengthMismatch)?;
    record_slice_access(bytes, 1);
    match tag {
        0 => Ok((None, 1)),
        1 => {
            let (payload, used) = take_length_prefixed_field(bytes, 1)?;
            Ok((Some(payload), used))
        }
        tag => Err(invalid_tag(tag)),
    }
}

impl<T> CanonicalField<'_, Option<T>>
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    /// Decode this optional field into the caller's original prepared destination.
    ///
    /// `None` does not visit or allocate a child. `Some` passes the exact typed
    /// child field through the same prefix kernel as owning option decoding.
    /// The visitor uses [`CanonicalField::with_payload`] or the owning decoder
    /// to preserve the child's length, flags, depth and consumption checks.
    /// Complete outer consumption is checked after the child, preserving the
    /// owning decoder's child-before-trailing-byte error precedence.
    ///
    /// This method provides no storage or semantic authority. The caller must
    /// retain every prepared child through refusal and authenticate the complete
    /// filled enclosing record with its canonical frame verifier.
    ///
    /// # Errors
    /// Returns the original canonical framing, child or destination refusal.
    /// No alignment copy, heap allocation or alternative decoder is introduced.
    pub fn decode_optional<R, E>(
        self,
        visit: impl FnOnce(CanonicalField<'_, T>) -> Result<R, DecodeIntoError<E>>,
    ) -> Result<Option<R>, DecodeIntoError<E>> {
        self.with_payload(|bytes| {
            let (payload, used) = option_payload_prefix(bytes, |tag| {
                Error::invalid_tag("Option::try_deserialize", tag)
            })?;
            let value = match payload {
                None => None,
                Some(payload) => Some(visit(field_destination::canonical_field_from_slice(
                    payload,
                ))?),
            };
            record_slice_access(bytes, used);
            if used != bytes.len() {
                return Err(Error::LengthMismatch.into());
            }
            Ok(value)
        })
    }
}
