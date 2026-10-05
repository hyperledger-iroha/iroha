//! Owning String framing and UTF-8 validation shared with prepared destinations.

use super::{Error, read_len_dyn_slice, record_slice_access};

/// One length/body/UTF-8 kernel; archived reads keep their original access order.
pub(super) fn string_payload<'a>(
    bytes: &'a [u8],
    before_utf8: impl FnOnce(&[u8]),
) -> Result<(&'a str, usize), Error> {
    let (length, prefix) = read_len_dyn_slice(bytes)?;
    let end = prefix.checked_add(length).ok_or(Error::LengthMismatch)?;
    let raw = bytes.get(prefix..end).ok_or(Error::LengthMismatch)?;
    before_utf8(raw);
    let value = std::str::from_utf8(raw).map_err(|_| Error::InvalidUtf8)?;
    Ok((value, end))
}

/// Borrow a String payload prefix through its original owning decoder kernel.
///
/// The active advertised flags determine its length prefix. Field length and
/// cumulative logical byte work are charged before body/UTF-8 validation, exactly
/// as for owning String decoding. No String, alignment copy or budget control is
/// allocated. This borrow does not admit physical storage or authenticate bytes.
/// Use [`super::CanonicalField::with_payload`] for the enclosing field's flags
/// and depth, then require exact consumption and full canonical frame comparison.
///
/// # Errors
/// Returns the original length/resource error or [`Error::InvalidUtf8`]. The used
/// length includes the prefix; trailing caller bytes are not consumed.
pub fn borrow_canonical_string(bytes: &[u8]) -> Result<(&str, usize), Error> {
    let (value, used) = string_payload(bytes, |_| {})?;
    record_slice_access(bytes, used);
    Ok((value, used))
}

/// Original String codec failure or a caller-owned destination geometry mismatch.
#[derive(Debug, thiserror::Error)]
pub enum StringDestinationError {
    /// Original canonical framing, UTF-8 or active logical-budget failure.
    #[error(transparent)]
    Codec(#[from] Error),
    /// The initialized destination is smaller than the validated String body.
    /// This is local storage geometry, never intrinsic protocol invalidity.
    #[error("prepared string holds {available} bytes but needs {required}")]
    Storage {
        /// Number of initialized caller-owned destination bytes.
        available: usize,
        /// Original validated UTF-8 byte length, excluding its prefix.
        required: usize,
    },
}

/// Fill initialized caller storage using the sole owning String payload kernel.
///
/// The complete declared body and UTF-8 are checked before any destination byte
/// changes. Logical length accounting matches owning decoding; this does not
/// reserve physical memory. The original destination must already be admitted.
/// Successful output is `destination[..written]`; its suffix stays unchanged.
/// The returned used length includes the original prefix. An enclosing canonical
/// field/record must still check exact consumption and its complete frame.
///
/// # Errors
/// Preserves the original codec error or reports a distinct short destination.
/// Every failure leaves the destination bytes and their owner unchanged.
pub fn decode_string_into(
    bytes: &[u8],
    destination: &mut [u8],
) -> Result<(usize, usize), StringDestinationError> {
    let (value, used) = borrow_canonical_string(bytes)?;
    if value.len() > destination.len() {
        return Err(StringDestinationError::Storage {
            available: destination.len(),
            required: value.len(),
        });
    }
    destination[..value.len()].copy_from_slice(value.as_bytes());
    Ok((value.len(), used))
}
