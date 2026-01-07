//! Time event and filter
use std::{convert::TryFrom, ops::Range, string::String, time::Duration};

use derive_more::Constructor;
use getset::Getters;
use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Special event that is emitted when state is ready for handling time-triggers
    ///
    /// Contains time interval which is used to identify time-triggers to be executed
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        IntoSchema,
        Constructor,
    )]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TimeEvent {
        /// Time interval between creation of two blocks
        pub interval: TimeInterval,
    }

    /// Filter time-events and allow only the ones within the given time interval.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Constructor, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TimeEventFilter(pub ExecutionTime);

    /// Trigger execution time
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum ExecutionTime {
        /// Execute right before block commit
        PreCommit,
        /// Execute with some schedule
        Schedule(Schedule),
    }

    /// Schedule of the trigger
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Schedule {
        /// The first execution time
        pub start_ms: u64,
        /// If some, the period between cyclic executions
        pub period_ms: Option<u64>,
    }

    /// Time interval in which `TimeAction` should appear
    ///
    /// `since_ms` and `length_ms` are serialized as a number of milliseconds.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    // Durations are represented explicitly as millisecond counts (`since_ms`, `length_ms`) and
    // JSON serialization is implemented manually below to keep the historical object layout.
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TimeInterval {
        /// The start of a time interval
        pub since_ms: u64,
        /// The length of a time interval
        pub length_ms: u64,
    }
}

// Norito slice decoding helpers: delegate to `Decode` using a cursor.
impl<'a> norito::core::DecodeFromSlice<'a> for ExecutionTime {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((
            v,
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for TimeEventFilter {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((
            v,
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?,
        ))
    }
}

// Internal wire helper with a stable Norito tuple layout
mod wire {
    use norito::core as ncore;

    use super::*;

    pub(super) struct TimeIntervalWire(pub u64, pub u64);

    impl From<TimeInterval> for TimeIntervalWire {
        fn from(t: TimeInterval) -> Self {
            Self(t.since_ms, t.length_ms)
        }
    }
    impl From<TimeIntervalWire> for TimeInterval {
        fn from(w: TimeIntervalWire) -> Self {
            Self {
                since_ms: w.0,
                length_ms: w.1,
            }
        }
    }

    impl ncore::NoritoSerialize for TimeIntervalWire {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            <(u64, u64) as ncore::NoritoSerialize>::serialize(&(self.0, self.1), writer)
        }
    }
    impl<'de> ncore::NoritoDeserialize<'de> for TimeIntervalWire {
        fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
            let (a, b): (u64, u64) =
                <(u64, u64) as ncore::NoritoDeserialize>::deserialize(archived.cast());
            Self(a, b)
        }
    }
}
// (Codec for TimeInterval is provided by derive; use the wire helper for stable transport in tests.)

#[cfg(feature = "transparent_api")]
impl EventFilter for TimeEventFilter {
    type Event = TimeEvent;

    /// Isn't useful for time-triggers
    fn matches(&self, event: &TimeEvent) -> bool {
        self.count_matches(event) > 0
    }

    fn count_matches(&self, event: &TimeEvent) -> u32 {
        match &self.0 {
            ExecutionTime::PreCommit => 1,
            ExecutionTime::Schedule(schedule) => {
                count_matches_in_interval(schedule, &event.interval)
            }
        }
    }

    fn mintable(&self) -> bool {
        !matches!(
            self.0,
            ExecutionTime::Schedule(Schedule {
                period_ms: None,
                ..
            })
        )
    }
}

/// Count something with the `schedule` within the `interval`
#[cfg(feature = "transparent_api")]
fn count_matches_in_interval(schedule: &Schedule, interval: &TimeInterval) -> u32 {
    schedule.period().map_or_else(
        || {
            // One-shot schedule: include if start lies within [since, to) (right-open)
            // Align semantics with periodic matching (which uses Range::contains and is end-exclusive)
            let start = schedule.start();
            let since = interval.since();
            let end = since + interval.length();
            u32::from(start >= since && start < end)
        },
        |period| {
            if period.is_zero() {
                // Zero period is invalid; treat as non-matching to avoid panicking.
                return 0;
            }
            #[allow(clippy::integer_division)]
            let k = interval
                .since()
                .saturating_sub(schedule.start())
                .as_millis()
                / period.as_millis();
            let start = schedule.start() + multiply_duration_by_u128(period, k);
            let range = Range::from(*interval);
            (0..)
                .map(|i| start + period * i)
                .skip_while(|time| *time < interval.since())
                .take_while(|time| range.contains(time))
                .count()
                .try_into()
                .expect("Overflow. The schedule is too frequent relative to the interval length")
        },
    )
}

/// Multiply `duration` by `n`
///
/// Usage of this function allows to operate with much longer time *intervals*
/// with much less *periods* than just `impl Mul<u32> for Duration` does
///
/// # Panics
/// Panics if resulting number in seconds can't be represented as `u64`
#[cfg(feature = "transparent_api")]
fn multiply_duration_by_u128(duration: Duration, n: u128) -> Duration {
    if let Ok(n) = u32::try_from(n) {
        return duration * n;
    }

    let new_ms = duration.as_millis() * n;
    if let Ok(ms) = u64::try_from(new_ms) {
        return Duration::from_millis(ms);
    }

    #[allow(clippy::integer_division)]
    let new_secs = u64::try_from(new_ms / 1000)
        .expect("Overflow. Resulting number in seconds can't be represented as `u64`");
    Duration::from_secs(new_secs)
}

impl Schedule {
    /// Create new [`Schedule`] starting at `start` and without period
    #[must_use]
    #[inline]
    pub fn starting_at(start: Duration) -> Self {
        Self {
            start_ms: start
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX"),
            period_ms: None,
        }
    }

    /// Add `period` to `self`
    #[must_use]
    #[inline]
    pub fn with_period(mut self, period: Duration) -> Self {
        self.period_ms = Some(
            period
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX"),
        );
        self
    }

    /// Instant of the first execution
    pub fn start(&self) -> Duration {
        Duration::from_millis(self.start_ms)
    }

    /// Period of repeated executions
    pub fn period(&self) -> Option<Duration> {
        self.period_ms.map(Duration::from_millis)
    }
}

#[cfg(feature = "json")]
fn write_key(out: &mut String, key: &str) {
    out.push('"');
    out.push_str(key);
    out.push('"');
    out.push(':');
}

#[cfg(feature = "json")]
fn expect_u64(field: &str, value: &norito::json::Value) -> Result<u64, norito::json::Error> {
    if let norito::json::Value::Number(num) = value {
        num.as_u64()
            .ok_or_else(|| norito::json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected unsigned integer"),
            })
    } else {
        Err(norito::json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("expected unsigned integer"),
        })
    }
}

#[cfg(feature = "json")]
fn parse_value_as<T>(value: &norito::json::Value) -> Result<T, norito::json::Error>
where
    T: norito::json::JsonDeserialize,
{
    let json =
        norito::json::to_json(value).map_err(|e| norito::json::Error::Message(e.to_string()))?;
    let mut parser = norito::json::Parser::new(&json);
    T::json_deserialize(&mut parser)
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for TimeEvent {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        write_key(out, "interval");
        norito::json::JsonSerialize::json_serialize(&self.interval, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TimeEvent {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let mut map = match value {
            norito::json::Value::Object(map) => map,
            _ => {
                return Err(norito::json::Error::InvalidField {
                    field: "TimeEvent".into(),
                    message: String::from("expected object"),
                });
            }
        };
        let interval_value =
            map.remove("interval")
                .ok_or_else(|| norito::json::Error::MissingField {
                    field: "interval".into(),
                })?;
        if let Some((field, _)) = map.into_iter().next() {
            return Err(norito::json::Error::UnknownField { field });
        }
        let interval = parse_value_as::<TimeInterval>(&interval_value)?;
        Ok(Self { interval })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for TimeEventFilter {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TimeEventFilter {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        ExecutionTime::json_deserialize(parser).map(TimeEventFilter)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ExecutionTime {
    fn write_json(&self, out: &mut String) {
        match self {
            ExecutionTime::PreCommit => {
                norito::json::JsonSerialize::json_serialize("PreCommit", out);
            }
            ExecutionTime::Schedule(schedule) => {
                out.push('{');
                write_key(out, "Schedule");
                norito::json::JsonSerialize::json_serialize(schedule, out);
                out.push('}');
            }
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ExecutionTime {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        match value {
            norito::json::Value::String(s) => {
                if s == "PreCommit" {
                    Ok(ExecutionTime::PreCommit)
                } else {
                    Err(norito::json::Error::InvalidField {
                        field: "ExecutionTime".into(),
                        message: String::from("unknown variant"),
                    })
                }
            }
            norito::json::Value::Object(map) => {
                if map.len() != 1 {
                    return Err(norito::json::Error::InvalidField {
                        field: "ExecutionTime".into(),
                        message: String::from("expected single-key object"),
                    });
                }
                let mut iter = map.into_iter();
                let (variant, inner) = iter.next().unwrap();
                if iter.next().is_some() {
                    return Err(norito::json::Error::InvalidField {
                        field: "ExecutionTime".into(),
                        message: String::from("expected single-key object"),
                    });
                }
                match variant.as_str() {
                    "Schedule" => Ok(ExecutionTime::Schedule(parse_value_as(&inner)?)),
                    other => Err(norito::json::Error::UnknownField {
                        field: other.to_owned(),
                    }),
                }
            }
            _ => Err(norito::json::Error::InvalidField {
                field: "ExecutionTime".into(),
                message: String::from("expected string or object"),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Schedule {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        write_key(out, "start_ms");
        norito::json::JsonSerialize::json_serialize(&self.start_ms, out);
        out.push(',');
        write_key(out, "period_ms");
        norito::json::JsonSerialize::json_serialize(&self.period_ms, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Schedule {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let map = match value {
            norito::json::Value::Object(map) => map,
            _ => {
                return Err(norito::json::Error::InvalidField {
                    field: "Schedule".into(),
                    message: String::from("expected object"),
                });
            }
        };
        let mut start_ms = None;
        let mut period_ms = None;
        for (key, val) in map {
            match key.as_str() {
                "start_ms" => start_ms = Some(expect_u64(&key, &val)?),
                "period_ms" => {
                    period_ms = Some(match val {
                        norito::json::Value::Null => None,
                        other => Some(expect_u64(&key, &other)?),
                    })
                }
                other => {
                    return Err(norito::json::Error::UnknownField {
                        field: other.to_owned(),
                    });
                }
            }
        }
        let start_ms = start_ms.ok_or_else(|| norito::json::Error::MissingField {
            field: "start_ms".into(),
        })?;
        let period_ms = period_ms.unwrap_or(None);
        Ok(Self {
            start_ms,
            period_ms,
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for TimeInterval {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        write_key(out, "since_ms");
        norito::json::JsonSerialize::json_serialize(&self.since_ms, out);
        out.push(',');
        write_key(out, "length_ms");
        norito::json::JsonSerialize::json_serialize(&self.length_ms, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TimeInterval {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let map = match value {
            norito::json::Value::Object(map) => map,
            _ => {
                return Err(norito::json::Error::InvalidField {
                    field: "TimeInterval".into(),
                    message: String::from("expected object"),
                });
            }
        };
        let mut since_ms = None;
        let mut length_ms = None;
        for (key, val) in map {
            match key.as_str() {
                "since_ms" => since_ms = Some(expect_u64(&key, &val)?),
                "length_ms" => length_ms = Some(expect_u64(&key, &val)?),
                other => {
                    return Err(norito::json::Error::UnknownField {
                        field: other.to_owned(),
                    });
                }
            }
        }
        let since_ms = since_ms.ok_or_else(|| norito::json::Error::MissingField {
            field: "since_ms".into(),
        })?;
        let length_ms = length_ms.ok_or_else(|| norito::json::Error::MissingField {
            field: "length_ms".into(),
        })?;
        Ok(Self {
            since_ms,
            length_ms,
        })
    }
}

impl TimeInterval {
    /// Create new [`Self`]
    pub fn new(since: Duration, length: Duration) -> Self {
        Self {
            since_ms: since
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX"),
            length_ms: length
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX"),
        }
    }

    /// Create [`Self`] from since and to points
    pub fn new_since_to(since: Duration, to: Duration) -> Self {
        let length = to
            .checked_sub(since)
            .expect("time interval end must be after start");
        Self::new(since, length)
    }

    /// Instant of the previous execution
    pub fn since(&self) -> Duration {
        Duration::from_millis(self.since_ms)
    }

    /// Time since the previous execution
    pub fn length(&self) -> Duration {
        Duration::from_millis(self.length_ms)
    }
}

impl From<TimeInterval> for Range<Duration> {
    #[inline]
    fn from(interval: TimeInterval) -> Self {
        interval.since()..interval.since() + interval.length()
    }
}

/// Exports common structs and enums from this module.
pub mod prelude {
    pub use super::{
        ExecutionTime, Schedule as TimeSchedule, TimeEvent, TimeEventFilter, TimeInterval,
    };
}

#[cfg(test)]
mod json_tests {
    use super::*;

    #[cfg(feature = "json")]
    #[test]
    fn json_roundtrip() {
        let interval = TimeInterval {
            since_ms: 1_000,
            length_ms: 2_000,
        };
        let json = norito::json::to_json(&interval).expect("serialize");
        assert_eq!(json, r#"{"since_ms":1000,"length_ms":2000}"#);

        let deserialized: TimeInterval = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, interval);
    }

    #[test]
    fn codec_roundtrip() {
        // Use header-framed Norito path; headers now include hybrid packed-struct bits.
        let interval = TimeInterval {
            since_ms: 10,
            length_ms: 20,
        };
        let bytes = norito::to_bytes(&wire::TimeIntervalWire::from(interval)).expect("encode");
        let archived = norito::from_bytes::<wire::TimeIntervalWire>(&bytes).expect("archived");
        let decoded_w = norito::core::NoritoDeserialize::deserialize(archived);
        let decoded: TimeInterval = decoded_w.into();
        assert_eq!(decoded, interval);
    }

    #[test]
    fn ti_roundtrip_diagnostics() {
        use norito::codec::{Decode, Encode};

        fn hex_prefix(bytes: &[u8], n: usize) -> String {
            let mut s = String::new();
            for b in bytes.iter().take(n) {
                use std::fmt::Write as _;
                let _ = write!(&mut s, "{b:02X}");
            }
            s
        }

        let interval = TimeInterval {
            since_ms: 10,
            length_ms: 20,
        };

        // Bare codec encode/decode
        let bare = interval.encode();
        let mut cur = bare.as_slice();
        let decoded_bare = TimeInterval::decode(&mut cur).unwrap_or(TimeInterval {
            since_ms: 0,
            length_ms: 0,
        });

        // Header-framed encode/decode
        let hdr = norito::to_bytes(&wire::TimeIntervalWire::from(interval)).expect("to_bytes");
        let archived = norito::from_bytes::<wire::TimeIntervalWire>(&hdr).expect("from_bytes");
        let decoded_hdr_w = norito::core::NoritoDeserialize::deserialize(archived);
        let decoded_hdr: TimeInterval = decoded_hdr_w.into();

        eprintln!("TI bare len={} hdr len={}", bare.len(), hdr.len());
        eprintln!("bare[0..32] = {}", hex_prefix(&bare, 32));
        eprintln!("hdr[0..32]  = {}", hex_prefix(&hdr, 32));
        eprintln!(
            "decoded: bare=({}, {}), hdr=({}, {})",
            decoded_bare.since_ms,
            decoded_bare.length_ms,
            decoded_hdr.since_ms,
            decoded_hdr.length_ms
        );
    }
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use super::*;

    /// Sample timestamp
    const TIMESTAMP: u64 = 1_647_443_386;

    /// Tests for `count_matches_in_interval()`
    mod count_matches_in_interval {
        use super::*;

        #[test]
        fn test_no_period_before_left_border() {
            // ----|-----[-----)-------
            //     p    i1     i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP - 5));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(10);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 0);
        }

        #[test]
        fn test_no_period_on_left_border() {
            //     |
            // ----[---------)------
            //   p, i1      i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(10);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 1);
        }

        #[test]
        fn test_no_period_inside() {
            // ----[------|-----)----
            //     i1     p    i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP + 5));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(10);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 1);
        }

        #[test]
        fn test_no_period_on_right_border() {
            //               |
            // ----[---------)------
            //    i1      i2, p

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP + 10));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(10);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 0);
        }

        #[test]
        fn test_jump_over_inside() {
            // ----[------|-----)----*----
            //     i1     p    i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP + 5))
                .with_period(Duration::from_secs(30));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(10);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 1);
        }

        #[test]
        fn test_jump_over_outside() {
            // ----|------[-----)----*----
            //     p     i1    i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(10));
            let since = Duration::from_secs(TIMESTAMP + 35);
            let length = Duration::from_secs(4);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 0);
        }

        #[test]
        fn test_interval_on_the_left() {
            // ----[----)----|-----*-----*----
            //     i1   i2   p

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(6));
            let since = Duration::from_secs(TIMESTAMP - 10);
            let length = Duration::from_secs(4);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 0);
        }

        #[test]
        fn test_schedule_starts_at_the_middle() {
            // ----[------|----*----*----*--)-*----
            //     i1     p                i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(6));
            let since = Duration::from_secs(TIMESTAMP - 10);
            let length = Duration::from_secs(30);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 4);
        }

        #[test]
        fn test_interval_on_the_right() {
            // ----|----*--[----*----*----*----*----)----*----
            //     p      i1                       i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_millis(600));
            let since = Duration::from_secs(TIMESTAMP + 3) + Duration::from_millis(500);
            let length = Duration::from_secs(2);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 4);
        }

        #[test]
        fn test_only_left_border() {
            //             *
            // ----|-------[----)--*-------*--
            //     p      i1   i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP - 10))
                .with_period(Duration::from_secs(10));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(5);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 1);
        }

        #[test]
        fn test_only_right_border_inside() {
            //               *
            // ----[----|----)----*----*----
            //     i1   p    i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(5));
            let since = Duration::from_secs(TIMESTAMP - 10);
            let length = Duration::from_secs(15);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 1);
        }

        #[test]
        fn test_only_right_border_outside() {
            //              *
            // ----|---[----)--------*----
            //     p   i1   i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP - 10))
                .with_period(Duration::from_secs(15));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(5);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 0);
        }

        #[test]
        fn test_matches_right_border_and_ignores_left() {
            //     |             *
            // ----[-*-*-*-*-*-*-)-*-*-*
            //   p, i1           i2

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(1));
            let since = Duration::from_secs(TIMESTAMP);
            let length = Duration::from_secs(7);
            let interval = TimeInterval::new(since, length);
            assert_eq!(count_matches_in_interval(&schedule, &interval), 7);
        }
    }

    // Tests for [`TimeEventFilter`]
    mod time_event_filter {
        use super::*;

        #[test]
        fn test_schedule_start_before_interval() {
            //
            // ----|---[--*----*--)--*------
            //     s   a          b

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(10));
            let filter = TimeEventFilter(ExecutionTime::Schedule(schedule));

            let a = Duration::from_secs(TIMESTAMP + 5);
            let b = Duration::from_secs(TIMESTAMP + 25);
            let interval = TimeInterval::new_since_to(a, b);

            let event = TimeEvent { interval };

            assert_eq!(filter.count_matches(&event), 2);
        }

        #[test]
        fn test_schedule_start_inside_interval() {
            //
            // -------[--|-----*--)--*------
            //        a  s        b

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP + 5))
                .with_period(Duration::from_secs(10));
            let filter = TimeEventFilter(ExecutionTime::Schedule(schedule));

            let a = Duration::from_secs(TIMESTAMP);
            let b = Duration::from_secs(TIMESTAMP + 20);
            let interval = TimeInterval::new_since_to(a, b);

            let event = TimeEvent { interval };

            assert_eq!(filter.count_matches(&event), 2);
        }

        #[test]
        fn test_schedule_start_after_interval() {
            //
            // -------[--------)----|--
            //        a        b    s

            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP + 35))
                .with_period(Duration::from_secs(10));
            let filter = TimeEventFilter(ExecutionTime::Schedule(schedule));

            let a = Duration::from_secs(TIMESTAMP);
            let b = Duration::from_secs(TIMESTAMP + 20);
            let interval = TimeInterval::new_since_to(a, b);

            let event = TimeEvent { interval };

            assert_eq!(filter.count_matches(&event), 0);
        }

        #[test]
        fn zero_period_schedule_never_matches() {
            let schedule = Schedule::starting_at(Duration::from_secs(TIMESTAMP))
                .with_period(Duration::from_secs(0));
            let filter = TimeEventFilter(ExecutionTime::Schedule(schedule));

            let interval = TimeInterval::new_since_to(
                Duration::from_secs(TIMESTAMP),
                Duration::from_secs(TIMESTAMP + 10),
            );
            let event = TimeEvent { interval };

            assert_eq!(filter.count_matches(&event), 0);
        }
    }
}
