//! Module with telemetry layer for tracing

use std::{error::Error, fmt::Debug};

use derive_more::{Deref, DerefMut};
use iroha_data_model::nexus::{DataSpaceId, LaneId};
use norito::json::{Value, native::Map as JsonMap};
use tokio::sync::mpsc;
use tracing::{
    Event as TracingEvent, Subscriber,
    field::{Field, Visit},
};

use crate::layer::{EventInspectorTrait, EventSubscriber};

/// Target for telemetry in `tracing`
pub const TARGET_PREFIX: &str = "telemetry::";
/// Target for telemetry future in `tracing`
pub const FUTURE_TARGET_PREFIX: &str = "telemetry_future::";

/// Placeholder emitted when telemetry fields are redacted.
pub const REDACTED_PLACEHOLDER: &str = "[REDACTED]";
/// Suffix appended to truncated string payloads.
pub const TRUNCATION_SUFFIX: &str = "...(truncated)";
/// Maximum allowed string length for telemetry fields before truncation.
pub const MAX_FIELD_LENGTH: usize = 2048;

// Keywords that signal sensitive payloads. The list intentionally errs on the
// side of caution; matching fields are redacted even if that may hide
// non-secret diagnostic data.
const SENSITIVE_FIELD_KEYWORDS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "access_token",
    "refresh_token",
    "session_token",
    "api_key",
    "apikey",
    "private_key",
    "privkey",
    "mnemonic",
    "seed",
];

/// Fields for telemetry (type for efficient saving)
#[derive(Clone, Debug, PartialEq, Eq, Default, Deref, DerefMut)]
pub struct Fields(pub Vec<(&'static str, Value)>);

impl Fields {
    #[inline]
    fn push_sanitized(&mut self, name: &'static str, value: Value) {
        self.0.push((name, sanitize_value(name, value)));
    }
}

impl From<Fields> for Value {
    fn from(Fields(fields): Fields) -> Self {
        let mut map = JsonMap::new();
        for (key, value) in fields {
            map.insert(key.to_owned(), value);
        }
        Value::Object(map)
    }
}

/// Telemetry which can be received from telemetry layer
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Subsystem from which telemetry was received
    pub target: &'static str,
    /// Fields which was recorded
    pub fields: Fields,
}

impl Visit for Event {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.fields
            .push_sanitized(field.name(), format!("{:?}", &value).into())
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push_sanitized(field.name(), value.into())
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push_sanitized(field.name(), value.into())
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push_sanitized(field.name(), value.into())
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push_sanitized(field.name(), value.into())
    }

    fn record_error(&mut self, field: &Field, mut error: &(dyn Error + 'static)) {
        let mut vec = vec![error.to_string()];
        while let Some(inner) = error.source() {
            error = inner;
            vec.push(inner.to_string())
        }
        let entries = vec.into_iter().map(Value::from).collect();
        self.fields
            .push_sanitized(field.name(), Value::Array(entries))
    }
}

impl Event {
    fn from_event(target: &'static str, event: &TracingEvent<'_>) -> Self {
        let fields = Fields::default();
        let mut telemetry = Self { target, fields };
        // Include event level from metadata to enable downstream filtering
        let level = event.metadata().level().to_string();
        telemetry.fields.push_sanitized("level", Value::from(level));
        if !telemetry.fields.iter().any(|(key, _)| *key == "lane_id") {
            telemetry
                .fields
                .push_sanitized("lane_id", Value::from(u64::from(LaneId::SINGLE.as_u32())));
        }
        if !telemetry
            .fields
            .iter()
            .any(|(key, _)| *key == "dataspace_id")
        {
            telemetry
                .fields
                .push_sanitized("dataspace_id", Value::from(DataSpaceId::GLOBAL.as_u64()));
        }
        event.record(&mut telemetry);
        telemetry
    }
}

/// Telemetry layer
#[derive(Debug, Clone)]
pub struct Layer<S: Subscriber> {
    sender: mpsc::Sender<ChannelEvent>,
    subscriber: S,
}

impl<S: Subscriber> Layer<S> {
    /// Create new telemetry layer with specific channel size
    #[allow(clippy::new_ret_no_self)]
    pub fn with_capacity(
        subscriber: S,
        channel_size: usize,
    ) -> (impl Subscriber, mpsc::Receiver<ChannelEvent>) {
        let (sender, receiver) = mpsc::channel(channel_size);
        let telemetry = EventSubscriber(Self { sender, subscriber });
        (telemetry, receiver)
    }

    fn send_event(&self, channel: Channel, target: &'static str, event: &TracingEvent<'_>) {
        let _ = self
            .sender
            .try_send(ChannelEvent(channel, Event::from_event(target, event)));
    }
}

impl<S: Subscriber> EventInspectorTrait for Layer<S> {
    type Subscriber = S;

    fn inner_subscriber(&self) -> &Self::Subscriber {
        &self.subscriber
    }

    fn event(&self, event: &TracingEvent<'_>) {
        let target = event.metadata().target();
        #[allow(clippy::option_if_let_else)] // This is actually more readable.
        if let Some(target) = target.strip_prefix(TARGET_PREFIX) {
            self.send_event(Channel::Regular, target, event);
        } else if let Some(target) = target.strip_prefix(FUTURE_TARGET_PREFIX) {
            self.send_event(Channel::Future, target, event);
        } else {
            self.subscriber.event(event)
        }
    }
}

#[inline]
fn sanitize_value(field_name: &str, value: Value) -> Value {
    if should_redact(field_name) {
        return Value::from(REDACTED_PLACEHOLDER);
    }

    match value {
        Value::String(mut raw) => {
            if raw.len() > MAX_FIELD_LENGTH {
                let keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
                raw.truncate(keep);
                raw.push_str(TRUNCATION_SUFFIX);
            }
            Value::String(raw)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|inner| sanitize_value(field_name, inner))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let sanitized = sanitize_value(&k, v);
                    (k, sanitized)
                })
                .collect(),
        ),
        other => other,
    }
}

#[inline]
fn should_redact(field_name: &str) -> bool {
    if cfg!(feature = "log-obfuscation") {
        is_sensitive_field(field_name)
    } else {
        let _ = field_name;
        false
    }
}

fn is_sensitive_field(field_name: &str) -> bool {
    let mut normalized = String::with_capacity(field_name.len());
    for ch in field_name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else {
            normalized.push('_');
        }
    }

    if SENSITIVE_FIELD_KEYWORDS
        .iter()
        .any(|keyword| normalized == *keyword)
    {
        return true;
    }

    normalized
        .split('_')
        .filter(|segment| !segment.is_empty())
        .any(|segment| SENSITIVE_FIELD_KEYWORDS.contains(&segment))
}

/// A pair of [`Channel`] associated with [`Event`]
pub struct ChannelEvent(pub Channel, pub Event);

/// Supported telemetry channels
#[derive(Copy, Clone)]
pub enum Channel {
    /// Regular telemetry
    Regular,
    /// Telemetry collected from futures instrumented with `iroha_futures::TelemetryFuture`.
    Future,
}

#[cfg(test)]
mod tests {
    use norito::json::{self, Value};

    use super::*;

    #[test]
    fn conditionally_redacts_sensitive_fields() {
        let value = sanitize_value("password", Value::from("super-secret"));
        let direct = sanitize_value("access_token", Value::from("token"));

        if cfg!(feature = "log-obfuscation") {
            assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
            assert_eq!(direct, Value::from(REDACTED_PLACEHOLDER));
        } else {
            assert_eq!(value, Value::from("super-secret"));
            assert_eq!(direct, Value::from("token"));
        }
    }

    #[test]
    fn truncates_oversized_strings() {
        let payload = "x".repeat(MAX_FIELD_LENGTH + 64);
        let sanitized = sanitize_value("payload", Value::from(payload.clone()));

        let Value::String(output) = sanitized else {
            panic!("sanitized value is not a string");
        };

        assert_eq!(output.len(), MAX_FIELD_LENGTH);
        assert!(output.ends_with(TRUNCATION_SUFFIX));

        let keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
        assert_eq!(&output[..keep], &payload[..keep]);
    }

    #[test]
    fn sanitizes_nested_structures() {
        let nested = json::object([("token", Value::from("abc")), ("note", Value::from("ok"))])
            .expect("construct nested object");

        let sanitized = sanitize_value("wrapper", nested);
        let Value::Object(mut map) = sanitized else {
            panic!("expected object after sanitization");
        };

        let token_entry = map.remove("token");
        if cfg!(feature = "log-obfuscation") {
            assert_eq!(token_entry, Some(Value::from(REDACTED_PLACEHOLDER)));
        } else {
            assert_eq!(token_entry, Some(Value::from("abc")));
        }

        assert_eq!(map.remove("note"), Some(Value::from("ok")));
    }

    #[test]
    fn leaves_non_sensitive_values_intact() {
        let before = json::object([
            ("count", Value::Number(10_u64.into())),
            ("status", Value::from("ready")),
        ])
        .expect("construct metrics object");
        let sanitized = sanitize_value("metrics", before.clone());
        assert_eq!(sanitized, before);

        assert!(!is_sensitive_field("metrics"));
    }
}
