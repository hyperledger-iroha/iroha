//! Content hosting helpers.

use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
};

use crate::{Run, RunContext};
use eyre::{Result, WrapErr};
use iroha::data_model::{
    content::{ContentAuthMode, ContentBundleManifest, ContentCachePolicy},
    da::types::{BlobClass, RetentionPolicy},
    isi,
    nexus::{DataSpaceId, LaneId, UniversalAccountId},
    prelude::*,
};
use iroha_config::parameters::{actual, defaults};
use iroha_core::smartcontracts::isi::content::{hash_index, parse_tar_index};
use iroha_crypto::Hash;

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Publish a content bundle (tar archive) to the content lane.
    Publish(PublishArgs),
    /// Pack a directory into a deterministic tarball + manifest without submitting it.
    Pack(PackArgs),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Publish(args) => args.run(context),
            Command::Pack(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct PublishArgs {
    /// Path to a tar archive containing the static bundle.
    #[arg(long, value_name = "PATH", conflicts_with = "root")]
    pub bundle: Option<PathBuf>,
    /// Directory to pack into a tarball before publishing.
    #[arg(long, value_name = "DIR", conflicts_with = "bundle")]
    pub root: Option<PathBuf>,
    /// Optional block height when the bundle expires.
    #[arg(long, value_name = "HEIGHT")]
    pub expires_at_height: Option<u64>,
    /// Optional dataspace id override for the bundle manifest.
    #[arg(long, value_name = "ID")]
    pub dataspace: Option<u64>,
    /// Optional lane id override for the bundle manifest.
    #[arg(long, value_name = "ID")]
    pub lane: Option<u32>,
    /// Auth mode (`public`, `role:<role_id>`, `sponsor:<uaid>`).
    #[arg(long, value_name = "MODE")]
    pub auth: Option<String>,
    /// Cache-Control max-age override (seconds).
    #[arg(long, value_name = "SECS")]
    pub cache_max_age_secs: Option<u32>,
    /// Mark bundle as immutable (adds `immutable` to Cache-Control).
    #[arg(long)]
    pub immutable: bool,
    /// Optional path to write the packed tarball when using `--root`.
    #[arg(long, value_name = "PATH")]
    pub bundle_out: Option<PathBuf>,
    /// Optional path to write the generated manifest JSON.
    #[arg(long, value_name = "PATH")]
    pub manifest_out: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct PackArgs {
    /// Directory to pack into a tarball.
    #[arg(long, value_name = "DIR")]
    pub root: PathBuf,
    /// Path to write the tarball.
    #[arg(long, value_name = "PATH")]
    pub bundle_out: PathBuf,
    /// Path to write the generated manifest JSON.
    #[arg(long, value_name = "PATH")]
    pub manifest_out: PathBuf,
    /// Optional dataspace id override for the bundle manifest.
    #[arg(long, value_name = "ID")]
    pub dataspace: Option<u64>,
    /// Optional lane id override for the bundle manifest.
    #[arg(long, value_name = "ID")]
    pub lane: Option<u32>,
    /// Auth mode (`public`, `role:<role_id>`, `sponsor:<uaid>`).
    #[arg(long, value_name = "MODE")]
    pub auth: Option<String>,
    /// Cache-Control max-age override (seconds).
    #[arg(long, value_name = "SECS")]
    pub cache_max_age_secs: Option<u32>,
    /// Mark bundle as immutable (adds `immutable` to Cache-Control).
    #[arg(long)]
    pub immutable: bool,
}

impl PublishArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let defaults = default_content_config();

        let pack_result = if let Some(bundle_path) = self.bundle {
            let tarball = fs::read(&bundle_path)
                .wrap_err_with(|| format!("failed to read bundle at {}", bundle_path.display()))?;
            let manifest = build_manifest(
                &tarball,
                &defaults,
                self.dataspace,
                self.lane,
                self.auth.as_deref(),
                self.cache_max_age_secs,
                self.immutable,
            )?;
            PackResult { tarball, manifest }
        } else if let Some(root) = &self.root {
            let pack = pack_directory(
                root,
                &defaults,
                self.dataspace,
                self.lane,
                self.auth.as_deref(),
                self.cache_max_age_secs,
                self.immutable,
            )?;
            if let Some(out) = &self.bundle_out {
                fs::write(out, &pack.tarball)
                    .wrap_err_with(|| format!("failed to write bundle to {}", out.display()))?;
            }
            if let Some(out) = &self.manifest_out {
                let bytes =
                    norito::json::to_vec_pretty(&pack.manifest).wrap_err("encode manifest JSON")?;
                fs::write(out, bytes)
                    .wrap_err_with(|| format!("failed to write manifest to {}", out.display()))?;
            }
            pack
        } else {
            eyre::bail!("either --bundle or --root must be supplied");
        };

        let bundle_id = Hash::new(&pack_result.tarball);
        let isi = isi::content::PublishContentBundle {
            bundle_id,
            tarball: pack_result.tarball,
            expires_at_height: self.expires_at_height,
            manifest: Some(pack_result.manifest),
        };
        context.finish(vec![InstructionBox::from(isi)])
    }
}

impl PackArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        let defaults = default_content_config();
        let pack = pack_directory(
            &self.root,
            &defaults,
            self.dataspace,
            self.lane,
            self.auth.as_deref(),
            self.cache_max_age_secs,
            self.immutable,
        )?;

        fs::write(&self.bundle_out, &pack.tarball).wrap_err_with(|| {
            format!(
                "failed to write bundle to {}",
                self.bundle_out.as_path().display()
            )
        })?;
        let manifest_bytes =
            norito::json::to_vec_pretty(&pack.manifest).wrap_err("encode manifest JSON")?;
        fs::write(&self.manifest_out, manifest_bytes).wrap_err_with(|| {
            format!(
                "failed to write manifest to {}",
                self.manifest_out.as_path().display()
            )
        })?;
        Ok(())
    }
}

struct PackResult {
    tarball: Vec<u8>,
    manifest: ContentBundleManifest,
}

fn default_content_config() -> actual::Content {
    actual::Content {
        max_bundle_bytes: defaults::content::MAX_BUNDLE_BYTES,
        max_files: defaults::content::MAX_FILES,
        max_path_len: defaults::content::MAX_PATH_LEN,
        max_retention_blocks: defaults::content::MAX_RETENTION_BLOCKS,
        chunk_size_bytes: defaults::content::CHUNK_SIZE_BYTES,
        publish_allow_accounts: Vec::new(),
        limits: actual::ContentLimits {
            max_requests_per_second: NonZeroU32::new(defaults::content::MAX_REQUESTS_PER_SECOND)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            request_burst: NonZeroU32::new(defaults::content::REQUEST_BURST)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            max_egress_bytes_per_second: NonZeroU64::new(u64::from(
                defaults::content::MAX_EGRESS_BYTES_PER_SECOND,
            ))
            .unwrap_or_else(|| NonZeroU64::new(1).unwrap()),
            egress_burst_bytes: NonZeroU64::new(defaults::content::EGRESS_BURST_BYTES)
                .unwrap_or_else(|| NonZeroU64::new(1).unwrap()),
        },
        default_cache_max_age_secs: defaults::content::DEFAULT_CACHE_MAX_AGE_SECS,
        max_cache_max_age_secs: defaults::content::MAX_CACHE_MAX_AGE_SECS,
        immutable_bundles: defaults::content::IMMUTABLE_BUNDLES,
        default_auth_mode: ContentAuthMode::Public,
        slo: actual::ContentSlo {
            target_p50_latency_ms: NonZeroU32::new(defaults::content::TARGET_P50_LATENCY_MS)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            target_p99_latency_ms: NonZeroU32::new(defaults::content::TARGET_P99_LATENCY_MS)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            target_availability_bps: NonZeroU32::new(defaults::content::TARGET_AVAILABILITY_BPS)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
        },
        pow: actual::ContentPow {
            difficulty_bits: defaults::content::POW_DIFFICULTY_BITS,
            header_name: defaults::content::default_pow_header(),
        },
        stripe_layout: defaults::content::default_stripe_layout(),
    }
}

fn pack_directory(
    root: &Path,
    defaults: &actual::Content,
    dataspace: Option<u64>,
    lane: Option<u32>,
    auth: Option<&str>,
    cache_max_age_secs: Option<u32>,
    immutable: bool,
) -> Result<PackResult> {
    let entries = collect_entries(root)?;
    let tarball = build_tar(&entries)?;
    let manifest = build_manifest(
        &tarball,
        defaults,
        dataspace,
        lane,
        auth,
        cache_max_age_secs,
        immutable,
    )?;
    Ok(PackResult { tarball, manifest })
}

fn build_manifest(
    tarball: &[u8],
    defaults: &actual::Content,
    dataspace: Option<u64>,
    lane: Option<u32>,
    auth: Option<&str>,
    cache_max_age_secs: Option<u32>,
    immutable: bool,
) -> Result<ContentBundleManifest> {
    let files = parse_tar_index(tarball, defaults.max_files, defaults.max_path_len, defaults)
        .wrap_err("failed to parse tarball index")?;

    let index_hash = hash_index(&files).wrap_err("failed to hash index")?;
    let cache_max_age = cache_max_age_secs
        .unwrap_or(defaults.default_cache_max_age_secs)
        .min(defaults.max_cache_max_age_secs)
        .max(1);
    let auth_mode = parse_auth_mode(auth).wrap_err("invalid auth mode")?;

    let mut mime_overrides = BTreeMap::new();
    for entry in &files {
        if let Some(mime) = guess_mime(&entry.path) {
            mime_overrides.insert(entry.path.clone(), mime);
        }
    }

    Ok(ContentBundleManifest {
        bundle_id: Hash::new(tarball),
        index_hash,
        dataspace: dataspace.map_or(DataSpaceId::GLOBAL, DataSpaceId::new),
        lane: lane.map_or(LaneId::SINGLE, LaneId::new),
        blob_class: BlobClass::GovernanceArtifact,
        retention: RetentionPolicy::default(),
        cache: ContentCachePolicy {
            max_age_seconds: cache_max_age,
            immutable: immutable || defaults.immutable_bundles,
        },
        auth: auth_mode,
        stripe_layout: defaults.stripe_layout,
        mime_overrides,
    })
}

fn parse_auth_mode(raw: Option<&str>) -> Result<ContentAuthMode> {
    let Some(raw) = raw else {
        return Ok(ContentAuthMode::Public);
    };
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("public") {
        return Ok(ContentAuthMode::Public);
    }
    if let Some(role_str) = trimmed.strip_prefix("role:") {
        let role = role_str
            .parse::<RoleId>()
            .wrap_err("invalid role id in auth mode")?;
        return Ok(ContentAuthMode::RoleGate(role));
    }
    if let Some(uaid_raw) = trimmed.strip_prefix("sponsor:") {
        let cleaned = uaid_raw
            .trim()
            .strip_prefix("uaid:")
            .unwrap_or_else(|| uaid_raw.trim());
        let uaid = cleaned
            .parse::<UniversalAccountId>()
            .wrap_err("invalid UAID in auth mode")?;
        return Ok(ContentAuthMode::Sponsor(uaid));
    }
    eyre::bail!("unsupported auth mode `{trimmed}`")
}

fn collect_entries(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut stack = vec![root.to_path_buf()];
    let mut entries = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).wrap_err_with(|| format!("read dir {}", dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .wrap_err_with(|| format!("strip prefix {}", path.display()))?;
            let rel_str = rel
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else if entry.file_type()?.is_file() {
                if rel_str.is_empty() {
                    continue;
                }
                let data =
                    fs::read(&path).wrap_err_with(|| format!("read file {}", path.display()))?;
                entries.push((rel_str, data));
            }
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn build_tar(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    const HEADER_LEN: usize = 512;
    let mut out = Vec::new();
    for (path, data) in entries {
        if path.len() > defaults::content::MAX_PATH_LEN as usize {
            eyre::bail!(
                "path `{path}` exceeds max length {}",
                defaults::content::MAX_PATH_LEN
            );
        }
        let (name, prefix) = split_tar_path(path)?;
        let mut header = [0u8; HEADER_LEN];
        header[..name.len()].copy_from_slice(name.as_bytes());
        let size_str = format!("{:0>11o}\0", data.len());
        header[124..124 + size_str.len()].copy_from_slice(size_str.as_bytes());
        header[156] = b'0';
        if !prefix.is_empty() {
            header[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
        }
        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let pad = (HEADER_LEN - (data.len() % HEADER_LEN)) % HEADER_LEN;
        out.resize(out.len() + pad, 0);
    }
    out.resize(out.len() + HEADER_LEN * 2, 0);
    Ok(out)
}

fn split_tar_path(path: &str) -> Result<(String, String)> {
    const NAME_LIMIT: usize = 100;
    const PREFIX_LIMIT: usize = 155;
    if path.len() <= NAME_LIMIT {
        return Ok((path.to_string(), String::new()));
    }
    let mut parts = path.rsplitn(2, '/');
    let name = parts.next().unwrap_or(path);
    let prefix = parts.next().unwrap_or("");
    if name.len() > NAME_LIMIT {
        eyre::bail!("file name `{name}` exceeds tar header limit");
    }
    if prefix.len() > PREFIX_LIMIT {
        eyre::bail!("path prefix `{prefix}` exceeds tar header limit");
    }
    Ok((name.to_string(), prefix.to_string()))
}

fn guess_mime(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        "ico" => "image/x-icon",
        "gif" => "image/gif",
        _ => return None,
    };
    Some(mime.to_string())
}
