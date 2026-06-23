//! CLI wrapper around the trustless verifier used for SoraNet gateway CAR payloads.
//! Only the `dag-scope=full` path is supported today; chunk plans and PoR roots
//! are reconstructed and can be cross-checked against registry pin records.

#![allow(unexpected_cfgs)]

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use eyre::{Context, Result};
use norito::{
    decode_from_bytes,
    json::{self, Value},
};
use sorafs_car::{TrustlessVerifier, TrustlessVerifierConfig, validate_manifest_car_replay};
use sorafs_manifest::{ManifestV1, ValidationOutcomeV1, pin_registry::PinRecordV1};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Verify SoraNet gateway CAR payloads against manifests and registry pin records.",
    propagate_version = true
)]
struct Args {
    /// Path to the manifest (`.to` or JSON).
    #[arg(long)]
    manifest: PathBuf,

    /// Path to the CAR stream to verify.
    #[arg(long)]
    car: PathBuf,

    /// Optional path to the gate config (defaults to SNNet-15 M0 pack).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Optional pin record to cross-check chunk plan + PoR roots (`.to` or JSON).
    #[arg(long)]
    pin_record: Option<PathBuf>,

    /// Write the verification summary JSON to this path (defaults to stdout).
    #[arg(long)]
    json_out: Option<PathBuf>,

    /// Emit a stable ValidationOutcomeV1 manifest/CAR replay result instead of the summary.
    #[arg(long)]
    validation_outcome: bool,

    /// Override the ValidationOutcomeV1 generated_at timestamp.
    #[arg(long)]
    generated_at: Option<u64>,

    /// Suppress stdout output (useful when `--json-out` is set).
    #[arg(long)]
    quiet: bool,
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("error: {err:?}");
            process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let args = Args::parse();
    let config_path = args.config.clone().unwrap_or_else(default_config_path);
    let config = TrustlessVerifierConfig::from_file(&config_path)?;

    let manifest = load_manifest(&args.manifest)?;
    let car_bytes = fs::read(&args.car)
        .wrap_err_with(|| format!("failed to read CAR `{}`", args.car.display()))?;

    if args.validation_outcome {
        if args.pin_record.is_some() {
            return Err(eyre::eyre!(
                "--validation-outcome emits manifest/CAR replay outcomes; omit --pin-record or use summary mode for pin-record validation"
            ));
        }
        let generated_at = match args.generated_at {
            Some(generated_at) => generated_at,
            None => unix_time_now()
                .wrap_err("failed to derive ValidationOutcomeV1 generated_at timestamp")?,
        };
        let outcome = validate_manifest_car_replay(
            &manifest,
            &car_bytes,
            args.manifest.display().to_string(),
            args.car.display().to_string(),
            &config,
            generated_at,
        );
        emit_validation_outcome(&args, &outcome)?;
        return Ok(if outcome.is_ok() { 0 } else { 2 });
    }

    let verifier = TrustlessVerifier::new(config);
    let outcome = verifier
        .verify_full(&manifest, &car_bytes)
        .map_err(|err| eyre::eyre!(err))?;

    if let Some(pin_path) = args.pin_record.as_ref() {
        let pin_record = load_pin_record(pin_path)?;
        outcome
            .validate_pin_record(&pin_record)
            .map_err(|err| eyre::eyre!(err))?;
    }

    let mut summary = outcome.to_summary_json();
    if let Some(object) = summary.as_object_mut() {
        object.insert(
            "config_path".into(),
            Value::from(config_path.display().to_string()),
        );
        object.insert(
            "manifest_path".into(),
            Value::from(args.manifest.display().to_string()),
        );
        object.insert(
            "car_path".into(),
            Value::from(args.car.display().to_string()),
        );
    }

    if let Some(out) = args.json_out.as_ref() {
        let file = fs::File::create(out)
            .wrap_err_with(|| format!("failed to create JSON output `{}`", out.display()))?;
        json::to_writer_pretty(file, &summary)
            .wrap_err_with(|| format!("failed to write JSON output `{}`", out.display()))?;
    } else if !args.quiet {
        println!(
            "{}",
            json::to_string_pretty(&summary).expect("summary serialization")
        );
    }

    Ok(0)
}

fn emit_validation_outcome(args: &Args, outcome: &ValidationOutcomeV1) -> Result<()> {
    let rendered = json::to_string_pretty(outcome).wrap_err("failed to render outcome JSON")?;
    if let Some(out) = args.json_out.as_ref() {
        fs::write(out, format!("{rendered}\n"))
            .wrap_err_with(|| format!("failed to write JSON output `{}`", out.display()))?;
    } else if !args.quiet {
        println!("{rendered}");
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<ManifestV1> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read manifest `{}`", path.display()))?;
    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        let text = String::from_utf8(bytes)
            .wrap_err_with(|| format!("manifest at `{}` is not valid UTF-8", path.display()))?;
        return json::from_json(&text)
            .wrap_err_with(|| format!("failed to parse manifest JSON `{}`", path.display()));
    }
    decode_from_bytes(&bytes)
        .wrap_err_with(|| format!("failed to decode manifest bytes `{}`", path.display()))
}

fn load_pin_record(path: &Path) -> Result<PinRecordV1> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read pin record `{}`", path.display()))?;
    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        let text = String::from_utf8(bytes)
            .wrap_err_with(|| format!("pin record at `{}` is not valid UTF-8", path.display()))?;
        return json::from_json(&text)
            .wrap_err_with(|| format!("failed to parse pin record JSON `{}`", path.display()));
    }
    decode_from_bytes(&bytes)
        .wrap_err_with(|| format!("failed to decode pin record `{}`", path.display()))
}

fn default_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../configs/soranet/gateway_m0/gateway_trustless_verifier.toml")
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system time is before the UNIX epoch")?
        .as_secs())
}
