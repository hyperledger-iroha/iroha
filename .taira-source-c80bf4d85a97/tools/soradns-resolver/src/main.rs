use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, bail};
use hex::{decode as hex_decode, encode as hex_encode};
use iroha_data_model::soradns::ResolverDirectoryRecordV1;
use sha2::{Digest, Sha256};
use soradns_resolver::{
    ResolverDaemon,
    config::ResolverConfig,
    directory::{parse_directory_listing, signing_payload_bytes},
    rad::{compute_rad_digest, decode_rad_entries, validate_rad},
};
use tracing_subscriber::EnvFilter;

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
    },
    /// Verify a previously downloaded resolver directory bundle.
    Verify {
        /// Path containing `record.json`, `directory.json`, and the `rad/` tree.
        #[arg(long, default_value = ".")]
        bundle: PathBuf,
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
            } => {
                let record_source = parse_fetch_source(record_url, record_file, "record")?;
                let directory_source =
                    parse_fetch_source(directory_url, directory_file, "directory")?;
                run_directory_fetch(record_source, directory_source, output).await
            }
            DirectoryCommand::Verify { bundle } => run_directory_verify(bundle),
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

    let mut verified = 0usize;
    for path in paths {
        let bytes =
            fs::read(&path).wrap_err_with(|| format!("failed to read `{}`", path.display()))?;
        let entries = decode_rad_entries(&bytes).wrap_err_with(|| {
            format!("failed to decode resolver attestation `{}`", path.display())
        })?;
        if entries.is_empty() {
            bail!("no RAD entries were found in `{}`", path.display());
        }
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
            verified += 1;
        }
    }

    println!("Validated {verified} RAD entries");
    Ok(())
}

async fn run_directory_fetch(
    record_source: FetchSource,
    directory_source: FetchSource,
    output: PathBuf,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("soradns-resolver-cli/0.1.0")
        .build()
        .wrap_err("failed to build HTTP client")?;

    let record_bytes = read_source(&client, &record_source).await?;
    let record: ResolverDirectoryRecordV1 = norito::json::from_slice(&record_bytes)
        .wrap_err("failed to decode resolver directory record")?;

    let directory_bytes = read_source(&client, &directory_source).await?;
    let (listing, digest) =
        parse_directory_listing(&directory_bytes).wrap_err("failed to parse directory.json")?;

    if digest != record.directory_json_sha256 {
        bail!(
            "directory.json digest mismatch (expected {}, got {})",
            hex_encode(record.directory_json_sha256),
            hex_encode(digest)
        );
    }
    if listing.entry_count() != record.rad_count as usize {
        bail!(
            "directory entry count mismatch (record declares {}, listing contains {})",
            record.rad_count,
            listing.entry_count()
        );
    }

    fs::create_dir_all(&output)
        .wrap_err_with(|| format!("failed to create `{}`", output.display()))?;
    let record_path = output.join("record.json");
    fs::write(&record_path, &record_bytes)
        .wrap_err_with(|| format!("failed to write `{}`", record_path.display()))?;
    let directory_path = output.join("directory.json");
    fs::write(&directory_path, &directory_bytes)
        .wrap_err_with(|| format!("failed to write `{}`", directory_path.display()))?;

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

fn run_directory_verify(bundle_root: PathBuf) -> Result<()> {
    let record_path = bundle_root.join("record.json");
    let directory_path = bundle_root.join("directory.json");
    let rad_root = bundle_root.join("rad");

    let record_bytes = fs::read(&record_path)
        .wrap_err_with(|| format!("failed to read `{}`", record_path.display()))?;
    let record: ResolverDirectoryRecordV1 = norito::json::from_slice(&record_bytes)
        .wrap_err("failed to decode resolver directory record")?;

    let directory_bytes = fs::read(&directory_path)
        .wrap_err_with(|| format!("failed to read `{}`", directory_path.display()))?;
    let (listing, digest) =
        parse_directory_listing(&directory_bytes).wrap_err("failed to parse directory.json")?;

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
    let listing_root = parse_hex_hash(&listing.merkle_root, "directory.merkle_root")?;
    if listing_root != record.root_hash {
        bail!(
            "directory root mismatch (record {}, directory.json {})",
            hex_encode(record.root_hash),
            hex_encode(listing_root)
        );
    }

    verify_directory_record_signature(&record)?;

    if !rad_root.is_dir() {
        bail!(
            "`{}` does not contain a `rad/` directory",
            bundle_root.display()
        );
    }

    let mut leaves = Vec::with_capacity(listing.entry_count());
    for entry in &listing.rad {
        let rad_path = bundle_root.join(Path::new(&entry.file));
        let bytes = fs::read(&rad_path)
            .wrap_err_with(|| format!("failed to read RAD `{}`", rad_path.display()))?;
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

    let computed_root = compute_merkle_root(&leaves);
    if computed_root != record.root_hash {
        bail!(
            "computed Merkle root {} differs from record {}",
            hex_encode(computed_root),
            hex_encode(record.root_hash)
        );
    }

    println!(
        "Verified {} RAD entries in `{}`",
        leaves.len(),
        bundle_root.display()
    );
    println!("Directory root {}", hex_encode(record.root_hash));
    Ok(())
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .wrap_err_with(|| format!("failed to fetch `{url}`"))?;
    if !response.status().is_success() {
        bail!(
            "request to `{url}` failed with status {}",
            response.status()
        );
    }
    Ok(response
        .bytes()
        .await
        .wrap_err_with(|| format!("failed to read body from `{url}`"))?
        .to_vec())
}

async fn read_source(client: &reqwest::Client, source: &FetchSource) -> Result<Vec<u8>> {
    match source {
        FetchSource::Url(url) => fetch_bytes(client, url).await,
        FetchSource::File(path) => {
            fs::read(path).wrap_err_with(|| format!("failed to read `{}`", path.display()))
        }
    }
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

fn verify_directory_record_signature(record: &ResolverDirectoryRecordV1) -> Result<()> {
    let payload =
        signing_payload_bytes(record).wrap_err("failed to build directory signing payload")?;
    record
        .builder_signature
        .verify(&record.builder_public_key, &payload)
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

fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    assert!(!leaves.is_empty(), "at least one leaf is required");
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
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
    level[0]
}
