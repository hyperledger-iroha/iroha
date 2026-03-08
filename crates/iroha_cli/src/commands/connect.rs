//! Connect diagnostics helpers (`iroha connect queue inspect`).

use std::{
    collections::BTreeMap,
    fmt::{self, Write as _},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use base64::Engine as _;
use clap::{Args, Subcommand, ValueEnum};
use eyre::{Context, Result, eyre};
use norito::json;

use crate::{
    CliOutputFormat, Run, RunContext,
    json_macros::{JsonDeserialize, JsonSerialize},
};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Queue inspection tooling
    #[command(subcommand)]
    Queue(queue::Command),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let connect_root = context.config().connect_queue_root.clone();
        run(self, &connect_root, context)
    }
}

pub fn run<C: RunContext>(command: Command, connect_root: &Path, context: &mut C) -> Result<()> {
    match command {
        Command::Queue(sub) => queue::run(sub, connect_root, context),
    }
}

pub mod queue {
    use super::*;

    #[derive(Debug, Subcommand)]
    pub enum Command {
        /// Inspect on-disk queue diagnostics for a Connect session
        Inspect(Inspect),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let connect_root = context.config().connect_queue_root.clone();
            run(self, &connect_root, context)
        }
    }

    pub fn run<C: RunContext>(
        command: Command,
        connect_root: &Path,
        context: &mut C,
    ) -> Result<()> {
        match command {
            Command::Inspect(args) => {
                let report = build_report(&args, connect_root)?;
                match context.output_format() {
                    CliOutputFormat::Json => context.print_data(&report),
                    CliOutputFormat::Text => match args.format {
                        OutputFormat::Table => context.println(render_table(&report)),
                        OutputFormat::Json => context.print_data(&report),
                    },
                }
            }
        }
    }

    #[derive(Debug, Clone, Args)]
    pub struct Inspect {
        /// Connect session identifier (base64url, no padding). Required unless `--snapshot` is provided.
        #[arg(long)]
        pub sid: Option<String>,
        /// Path to an explicit snapshot JSON file (defaults to `<root>/<sid>/state.json`).
        #[arg(long)]
        pub snapshot: Option<PathBuf>,
        /// Root directory containing Connect queue state (defaults to `connect.queue.root` or `~/.iroha/connect`).
        #[arg(long)]
        pub root: Option<PathBuf>,
        /// Include metrics summary derived from `metrics.ndjson`.
        #[arg(long)]
        pub metrics: bool,
        /// Output format for text mode (`table` or `json`).
        ///
        /// Ignored when `--output-format json` is used.
        #[arg(long, value_enum, default_value = "table")]
        pub format: OutputFormat,
    }

    #[derive(Debug, Clone, Copy, ValueEnum)]
    #[value(rename_all = "kebab-case")]
    pub enum OutputFormat {
        Table,
        Json,
    }

    fn build_report(args: &Inspect, connect_root: &Path) -> Result<QueueInspectionReport> {
        let snapshot_path = if let Some(path) = &args.snapshot {
            path.clone()
        } else {
            let sid = args
                .sid
                .as_ref()
                .ok_or_else(|| eyre!("--sid is required when --snapshot is not provided"))?;
            let root = args
                .root
                .clone()
                .unwrap_or_else(|| connect_root.to_path_buf());
            let session_dir = derive_session_dir(&root, sid)?;
            session_dir.join("state.json")
        };

        let snapshot_bytes = fs::read(&snapshot_path)
            .wrap_err_with(|| format!("failed to read snapshot {}", snapshot_path.display()))?;
        let snapshot_value: json::Value =
            json::from_slice(&snapshot_bytes).wrap_err("failed to parse snapshot JSON value")?;
        validate_snapshot_schema_keys(&snapshot_value)
            .wrap_err("failed to validate snapshot JSON schema keys")?;
        let snapshot: ConnectQueueSnapshot = json::from_value(snapshot_value)
            .wrap_err("failed to parse snapshot JSON (expected Norito schema)")?;
        if snapshot.schema_version != 1 {
            return Err(eyre!(
                "unsupported connect snapshot schema_version {}; expected 1",
                snapshot.schema_version
            ));
        }

        let session_dir = snapshot_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| eyre!("snapshot path has no parent directory"))?;
        let metrics_summary = if args.metrics {
            let metrics_path = session_dir.join("metrics.ndjson");
            read_metrics_summary(metrics_path.as_path())?
        } else {
            None
        };

        Ok(QueueInspectionReport {
            snapshot,
            metrics: metrics_summary,
            session_dir: session_dir.display().to_string(),
            state_path: snapshot_path.display().to_string(),
        })
    }

    fn render_table(report: &QueueInspectionReport) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "Session: {}", report.snapshot.session_id_base64);
        let _ = writeln!(
            out,
            "State: {}{}",
            report.snapshot.state,
            report
                .snapshot
                .reason
                .as_deref()
                .map(|reason| format!(" ({reason})"))
                .unwrap_or_default()
        );
        let _ = writeln!(
            out,
            "Watermarks: warn={:.0}% drop={:.0}%",
            report.snapshot.warning_watermark * 100.0,
            report.snapshot.drop_watermark * 100.0
        );
        let _ = writeln!(out, "Snapshot: {}", report.state_path);
        let _ = writeln!(out, "Session dir: {}", report.session_dir);
        let _ = writeln!(
            out,
            "App→Wallet depth={} bytes={}",
            report.snapshot.app_to_wallet.depth, report.snapshot.app_to_wallet.bytes
        );
        let _ = writeln!(
            out,
            "Wallet→App depth={} bytes={}",
            report.snapshot.wallet_to_app.depth, report.snapshot.wallet_to_app.bytes
        );
        if let Some(metrics) = &report.metrics {
            let _ = writeln!(
                out,
                "Metrics: samples={} last_ts={}",
                metrics.samples_total,
                metrics
                    .last_sample_ms
                    .map_or_else(|| "n/a".to_string(), |ts| ts.to_string())
            );
            let mut totals: Vec<_> = metrics.state_totals.iter().collect();
            totals.sort_by_key(|(state, _)| *state);
            for (state, count) in totals {
                let _ = writeln!(out, "  - {state}: {count}");
            }
        }
        out
    }

    fn read_metrics_summary(path: &Path) -> Result<Option<ConnectQueueMetricsSummary>> {
        if !path.exists() {
            return Ok(None);
        }
        let file =
            fs::File::open(path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut summary = ConnectQueueMetricsSummary::default();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let sample_value: json::Value = json::from_str(&line).wrap_err_with(|| {
                format!(
                    "failed to parse metrics line `{line}` in {}",
                    path.display()
                )
            })?;
            validate_metrics_sample_schema_keys(&sample_value).wrap_err_with(|| {
                format!(
                    "failed to validate metrics line `{line}` in {}",
                    path.display()
                )
            })?;
            let sample: ConnectQueueMetricsSample =
                json::from_value(sample_value).wrap_err_with(|| {
                    format!(
                        "failed to decode metrics line `{line}` in {}",
                        path.display()
                    )
                })?;
            summary.record(&sample);
        }
        Ok(Some(summary))
    }

    fn validate_snapshot_schema_keys(snapshot: &json::Value) -> Result<()> {
        let root = snapshot
            .as_object()
            .ok_or_else(|| eyre!("snapshot JSON must be an object"))?;
        for field in [
            "schema_version",
            "session_id_base64",
            "state",
            "reason",
            "warning_watermark",
            "drop_watermark",
            "last_updated_ms",
            "app_to_wallet",
            "wallet_to_app",
        ] {
            if !root.contains_key(field) {
                return Err(eyre!("snapshot missing required field `{field}`"));
            }
        }
        let app_to_wallet = root
            .get("app_to_wallet")
            .expect("checked app_to_wallet presence");
        let wallet_to_app = root
            .get("wallet_to_app")
            .expect("checked wallet_to_app presence");
        validate_direction_stats_schema_keys(app_to_wallet, "app_to_wallet")?;
        validate_direction_stats_schema_keys(wallet_to_app, "wallet_to_app")?;
        Ok(())
    }

    fn validate_direction_stats_schema_keys(value: &json::Value, label: &str) -> Result<()> {
        let object = value
            .as_object()
            .ok_or_else(|| eyre!("snapshot field `{label}` must be an object"))?;
        for field in [
            "depth",
            "bytes",
            "oldest_sequence",
            "newest_sequence",
            "oldest_timestamp_ms",
            "newest_timestamp_ms",
        ] {
            if !object.contains_key(field) {
                return Err(eyre!(
                    "snapshot field `{label}` missing required key `{field}`"
                ));
            }
        }
        Ok(())
    }

    fn validate_metrics_sample_schema_keys(sample: &json::Value) -> Result<()> {
        let object = sample
            .as_object()
            .ok_or_else(|| eyre!("metrics line must be a JSON object"))?;
        for field in [
            "timestamp_ms",
            "state",
            "app_to_wallet_depth",
            "wallet_to_app_depth",
            "reason",
        ] {
            if !object.contains_key(field) {
                return Err(eyre!("metrics line missing required key `{field}`"));
            }
        }
        Ok(())
    }

    fn derive_session_dir(root: &Path, sid: &str) -> Result<PathBuf> {
        let sid_bytes = decode_session_id(sid)?;
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sid_bytes);
        Ok(root.join(encoded))
    }

    fn decode_session_id(value: &str) -> Result<Vec<u8>> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(eyre!("sid must be non-empty"));
        }
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(trimmed)
            .map_err(|err| eyre!("sid must be base64url (no padding): {err}"))
    }

    #[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
    #[norito(rename_all = "snake_case")]
    pub struct ConnectQueueSnapshot {
        pub schema_version: u32,
        pub session_id_base64: String,
        pub state: ConnectQueueState,
        pub reason: Option<String>,
        pub warning_watermark: f64,
        pub drop_watermark: f64,
        pub last_updated_ms: u64,
        pub app_to_wallet: ConnectQueueDirectionStats,
        pub wallet_to_app: ConnectQueueDirectionStats,
    }

    #[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
    #[norito(rename_all = "snake_case")]
    pub struct ConnectQueueDirectionStats {
        pub depth: i64,
        pub bytes: i64,
        pub oldest_sequence: Option<u64>,
        pub newest_sequence: Option<u64>,
        pub oldest_timestamp_ms: Option<u64>,
        pub newest_timestamp_ms: Option<u64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ConnectQueueState {
        Healthy,
        Throttled,
        Quarantined,
        Disabled,
    }

    impl fmt::Display for ConnectQueueState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                Self::Healthy => "healthy",
                Self::Throttled => "throttled",
                Self::Quarantined => "quarantined",
                Self::Disabled => "disabled",
            })
        }
    }

    impl json::JsonSerialize for ConnectQueueState {
        fn json_serialize(&self, out: &mut String) {
            let value = match self {
                Self::Healthy => "healthy",
                Self::Throttled => "throttled",
                Self::Quarantined => "quarantined",
                Self::Disabled => "disabled",
            };
            json::write_json_string(value, out);
        }
    }

    impl json::JsonDeserialize for ConnectQueueState {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let start = parser.position();
            let raw = String::json_deserialize(parser)?;
            match raw.as_str() {
                "healthy" => Ok(Self::Healthy),
                "throttled" => Ok(Self::Throttled),
                "quarantined" => Ok(Self::Quarantined),
                "disabled" => Ok(Self::Disabled),
                _ => Err(queue_state_parse_error(parser, start)),
            }
        }
    }

    fn queue_state_parse_error(parser: &json::Parser<'_>, start: usize) -> json::Error {
        const MSG: &str = "expected \"healthy\", \"throttled\", \"quarantined\", or \"disabled\"";
        let input = parser.input();
        let clamped = start.min(input.len());
        let mut line = 1usize;
        let mut col = 1usize;
        for ch in input[..clamped].chars() {
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        json::Error::WithPos {
            msg: MSG,
            byte: clamped,
            line,
            col,
        }
    }

    #[derive(Debug, Clone, Default, JsonSerialize, JsonDeserialize)]
    #[norito(rename_all = "snake_case")]
    pub struct ConnectQueueMetricsSummary {
        pub samples_total: usize,
        #[norito(default)]
        pub state_totals: BTreeMap<ConnectQueueState, usize>,
        #[norito(default)]
        pub last_sample_ms: Option<u64>,
    }

    impl ConnectQueueMetricsSummary {
        fn record(&mut self, sample: &ConnectQueueMetricsSample) {
            self.samples_total += 1;
            *self.state_totals.entry(sample.state).or_insert(0) += 1;
            self.last_sample_ms = Some(sample.timestamp_ms);
        }
    }

    #[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
    #[norito(rename_all = "snake_case")]
    pub struct ConnectQueueMetricsSample {
        pub timestamp_ms: u64,
        pub state: ConnectQueueState,
        pub app_to_wallet_depth: i64,
        pub wallet_to_app_depth: i64,
        pub reason: Option<String>,
    }

    #[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
    #[norito(rename_all = "snake_case")]
    pub struct QueueInspectionReport {
        pub snapshot: ConnectQueueSnapshot,
        #[norito(default)]
        pub metrics: Option<ConnectQueueMetricsSummary>,
        pub session_dir: String,
        pub state_path: String,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{CliOutputFormat, RunContext};
        use iroha_i18n::{Bundle, Language, Localizer};
        use tempfile::TempDir;

        struct TestContext {
            output_format: CliOutputFormat,
            printed: Vec<String>,
            config: iroha::config::Config,
            i18n: Localizer,
        }

        impl TestContext {
            fn new(output_format: CliOutputFormat) -> Self {
                Self {
                    output_format,
                    printed: Vec::new(),
                    config: crate::fallback_config(),
                    i18n: Localizer::new(Bundle::Cli, Language::English),
                }
            }
        }

        impl RunContext for TestContext {
            fn config(&self) -> &iroha::config::Config {
                &self.config
            }

            fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
                None
            }

            fn input_instructions(&self) -> bool {
                false
            }

            fn output_instructions(&self) -> bool {
                false
            }

            fn i18n(&self) -> &Localizer {
                &self.i18n
            }

            fn output_format(&self) -> CliOutputFormat {
                self.output_format
            }

            fn print_data<T>(&mut self, data: &T) -> Result<()>
            where
                T: json::JsonSerialize + ?Sized,
            {
                let payload = json::to_json_pretty(data).expect("serialize json");
                self.printed.push(payload);
                Ok(())
            }

            fn println(&mut self, data: impl std::fmt::Display) -> Result<()> {
                self.printed.push(data.to_string());
                Ok(())
            }
        }

        fn write_snapshot(dir: &TempDir, sid: &str, snapshot: &ConnectQueueSnapshot) -> PathBuf {
            let session_dir = dir.path().join(sid);
            fs::create_dir_all(&session_dir).unwrap();
            let path = session_dir.join("state.json");
            let json_value = json::to_value(snapshot).unwrap();
            let json = json::to_string_pretty(&json_value).unwrap();
            fs::write(&path, json).unwrap();
            path
        }

        fn sample_snapshot(sid: &str) -> ConnectQueueSnapshot {
            ConnectQueueSnapshot {
                schema_version: 1,
                session_id_base64: sid.to_string(),
                state: ConnectQueueState::Healthy,
                reason: None,
                warning_watermark: 0.6,
                drop_watermark: 0.85,
                last_updated_ms: 123,
                app_to_wallet: ConnectQueueDirectionStats {
                    depth: 1,
                    bytes: 64,
                    oldest_sequence: None,
                    newest_sequence: None,
                    oldest_timestamp_ms: None,
                    newest_timestamp_ms: None,
                },
                wallet_to_app: ConnectQueueDirectionStats {
                    depth: 0,
                    bytes: 0,
                    oldest_sequence: None,
                    newest_sequence: None,
                    oldest_timestamp_ms: None,
                    newest_timestamp_ms: None,
                },
            }
        }

        #[test]
        fn inspect_reads_snapshot() {
            let tmp = TempDir::new().unwrap();
            let sid = "AQID";
            let snapshot = sample_snapshot(sid);
            let path = write_snapshot(&tmp, sid, &snapshot);
            let args = Inspect {
                sid: Some(sid.to_string()),
                snapshot: Some(path),
                root: Some(tmp.path().to_path_buf()),
                metrics: false,
                format: OutputFormat::Table,
            };
            let report = build_report(&args, tmp.path()).unwrap();
            assert_eq!(report.snapshot.state, ConnectQueueState::Healthy);
            assert!(render_table(&report).contains("Session: AQID"));
        }

        #[test]
        fn inspect_rejects_snapshot_missing_reason_field() {
            let tmp = TempDir::new().unwrap();
            let sid = "AQID";
            let path = tmp.path().join(sid).join("state.json");
            fs::create_dir_all(path.parent().expect("parent")).unwrap();
            fs::write(
                &path,
                r#"{
  "schema_version": 1,
  "session_id_base64": "AQID",
  "state": "healthy",
  "warning_watermark": 0.6,
  "drop_watermark": 0.85,
  "last_updated_ms": 123,
  "app_to_wallet": {
    "depth": 1,
    "bytes": 64,
    "oldest_sequence": null,
    "newest_sequence": null,
    "oldest_timestamp_ms": null,
    "newest_timestamp_ms": null
  },
  "wallet_to_app": {
    "depth": 0,
    "bytes": 0,
    "oldest_sequence": null,
    "newest_sequence": null,
    "oldest_timestamp_ms": null,
    "newest_timestamp_ms": null
  }
}"#,
            )
            .unwrap();
            let args = Inspect {
                sid: Some(sid.to_string()),
                snapshot: Some(path),
                root: Some(tmp.path().to_path_buf()),
                metrics: false,
                format: OutputFormat::Table,
            };
            let err = build_report(&args, tmp.path()).expect_err("missing reason field must fail");
            assert!(
                err.to_string()
                    .contains("failed to validate snapshot JSON schema keys"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn inspect_rejects_legacy_schema_version_zero() {
            let tmp = TempDir::new().unwrap();
            let sid = "AQID";
            let mut snapshot = sample_snapshot(sid);
            snapshot.schema_version = 0;
            let path = write_snapshot(&tmp, sid, &snapshot);
            let args = Inspect {
                sid: Some(sid.to_string()),
                snapshot: Some(path),
                root: Some(tmp.path().to_path_buf()),
                metrics: false,
                format: OutputFormat::Table,
            };
            let err = build_report(&args, tmp.path()).expect_err("legacy schema must be rejected");
            assert!(
                err.to_string()
                    .contains("unsupported connect snapshot schema_version 0"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn inspect_uses_connect_root_when_root_not_provided() {
            let tmp = TempDir::new().unwrap();
            let sid = "AQID";
            let snapshot = sample_snapshot(sid);
            let expected_state = snapshot.state;
            let state_path = write_snapshot(&tmp, sid, &snapshot);

            let args = Inspect {
                sid: Some(sid.to_string()),
                snapshot: None,
                root: None,
                metrics: false,
                format: OutputFormat::Table,
            };
            let report = build_report(&args, tmp.path()).unwrap();
            assert_eq!(report.snapshot.state, expected_state);
            assert_eq!(report.state_path, state_path.display().to_string());
            assert!(report.session_dir.ends_with("AQID"));
        }

        #[test]
        fn connect_queue_state_json_roundtrip() {
            for state in [
                ConnectQueueState::Healthy,
                ConnectQueueState::Throttled,
                ConnectQueueState::Quarantined,
                ConnectQueueState::Disabled,
            ] {
                let json_text = json::to_json(&state).expect("serialize queue state");
                assert_eq!(json_text, format!("\"{state}\""));
                let decoded: ConnectQueueState =
                    json::from_str(&json_text).expect("deserialize state");
                assert_eq!(decoded, state);
            }
        }

        #[test]
        fn inspect_outputs_json_in_json_mode() {
            let tmp = TempDir::new().unwrap();
            let sid = "AQID";
            let snapshot = sample_snapshot(sid);
            let path = write_snapshot(&tmp, sid, &snapshot);
            let args = Inspect {
                sid: Some(sid.to_string()),
                snapshot: Some(path),
                root: Some(tmp.path().to_path_buf()),
                metrics: false,
                format: OutputFormat::Table,
            };
            let mut ctx = TestContext::new(CliOutputFormat::Json);
            run(Command::Inspect(args), tmp.path(), &mut ctx).unwrap();
            assert_eq!(ctx.printed.len(), 1);
            let value: json::Value = json::from_str(&ctx.printed[0]).expect("json output");
            assert_eq!(
                value
                    .get("snapshot")
                    .and_then(|v| v.get("state"))
                    .and_then(|v| v.as_str()),
                Some("healthy")
            );
        }

        #[test]
        fn decode_session_id_rejects_non_base64url() {
            let err = decode_session_id("plain-text").expect_err("must reject plain text sid");
            assert!(
                err.to_string()
                    .contains("sid must be base64url (no padding)"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn read_metrics_summary_rejects_missing_required_depth_fields() {
            let tmp = TempDir::new().unwrap();
            let path = tmp.path().join("metrics.ndjson");
            fs::write(&path, "{\"timestamp_ms\":1,\"state\":\"healthy\"}\n").unwrap();
            let err = read_metrics_summary(&path).expect_err("legacy metrics shape must fail");
            assert!(
                err.to_string().contains("failed to validate metrics line"),
                "unexpected error: {err}"
            );
        }
    }
}
