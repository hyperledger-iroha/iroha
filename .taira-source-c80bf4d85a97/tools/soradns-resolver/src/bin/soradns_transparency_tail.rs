use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use eyre::{Result, WrapErr};
use norito::json;
use soradns_resolver::transparency::{TransparencyRecord, TransparencyTailer};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Convert resolver transparency logs into structured telemetry rows."
)]
struct Cli {
    /// Path to the transparency log file (defaults to stdin when omitted).
    #[arg(long)]
    input: Option<PathBuf>,
    /// Destination path for rendered output (defaults to stdout).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Output format suitable for ClickHouse ingestion.
    #[arg(long, default_value = "jsonl")]
    format: OutputFormat,
    /// Path to write Prometheus metrics text (use '-' for stdout).
    #[arg(long)]
    metrics_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Emit JSON Lines compatible with ClickHouse JSONEachRow ingestion.
    Jsonl,
    /// Emit tab-separated rows for ClickHouse TSV ingestion.
    Tsv,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let reader: Box<dyn BufRead> = match &cli.input {
        Some(path) => {
            let file = File::open(path).wrap_err("failed to open input log file")?;
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };
    let mut writer: Box<dyn Write> = match &cli.output {
        Some(path) => {
            let file = File::create(path).wrap_err("failed to create output file")?;
            Box::new(BufWriter::new(file))
        }
        None => Box::new(BufWriter::new(io::stdout())),
    };

    let mut tailer = TransparencyTailer::new();
    let writer_opt: Option<&mut dyn Write> = Some(writer.as_mut());
    process_stream(reader, writer_opt, cli.format, &mut tailer)?;
    writer.flush().wrap_err("failed to flush output writer")?;

    if let Some(target) = cli.metrics_output {
        write_metrics(&target, &tailer)?;
    }

    Ok(())
}

fn process_stream<R: BufRead>(
    mut reader: R,
    mut writer: Option<&mut dyn Write>,
    format: OutputFormat,
    tailer: &mut TransparencyTailer,
) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .wrap_err("failed reading log line")?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match tailer.ingest_line(trimmed) {
            Ok(records) => {
                if let Some(writer) = writer.as_deref_mut() {
                    for record in records {
                        write_record(writer, &record, format)?;
                    }
                }
            }
            Err(error) => {
                eprintln!("transparency parser warning: {error}");
            }
        }
    }
    Ok(())
}

fn write_metrics(path: &PathBuf, tailer: &TransparencyTailer) -> Result<()> {
    let lines = tailer.metrics();
    let mut needs_type = std::collections::HashSet::new();

    if path.as_os_str() == "-" {
        let mut out = BufWriter::new(io::stdout());
        emit_metrics(&lines, &mut needs_type, &mut out)?;
        out.flush().wrap_err("failed to flush metrics stdout")?;
        return Ok(());
    }

    let file = File::create(path).wrap_err("failed to create metrics output file")?;
    let mut out = BufWriter::new(file);
    emit_metrics(&lines, &mut needs_type, &mut out)?;
    out.flush()
        .wrap_err("failed to flush metrics output writer")?;
    Ok(())
}

fn emit_metrics<W: Write + ?Sized>(
    lines: &[soradns_resolver::transparency::MetricLine],
    seen_types: &mut std::collections::HashSet<&'static str>,
    writer: &mut W,
) -> Result<()> {
    for line in lines {
        if seen_types.insert(line.metric) {
            let metric_type = if line.metric.ends_with("_total") {
                "counter"
            } else {
                "gauge"
            };
            writeln!(writer, "# TYPE {} {}", line.metric, metric_type)
                .wrap_err("failed to write metric type line")?;
        }
        writeln!(writer, "{}", line.to_prometheus()).wrap_err("failed to write metric sample")?;
    }
    Ok(())
}

fn write_record<W: Write + ?Sized>(
    writer: &mut W,
    record: &TransparencyRecord,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Jsonl => {
            let value = record.to_json_value();
            let mut text =
                json::to_string(&value).wrap_err("failed to serialise record as JSON")?;
            text.push('\n');
            writer
                .write_all(text.as_bytes())
                .wrap_err("failed to write JSON row")?;
        }
        OutputFormat::Tsv => {
            let mut row = record.to_tsv_row();
            row.push('\n');
            writer
                .write_all(row.as_bytes())
                .wrap_err("failed to write TSV row")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use soradns_resolver::transparency::{
        BundleEventKind, BundleRecord, BundleSnapshot, ResolverEventKind, ResolverRecord,
    };

    use super::*;

    #[test]
    fn json_writer_outputs_line() {
        let mut buffer = Vec::new();
        let snapshot = BundleSnapshot {
            zone_version: 1,
            manifest_hash_hex: "bb".into(),
            policy_hash_hex: "aa".into(),
            car_root_cid: "cid".into(),
            freshness_issued_at: 37,
            freshness_expires_at: 50,
            freshness_signer: "council".into(),
            freshness_signature_hex: "ff".into(),
        };
        let record = TransparencyRecord::Bundle(Box::new(BundleRecord::new(
            42,
            "resolver-1".into(),
            "deadbeef".into(),
            &snapshot,
            BundleEventKind::Added,
            None,
        )));
        write_record(&mut buffer, &record, OutputFormat::Jsonl).unwrap();
        assert!(
            String::from_utf8(buffer)
                .unwrap()
                .contains("\"record_type\":\"bundle\""),
            "json writer should emit bundle record"
        );
    }

    #[test]
    fn tsv_writer_outputs_line() {
        let mut buffer = Vec::new();
        let record = TransparencyRecord::Resolver(ResolverRecord {
            timestamp: 99,
            resolver_id: "resolver-1".into(),
            event: ResolverEventKind::Added,
        });
        write_record(&mut buffer, &record, OutputFormat::Tsv).unwrap();
        assert!(
            String::from_utf8(buffer)
                .unwrap()
                .starts_with("resolver\t99"),
            "tsv writer should emit resolver record"
        );
    }
}
