//! Module with telemetry layer for tracing

use std::{
    collections::BTreeSet,
    error::Error,
    fmt::Debug,
    sync::{Arc, OnceLock, RwLock},
};

use derive_more::{Deref, DerefMut};
use iroha_config::parameters::actual::{TelemetryRedaction, TelemetryRedactionMode};
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
    "private_key",
    "privkey",
    "mnemonic",
    "seed",
];

// Prefixes that explicitly mark a field as sensitive (takes precedence over allow-list entries).
const EXPLICIT_REDACTION_PREFIXES: &[&str] = &["redact", "sensitive", "secret", "pii"];

/// Normalized field names explicitly allowed to bypass keyword redaction.
///
/// This list must remain short and documented in `docs/source/telemetry.md`.
pub const REDACTION_ALLOWLIST_POLICY: &[&str] = &[];

/// Compile-time support flag for telemetry redaction.
pub const REDACTION_SUPPORTED: bool = cfg!(feature = "log-obfuscation");

/// Redaction classification for telemetry fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionReason {
    /// Field was explicitly marked as sensitive.
    Explicit,
    /// Field matched a sensitive keyword.
    Keyword,
}

/// Reasons a redaction was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionSkipReason {
    /// Redaction disabled by configuration.
    Disabled,
    /// Field allow-listed by configuration.
    Allowlist,
    /// Redaction feature not compiled in.
    Unsupported,
}

/// Audit events emitted by the telemetry redaction layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionMetricEvent {
    /// A redaction was applied.
    Redacted {
        #[doc = "Reason for applying redaction."]
        reason: RedactionReason,
    },
    /// A redaction was skipped.
    Skipped {
        #[doc = "Reason for skipping redaction."]
        reason: RedactionSkipReason,
    },
    /// A string payload was truncated.
    Truncated,
}

/// Hook invoked for redaction audit events.
pub type RedactionAuditHook = Arc<dyn Fn(RedactionMetricEvent) + Send + Sync + 'static>;

/// Redaction policy derived from configuration.
#[derive(Debug, Clone)]
pub struct RedactionPolicy {
    mode: TelemetryRedactionMode,
    allowlist: BTreeSet<String>,
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self {
            mode: TelemetryRedactionMode::Strict,
            allowlist: BTreeSet::new(),
        }
    }
}

impl RedactionPolicy {
    /// Construct a policy from the runtime telemetry configuration.
    #[must_use]
    pub fn from_config(config: &TelemetryRedaction) -> Self {
        let allowlist = config
            .allowlist
            .iter()
            .map(|entry| normalize_field_name(entry))
            .filter(|entry| !entry.is_empty())
            .collect();
        Self {
            mode: config.mode,
            allowlist,
        }
    }

    #[inline]
    fn allowlist_enabled(&self) -> bool {
        self.mode.allowlist_enabled()
    }

    #[inline]
    fn is_allowlisted(&self, field_name: &str) -> bool {
        let normalized = normalize_field_name(field_name);
        self.allowlist.contains(&normalized)
    }
}

static REDACTION_POLICY: OnceLock<RwLock<RedactionPolicy>> = OnceLock::new();
static REDACTION_AUDIT_HOOK: OnceLock<RwLock<Option<RedactionAuditHook>>> = OnceLock::new();

/// Return true if the build includes telemetry redaction support.
#[inline]
#[must_use]
pub const fn redaction_supported() -> bool {
    REDACTION_SUPPORTED
}

/// Return the approved allow-list policy for bypassing keyword redaction.
#[inline]
#[must_use]
pub const fn redaction_allowlist_policy() -> &'static [&'static str] {
    REDACTION_ALLOWLIST_POLICY
}

/// Normalize a field name for allow-list comparisons.
#[must_use]
pub fn normalize_redaction_field(field_name: &str) -> String {
    normalize_field_name(field_name)
}

/// Override the telemetry redaction policy at runtime.
pub fn set_redaction_policy(policy: RedactionPolicy) {
    let slot = REDACTION_POLICY.get_or_init(|| RwLock::new(policy.clone()));
    let mut guard = slot
        .write()
        .expect("telemetry redaction policy lock poisoned");
    *guard = policy;
}

/// Install a telemetry redaction audit hook.
pub fn set_redaction_audit_hook(hook: RedactionAuditHook) {
    let slot = REDACTION_AUDIT_HOOK.get_or_init(|| RwLock::new(None));
    let mut guard = slot
        .write()
        .expect("telemetry redaction audit hook lock poisoned");
    *guard = Some(hook);
}

#[cfg(test)]
fn clear_redaction_audit_hook() {
    if let Some(slot) = REDACTION_AUDIT_HOOK.get() {
        let mut guard = slot
            .write()
            .expect("telemetry redaction audit hook lock poisoned");
        *guard = None;
    }
}

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
    if matches!(redaction_decision(field_name), RedactionDecision::Redact) {
        return Value::from(REDACTED_PLACEHOLDER);
    }

    match value {
        Value::String(mut raw) => {
            if raw.len() > MAX_FIELD_LENGTH {
                let keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
                raw.truncate(keep);
                raw.push_str(TRUNCATION_SUFFIX);
                emit_redaction_event(RedactionMetricEvent::Truncated);
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

#[derive(Debug, Clone, Copy)]
enum RedactionDecision {
    Redact,
    Allow,
}

#[inline]
fn redaction_decision(field_name: &str) -> RedactionDecision {
    let Some(reason) = classify_sensitive_field(field_name) else {
        return RedactionDecision::Allow;
    };

    if !redaction_supported() {
        emit_redaction_event(RedactionMetricEvent::Skipped {
            reason: RedactionSkipReason::Unsupported,
        });
        return RedactionDecision::Allow;
    }

    let policy = current_redaction_policy();
    if policy.mode.is_disabled() {
        emit_redaction_event(RedactionMetricEvent::Skipped {
            reason: RedactionSkipReason::Disabled,
        });
        return RedactionDecision::Allow;
    }

    if matches!(reason, RedactionReason::Keyword)
        && policy.allowlist_enabled()
        && policy.is_allowlisted(field_name)
    {
        emit_redaction_event(RedactionMetricEvent::Skipped {
            reason: RedactionSkipReason::Allowlist,
        });
        return RedactionDecision::Allow;
    }

    emit_redaction_event(RedactionMetricEvent::Redacted { reason });
    RedactionDecision::Redact
}

fn current_redaction_policy() -> RedactionPolicy {
    REDACTION_POLICY
        .get_or_init(|| RwLock::new(RedactionPolicy::default()))
        .read()
        .expect("telemetry redaction policy lock poisoned")
        .clone()
}

fn emit_redaction_event(event: RedactionMetricEvent) {
    let Some(slot) = REDACTION_AUDIT_HOOK.get() else {
        return;
    };
    let hook = slot
        .read()
        .expect("telemetry redaction audit hook lock poisoned")
        .clone();
    if let Some(hook) = hook {
        hook(event);
    }
}

fn classify_sensitive_field(field_name: &str) -> Option<RedactionReason> {
    let normalized = normalize_field_name(field_name);
    if normalized.is_empty() {
        return None;
    }
    let segments: Vec<&str> = normalized
        .split('_')
        .filter(|seg| !seg.is_empty())
        .collect();

    if let Some(first) = segments.first()
        && EXPLICIT_REDACTION_PREFIXES.contains(first)
    {
        return Some(RedactionReason::Explicit);
    }

    if SENSITIVE_FIELD_KEYWORDS
        .iter()
        .any(|keyword| normalized == *keyword)
    {
        return Some(RedactionReason::Keyword);
    }

    if segments
        .iter()
        .any(|segment| SENSITIVE_FIELD_KEYWORDS.contains(segment))
    {
        return Some(RedactionReason::Keyword);
    }

    None
}

#[cfg(test)]
fn is_sensitive_field(field_name: &str) -> bool {
    classify_sensitive_field(field_name).is_some()
}

fn normalize_field_name(field_name: &str) -> String {
    let chars: Vec<char> = field_name.chars().collect();
    let mut normalized = String::with_capacity(field_name.len() + 4);

    for (idx, ch) in chars.iter().enumerate() {
        if ch.is_ascii_alphanumeric() {
            let is_upper = ch.is_ascii_uppercase();
            if is_upper {
                let prev_is_alnum = idx > 0 && chars[idx - 1].is_ascii_alphanumeric();
                let prev_is_upper = idx > 0 && chars[idx - 1].is_ascii_uppercase();
                let next_is_lower = idx + 1 < chars.len() && chars[idx + 1].is_ascii_lowercase();
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
    use std::sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    };

    use norito::json::{self, Value};

    use super::*;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_test_lock<F: FnOnce()>(f: F) {
        let lock = TEST_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("telemetry test lock poisoned");
        f();
    }

    fn configure_policy(mode: TelemetryRedactionMode, allowlist: &[&str]) {
        let cfg = TelemetryRedaction {
            mode,
            allowlist: allowlist.iter().map(|entry| (*entry).to_string()).collect(),
        };
        set_redaction_policy(RedactionPolicy::from_config(&cfg));
    }

    fn install_counter_hook() -> (Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let redacted = Arc::new(AtomicUsize::new(0));
        let skipped = Arc::new(AtomicUsize::new(0));
        let truncated = Arc::new(AtomicUsize::new(0));
        let redacted_hook = Arc::clone(&redacted);
        let skipped_hook = Arc::clone(&skipped);
        let truncated_hook = Arc::clone(&truncated);
        set_redaction_audit_hook(Arc::new(move |event| match event {
            RedactionMetricEvent::Redacted { .. } => {
                redacted_hook.fetch_add(1, Ordering::SeqCst);
            }
            RedactionMetricEvent::Skipped { .. } => {
                skipped_hook.fetch_add(1, Ordering::SeqCst);
            }
            RedactionMetricEvent::Truncated => {
                truncated_hook.fetch_add(1, Ordering::SeqCst);
            }
        }));
        (redacted, skipped, truncated)
    }

    #[test]
    fn redacts_sensitive_fields_by_default() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Strict, &[]);

            let value = sanitize_value("password", Value::from("super-secret"));
            let direct = sanitize_value("accessToken", Value::from("token"));

            if redaction_supported() {
                assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
                assert_eq!(direct, Value::from(REDACTED_PLACEHOLDER));
            } else {
                assert_eq!(value, Value::from("super-secret"));
                assert_eq!(direct, Value::from("token"));
            }
        });
    }

    #[test]
    fn allowlist_skips_keyword_redaction_when_enabled() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Allowlist, &["ApiKeyHash"]);
            let value = sanitize_value("api_key_hash", Value::from("hash"));
            assert_eq!(value, Value::from("hash"));
        });
    }

    #[test]
    fn normalize_redaction_field_handles_camel_case() {
        assert_eq!(normalize_redaction_field("ApiKeyHash"), "api_key_hash");
        assert_eq!(normalize_redaction_field("apiKeyHash"), "api_key_hash");
        assert_eq!(normalize_redaction_field("api_key_hash"), "api_key_hash");
        assert_eq!(normalize_redaction_field("APIKey"), "api_key");
    }

    #[test]
    fn strict_mode_ignores_allowlist() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Strict, &["api_key_hash"]);
            let value = sanitize_value("api_key_hash", Value::from("hash"));
            if redaction_supported() {
                assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
            } else {
                assert_eq!(value, Value::from("hash"));
            }
        });
    }

    #[test]
    fn explicit_markers_force_redaction() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Allowlist, &["sensitive_payload"]);
            let value = sanitize_value("sensitive_payload", Value::from("data"));
            if redaction_supported() {
                assert_eq!(value, Value::from(REDACTED_PLACEHOLDER));
            } else {
                assert_eq!(value, Value::from("data"));
            }
        });
    }

    #[test]
    fn truncates_oversized_strings_and_emits_metric() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Strict, &[]);
            let (_redacted, _skipped, truncated) = install_counter_hook();

            let payload = "x".repeat(MAX_FIELD_LENGTH + 64);
            let sanitized = sanitize_value("payload", Value::from(payload.clone()));

            let Value::String(output) = sanitized else {
                panic!("sanitized value is not a string");
            };

            assert_eq!(output.len(), MAX_FIELD_LENGTH);
            assert!(output.ends_with(TRUNCATION_SUFFIX));

            let keep = MAX_FIELD_LENGTH.saturating_sub(TRUNCATION_SUFFIX.len());
            assert_eq!(&output[..keep], &payload[..keep]);
            assert!(truncated.load(Ordering::SeqCst) >= 1);
            clear_redaction_audit_hook();
        });
    }

    #[test]
    fn sanitizes_nested_structures() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Strict, &[]);
            let nested = json::object([("token", Value::from("abc")), ("note", Value::from("ok"))])
                .expect("construct nested object");

            let sanitized = sanitize_value("wrapper", nested);
            let Value::Object(mut map) = sanitized else {
                panic!("expected object after sanitization");
            };

            let token_entry = map.remove("token");
            if redaction_supported() {
                assert_eq!(token_entry, Some(Value::from(REDACTED_PLACEHOLDER)));
            } else {
                assert_eq!(token_entry, Some(Value::from("abc")));
            }

            assert_eq!(map.remove("note"), Some(Value::from("ok")));
        });
    }

    #[test]
    fn leaves_non_sensitive_values_intact() {
        with_test_lock(|| {
            configure_policy(TelemetryRedactionMode::Strict, &[]);
            let before = json::object([
                ("count", Value::Number(10_u64.into())),
                ("status", Value::from("ready")),
            ])
            .expect("construct metrics object");
            let sanitized = sanitize_value("metrics", before.clone());
            assert_eq!(sanitized, before);

            assert!(!is_sensitive_field("metrics"));
        });
    }

    #[test]
    fn detects_camel_case_keywords() {
        assert!(is_sensitive_field("refreshToken"));
        assert!(is_sensitive_field("APIKey"));
    }
}
