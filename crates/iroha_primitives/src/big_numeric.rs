//! Signed decimal with variable-length mantissa (up to 512 bits) and an explicit scale.
//!
//! This is an incremental bridge type toward replacing the previous fixed-precision
//! `Numeric`. It supports negative values and a configurable decimal scale, while
//! storing the mantissa in [`crate::bigint::BigInt`].

use std::io::Write;

use norito::{
    Archived, Error as NoritoError, NoritoDeserialize, NoritoSerialize, codec,
    json::{self, FastJsonWrite, JsonDeserialize},
};

use crate::bigint::{BigInt, BigIntError, MAX_BITS as BIGINT_MAX_BITS};

/// Error raised by [`BigNumeric`].
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error, PartialEq, Eq)]
pub enum BigNumericError {
    /// Scale exceeds 28 decimal places
    ScaleTooLarge,
    /// Mantissa exceeds 512-bit cap
    MantissaTooLarge,
}

/// Signed decimal with a bounded, variable-width mantissa.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BigNumeric {
    mantissa: BigInt,
    scale: u32,
}

impl BigNumeric {
    /// Construct a new value.
    ///
    /// # Errors
    /// Returns [`BigNumericError::ScaleTooLarge`] when `scale` exceeds 28 and
    /// [`BigNumericError::MantissaTooLarge`] when the mantissa exceeds the
    /// supported bit width.
    pub fn new<T: Into<BigInt>>(mantissa: T, scale: u32) -> Result<Self, BigNumericError> {
        if scale > 28 {
            return Err(BigNumericError::ScaleTooLarge);
        }
        let mantissa = mantissa.into();
        if mantissa.bit_len() > BIGINT_MAX_BITS {
            return Err(BigNumericError::MantissaTooLarge);
        }
        Ok(Self { mantissa, scale })
    }

    /// Return the mantissa.
    pub fn mantissa(&self) -> &BigInt {
        &self.mantissa
    }

    /// Return the scale.
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Checked addition.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if scaling either operand causes the
    /// mantissa to exceed the configured bit cap or if the sum would overflow.
    pub fn checked_add(&self, other: &Self) -> Result<Self, BigIntError> {
        let target_scale = self.scale.max(other.scale);
        let lhs = Self::scale_up(&self.mantissa, target_scale - self.scale)?;
        let rhs = Self::scale_up(&other.mantissa, target_scale - other.scale)?;
        let sum = lhs.checked_add(&rhs)?;
        Self::new(sum, target_scale).map_err(|_| BigIntError::Overflow)
    }

    fn scale_up(m: &BigInt, delta: u32) -> Result<BigInt, BigIntError> {
        if delta == 0 {
            return Ok(m.clone());
        }
        let factor = 10i128.checked_pow(delta).ok_or(BigIntError::Overflow)?;
        m.checked_mul(&BigInt::from(factor))
    }
}

impl NoritoSerialize for BigNumeric {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        let encoded_m = codec::Encode::encode(&self.mantissa);
        let encoded_s = codec::Encode::encode(&self.scale);
        writer
            .write_all(&encoded_m)
            .map_err(|e| NoritoError::Message(e.to_string()))?;
        writer
            .write_all(&encoded_s)
            .map_err(|e| NoritoError::Message(e.to_string()))
    }
}

impl<'a> NoritoDeserialize<'a> for BigNumeric {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let slice =
            norito::core::payload_slice_from_ptr(ptr).expect("payload slice for bignumeric");
        let (value, _) = <BigNumeric as norito::core::DecodeFromSlice>::decode_from_slice(slice)
            .expect("decode");
        value
    }
}

impl norito::core::DecodeFromSlice<'_> for BigNumeric {
    fn decode_from_slice(bytes: &[u8]) -> Result<(Self, usize), norito::core::Error> {
        let (mantissa, used_m) =
            <BigInt as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        let (scale, used_s) =
            <u32 as norito::core::DecodeFromSlice>::decode_from_slice(&bytes[used_m..])?;
        let total = used_m + used_s;
        let numeric = BigNumeric::new(mantissa, scale)
            .map_err(|e| norito::core::Error::Message(e.to_string()))?;
        Ok((numeric, total))
    }
}

impl FastJsonWrite for BigNumeric {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for BigNumeric {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<BigNumeric>()
            .map_err(|err| json::Error::InvalidField {
                field: "bignumeric".into(),
                message: format!("invalid bignumeric `{value}`: {err}"),
            })
    }
}

impl core::fmt::Display for BigNumeric {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.mantissa);
        }
        let mut s = self.mantissa.abs().to_string();
        while s.len() <= self.scale as usize {
            s.insert(0, '0');
        }
        let (int_part, frac_part) = s.split_at(s.len() - self.scale as usize);
        if self.mantissa.is_negative() {
            write!(f, "-{int_part}.{frac_part}")
        } else {
            write!(f, "{int_part}.{frac_part}")
        }
    }
}

impl core::str::FromStr for BigNumeric {
    type Err = BigNumericError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let negative = trimmed.starts_with('-');
        let digits = trimmed.trim_start_matches(['+', '-']);
        let mut scale = 0u32;
        let mut mantissa_str = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if ch == '.' {
                scale = u32::try_from(digits.len() - i - 1)
                    .map_err(|_| BigNumericError::ScaleTooLarge)?;
            } else if ch.is_ascii_digit() {
                mantissa_str.push(ch);
            } else {
                return Err(BigNumericError::MantissaTooLarge);
            }
        }
        let mut mantissa = BigInt::from(
            mantissa_str
                .parse::<i128>()
                .map_err(|_| BigNumericError::MantissaTooLarge)?,
        );
        if negative {
            mantissa = mantissa.neg();
        }
        BigNumeric::new(mantissa, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        let v: BigNumeric = "-123.4500".parse().expect("parse");
        assert_eq!(v.to_string(), "-123.4500");
    }

    #[test]
    fn norito_roundtrip() {
        let v: BigNumeric = "42.01".parse().expect("parse");
        let bytes = norito::codec::Encode::encode(&v);
        let mut slice = bytes.as_slice();
        let decoded = <BigNumeric as norito::codec::Decode>::decode(&mut slice).expect("decode");
        assert_eq!(v, decoded);
    }

    #[test]
    fn new_rejects_scale_above_limit() {
        let err = BigNumeric::new(BigInt::from_i128(1), 29)
            .expect_err("scale above 28 should be rejected");

        assert_eq!(err, BigNumericError::ScaleTooLarge);
    }

    #[test]
    fn accessors_expose_mantissa_and_scale() {
        let value = BigNumeric::new(BigInt::from_i128(-12345), 3).expect("construct");

        assert_eq!(value.mantissa(), &BigInt::from_i128(-12345));
        assert_eq!(value.scale(), 3);
        assert_eq!(value.to_string(), "-12.345");
    }

    #[test]
    fn checked_add_aligns_scales() {
        let lhs: BigNumeric = "1.20".parse().expect("parse lhs");
        let rhs: BigNumeric = "3.004".parse().expect("parse rhs");

        let sum = lhs.checked_add(&rhs).expect("add");

        assert_eq!(sum.to_string(), "4.204");
        assert_eq!(sum.scale(), 3);
        assert_eq!(sum.mantissa(), &BigInt::from_i128(4204));
    }

    #[test]
    fn checked_add_preserves_negative_result() {
        let lhs: BigNumeric = "-1.50".parse().expect("parse lhs");
        let rhs: BigNumeric = "0.25".parse().expect("parse rhs");

        let sum = lhs.checked_add(&rhs).expect("add");

        assert_eq!(sum.to_string(), "-1.25");
        assert_eq!(sum.mantissa(), &BigInt::from_i128(-125));
    }

    #[test]
    fn parse_rejects_invalid_character_and_large_scale() {
        let bad_char = "12.x"
            .parse::<BigNumeric>()
            .expect_err("invalid character should be rejected");
        assert_eq!(bad_char, BigNumericError::MantissaTooLarge);

        let too_many_decimal_places = "0.12345678901234567890123456789"
            .parse::<BigNumeric>()
            .expect_err("scale above 28 should be rejected");
        assert_eq!(too_many_decimal_places, BigNumericError::ScaleTooLarge);
    }

    #[test]
    fn json_roundtrip_and_invalid_error_field() {
        let value: BigNumeric = "-42.500".parse().expect("parse");
        let json = norito::json::to_json(&value).expect("serialize");
        assert_eq!(json, "\"-42.500\"");

        let decoded: BigNumeric = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, value);

        let err = norito::json::from_str::<BigNumeric>("\"not-a-number\"")
            .expect_err("invalid bignumeric string should be rejected");
        match err {
            json::Error::InvalidField { field, message } => {
                assert_eq!(field, "bignumeric");
                assert!(message.contains("invalid bignumeric `not-a-number`"));
            }
            other => panic!("unexpected JSON error: {other:?}"),
        }
    }

    #[test]
    fn decode_from_slice_reports_used_bytes() {
        let value: BigNumeric = "987.65".parse().expect("parse");
        let mut bytes = norito::codec::Encode::encode(&value);
        let encoded_len = bytes.len();
        bytes.extend([0xaa, 0xbb]);

        let (decoded, used) =
            <BigNumeric as norito::core::DecodeFromSlice>::decode_from_slice(&bytes)
                .expect("decode");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded_len);
    }

    #[test]
    fn decode_from_slice_rejects_too_large_scale() {
        let mut bytes = norito::codec::Encode::encode(&BigInt::from_i128(1));
        bytes.extend(norito::codec::Encode::encode(&29_u32));

        let err = <BigNumeric as norito::core::DecodeFromSlice>::decode_from_slice(&bytes)
            .expect_err("scale above 28 should be rejected");
        match err {
            norito::core::Error::Message(message) => {
                assert_eq!(message, "Scale exceeds 28 decimal places");
            }
            other => panic!("unexpected decode error: {other:?}"),
        }
    }
}
