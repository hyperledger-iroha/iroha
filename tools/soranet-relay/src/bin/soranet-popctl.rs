//! Bounded provisioning and promotion checks for SoraNet points of presence.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use clap::{Args, Parser, Subcommand};
use norito::{DecodeLimits, json};
use soranet_relay::{
    config::read_bounded_direct_regular_file,
    popctl::{
        AttestationError, HealthError, HealthReport, HealthState, PopConfig, PopValidationError,
        PxeEvent, PxeLogError, SigstoreBundle, TemplateOptions, build_template, evaluate_health,
        validate_config, verify_attestation, verify_pxe_log,
    },
};
// First-release local-input corridors. Raw bytes are bounded before allocation,
// lexical profiles are collected without allocation, and typed decoding runs
// under matching field/count/allocation/depth limits.
const POP_CONFIG_JSON_MAX_BYTES_V1: usize = 1024 * 1024;
const POP_CONFIG_JSON_MAX_FIELD_BYTES_V1: usize = 64 * 1024;
const POP_CONFIG_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 768 * 1024;
const POP_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 4_096;
const POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 32_768;
const POP_CONFIG_JSON_MAX_ALLOCATED_BYTES_V1: usize = 8 * 1024 * 1024;
const POP_CONFIG_JSON_MAX_DEPTH_V1: usize = 16;
const HEALTH_REPORT_JSON_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const HEALTH_REPORT_JSON_MAX_FIELD_BYTES_V1: usize = 16 * 1024;
const HEALTH_REPORT_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 3 * 1024 * 1024;
const HEALTH_REPORT_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 8_192;
const HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 65_536;
const HEALTH_REPORT_JSON_MAX_ALLOCATED_BYTES_V1: usize = 32 * 1024 * 1024;
const HEALTH_REPORT_JSON_MAX_DEPTH_V1: usize = 8;
const ATTESTATION_JSON_MAX_BYTES_V1: usize = 512 * 1024;
const ATTESTATION_JSON_MAX_FIELD_BYTES_V1: usize = 16 * 1024;
const ATTESTATION_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 384 * 1024;
const ATTESTATION_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 2_048;
const ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 4_096;
const ATTESTATION_JSON_MAX_ALLOCATED_BYTES_V1: usize = 4 * 1024 * 1024;
const ATTESTATION_JSON_MAX_DEPTH_V1: usize = 4;
const PXE_LOG_JSON_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
const PXE_LOG_JSON_MAX_FIELD_BYTES_V1: usize = 16 * 1024;
const PXE_LOG_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 6 * 1024 * 1024;
const PXE_LOG_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 8_192;
const PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 65_536;
const PXE_LOG_JSON_MAX_ALLOCATED_BYTES_V1: usize = 64 * 1024 * 1024;
const PXE_LOG_JSON_MAX_DEPTH_V1: usize = 4;
const POP_CONFIG_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    POP_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    POP_CONFIG_JSON_MAX_FIELD_BYTES_V1,
    POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
    POP_CONFIG_JSON_MAX_ALLOCATED_BYTES_V1,
    POP_CONFIG_JSON_MAX_DEPTH_V1,
);
const HEALTH_REPORT_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    HEALTH_REPORT_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    HEALTH_REPORT_JSON_MAX_FIELD_BYTES_V1,
    HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1,
    HEALTH_REPORT_JSON_MAX_ALLOCATED_BYTES_V1,
    HEALTH_REPORT_JSON_MAX_DEPTH_V1,
);
const ATTESTATION_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    ATTESTATION_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    ATTESTATION_JSON_MAX_FIELD_BYTES_V1,
    ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1,
    ATTESTATION_JSON_MAX_ALLOCATED_BYTES_V1,
    ATTESTATION_JSON_MAX_DEPTH_V1,
);
const PXE_LOG_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    PXE_LOG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    PXE_LOG_JSON_MAX_FIELD_BYTES_V1,
    PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1,
    PXE_LOG_JSON_MAX_ALLOCATED_BYTES_V1,
    PXE_LOG_JSON_MAX_DEPTH_V1,
);
const fn pop_config_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        POP_CONFIG_JSON_MAX_BYTES_V1,
        POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1 + 1,
        POP_CONFIG_JSON_MAX_BYTES_V1,
        POP_CONFIG_JSON_MAX_FIELD_BYTES_V1,
        POP_CONFIG_JSON_MAX_TOTAL_STRING_BYTES_V1,
        POP_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        POP_CONFIG_JSON_MAX_TOTAL_ELEMENTS_V1,
        POP_CONFIG_JSON_MAX_DEPTH_V1,
    )
}
const fn health_report_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        HEALTH_REPORT_JSON_MAX_BYTES_V1,
        HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1 + 1,
        HEALTH_REPORT_JSON_MAX_BYTES_V1,
        HEALTH_REPORT_JSON_MAX_FIELD_BYTES_V1,
        HEALTH_REPORT_JSON_MAX_TOTAL_STRING_BYTES_V1,
        HEALTH_REPORT_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1,
        HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1,
        HEALTH_REPORT_JSON_MAX_TOTAL_ELEMENTS_V1,
        HEALTH_REPORT_JSON_MAX_DEPTH_V1,
    )
}
const fn attestation_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        ATTESTATION_JSON_MAX_BYTES_V1,
        ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1 + 1,
        ATTESTATION_JSON_MAX_BYTES_V1,
        ATTESTATION_JSON_MAX_FIELD_BYTES_V1,
        ATTESTATION_JSON_MAX_TOTAL_STRING_BYTES_V1,
        ATTESTATION_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1,
        ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1,
        ATTESTATION_JSON_MAX_TOTAL_ELEMENTS_V1,
        ATTESTATION_JSON_MAX_DEPTH_V1,
    )
}
const fn pxe_log_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        PXE_LOG_JSON_MAX_BYTES_V1,
        PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1 + 1,
        PXE_LOG_JSON_MAX_BYTES_V1,
        PXE_LOG_JSON_MAX_FIELD_BYTES_V1,
        PXE_LOG_JSON_MAX_TOTAL_STRING_BYTES_V1,
        PXE_LOG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1,
        PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1,
        PXE_LOG_JSON_MAX_TOTAL_ELEMENTS_V1,
        PXE_LOG_JSON_MAX_DEPTH_V1,
    )
}
#[derive(Parser, Debug)]
#[command(
    name = "soranet-popctl",
    version,
    about = "Provisioning helper for SoraNet PoP automation and health checks"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a PoP configuration template.
    Template(TemplateArgs),
    /// Validate a PoP configuration file.
    Validate(ValidateArgs),
    /// Evaluate a health report against the PoP configuration.
    Health(HealthArgs),
    /// Verify sigstore attestations and PXE execution logs before promotion.
    Attest(AttestArgs),
}
#[derive(Args, Debug)]
struct TemplateArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    region: Option<String>,
    #[arg(long)]
    environment: Option<String>,
    #[arg(long)]
    asn: Option<u32>,
    #[arg(long, value_delimiter = ',')]
    anycast_ipv4: Option<Vec<String>>,
    #[arg(long, value_delimiter = ',')]
    anycast_ipv6: Option<Vec<String>>,
    #[arg(long)]
    control_plane_image: Option<String>,
    #[arg(long)]
    edge_image: Option<String>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    overwrite: bool,
}
#[derive(Args, Debug)]
struct ValidateArgs {
    #[arg(long)]
    config: PathBuf,
}
#[derive(Args, Debug)]
struct HealthArgs {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    allow_degraded: bool,
}
#[derive(Args, Debug)]
struct AttestArgs {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    bundle: PathBuf,
    #[arg(long)]
    pxe_log: PathBuf,
}
fn main() {
    if let Err(err) = run() {
        eprintln!("soranet-popctl error: {err}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Template(args) => command_template(&args),
        Command::Validate(args) => command_validate(&args),
        Command::Health(args) => command_health(&args),
        Command::Attest(args) => command_attest(&args),
    }
}
fn command_template(args: &TemplateArgs) -> Result<(), String> {
    let mut options = TemplateOptions::default();
    if let Some(name) = &args.name {
        options.name = name.clone();
    }
    if let Some(region) = &args.region {
        options.region = region.clone();
    }
    if let Some(environment) = &args.environment {
        options.environment = environment.clone();
    }
    if let Some(asn) = args.asn {
        options.asn = asn;
    }
    if let Some(control_plane_image) = &args.control_plane_image {
        options.control_plane_image = control_plane_image.clone();
    }
    if let Some(edge_image) = &args.edge_image {
        options.edge_image = edge_image.clone();
    }
    if let Some(prefixes) = &args.anycast_ipv4 {
        options.anycast_ipv4 = prefixes
            .iter()
            .map(|item| item.trim().to_string())
            .collect();
    }
    if let Some(prefixes) = &args.anycast_ipv6 {
        options.anycast_ipv6 = prefixes
            .iter()
            .map(|item| item.trim().to_string())
            .collect();
    }
    let config = build_template(&options);
    write_config(args.out.as_deref(), args.overwrite, &config)?;
    if let Some(path) = &args.out {
        println!("Template written to {}", path.display());
    }
    Ok(())
}
fn command_validate(args: &ValidateArgs) -> Result<(), String> {
    read_config(&args.config)?;
    println!("Configuration `{}` is valid", args.config.display());
    Ok(())
}
fn command_health(args: &HealthArgs) -> Result<(), String> {
    let config = read_config(&args.config)?;
    let report = read_health_report(&args.report)?;
    let summary = evaluate_health(&config, &report).map_err(|err| format_health_error(&err))?;
    println!("Health report timestamp: {}", report.generated_at);
    println!("Overall status: {}", summary.overall_status.as_str());
    if !summary.failed_checks.is_empty() {
        println!("Failed checks:");
        for failed in &summary.failed_checks {
            match &failed.message {
                Some(message) => println!(
                    "  - {}::{} => {} ({message})",
                    failed.service,
                    failed.check,
                    failed.status.as_str()
                ),
                None => println!(
                    "  - {}::{} => {}",
                    failed.service,
                    failed.check,
                    failed.status.as_str()
                ),
            }
        }
    }
    if !summary.missing_checks.is_empty() {
        println!("Missing checks:");
        for missing in &summary.missing_checks {
            println!("  - {}::{}", missing.service, missing.check);
        }
    }
    let should_fail = match summary.overall_status {
        HealthState::Healthy => false,
        HealthState::Degraded => !args.allow_degraded,
        HealthState::Unhealthy => true,
    };
    if should_fail {
        Err(format!(
            "health status is {}, refusing promotion",
            summary.overall_status.as_str()
        ))
    } else {
        Ok(())
    }
}
fn command_attest(args: &AttestArgs) -> Result<(), String> {
    let config = read_config(&args.config)?;
    let bundle = read_attestation_bundle(&args.bundle)?;
    let events = read_pxe_log(&args.pxe_log)?;
    verify_attestation(&config.sigstore, &bundle).map_err(|err| format_attestation_error(&err))?;
    verify_pxe_log(&config, &events).map_err(|err| format_pxe_log_error(&err))?;
    println!("Attestation issuer: {}", bundle.issuer);
    println!("Image digest: {}", bundle.image_digest);
    println!("PXE log validated for {} hosts", events.len());
    Ok(())
}
fn read_config(path: &Path) -> Result<PopConfig, String> {
    let bytes =
        read_bounded_direct_regular_file(path, POP_CONFIG_JSON_MAX_BYTES_V1, "PoP configuration")
            .map_err(|err| format!("failed to read config `{}`: {err}", path.display()))?;
    json::preflight_slice(&bytes, pop_config_json_preflight_limits_v1())
        .map_err(|err| format!("failed to parse config `{}`: {err}", path.display()))?;
    let config = norito::with_decode_limits_scope(POP_CONFIG_JSON_DECODE_LIMITS_V1, || {
        json::from_slice(&bytes)
    })
    .map_err(|err| format!("failed to parse config `{}`: {err}", path.display()))?;
    validate_config(&config).map_err(|err| format_validation_error(&err))?;
    Ok(config)
}
fn read_health_report(path: &Path) -> Result<HealthReport, String> {
    let bytes = read_bounded_direct_regular_file(
        path,
        HEALTH_REPORT_JSON_MAX_BYTES_V1,
        "PoP health report",
    )
    .map_err(|err| format!("failed to read health report `{}`: {err}", path.display()))?;
    json::preflight_slice(&bytes, health_report_json_preflight_limits_v1())
        .map_err(|err| format!("failed to parse health report `{}`: {err}", path.display()))?;
    norito::with_decode_limits_scope(HEALTH_REPORT_JSON_DECODE_LIMITS_V1, || {
        json::from_slice(&bytes)
    })
    .map_err(|err| format!("failed to parse health report `{}`: {err}", path.display()))
}
fn write_config(path: Option<&Path>, overwrite: bool, config: &PopConfig) -> Result<(), String> {
    match path {
        Some(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent).map_err(|err| {
                    format!("failed to create directory `{}`: {err}", parent.display())
                })?;
            }
            let file = OpenOptions::new()
                .create(true)
                .truncate(overwrite)
                .write(true)
                .create_new(!overwrite)
                .open(path)
                .map_err(|err| {
                    if overwrite {
                        format!("failed to write `{}`: {err}", path.display())
                    } else {
                        format!(
                            "failed to write `{}`: {err} (use --overwrite to replace existing files)",
                            path.display()
                        )
                    }
                })?;
            write_pretty_json(file, config)
        }
        None => {
            let stdout = io::stdout();
            let handle = stdout.lock();
            write_pretty_json(handle, config)
        }
    }
}
fn write_pretty_json<W: Write>(mut writer: W, config: &PopConfig) -> Result<(), String> {
    json::to_writer_pretty(&mut writer, config)
        .map_err(|err| format!("failed to render json: {err}"))?;
    writer
        .write_all(b"\n")
        .map_err(|err| format!("failed to flush json output: {err}"))
}
fn format_validation_error(err: &PopValidationError) -> String {
    err.to_string()
}
fn format_health_error(err: &HealthError) -> String {
    err.to_string()
}
fn read_attestation_bundle(path: &Path) -> Result<SigstoreBundle, String> {
    let bytes = read_bounded_direct_regular_file(
        path,
        ATTESTATION_JSON_MAX_BYTES_V1,
        "sigstore attestation bundle",
    )
    .map_err(|err| {
        format!(
            "failed to read attestation bundle `{}`: {err}",
            path.display()
        )
    })?;
    json::preflight_slice(&bytes, attestation_json_preflight_limits_v1()).map_err(|err| {
        format!(
            "failed to parse attestation bundle `{}`: {err}",
            path.display()
        )
    })?;
    norito::with_decode_limits_scope(ATTESTATION_JSON_DECODE_LIMITS_V1, || {
        json::from_slice(&bytes)
    })
    .map_err(|err| {
        format!(
            "failed to parse attestation bundle `{}`: {err}",
            path.display()
        )
    })
}
fn read_pxe_log(path: &Path) -> Result<Vec<PxeEvent>, String> {
    let bytes = read_bounded_direct_regular_file(path, PXE_LOG_JSON_MAX_BYTES_V1, "PXE log")
        .map_err(|err| format!("failed to read PXE log `{}`: {err}", path.display()))?;
    json::preflight_slice(&bytes, pxe_log_json_preflight_limits_v1())
        .map_err(|err| format!("failed to parse PXE log `{}`: {err}", path.display()))?;
    norito::with_decode_limits_scope(PXE_LOG_JSON_DECODE_LIMITS_V1, || json::from_slice(&bytes))
        .map_err(|err| format!("failed to parse PXE log `{}`: {err}", path.display()))
}
fn format_attestation_error(err: &AttestationError) -> String {
    err.to_string()
}
fn format_pxe_log_error(err: &PxeLogError) -> String {
    err.to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    fn padded_json(mut encoded: Vec<u8>, maximum: usize) -> Vec<u8> {
        assert!(encoded.len() < maximum);
        encoded.resize(maximum, b' ');
        encoded
    }
    fn json_array(entries: usize) -> Vec<u8> {
        let mut json = Vec::with_capacity(entries.saturating_mul(5).saturating_add(2));
        json.push(b'[');
        for index in 0..entries {
            if index != 0 {
                json.push(b',');
            }
            json.extend_from_slice(b"null");
        }
        json.push(b']');
        json
    }
    fn nested_json(depth: usize) -> Vec<u8> {
        let containers = depth.saturating_sub(1);
        let mut json = Vec::with_capacity(containers.saturating_mul(2).saturating_add(4));
        json.extend(std::iter::repeat_n(b'[', containers));
        json.extend_from_slice(b"null");
        json.extend(std::iter::repeat_n(b']', containers));
        json
    }
    #[test]
    fn config_reader_accepts_exact_raw_limit_and_rejects_plus_one() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("pop.json");
        let encoded = json::to_vec(&build_template(&TemplateOptions::default()))
            .expect("encode valid config");
        let mut exact = padded_json(encoded, POP_CONFIG_JSON_MAX_BYTES_V1);
        std::fs::write(&path, &exact).expect("write exact config");
        read_config(&path).expect("exact config limit must load");
        exact.push(b' ');
        std::fs::write(&path, exact).expect("write oversized config");
        let error = read_config(&path).expect_err("config limit + 1 must fail before decode");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[test]
    fn health_reader_accepts_exact_raw_limit_and_rejects_plus_one() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("health.json");
        let mut exact = padded_json(
            br#"{"generated_at":"2026-01-02T03:04:05Z","services":[]}"#.to_vec(),
            HEALTH_REPORT_JSON_MAX_BYTES_V1,
        );
        std::fs::write(&path, &exact).expect("write exact health report");
        let report = read_health_report(&path).expect("exact health report limit must load");
        assert_eq!(report.generated_at, "2026-01-02T03:04:05Z");
        exact.push(b' ');
        std::fs::write(&path, exact).expect("write oversized health report");
        let error =
            read_health_report(&path).expect_err("health report limit + 1 must fail before decode");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[test]
    fn attestation_reader_accepts_exact_raw_limit_and_rejects_plus_one() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("bundle.json");
        let mut exact = padded_json(
            br#"{"issuer":"https://issuer.example","subject":"workload@example","image_digest":"sha256:1234","annotations":{},"issued_at":null}"#.to_vec(),
            ATTESTATION_JSON_MAX_BYTES_V1,
        );
        std::fs::write(&path, &exact).expect("write exact attestation bundle");
        let bundle =
            read_attestation_bundle(&path).expect("exact attestation bundle limit must load");
        assert_eq!(bundle.image_digest, "sha256:1234");
        exact.push(b' ');
        std::fs::write(&path, exact).expect("write oversized attestation bundle");
        let error = read_attestation_bundle(&path)
            .expect_err("attestation bundle limit + 1 must fail before decode");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[test]
    fn pxe_reader_accepts_exact_raw_limit_and_rejects_plus_one() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("pxe.json");
        let mut exact = padded_json(b"[]".to_vec(), PXE_LOG_JSON_MAX_BYTES_V1);
        std::fs::write(&path, &exact).expect("write exact PXE log");
        assert!(
            read_pxe_log(&path)
                .expect("exact PXE log limit must load")
                .is_empty()
        );
        exact.push(b' ');
        std::fs::write(&path, exact).expect("write oversized PXE log");
        let error = read_pxe_log(&path).expect_err("PXE log limit + 1 must fail before decode");
        assert!(error.contains("first-release limit"), "{error}");
    }
    #[test]
    fn preflight_profiles_accept_exact_sequence_limits_and_reject_plus_one() {
        for (limits, maximum, label) in [
            (
                pop_config_json_preflight_limits_v1(),
                POP_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
                "config",
            ),
            (
                health_report_json_preflight_limits_v1(),
                HEALTH_REPORT_JSON_MAX_SEQUENCE_ELEMENTS_V1,
                "health report",
            ),
            (
                attestation_json_preflight_limits_v1(),
                ATTESTATION_JSON_MAX_SEQUENCE_ELEMENTS_V1,
                "attestation",
            ),
            (
                pxe_log_json_preflight_limits_v1(),
                PXE_LOG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
                "PXE log",
            ),
        ] {
            json::preflight_slice(&json_array(maximum), limits)
                .unwrap_or_else(|error| panic!("exact {label} sequence limit failed: {error}"));
            assert!(
                json::preflight_slice(&json_array(maximum + 1), limits).is_err(),
                "{label} sequence limit + 1 must fail"
            );
        }
    }
    #[test]
    fn preflight_profiles_accept_exact_field_limits_and_reject_plus_one() {
        for (limits, maximum, label) in [
            (
                pop_config_json_preflight_limits_v1(),
                POP_CONFIG_JSON_MAX_FIELD_BYTES_V1,
                "config",
            ),
            (
                health_report_json_preflight_limits_v1(),
                HEALTH_REPORT_JSON_MAX_FIELD_BYTES_V1,
                "health report",
            ),
            (
                attestation_json_preflight_limits_v1(),
                ATTESTATION_JSON_MAX_FIELD_BYTES_V1,
                "attestation",
            ),
            (
                pxe_log_json_preflight_limits_v1(),
                PXE_LOG_JSON_MAX_FIELD_BYTES_V1,
                "PXE log",
            ),
        ] {
            let exact = format!("\"{}\"", "a".repeat(maximum));
            json::preflight_slice(exact.as_bytes(), limits)
                .unwrap_or_else(|error| panic!("exact {label} field limit failed: {error}"));
            let plus_one = format!("\"{}\"", "a".repeat(maximum + 1));
            assert!(
                json::preflight_slice(plus_one.as_bytes(), limits).is_err(),
                "{label} field limit + 1 must fail"
            );
        }
    }
    #[test]
    fn preflight_profiles_accept_exact_depth_limits_and_reject_plus_one() {
        for (limits, maximum, label) in [
            (
                pop_config_json_preflight_limits_v1(),
                POP_CONFIG_JSON_MAX_DEPTH_V1,
                "config",
            ),
            (
                health_report_json_preflight_limits_v1(),
                HEALTH_REPORT_JSON_MAX_DEPTH_V1,
                "health report",
            ),
            (
                attestation_json_preflight_limits_v1(),
                ATTESTATION_JSON_MAX_DEPTH_V1,
                "attestation",
            ),
            (
                pxe_log_json_preflight_limits_v1(),
                PXE_LOG_JSON_MAX_DEPTH_V1,
                "PXE log",
            ),
        ] {
            json::preflight_slice(&nested_json(maximum), limits)
                .unwrap_or_else(|error| panic!("exact {label} depth limit failed: {error}"));
            assert!(
                json::preflight_slice(&nested_json(maximum + 1), limits).is_err(),
                "{label} depth limit + 1 must fail"
            );
        }
    }
}
