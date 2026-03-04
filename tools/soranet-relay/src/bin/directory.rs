use std::{
    fs,
    io::{Error as IoError, ErrorKind},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use iroha_crypto::soranet::directory::GuardDirectorySnapshotV2;
use norito::json;
use soranet_relay::{
    directory::{
        DirectoryBuildError, DirectoryBuildOptions, DirectoryMetadata, DirectoryRotateError,
        RotationOutput, build_snapshot_from_config_with_options,
        collect_guard_pinning_proofs_from_directory, inspect_snapshot, rotate_snapshot_with_os_rng,
    },
    guard::{GuardPinningProof, verify_guard_pinning_proof},
};

#[derive(Parser, Debug)]
#[command(
    name = "soranet-directory",
    version,
    about = "Build, rotate, and inspect SoraNet guard directory snapshots"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build a guard directory snapshot from the supplied JSON configuration.
    Build {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_name = "DIR")]
        guard_proofs_dir: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    /// Rotate issuer material for an existing snapshot and reissue certificates.
    Rotate {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        keys_out: Option<PathBuf>,
    },
    /// Inspect a snapshot and print its metadata.
    Inspect {
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// Verify a guard pinning proof against a guard directory snapshot.
    VerifyProof {
        #[arg(long)]
        proof: PathBuf,
        /// Optional guard directory snapshot to override the path recorded inside the proof.
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// Collect guard pinning proofs from a directory and verify them against a snapshot.
    CollectProofs {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long, value_name = "DIR")]
        proofs_dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("soranet-directory error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    match args.command {
        Command::Build {
            config,
            out,
            overwrite,
            guard_proofs_dir,
        } => command_build(&config, &out, guard_proofs_dir.as_deref(), overwrite),
        Command::Rotate {
            snapshot,
            out,
            overwrite,
            keys_out,
        } => command_rotate(&snapshot, &out, overwrite, keys_out.as_deref()),
        Command::Inspect { snapshot } => command_inspect(&snapshot),
        Command::VerifyProof { proof, snapshot } => {
            command_verify_proof(&proof, snapshot.as_deref())
        }
        Command::CollectProofs {
            snapshot,
            proofs_dir,
            out,
            overwrite,
        } => command_collect_proofs(&snapshot, &proofs_dir, out.as_deref(), overwrite),
    }
}

fn command_build(
    config: &Path,
    out: &Path,
    guard_proofs_dir: Option<&Path>,
    overwrite: bool,
) -> Result<(), String> {
    let bundle = build_snapshot_from_config_with_options(
        config,
        DirectoryBuildOptions {
            guard_pinning_proofs_dir: guard_proofs_dir,
        },
    )
    .map_err(build_error)?;
    let bytes = bundle
        .snapshot
        .to_bytes()
        .map_err(|err| format!("failed to encode snapshot: {err}"))?;
    write_output(out, &bytes, overwrite)
        .map_err(|err| format!("failed to write snapshot to `{}`: {err}", out.display()))?;

    println!("Snapshot written to {}", out.display());
    print_metadata(&bundle.metadata);
    Ok(())
}

fn command_rotate(
    snapshot_path: &Path,
    out: &Path,
    overwrite: bool,
    keys_out: Option<&Path>,
) -> Result<(), String> {
    let bytes = fs::read(snapshot_path).map_err(|err| {
        format!(
            "failed to read snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let rotation =
        rotate_snapshot_with_os_rng(&bytes).map_err(|err| rotate_error(snapshot_path, err))?;

    let encoded = rotation
        .bundle
        .snapshot
        .to_bytes()
        .map_err(|err| format!("failed to encode rotated snapshot: {err}"))?;
    write_output(out, &encoded, overwrite).map_err(|err| {
        format!(
            "failed to write rotated snapshot to `{}`: {err}",
            out.display()
        )
    })?;
    println!("Rotated snapshot written to {}", out.display());
    print_metadata(&rotation.bundle.metadata);

    if let Some(dir) = keys_out {
        store_rotation_keys(dir, &rotation).map_err(|err| {
            format!(
                "failed to write rotation keys to `{}`: {err}",
                dir.display()
            )
        })?;
        println!("Issuer key material written to {}", dir.display());
    }

    Ok(())
}

fn command_inspect(snapshot_path: &Path) -> Result<(), String> {
    let bytes = fs::read(snapshot_path).map_err(|err| {
        format!(
            "failed to read snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let bundle = inspect_snapshot(&bytes).map_err(|err| rotate_error(snapshot_path, err))?;
    println!("Snapshot {}", snapshot_path.display());
    print_metadata(&bundle.metadata);
    Ok(())
}

fn command_verify_proof(proof_path: &Path, snapshot_override: Option<&Path>) -> Result<(), String> {
    let proof_bytes = fs::read(proof_path).map_err(|err| {
        format!(
            "failed to read guard pinning proof `{}`: {err}",
            proof_path.display()
        )
    })?;
    let proof: GuardPinningProof = json::from_slice(&proof_bytes).map_err(|err| {
        format!(
            "failed to decode guard pinning proof `{}`: {err}",
            proof_path.display()
        )
    })?;
    let snapshot_path = if let Some(path) = snapshot_override {
        path.to_path_buf()
    } else if proof.snapshot_path().is_empty() {
        return Err("proof did not record snapshot_path; supply --snapshot".to_string());
    } else {
        PathBuf::from(proof.snapshot_path())
    };
    let snapshot_bytes = fs::read(&snapshot_path).map_err(|err| {
        format!(
            "failed to read guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let snapshot = GuardDirectorySnapshotV2::from_bytes(&snapshot_bytes).map_err(|err| {
        format!(
            "failed to decode guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    verify_guard_pinning_proof(&snapshot, &proof)
        .map_err(|err| format!("guard pinning proof verification failed: {err}"))?;

    println!(
        "Guard pinning proof `{}` verified against `{}`",
        proof_path.display(),
        snapshot_path.display()
    );
    println!(" relay_id: {}", proof.relay_id_hex());
    println!(" directory_hash: {}", proof.directory_hash_hex());
    println!(" recorded_at_unix: {}", proof.recorded_at_unix());
    Ok(())
}

fn command_collect_proofs(
    snapshot_path: &Path,
    proofs_dir: &Path,
    out_path: Option<&Path>,
    overwrite: bool,
) -> Result<(), String> {
    let snapshot_bytes = fs::read(snapshot_path).map_err(|err| {
        format!(
            "failed to read guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let snapshot = GuardDirectorySnapshotV2::from_bytes(&snapshot_bytes).map_err(|err| {
        format!(
            "failed to decode guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let summaries = collect_guard_pinning_proofs_from_directory(proofs_dir, &snapshot)
        .map_err(|err| format!("failed to collect guard pinning proofs: {err}"))?;

    println!(
        "Verified {} guard pinning proofs under {}",
        summaries.len(),
        proofs_dir.display()
    );
    for summary in &summaries {
        println!(
            " relay_id: {} descriptor_commit: {} guard_weight: {} bandwidth: {} B/s validity: {}..{}",
            summary.relay_id_hex,
            summary.descriptor_commit_hex,
            summary.guard_weight,
            summary.bandwidth_bytes_per_sec,
            summary.valid_after_unix,
            summary.valid_until_unix,
        );
    }

    if let Some(path) = out_path {
        let bytes = json::to_vec_pretty(&summaries)
            .map_err(|err| format!("failed to encode proof summaries: {err}"))?;
        write_output(path, &bytes, overwrite).map_err(|err| {
            format!(
                "failed to write proof summaries to `{}`: {err}",
                path.display()
            )
        })?;
        println!("Proof summaries written to {}", path.display());
    }

    Ok(())
}

fn build_error(err: DirectoryBuildError) -> String {
    match err {
        DirectoryBuildError::Io { path, source } => {
            format!("failed to read `{}`: {source}", path.display())
        }
        DirectoryBuildError::Json { path, source } => {
            format!("failed to parse config `{}`: {source}", path.display())
        }
        other => other.to_string(),
    }
}

fn rotate_error(path: &Path, err: DirectoryRotateError) -> String {
    match err {
        DirectoryRotateError::Decode { source } => format!(
            "failed to decode guard directory `{}`: {source}",
            path.display()
        ),
        other => other.to_string(),
    }
}

fn write_output(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), IoError> {
    if !overwrite && path.exists() {
        return Err(IoError::new(
            ErrorKind::AlreadyExists,
            format!("file `{}` already exists (use --overwrite)", path.display()),
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

fn store_rotation_keys(dir: &Path, rotation: &RotationOutput) -> Result<(), IoError> {
    if dir.exists() && !dir.is_dir() {
        return Err(IoError::new(
            ErrorKind::AlreadyExists,
            format!("`{}` exists and is not a directory", dir.display()),
        ));
    }
    fs::create_dir_all(dir)?;

    let ed25519_secret_hex = hex::encode(rotation.keys.ed25519_secret);
    let ed25519_public_hex = hex::encode(rotation.keys.ed25519_public);
    let mldsa_public_hex = hex::encode(&rotation.keys.mldsa_public);
    let mldsa_secret_hex = hex::encode(&rotation.keys.mldsa_secret);
    let fingerprint_hex = hex::encode(rotation.keys.fingerprint);

    write_text(dir.join("issuer_ed25519_secret.hex"), &ed25519_secret_hex)?;
    write_text(dir.join("issuer_ed25519_public.hex"), &ed25519_public_hex)?;
    write_text(dir.join("issuer_mldsa_public.hex"), &mldsa_public_hex)?;
    write_text(dir.join("issuer_mldsa_secret.hex"), &mldsa_secret_hex)?;
    fs::write(
        dir.join("issuer_mldsa_secret.bin"),
        &rotation.keys.mldsa_secret,
    )?;
    write_text(dir.join("issuer_fingerprint.hex"), &fingerprint_hex)?;

    Ok(())
}

fn write_text(path: PathBuf, contents: &str) -> Result<(), IoError> {
    fs::write(path, format!("{contents}\n"))
}

fn print_metadata(metadata: &DirectoryMetadata) {
    println!("directory_hash: {}", metadata.directory_hash_hex);
    println!(
        "validity: published={} valid_after={} valid_until={}",
        metadata.published_at_unix, metadata.valid_after_unix, metadata.valid_until_unix
    );
    println!("validation_phase: {:?}", metadata.validation_phase);
    println!("issuers ({}):", metadata.issuers.len());
    for issuer in &metadata.issuers {
        let label = issuer.label.as_deref().unwrap_or("-");
        let mode = if issuer.has_mldsa {
            "dual-signature"
        } else {
            "ed25519-only"
        };
        println!(
            "  - {label}: fingerprint={}, ed25519={}, mode={mode}",
            issuer.fingerprint_hex, issuer.ed25519_hex
        );
    }
    println!("relays ({}):", metadata.certificates.len());
    for cert in &metadata.certificates {
        let path = cert
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "  - relay={} guard={} reputation={} bandwidth={} path={}",
            cert.relay_id_hex,
            cert.guard_weight,
            cert.reputation_weight,
            cert.bandwidth_bytes_per_sec,
            path
        );
        println!("    validity: {} -> {}", cert.valid_after, cert.valid_until);
    }
    if metadata.guard_pinning_proofs.is_empty() {
        println!("guard pinning proofs: none supplied");
    } else {
        println!(
            "guard pinning proofs ({}):",
            metadata.guard_pinning_proofs.len()
        );
        for proof in &metadata.guard_pinning_proofs {
            println!(
                "  - relay={} recorded_at={} path={}",
                proof.relay_id_hex,
                proof.recorded_at_unix,
                proof.path.display()
            );
            println!(
                "    directory_hash={} descriptor={} issuer={}",
                proof.directory_hash_hex, proof.descriptor_commit_hex, proof.issuer_fingerprint_hex
            );
            println!(
                "    pq_kem={} guard={} reputation={} bandwidth={}",
                proof.pq_kem_public_hex,
                proof.guard_weight,
                proof.reputation_weight,
                proof.bandwidth_bytes_per_sec
            );
            println!(
                "    validity: {} -> {} phase={}",
                proof.valid_after_unix, proof.valid_until_unix, proof.validation_phase
            );
        }
    }
}
