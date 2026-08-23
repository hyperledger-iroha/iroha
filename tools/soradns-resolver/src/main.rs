use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, bail};
use hex::{decode as hex_decode, encode as hex_encode};
use iroha_crypto::PublicKey;
use iroha_data_model::soradns::ResolverDirectoryRecordV1;
use sha2::{Digest, Sha256};
use soradns_resolver::{
    ResolverDaemon,
    config::ResolverConfig,
    directory::{DirectoryListing, parse_directory_listing, signing_payload_bytes},
    limits::{
        MAX_DIRECTORY_JSON_BYTES, MAX_DIRECTORY_RECORD_BYTES, MAX_IDENTIFIER_BYTES,
        MAX_RAD_ENTRIES, MAX_RAD_SNAPSHOT_BYTES, directory_record_decode_limits, preflight_json,
        read_bounded_file, read_http_body_bounded,
    },
    rad::{compute_rad_digest, decode_rad_entries, validate_rad},
};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing_subscriber::EnvFilter;
const HTTP_CONNECT_TIMEOUT_V1: Duration = Duration::from_secs(5);
const HTTP_REQUEST_TIMEOUT_V1: Duration = Duration::from_secs(15);
const HTTP_REDIRECT_LIMIT_V1: usize = 5;
#[derive(Debug, Parser)]
#[command(author, version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the resolver daemon using the supplied configuration.
    Serve {
        /// Path to the resolver configuration file (Norito JSON).
        #[arg(long, default_value = "soradns-resolver.json")]
        config: PathBuf,
        /// Override the configured sync interval (seconds) without editing the config file.
        #[arg(long)]
        sync_interval_secs: Option<u64>,
    },
    /// RAD-related helpers.
    #[command(subcommand)]
    Rad(RadCommand),
    /// Directory-related helpers.
    #[command(subcommand)]
    Directory(DirectoryCommand),
}
#[derive(Debug, Subcommand)]
enum RadCommand {
    /// Validate resolver attestation documents (RADs).
    Verify {
        /// One or more RAD files (Norito or JSON).
        #[arg(required = true)]
        rad: Vec<PathBuf>,
    },
}
#[derive(Debug, Subcommand)]
enum DirectoryCommand {
    /// Fetch and verify the latest resolver directory bundle.
    Fetch {
        /// URL returning the latest `ResolverDirectoryRecordV1` payload (Norito JSON).
        #[arg(long, conflicts_with = "record_file")]
        record_url: Option<String>,
        /// Local path to a `ResolverDirectoryRecordV1` payload.
        #[arg(long, conflicts_with = "record_url")]
        record_file: Option<PathBuf>,
        /// URL serving the corresponding `directory.json` artifact.
        #[arg(long, conflicts_with = "directory_file")]
        directory_url: Option<String>,
        /// Local path to a `directory.json` artifact.
        #[arg(long, conflicts_with = "directory_url")]
        directory_file: Option<PathBuf>,
        /// Directory to store the fetched artifacts.
        #[arg(long, default_value = "soradns-directory")]
        output: PathBuf,
        /// Governance-pinned directory builder public key.
        #[arg(long)]
        builder_public_key: String,
        /// Governance-pinned Merkle root expected for this exact release.
        #[arg(long)]
        expected_root: String,
    },
    /// Verify a previously downloaded resolver directory bundle.
    Verify {
        /// Path containing `record.json`, `directory.json`, and the `rad/` tree.
        #[arg(long, default_value = ".")]
        bundle: PathBuf,
        /// Governance-pinned directory builder public key.
        #[arg(long)]
        builder_public_key: String,
        /// Governance-pinned Merkle root expected for this exact release.
        #[arg(long)]
        expected_root: String,
    },
}
#[derive(Debug)]
enum FetchSource {
    Url(String),
    File(PathBuf),
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing();
    match cli.command.unwrap_or_else(|| Command::Serve {
        config: PathBuf::from("soradns-resolver.json"),
        sync_interval_secs: None,
    }) {
        Command::Serve {
            config,
            sync_interval_secs,
        } => run_serve(config, sync_interval_secs).await,
        Command::Rad(rad_cmd) => match rad_cmd {
            RadCommand::Verify { rad } => run_rad_verify(rad),
        },
        Command::Directory(directory_cmd) => match directory_cmd {
            DirectoryCommand::Fetch {
                record_url,
                record_file,
                directory_url,
                directory_file,
                output,
                builder_public_key,
                expected_root,
            } => {
                let record_source = parse_fetch_source(record_url, record_file, "record")?;
                let directory_source =
                    parse_fetch_source(directory_url, directory_file, "directory")?;
                let builder_public_key = parse_builder_trust_anchor(&builder_public_key)?;
                let expected_root = parse_hex_hash(&expected_root, "expected root")?;
                run_directory_fetch(
                    record_source,
                    directory_source,
                    output,
                    &builder_public_key,
                    expected_root,
                )
                .await
            }
            DirectoryCommand::Verify {
                bundle,
                builder_public_key,
                expected_root,
            } => {
                let builder_public_key = parse_builder_trust_anchor(&builder_public_key)?;
                let expected_root = parse_hex_hash(&expected_root, "expected root")?;
                run_directory_verify(bundle, &builder_public_key, expected_root)
            }
        },
    }
}
async fn run_serve(config_path: PathBuf, sync_interval_override: Option<u64>) -> Result<()> {
    let mut config = ResolverConfig::load_from_path(&config_path)
        .wrap_err_with(|| format!("failed to load config `{}`", config_path.display()))?;
    if let Some(secs) = sync_interval_override {
        let interval = Duration::from_secs(secs);
        config
            .override_sync_interval(interval)
            .wrap_err("failed to apply sync interval override")?;
    }
    config.validate()?;
    let daemon = ResolverDaemon::new(config)?;
    daemon.run().await
}
fn parse_fetch_source(
    url: Option<String>,
    file: Option<PathBuf>,
    label: &str,
) -> Result<FetchSource> {
    match (url, file) {
        (Some(url), None) => Ok(FetchSource::Url(url)),
        (None, Some(path)) => Ok(FetchSource::File(path)),
        _ => bail!("provide either --{label}-url or --{label}-file"),
    }
}
fn run_rad_verify(paths: Vec<PathBuf>) -> Result<()> {
    if paths.is_empty() {
        bail!("supply at least one RAD file to verify");
    }
    if paths.len() > MAX_RAD_ENTRIES {
        bail!(
            "RAD verify received {} files; the limit is {MAX_RAD_ENTRIES}",
            paths.len()
        );
    }
    let mut verified = 0usize;
    for path in paths {
        let bytes = read_bounded_file(&path, MAX_RAD_SNAPSHOT_BYTES, "RAD input")?;
        let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
            format!("failed to decode resolver attestation `{}`", path.display())
        })?;
        if entries.is_empty() {
            bail!("no RAD entries were found in `{}`", path.display());
        }
        verified = verified
            .checked_add(entries.len())
            .filter(|count| *count <= MAX_RAD_ENTRIES)
            .ok_or_else(|| {
                eyre::eyre!("RAD verify inputs exceed the aggregate {MAX_RAD_ENTRIES}-entry limit")
            })?;
        for entry in entries {
            validate_rad(&entry)
                .wrap_err_with(|| format!("RAD validation failed for `{}`", entry.fqdn))?;
            let digest = compute_rad_digest(&entry).wrap_err("failed to hash RAD")?;
            println!(
                "[ok] fqdn={} resolver_id={} valid_from={} valid_until={} digest={}",
                entry.fqdn,
                hex_encode(entry.resolver_id),
                entry.valid_from_unix,
                entry.valid_until_unix,
                hex_encode(digest),
            );
        }
    }
    println!("Validated {verified} RAD entries");
    Ok(())
}
async fn run_directory_fetch(
    record_source: FetchSource,
    directory_source: FetchSource,
    output: PathBuf,
    trusted_builder_public_key: &PublicKey,
    expected_root: [u8; 32],
) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("soradns-resolver-cli/0.1.0")
        .connect_timeout(HTTP_CONNECT_TIMEOUT_V1)
        .timeout(HTTP_REQUEST_TIMEOUT_V1)
        .redirect(reqwest::redirect::Policy::limited(HTTP_REDIRECT_LIMIT_V1))
        .build()
        .wrap_err("failed to build HTTP client")?;
    let record_bytes = read_source(
        &client,
        &record_source,
        MAX_DIRECTORY_RECORD_BYTES,
        "resolver directory record",
    )
    .await?;
    let record = decode_directory_record(&record_bytes)?;
    let directory_bytes = read_source(
        &client,
        &directory_source,
        MAX_DIRECTORY_JSON_BYTES,
        "resolver directory listing",
    )
    .await?;
    let (listing, digest) =
        parse_directory_listing(&directory_bytes).wrap_err("failed to parse directory.json")?;
    verify_directory_metadata(
        &record,
        &listing,
        digest,
        trusted_builder_public_key,
        expected_root,
    )?;
    let output = create_fresh_private_output_directory(&output)?;
    let record_path = output.join("record.json");
    write_new_private_file(&record_path, &record_bytes)?;
    let directory_path = output.join("directory.json");
    write_new_private_file(&directory_path, &directory_bytes)?;
    println!(
        "Fetched resolver directory root {} ({} RAD entries)",
        hex_encode(record.root_hash),
        listing.entry_count()
    );
    println!(
        "Saved artifacts to `{}` (record + directory JSON)",
        output.display()
    );
    Ok(())
}
fn create_fresh_private_output_directory(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .ok_or_else(|| eyre::eyre!("directory output must name a fresh child directory"))?;
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent)
        .wrap_err_with(|| format!("failed to resolve output parent `{}`", parent.display()))?;
    validate_output_parent_chain(&parent)?;
    let output = parent.join(name);
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(&output)
        .wrap_err_with(|| format!("failed to create fresh `{}`", output.display()))?;
    let metadata = fs::symlink_metadata(&output)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "fresh output `{}` is not a direct directory",
            output.display()
        );
    }
    #[cfg(unix)]
    {
        let effective_uid = soradns_resolver::limits::current_process_owner_uid()?;
        if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 {
            bail!(
                "fresh output `{}` must be owned by the effective user with no group or other permissions",
                output.display()
            );
        }
    }
    Ok(output)
}
#[cfg(unix)]
fn validate_output_parent_chain(parent: &Path) -> Result<()> {
    let effective_uid = soradns_resolver::limits::current_process_owner_uid()?;
    for ancestor in parent.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!(
                "output parent `{}` must be a direct directory",
                ancestor.display()
            );
        }
        if !soradns_resolver::limits::trusted_private_ancestor(
            metadata.uid(),
            metadata.mode(),
            effective_uid,
        ) {
            bail!(
                "output parent `{}` must be owned by the effective user or root and must not be unsafely writable",
                ancestor.display()
            );
        }
    }
    Ok(())
}
#[cfg(not(unix))]
fn validate_output_parent_chain(parent: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "output parent `{}` must be a direct directory",
            parent.display()
        );
    }
    Ok(())
}
fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(no_follow_flag());
    let mut file = options
        .open(path)
        .wrap_err_with(|| format!("failed to create `{}`", path.display()))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("output `{}` is not a regular file", path.display());
    }
    #[cfg(unix)]
    {
        let effective_uid = soradns_resolver::limits::current_process_owner_uid()?;
        if metadata.uid() != effective_uid || metadata.nlink() != 1 || metadata.mode() & 0o077 != 0
        {
            bail!(
                "output `{}` is not an effective-user-owned private single-link file",
                path.display()
            );
        }
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
#[cfg(unix)]
const fn no_follow_flag() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        0x20_000
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        0x0100
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    {
        0x0100
    }
}
fn run_directory_verify(
    bundle_root: PathBuf,
    trusted_builder_public_key: &PublicKey,
    expected_root: [u8; 32],
) -> Result<()> {
    let record_path = bundle_root.join("record.json");
    let directory_path = bundle_root.join("directory.json");
    let rad_root = bundle_root.join("rad");
    let record_bytes = read_bounded_file(
        &record_path,
        MAX_DIRECTORY_RECORD_BYTES,
        "resolver directory record",
    )?;
    let record = decode_directory_record(&record_bytes)?;
    let directory_bytes = read_bounded_file(
        &directory_path,
        MAX_DIRECTORY_JSON_BYTES,
        "resolver directory listing",
    )?;
    let (listing, digest) =
        parse_directory_listing(&directory_bytes).wrap_err("failed to parse directory.json")?;
    verify_directory_metadata(
        &record,
        &listing,
        digest,
        trusted_builder_public_key,
        expected_root,
    )?;
    if !rad_root.is_dir() {
        bail!(
            "`{}` does not contain a `rad/` directory",
            bundle_root.display()
        );
    }
    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(listing.entry_count())
        .wrap_err("failed to reserve bounded directory Merkle leaf table")?;
    for entry in &listing.rad {
        let rad_path = bundle_root.join(Path::new(&entry.file));
        let bytes = read_bounded_file(&rad_path, MAX_RAD_SNAPSHOT_BYTES, "directory RAD")?;
        let mut decoded = decode_rad_entries(&bytes).wrap_err_with(|| {
            format!(
                "failed to decode resolver attestation `{}`",
                rad_path.display()
            )
        })?;
        if decoded.len() != 1 {
            bail!(
                "expected `{}` to contain exactly one RAD entry (found {})",
                rad_path.display(),
                decoded.len()
            );
        }
        let rad = decoded.pop().expect("exactly one entry");
        let resolver_id_hex = hex_encode(rad.resolver_id);
        if resolver_id_hex != entry.resolver_id {
            bail!(
                "RAD identity mismatch for `{}` (directory lists {}, document contains {})",
                rad_path.display(),
                entry.resolver_id,
                resolver_id_hex
            );
        }
        validate_rad(&rad).wrap_err_with(|| {
            format!(
                "resolver attestation validation failed for `{}`",
                rad_path.display()
            )
        })?;
        let digest =
            compute_rad_digest(&rad).wrap_err("failed to compute resolver attestation digest")?;
        let digest_hex = hex_encode(digest);
        if !digest_hex.eq_ignore_ascii_case(&entry.rad_sha256) {
            bail!(
                "RAD digest mismatch for `{}` (directory lists {}, computed {})",
                rad_path.display(),
                entry.rad_sha256,
                digest_hex
            );
        }
        let leaf = hash_leaf(&digest);
        let expected_leaf = parse_hex_hash(&entry.leaf_hash, "directory.rad[].leaf_hash")?;
        if leaf != expected_leaf {
            bail!(
                "MERKLE leaf mismatch for `{}` (directory lists {}, computed {})",
                rad_path.display(),
                entry.leaf_hash,
                hex_encode(leaf)
            );
        }
        leaves.push(leaf);
    }
    if leaves.is_empty() {
        bail!("directory bundle does not contain any RAD entries");
    }
    let leaf_count = leaves.len();
    let computed_root = compute_merkle_root(leaves)?;
    if computed_root != record.root_hash {
        bail!(
            "computed Merkle root {} differs from record {}",
            hex_encode(computed_root),
            hex_encode(record.root_hash)
        );
    }
    println!(
        "Verified {} RAD entries in `{}`",
        leaf_count,
        bundle_root.display()
    );
    println!("Directory root {}", hex_encode(record.root_hash));
    Ok(())
}
fn verify_directory_metadata(
    record: &ResolverDirectoryRecordV1,
    listing: &DirectoryListing,
    digest: [u8; 32],
    trusted_builder_public_key: &PublicKey,
    expected_root: [u8; 32],
) -> Result<()> {
    if record.root_hash != expected_root {
        bail!(
            "directory record root {} does not match the governance-pinned expected root {}",
            hex_encode(record.root_hash),
            hex_encode(expected_root)
        );
    }
    if digest != record.directory_json_sha256 {
        bail!(
            "directory.json digest mismatch (record declares {}, computed {})",
            hex_encode(record.directory_json_sha256),
            hex_encode(digest)
        );
    }
    if listing.entry_count() != record.rad_count as usize {
        bail!(
            "RAD count mismatch (record declares {}, directory.json contains {})",
            record.rad_count,
            listing.entry_count()
        );
    }
    if listing.created_at_ms != record.created_at_ms {
        bail!(
            "directory creation time mismatch (record declares {}, listing contains {})",
            record.created_at_ms,
            listing.created_at_ms
        );
    }
    let listing_root = parse_hex_hash(&listing.merkle_root, "directory.merkle_root")?;
    if listing_root != record.root_hash {
        bail!(
            "directory root mismatch (record {}, directory.json {})",
            hex_encode(record.root_hash),
            hex_encode(listing_root)
        );
    }
    verify_directory_record_signature(record, trusted_builder_public_key)
}
async fn fetch_bytes(
    client: &reqwest::Client,
    url: &str,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .wrap_err_with(|| format!("failed to fetch `{url}`"))?;
    read_http_body_bounded(response, max_bytes, label).await
}
async fn read_source(
    client: &reqwest::Client,
    source: &FetchSource,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>> {
    match source {
        FetchSource::Url(url) => fetch_bytes(client, url, max_bytes, label).await,
        FetchSource::File(path) => read_bounded_file(path, max_bytes, label),
    }
}
fn decode_directory_record(bytes: &[u8]) -> Result<ResolverDirectoryRecordV1> {
    let decode_limits = directory_record_decode_limits();
    preflight_json(
        bytes,
        MAX_DIRECTORY_RECORD_BYTES,
        decode_limits,
        "resolver directory record",
    )?;
    let record: ResolverDirectoryRecordV1 =
        norito::with_decode_limits_scope(decode_limits, || norito::json::from_slice(bytes))
            .wrap_err("failed to decode resolver directory record")?;
    if record.rad_count as usize > MAX_RAD_ENTRIES {
        bail!(
            "resolver directory record declares {} RAD entries; the limit is {MAX_RAD_ENTRIES}",
            record.rad_count
        );
    }
    Ok(record)
}
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,soradns_resolver=debug"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .init();
}
fn parse_builder_trust_anchor(value: &str) -> Result<PublicKey> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        bail!("builder public key must contain 1 to {MAX_IDENTIFIER_BYTES} characters");
    }
    value
        .parse()
        .wrap_err("failed to parse pinned directory builder public key")
}
fn verify_directory_record_signature(
    record: &ResolverDirectoryRecordV1,
    trusted_builder_public_key: &PublicKey,
) -> Result<()> {
    if &record.builder_public_key != trusted_builder_public_key {
        bail!("directory record builder key does not match the pinned trust anchor");
    }
    let payload =
        signing_payload_bytes(record).wrap_err("failed to build directory signing payload")?;
    record
        .builder_signature
        .verify(trusted_builder_public_key, &payload)
        .wrap_err("directory record builder signature is invalid")
}
fn parse_hex_hash(value: &str, label: &str) -> Result<[u8; 32]> {
    let decoded =
        hex_decode(value).wrap_err_with(|| format!("`{label}` is not valid hex: {value}"))?;
    if decoded.len() != 32 {
        bail!(
            "`{label}` must decode to 32 bytes (found {} bytes)",
            decoded.len()
        );
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&decoded);
    Ok(hash)
}
fn hash_leaf(rad_digest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(rad_digest);
    hasher.finalize().into()
}
fn hash_branch(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}
fn compute_merkle_root(mut level: Vec<[u8; 32]>) -> Result<[u8; 32]> {
    if level.is_empty() {
        bail!("at least one Merkle leaf is required");
    }
    while level.len() > 1 {
        let mut next = Vec::new();
        next.try_reserve_exact(level.len().div_ceil(2))
            .wrap_err("failed to reserve bounded Merkle level")?;
        for chunk in level.chunks(2) {
            let branch = match chunk {
                [left, right] => hash_branch(left, right),
                [single] => hash_branch(single, single),
                _ => unreachable!(),
            };
            next.push(branch);
        }
        level = next;
    }
    Ok(level[0])
}
