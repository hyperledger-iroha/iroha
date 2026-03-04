#![allow(clippy::excessive_precision)]

use core::fmt;
use core::ops::{Add, Div, Mul, Sub};
use core::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod prelude {
    pub use crate::{FromPrimitive, ToPrimitive};
}

const DECIMAL_PRECISION: u32 = 18;
const MAX_SCALE: u32 = 28;
const MAX_MANTISSA: i128 = (1_i128 << 96) - 1;

fn pow10(exp: u32) -> i128 {
    10_i128.pow(exp)
}

fn normalize(mantissa: i128, mut scale: u32) -> (i128, u32) {
    let mut value = mantissa;
    while scale > 0 {
        let divisor = 10_i128;
        if value % divisor != 0 {
            break;
        }
        value /= divisor;
        scale -= 1;
    }
    (value, scale)
}

fn digits_to_i128(digits: &[u8]) -> i128 {
    let mut value = 0i128;
    for &digit in digits {
        value = value * 10 + i128::from(digit);
    }
    value
}

fn round_digits(digits: &mut Vec<u8>) {
    let mut index = digits.len();
    while index > 0 {
        index -= 1;
        if digits[index] < 9 {
            digits[index] += 1;
            return;
        }
        digits[index] = 0;
    }
    digits.insert(0, 1);
}

fn reduce_scale_with_rounding(digits: &mut Vec<u8>, scale: &mut u32) -> bool {
    if *scale == 0 || digits.is_empty() {
        return false;
    }
    let removed = digits.pop().unwrap();
    *scale -= 1;
    if removed >= 5 {
        round_digits(digits);
    }
    if digits.is_empty() {
        digits.push(0);
    }
    true
}

fn trim_leading_integer_zeros(digits: &mut Vec<u8>, scale: u32) {
    let integer_len = digits.len().saturating_sub(scale as usize);
    if integer_len <= 1 {
        return;
    }
    let mut remove = 0usize;
    while remove + 1 < integer_len && digits.get(remove) == Some(&0) {
        remove += 1;
    }
    if remove > 0 {
        digits.drain(0..remove);
    }
}

/// Minimal deterministic decimal implementation supporting the subset of
/// operations required by the settlement router.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decimal {
    mantissa: i128,
    scale: u32,
}

/// Error returned when a conversion to or from `Decimal` fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertError;

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("decimal conversion failed")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConvertError {}

impl Decimal {
    /// Zero constant.
    pub const ZERO: Self = Self::new_raw(0, 0);
    /// Unit constant.
    pub const ONE: Self = Self::new_raw(1, 0);
    /// Maximum decimal (2^96 - 1).
    pub const MAX: Self = Self::new_raw(MAX_MANTISSA, 0);

    const fn new_raw(mantissa: i128, scale: u32) -> Self {
        Self { mantissa, scale }
    }

    /// Construct from an integer mantissa and scale.
    #[must_use]
    pub fn new(mantissa: i64, scale: u32) -> Self {
        let (mantissa, scale) = normalize(i128::from(mantissa), scale);
        Self::new_raw(mantissa, scale)
    }

    /// General constructor from types implementing `Into<Decimal>`.
    #[must_use]
    pub fn from<T: Into<Self>>(value: T) -> Self {
        value.into()
    }

    /// Construct a decimal from its raw representation.
    #[must_use]
    pub const fn from_parts(lo: u32, mid: u32, hi: u32, negative: bool, scale: u32) -> Self {
        if scale > MAX_SCALE {
            panic!("scale too large");
        }
        let mantissa = ((hi as i128) << 64) | ((mid as i128) << 32) | (lo as i128);
        let mantissa = if negative { -mantissa } else { mantissa };
        Self { mantissa, scale }
    }

    /// Convert from `u128`.
    #[must_use]
    pub fn from_u128(value: u128) -> Option<Self> {
        if value > MAX_MANTISSA as u128 {
            return None;
        }
        Some(Self::new_raw(value as i128, 0))
    }

    /// Check whether the value is zero.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.mantissa == 0
    }

    /// Return the fractional component.
    #[must_use]
    pub fn fract(&self) -> Self {
        if self.scale == 0 || self.mantissa == 0 {
            return Self::ZERO;
        }
        let factor = pow10(self.scale);
        let fractional = self.mantissa.rem_euclid(factor);
        let (mantissa, scale) = normalize(fractional, self.scale);
        Self::new_raw(mantissa, scale)
    }

    /// Return the integer component.
    #[must_use]
    pub fn trunc(&self) -> Self {
        if self.scale == 0 {
            return *self;
        }
        let factor = pow10(self.scale);
        Self::new_raw(self.mantissa / factor, 0)
    }

    /// Returns `true` if the value is negative.
    #[must_use]
    pub const fn is_sign_negative(&self) -> bool {
        self.mantissa < 0
    }

    /// Returns `true` if the value is positive or zero.
    #[must_use]
    pub const fn is_sign_positive(&self) -> bool {
        !self.is_sign_negative()
    }

    /// Expose the raw mantissa.
    #[must_use]
    pub const fn mantissa(&self) -> i128 {
        self.mantissa
    }

    /// Expose the scale.
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    fn align(self, other: Self) -> (i128, i128, u32) {
        if self.scale == other.scale {
            return (self.mantissa, other.mantissa, self.scale);
        }
        let target_scale = self.scale.max(other.scale);
        let lhs = self.mantissa * pow10(target_scale - self.scale);
        let rhs = other.mantissa * pow10(target_scale - other.scale);
        (lhs, rhs, target_scale)
    }

    fn reduce(self) -> Self {
        let (mantissa, scale) = normalize(self.mantissa, self.scale);
        Self::new_raw(mantissa, scale)
    }

    fn limit_scale(self) -> Self {
        if self.scale <= MAX_SCALE {
            return self;
        }
        self.round_dp(MAX_SCALE)
    }

    fn div_internal(self, other: Self) -> Self {
        assert!(other.mantissa != 0, "division by zero");
        let scale = self.scale.saturating_add(DECIMAL_PRECISION);
        let numerator = self
            .mantissa
            .checked_mul(pow10(DECIMAL_PRECISION))
            .expect("division scale overflow");
        let mantissa = numerator / other.mantissa;
        Self::new_raw(mantissa, scale).reduce()
    }

    /// Convert into a `u64` if the value is an integer and fits.
    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        if self.scale == 0 {
            return u64::try_from(self.mantissa).ok();
        }
        let factor = pow10(self.scale);
        if self.mantissa % factor != 0 {
            return None;
        }
        u64::try_from(self.mantissa / factor).ok()
    }

    /// Checked addition.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let (lhs, rhs, scale) = self.align(other);
        lhs.checked_add(rhs)
            .map(|mantissa| Self::new_raw(mantissa, scale).reduce())
    }

    /// Checked subtraction.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        let (lhs, rhs, scale) = self.align(other);
        lhs.checked_sub(rhs)
            .map(|mantissa| Self::new_raw(mantissa, scale).reduce())
    }

    /// Checked multiplication.
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let scale = self.scale.saturating_add(other.scale);
        let mantissa = self.mantissa.checked_mul(other.mantissa)?;
        let result = Self::new_raw(mantissa, scale).reduce();
        Some(result.limit_scale())
    }

    /// Checked division.
    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.mantissa == 0 {
            return None;
        }
        let result = self.div_internal(other);
        Some(result.limit_scale())
    }

    /// Checked remainder.
    pub fn checked_rem(self, other: Self) -> Option<Self> {
        let (lhs, rhs, scale) = self.align(other);
        let remainder = lhs.checked_rem(rhs)?;
        Some(Self::new_raw(remainder, scale).reduce())
    }

    /// Round to `scale` decimal places using bankers rounding.
    #[must_use]
    pub fn round_dp(self, scale: u32) -> Self {
        if self.scale <= scale {
            return self;
        }
        let drop = self.scale - scale;
        let factor = pow10(drop);
        let quotient = self.mantissa / factor;
        let remainder = self.mantissa % factor;
        let half = factor / 2;
        let abs_remainder = remainder.abs();
        let mut rounded = quotient;
        if abs_remainder > half {
            rounded += if self.mantissa.is_negative() { -1 } else { 1 };
        } else if abs_remainder == half && (quotient & 1) != 0 {
            rounded += if self.mantissa.is_negative() { -1 } else { 1 };
        }
        Self::new_raw(rounded, scale).reduce()
    }

}

impl Add for Decimal {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let (lhs_adj, rhs_adj, scale) = self.align(rhs);
        Self::new_raw(lhs_adj + rhs_adj, scale).reduce()
    }
}

impl Sub for Decimal {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let (lhs_adj, rhs_adj, scale) = self.align(rhs);
        Self::new_raw(lhs_adj - rhs_adj, scale).reduce()
    }
}

impl Mul for Decimal {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new_raw(self.mantissa * rhs.mantissa, self.scale + rhs.scale).reduce()
    }
}

impl<'a> Mul<Decimal> for &'a Decimal {
    type Output = Decimal;

    fn mul(self, rhs: Decimal) -> Self::Output {
        (*self).mul(rhs)
    }
}

impl<'a> Mul<&'a Decimal> for Decimal {
    type Output = Decimal;

    fn mul(self, rhs: &'a Decimal) -> Self::Output {
        self.mul(*rhs)
    }
}

impl<'a, 'b> Mul<&'a Decimal> for &'b Decimal {
    type Output = Decimal;

    fn mul(self, rhs: &'a Decimal) -> Self::Output {
        (*self).mul(*rhs)
    }
}

impl Div for Decimal {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        self.div_internal(rhs)
    }
}

impl<'a> Div<Decimal> for &'a Decimal {
    type Output = Decimal;

    fn div(self, rhs: Decimal) -> Self::Output {
        (*self).div(rhs)
    }
}

impl<'a> Div<&'a Decimal> for Decimal {
    type Output = Decimal;

    fn div(self, rhs: &'a Decimal) -> Self::Output {
        self.div(*rhs)
    }
}

impl<'a, 'b> Div<&'a Decimal> for &'b Decimal {
    type Output = Decimal;

    fn div(self, rhs: &'a Decimal) -> Self::Output {
        (*self).div(*rhs)
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        let (lhs, rhs, _) = self.align(*other);
        lhs.partial_cmp(&rhs)
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let (lhs, rhs, _) = self.align(*other);
        lhs.cmp(&rhs)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.mantissa);
        }
        let factor = pow10(self.scale);
        let mantissa = self.mantissa;
        let sign = if mantissa < 0 { "-" } else { "" };
        let mantissa_abs = mantissa.abs();
        let integer = mantissa_abs / factor;
        let fractional = mantissa_abs % factor;
        write!(f, "{}{}.{:0width$}", sign, integer, fractional, width = self.scale as usize)
    }
}

impl FromStr for Decimal {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err("empty");
        }
        let negative = s.starts_with('-');
        let digits = if negative || s.starts_with('+') {
            &s[1..]
        } else {
            s
        };
        let parts: Vec<&str> = digits.split('.').collect();
        if parts.len() > 2 {
            return Err("invalid format");
        }
        let integer = parts[0];
        let fractional = if parts.len() == 2 { parts[1] } else { "" };
        if fractional.len() > MAX_SCALE as usize {
            return Err("scale too large");
        }
        let scale = fractional.len() as u32;
        let mantissa_str = format!("{}{}", integer, fractional);
        let mut mantissa = mantissa_str.parse::<i128>().map_err(|_| "invalid digits")?;
        if mantissa.abs() > MAX_MANTISSA {
            return Err("mantissa too large");
        }
        if negative {
            mantissa = -mantissa;
        }
        Ok(Decimal::new_raw(mantissa, scale).reduce())
    }
}

#[cfg(feature = "serde")]
impl Serialize for Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Decimal::from_str(&s).map_err(serde::de::Error::custom)
    }
}

macro_rules! impl_from_int {
    ($($t:ty),*) => {
        $(impl From<$t> for Decimal {
            fn from(value: $t) -> Self {
                Decimal::new_raw(i128::from(value as i64), 0)
            }
        })*
    };
}

impl_from_int!(u8, u16, u32, i8, i16, i32, i64);

impl From<u64> for Decimal {
    fn from(value: u64) -> Self {
        Decimal::new_raw(value as i128, 0)
    }
}

impl From<i128> for Decimal {
    fn from(value: i128) -> Self {
        Decimal::new_raw(value, 0)
    }
}

impl From<u128> for Decimal {
    fn from(value: u128) -> Self {
        Decimal::new_raw(value as i128, 0)
    }
}

impl TryFrom<f64> for Decimal {
    type Error = ConvertError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(ConvertError);
        }
        if value == 0.0 {
            return Ok(Decimal::ZERO);
        }
        let mut repr = format!("{value:.prec$}", value = value, prec = MAX_SCALE as usize);
        if let Some(dot) = repr.find('.') {
            while repr.len() > dot + 1 && repr.ends_with('0') {
                repr.pop();
            }
            if repr.ends_with('.') {
                repr.pop();
            }
        }
        if repr == "-0" {
            repr = "0".to_owned();
        }
        match Decimal::from_str(&repr) {
            Ok(decimal) => Ok(decimal),
            Err("mantissa too large") => {
                let negative = repr.starts_with('-');
                let unsigned = if negative { &repr[1..] } else { &repr[..] };
                let mut integer = unsigned;
                let mut fractional = "";
                if let Some(dot) = unsigned.find('.') {
                    integer = &unsigned[..dot];
                    fractional = &unsigned[dot + 1..];
                }
                let mut digits: Vec<u8> = integer
                    .chars()
                    .chain(fractional.chars())
                    .map(|ch| ch.to_digit(10).map(|d| d as u8).ok_or(ConvertError))
                    .collect::<Result<_, _>>()?;
                let mut scale = fractional.len() as u32;

                while scale > 0 && digits.last() == Some(&0) {
                    digits.pop();
                    scale -= 1;
                }
                if digits.is_empty() {
                    return Ok(Decimal::ZERO);
                }
                loop {
                    trim_leading_integer_zeros(&mut digits, scale);
                    while digits.len() > 29 {
                        if !reduce_scale_with_rounding(&mut digits, &mut scale) {
                            return Err(ConvertError);
                        }
                    }
                    let mantissa_abs = digits_to_i128(&digits);
                    if mantissa_abs <= MAX_MANTISSA {
                        let mantissa = if negative {
                            -mantissa_abs
                        } else {
                            mantissa_abs
                        };
                        return Ok(Decimal::new_raw(mantissa, scale).reduce());
                    }
                    if scale == 0 {
                        return Err(ConvertError);
                    }
                    if !reduce_scale_with_rounding(&mut digits, &mut scale) {
                        return Err(ConvertError);
                    }
                    if digits.iter().all(|&d| d == 0) {
                        return Ok(Decimal::ZERO);
                    }
                }
            }
            Err(_) => Err(ConvertError),
        }
    }
}

impl TryFrom<Decimal> for u32 {
    type Error = ConvertError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        let v = value.to_u64().ok_or(ConvertError)?;
        u32::try_from(v).map_err(|_| ConvertError)
    }
}

impl TryFrom<Decimal> for u64 {
    type Error = ConvertError;

    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        value.to_u64().ok_or(ConvertError)
    }
}

pub trait FromPrimitive {
    fn from_u128(n: u128) -> Option<Decimal>;
}

pub trait ToPrimitive {
    fn to_u64(&self) -> Option<u64>;
    fn to_f64(&self) -> Option<f64>;
}

impl FromPrimitive for Decimal {
    fn from_u128(n: u128) -> Option<Decimal> {
        Decimal::from_u128(n)
    }
}

impl ToPrimitive for Decimal {
    fn to_u64(&self) -> Option<u64> {
        Decimal::to_u64(self)
    }

    fn to_f64(&self) -> Option<f64> {
        let divisor = 10f64.powi(self.scale as i32);
        Some(self.mantissa as f64 / divisor)
    }
}

impl ToPrimitive for &Decimal {
    fn to_u64(&self) -> Option<u64> {
        Decimal::to_u64(self)
    }

    fn to_f64(&self) -> Option<f64> {
        let divisor = 10f64.powi(self.scale as i32);
        Some(self.mantissa as f64 / divisor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::ToPrimitive;

    #[test]
    fn from_parts_roundtrip() {
        let dec = Decimal::from_parts(1, 0, 0, false, 0);
        assert_eq!(dec.mantissa(), 1);
        assert_eq!(dec.scale(), 0);

        let dec = Decimal::from_parts(42, 0, 0, true, 2);
        assert_eq!(dec.mantissa(), -42);
        assert_eq!(dec.scale(), 2);
    }

    #[test]
    fn bankers_rounding() {
        let lower = Decimal::from_str("2.5").unwrap().round_dp(0);
        let upper = Decimal::from_str("3.5").unwrap().round_dp(0);
        assert_eq!(lower.to_string(), "2");
        assert_eq!(upper.to_string(), "4");
    }

    #[test]
    fn display_preserves_negative_fractional_sign() {
        let dec = Decimal::new_raw(-5, 1);
        assert_eq!(dec.to_string(), "-0.5");
    }

    #[test]
    fn checked_mul_limits_scale() {
        let lhs = Decimal::from_str("1.50").unwrap();
        let rhs = Decimal::from_str("2").unwrap();
        let product = lhs.checked_mul(rhs).unwrap();
        assert_eq!(product.to_string(), "3");
    }

    #[test]
    fn try_from_f64_roundtrips() {
        let value = 12.345;
        let dec = Decimal::try_from(value).unwrap();
        let roundtrip = dec.to_f64().unwrap();
        assert!((roundtrip - value).abs() < 1e-6);
    }

    #[test]
    fn try_from_f64_negative_fraction() {
        let dec = Decimal::try_from(-0.5f64).unwrap();
        assert_eq!(dec.to_string(), "-0.5");
    }

    #[test]
    fn try_from_f64_handles_binary_rounding() {
        let value = 0.1f64;
        let dec = Decimal::try_from(value).unwrap();
        let roundtrip = dec.to_f64().unwrap();
        assert!((roundtrip - value).abs() < 1e-6);
    }

    #[test]
    fn try_into_u64() {
        let dec = Decimal::from_str("42").unwrap();
        let value: u64 = dec.try_into().unwrap();
        assert_eq!(value, 42);
    }
}
