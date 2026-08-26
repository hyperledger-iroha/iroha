//! Build, rotate, inspect, and verify SoraNet guard-directory artifacts.
use clap::{Parser, Subcommand};
use iroha_crypto::soranet::directory::{
    GuardDirectorySnapshotV2, compute_snapshot_digest, read_guard_directory_snapshot_file,
};
use norito::json;
use soranet_relay::{
    directory::{
        DirectoryBuildError, DirectoryBuildOptions, DirectoryMetadata, DirectoryRotateError,
        RotationKeys, build_snapshot_from_config_with_options,
        collect_guard_pinning_proofs_from_directory, inspect_snapshot,
        read_guard_pinning_proof_file, rotate_snapshot_with_os_rng,
    },
    guard::verify_guard_pinning_proof,
};
use std::{
    fs,
    io::{Error as IoError, ErrorKind, Write as _},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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
    /// Authenticate an active snapshot, rotate issuer material, and reissue certificates.
    Rotate {
        #[arg(long)]
        snapshot: PathBuf,
        /// Independently trusted digest of the exact source snapshot (64 lowercase hex characters).
        #[arg(long, value_name = "LOWERCASE_HEX")]
        expected_snapshot_digest: String,
        /// Unix second at which to authenticate the source; defaults to the current time.
        #[arg(long, value_name = "UNIX_SECONDS")]
        at_unix: Option<i64>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        overwrite: bool,
        /// New owner-private directory in which all generated issuer keys are published.
        #[arg(long)]
        keys_out: PathBuf,
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
            expected_snapshot_digest,
            at_unix,
            out,
            overwrite,
            keys_out,
        } => command_rotate(
            &snapshot,
            &expected_snapshot_digest,
            at_unix,
            &out,
            overwrite,
            &keys_out,
        ),
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
    println!(
        " snapshot_digest: {}",
        hex::encode(compute_snapshot_digest(&bytes))
    );
    print_metadata(&bundle.metadata);
    Ok(())
}
fn command_rotate(
    snapshot_path: &Path,
    expected_snapshot_digest_hex: &str,
    at_unix: Option<i64>,
    out: &Path,
    overwrite: bool,
    keys_out: &Path,
) -> Result<(), String> {
    let expected_snapshot_digest = parse_expected_snapshot_digest(expected_snapshot_digest_hex)?;
    let at_unix = match at_unix {
        Some(value) if value >= 0 => value,
        Some(value) => {
            return Err(format!(
                "--at-unix must be a non-negative Unix second (got {value})"
            ));
        }
        None => current_unix_seconds()?,
    };
    let bytes = read_guard_directory_snapshot_file(snapshot_path).map_err(|err| {
        format!(
            "failed to read snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let rotation = rotate_snapshot_with_os_rng(&bytes, expected_snapshot_digest, at_unix)
        .map_err(|err| rotate_error(snapshot_path, err))?;
    let encoded = rotation
        .bundle
        .snapshot
        .to_bytes()
        .map_err(|err| format!("failed to encode rotated snapshot: {err}"))?;
    let staged_snapshot = stage_output(out, &encoded, overwrite).map_err(|err| {
        format!(
            "failed to stage rotated snapshot for `{}`: {err}",
            out.display()
        )
    })?;
    publish_rotation_artifacts(staged_snapshot, keys_out, &rotation.keys).map_err(|err| {
        format!(
            "failed to publish rotation key bundle `{}` before snapshot `{}`: {err}",
            keys_out.display(),
            out.display()
        )
    })?;
    println!("Rotated snapshot written to {}", out.display());
    println!(
        " snapshot_digest: {}",
        hex::encode(compute_snapshot_digest(&encoded))
    );
    print_metadata(&rotation.bundle.metadata);
    println!("Issuer key material written to {}", keys_out.display());
    Ok(())
}
fn parse_expected_snapshot_digest(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "--expected-snapshot-digest must be exactly 64 lowercase hexadecimal characters"
                .to_string(),
        );
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest)
        .map_err(|err| format!("failed to decode --expected-snapshot-digest as 32 bytes: {err}"))?;
    if digest.iter().all(|byte| *byte == 0) {
        return Err("--expected-snapshot-digest must not be all zero".to_string());
    }
    Ok(digest)
}
fn current_unix_seconds() -> Result<i64, String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock is before the Unix epoch: {err}"))?
        .as_secs();
    i64::try_from(seconds)
        .map_err(|_| "current Unix timestamp exceeds the supported i64 range".to_string())
}
fn command_inspect(snapshot_path: &Path) -> Result<(), String> {
    let bytes = read_guard_directory_snapshot_file(snapshot_path).map_err(|err| {
        format!(
            "failed to read snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let bundle = inspect_snapshot(&bytes).map_err(|err| rotate_error(snapshot_path, err))?;
    println!(
        "Snapshot {} (structural inspection only; not authenticated)",
        snapshot_path.display()
    );
    println!(
        " snapshot_digest: {}",
        hex::encode(compute_snapshot_digest(&bytes))
    );
    print_metadata(&bundle.metadata);
    Ok(())
}
fn command_verify_proof(proof_path: &Path, snapshot_override: Option<&Path>) -> Result<(), String> {
    // Keep CLI verification on the same stable-file and JSON admission policy
    // used by build/collection paths in the relay library.
    let proof = read_guard_pinning_proof_file(proof_path)
        .map_err(|err| guard_pinning_proof_error(proof_path, err))?;
    let snapshot_path = if let Some(path) = snapshot_override {
        path.to_path_buf()
    } else if proof.snapshot_path().is_empty() {
        return Err("proof did not record snapshot_path; supply --snapshot".to_string());
    } else {
        PathBuf::from(proof.snapshot_path())
    };
    let snapshot_bytes = read_guard_directory_snapshot_file(&snapshot_path).map_err(|err| {
        format!(
            "failed to read guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let snapshot = GuardDirectorySnapshotV2::inspect_bytes(&snapshot_bytes).map_err(|err| {
        format!(
            "failed to decode guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    verify_guard_pinning_proof(&snapshot, &proof)
        .map_err(|err| format!("guard pinning proof verification failed: {err}"))?;
    println!(
        "Guard pinning proof `{}` is structurally consistent with unauthenticated snapshot `{}`",
        proof_path.display(),
        snapshot_path.display()
    );
    println!(" relay_id: {}", proof.relay_id_hex());
    println!(" directory_hash: {}", proof.directory_hash_hex());
    println!(" recorded_at_unix: {}", proof.recorded_at_unix());
    Ok(())
}
fn guard_pinning_proof_error(path: &Path, err: DirectoryBuildError) -> String {
    match err {
        DirectoryBuildError::GuardPinningProofIo { source, .. } => format!(
            "failed to read guard pinning proof `{}`: {source}",
            path.display()
        ),
        DirectoryBuildError::GuardPinningProofDecode { source, .. } => format!(
            "failed to decode guard pinning proof `{}`: {source}",
            path.display()
        ),
        other => other.to_string(),
    }
}
fn command_collect_proofs(
    snapshot_path: &Path,
    proofs_dir: &Path,
    out_path: Option<&Path>,
    overwrite: bool,
) -> Result<(), String> {
    let snapshot_bytes = read_guard_directory_snapshot_file(snapshot_path).map_err(|err| {
        format!(
            "failed to read guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let snapshot = GuardDirectorySnapshotV2::inspect_bytes(&snapshot_bytes).map_err(|err| {
        format!(
            "failed to decode guard directory snapshot `{}`: {err}",
            snapshot_path.display()
        )
    })?;
    let summaries = collect_guard_pinning_proofs_from_directory(proofs_dir, &snapshot)
        .map_err(|err| format!("failed to collect guard pinning proofs: {err}"))?;
    println!(
        "Structurally checked {} guard pinning proofs against an unauthenticated snapshot under {}",
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
        DirectoryRotateError::Authentication { source } => format!(
            "failed to authenticate source guard directory `{}`: {source}",
            path.display()
        ),
        other => other.to_string(),
    }
}
struct StagedOutput {
    file: tempfile::NamedTempFile,
    destination: PathBuf,
    parent: PathBuf,
    overwrite: bool,
}
struct PublishedOutput {
    file: fs::File,
    parent: PathBuf,
}
impl PublishedOutput {
    fn sync(self) -> Result<(), IoError> {
        self.file.sync_all()?;
        #[cfg(unix)]
        fs::File::open(self.parent)?.sync_all()?;
        Ok(())
    }
}
impl StagedOutput {
    fn persist(self) -> Result<PublishedOutput, IoError> {
        let Self {
            file,
            destination,
            parent,
            overwrite,
        } = self;
        let persisted = if overwrite {
            file.persist(&destination)
        } else {
            file.persist_noclobber(&destination)
        }
        .map_err(|error| error.error)?;
        Ok(PublishedOutput {
            file: persisted,
            parent,
        })
    }

    fn publish(self) -> Result<(), IoError> {
        self.persist()?.sync()
    }
}
fn stage_output(path: &Path, bytes: &[u8], overwrite: bool) -> Result<StagedOutput, IoError> {
    let file_name = path.file_name().ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "directory output path must name a file",
        )
    })?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let parent = fs::canonicalize(parent)?;
    let destination = parent.join(file_name);
    match fs::symlink_metadata(&destination) {
        Ok(_) if !overwrite => {
            return Err(IoError::new(
                ErrorKind::AlreadyExists,
                format!("output `{}` already exists", destination.display()),
            ));
        }
        Ok(metadata) if metadata.is_dir() => {
            return Err(IoError::new(
                ErrorKind::InvalidInput,
                format!("output `{}` must not be a directory", destination.display()),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let mut staged = tempfile::NamedTempFile::new_in(&parent)?;
    staged.write_all(bytes)?;
    staged.flush()?;
    staged.as_file().sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o644))?;
    }
    Ok(StagedOutput {
        file: staged,
        destination,
        parent,
        overwrite,
    })
}
fn write_output(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), IoError> {
    stage_output(path, bytes, overwrite)?.publish()
}
fn publish_rotation_artifacts(
    staged_snapshot: StagedOutput,
    keys_out: &Path,
    keys: &RotationKeys,
) -> Result<(), IoError> {
    // Publish the complete key directory first. A snapshot is unusable if its
    // newly generated issuer secrets were not durably persisted.
    let published_keys = store_rotation_keys(keys_out, keys)?;
    let published_snapshot = match staged_snapshot.persist() {
        Ok(published) => published,
        Err(snapshot_error) => {
            let rollback = remove_published_rotation_keys(&published_keys);
            return match rollback {
                Ok(()) => Err(snapshot_error),
                Err(rollback_error) => Err(IoError::new(
                    snapshot_error.kind(),
                    format!(
                        "snapshot publication failed: {snapshot_error}; rotation-key rollback failed: {rollback_error}"
                    ),
                )),
            };
        }
    };
    // Once the namespace update succeeds, retain the already durable keys even
    // if a subsequent file/directory sync reports an error. Removing them here
    // could leave a visible snapshot whose issuer secrets were destroyed.
    published_snapshot.sync()
}
fn remove_published_rotation_keys(dir: &Path) -> Result<(), IoError> {
    validate_rotation_key_directory(dir)?;
    let parent = dir.parent().ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "rotation key output path has no parent",
        )
    })?;
    fs::remove_dir_all(dir)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn store_rotation_keys(dir: &Path, keys: &RotationKeys) -> Result<PathBuf, IoError> {
    let absolute = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(dir)
    };
    if absolute.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "rotation key output path must not contain dot components",
        ));
    }
    let file_name = absolute.file_name().ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "rotation key output path must name a directory",
        )
    })?;
    let parent_path = fs::canonicalize(absolute.parent().ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "rotation key output path must have an existing parent directory",
        )
    })?)?;
    let destination = parent_path.join(file_name);
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            return Err(IoError::new(
                ErrorKind::AlreadyExists,
                format!(
                    "rotation key output `{}` already exists; refusing to replace key material",
                    destination.display()
                ),
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    validate_rotation_key_parent(&parent_path)?;

    let staging = tempfile::Builder::new()
        .prefix(".soranet-directory-keys-")
        .tempdir_in(&parent_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(staging.path(), fs::Permissions::from_mode(0o700))?;
    }
    validate_rotation_key_directory(staging.path())?;

    let ed25519_secret_hex = encode_secret_hex_line(&keys.ed25519_secret)?;
    let ed25519_public_hex = hex::encode(keys.ed25519_public);
    let mldsa_public_hex = hex::encode(&keys.mldsa_public);
    let mldsa_secret_hex = encode_secret_hex_line(&keys.mldsa_secret)?;
    let fingerprint_hex = hex::encode(keys.fingerprint);
    write_private_file(
        &staging.path().join("issuer_ed25519_secret.hex"),
        ed25519_secret_hex.expose(),
    )?;
    write_private_text(
        &staging.path().join("issuer_ed25519_public.hex"),
        &ed25519_public_hex,
    )?;
    write_private_text(
        &staging.path().join("issuer_mldsa_public.hex"),
        &mldsa_public_hex,
    )?;
    write_private_file(
        &staging.path().join("issuer_mldsa_secret.hex"),
        mldsa_secret_hex.expose(),
    )?;
    write_private_file(
        &staging.path().join("issuer_mldsa_secret.bin"),
        &keys.mldsa_secret,
    )?;
    write_private_text(
        &staging.path().join("issuer_fingerprint.hex"),
        &fingerprint_hex,
    )?;
    fs::File::open(staging.path())?.sync_all()?;

    // The parent chain is not replaceable by another principal, so a final
    // rename publishes the complete bundle without exposing partially written
    // key files or replacing an existing bundle.
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            return Err(IoError::new(
                ErrorKind::AlreadyExists,
                "rotation key output appeared while the bundle was staged",
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(staging.path(), &destination)?;
    validate_rotation_key_directory(&destination)?;
    fs::File::open(&parent_path)?.sync_all()?;
    Ok(destination)
}

struct SecretBytes(Vec<u8>);

impl SecretBytes {
    fn expose(&self) -> &[u8] {
        &self.0
    }

    fn clear(&mut self) {
        self.0.resize(self.0.capacity(), 0);
        zeroize::Zeroize::zeroize(self.0.as_mut_slice());
        self.0.clear();
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.clear();
    }
}

fn encode_secret_hex_line(secret: &[u8]) -> Result<SecretBytes, IoError> {
    let hex_len = secret.len().checked_mul(2).ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "secret key is too large to encode safely",
        )
    })?;
    let encoded_len = hex_len.checked_add(1).ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "secret key is too large to encode safely",
        )
    })?;
    let mut encoded = SecretBytes(vec![0; encoded_len]);
    hex::encode_to_slice(secret, &mut encoded.0[..hex_len]).map_err(|error| {
        IoError::new(
            ErrorKind::InvalidData,
            format!("failed to encode secret key: {error}"),
        )
    })?;
    encoded.0[hex_len] = b'\n';
    Ok(encoded)
}

fn write_private_text(path: &Path, contents: &str) -> Result<(), IoError> {
    let mut bytes = Vec::with_capacity(contents.len().saturating_add(1));
    bytes.extend_from_slice(contents.as_bytes());
    bytes.push(b'\n');
    write_private_file(path, &bytes)
}

fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), IoError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    validate_rotation_key_file(path, &file)?;
    Ok(())
}

fn validate_rotation_key_directory(path: &Path) -> Result<(), IoError> {
    let named = fs::symlink_metadata(path)?;
    let opened = fs::File::open(path)?;
    let metadata = opened.metadata()?;
    if named.file_type().is_symlink() || !named.is_dir() || !metadata.is_dir() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "rotation key output must be a direct directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if named.dev() != metadata.dev()
            || named.ino() != metadata.ino()
            || named.uid() != metadata.uid()
            || named.mode() & 0o077 != 0
        {
            return Err(IoError::new(
                ErrorKind::PermissionDenied,
                "rotation key output directory must be stable and owner-private",
            ));
        }
    }
    Ok(())
}

fn validate_rotation_key_file(path: &Path, file: &fs::File) -> Result<(), IoError> {
    let named = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    if named.file_type().is_symlink() || !named.is_file() || !opened.is_file() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "rotation key artifact must be a direct regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if named.dev() != opened.dev()
            || named.ino() != opened.ino()
            || opened.mode() & 0o077 != 0
            || opened.nlink() != 1
        {
            return Err(IoError::new(
                ErrorKind::PermissionDenied,
                "rotation key artifact must be stable, owner-private, and singly linked",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_rotation_key_parent(parent: &Path) -> Result<(), IoError> {
    use std::os::unix::fs::MetadataExt as _;
    let owner_uid = tempfile::tempfile()?.metadata()?.uid();
    let mut ancestors = Vec::new();
    let mut cursor = parent;
    loop {
        ancestors.push(cursor.to_path_buf());
        let Some(next) = cursor.parent() else {
            break;
        };
        if next == cursor {
            break;
        }
        cursor = next;
    }
    ancestors.reverse();
    let metadata = ancestors
        .iter()
        .map(fs::symlink_metadata)
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (path, observed)) in ancestors.iter().zip(&metadata).enumerate() {
        if observed.file_type().is_symlink() || !observed.is_dir() {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "rotation key output ancestor `{}` must be a direct directory",
                    path.display()
                ),
            ));
        }
        if observed.uid() != 0 && observed.uid() != owner_uid {
            return Err(IoError::new(
                ErrorKind::PermissionDenied,
                format!(
                    "rotation key output ancestor `{}` is not owner-or-root held",
                    path.display()
                ),
            ));
        }
        if observed.mode() & 0o022 == 0 {
            continue;
        }
        let sticky_root_boundary = observed.uid() == 0 && observed.mode() & 0o1000 != 0;
        let protected_child = metadata
            .get(index + 1)
            .is_some_and(|child| child.uid() == owner_uid && child.mode() & 0o022 == 0);
        let planned_private_child = index + 1 == metadata.len();
        if !sticky_root_boundary || (!protected_child && !planned_private_child) {
            return Err(IoError::new(
                ErrorKind::PermissionDenied,
                format!(
                    "rotation key output ancestor `{}` is replaceable by another principal",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}
fn print_metadata(metadata: &DirectoryMetadata) {
    println!("directory_hash: {}", metadata.directory_hash_hex);
    println!(
        "validity: published={} valid_after={} valid_until={}",
        metadata.published_at_unix, metadata.valid_after_unix, metadata.valid_until_unix
    );
    println!("issuers ({}):", metadata.issuers.len());
    for issuer in &metadata.issuers {
        let label = issuer.label.as_deref().unwrap_or("-");
        println!(
            "  - {label}: fingerprint={}, ed25519={}",
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
                "    guard={} reputation={} bandwidth={}",
                proof.guard_weight, proof.reputation_weight, proof.bandwidth_bytes_per_sec
            );
            println!(
                "    validity: {} -> {}",
                proof.valid_after_unix, proof.valid_until_unix
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rotation_keys() -> RotationKeys {
        RotationKeys {
            ed25519_secret: [0x11; 32],
            ed25519_public: [0x22; 32],
            mldsa_public: vec![0x33; 64],
            mldsa_secret: vec![0x44; 96],
            fingerprint: [0x55; 32],
        }
    }

    #[test]
    fn rotate_requires_an_independently_supplied_source_digest() {
        let error = Args::try_parse_from([
            "soranet-directory",
            "rotate",
            "--snapshot",
            "source.norito",
            "--out",
            "rotated.norito",
            "--keys-out",
            "issuer-keys",
        ])
        .expect_err("rotation without an expected source digest must be rejected");
        assert!(error.to_string().contains("--expected-snapshot-digest"));
    }

    #[test]
    fn rotate_requires_a_key_output_directory() {
        let digest = "11".repeat(32);
        let error = Args::try_parse_from([
            "soranet-directory",
            "rotate",
            "--snapshot",
            "source.norito",
            "--expected-snapshot-digest",
            digest.as_str(),
            "--out",
            "rotated.norito",
        ])
        .expect_err("rotation without durable key output must be rejected");
        assert!(error.to_string().contains("--keys-out"));
    }

    #[test]
    fn expected_snapshot_digest_parser_is_canonical_and_nonzero() {
        let expected = [0xabu8; 32];
        assert_eq!(
            parse_expected_snapshot_digest(&hex::encode(expected)).expect("canonical digest"),
            expected
        );
        assert!(parse_expected_snapshot_digest(&"AB".repeat(32)).is_err());
        assert!(parse_expected_snapshot_digest(&"00".repeat(32)).is_err());
        assert!(parse_expected_snapshot_digest("abcd").is_err());
    }

    #[test]
    fn secret_hex_temporary_is_wiped_explicitly() {
        let mut encoded = encode_secret_hex_line(&[0xab, 0xcd]).expect("encode secret");
        assert_eq!(encoded.expose(), b"abcd\n");
        encoded.clear();
        assert!(encoded.expose().iter().all(|byte| *byte == 0));
    }

    #[test]
    fn rotation_key_bundle_refuses_to_replace_existing_path() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("issuer-keys");
        fs::create_dir(&output).expect("create existing output");
        let error = store_rotation_keys(&output, &test_rotation_keys())
            .expect_err("existing key output must not be replaced");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    }

    #[test]
    fn rotation_key_failure_leaves_snapshot_unpublished() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let snapshot = temporary.path().join("rotated.norito");
        let staged = stage_output(&snapshot, b"rotated snapshot", false)
            .expect("stage snapshot without publishing it");
        let keys_out = temporary.path().join("issuer-keys");
        fs::create_dir(&keys_out).expect("create conflicting key destination");

        let error = publish_rotation_artifacts(staged, &keys_out, &test_rotation_keys())
            .expect_err("key publication failure must abort snapshot publication");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert!(!snapshot.exists());
        assert!(keys_out.is_dir());
    }

    #[test]
    fn snapshot_publication_race_rolls_back_new_key_bundle() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let snapshot = temporary.path().join("rotated.norito");
        let staged = stage_output(&snapshot, b"rotated snapshot", false)
            .expect("stage snapshot without publishing it");
        fs::write(&snapshot, b"racing publisher").expect("race snapshot destination");
        let keys_out = temporary.path().join("issuer-keys");

        let error = publish_rotation_artifacts(staged, &keys_out, &test_rotation_keys())
            .expect_err("snapshot publication race must roll back generated keys");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(&snapshot).expect("read racing destination"),
            b"racing publisher"
        );
        assert!(!keys_out.exists());
    }

    #[test]
    fn output_publication_refuses_to_replace_existing_file() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("directory.norito");
        fs::write(&output, b"existing").expect("write existing output");
        let error = write_output(&output, b"replacement", false)
            .expect_err("non-overwrite publication must fail closed");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(output).expect("read retained output"), b"existing");
    }

    #[cfg(unix)]
    #[test]
    fn explicit_output_overwrite_replaces_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let target = temporary.path().join("unrelated");
        let output = temporary.path().join("directory.norito");
        fs::write(&target, b"retain me").expect("write unrelated target");
        symlink(&target, &output).expect("create output symlink");

        write_output(&output, b"published", true).expect("publish without following symlink");
        assert_eq!(
            fs::read(&target).expect("read unrelated target"),
            b"retain me"
        );
        assert_eq!(
            fs::read(&output).expect("read published output"),
            b"published"
        );
        assert!(
            !fs::symlink_metadata(output)
                .expect("published metadata")
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rotation_key_bundle_is_complete_and_owner_private() {
        use std::os::unix::fs::MetadataExt as _;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("issuer-keys");
        let keys = test_rotation_keys();
        store_rotation_keys(&output, &keys).expect("store protected key bundle");

        let directory_metadata = fs::symlink_metadata(&output).expect("key directory metadata");
        assert!(directory_metadata.is_dir());
        assert_eq!(directory_metadata.mode() & 0o777, 0o700);

        let expected = [
            (
                "issuer_ed25519_secret.hex",
                format!("{}\n", hex::encode(keys.ed25519_secret)).into_bytes(),
            ),
            (
                "issuer_ed25519_public.hex",
                format!("{}\n", hex::encode(keys.ed25519_public)).into_bytes(),
            ),
            (
                "issuer_mldsa_public.hex",
                format!("{}\n", hex::encode(&keys.mldsa_public)).into_bytes(),
            ),
            (
                "issuer_mldsa_secret.hex",
                format!("{}\n", hex::encode(&keys.mldsa_secret)).into_bytes(),
            ),
            ("issuer_mldsa_secret.bin", keys.mldsa_secret.clone()),
            (
                "issuer_fingerprint.hex",
                format!("{}\n", hex::encode(keys.fingerprint)).into_bytes(),
            ),
        ];
        for (name, contents) in expected {
            let path = output.join(name);
            assert_eq!(fs::read(&path).expect("read key artifact"), contents);
            let metadata = fs::symlink_metadata(path).expect("key artifact metadata");
            assert!(metadata.is_file());
            assert_eq!(metadata.mode() & 0o777, 0o600);
            assert_eq!(metadata.nlink(), 1);
        }
    }
}
