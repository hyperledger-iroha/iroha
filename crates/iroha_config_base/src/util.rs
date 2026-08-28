//! Various utilities
use derive_more::Display;
use drop_bomb::DropBomb;
use error_stack::Report;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    json::{self, JsonDeserialize, JsonSerialize},
};
use std::time::Duration;
const U64_BYTES: usize = core::mem::size_of::<u64>();
const DURATION_OVERFLOW: &str = "duration does not fit into u64 milliseconds";
/// Serialize [`Duration`] as a number of milliseconds.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Display)]
#[display("{_0:?}")]
#[repr(transparent)]
pub struct DurationMs(pub Duration);
/// Error produced when parsing a [`DurationMs`] from a string fails.
#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("failed to parse duration in milliseconds")]
pub struct ParseDurationMsError;
/// Error produced when parsing a [`Bytes`] value from a string fails.
#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("failed to parse byte count")]
pub struct ParseBytesError;
impl DurationMs {
    /// Access the wrapped [`Duration`].
    #[inline]
    pub fn get(self) -> Duration {
        self.0
    }
    #[inline]
    fn to_millis(self) -> Result<u64, norito::core::Error> {
        self.0
            .as_millis()
            .try_into()
            .map_err(|_| norito::core::Error::Message(DURATION_OVERFLOW.into()))
    }
}
impl From<Duration> for DurationMs {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}
impl NoritoSerialize for DurationMs {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let millis = self.to_millis()?;
        <u64 as NoritoSerialize>::serialize(&millis, writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(U64_BYTES)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(U64_BYTES)
    }
}
impl<'de> NoritoDeserialize<'de> for DurationMs {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let millis = <u64 as NoritoDeserialize>::deserialize(archived.cast());
        Self(Duration::from_millis(millis))
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let millis = <u64 as NoritoDeserialize>::deserialize(archived.cast());
        Ok(Self(Duration::from_millis(millis)))
    }
}
impl JsonSerialize for DurationMs {
    fn json_serialize(&self, out: &mut String) {
        match self.to_millis() {
            Ok(millis) => out.push_str(&millis.to_string()),
            Err(_) => out.push('0'),
        }
    }
}
impl JsonDeserialize for DurationMs {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let millis = parser.parse_u64()?;
        Ok(Self(Duration::from_millis(millis)))
    }
    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        u64::json_from_value(value).map(|millis| Self(Duration::from_millis(millis)))
    }
}
impl core::str::FromStr for DurationMs {
    type Err = ParseDurationMsError;
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        let millis = s.parse::<u64>().map_err(|_| ParseDurationMsError)?;
        Ok(Self(Duration::from_millis(millis)))
    }
}
/// A byte count represented canonically as an unsigned 64-bit integer.
#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct Bytes(pub u64);
impl Bytes {
    /// Access the wrapped value.
    #[inline]
    pub fn get(self) -> u64 {
        self.0
    }
}
impl From<u64> for Bytes {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl core::str::FromStr for Bytes {
    type Err = ParseBytesError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.parse::<u64>().map_err(|_| ParseBytesError)?;
        Ok(Self(value))
    }
}
impl NoritoSerialize for Bytes {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
impl<'de> NoritoDeserialize<'de> for Bytes {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let inner = u64::deserialize(archived.cast());
        Self(inner)
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let inner = u64::try_deserialize(archived.cast())?;
        Ok(Self(inner))
    }
}
impl JsonSerialize for Bytes {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(&self.0.to_string());
    }
}
impl JsonDeserialize for Bytes {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_u64()?;
        Ok(Self(value))
    }
    fn json_from_value(value: &json::Value) -> Result<Self, json::Error> {
        u64::json_from_value(value).map(Self)
    }
}
/// A tool to collect multiple [`Report`]s.
///
/// Will panic on [`Drop`] unless [`Emitter::into_result`] is called.
#[derive(Debug)]
pub struct Emitter<C> {
    report: Option<Report<[C]>>,
    bomb: DropBomb,
}
impl<C> Default for Emitter<C> {
    fn default() -> Self {
        Self::new()
    }
}
impl<C> Emitter<C> {
    /// Constructor
    pub fn new() -> Self {
        Self {
            report: None,
            bomb: DropBomb::new("Emitter dropped without calling `into_result()`"),
        }
    }
    /// Emit a single report
    pub fn emit(&mut self, report: Report<C>) {
        match &mut self.report {
            Some(existing) => existing.extend(core::iter::once(report)),
            None => {
                self.report = Some(report.expand());
            }
        }
    }
    /// Convert into [`Err`] if any report was emitted, otherwise [`Ok`].
    ///
    /// # Errors
    /// If at least one report was emitted.
    pub fn into_result(mut self) -> core::result::Result<(), Report<[C]>> {
        self.bomb.defuse();
        self.report.map_or_else(|| Ok(()), Err)
    }
}
/// An extension of [`Result`] to add convenience methods to work with [`Emitter`].
pub trait EmitterResultExt<T, C> {
    /// If [`Ok`], return [`Some`]; otherwise, emit an error and return [`None`].
    fn ok_or_emit(self, emitter: &mut Emitter<C>) -> Option<T>;
}
impl<T, C> EmitterResultExt<T, C> for core::result::Result<T, Report<C>> {
    fn ok_or_emit(self, emitter: &mut Emitter<C>) -> Option<T> {
        self.map_or_else(
            |report| {
                emitter.emit(report);
                None
            },
            Some,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_ms_json_roundtrip() {
        let original = DurationMs(Duration::from_millis(42));
        let json = json::to_json(&original).expect("serialize");
        assert_eq!(json, "42");
        let parsed: DurationMs = json::from_json(&json).expect("deserialize");
        assert_eq!(parsed.get(), Duration::from_millis(42));
    }
    #[test]
    fn bytes_json_roundtrip() {
        let original: Bytes = Bytes(1024);
        let json = json::to_json(&original).expect("serialize");
        assert_eq!(json, "1024");
        let parsed: Bytes = json::from_json(&json).expect("deserialize");
        assert_eq!(parsed.get(), 1024);
    }
    #[test]
    fn bytes_from_str_parses_numeric() {
        let parsed: Bytes = "2048".parse().expect("parse bytes");
        assert_eq!(parsed.get(), 2048);
    }
    #[test]
    fn bytes_from_str_rejects_invalid_input() {
        let parsed = "nope".parse::<Bytes>();
        assert!(parsed.is_err());
    }
}
