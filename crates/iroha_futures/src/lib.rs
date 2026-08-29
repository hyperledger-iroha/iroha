//! Crate with various Iroha futures
pub mod supervisor;
pub use iroha_derive::telemetry_future;
use iroha_logger::telemetry::{Event as Telemetry, Fields as TelemetryFields};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
    json::Value,
};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
    time::Instant,
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
        let id = NEXT_TELEMETRY_FUTURE_ID.fetch_add(1, Ordering::Relaxed);
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
const ID: &str = "id";
const NAME: &str = "name";
const DURATION: &str = "duration";
static NEXT_TELEMETRY_FUTURE_ID: AtomicU64 = AtomicU64::new(1);
/// Telemetry conversion error
#[derive(Debug, Clone, Copy)]
pub struct TelemetryConversionError;
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
                    id = Some(id_value.as_u64().ok_or(TelemetryConversionError)?)
                }
                (NAME, Value::String(name_value)) if name.is_none() => name = Some(name_value),
                (DURATION, Value::Number(duration_value)) if duration.is_none() => {
                    duration = Some(duration_value.as_u64().ok_or(TelemetryConversionError)?)
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
    use super::*;
    use iroha_logger::telemetry::{Event as Telemetry, Fields as TelemetryFields};
    use norito::json::Value;
    fn telemetry_event(id: Value, duration: Value) -> Telemetry {
        Telemetry {
            target: "iroha_futures",
            fields: TelemetryFields(vec![
                ("id", id),
                ("name", Value::from("basic::sleep")),
                ("duration", duration),
            ]),
        }
    }
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
    }
    #[test]
    fn telemetry_future_ids_are_monotonic() {
        let first = TelemetryFuture::new(async {}, "first");
        let second = TelemetryFuture::new(async {}, "second");
        assert_eq!(second.id, first.id + 1);
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
    #[test]
    fn owned_future_poll_telemetry_rejects_non_u64_numbers() {
        for event in [
            telemetry_event(Value::from(-1_i64), Value::from(123_u64)),
            telemetry_event(Value::from(42_u64), Value::from(0.5_f64)),
        ] {
            assert!(
                FuturePollTelemetry::try_from(event).is_err(),
                "negative or fractional numbers must return a conversion error"
            );
        }
    }
}
