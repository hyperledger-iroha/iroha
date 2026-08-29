//! Telegram alerts delivery (feature-gated).
use crate::metrics::Metrics;
use eyre::{Result, eyre};
use iroha_config::parameters::actual::Telemetry as Config;
use iroha_logger::telemetry::Event as Telemetry;
use reqwest::Client;
use std::{fmt::Write as _, future::Future};
use tokio::{sync::broadcast, task::JoinHandle};
const HTTP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const METRICS_SNAPSHOT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
fn build_http_client() -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
/// Start Telegram alerts with canonical node and optional chain context.
///
/// `metrics_snapshot` is called only for a filtered, rate-admitted alert when
/// `telegram_include_metrics` is enabled. Returning `None` sends the alert
/// without an optional metrics suffix.
///
/// # Errors
///
/// Returns an error when the Telegram credentials are missing or the HTTP
/// client cannot be constructed.
pub fn start<F, Fut>(
    config: Config,
    node_name: String,
    chain_id: Option<String>,
    metrics_snapshot: F,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<MetricsSnapshot>> + Send + 'static,
{
    let (bot_key, chat_id) = match (
        config.telegram_bot_key.as_deref(),
        config.telegram_chat_id.as_deref(),
    ) {
        (Some(k), Some(c)) if !k.is_empty() && !c.is_empty() => (k.to_owned(), c.to_owned()),
        _ => return Err(eyre!("Telegram configuration missing bot_key or chat_id")),
    };
    let client = build_http_client()?;
    let settings = AlertSettings::from_config(&config)?;
    let worker = TelegramWorker {
        client,
        bot_key,
        chat_id,
        node_name,
        chain_id,
        settings,
        metrics_snapshot,
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
        if value.eq_ignore_ascii_case("TRACE") {
            Some(Self::Trace)
        } else if value.eq_ignore_ascii_case("DEBUG") {
            Some(Self::Debug)
        } else if value.eq_ignore_ascii_case("INFO") {
            Some(Self::Info)
        } else if value.eq_ignore_ascii_case("WARN") || value.eq_ignore_ascii_case("WARNING") {
            Some(Self::Warn)
        } else if value.eq_ignore_ascii_case("ERROR") {
            Some(Self::Error)
        } else {
            None
        }
    }
}
impl AlertSettings {
    fn from_config(c: &Config) -> Result<Self> {
        let min_level = parse_min_level(c.telegram_min_level.as_deref())?;
        let targets = c.telegram_targets.clone();
        let rate_per_minute = c.telegram_rate_per_minute.map(std::num::NonZeroU32::get);
        Ok(Self {
            min_level,
            targets,
            rate_per_minute,
            allow_kinds: c.telegram_allow_kinds.clone(),
            deny_kinds: c.telegram_deny_kinds.clone(),
            include_metrics: c.telegram_include_metrics,
        })
    }
}
fn parse_min_level(value: Option<&str>) -> Result<Option<Level>> {
    value
        .map(|value| {
            Level::parse(value).ok_or_else(|| {
                eyre!("telegram_min_level must be TRACE, DEBUG, INFO, WARN, or ERROR")
            })
        })
        .transpose()
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

fn level_allowed(event: &Telemetry, settings: &AlertSettings) -> bool {
    settings
        .min_level
        .is_none_or(|minimum| parse_level(event).is_some_and(|level| level >= minimum))
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
struct TelegramWorker<F> {
    client: Client,
    bot_key: String,
    chat_id: String,
    node_name: String,
    chain_id: Option<String>,
    settings: AlertSettings,
    metrics_snapshot: F,
}

impl<F, Fut> TelegramWorker<F>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<MetricsSnapshot>> + Send + 'static,
{
    async fn run(self, mut receiver: broadcast::Receiver<Telemetry>) {
        let Self {
            client,
            bot_key,
            chat_id,
            node_name,
            chain_id,
            settings,
            metrics_snapshot,
        } = self;
        let mut limiter = settings.rate_per_minute.map(RateLimiter::new);
        loop {
            let event = match receiver.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    iroha_logger::warn!(%skipped, "Telegram telemetry channel lagged; dropped events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };
            // Filter by target prefix and minimum level
            if !target_allowed(&event, &settings) {
                continue;
            }
            if !level_allowed(&event, &settings) {
                continue;
            }
            // Allow/deny lists by message kind
            let Some(kind) = event
                .fields
                .0
                .iter()
                .find(|(k, _)| *k == "msg")
                .and_then(|(_, v)| v.as_str())
            else {
                continue;
            };
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
            if limiter.as_mut().is_some_and(|limiter| !limiter.allow()) {
                continue;
            }
            let snapshot = if settings.include_metrics {
                tokio::time::timeout(METRICS_SNAPSHOT_TIMEOUT, metrics_snapshot())
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };
            let text = format_alert(
                &node_name,
                chain_id.as_deref(),
                snapshot.as_ref(),
                &event,
                kind,
            );
            if let Err(error) = send_message(&client, &bot_key, &chat_id, &text).await {
                iroha_logger::warn!(%error, "Failed to send Telegram alert");
            }
        }
    }
}

fn format_alert(
    node_name: &str,
    chain: Option<&str>,
    snap: Option<&MetricsSnapshot>,
    event: &Telemetry,
    kind: &str,
) -> String {
    // Expect a `msg` field naming the event, and a `text`/`error` field for content.
    let mut text: Option<String> = None;
    let mut extra: Vec<(String, String)> = Vec::new();
    for (k, v) in &event.fields.0 {
        match *k {
            "msg" => {}
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
        prefix.push_str(" [");
        for (index, (name, value)) in metrics.into_iter().enumerate() {
            if index != 0 {
                prefix.push(' ');
            }
            write!(&mut prefix, "{name}={value}").expect("writing to a String cannot fail");
        }
        prefix.push(']');
    }
    format!("{prefix} {text}")
}
fn render_json_value(value: &norito::json::Value) -> String {
    norito::json::to_json(value).unwrap_or_else(|error| format!("<invalid JSON value: {error}>"))
}
/// Four bounded scalar metrics attached to a Telegram alert.
#[derive(Clone, Copy, Debug)]
pub struct MetricsSnapshot {
    connected_peers: u64,
    queue_size: u64,
    block_height: u64,
    last_commit_time_ms: u64,
}
impl MetricsSnapshot {
    /// Read an alert snapshot from an already-refreshed metrics registry.
    #[must_use]
    pub fn from_metrics(metrics: &Metrics) -> Self {
        Self {
            connected_peers: metrics.connected_peers.get(),
            queue_size: metrics.queue_size.get(),
            block_height: metrics.block_height.get(),
            last_commit_time_ms: metrics.last_commit_time_ms.get(),
        }
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
        .await
        .map_err(|error| eyre!("Telegram request failed: {}", error.without_url()))?;
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
        assert_eq!(parse_min_level(None).expect("unset level"), None);
        assert_eq!(
            parse_min_level(Some("warning")).expect("warning level"),
            Some(Level::Warn)
        );
        let error = parse_min_level(Some("critical")).expect_err("invalid minimum level");
        assert!(error.to_string().contains("must be TRACE"));
    }

    #[test]
    fn configured_minimum_level_rejects_missing_or_lower_levels() {
        let settings = AlertSettings {
            min_level: Some(Level::Warn),
            targets: None,
            rate_per_minute: None,
            allow_kinds: None,
            deny_kinds: None,
            include_metrics: false,
        };
        assert!(level_allowed(&event_with_level("error"), &settings));
        assert!(!level_allowed(&event_with_level("info"), &settings));
        let missing = Telemetry {
            target: "telegram::tests",
            fields: Fields::default(),
        };
        assert!(!level_allowed(&missing, &settings));
    }

    #[test]
    fn metrics_snapshot_reads_the_shared_registry_directly() {
        let metrics = Metrics::default();
        metrics.connected_peers.set(4);
        metrics.queue_size.set(5);
        metrics.block_height.inc_by(90);
        metrics.last_commit_time_ms.set(12);

        let snapshot = MetricsSnapshot::from_metrics(&metrics);
        assert_eq!(snapshot.connected_peers, 4);
        assert_eq!(snapshot.queue_size, 5);
        assert_eq!(snapshot.block_height, 90);
        assert_eq!(snapshot.last_commit_time_ms, 12);
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
}
