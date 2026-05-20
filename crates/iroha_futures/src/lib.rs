//! Crate with various Iroha futures

pub mod supervisor;

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

pub use iroha_derive::telemetry_future;
use iroha_logger::telemetry::{Event as Telemetry, Fields as TelemetryFields};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
    json::Value,
};

/// Future which sends info with telemetry about number and length of polls
#[derive(Debug, Clone, Copy)]
pub struct TelemetryFuture<F> {
    future: F,
    id: u64,
    name: &'static str,
}

impl<F> TelemetryFuture<F> {
    /// Constructor for future
    pub fn new(future: F, name: &'static str) -> Self {
        let id = rand::random();
        Self { future, id, name }
    }
}

/// Telemetry info for future polling
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize)]
pub struct FuturePollTelemetry {
    /// Future id
    pub id: u64,
    /// Future name
    pub name: String,
    /// Duration of poll encoded in nanoseconds
    pub duration: u64,
}

impl FuturePollTelemetry {
    /// Poll duration as a `Duration` value.
    #[inline]
    pub fn duration(&self) -> Duration {
        Duration::from_nanos(self.duration)
    }
}

const ID: &str = "id";
const NAME: &str = "name";
const DURATION: &str = "duration";

/// Telemetry conversion error
#[derive(Debug, Clone, Copy)]
pub struct TelemetryConversionError;

impl TryFrom<&Telemetry> for FuturePollTelemetry {
    type Error = TelemetryConversionError;

    fn try_from(
        Telemetry { target, fields }: &Telemetry,
    ) -> Result<Self, TelemetryConversionError> {
        if *target != "iroha_futures" {
            return Err(TelemetryConversionError);
        }

        let TelemetryFields(fields) = fields;
        let (mut id, mut name, mut duration) = (None, None, None);

        for field in fields {
            match field {
                (ID, Value::Number(id_value)) if id.is_none() => {
                    id = Some(id_value.as_u64().unwrap())
                }
                (NAME, Value::String(name_value)) if name.is_none() => name = Some(name_value),
                (DURATION, Value::Number(duration_value)) if duration.is_none() => {
                    duration = Some(duration_value.as_u64().unwrap())
                }
                _ => {}
            }
        }

        let (Some(id), Some(name), Some(duration)) = (id, name, duration) else {
            return Err(TelemetryConversionError);
        };

        Ok(Self {
            id,
            name: name.clone(),
            duration,
        })
    }
}

impl TryFrom<Telemetry> for FuturePollTelemetry {
    type Error = TelemetryConversionError;

    fn try_from(Telemetry { target, fields }: Telemetry) -> Result<Self, TelemetryConversionError> {
        if target != "iroha_futures" {
            return Err(TelemetryConversionError);
        }

        let TelemetryFields(fields) = fields;
        let (mut id, mut name, mut duration) = (None, None, None);

        for field in fields {
            match field {
                (ID, Value::Number(id_value)) if id.is_none() => {
                    id = Some(id_value.as_u64().unwrap())
                }
                (NAME, Value::String(name_value)) if name.is_none() => name = Some(name_value),
                (DURATION, Value::Number(duration_value)) if duration.is_none() => {
                    duration = Some(duration_value.as_u64().unwrap())
                }
                _ => {}
            }
        }

        let (Some(id), Some(name), Some(duration)) = (id, name, duration) else {
            return Err(TelemetryConversionError);
        };

        Ok(Self { id, name, duration })
    }
}

impl<F: Future> Future for TelemetryFuture<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let name = self.name;
        let id = self.id;
        let now = Instant::now();

        #[allow(unsafe_code)]
        // SAFETY: This is safe because `future` is a field of pinned structure and therefore is also pinned
        let future = unsafe { self.map_unchecked_mut(|telemetry| &mut telemetry.future) };
        let result = future.poll(cx);

        // 100 seconds in nanos is less than 2 ** 37. It would be more than enough for us
        #[allow(clippy::cast_possible_truncation)]
        let duration = now.elapsed().as_nanos() as u64;
        iroha_logger::telemetry_future!(id, name, duration);

        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iroha_logger::telemetry::{Event as Telemetry, Fields as TelemetryFields};
    use norito::json::Value;

    use super::*;
    #[test]
    fn future_poll_telemetry_json_roundtrip() {
        let sample = FuturePollTelemetry {
            id: 42,
            name: "test-future".into(),
            duration: 123_456,
        };

        let json = norito::json::to_json(&sample).expect("serialize telemetry");
        let decoded: FuturePollTelemetry =
            norito::json::from_json(&json).expect("deserialize telemetry");

        assert_eq!(decoded.id, sample.id);
        assert_eq!(decoded.name, sample.name);
        assert_eq!(decoded.duration, sample.duration);
        assert_eq!(decoded.duration(), Duration::from_nanos(sample.duration));
    }

    #[test]
    fn future_poll_telemetry_ignores_logger_enrichment_fields() {
        let event = Telemetry {
            target: "iroha_futures",
            fields: TelemetryFields(vec![
                ("level", Value::from("INFO")),
                ("lane_id", Value::Number(0_u64.into())),
                ("dataspace_id", Value::Number(0_u64.into())),
                ("id", Value::Number(42_u64.into())),
                ("name", Value::from("basic::sleep")),
                ("duration", Value::Number(123_u64.into())),
            ]),
        };

        let telemetry = FuturePollTelemetry::try_from(event).expect("convert future telemetry");
        assert_eq!(telemetry.id, 42);
        assert_eq!(telemetry.name, "basic::sleep");
        assert_eq!(telemetry.duration, 123);
    }
}
