//! Telegram alerts delivery (feature-gated).

#![cfg(feature = "telegram")]

use eyre::{Result, eyre};
use iroha_config::parameters::actual::Telemetry as Config;
use iroha_logger::telemetry::Event as Telemetry;
use reqwest::Client;
use tokio::{sync::broadcast, task::JoinHandle};
use tokio_stream::wrappers::BroadcastStream;
use url::Url;

/// Start a background task that listens for telemetry events and sends alerts
/// to a designated Telegram chat.
///
/// Behavior: sends human-readable messages for events that include a `msg` field
/// and a textual `text`/`error`/`warning` field. Extend as needed.
pub async fn start(
    config: Config,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    start_with_context(config, None, telemetry).await
}

/// Start Telegram alerts with optional chain id context.
pub async fn start_with_context(
    config: Config,
    chain_id: Option<String>,
    telemetry: broadcast::Receiver<Telemetry>,
) -> Result<JoinHandle<()>> {
    let (bot_key, chat_id) = match (
        config.telegram_bot_key.as_deref(),
        config.telegram_chat_id.as_deref(),
    ) {
        (Some(k), Some(c)) if !k.is_empty() && !c.is_empty() => (k.to_owned(), c.to_owned()),
        _ => return Err(eyre!("Telegram configuration missing bot_key or chat_id")),
    };
    let client = Client::builder().build()?;
    let settings = AlertSettings::from_config(&config);
    let handle = tokio::spawn(run(
        client,
        bot_key,
        chat_id,
        config.name.clone(),
        chain_id,
        settings,
        config.telegram_metrics_url.clone(),
        config.telegram_metrics_period,
        telemetry,
    ));
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
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

impl AlertSettings {
    fn from_config(c: &Config) -> Self {
        let min_level =
            c.telegram_min_level
                .as_deref()
                .and_then(|s| match s.to_ascii_uppercase().as_str() {
                    "TRACE" => Some(Level::TRACE),
                    "DEBUG" => Some(Level::DEBUG),
                    "INFO" => Some(Level::INFO),
                    "WARN" | "WARNING" => Some(Level::WARN),
                    "ERROR" => Some(Level::ERROR),
                    _ => None,
                });
        let targets = c.telegram_targets.clone();
        let rate_per_minute = c.telegram_rate_per_minute.map(|n| n.get());
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
        .find(|(k, _)| *k == &"level")
        .map(|(_, v)| v);
    match level_val
        .and_then(|v| v.as_str())
        .map(|s| s.to_ascii_uppercase())
    {
        Some(s) if s == "TRACE" => Some(Level::TRACE),
        Some(s) if s == "DEBUG" => Some(Level::DEBUG),
        Some(s) if s == "INFO" => Some(Level::INFO),
        Some(s) if s == "WARN" || s == "WARNING" => Some(Level::WARN),
        Some(s) if s == "ERROR" => Some(Level::ERROR),
        _ => None,
    }
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
        let now = tokio::time::Instant::now();
        while let Some(&t) = self.sent.front() {
            if now.duration_since(t).as_secs() >= 60 {
                self.sent.pop_front();
            } else {
                break;
            }
        }
        if (self.sent.len() as u32) < self.limit {
            self.sent.push_back(now);
            true
        } else {
            false
        }
    }
}

async fn run(
    client: Client,
    bot_key: String,
    chat_id: String,
    node_name: String,
    chain_id: Option<String>,
    settings: AlertSettings,
    metrics_url: Option<Url>,
    metrics_period: Option<std::time::Duration>,
    receiver: broadcast::Receiver<Telemetry>,
) {
    let mut stream = BroadcastStream::new(receiver).fuse();
    let mut limiter = settings.rate_per_minute.map(RateLimiter::new);
    // Optional metrics sampler
    let snapshot: std::sync::Arc<tokio::sync::Mutex<Option<Snapshot>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(None));
    if settings.include_metrics {
        if let (Some(url), Some(period)) = (metrics_url, metrics_period) {
            let client = client.clone();
            let snap = snapshot.clone();
            tokio::spawn(async move {
                sample_metrics_loop(client, url, period, snap).await;
            });
        }
    }
    while let Some(item) = stream.next().await {
        let Ok(event) = item else {
            continue;
        };
        // Filter by target prefix and minimum level
        if !target_allowed(&event, &settings) {
            continue;
        }
        if let Some(min) = settings.min_level {
            if let Some(lvl) = parse_level(&event) {
                if lvl < min {
                    continue;
                }
            }
        }
        // Allow/deny lists by message kind
        let kind = event
            .fields
            .0
            .iter()
            .find(|(k, _)| *k == &"msg")
            .and_then(|(_, v)| v.as_str());
        if let Some(k) = kind {
            if let Some(deny) = &settings.deny_kinds {
                if deny.iter().any(|d| d == k) {
                    continue;
                }
            }
            if let Some(allow) = &settings.allow_kinds {
                if !allow.is_empty() && !allow.iter().any(|a| a == k) {
                    continue;
                }
            }
        }
        // Rate limit
        if let Some(lim) = limiter.as_mut() {
            if !lim.allow() {
                continue;
            }
        }
        // Include latest sampled snapshot if any
        let snap_opt = snapshot.lock().await.clone();
        if let Some(text) = format_alert(&node_name, chain_id.as_deref(), snap_opt.as_ref(), &event)
        {
            if let Err(e) = send_message(&client, &bot_key, &chat_id, &text).await {
                iroha_logger::warn!(%e, "Failed to send Telegram alert");
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
                    _ => v.to_string(),
                })
            }
            // include small snapshot fields when present
            "connected_peers" | "peers" | "queue_size" => {
                extra.push(((*k).to_string(), v.to_string()));
            }
            _ => {}
        }
    }
    let kind = kind?;
    let text = text.unwrap_or_else(|| "event".to_string());
    let mut prefix = format!("[{}] [node:{}]", kind, node_name);
    if let Some(c) = chain {
        prefix.push_str(&format!(" [chain:{}]", c));
    }
    if !extra.is_empty() {
        let sn = extra
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(" ");
        prefix.push_str(&format!(" [{}]", sn));
    }
    if let Some(s) = snap {
        let mut parts = vec![];
        if let Some(p) = s.connected_peers {
            parts.push(format!("connected_peers={}", p));
        }
        if let Some(q) = s.queue_size {
            parts.push(format!("queue_size={}", q));
        }
        if let Some(bh) = s.block_height {
            parts.push(format!("block_height={}", bh));
        }
        if let Some(ct) = s.last_commit_time_ms {
            parts.push(format!("last_commit_time_ms={}", ct));
        }
        if !parts.is_empty() {
            prefix.push_str(&format!(" [{}]", parts.join(" ")));
        }
    }
    Some(format!("{} {}", prefix, text))
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
    let txt = client.get(url).send().await?.text().await?;
    let mut snap = Snapshot::default();
    for line in txt.lines() {
        if line.starts_with("connected_peers ") {
            if let Some(val) = line.split_whitespace().nth(1) {
                snap.connected_peers = val.parse::<f64>().ok().map(|f| f as u64);
            }
        } else if line.starts_with("queue_size ") {
            if let Some(val) = line.split_whitespace().nth(1) {
                snap.queue_size = val.parse::<f64>().ok().map(|f| f as u64);
            }
        } else if line.starts_with("block_height ") {
            if let Some(val) = line.split_whitespace().nth(1) {
                snap.block_height = val.parse::<f64>().ok().map(|f| f as u64);
            }
        } else if line.starts_with("last_commit_time_ms ") {
            if let Some(val) = line.split_whitespace().nth(1) {
                snap.last_commit_time_ms = val.parse::<f64>().ok().map(|f| f as u64);
            }
        }
    }
    Ok(snap)
}

async fn send_message(client: &Client, bot_key: &str, chat_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_key);
    let body = norito::json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true,
    });
    let res = client.post(url).json(&body).send().await?;
    if !res.status().is_success() {
        return Err(eyre!("telegram send failed: {}", res.status()));
    }
    Ok(())
}

use futures::StreamExt;
