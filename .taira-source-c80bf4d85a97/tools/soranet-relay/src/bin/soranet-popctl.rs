use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand};
use norito::json;
use soranet_relay::popctl::{
    AttestationError, HealthError, HealthReport, HealthState, PopConfig, PopValidationError,
    PxeEvent, PxeLogError, SigstoreBundle, TemplateOptions, build_template, evaluate_health,
    validate_config, verify_attestation, verify_pxe_log,
};

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
    let config = read_config(&args.config)?;
    validate_config(&config).map_err(|err| format_validation_error(&err))?;
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
    let file = File::open(path)
        .map_err(|err| format!("failed to read config `{}`: {err}", path.display()))?;
    json::from_reader(file)
        .map_err(|err| format!("failed to parse config `{}`: {err}", path.display()))
}

fn read_health_report(path: &Path) -> Result<HealthReport, String> {
    let file = File::open(path)
        .map_err(|err| format!("failed to read health report `{}`: {err}", path.display()))?;
    json::from_reader(file)
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
    let file = File::open(path).map_err(|err| {
        format!(
            "failed to read attestation bundle `{}`: {err}",
            path.display()
        )
    })?;
    json::from_reader(file).map_err(|err| {
        format!(
            "failed to parse attestation bundle `{}`: {err}",
            path.display()
        )
    })
}

fn read_pxe_log(path: &Path) -> Result<Vec<PxeEvent>, String> {
    let file = File::open(path)
        .map_err(|err| format!("failed to read PXE log `{}`: {err}", path.display()))?;
    json::from_reader(file)
        .map_err(|err| format!("failed to parse PXE log `{}`: {err}", path.display()))
}

fn format_attestation_error(err: &AttestationError) -> String {
    err.to_string()
}

fn format_pxe_log_error(err: &PxeLogError) -> String {
    err.to_string()
}
