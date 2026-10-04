//! Borrowed canonical text under the original nominal frame declaration.
//!
//! These operations create no decode-budget layer, aligned copy, replacement
//! string or retained thread-local capacity. Model syntax and normalization are
//! separate obligations: canonical framing alone does not validate an identity.

use super::{
    Error, ExactSliceWriter, NoritoSerialize, PayloadCtxGuard, default_encode_flags,
    from_bytes_view, inspect_len_from_slice, payload_alignment_padding_for,
};

/// A nominal wire type whose payload is exactly one UTF-8 string field.
///
/// Implementations must serialize precisely the existing `&str` payload and
/// expose their original literal frame identity through `static_frame_name`.
/// The byte bound is a model obligation, independent of ambient decode budgets.
/// This declaration supplies no syntax/NFC validation or allocation custody.
pub trait NominalText: NoritoSerialize {
    /// Maximum UTF-8 bytes in the single text field.
    const MAX_TEXT_BYTES: usize;
}

/// Borrow one text payload using the existing length-prefix decoder.
///
/// `check_length` runs after the original prefix/outer field checks and before
/// accessing the body. Owned and borrowed model decoders share this path; an
/// owned decoder may preserve its model-specific oversize error. Nothing is
/// allocated or charged as retained text by this operation.
///
/// # Errors
/// Returns the original length, caller-bound or UTF-8 error. The consumed length
/// includes the prefix; an enclosing frame must require exact consumption.
pub fn borrow_text_payload(
    bytes: &[u8],
    check_length: impl FnOnce(usize) -> Result<(), Error>,
) -> Result<(&str, usize), Error> {
    let (length, prefix) = inspect_len_from_slice(bytes)?;
    check_length(length)?;
    let end = prefix.checked_add(length).ok_or(Error::LengthMismatch)?;
    let raw = bytes.get(prefix..end).ok_or(Error::LengthMismatch)?;
    let value = std::str::from_utf8(raw).map_err(|_| Error::InvalidUtf8)?;
    super::note_payload_access(bytes, end);
    Ok((value, end))
}

/// Borrow the original text from one exact canonical nominal frame.
///
/// The original type determines schema, padding and text ceiling. The existing
/// canonical writer checks the complete header, flags, prefix and payload by
/// streaming over the input; callers cannot provide an alternate schema or
/// layout. Input need not be aligned. No owned `T` is constructed, and no new
/// decode-budget bookkeeping is created; existing outer field limits still
/// apply. Syntax/NFC validation remains with the model after scratch admission.
///
/// # Errors
/// Returns original framing/field errors or `NonCanonicalEncoding` for an
/// alternate canonical representation. A missing literal nominal declaration
/// is a schema error and is never resolved by allocating a schema name.
pub fn borrow_canonical_text<T: NominalText>(bytes: &[u8]) -> Result<&str, Error> {
    // Restrict this allocation-free operation to its declared literal identity.
    T::static_frame_name().ok_or(Error::SchemaMismatch)?;
    let view = from_bytes_view(bytes)?;
    if view.schema != crate::schema::identity::frame_hash::<T>() {
        return Err(Error::SchemaMismatch);
    }
    if view.padding_len != payload_alignment_padding_for::<T>() {
        return Err(Error::LengthMismatch);
    }
    let (text, used) = {
        let _context =
            PayloadCtxGuard::enter_with_schema_and_flags(view.bytes, view.schema, view.flags);
        borrow_text_payload(view.bytes, |length| {
            if length > T::MAX_TEXT_BYTES {
                return Err(Error::FieldLengthExceeded {
                    length: super::limit_to_u64(length),
                    limit: super::limit_to_u64(T::MAX_TEXT_BYTES),
                });
            }
            Ok(())
        })?
    };
    super::validate_decode_consumption(view.bytes, used)?;
    let mut exact = ExactSliceWriter::new(bytes);
    let result = super::encode_frames::write_typed_payload_frame::<T, _, _>(
        &text,
        &mut exact,
        default_encode_flags(),
    );
    if exact.mismatched() {
        return Err(Error::NonCanonicalEncoding);
    }
    result?;
    if !exact.is_complete() {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
