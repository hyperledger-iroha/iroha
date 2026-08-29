//! Module with telemetry layer for tracing
use crate::layer::{EventInspectorTrait, EventSubscriber};
use derive_more::{Deref, DerefMut};
use iroha_data_model::nexus::{DataSpaceId, LaneId};
use norito::json::{Value, native::Map as JsonMap};
use std::{borrow::Cow, error::Error, fmt::Debug};
use tokio::sync::mpsc;
use tracing::{
    Event as TracingEvent, Subscriber,
    field::{Field, Visit},
};
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
/// Maximum number of linked error values retained in one telemetry field.
pub const MAX_ERROR_CHAIN_DEPTH: usize = 16;
/// Marker appended when an error source chain exceeds its bound.
pub const ERROR_CHAIN_TRUNCATION_MARKER: &str = "[error chain truncated]";
// Keywords that signal sensitive payloads. The list intentionally errs on the
// side of caution; matching fields are redacted even if that may hide
// non-secret diagnostic data.
const SENSITIVE_FIELD_KEYWORDS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "credential",
    "token",
    "access_token",
    "refresh_token",
    "session_token",
    "session",
    "authorization",
    "cookie",
    "jwt",
    "bearer",
    "api_key",
    "api_key_hash",
    "apikey",
    "bot_key",
    "private_key",
    "privkey",
    "signing_key",
    "mnemonic",
    "seed",
];
// Prefixes that explicitly mark a field as sensitive.
const EXPLICIT_REDACTION_PREFIXES: &[&str] = &["redact", "sensitive", "secret", "pii"];
/// Fields for telemetry (type for efficient saving)
#[derive(Clone, Debug, PartialEq, Eq, Default, Deref, DerefMut)]
pub struct Fields(pub Vec<(&'static str, Value)>);
impl Fields {
    #[inline]
    fn push_sanitized(&mut self, name: &'static str, value: Value) {
        self.push_sanitized_with(name, || value);
    }

    #[inline]
    fn push_sanitized_with(&mut self, name: &'static str, value: impl FnOnce() -> Value) {
        let value = if is_normalized_sensitive_field(&normalized_field_name(name)) {
            Value::from(REDACTED_PLACEHOLDER)
        } else {
            sanitize_non_sensitive_value(name, value())
        };
        self.0.push((name, value));
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
            .push_sanitized_with(field.name(), || format!("{value:?}").into())
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
        self.fields
            .push_sanitized_with(field.name(), || value.into())
    }
    fn record_error(&mut self, field: &Field, error: &(dyn Error + 'static)) {
        self.fields
            .push_sanitized_with(field.name(), || Value::Array(bounded_error_chain(error)))
    }
}

fn bounded_error_chain(mut error: &(dyn Error + 'static)) -> Vec<Value> {
    let mut values = Vec::with_capacity(MAX_ERROR_CHAIN_DEPTH.saturating_add(1));
    for depth in 0..MAX_ERROR_CHAIN_DEPTH {
        values.push(Value::from(error.to_string()));
        let Some(inner) = error.source() else {
            break;
        };
        if depth + 1 == MAX_ERROR_CHAIN_DEPTH {
            values.push(Value::from(ERROR_CHAIN_TRUNCATION_MARKER));
            break;
        }
        error = inner;
    }
    values
}

impl Event {
    fn from_event(target: &'static str, event: &TracingEvent<'_>) -> Self {
        let fields = Fields::default();
        let mut telemetry = Self { target, fields };
        // Include event level from metadata to enable downstream filtering
        let level = event.metadata().level().to_string();
        telemetry.fields.push_sanitized("level", Value::from(level));
        event.record(&mut telemetry);
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
                .push_sanitized("dataspace_id", Value::from(DataSpaceId::UNIVERSAL.as_u64()));
        }
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
        let Ok(permit) = self.sender.try_reserve() else {
            return;
        };
        permit.send(ChannelEvent(channel, Event::from_event(target, event)));
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
    if is_normalized_sensitive_field(&normalized_field_name(field_name)) {
        return Value::from(REDACTED_PLACEHOLDER);
    }
    sanitize_non_sensitive_value(field_name, value)
}

fn sanitize_non_sensitive_value(field_name: &str, value: Value) -> Value {
    match value {
        Value::String(mut raw) => {
            if raw.len() > MAX_FIELD_LENGTH {
                let mut keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
                while !raw.is_char_boundary(keep) {
                    keep = keep.saturating_sub(1);
                }
                raw.truncate(keep);
                raw.push_str(TRUNCATION_SUFFIX);
            }
            Value::String(raw)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|inner| sanitize_non_sensitive_value(field_name, inner))
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
fn is_normalized_sensitive_field(normalized: &str) -> bool {
    if normalized.is_empty() {
        return false;
    }
    let mut segments = normalized.split('_').filter(|segment| !segment.is_empty());
    let Some(first) = segments.next() else {
        return false;
    };
    if EXPLICIT_REDACTION_PREFIXES.contains(&first) {
        return true;
    }
    SENSITIVE_FIELD_KEYWORDS
        .iter()
        .any(|keyword| contains_delimited_keyword(normalized, keyword))
}

fn contains_delimited_keyword(normalized: &str, keyword: &str) -> bool {
    normalized.match_indices(keyword).any(|(start, matched)| {
        let end = start + matched.len();
        (start == 0 || normalized.as_bytes()[start - 1] == b'_')
            && (end == normalized.len() || normalized.as_bytes()[end] == b'_')
    })
}
#[cfg(test)]
fn is_sensitive_field(field_name: &str) -> bool {
    is_normalized_sensitive_field(&normalized_field_name(field_name))
}
fn normalized_field_name(field_name: &str) -> Cow<'_, str> {
    let mut previous_underscore = false;
    let already_normalized = field_name.bytes().all(|byte| {
        let accepted = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_';
        let duplicate_underscore = byte == b'_' && previous_underscore;
        previous_underscore = byte == b'_';
        accepted && !duplicate_underscore
    });
    if already_normalized {
        Cow::Borrowed(field_name)
    } else {
        Cow::Owned(normalize_field_name(field_name))
    }
}
fn normalize_field_name(field_name: &str) -> String {
    let mut normalized = String::with_capacity(field_name.len() + 4);
    let mut chars = field_name.chars().peekable();
    let mut previous = None;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_alphanumeric() {
            let is_upper = ch.is_ascii_uppercase();
            if is_upper {
                let prev_is_alnum = previous.is_some_and(|prev: char| prev.is_ascii_alphanumeric());
                let prev_is_upper = previous.is_some_and(|prev: char| prev.is_ascii_uppercase());
                let next_is_lower = chars.peek().is_some_and(|next| next.is_ascii_lowercase());
                if ((prev_is_alnum && !prev_is_upper) || (prev_is_upper && next_is_lower))
                    && !normalized.ends_with('_')
                {
                    normalized.push('_');
                }
                normalized.push(ch.to_ascii_lowercase());
            } else {
                normalized.push(ch.to_ascii_lowercase());
            }
        } else if !normalized.ends_with('_') {
            normalized.push('_');
        }
        previous = Some(ch);
    }
    normalized
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
    use super::*;
    use norito::json::{self, Value};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[test]
    fn redacts_sensitive_fields_by_default() {
        let value = sanitize_value("password", Value::from("super-secret"));
        let direct = sanitize_value("accessToken", Value::from("token"));
        assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
        assert_eq!(direct, Value::from(REDACTED_PLACEHOLDER));
    }
    #[test]
    fn redaction_skips_sensitive_value_construction() {
        let constructions = AtomicUsize::new(0);
        let mut fields = Fields::default();
        fields.push_sanitized_with("private_key", || {
            constructions.fetch_add(1, Ordering::Relaxed);
            Value::from("must not be constructed")
        });
        assert_eq!(constructions.load(Ordering::Relaxed), 0);
        assert_eq!(
            fields.0,
            vec![("private_key", Value::from(REDACTED_PLACEHOLDER))]
        );
    }
    #[test]
    fn normalize_redaction_field_handles_camel_case() {
        assert_eq!(normalize_field_name("ApiKeyHash"), "api_key_hash");
        assert_eq!(normalize_field_name("apiKeyHash"), "api_key_hash");
        assert_eq!(normalize_field_name("api_key_hash"), "api_key_hash");
        assert_eq!(normalize_field_name("APIKey"), "api_key");
    }
    #[test]
    fn normalized_field_names_borrow_the_common_snake_case_path() {
        assert!(matches!(
            normalized_field_name("access_token"),
            Cow::Borrowed("access_token")
        ));
        assert!(matches!(normalized_field_name("APIKey"), Cow::Owned(_)));
        assert_eq!(normalized_field_name("api__key"), "api_key");
    }
    #[test]
    fn explicit_markers_force_redaction() {
        let value = sanitize_value("sensitive_payload", Value::from("data"));
        assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
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
    fn truncates_oversized_utf8_at_a_character_boundary() {
        let keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
        let payload = format!("{}😀{}", "x".repeat(keep - 1), "y".repeat(64));
        let sanitized = sanitize_value("payload", Value::from(payload));
        let Value::String(output) = sanitized else {
            panic!("sanitized value is not a string");
        };
        assert!(output.len() <= MAX_FIELD_LENGTH);
        assert!(output.ends_with(TRUNCATION_SUFFIX));
        assert_eq!(output.matches('😀').count(), 0);
    }
    #[test]
    fn bounds_cyclic_error_source_chains() {
        #[derive(Debug)]
        struct CyclicError;

        impl core::fmt::Display for CyclicError {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("cyclic")
            }
        }

        impl std::error::Error for CyclicError {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(self)
            }
        }

        let values = bounded_error_chain(&CyclicError);
        assert_eq!(values.len(), MAX_ERROR_CHAIN_DEPTH + 1);
        assert_eq!(
            values.last(),
            Some(&Value::from(ERROR_CHAIN_TRUNCATION_MARKER))
        );
    }
    #[test]
    fn sanitizes_nested_structures() {
        let nested = json::object([("token", Value::from("abc")), ("note", Value::from("ok"))])
            .expect("construct nested object");
        let sanitized = sanitize_value("wrapper", nested);
        let Value::Object(mut map) = sanitized else {
            panic!("expected object after sanitization");
        };
        assert_eq!(map.remove("token"), Some(Value::from(REDACTED_PLACEHOLDER)));
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
    #[test]
    fn detects_camel_case_keywords() {
        assert!(is_sensitive_field("refreshToken"));
        assert!(is_sensitive_field("APIKey"));
    }
    #[test]
    fn detects_compound_sensitive_keywords_at_segment_boundaries() {
        for field in [
            "validator_private_key",
            "service_api_key_value",
            "telegram_bot_key",
            "telemetry_signing_key_hex",
        ] {
            assert!(is_sensitive_field(field), "failed to classify {field}");
        }
        assert!(!is_sensitive_field("monkey_business"));
        assert!(!is_sensitive_field("tokenized_amount"));
    }
    #[test]
    fn full_channel_skips_event_formatting() {
        struct FormatCounter(Arc<AtomicUsize>);

        impl core::fmt::Debug for FormatCounter {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.0.fetch_add(1, Ordering::Relaxed);
                formatter.write_str("formatted")
            }
        }

        let (subscriber, _receiver) = Layer::with_capacity(tracing_subscriber::registry(), 1);
        let formats = Arc::new(AtomicUsize::new(0));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "telemetry::capacity", first = true);
            tracing::info!(
                target: "telemetry::capacity",
                expensive = ?FormatCounter(Arc::clone(&formats))
            );
        });
        assert_eq!(formats.load(Ordering::Relaxed), 0);
    }
}
