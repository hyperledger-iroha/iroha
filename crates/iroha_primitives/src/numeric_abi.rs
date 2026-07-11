//! Canonical schema-bound Norito frames for Kotodama V1 numeric values.
//!
//! These frames are consensus-visible. They deliberately use fixed-width
//! `u32` mantissa lengths, minimal little-endian two's-complement bytes, no
//! compression, no layout flags, and no alignment padding.

use std::io::Write;

use norito::{Archived, Error as NoritoError, NoritoDeserialize, NoritoSerialize};

use crate::{
    bigint::{BigInt, BigIntError},
    numeric::{MAX_DECIMAL_SCALE, MAX_MANTISSA_BYTES, Numeric, NumericOperationError, Quantity},
};

/// Nominal schema name of a V1 integer frame.
pub const INT_SCHEMA_NAME_V1: &str = "iroha.numeric.IntValueV1";
/// Nominal schema name of a V1 decimal frame.
pub const DECIMAL_SCHEMA_NAME_V1: &str = "iroha.numeric.DecimalValueV1";
/// Nominal schema name of a V1 quantity frame.
pub const QUANTITY_SCHEMA_NAME_V1: &str = "iroha.numeric.QuantityValueV1";

/// Type-name schema hash of [`INT_SCHEMA_NAME_V1`].
pub const INT_SCHEMA_HASH_V1: [u8; 16] = [
    0x07, 0xc0, 0x39, 0x45, 0x73, 0x63, 0xb9, 0xe1, 0xd3, 0x6b, 0xbd, 0x31, 0xd9, 0x3d, 0xec, 0x4a,
];
/// Type-name schema hash of [`DECIMAL_SCHEMA_NAME_V1`].
pub const DECIMAL_SCHEMA_HASH_V1: [u8; 16] = [
    0xba, 0x2f, 0xfe, 0xd5, 0x2e, 0x4d, 0x8e, 0xe1, 0x6f, 0x17, 0xef, 0xef, 0xe1, 0x82, 0x85, 0x24,
];
/// Type-name schema hash of [`QUANTITY_SCHEMA_NAME_V1`].
pub const QUANTITY_SCHEMA_HASH_V1: [u8; 16] = [
    0xe4, 0x76, 0x99, 0x84, 0xc8, 0x1c, 0xe0, 0xe8, 0xb6, 0x78, 0xf2, 0xeb, 0x06, 0x27, 0x4e, 0xe3,
];

/// Norito header length used by all V1 numeric frames.
pub const NUMERIC_FRAME_HEADER_BYTES_V1: usize = 40;
/// Maximum canonical integer frame length.
pub const MAX_INT_FRAME_BYTES_V1: usize = NUMERIC_FRAME_HEADER_BYTES_V1 + 4 + MAX_MANTISSA_BYTES;
/// Maximum canonical decimal frame length.
pub const MAX_DECIMAL_FRAME_BYTES_V1: usize = MAX_INT_FRAME_BYTES_V1 + 1;
/// Maximum canonical quantity frame length.
pub const MAX_QUANTITY_FRAME_BYTES_V1: usize = MAX_DECIMAL_FRAME_BYTES_V1;

/// Pointer-ABI TLV overhead outside the schema-bound frame.
pub const NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1: usize = 39;
/// Maximum integer pointer-envelope length.
pub const MAX_INT_ENVELOPE_BYTES_V1: usize =
    MAX_INT_FRAME_BYTES_V1 + NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1;
/// Maximum decimal pointer-envelope length.
pub const MAX_DECIMAL_ENVELOPE_BYTES_V1: usize =
    MAX_DECIMAL_FRAME_BYTES_V1 + NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1;
/// Maximum quantity pointer-envelope length.
pub const MAX_QUANTITY_ENVELOPE_BYTES_V1: usize =
    MAX_QUANTITY_FRAME_BYTES_V1 + NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1;

/// Failure while validating or decoding a canonical V1 numeric frame.
#[derive(Debug, Clone, displaydoc::Display, thiserror::Error)]
pub enum NumericAbiError {
    /// Frame is shorter than the fixed Norito header
    FrameTooShort,
    /// Frame exceeds the maximum size for its declared numeric type
    FrameTooLarge,
    /// Frame does not use the canonical Norito V1 header
    InvalidHeader,
    /// Frame schema hash does not match the nominal numeric type
    SchemaMismatch,
    /// Numeric frames must be uncompressed
    CompressionNotAllowed,
    /// Numeric frames must use layout flags zero
    LayoutFlagsNotAllowed,
    /// Declared payload length is invalid or does not consume the frame exactly
    LengthMismatch,
    /// Mantissa encoding is outside the signed domain
    MantissaOverflow,
    /// Mantissa encoding is not minimal
    NonCanonicalMantissa,
    /// Decimal scale is outside `0..=28`
    InvalidScale,
    /// Decimal representation contains removable trailing zeroes
    NonCanonicalDecimal,
    /// Quantity mantissa is negative
    NegativeQuantity,
    /// Norito frame validation failed: {_0}
    Norito(String),
}

impl PartialEq for NumericAbiError {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
            && match (self, other) {
                (Self::Norito(lhs), Self::Norito(rhs)) => lhs == rhs,
                _ => true,
            }
    }
}

impl Eq for NumericAbiError {}

/// Canonical V1 integer frame value.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntValueV1(BigInt);

impl IntValueV1 {
    /// Validate and wrap a signed-domain integer.
    ///
    /// # Errors
    /// Rejects values outside the signed 512-bit V1 domain.
    pub fn try_new(value: BigInt) -> Result<Self, NumericAbiError> {
        if value.twos_byte_len() > MAX_MANTISSA_BYTES {
            return Err(NumericAbiError::MantissaOverflow);
        }
        Ok(Self(value))
    }

    /// Borrow the integer.
    #[must_use]
    pub fn as_int(&self) -> &BigInt {
        &self.0
    }

    /// Consume the wrapper and return the integer.
    #[must_use]
    pub fn into_int(self) -> BigInt {
        self.0
    }

    /// Encode a canonical, uncompressed, schema-bound Norito frame.
    ///
    /// # Errors
    /// Returns a Norito framing error.
    pub fn encode_frame(&self) -> Result<Vec<u8>, NumericAbiError> {
        encode_frame::<Self>(&encode_int_body(&self.0))
    }

    /// Strictly decode a canonical schema-bound integer frame.
    ///
    /// # Errors
    /// Rejects malformed headers, checksum errors, wrong schemas, nonzero
    /// flags, noncanonical signed bytes, trailing bytes, and oversized values.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, NumericAbiError> {
        validate_frame(frame, INT_SCHEMA_HASH_V1, MAX_INT_FRAME_BYTES_V1)?;
        let (value, used) = decode_int_body(&frame[NUMERIC_FRAME_HEADER_BYTES_V1..])?;
        if used != frame.len() - NUMERIC_FRAME_HEADER_BYTES_V1 {
            return Err(NumericAbiError::LengthMismatch);
        }
        Ok(Self(value))
    }
}

/// Canonical V1 exact-decimal frame value.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DecimalValueV1(Numeric);

impl DecimalValueV1 {
    /// Canonicalize a decimal for the V1 wire domain.
    ///
    /// # Errors
    /// Returns a canonical result-domain failure.
    pub fn try_from_numeric(value: Numeric) -> Result<Self, NumericOperationError> {
        Ok(Self(value.canonicalize_decimal()?))
    }

    /// Wrap an already canonical decimal.
    ///
    /// # Errors
    /// Rejects noncanonical input.
    pub fn from_canonical_numeric(value: Numeric) -> Result<Self, NumericOperationError> {
        value.validate_decimal()?;
        Ok(Self(value))
    }

    /// Borrow the canonical decimal.
    #[must_use]
    pub fn as_numeric(&self) -> &Numeric {
        &self.0
    }

    /// Consume the wrapper and return the canonical decimal.
    #[must_use]
    pub fn into_numeric(self) -> Numeric {
        self.0
    }

    /// Encode a canonical, uncompressed, schema-bound Norito frame.
    ///
    /// # Errors
    /// Returns a Norito framing error.
    pub fn encode_frame(&self) -> Result<Vec<u8>, NumericAbiError> {
        encode_frame::<Self>(&encode_scaled_body(&self.0))
    }

    /// Strictly decode a canonical schema-bound decimal frame.
    ///
    /// # Errors
    /// Rejects every noncanonical or malformed representation.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, NumericAbiError> {
        validate_frame(frame, DECIMAL_SCHEMA_HASH_V1, MAX_DECIMAL_FRAME_BYTES_V1)?;
        let (value, used) = decode_scaled_body(&frame[NUMERIC_FRAME_HEADER_BYTES_V1..])?;
        if used != frame.len() - NUMERIC_FRAME_HEADER_BYTES_V1 {
            return Err(NumericAbiError::LengthMismatch);
        }
        Ok(Self(value))
    }
}

/// Canonical V1 non-negative quantity frame value.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QuantityValueV1(Quantity);

impl QuantityValueV1 {
    /// Wrap a nominal canonical quantity.
    #[must_use]
    pub fn new(value: Quantity) -> Self {
        Self(value)
    }

    /// Borrow the quantity.
    #[must_use]
    pub fn as_quantity(&self) -> &Quantity {
        &self.0
    }

    /// Consume the wrapper and return the quantity.
    #[must_use]
    pub fn into_quantity(self) -> Quantity {
        self.0
    }

    /// Encode a canonical, uncompressed, schema-bound Norito frame.
    ///
    /// # Errors
    /// Returns a Norito framing error.
    pub fn encode_frame(&self) -> Result<Vec<u8>, NumericAbiError> {
        encode_frame::<Self>(&encode_scaled_body(self.0.as_numeric()))
    }

    /// Strictly decode a canonical schema-bound quantity frame.
    ///
    /// # Errors
    /// Rejects every malformed, noncanonical, or negative representation.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, NumericAbiError> {
        validate_frame(frame, QUANTITY_SCHEMA_HASH_V1, MAX_QUANTITY_FRAME_BYTES_V1)?;
        let (value, used) = decode_scaled_body(&frame[NUMERIC_FRAME_HEADER_BYTES_V1..])?;
        if used != frame.len() - NUMERIC_FRAME_HEADER_BYTES_V1 {
            return Err(NumericAbiError::LengthMismatch);
        }
        let quantity = Quantity::from_canonical_numeric(value).map_err(|error| match error {
            NumericOperationError::NegativeQuantity => NumericAbiError::NegativeQuantity,
            NumericOperationError::NonCanonical => NumericAbiError::NonCanonicalDecimal,
            _ => NumericAbiError::Norito(error.to_string()),
        })?;
        Ok(Self(quantity))
    }
}

fn encode_int_body(value: &BigInt) -> Vec<u8> {
    let bytes = value.to_twos_bytes();
    let mut body = Vec::with_capacity(4 + bytes.len());
    body.extend_from_slice(
        &u32::try_from(bytes.len())
            .expect("bounded mantissa length fits u32")
            .to_le_bytes(),
    );
    body.extend_from_slice(&bytes);
    body
}

fn encode_scaled_body(value: &Numeric) -> Vec<u8> {
    let mut body = encode_int_body(value.mantissa());
    body.push(u8::try_from(value.scale()).expect("validated decimal scale fits u8"));
    body
}

fn encode_frame<T: NoritoSerialize>(body: &[u8]) -> Result<Vec<u8>, NumericAbiError> {
    norito::core::frame_bare_with_header_flags::<T>(body, 0)
        .map_err(|error| NumericAbiError::Norito(error.to_string()))
}

fn validate_frame_header(
    frame: &[u8],
    schema: [u8; 16],
    maximum: usize,
) -> Result<(), NumericAbiError> {
    if frame.len() < NUMERIC_FRAME_HEADER_BYTES_V1 {
        return Err(NumericAbiError::FrameTooShort);
    }
    if frame.len() > maximum {
        return Err(NumericAbiError::FrameTooLarge);
    }
    if frame[..4] != norito::core::MAGIC
        || frame[4] != norito::core::VERSION_MAJOR
        || frame[5] != norito::core::VERSION_MINOR
    {
        return Err(NumericAbiError::InvalidHeader);
    }
    if frame[6..22] != schema {
        return Err(NumericAbiError::SchemaMismatch);
    }
    if frame[22] != 0 {
        return Err(NumericAbiError::CompressionNotAllowed);
    }
    if frame[39] != 0 {
        return Err(NumericAbiError::LayoutFlagsNotAllowed);
    }
    let length = u64::from_le_bytes(
        frame[23..31]
            .try_into()
            .expect("fixed header length was checked"),
    );
    let length = usize::try_from(length).map_err(|_| NumericAbiError::LengthMismatch)?;
    if NUMERIC_FRAME_HEADER_BYTES_V1.checked_add(length) != Some(frame.len()) {
        return Err(NumericAbiError::LengthMismatch);
    }
    Ok(())
}

fn validate_frame(frame: &[u8], schema: [u8; 16], maximum: usize) -> Result<(), NumericAbiError> {
    validate_frame_header(frame, schema, maximum)?;
    norito::core::from_bytes_view(frame)
        .map_err(|error| NumericAbiError::Norito(error.to_string()))?;
    Ok(())
}

fn decode_int_body(bytes: &[u8]) -> Result<(BigInt, usize), NumericAbiError> {
    if bytes.len() < 4 {
        return Err(NumericAbiError::LengthMismatch);
    }
    let length = usize::try_from(u32::from_le_bytes(
        bytes[..4].try_into().expect("length prefix was checked"),
    ))
    .map_err(|_| NumericAbiError::LengthMismatch)?;
    if length > MAX_MANTISSA_BYTES {
        return Err(NumericAbiError::MantissaOverflow);
    }
    let end = 4_usize
        .checked_add(length)
        .ok_or(NumericAbiError::LengthMismatch)?;
    if end > bytes.len() {
        return Err(NumericAbiError::LengthMismatch);
    }
    let payload = &bytes[4..end];
    let value = BigInt::from_twos_bytes(payload).map_err(|error| match error {
        BigIntError::Overflow => NumericAbiError::MantissaOverflow,
        BigIntError::NonCanonical => NumericAbiError::NonCanonicalMantissa,
        BigIntError::DivisionByZero => unreachable!("decoding does not divide"),
    })?;
    if value.to_twos_bytes() != payload {
        return Err(NumericAbiError::NonCanonicalMantissa);
    }
    Ok((value, end))
}

fn decode_scaled_body(bytes: &[u8]) -> Result<(Numeric, usize), NumericAbiError> {
    let (mantissa, used) = decode_int_body(bytes)?;
    let end = used.checked_add(1).ok_or(NumericAbiError::LengthMismatch)?;
    if end > bytes.len() {
        return Err(NumericAbiError::LengthMismatch);
    }
    let scale = u32::from(bytes[used]);
    if scale > MAX_DECIMAL_SCALE {
        return Err(NumericAbiError::InvalidScale);
    }
    let value = Numeric::try_new_raw(mantissa, scale).map_err(|error| match error {
        crate::numeric::NumericError::MantissaTooLarge => NumericAbiError::MantissaOverflow,
        crate::numeric::NumericError::ScaleTooLarge => NumericAbiError::InvalidScale,
        crate::numeric::NumericError::Malformed => unreachable!("decoded fields are structured"),
    })?;
    value
        .validate_decimal()
        .map_err(|_| NumericAbiError::NonCanonicalDecimal)?;
    Ok((value, end))
}

macro_rules! impl_frame_codec {
    ($ty:ty, $schema:expr, $encode:expr, $decode:expr) => {
        impl NoritoSerialize for $ty {
            fn schema_hash() -> [u8; 16] {
                $schema
            }

            fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
                writer
                    .write_all(&$encode(self))
                    .map_err(|error| NoritoError::Message(error.to_string()))
            }

            fn encoded_len_exact(&self) -> Option<usize> {
                Some($encode(self).len())
            }
        }

        impl<'a> NoritoDeserialize<'a> for $ty {
            fn schema_hash() -> [u8; 16] {
                $schema
            }

            fn deserialize(archived: &'a Archived<Self>) -> Self {
                Self::try_deserialize(archived).expect("invalid canonical numeric frame")
            }

            fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
                let bytes =
                    norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast())?;
                let (value, used) = $decode(bytes)
                    .map_err(|error: NumericAbiError| NoritoError::Message(error.to_string()))?;
                if used != bytes.len() {
                    return Err(NoritoError::Message(
                        NumericAbiError::LengthMismatch.to_string(),
                    ));
                }
                Ok(value)
            }
        }

        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                $decode(bytes).map_err(|error: NumericAbiError| {
                    norito::core::Error::Message(error.to_string())
                })
            }
        }
    };
}

impl_frame_codec!(
    IntValueV1,
    INT_SCHEMA_HASH_V1,
    |value: &IntValueV1| encode_int_body(&value.0),
    |bytes: &[u8]| decode_int_body(bytes).map(|(value, used)| (IntValueV1(value), used))
);

impl_frame_codec!(
    DecimalValueV1,
    DECIMAL_SCHEMA_HASH_V1,
    |value: &DecimalValueV1| encode_scaled_body(&value.0),
    |bytes: &[u8]| decode_scaled_body(bytes).map(|(value, used)| (DecimalValueV1(value), used))
);

impl_frame_codec!(
    QuantityValueV1,
    QUANTITY_SCHEMA_HASH_V1,
    |value: &QuantityValueV1| encode_scaled_body(value.0.as_numeric()),
    |bytes: &[u8]| {
        let (value, used) = decode_scaled_body(bytes)?;
        let quantity = Quantity::from_canonical_numeric(value).map_err(|error| match error {
            NumericOperationError::NegativeQuantity => NumericAbiError::NegativeQuantity,
            NumericOperationError::NonCanonical => NumericAbiError::NonCanonicalDecimal,
            _ => NumericAbiError::Norito(error.to_string()),
        })?;
        Ok((QuantityValueV1(quantity), used))
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_hashes_match_normative_names() {
        assert_eq!(
            norito::core::schema_hash_for_name(INT_SCHEMA_NAME_V1),
            INT_SCHEMA_HASH_V1
        );
        assert_eq!(
            norito::core::schema_hash_for_name(DECIMAL_SCHEMA_NAME_V1),
            DECIMAL_SCHEMA_HASH_V1
        );
        assert_eq!(
            norito::core::schema_hash_for_name(QUANTITY_SCHEMA_NAME_V1),
            QUANTITY_SCHEMA_HASH_V1
        );
    }

    #[test]
    fn canonical_frames_roundtrip_and_have_exact_small_sizes() {
        let integer = IntValueV1::try_new(BigInt::from_i128(-129)).expect("bounded integer");
        let integer_frame = integer.encode_frame().expect("encode integer");
        assert_eq!(integer_frame.len(), NUMERIC_FRAME_HEADER_BYTES_V1 + 6);
        assert_eq!(IntValueV1::decode_frame(&integer_frame), Ok(integer));

        let decimal = DecimalValueV1::try_from_numeric(Numeric::new(-12_500, 3))
            .expect("canonicalize decimal");
        assert_eq!(decimal.as_numeric(), &Numeric::new(-125, 1));
        let decimal_frame = decimal.encode_frame().expect("encode decimal");
        assert_eq!(DecimalValueV1::decode_frame(&decimal_frame), Ok(decimal));

        let quantity = QuantityValueV1::new("12.50".parse().expect("quantity"));
        let quantity_frame = quantity.encode_frame().expect("encode quantity");
        assert_eq!(QuantityValueV1::decode_frame(&quantity_frame), Ok(quantity));
    }

    #[test]
    fn signed_endpoint_frames_hit_pinned_maximum() {
        let endpoint_bytes = [
            vec![0xff_u8; MAX_MANTISSA_BYTES - 1]
                .into_iter()
                .chain([0x7f])
                .collect::<Vec<_>>(),
            vec![0_u8; MAX_MANTISSA_BYTES - 1]
                .into_iter()
                .chain([0x80])
                .collect::<Vec<_>>(),
        ];
        for bytes in endpoint_bytes {
            let value = IntValueV1::try_new(BigInt::from_twos_bytes(&bytes).expect("endpoint"))
                .expect("512-bit endpoint");
            let frame = value.encode_frame().expect("frame");
            assert_eq!(frame.len(), MAX_INT_FRAME_BYTES_V1);
            assert_eq!(IntValueV1::decode_frame(&frame), Ok(value));
        }
        assert_eq!(MAX_INT_ENVELOPE_BYTES_V1, 147);
        assert_eq!(MAX_DECIMAL_ENVELOPE_BYTES_V1, 148);
        assert_eq!(MAX_QUANTITY_ENVELOPE_BYTES_V1, 148);
    }

    #[test]
    fn integer_wrapper_rejects_both_signed_domain_neighbors() {
        let mut maximum_bytes = vec![0xff_u8; MAX_MANTISSA_BYTES - 1];
        maximum_bytes.push(0x7f);
        let maximum = BigInt::from_twos_bytes(&maximum_bytes).expect("maximum");
        let above_maximum = maximum
            .checked_add(&BigInt::one())
            .expect("generic bigint can represent the upper neighbor");

        let mut minimum_bytes = vec![0_u8; MAX_MANTISSA_BYTES - 1];
        minimum_bytes.push(0x80);
        let minimum = BigInt::from_twos_bytes(&minimum_bytes).expect("minimum");
        let below_minimum = minimum
            .checked_sub(&BigInt::one())
            .expect("generic bigint can represent the lower neighbor");

        assert_eq!(
            IntValueV1::try_new(above_maximum),
            Err(NumericAbiError::MantissaOverflow)
        );
        assert_eq!(
            IntValueV1::try_new(below_minimum),
            Err(NumericAbiError::MantissaOverflow)
        );
    }

    #[test]
    fn cross_type_and_header_mutations_are_rejected_before_payload_decode() {
        let frame = IntValueV1::try_new(BigInt::one())
            .expect("bounded integer")
            .encode_frame()
            .expect("integer frame");
        assert!(matches!(
            DecimalValueV1::decode_frame(&frame),
            Err(NumericAbiError::SchemaMismatch)
        ));

        for (index, expected) in [
            (0, NumericAbiError::InvalidHeader),
            (4, NumericAbiError::InvalidHeader),
            (5, NumericAbiError::InvalidHeader),
            (6, NumericAbiError::SchemaMismatch),
            (22, NumericAbiError::CompressionNotAllowed),
            (39, NumericAbiError::LayoutFlagsNotAllowed),
        ] {
            let mut mutated = frame.clone();
            mutated[index] ^= 1;
            assert_eq!(
                IntValueV1::decode_frame(&mutated).expect_err("mutation must fail"),
                expected,
                "header byte {index}"
            );
        }
    }

    #[test]
    fn truncation_extension_and_declared_length_attacks_are_rejected() {
        let frame = IntValueV1::try_new(BigInt::from_i128(128))
            .expect("bounded integer")
            .encode_frame()
            .expect("frame");
        for end in 0..frame.len() {
            assert!(
                IntValueV1::decode_frame(&frame[..end]).is_err(),
                "end={end}"
            );
        }
        let mut extended = frame.clone();
        extended.push(0);
        assert_eq!(
            IntValueV1::decode_frame(&extended),
            Err(NumericAbiError::LengthMismatch)
        );

        for declared in [0_u64, 1, u64::MAX] {
            let mut malformed = frame.clone();
            malformed[23..31].copy_from_slice(&declared.to_le_bytes());
            assert_eq!(
                IntValueV1::decode_frame(&malformed),
                Err(NumericAbiError::LengthMismatch)
            );
        }
    }

    #[test]
    fn bare_body_rejects_every_redundant_sign_extension_and_scaled_noncanonical_form() {
        for payload in [&[0_u8][..], &[1, 0], &[0xff, 0xff]] {
            let mut body = Vec::new();
            body.extend_from_slice(&u32::try_from(payload.len()).expect("small").to_le_bytes());
            body.extend_from_slice(payload);
            assert_eq!(
                decode_int_body(&body),
                Err(NumericAbiError::NonCanonicalMantissa)
            );
            let frame = encode_frame::<IntValueV1>(&body)
                .expect("frame helper recomputes checksum for malformed body");
            assert_eq!(
                IntValueV1::decode_frame(&frame),
                Err(NumericAbiError::NonCanonicalMantissa)
            );
        }

        for (mantissa, scale, expected) in [
            (0_i128, 1_u8, NumericAbiError::NonCanonicalDecimal),
            (10, 1, NumericAbiError::NonCanonicalDecimal),
            (1, 29, NumericAbiError::InvalidScale),
        ] {
            let mut body = encode_int_body(&BigInt::from_i128(mantissa));
            body.push(scale);
            assert_eq!(decode_scaled_body(&body), Err(expected.clone()));
            let frame = encode_frame::<DecimalValueV1>(&body)
                .expect("frame helper recomputes checksum for malformed body");
            assert_eq!(DecimalValueV1::decode_frame(&frame), Err(expected));
        }
    }

    #[test]
    fn quantity_body_rejects_negative_value_even_when_otherwise_canonical() {
        let mut body = encode_int_body(&BigInt::from_i128(-1));
        body.push(0);
        let frame = encode_frame::<QuantityValueV1>(&body).expect("well-formed frame");
        assert_eq!(
            QuantityValueV1::decode_frame(&frame),
            Err(NumericAbiError::NegativeQuantity)
        );
    }

    #[test]
    fn checksum_tampering_is_rejected() {
        let mut frame = IntValueV1::try_new(BigInt::from_i128(42))
            .expect("bounded integer")
            .encode_frame()
            .expect("frame");
        let final_index = frame.len() - 1;
        frame[final_index] ^= 0x80;
        assert!(matches!(
            IntValueV1::decode_frame(&frame),
            Err(NumericAbiError::Norito(_))
        ));
    }
}
