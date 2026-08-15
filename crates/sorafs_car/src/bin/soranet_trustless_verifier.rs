//! CLI wrapper around the trustless verifier used for SoraNet gateway CAR payloads.
//! Only the `dag-scope=full` path is supported today; chunk plans and PoR roots
//! are reconstructed and checked against the manifest's mandatory commitments.
#![allow(unexpected_cfgs)]
use clap::Parser;
use eyre::{Context, Result};
use norito::{
    decode_from_bytes,
    json::{self, Value},
};
use sorafs_car::{TrustlessVerifier, TrustlessVerifierConfig, validate_manifest_car_replay};
use sorafs_manifest::{ManifestV1, ValidationOutcomeV1};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Verify SoraNet gateway CAR payloads against mandatory manifest commitments.",
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
    /// Write the verification summary JSON to this path (defaults to stdout).
    #[arg(long)]
    json_out: Option<PathBuf>,
    /// Emit a stable ValidationOutcomeV1 manifest/CAR replay result instead of the summary.
    #[arg(long)]
    validation_outcome: bool,
    /// Override the ValidationOutcomeV1 generated_at timestamp.
    #[arg(long)]
    generated_at: Option<String>,
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
    if !args.validation_outcome && args.generated_at.is_some() {
        return Err(eyre::eyre!(
            "--generated-at only applies with --validation-outcome"
        ));
    }
    let config_path = args.config.clone().unwrap_or_else(default_config_path);
    let config = TrustlessVerifierConfig::from_file(&config_path)?;
    let manifest = load_manifest(&args.manifest)?;
    let car_bytes = fs::read(&args.car)
        .wrap_err_with(|| format!("failed to read CAR `{}`", args.car.display()))?;
    if args.validation_outcome {
        let generated_at = match args.generated_at.as_deref() {
            Some(generated_at) => parse_generated_at(generated_at)?,
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
        let file = open_output_file(out, "JSON output")?;
        json::to_writer_pretty(file, &summary)
            .wrap_err_with(|| format!("failed to write JSON output `{}`", out.display()))?;
    } else if !args.quiet {
        let rendered =
            json::to_string_pretty(&summary).wrap_err("failed to render summary JSON")?;
        println!("{rendered}");
    }
    Ok(0)
}
fn emit_validation_outcome(args: &Args, outcome: &ValidationOutcomeV1) -> Result<()> {
    let rendered = json::to_string_pretty(outcome).wrap_err("failed to render outcome JSON")?;
    if let Some(out) = args.json_out.as_ref() {
        let mut file = open_output_file(out, "JSON output")?;
        file.write_all(format!("{rendered}\n").as_bytes())
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
fn parse_generated_at(value: &str) -> Result<u64> {
    require_canonical_positive_decimal("--generated-at", value)?;
    value
        .parse::<u64>()
        .wrap_err_with(|| format!("invalid --generated-at value `{value}`"))
}
fn require_canonical_positive_decimal(label: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(eyre::eyre!("{label} must not be empty"));
    }
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(eyre::eyre!("{label} must not contain ASCII whitespace"));
    }
    if value.starts_with('+') || value.starts_with('-') {
        return Err(eyre::eyre!("{label} must be an unsigned decimal token"));
    }
    if !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre::eyre!("{label} must contain only decimal digits"));
    }
    if value.len() > 1 && value.starts_with('0') {
        return Err(eyre::eyre!(
            "{label} must use canonical decimal without leading zeros"
        ));
    }
    if value == "0" {
        return Err(eyre::eyre!("{label} must be greater than zero"));
    }
    Ok(())
}
fn open_output_file(path: &Path, label: &str) -> Result<fs::File> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {label} `{}`", path.display()))?;
    let metadata = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect {label} `{}` after open", path.display()))?;
    if !metadata.is_file() {
        return Err(eyre::eyre!(
            "failed to write {label} `{}`: output must be a regular file",
            path.display()
        ));
    }
    Ok(file)
}
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create output parent `{}`", parent.display()))?;
    }
    Ok(())
}
fn validate_output_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(eyre::eyre!(
                    "output `{}` must not be a symlink",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                return Err(eyre::eyre!(
                    "output `{}` must not be a directory",
                    path.display()
                ));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err)
                .wrap_err_with(|| format!("failed to inspect output `{}`", path.display()));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(eyre::eyre!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        ));
                    }
                    if !metadata.is_dir() {
                        return Err(eyre::eyre!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        ));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).wrap_err_with(|| {
                        format!("failed to inspect output parent `{}`", ancestor.display())
                    });
                }
            }
        }
    }
    Ok(())
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, tempdir};
    fn canonical_tempdir() -> (TempDir, PathBuf) {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().canonicalize().expect("canonical tempdir");
        (temp, path)
    }
    #[test]
    fn parse_generated_at_rejects_noncanonical_values() {
        assert_eq!(parse_generated_at("123").expect("timestamp"), 123);
        for value in [
            "",
            "0",
            "0123",
            "+123",
            "-123",
            "123 ",
            "12_3",
            "18446744073709551616",
        ] {
            let err = parse_generated_at(value).expect_err("invalid generated_at must fail");
            let message = err.to_string();
            assert!(
                message.contains("empty")
                    || message.contains("greater than zero")
                    || message.contains("leading zeros")
                    || message.contains("unsigned decimal")
                    || message.contains("whitespace")
                    || message.contains("decimal digits")
                    || message.contains("invalid --generated-at"),
                "unexpected generated_at error for {value:?}: {message}"
            );
        }
    }
    #[test]
    fn open_output_file_creates_parent_and_writes_all_bytes() {
        let (_temp, temp_path) = canonical_tempdir();
        let output_path = temp_path.join("nested").join("summary.json");
        let mut file = open_output_file(&output_path, "test output").expect("open output");
        file.write_all(b"{\"ok\":true}\n").expect("write output");
        drop(file);
        assert_eq!(
            fs::read(&output_path).expect("read output"),
            b"{\"ok\":true}\n"
        );
    }
    #[cfg(unix)]
    #[test]
    fn open_output_file_rejects_symlink_output() {
        let (_temp, temp_path) = canonical_tempdir();
        let target_path = temp_path.join("target.json");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("summary.json");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");
        let err = match open_output_file(&output_path, "test output") {
            Ok(_) => panic!("symlink output should be rejected"),
            Err(err) => err,
        };
        let message = err.to_string();
        assert!(
            message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }
    #[cfg(unix)]
    #[test]
    fn open_output_file_rejects_symlink_parent() {
        let (_temp, temp_path) = canonical_tempdir();
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("summary.json");
        let err = match open_output_file(&output_path, "test output") {
            Ok(_) => panic!("symlink parent should be rejected"),
            Err(err) => err,
        };
        let message = err.to_string();
        assert!(
            message.contains("parent") && message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join("summary.json").exists(),
            "symlink parent should not receive output"
        );
    }
}
