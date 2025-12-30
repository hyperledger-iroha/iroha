//! Minimal Taikai viewer harness used for SN13-F/SN13-G validation.
//!
//! The tool validates Taikai segment envelopes against CAR archives, records
//! playback telemetry (segments, rebuffer events, CEK fetch/rotation, PQ
//! health), and emits Prometheus text along with an optional JSON summary.
//! Multiple renditions can be supplied via repeated `--segment` flags to cover
//! ABR ladders in one run.
#![allow(unexpected_cfgs)]

use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use blake3::hash as blake3_hash;
use iroha_data_model::taikai::{CekRotationReceiptV1, TaikaiSegmentEnvelopeV1};
use iroha_telemetry::metrics::Metrics;
use norito::{
    decode_from_bytes, json,
    json::{Map, Value},
};

const USAGE: &str = "\
taikai_viewer --segment envelope=PATH,car=PATH [--segment ...] [--cluster LABEL] [--lane LABEL]
              [--rebuffer-events N] [--pq-health PCT] [--cek-receipt PATH] [--cek-fetch-ms N]
              [--alert ALERTNAME ...] [--metrics-out PATH] [--summary-out PATH]
";

#[derive(Debug)]
struct SegmentInput {
    envelope: PathBuf,
    car: PathBuf,
}

#[derive(Debug, Default)]
struct StreamStats {
    segments: u64,
    rebuffer_events: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("taikai_viewer: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    if args.segments.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one --segment=... entry is required",
        )
        .into());
    }

    let metrics = Metrics::default();
    let mut summaries: Vec<Value> = Vec::new();
    let mut stream_stats: HashMap<String, StreamStats> = HashMap::new();
    let mut stream_order: Vec<String> = Vec::new();

    for segment in &args.segments {
        let envelope = load_envelope(&segment.envelope)?;
        let car_bytes = fs::read(&segment.car)?;
        validate_car(&envelope, &car_bytes, &segment.car)?;

        let render_name = envelope.rendition_id.to_string();
        let stream = envelope.stream_id.to_string();
        if !stream_stats.contains_key(&stream) {
            stream_order.push(stream.clone());
        }
        let stats = stream_stats.entry(stream.clone()).or_default();
        stats.segments += 1;

        let ingest = &envelope.ingest;
        let instrumentation = &envelope.instrumentation;
        let mut entry = Map::new();
        entry.insert("event".into(), Value::from(envelope.event_id.to_string()));
        entry.insert("stream".into(), Value::from(stream));
        entry.insert("rendition".into(), Value::from(render_name));
        entry.insert("sequence".into(), Value::from(envelope.segment_sequence));
        entry.insert(
            "car_path".into(),
            Value::from(segment.car.to_string_lossy().into_owned()),
        );
        entry.insert(
            "car_digest_hex".into(),
            Value::from(hex::encode(ingest.car.car_digest.as_bytes())),
        );
        entry.insert(
            "car_size_bytes".into(),
            Value::from(ingest.car.car_size_bytes),
        );
        entry.insert(
            "chunk_count".into(),
            Value::from(u64::from(ingest.chunk_count)),
        );
        if let Some(latency) = instrumentation.encoder_to_ingest_latency_ms {
            entry.insert("encoder_to_ingest_latency_ms".into(), Value::from(latency));
        }
        if let Some(drift) = instrumentation.live_edge_drift_ms {
            entry.insert("live_edge_drift_ms".into(), Value::from(drift));
        }
        summaries.push(Value::Object(entry));
    }

    if let Some(first_stream) = stream_order.first().cloned()
        && let Some(first_stats) = stream_stats.get_mut(&first_stream)
    {
        first_stats.rebuffer_events = args.rebuffer_events;
        if args.rebuffer_events > 0 {
            metrics.inc_taikai_viewer_rebuffer(&args.cluster, &first_stream, args.rebuffer_events);
        }
    }

    for (stream, stats) in &stream_stats {
        metrics.inc_taikai_viewer_segments(&args.cluster, stream, stats.segments);
    }

    metrics.set_taikai_viewer_pq_health(&args.cluster, args.pq_health);

    let mut cek_summary: Option<Map> = None;
    if let Some(path) = args.cek_receipt.as_ref() {
        let (measured_ms, age_seconds) = read_cek_receipt(path)?;
        let applied_ms = args.cek_fetch_ms.unwrap_or(measured_ms);
        metrics.observe_taikai_viewer_cek_fetch_duration(&args.cluster, &args.lane, applied_ms);
        metrics.set_taikai_viewer_cek_rotation_age(&args.lane, age_seconds);
        let mut cek = Map::new();
        cek.insert(
            "path".into(),
            Value::from(path.to_string_lossy().into_owned()),
        );
        cek.insert("duration_ms".into(), Value::from(u64::from(applied_ms)));
        cek.insert(
            "measured_duration_ms".into(),
            Value::from(u64::from(measured_ms)),
        );
        cek.insert("rotation_age_seconds".into(), Value::from(age_seconds));
        cek_summary = Some(cek);
    }

    for alertname in &args.alerts {
        metrics.inc_taikai_viewer_alert_firing(&args.cluster, alertname);
    }

    let metrics_text = metrics.try_to_string()?;
    if let Some(path) = args.metrics_out.as_ref() {
        fs::write(path, &metrics_text)?;
    } else {
        println!("{metrics_text}");
    }

    if let Some(path) = args.summary_out.as_ref() {
        let mut root = Map::new();
        root.insert("cluster".into(), Value::from(args.cluster));
        root.insert("lane".into(), Value::from(args.lane));
        root.insert("pq_health_percent".into(), Value::from(args.pq_health));
        root.insert(
            "rebuffer_events_applied".into(),
            Value::from(args.rebuffer_events),
        );
        if let Some(cek) = cek_summary {
            root.insert("cek".into(), Value::Object(cek));
        }
        if let Some(path) = args.metrics_out.as_ref() {
            root.insert(
                "metrics_out".into(),
                Value::from(path.to_string_lossy().into_owned()),
            );
        }
        if !args.alerts.is_empty() {
            root.insert(
                "alerts".into(),
                Value::Array(args.alerts.iter().cloned().map(Value::from).collect()),
            );
        }
        root.insert("segments".into(), Value::Array(summaries));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        json::to_writer_pretty(file, &Value::Object(root))?;
    }

    Ok(())
}

fn load_envelope(path: &Path) -> Result<TaikaiSegmentEnvelopeV1, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    decode_from_bytes(&bytes)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()).into())
}

fn validate_car(
    envelope: &TaikaiSegmentEnvelopeV1,
    car_bytes: &[u8],
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let digest = blake3_hash(car_bytes);
    let expected = envelope.ingest.car.car_digest.as_bytes();
    if digest.as_bytes() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CAR digest mismatch for {} (expected {}, got {})",
                path.display(),
                hex::encode(expected),
                hex::encode(digest.as_bytes())
            ),
        )
        .into());
    }
    let actual_size = car_bytes.len() as u64;
    if actual_size != envelope.ingest.car.car_size_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CAR size mismatch for {} (expected {} bytes, got {} bytes)",
                path.display(),
                envelope.ingest.car.car_size_bytes,
                actual_size
            ),
        )
        .into());
    }
    Ok(())
}

fn read_cek_receipt(path: &Path) -> Result<(u32, u64), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let bytes = fs::read(path)?;
    let decode_start = Instant::now();
    let receipt: CekRotationReceiptV1 = decode_from_bytes(&bytes)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let decode_elapsed = decode_start.elapsed();
    let duration_ms = start.elapsed().as_millis();
    let observed_ms = duration_ms.max(decode_elapsed.as_millis());
    let clamped_ms = observed_ms.min(u128::from(u32::MAX)) as u32;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(receipt.issued_at_unix);
    Ok((clamped_ms, age))
}

struct ParsedArgs {
    cluster: String,
    lane: String,
    rebuffer_events: u64,
    pq_health: f64,
    cek_receipt: Option<PathBuf>,
    cek_fetch_ms: Option<u32>,
    metrics_out: Option<PathBuf>,
    summary_out: Option<PathBuf>,
    segments: Vec<SegmentInput>,
    alerts: Vec<String>,
}

fn parse_args() -> Result<ParsedArgs, Box<dyn std::error::Error>> {
    let mut cluster = String::from("local");
    let mut lane = String::from("lane-a");
    let mut rebuffer_events: u64 = 0;
    let mut pq_health: f64 = 100.0;
    let mut cek_receipt: Option<PathBuf> = None;
    let mut cek_fetch_ms: Option<u32> = None;
    let mut metrics_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut segments = Vec::new();
    let mut alerts = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            flag if flag.starts_with("--segment=") => {
                let value = flag.trim_start_matches("--segment=");
                segments.push(parse_segment(value)?);
            }
            "--segment" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--segment requires envelope=...,car=... value",
                    )
                })?;
                segments.push(parse_segment(&value)?);
            }
            flag if flag.starts_with("--cluster=") => {
                cluster = flag.trim_start_matches("--cluster=").to_string()
            }
            "--cluster" => {
                cluster = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing cluster")
                })?;
            }
            flag if flag.starts_with("--lane=") => {
                lane = flag.trim_start_matches("--lane=").to_string()
            }
            "--lane" => {
                lane = args
                    .next()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing lane"))?;
            }
            flag if flag.starts_with("--rebuffer-events=") => {
                rebuffer_events = flag
                    .trim_start_matches("--rebuffer-events=")
                    .parse::<u64>()?
            }
            "--rebuffer-events" => {
                rebuffer_events = args
                    .next()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "missing value for --rebuffer-events",
                        )
                    })?
                    .parse::<u64>()?;
            }
            flag if flag.starts_with("--pq-health=") => {
                pq_health = flag.trim_start_matches("--pq-health=").parse::<f64>()?;
            }
            "--pq-health" => {
                pq_health = args
                    .next()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "missing --pq-health value")
                    })?
                    .parse::<f64>()?;
            }
            flag if flag.starts_with("--cek-receipt=") => {
                cek_receipt = Some(PathBuf::from(flag.trim_start_matches("--cek-receipt=")));
            }
            "--cek-receipt" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "missing value for --cek-receipt",
                    )
                })?;
                cek_receipt = Some(PathBuf::from(value));
            }
            flag if flag.starts_with("--cek-fetch-ms=") => {
                cek_fetch_ms = Some(flag.trim_start_matches("--cek-fetch-ms=").parse::<u32>()?);
            }
            "--cek-fetch-ms" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "missing value for --cek-fetch-ms",
                    )
                })?;
                cek_fetch_ms = Some(value.parse::<u32>()?);
            }
            flag if flag.starts_with("--metrics-out=") => {
                metrics_out = Some(PathBuf::from(flag.trim_start_matches("--metrics-out=")));
            }
            "--metrics-out" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "missing path for --metrics-out",
                    )
                })?;
                metrics_out = Some(PathBuf::from(value));
            }
            flag if flag.starts_with("--summary-out=") => {
                summary_out = Some(PathBuf::from(flag.trim_start_matches("--summary-out=")));
            }
            "--summary-out" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "missing path for --summary-out",
                    )
                })?;
                summary_out = Some(PathBuf::from(value));
            }
            flag if flag.starts_with("--alert=") => {
                alerts.push(flag.trim_start_matches("--alert=").to_string());
            }
            "--alert" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing value for --alert")
                })?;
                alerts.push(value);
            }
            unknown => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unrecognised argument `{unknown}`\n\n{USAGE}"),
                )
                .into());
            }
        }
    }

    Ok(ParsedArgs {
        cluster,
        lane,
        rebuffer_events,
        pq_health,
        cek_receipt,
        cek_fetch_ms,
        metrics_out,
        summary_out,
        segments,
        alerts,
    })
}

fn parse_segment(raw: &str) -> Result<SegmentInput, Box<dyn std::error::Error>> {
    let mut envelope = None;
    let mut car = None;
    for part in raw.split(',') {
        if let Some(rest) = part.strip_prefix("envelope=") {
            envelope = Some(PathBuf::from(rest));
        } else if let Some(rest) = part.strip_prefix("car=") {
            car = Some(PathBuf::from(rest));
        }
    }
    let envelope = envelope.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "segment missing envelope=PATH component",
        )
    })?;
    let car = car.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "segment missing car=PATH component",
        )
    })?;
    Ok(SegmentInput { envelope, car })
}
