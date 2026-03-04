//! Deterministic settlement and liquidity primitives shared across Nexus lanes.
//!
//! This crate provides the building blocks required by the unified settlement
//! router outlined in `roadmap.md`.  The goal is to keep all arithmetic and
//! accounting logic deterministic so the same inputs always produce the same
//! XOR liability regardless of hardware or thread scheduling.  It packages the
//! shadow-price calculator, buffer guard rails, volatility handling, swap-line
//! health checks, and receipt helpers consumed by `iroha_core`'s
//! `SettlementEngine`; treasury automation and AMM/RFQ execution hook into
//! these deterministic primitives.

#![deny(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::clone_on_ref_ptr,
    clippy::if_not_else,
    clippy::implicit_clone,
    clippy::missing_const_for_fn,
    clippy::multiple_inherent_impl,
    clippy::redundant_clone,
    clippy::unwrap_used,
    clippy::use_self
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::{
    fmt,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use norito::{
    Archived, Error, NoritoDeserialize, NoritoSerialize,
    core::DecodeFromSlice,
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize, Parser},
};
use rust_decimal::Decimal;
use time::{Duration, OffsetDateTime};

pub mod config;
pub mod haircut;
pub mod policy;
pub mod price;
pub mod receipt;
pub mod swapline;
pub mod volatility;

pub use DurationSeconds as TwapWindowSeconds;
pub use config::{EpsilonBps, SettlementConfig};
pub use haircut::{HaircutTier, LiquidityProfile};
pub use policy::{BufferCapacity, BufferPolicy, BufferStatus};
pub use price::{ShadowPrice, ShadowPriceCalculator};
pub use receipt::SettlementReceipt;
pub use swapline::{CollateralKind, SwapLineConfig, SwapLineExposure, SwapLineId};
pub use volatility::VolatilityBucket;

/// Convenience alias for deterministic XOR amounts measured in the smallest
/// unit (micro-XOR).  Using `Decimal` keeps arithmetic precise without relying
/// on floating point operations, which would violate determinism guarantees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MicroXor(Decimal);

impl MicroXor {
    /// Zero-valued amount.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Construct from a raw decimal value.
    #[must_use]
    pub const fn new(inner: Decimal) -> Self {
        Self(inner)
    }

    /// Access the underlying decimal value.
    #[must_use]
    pub const fn into_decimal(self) -> Decimal {
        self.0
    }

    /// Whether the amount is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0.is_zero()
    }
}

impl From<Decimal> for MicroXor {
    fn from(value: Decimal) -> Self {
        Self(value)
    }
}

impl From<MicroXor> for Decimal {
    fn from(value: MicroXor) -> Self {
        value.0
    }
}

impl Deref for MicroXor {
    type Target = Decimal;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MicroXor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl NoritoSerialize for MicroXor {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
        self.0.to_string().serialize(writer)
    }
}

impl<'a> NoritoDeserialize<'a> for MicroXor {
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        let ptr = std::ptr::from_ref(archived).cast::<u8>();
        let (base, total) = norito::core::payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let ptr_us = ptr as usize;
        let base_end = base.checked_add(total).ok_or(Error::LengthMismatch)?;
        if ptr_us < base || ptr_us >= base_end {
            return Err(Error::LengthMismatch);
        }
        let start = ptr_us - base;
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let (len, hdr) = norito::core::read_len_dyn_slice(&payload[start..])?;
        let data_start = start.checked_add(hdr).ok_or(Error::LengthMismatch)?;
        let data_end = data_start.checked_add(len).ok_or(Error::LengthMismatch)?;
        let bytes = payload
            .get(data_start..data_end)
            .ok_or(Error::LengthMismatch)?;
        let raw = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        Decimal::from_str(raw)
            .map(MicroXor)
            .map_err(|err| Error::Message(format!("invalid decimal: {err}")))
    }

    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("deserializing MicroXor should not fail")
    }
}

impl<'a> DecodeFromSlice<'a> for MicroXor {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (raw, used) = <&str as DecodeFromSlice>::decode_from_slice(bytes)?;
        let value = Decimal::from_str(raw)
            .map_err(|err| Error::Message(format!("invalid decimal: {err}")))?;
        Ok((Self(value), used))
    }
}

impl FastJsonWrite for MicroXor {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.0.to_string(), out);
    }
}

impl JsonDeserialize for MicroXor {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let raw = parser.parse_string()?;
        Decimal::from_str(&raw)
            .map(Self)
            .map_err(|err| json::Error::InvalidField {
                field: "micro_xor".into(),
                message: format!("invalid decimal `{raw}`: {err}"),
            })
    }
}

/// UTC timestamp rounded to milliseconds since Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampMs(OffsetDateTime);

impl TimestampMs {
    /// Construct from milliseconds since Unix epoch.
    pub fn from_unix_millis(ms: u64) -> Result<Self, String> {
        let nanos = i128::from(ms) * 1_000_000;
        OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .map(Self)
            .map_err(|err| format!("timestamp out of range: {err}"))
    }

    /// Extract milliseconds since Unix epoch.
    #[must_use]
    pub fn as_unix_millis(self) -> u64 {
        let nanos = self.0.unix_timestamp_nanos();
        let clamped = nanos.clamp(0, i128::from(u64::MAX) * 1_000_000);
        u64::try_from(clamped / 1_000_000).expect("timestamp clamped to u64 milliseconds range")
    }

    /// Access the underlying timestamp.
    #[must_use]
    pub const fn as_offset_datetime(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for TimestampMs {
    fn from(value: OffsetDateTime) -> Self {
        Self(value)
    }
}

impl From<TimestampMs> for OffsetDateTime {
    fn from(value: TimestampMs) -> Self {
        value.0
    }
}

impl NoritoSerialize for TimestampMs {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
        self.as_unix_millis().serialize(writer)
    }
}

impl<'a> NoritoDeserialize<'a> for TimestampMs {
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        let ptr = std::ptr::from_ref(archived).cast::<u8>();
        let (base, total) = norito::core::payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let ptr_us = ptr as usize;
        let base_end = base.checked_add(total).ok_or(Error::LengthMismatch)?;
        let offset = ptr_us.checked_sub(base).ok_or(Error::LengthMismatch)?;
        let end = offset.checked_add(8).ok_or(Error::LengthMismatch)?;
        if ptr_us < base || ptr_us >= base_end || end > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = payload.get(offset..end).ok_or(Error::LengthMismatch)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        let millis = u64::from_le_bytes(buf);
        Self::from_unix_millis(millis)
            .map_err(|err| Error::Message(format!("invalid timestamp: {err}")))
    }

    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("timestamp should deserialize")
    }
}

impl<'a> DecodeFromSlice<'a> for TimestampMs {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (millis, used) = <u64 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let timestamp = Self::from_unix_millis(millis)
            .map_err(|err| Error::Message(format!("invalid timestamp: {err}")))?;
        Ok((timestamp, used))
    }
}

impl FastJsonWrite for TimestampMs {
    fn write_json(&self, out: &mut String) {
        self.as_unix_millis().json_serialize(out);
    }
}

impl JsonDeserialize for TimestampMs {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let millis = u64::json_deserialize(parser)?;
        Self::from_unix_millis(millis).map_err(json::Error::Message)
    }
}

/// Duration stored as whole seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DurationSeconds(Duration);

impl DurationSeconds {
    /// Construct from a `time::Duration`.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Access the underlying duration.
    #[must_use]
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    /// Whole seconds represented by this duration.
    #[must_use]
    pub const fn whole_seconds(self) -> i64 {
        self.0.whole_seconds()
    }
}

impl From<Duration> for DurationSeconds {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl From<DurationSeconds> for Duration {
    fn from(value: DurationSeconds) -> Self {
        value.0
    }
}

impl NoritoSerialize for DurationSeconds {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
        self.0.whole_seconds().serialize(writer)
    }
}

impl<'a> NoritoDeserialize<'a> for DurationSeconds {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        let seconds_arch: &Archived<i64> = archived.cast();
        let seconds = i64::deserialize(seconds_arch);
        Self(Duration::seconds(seconds))
    }
}

impl<'a> DecodeFromSlice<'a> for DurationSeconds {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (seconds, used) = <i64 as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok((Self(Duration::seconds(seconds)), used))
    }
}

impl FastJsonWrite for DurationSeconds {
    fn write_json(&self, out: &mut String) {
        self.0.whole_seconds().json_serialize(out);
    }
}

impl JsonDeserialize for DurationSeconds {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let seconds = i64::json_deserialize(parser)?;
        Ok(Self(Duration::seconds(seconds)))
    }
}

impl fmt::Display for MicroXor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use norito::decode_from_bytes;
    use rust_decimal::Decimal;

    use super::{MicroXor, TimestampMs};

    #[test]
    fn micro_xor_norito_roundtrip() {
        let amount = MicroXor::from(Decimal::new(123_456, 2));
        let bytes = norito::to_bytes(&amount).expect("encode");
        let decoded: MicroXor = decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, amount);
    }

    #[test]
    fn timestamp_ms_norito_roundtrip() {
        let timestamp = TimestampMs::from_unix_millis(1_700_000_000_000).expect("timestamp");
        let bytes = norito::to_bytes(&timestamp).expect("encode");
        let decoded: TimestampMs = decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, timestamp);
    }
}
