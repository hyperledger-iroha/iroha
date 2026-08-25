//! Telegram alerts delivery (feature-gated).
use eyre::{Result, eyre};
use futures::StreamExt;
use iroha_config::parameters::actual::Telemetry as Config;
use iroha_logger::telemetry::Event as Telemetry;
use reqwest::Client;
use std::fmt::Write as _;
use tokio::{sync::broadcast, task::JoinHandle};
use tokio_stream::wrappers::BroadcastStream;
use url::Url;
/// Maximum Prometheus text examined during one Telegram metrics sample.
///
/// This is a protocol-facing first-release resource ceiling, rather than a process-memory tuning
/// knob: identical responses are accepted or rejected on every node regardless of available memory.
const TELEGRAM_METRICS_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Maximum single Prometheus line retained while streaming a sample.
const TELEGRAM_METRICS_LINE_MAX_BYTES: usize = 16 * 1024;
/// Start a background task that listens for telemetry events and sends alerts
/// to a designated Telegram chat.
///
/// Behavior: sends human-readable messages for events that include a `msg` field
/// and a textual `text`/`error`/`warning` field. Extend as needed.
///
/// # Errors
///
/// Returns an error when the Telegram credentials are missing or the HTTP
/// client cannot be constructed.
pub async fn start(
    config: Config,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    start_with_context(config, None, telemetry).await
}
/// Start Telegram alerts with optional chain id context.
///
/// # Errors
///
/// Returns an error when the Telegram credentials are missing or the HTTP
/// client cannot be constructed.
pub async fn start_with_context(
    config: Config,
    chain_id: Option<String>,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    // Keep the established async API without introducing a scheduler yield.
    std::future::ready(()).await;
    let (bot_key, chat_id) = match (
        config.telegram_bot_key.as_deref(),
        config.telegram_chat_id.as_deref(),
    ) {
        (Some(k), Some(c)) if !k.is_empty() && !c.is_empty() => (k.to_owned(), c.to_owned()),
        _ => return Err(eyre!("Telegram configuration missing bot_key or chat_id")),
    };
    let client = Client::builder().build()?;
    let settings = AlertSettings::from_config(&config);
    let worker = TelegramWorker {
        client,
        bot_key,
        chat_id,
        node_name: config.name.clone(),
        chain_id,
        settings,
        metrics_url: config.telegram_metrics_url.clone(),
        metrics_period: config.telegram_metrics_period,
    };
    let handle = tokio::spawn(worker.run(telemetry));
    Ok(handle)
}
#[derive(Clone, Debug)]
struct AlertSettings {
    min_level: Option<Level>,
    targets: Option<Vec<String>>, // prefix match
    rate_per_minute: Option<u32>,
    allow_kinds: Option<Vec<String>>, // exact match
    deny_kinds: Option<Vec<String>>,  // exact match
    include_metrics: bool,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
impl Level {
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" | "WARNING" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            _ => None,
        }
    }
}
impl AlertSettings {
    fn from_config(c: &Config) -> Self {
        let min_level = c.telegram_min_level.as_deref().and_then(Level::parse);
        let targets = c.telegram_targets.clone();
        let rate_per_minute = c.telegram_rate_per_minute.map(std::num::NonZeroU32::get);
        Self {
            min_level,
            targets,
            rate_per_minute,
            allow_kinds: c.telegram_allow_kinds.clone(),
            deny_kinds: c.telegram_deny_kinds.clone(),
            include_metrics: c.telegram_include_metrics,
        }
    }
}
fn parse_level(event: &Telemetry) -> Option<Level> {
    let level_val = event
        .fields
        .0
        .iter()
        .find(|(k, _)| *k == "level")
        .map(|(_, v)| v);
    level_val.and_then(|v| v.as_str()).and_then(Level::parse)
}
fn target_allowed(event: &Telemetry, settings: &AlertSettings) -> bool {
    match &settings.targets {
        None => true,
        Some(list) if list.is_empty() => true,
        Some(list) => list.iter().any(|p| event.target.starts_with(p)),
    }
}
struct RateLimiter {
    limit: u32,
    sent: std::collections::VecDeque<tokio::time::Instant>,
}
impl RateLimiter {
    fn new(limit: u32) -> Self {
        Self {
            limit,
            sent: std::collections::VecDeque::new(),
        }
    }
    fn allow(&mut self) -> bool {
        self.allow_at(tokio::time::Instant::now())
    }
    fn allow_at(&mut self, now: tokio::time::Instant) -> bool {
        while let Some(&t) = self.sent.front() {
            if now.duration_since(t).as_secs() >= 60 {
                self.sent.pop_front();
            } else {
                break;
            }
        }
        let Ok(sent) = u32::try_from(self.sent.len()) else {
            return false;
        };
        if sent < self.limit {
            self.sent.push_back(now);
            true
        } else {
            false
        }
    }
}
struct TelegramWorker {
    client: Client,
    bot_key: String,
    chat_id: String,
    node_name: String,
    chain_id: Option<String>,
    settings: AlertSettings,
    metrics_url: Option<Url>,
    metrics_period: Option<std::time::Duration>,
}
impl TelegramWorker {
    async fn run(self, receiver: broadcast::Receiver<Telemetry>) {
        let Self {
            client,
            bot_key,
            chat_id,
            node_name,
            chain_id,
            settings,
            metrics_url,
            metrics_period,
        } = self;
        let mut stream = BroadcastStream::new(receiver).fuse();
        let mut limiter = settings.rate_per_minute.map(RateLimiter::new);
        // Optional metrics sampler
        let snapshot: std::sync::Arc<tokio::sync::Mutex<Option<Snapshot>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(None));
        if settings.include_metrics
            && let (Some(url), Some(period)) = (metrics_url, metrics_period)
        {
            let client = client.clone();
            let snap = snapshot.clone();
            tokio::spawn(async move {
                sample_metrics_loop(client, url, period, snap).await;
            });
        }
        while let Some(item) = stream.next().await {
            let Ok(event) = item else {
                continue;
            };
            // Filter by target prefix and minimum level
            if !target_allowed(&event, &settings) {
                continue;
            }
            if settings
                .min_level
                .zip(parse_level(&event))
                .is_some_and(|(minimum, level)| level < minimum)
            {
                continue;
            }
            // Allow/deny lists by message kind
            let kind = event
                .fields
                .0
                .iter()
                .find(|(k, _)| *k == "msg")
                .and_then(|(_, v)| v.as_str());
            if let Some(kind) = kind {
                let denied = settings
                    .deny_kinds
                    .as_ref()
                    .is_some_and(|deny| deny.iter().any(|candidate| candidate == kind));
                if denied {
                    continue;
                }
                let excluded = settings.allow_kinds.as_ref().is_some_and(|allow| {
                    !allow.is_empty() && !allow.iter().any(|candidate| candidate == kind)
                });
                if excluded {
                    continue;
                }
            }
            // Rate limit
            if limiter.as_mut().is_some_and(|limiter| !limiter.allow()) {
                continue;
            }
            // Include latest sampled snapshot if any
            let snap_opt = snapshot.lock().await.clone();
            let Some(text) =
                format_alert(&node_name, chain_id.as_deref(), snap_opt.as_ref(), &event)
            else {
                continue;
            };
            if let Err(error) = send_message(&client, &bot_key, &chat_id, &text).await {
                iroha_logger::warn!(%error, "Failed to send Telegram alert");
            }
        }
    }
}
fn format_alert(
    node_name: &str,
    chain: Option<&str>,
    snap: Option<&Snapshot>,
    event: &Telemetry,
) -> Option<String> {
    // Expect a `msg` field naming the event, and a `text`/`error` field for content.
    let mut kind: Option<&str> = None;
    let mut text: Option<String> = None;
    let mut extra: Vec<(String, String)> = Vec::new();
    for (k, v) in &event.fields.0 {
        match *k {
            "msg" => kind = v.as_str(),
            "error" | "text" | "warning" => {
                text = Some(match v {
                    norito::json::Value::String(s) => s.clone(),
                    _ => render_json_value(v),
                })
            }
            // include small snapshot fields when present
            "connected_peers" | "peers" | "queue_size" => {
                extra.push(((*k).to_string(), render_json_value(v)));
            }
            _ => {}
        }
    }
    let kind = kind?;
    let text = text.unwrap_or_else(|| "event".to_string());
    let mut prefix = format!("[{kind}] [node:{node_name}]");
    if let Some(chain) = chain {
        write!(&mut prefix, " [chain:{chain}]").expect("writing to a String cannot fail");
    }
    if !extra.is_empty() {
        prefix.push_str(" [");
        for (index, (key, value)) in extra.into_iter().enumerate() {
            if index != 0 {
                prefix.push(' ');
            }
            prefix.push_str(&key);
            prefix.push('=');
            prefix.push_str(&value);
        }
        prefix.push(']');
    }
    if let Some(snapshot) = snap {
        let metrics = [
            ("connected_peers", snapshot.connected_peers),
            ("queue_size", snapshot.queue_size),
            ("block_height", snapshot.block_height),
            ("last_commit_time_ms", snapshot.last_commit_time_ms),
        ];
        let mut emitted = false;
        for (name, value) in metrics {
            let Some(value) = value else {
                continue;
            };
            if emitted {
                prefix.push(' ');
            } else {
                prefix.push_str(" [");
                emitted = true;
            }
            write!(&mut prefix, "{name}={value}").expect("writing to a String cannot fail");
        }
        if emitted {
            prefix.push(']');
        }
    }
    Some(format!("{prefix} {text}"))
}
fn render_json_value(value: &norito::json::Value) -> String {
    norito::json::to_json(value).unwrap_or_else(|error| format!("<invalid JSON value: {error}>"))
}
#[derive(Clone, Debug, Default)]
struct Snapshot {
    connected_peers: Option<u64>,
    queue_size: Option<u64>,
    block_height: Option<u64>,
    last_commit_time_ms: Option<u64>,
}
async fn sample_metrics_loop(
    client: Client,
    url: Url,
    period: std::time::Duration,
    dst: std::sync::Arc<tokio::sync::Mutex<Option<Snapshot>>>,
) {
    let mut interval = tokio::time::interval(period);
    loop {
        interval.tick().await;
        match fetch_metrics(&client, url.clone()).await {
            Ok(s) => {
                *dst.lock().await = Some(s);
            }
            Err(e) => iroha_logger::debug!(%e, "metrics snapshot fetch failed"),
        }
    }
}
async fn fetch_metrics(client: &Client, url: Url) -> Result<Snapshot> {
    let mut response = client.get(url).send().await?;
    if let Some(declared) = response.content_length()
        && declared
            > u64::try_from(TELEGRAM_METRICS_RESPONSE_MAX_BYTES)
                .expect("Telegram metrics response cap fits u64")
    {
        return Err(eyre!(
            "metrics response declares {declared} bytes, exceeding the {}-byte limit",
            TELEGRAM_METRICS_RESPONSE_MAX_BYTES
        ));
    }
    let mut decoder = BoundedMetricsDecoder::default();
    while let Some(chunk) = response.chunk().await? {
        decoder.push(&chunk)?;
    }
    decoder.finish()
}
#[cfg(test)]
fn parse_metrics(text: &str) -> Snapshot {
    let mut snap = Snapshot::default();
    for line in text.lines() {
        observe_metric_line(line, &mut snap);
    }
    snap
}
#[derive(Default)]
struct BoundedMetricsDecoder {
    snapshot: Snapshot,
    pending_line: Vec<u8>,
    examined_bytes: usize,
}
impl BoundedMetricsDecoder {
    fn push(&mut self, chunk: &[u8]) -> Result<()> {
        self.examined_bytes = self
            .examined_bytes
            .checked_add(chunk.len())
            .ok_or_else(|| eyre!("metrics response byte count overflowed"))?;
        if self.examined_bytes > TELEGRAM_METRICS_RESPONSE_MAX_BYTES {
            return Err(eyre!(
                "metrics response exceeded the {}-byte limit while streaming",
                TELEGRAM_METRICS_RESPONSE_MAX_BYTES
            ));
        }
        for &byte in chunk {
            if byte == b'\n' {
                self.finish_line()?;
                continue;
            }
            if self.pending_line.len() >= TELEGRAM_METRICS_LINE_MAX_BYTES {
                return Err(eyre!(
                    "metrics response line exceeded the {}-byte limit",
                    TELEGRAM_METRICS_LINE_MAX_BYTES
                ));
            }
            self.pending_line.push(byte);
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Snapshot> {
        if !self.pending_line.is_empty() {
            self.finish_line()?;
        }
        Ok(self.snapshot)
    }
    fn finish_line(&mut self) -> Result<()> {
        if self.pending_line.last() == Some(&b'\r') {
            self.pending_line.pop();
        }
        let line = core::str::from_utf8(&self.pending_line)
            .map_err(|error| eyre!("metrics response is not UTF-8: {error}"))?;
        observe_metric_line(line, &mut self.snapshot);
        self.pending_line.clear();
        Ok(())
    }
}
fn observe_metric_line(line: &str, snap: &mut Snapshot) {
    let mut fields = line.split_whitespace();
    let (Some(name), Some(raw_value)) = (fields.next(), fields.next()) else {
        return;
    };
    let Some(value) = parse_metric_u64(raw_value) else {
        return;
    };
    match name {
        "connected_peers" => snap.connected_peers = Some(value),
        "queue_size" => snap.queue_size = Some(value),
        "block_height" => snap.block_height = Some(value),
        "last_commit_time_ms" => snap.last_commit_time_ms = Some(value),
        _ => {}
    }
}
fn parse_metric_u64(raw: &str) -> Option<u64> {
    if let Ok(value) = raw.parse::<u64>() {
        return Some(value);
    }
    let unsigned = raw.strip_prefix('+').unwrap_or(raw);
    if unsigned.starts_with('-') {
        return None;
    }
    let exponent_marker = unsigned
        .char_indices()
        .find_map(|(index, character)| matches!(character, 'e' | 'E').then_some(index));
    let (mantissa, exponent) = if let Some(index) = exponent_marker {
        let (mantissa, exponent_with_marker) = unsigned.split_at(index);
        let exponent = exponent_with_marker.get(1..)?.parse::<i64>().ok()?;
        if mantissa.contains(['e', 'E']) || exponent_with_marker[1..].contains(['e', 'E']) {
            return None;
        }
        (mantissa, exponent)
    } else {
        (unsigned, 0)
    };
    let (whole, fraction) = if let Some((whole, fraction)) = mantissa.split_once('.') {
        if fraction.contains('.') {
            return None;
        }
        (whole, fraction)
    } else {
        (mantissa, "")
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let mut digits = String::with_capacity(mantissa.len());
    digits.push_str(whole);
    digits.push_str(fraction);
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    let fraction_len = i64::try_from(fraction.len()).ok()?;
    let scale = exponent.checked_sub(fraction_len)?;
    if scale < 0 {
        let fractional_digits = usize::try_from(scale.unsigned_abs()).ok()?;
        if fractional_digits > digits.len() {
            return None;
        }
        let integer_len = digits.len() - fractional_digits;
        if !digits.as_bytes()[integer_len..]
            .iter()
            .all(|byte| *byte == b'0')
        {
            return None;
        }
        digits[..integer_len].parse::<u64>().ok()
    } else {
        let value = digits.parse::<u64>().ok()?;
        let scale = u32::try_from(scale).ok()?;
        value.checked_mul(10_u64.checked_pow(scale)?)
    }
}
async fn send_message(client: &Client, bot_key: &str, chat_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{bot_key}/sendMessage");
    let body = norito::json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true,
    });
    let encoded = norito::json::to_json(&body)?;
    let res = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(encoded)
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(eyre!("telegram send failed: {}", res.status()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_logger::telemetry::Fields;
    use norito::json::Value;
    fn event_with_level(level: &str) -> Telemetry {
        Telemetry {
            target: "telegram::tests",
            fields: Fields(vec![("level", Value::String(level.to_owned()))]),
        }
    }
    #[test]
    fn level_parsing_is_case_insensitive_and_accepts_warning_alias() {
        assert_eq!(Level::parse("trace"), Some(Level::Trace));
        assert_eq!(Level::parse("DeBuG"), Some(Level::Debug));
        assert_eq!(Level::parse("INFO"), Some(Level::Info));
        assert_eq!(Level::parse("warning"), Some(Level::Warn));
        assert_eq!(Level::parse("ERROR"), Some(Level::Error));
        assert_eq!(Level::parse("critical"), None);
        assert_eq!(parse_level(&event_with_level("warn")), Some(Level::Warn));
    }
    #[test]
    fn rate_limiter_enforces_the_sliding_minute_window() {
        let origin = tokio::time::Instant::now();
        let mut limiter = RateLimiter::new(2);
        assert!(limiter.allow_at(origin));
        assert!(limiter.allow_at(origin + std::time::Duration::from_secs(30)));
        assert!(!limiter.allow_at(origin + std::time::Duration::from_secs(59)));
        assert!(limiter.allow_at(origin + std::time::Duration::from_secs(60)));
        assert!(!limiter.allow_at(origin + std::time::Duration::from_secs(60)));
        assert!(limiter.allow_at(origin + std::time::Duration::from_secs(90)));
    }
    #[test]
    fn metric_integer_parser_accepts_only_exact_u64_values() {
        for (sample, expected) in [
            ("0", 0),
            ("42", 42),
            ("42.0", 42),
            ("4.2e1", 42),
            ("420e-1", 42),
            ("+42", 42),
            ("18446744073709551615.0", u64::MAX),
        ] {
            assert_eq!(parse_metric_u64(sample), Some(expected), "{sample}");
        }
        for sample in [
            "",
            "-1",
            "1.5",
            "1e-1",
            "NaN",
            "+Inf",
            "18446744073709551616",
            "1e100",
            "1.0.0",
        ] {
            assert_eq!(parse_metric_u64(sample), None, "{sample}");
        }
    }
    #[test]
    fn metrics_snapshot_ignores_invalid_and_unknown_samples() {
        let snapshot = parse_metrics(
            "connected_peers 4\n\
             queue_size 5.0\n\
             block_height 9e1\n\
             last_commit_time_ms -2\n\
             unrelated_metric 12\n",
        );
        assert_eq!(snapshot.connected_peers, Some(4));
        assert_eq!(snapshot.queue_size, Some(5));
        assert_eq!(snapshot.block_height, Some(90));
        assert_eq!(snapshot.last_commit_time_ms, None);
    }
    #[test]
    fn bounded_metrics_decoder_streams_split_lines_and_rejects_resource_overflow() {
        let mut decoder = BoundedMetricsDecoder::default();
        decoder
            .push(b"connected_pe")
            .expect("first split chunk is bounded");
        decoder
            .push(b"ers 7\r\nqueue_size 11\n")
            .expect("second split chunk is bounded");
        let snapshot = decoder.finish().expect("split metrics decode");
        assert_eq!(snapshot.connected_peers, Some(7));
        assert_eq!(snapshot.queue_size, Some(11));
        let mut response_boundary = BoundedMetricsDecoder {
            examined_bytes: TELEGRAM_METRICS_RESPONSE_MAX_BYTES - 1,
            ..BoundedMetricsDecoder::default()
        };
        response_boundary
            .push(b"x")
            .expect("exact aggregate byte limit is accepted");
        let response_error = response_boundary
            .push(b"x")
            .expect_err("aggregate max plus one must fail");
        assert!(response_error.to_string().contains("while streaming"));
        let mut line_boundary = BoundedMetricsDecoder {
            pending_line: vec![b'x'; TELEGRAM_METRICS_LINE_MAX_BYTES],
            ..BoundedMetricsDecoder::default()
        };
        let line_error = line_boundary
            .push(b"x")
            .expect_err("line max plus one must fail");
        assert!(line_error.to_string().contains("line exceeded"));
    }
}
