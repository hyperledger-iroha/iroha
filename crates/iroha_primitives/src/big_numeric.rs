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
}
