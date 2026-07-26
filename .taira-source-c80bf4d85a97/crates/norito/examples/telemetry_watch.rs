//! Periodically print aggregated Norito telemetry snapshots and deltas.
//!
//! Run:
//!   cargo run -p norito --example telemetry_watch
//! With timings/logs:
//!   cargo run -p norito --example telemetry_watch --features adaptive-telemetry,adaptive-telemetry-log

use std::{
    thread,
    time::{Duration, Instant},
};

fn main() {
    let config = match CliConfig::from_env() {
        Ok(cfg) => cfg,
        Err(CliConfigError::Help(msg)) => {
            println!("{msg}");
            return;
        }
        Err(CliConfigError::Message(msg)) => {
            let usage = CliConfig::usage();
            eprintln!("error: {msg}\n\n{usage}");
            std::process::exit(1);
        }
    };

    run(config);
}

fn run(config: CliConfig) {
    for _ in 0..config.warmup_rounds {
        touch_buckets(&config.buckets);
    }

    #[cfg(feature = "json")]
    {
        let mut last = norito::telemetry::snapshot_json_value();
        filter_buckets(&mut last, &config.buckets);
        emit_line("init", &last, config.json_lines);
        loop {
            let start = Instant::now();
            touch_buckets(&config.buckets);

            let mut snap = norito::telemetry::snapshot_json_value();
            filter_buckets(&mut snap, &config.buckets);
            let mut delta = norito::telemetry::snapshot_delta_json(&last, &snap);
            filter_buckets(&mut delta, &config.buckets);
            emit_line("delta", &delta, config.json_lines);
            last = snap;

            let spent = start.elapsed();
            if spent < config.interval {
                thread::sleep(config.interval - spent);
            }
        }
    }

    #[cfg(not(feature = "json"))]
    {
        let _ = config;
        eprintln!("telemetry_watch requires the `json` feature");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliConfig {
    interval: Duration,
    warmup_rounds: u32,
    buckets: BucketSelection,
    json_lines: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BucketSelection {
    columnar: bool,
    codec: bool,
    compression: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliConfigError {
    Help(String),
    Message(String),
}

impl CliConfig {
    fn from_env() -> Result<Self, CliConfigError> {
        Self::from_iter(std::env::args())
    }

    fn from_iter<I>(args: I) -> Result<Self, CliConfigError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut iter = args.into_iter();
        let _ = iter.next();

        let mut config = Self::default();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    return Err(CliConfigError::Help(Self::usage()));
                }
                "--json-lines" => {
                    config.json_lines = true;
                }
                "--interval-ms" => {
                    let value = iter.next().ok_or_else(|| {
                        CliConfigError::Message(String::from("--interval-ms requires a value"))
                    })?;
                    config.interval = parse_interval_ms(&value)?;
                }
                "--warmup" => {
                    let value = iter.next().ok_or_else(|| {
                        CliConfigError::Message(String::from("--warmup requires a value"))
                    })?;
                    config.warmup_rounds = parse_u32(&value, "--warmup")?;
                }
                _ => {
                    if let Some(rest) = arg.strip_prefix("--interval-ms=") {
                        config.interval = parse_interval_ms(rest)?;
                    } else if let Some(rest) = arg.strip_prefix("--warmup=") {
                        config.warmup_rounds = parse_u32(rest, "--warmup")?;
                    } else if let Some(rest) = arg.strip_prefix("--buckets=") {
                        config.buckets = BucketSelection::from_csv(rest)?;
                    } else if arg == "--buckets" {
                        let value = iter.next().ok_or_else(|| {
                            CliConfigError::Message(String::from("--buckets requires a value"))
                        })?;
                        config.buckets = BucketSelection::from_csv(&value)?;
                    } else {
                        return Err(CliConfigError::Message(format!("unknown option: {arg}")));
                    }
                }
            }
        }

        if !config.buckets.any_selected() {
            return Err(CliConfigError::Message(String::from(
                "--buckets must include at least one of columnar, codec, compression",
            )));
        }

        Ok(config)
    }

    fn usage() -> String {
        let bucket_options = if cfg!(feature = "columnar") {
            "columnar, codec, compression"
        } else {
            "codec, compression"
        };
        format!(
            "Usage: telemetry_watch [OPTIONS]\n\nOPTIONS:\n  --interval-ms <ms>    Set sampling interval in milliseconds (default 1000)\n  --warmup <count>      Run warmup touches this many times (default 1)\n  --buckets <list>      Comma-separated subset of {{{bucket_options}}}\n  --json-lines          Emit raw JSON lines without labels\n  -h, --help            Show this message\n"
        )
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(1000),
            warmup_rounds: 1,
            buckets: BucketSelection::default(),
            json_lines: false,
        }
    }
}

impl BucketSelection {
    fn default() -> Self {
        Self {
            columnar: cfg!(feature = "columnar"),
            codec: true,
            compression: true,
        }
    }

    fn none() -> Self {
        Self {
            columnar: false,
            codec: false,
            compression: false,
        }
    }

    fn from_csv(value: &str) -> Result<Self, CliConfigError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(Self::none());
        }
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(Self::default());
        }
        let mut selection = Self::none();
        for token in value.split(',') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.to_ascii_lowercase().as_str() {
                "columnar" => {
                    if cfg!(feature = "columnar") {
                        selection.columnar = true;
                    } else {
                        return Err(CliConfigError::Message(String::from(
                            "columnar bucket requires building with the 'columnar' feature",
                        )));
                    }
                }
                "codec" => {
                    selection.codec = true;
                }
                "compression" => {
                    selection.compression = true;
                }
                other => {
                    return Err(CliConfigError::Message(format!(
                        "unknown bucket '{other}'; expected columnar, codec, or compression"
                    )));
                }
            }
        }
        Ok(selection)
    }

    fn any_selected(&self) -> bool {
        self.columnar || self.codec || self.compression
    }

    fn columnar_enabled(&self) -> bool {
        cfg!(feature = "columnar") && self.columnar
    }
}

fn parse_interval_ms(value: &str) -> Result<Duration, CliConfigError> {
    let millis = value.parse::<u64>().map_err(|_| {
        CliConfigError::Message(format!("--interval-ms expects an integer, got '{value}'"))
    })?;
    Ok(Duration::from_millis(millis))
}

fn parse_u32(value: &str, flag: &str) -> Result<u32, CliConfigError> {
    value.parse::<u32>().map_err(|_| {
        CliConfigError::Message(format!(
            "{flag} expects a non-negative integer, got '{value}'"
        ))
    })
}

fn touch_buckets(selection: &BucketSelection) {
    #[cfg(feature = "columnar")]
    if selection.columnar_enabled() {
        let rows: Vec<(u64, &str, bool)> = vec![(1, "warmup", true), (2, "steady", false)];
        let _ = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);
    }
    if selection.codec {
        let _ = norito::codec::encode_adaptive(&vec![1u32, 2, 3]);
    }
    if selection.compression {
        let _ = norito::core::to_bytes_auto(&vec![0u8; 256]);
    }
}

#[cfg(feature = "json")]
fn filter_buckets(value: &mut norito::json::Value, selection: &BucketSelection) {
    if let Some(obj) = value.as_object_mut() {
        if !selection.columnar_enabled() {
            obj.remove("columnar");
        }
        if !selection.codec {
            obj.remove("codec");
        }
        if !selection.compression {
            obj.remove("compression");
        }
    }
}

#[cfg(feature = "json")]
fn emit_line(kind: &str, value: &norito::json::Value, json_lines: bool) {
    let json = norito::json::to_json(value).unwrap();
    if json_lines {
        println!("{json}");
    } else if kind == "init" {
        println!("init: {json}");
    } else {
        println!("delta: {json}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliConfig, CliConfigError> {
        CliConfig::from_iter(args.iter().map(|s| String::from(*s)))
    }

    #[test]
    fn defaults_match_expectations() {
        let cfg = parse(&["telemetry_watch"]).unwrap();
        assert_eq!(cfg.interval, Duration::from_millis(1000));
        assert_eq!(cfg.warmup_rounds, 1);
        assert!(cfg.buckets.codec);
        assert!(cfg.buckets.compression);
        assert!(!cfg.json_lines);
    }

    #[test]
    fn parses_interval_and_warmup() {
        let cfg = parse(&["telemetry_watch", "--interval-ms", "250", "--warmup", "5"]).unwrap();
        assert_eq!(cfg.interval, Duration::from_millis(250));
        assert_eq!(cfg.warmup_rounds, 5);
    }

    #[test]
    fn rejects_unknown_flag() {
        let err = parse(&["telemetry_watch", "--no-such"]).unwrap_err();
        assert!(matches!(err, CliConfigError::Message(_)));
    }

    #[test]
    fn parses_json_lines_flag() {
        let cfg = parse(&["telemetry_watch", "--json-lines"]).unwrap();
        assert!(cfg.json_lines);
    }

    #[test]
    fn parses_bucket_subset() {
        let cfg = parse(&["telemetry_watch", "--buckets", "codec"]).unwrap();
        assert!(cfg.buckets.codec);
        assert!(!cfg.buckets.compression);
        assert!(!cfg.buckets.columnar);
    }

    #[test]
    fn parses_all_alias() {
        let cfg = parse(&["telemetry_watch", "--buckets", "all"]).unwrap();
        assert!(cfg.buckets.codec);
        assert!(cfg.buckets.compression);
        if cfg!(feature = "columnar") {
            assert!(cfg.buckets.columnar);
        } else {
            assert!(!cfg.buckets.columnar);
        }
    }

    #[test]
    fn rejects_empty_buckets() {
        let err = parse(&["telemetry_watch", "--buckets", " "]).unwrap_err();
        assert!(matches!(err, CliConfigError::Message(_)));
    }
}
