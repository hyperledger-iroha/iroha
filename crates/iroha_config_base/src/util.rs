//! Various utilities
use error_stack::Report;
use norito::{
    DeserializePayload, SerializePayload,
    json::{self, JsonDeserialize, JsonSerialize},
};
use std::{
    fmt::{self, Write as _},
    time::Duration,
};
const U64_BYTES: usize = core::mem::size_of::<u64>();
const DURATION_OVERFLOW: &str = "duration does not fit into u64 milliseconds";
/// Serialize [`Duration`] as a number of milliseconds.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config_base::util::DurationMs")]
#[repr(transparent)]
pub struct DurationMs(pub Duration);
impl fmt::Display for DurationMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
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

impl SerializePayload for DurationMs {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let millis = self.to_millis()?;
        <u64 as SerializePayload>::serialize(&millis, writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(U64_BYTES)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(U64_BYTES)
    }
}

impl<'de> DeserializePayload<'de> for DurationMs {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let millis = <u64 as DeserializePayload>::deserialize(archived.cast());
        Self(Duration::from_millis(millis))
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let millis = <u64 as DeserializePayload>::try_deserialize(archived.cast())?;
        Ok(Self(Duration::from_millis(millis)))
    }
}
impl JsonSerialize for DurationMs {
    fn json_serialize(&self, out: &mut String) {
        match self.to_millis() {
            Ok(millis) => write!(out, "{millis}").expect("writing to String cannot fail"),
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
#[derive(Debug, Copy, Clone, norito::NoritoSchema)]
#[norito_schema(name = "iroha_config_base::util::Bytes")]
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

impl SerializePayload for Bytes {
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

impl<'de> DeserializePayload<'de> for Bytes {
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
        write!(out, "{}", self.0).expect("writing to String cannot fail");
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
/// Guard that panics when dropped unless [`DropBomb::defuse`] was called.
///
/// Values whose result must be consumed explicitly hold one. The panic is
/// skipped while the thread is already unwinding, so it never aborts.
#[derive(Debug)]
#[must_use]
pub(crate) struct DropBomb {
    message: &'static str,
    defused: bool,
}
impl DropBomb {
    /// Arm a guard that panics with `message` if dropped undefused.
    pub(crate) const fn new(message: &'static str) -> Self {
        Self {
            message,
            defused: false,
        }
    }
    /// Disarm the guard so dropping it is a no-op.
    pub(crate) fn defuse(&mut self) {
        self.defused = true;
    }
}
impl Drop for DropBomb {
    fn drop(&mut self) {
        if !self.defused && !std::thread::panicking() {
            panic!("{}", self.message);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_ms_display_preserves_debug_units_and_formatter_options() {
        for (duration, expected) in [
            (Duration::ZERO, "0ns"),
            (Duration::from_nanos(1), "1ns"),
            (Duration::from_micros(1), "1µs"),
            (Duration::from_millis(1), "1ms"),
            (Duration::from_millis(1234), "1.234s"),
            (Duration::MAX, "18446744073709551615.999999999s"),
        ] {
            let value = DurationMs(duration);
            assert_eq!(format!("{value}"), expected);
            assert_eq!(format!("{value:>64.3}"), format!("{duration:>64.3?}"));
            assert_eq!(format!("{value:🦀^64.1}"), format!("{duration:🦀^64.1?}"));
            assert_eq!(format!("{value:+#064.3}"), format!("{duration:+#064.3?}"));
        }
    }

    #[test]
    fn duration_ms_display_propagates_formatter_failure() {
        use std::fmt::Write as _;

        struct FailingWriter;
        impl fmt::Write for FailingWriter {
            fn write_str(&mut self, _: &str) -> fmt::Result {
                Err(fmt::Error)
            }
        }
        let value = DurationMs(Duration::from_millis(1234));
        assert_eq!(write!(FailingWriter, "{value}"), Err(fmt::Error));
    }

    #[test]
    #[should_panic(expected = "bomb went off")]
    fn drop_bomb_panics_when_dropped_armed() {
        drop(DropBomb::new("bomb went off"));
    }

    #[test]
    fn drop_bomb_is_silent_once_defused() {
        let mut bomb = DropBomb::new("bomb went off");
        bomb.defuse();
        drop(bomb);
    }

    #[test]
    fn drop_bomb_does_not_panic_while_unwinding() {
        let result = std::panic::catch_unwind(|| {
            let _bomb = DropBomb::new("second panic");
            panic!("first panic");
        });
        let payload = result.expect_err("the first panic propagates");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"first panic"));
    }

    #[test]
    fn manual_decode_duration_returns_missing_context_and_retries_same_payload() {
        use norito::core::{
            DecodeFlagsGuard, PayloadCtxGuard, archived_from_slice, serialize_to_buffer,
        };
        let _layout = DecodeFlagsGuard::enter(0);
        let expected = DurationMs(Duration::from_millis(42));
        let mut bytes = Vec::new();
        serialize_to_buffer(&expected, &mut bytes).unwrap();
        assert_eq!(bytes, 42_u64.to_le_bytes());
        let archived = archived_from_slice::<u8>(&bytes).unwrap();
        let missing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            DurationMs::try_deserialize(archived.archived().cast())
        }));
        assert!(
            matches!(missing, Ok(Err(norito::Error::MissingPayloadContext))),
            "fallible DurationMs decoder must return the missing-context error, not panic"
        );
        let decoded = {
            let _payload = PayloadCtxGuard::enter(archived.bytes());
            DurationMs::try_deserialize(archived.archived().cast()).unwrap()
        };
        assert_eq!(decoded, expected);
        let mut repeated = Vec::new();
        serialize_to_buffer(&decoded, &mut repeated).unwrap();
        assert_eq!(repeated, bytes);
        let truncated = norito::core::archived_from_slice::<u8>(&bytes[..7]).unwrap();
        let _payload = PayloadCtxGuard::enter(truncated.bytes());
        assert!(DurationMs::try_deserialize(truncated.archived().cast()).is_err());
    }

    fn check_scalar_frame<T>(owner: &str, value: &T)
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    {
        let rows: Vec<json::Value> = include_str!("../tests/fixtures/config_scalar_frames.jsonl")
            .lines()
            .map(|line| json::from_json(line).expect("original compiler observation"))
            .collect();
        assert_eq!(rows.len(), 4);
        let rows: Vec<_> = rows
            .iter()
            .filter(|row| row["owner"].as_str() == Some(owner))
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["direction"].as_str(), Some("Serialize"));
        assert_eq!(rows[1]["direction"].as_str(), Some("Deserialize"));
        for row in rows {
            assert_eq!(row["nominal"].as_str().unwrap(), T::nominal_name());
            assert_eq!(
                row["schema_hash"].as_str().unwrap(),
                hex::encode(norito::schema::identity::frame_hash::<T>()),
            );
            let expected = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
            assert_eq!(norito::encode_canonical(value).unwrap(), expected);
            let decoded: T = norito::decode_canonical(&expected).unwrap();
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), expected);
            for len in 0..expected.len() {
                assert!(norito::decode_canonical::<T>(&expected[..len]).is_err());
            }
            let mut wrong_owner = expected.clone();
            wrong_owner[6] ^= 1;
            assert!(norito::decode_canonical::<T>(&wrong_owner).is_err());
            let mut trailing = expected;
            trailing.push(0);
            assert!(norito::decode_canonical::<T>(&trailing).is_err());
        }
    }

    #[test]
    fn duration_ms_frame_matches_original_capture() {
        check_scalar_frame("DurationMs", &DurationMs(Duration::from_millis(42)));
    }

    #[test]
    fn bytes_frame_matches_original_capture() {
        check_scalar_frame("Bytes", &Bytes(1024));
    }

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
    fn scalar_json_preserves_decimal_boundaries_and_appends_to_existing_text() {
        let mut values = vec![0, u64::MAX];
        for exponent in 0..=19 {
            let power = 10_u64.pow(exponent);
            values.extend([power - 1, power, power + 1]);
        }
        for value in values {
            let expected = format!("prefix:{value}");
            let mut bytes = String::from("prefix:");
            Bytes(value).json_serialize(&mut bytes);
            assert_eq!(bytes, expected);
            let mut duration = String::from("prefix:");
            DurationMs(Duration::from_millis(value)).json_serialize(&mut duration);
            assert_eq!(duration, expected);
        }
    }

    #[test]
    fn duration_json_keeps_submillisecond_truncation_and_overflow_zero() {
        let maximum_millis = Duration::from_millis(u64::MAX);
        for (duration, expected) in [
            (Duration::ZERO, "0"),
            (Duration::from_nanos(999_999), "0"),
            (Duration::from_nanos(1_000_001), "1"),
            (Duration::new(1, 999_999_999), "1999"),
            (maximum_millis, "18446744073709551615"),
            (
                maximum_millis + Duration::from_nanos(999_999),
                "18446744073709551615",
            ),
            (maximum_millis + Duration::from_millis(1), "0"),
            (Duration::MAX, "0"),
        ] {
            let mut output = String::from("prefix:");
            DurationMs(duration).json_serialize(&mut output);
            assert_eq!(output, format!("prefix:{expected}"));
        }
    }

    #[test]
    fn scalar_json_keeps_bounded_serialization_unsupported() {
        for value in [0, u64::MAX] {
            assert_eq!(
                json::to_json_bounded(&Bytes(value), usize::MAX),
                Err(json::BoundedJsonError::Unsupported)
            );
        }
        for duration in [Duration::ZERO, Duration::MAX] {
            assert_eq!(
                json::to_json_bounded(&DurationMs(duration), usize::MAX),
                Err(json::BoundedJsonError::Unsupported)
            );
        }
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
