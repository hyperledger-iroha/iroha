//! Derived canonical field traversal shared by owned and prepared destinations.
//!
//! A destination controls custody only. The generated walk owns field order,
//! prefixes and complete consumption; complete framed canonical authentication
//! still requires the canonical frame verifier after the destination is filled.

use super::*;

/// Exact codec failure or a caller-owned destination refusal during one field walk.
#[derive(Debug)]
pub enum DecodeIntoError<E> {
    /// The unchanged canonical parser rejected or locally refused the original bytes.
    Codec(Error),
    /// The original prepared destination refused; no protocol classification is implied.
    Destination(E),
}
impl<E> From<Error> for DecodeIntoError<E> {
    fn from(error: Error) -> Self {
        Self::Codec(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for DecodeIntoError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(error) => error.fmt(f),
            Self::Destination(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DecodeIntoError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Destination(error) => Some(error),
        }
    }
}
impl DecodeIntoError<std::convert::Infallible> {
    /// Recover the sole original codec cause from an ordinary owned field walk.
    pub fn into_codec(self) -> Error {
        match self {
            Self::Codec(error) => error,
            Self::Destination(never) => match never {},
        }
    }
}

/// Destination-wide original refusal type shared by every field in one record.
pub trait FieldDestination {
    /// Local custody failure, kept distinct from canonical byte errors.
    type Error;
}
/// Receive one field selected by the record's sole derived positional traversal.
pub trait DecodeField<const INDEX: usize, T>: FieldDestination {
    /// Result retained by the generated walk; prepared writers normally return `()`.
    type Value;
    /// Decode into existing destination storage or return its original refusal.
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, T>,
    ) -> Result<Self::Value, DecodeIntoError<Self::Error>>;
}
/// Generated positional record kernel shared by ordinary and prepared decoding.
pub trait DecodeRecordFields<D: FieldDestination> {
    /// Field results; prepared destinations can make this a tuple of zero-sized values.
    type Values;
    /// Walk the original canonical field order with the currently advertised layout.
    ///
    /// Field and record bounds are checked by the sole shared framing kernel.
    /// The caller must install the original advertised frame flags and decode scope.
    fn decode_fields(
        bytes: &[u8],
        destination: &mut D,
    ) -> Result<(Self::Values, usize), DecodeIntoError<D::Error>>;
}

/// Ordinary custody implementation used by the same generated record traversal.
pub struct OwnedFields;
impl FieldDestination for OwnedFields {
    type Error = std::convert::Infallible;
}
impl<const INDEX: usize, T> DecodeField<INDEX, T> for OwnedFields {
    type Value = T;
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, T>,
    ) -> Result<T, DecodeIntoError<Self::Error>> {
        field.decode_owned().map_err(DecodeIntoError::Codec)
    }
}

/// A borrowed canonical field frame with the record's declared field type.
///
/// Construction is restricted to the shared framing functions. This does not
/// certify semantic validity: a prepared destination must use that type's
/// canonical decoder and the final filled record must pass exact frame checking.
/// It cannot outlive the actual input slice or expose an archived raw pointer.
pub struct CanonicalField<'a, T> {
    bytes: &'a [u8],
    decode: fn(&[u8]) -> Result<T, Error>,
    byte_array: bool,
}
impl<T> CanonicalField<'_, T> {
    /// Borrow the exact field payload after its canonical prefix was checked.
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }
    /// Decode with the same owning leaf used before destination support.
    pub fn decode_owned(self) -> Result<T, Error> {
        (self.decode)(self.bytes)
    }
    /// Run a prepared type decoder inside the same logical field/depth context.
    ///
    /// Unlike owned decoding this does not create an alignment copy or call an
    /// untrusted archived decoder. The supplied operation must consume/check the
    /// complete field and use its prepared storage. Fixed byte arrays keep the
    /// existing raw-field behavior and do not acquire an extra depth or budget.
    pub fn with_payload<R, E>(
        self,
        decode: impl FnOnce(&[u8]) -> Result<R, DecodeIntoError<E>>,
    ) -> Result<R, DecodeIntoError<E>> {
        if self.byte_array {
            return decode(self.bytes);
        }
        check_decode_field_length(
            u64::try_from(self.bytes.len()).map_err(|_| Error::LengthMismatch)?,
        )?;
        let _depth = DecodeDepthGuard::enter()?;
        let _flags = enter_field_codec_flags();
        let _context = PayloadCtxGuard::enter(self.bytes);
        let _boundary = FieldDecodeBoundaryGuard::enter(FieldDecodeBoundary::Canonical);
        decode(self.bytes)
    }
}

/// Borrow the active original record slice; used only by generated owned decoding.
#[doc(hidden)]
pub fn with_context_fields<R>(
    ptr: *const u8,
    body: impl for<'a> FnOnce(&'a [u8]) -> R,
) -> Result<R, Error> {
    Ok(body(payload_slice_from_ptr(ptr)?))
}
/// Read a normal positional field using the sole canonical prefix kernel.
#[doc(hidden)]
pub fn framed_field<'a, T>(
    payload: &'a [u8],
    offset: &mut usize,
) -> Result<CanonicalField<'a, T>, Error>
where
    T: for<'de> crate::DeserializePayload<'de> + crate::SerializePayload,
{
    let (bytes, next) = take_length_prefixed_field(payload, *offset)?;
    *offset = next;
    Ok(canonical_field_from_slice(bytes))
}
/// Keep the owning leaf relation shared by normal fields and sequence elements.
pub(super) fn canonical_field_from_slice<T>(bytes: &[u8]) -> CanonicalField<'_, T>
where
    T: for<'de> crate::DeserializePayload<'de> + crate::SerializePayload,
{
    CanonicalField {
        bytes,
        decode: |bytes| {
            let (value, used) = decode_field_canonical::<T>(bytes)?;
            if used != bytes.len() {
                return Err(Error::LengthMismatch);
            }
            Ok(value)
        },
        byte_array: false,
    }
}
/// Read a fixed byte-array field using its exact existing raw payload geometry.
#[doc(hidden)]
pub fn framed_byte_array_field<'a, const N: usize>(
    payload: &'a [u8],
    offset: &mut usize,
) -> Result<CanonicalField<'a, [u8; N]>, Error> {
    let (bytes, next) = take_length_prefixed_field(payload, *offset)?;
    if bytes.len() != N {
        return Err(Error::LengthMismatch);
    }
    *offset = next;
    Ok(CanonicalField {
        bytes,
        decode: |bytes| bytes.try_into().map_err(|_| Error::LengthMismatch),
        byte_array: true,
    })
}

/// A prepared record whose canonical payload can be inspected without extraction.
///
/// Implementations must retain all original initialized backing when resetting.
/// Serialization must borrow exactly the filled fields in their canonical order;
/// the declared wire type supplies the identity and alignment, never this owner.
pub trait PreparedRecordDestination<T>: FieldDestination + SerializePayload {
    /// Clear validity/initialized-prefix metadata without replacing, dropping or
    /// refunding any prepared backing. Called before every attempt and on failure.
    fn reset(&mut self);
}
