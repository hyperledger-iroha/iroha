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
const TAR_BLOCK_BYTES: u64 = 512;
const TAR_TRAILER_BYTES: u64 = TAR_BLOCK_BYTES * 2;
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
            let tarball = read_content_file_bounded(
                &bundle_path,
                defaults.max_bundle_bytes,
                "content bundle",
            )?;
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
    let entries = collect_entries(root, defaults)?;
    let tarball = build_tar(&entries, defaults.max_bundle_bytes)?;
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
    if tarball.len() as u64 > defaults.max_bundle_bytes {
        eyre::bail!(
            "content bundle exceeds the configured limit of {} bytes",
            defaults.max_bundle_bytes
        );
    }
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
        dataspace: dataspace.map_or(DataSpaceId::UNIVERSAL, DataSpaceId::new),
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
fn read_content_file_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>> {
    let (file, expected_bytes) = open_content_file_bounded(path, max_bytes, label)?;
    read_open_content_file_bounded(file, expected_bytes, label, path)
}
fn open_content_file_bounded(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<(fs::File, usize)> {
    let path_metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} {}", path.display()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        eyre::bail!("{label} must be a direct regular file: {}", path.display());
    }
    let file = open_content_input_file(path)
        .wrap_err_with(|| format!("failed to open {label} {}", path.display()))?;
    let before = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} {}", path.display()))?;
    if !before.is_file() {
        eyre::bail!("{label} must be a regular file: {}", path.display());
    }
    if before.len() > max_bytes {
        eyre::bail!(
            "{label} {} exceeds the first-release limit of {} bytes",
            path.display(),
            max_bytes,
        );
    }
    let expected_bytes = usize::try_from(before.len())
        .map_err(|_| eyre::eyre!("{label} length cannot be represented on this host"))?;
    Ok((file, expected_bytes))
}
fn read_open_content_file_bounded(
    mut file: fs::File,
    expected_bytes: usize,
    label: &str,
    path: &Path,
) -> Result<Vec<u8>> {
    let bytes = super::read_cli_input_bounded(&mut file, expected_bytes, label)
        .wrap_err_with(|| format!("failed to read {label} {}", path.display()))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to reinspect {label} {}", path.display()))?;
    if after.len() != expected_bytes as u64 || after.len() != bytes.len() as u64 {
        eyre::bail!("{label} changed while reading: {}", path.display());
    }
    Ok(bytes)
}
#[cfg(unix)]
fn open_content_input_file(path: &Path) -> std::io::Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )?;
    Ok(fs::File::from(descriptor))
}
#[cfg(windows)]
fn open_content_input_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}
#[cfg(not(any(unix, windows)))]
fn open_content_input_file(path: &Path) -> std::io::Result<fs::File> {
    fs::File::open(path)
}
#[derive(Debug)]
struct ContentSourceEntry {
    path: String,
    data: Vec<u8>,
}
fn collect_entries(root: &Path, defaults: &actual::Content) -> Result<Vec<ContentSourceEntry>> {
    let root_metadata = fs::symlink_metadata(root)
        .wrap_err_with(|| format!("failed to inspect content root {}", root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        eyre::bail!(
            "content root must be a direct directory: {}",
            root.display()
        );
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(defaults.max_files as usize)
        .map_err(|error| eyre::eyre!("failed to reserve content file index: {error}"))?;
    let mut archive_bytes = TAR_TRAILER_BYTES;
    collect_entries_from_directory(
        root,
        Path::new(""),
        defaults,
        &mut archive_bytes,
        &mut entries,
    )?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    if entries.windows(2).any(|pair| pair[0].path == pair[1].path) {
        eyre::bail!("content root contains duplicate canonical paths");
    }
    Ok(entries)
}
fn collect_entries_from_directory(
    root: &Path,
    relative_dir: &Path,
    defaults: &actual::Content,
    archive_bytes: &mut u64,
    entries: &mut Vec<ContentSourceEntry>,
) -> Result<()> {
    let directory = root.join(relative_dir);
    for entry in fs::read_dir(&directory)
        .wrap_err_with(|| format!("failed to read content directory {}", directory.display()))?
    {
        let entry = entry?;
        let relative_path = relative_dir.join(entry.file_name());
        let relative = relative_path.to_str().ok_or_else(|| {
            eyre::eyre!(
                "content path is not valid UTF-8: {}",
                relative_path.display()
            )
        })?;
        let canonical_path = relative.replace(std::path::MAIN_SEPARATOR, "/");
        if canonical_path.len() > defaults.max_path_len as usize {
            eyre::bail!(
                "content path `{canonical_path}` exceeds the configured limit of {} bytes",
                defaults.max_path_len
            );
        }
        let file_type = entry
            .file_type()
            .wrap_err_with(|| format!("failed to inspect content path `{canonical_path}`"))?;
        if file_type.is_dir() {
            collect_entries_from_directory(root, &relative_path, defaults, archive_bytes, entries)?;
            continue;
        }
        if !file_type.is_file() {
            eyre::bail!("content path `{canonical_path}` must be a direct regular file");
        }
        if entries.len() >= defaults.max_files as usize {
            eyre::bail!(
                "content root exceeds the configured limit of {} files",
                defaults.max_files
            );
        }
        let path = entry.path();
        let (file, source_bytes) =
            open_content_file_bounded(&path, defaults.max_bundle_bytes, "content source")?;
        let entry_bytes = tar_entry_encoded_len(source_bytes as u64)?;
        let next_archive_bytes = archive_bytes
            .checked_add(entry_bytes)
            .ok_or_else(|| eyre::eyre!("content archive length overflow for `{canonical_path}`"))?;
        if next_archive_bytes > defaults.max_bundle_bytes {
            eyre::bail!(
                "content archive exceeds the configured limit of {} bytes",
                defaults.max_bundle_bytes
            );
        }
        let data = read_open_content_file_bounded(file, source_bytes, "content source", &path)?;
        *archive_bytes = next_archive_bytes;
        entries.push(ContentSourceEntry {
            path: canonical_path,
            data,
        });
    }
    Ok(())
}
fn tar_entry_encoded_len(payload_bytes: u64) -> Result<u64> {
    let padded_payload = payload_bytes
        .checked_add(TAR_BLOCK_BYTES - 1)
        .ok_or_else(|| eyre::eyre!("content payload length overflow"))?
        / TAR_BLOCK_BYTES
        * TAR_BLOCK_BYTES;
    TAR_BLOCK_BYTES
        .checked_add(padded_payload)
        .ok_or_else(|| eyre::eyre!("content tar entry length overflow"))
}
fn build_tar(entries: &[ContentSourceEntry], max_bundle_bytes: u64) -> Result<Vec<u8>> {
    let archive_bytes = entries.iter().try_fold(TAR_TRAILER_BYTES, |total, entry| {
        total
            .checked_add(tar_entry_encoded_len(entry.data.len() as u64)?)
            .ok_or_else(|| eyre::eyre!("content archive length overflow"))
    })?;
    if archive_bytes > max_bundle_bytes {
        eyre::bail!("content archive exceeds the configured limit of {max_bundle_bytes} bytes");
    }
    let archive_bytes = usize::try_from(archive_bytes)
        .map_err(|_| eyre::eyre!("content archive length cannot be represented on this host"))?;
    let mut out = Vec::new();
    out.try_reserve_exact(archive_bytes)
        .map_err(|error| eyre::eyre!("failed to reserve content archive storage: {error}"))?;
    for entry in entries {
        let path = &entry.path;
        let data = &entry.data;
        if path.len() > defaults::content::MAX_PATH_LEN as usize {
            eyre::bail!(
                "path `{path}` exceeds max length {}",
                defaults::content::MAX_PATH_LEN
            );
        }
        let (name, prefix) = split_tar_path(path)?;
        let mut header = [0u8; TAR_BLOCK_BYTES as usize];
        header[..name.len()].copy_from_slice(name.as_bytes());
        let size_str = format!("{:0>11o}\0", data.len());
        header[124..124 + size_str.len()].copy_from_slice(size_str.as_bytes());
        header[156] = b'0';
        if !prefix.is_empty() {
            header[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
        }
        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let pad = (TAR_BLOCK_BYTES as usize - (data.len() % TAR_BLOCK_BYTES as usize))
            % TAR_BLOCK_BYTES as usize;
        out.resize(out.len() + pad, 0);
    }
    out.resize(out.len() + TAR_TRAILER_BYTES as usize, 0);
    debug_assert_eq!(out.len(), archive_bytes);
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tar_builder_accepts_exact_bundle_limit_and_rejects_next_block() {
        let max_bundle_bytes = defaults::content::MAX_BUNDLE_BYTES;
        let exact_payload_bytes = max_bundle_bytes - TAR_BLOCK_BYTES - TAR_TRAILER_BYTES;
        let exact = vec![ContentSourceEntry {
            path: "index.bin".to_string(),
            data: vec![0_u8; exact_payload_bytes as usize],
        }];
        let tarball = build_tar(&exact, max_bundle_bytes).expect("exact archive must fit");
        assert_eq!(tarball.len() as u64, max_bundle_bytes);
        let oversized = vec![ContentSourceEntry {
            path: "index.bin".to_string(),
            data: vec![0_u8; exact_payload_bytes as usize + 1],
        }];
        let error = build_tar(&oversized, max_bundle_bytes)
            .expect_err("one payload byte must require another tar block");
        assert!(error.to_string().contains("configured limit"));
    }
    #[test]
    fn bounded_bundle_reader_rejects_sparse_max_plus_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let exact_path = directory.path().join("exact.tar");
        let exact = fs::File::create(&exact_path).expect("create exact bundle");
        exact
            .set_len(defaults::content::MAX_BUNDLE_BYTES)
            .expect("size exact bundle");
        drop(exact);
        let bytes = read_content_file_bounded(
            &exact_path,
            defaults::content::MAX_BUNDLE_BYTES,
            "test bundle",
        )
        .expect("exact bundle must be accepted");
        assert_eq!(bytes.len() as u64, defaults::content::MAX_BUNDLE_BYTES);
        let oversized_path = directory.path().join("oversized.tar");
        let oversized = fs::File::create(&oversized_path).expect("create oversized bundle");
        oversized
            .set_len(defaults::content::MAX_BUNDLE_BYTES + 1)
            .expect("size oversized bundle");
        drop(oversized);
        let error = read_content_file_bounded(
            &oversized_path,
            defaults::content::MAX_BUNDLE_BYTES,
            "test bundle",
        )
        .expect_err("max plus one must fail before allocation");
        assert!(error.to_string().contains("first-release limit"));
    }
    #[test]
    fn directory_collection_rejects_file_count_overflow() {
        let directory = tempfile::tempdir().expect("temporary directory");
        for index in 0..=defaults::content::MAX_FILES {
            fs::write(directory.path().join(format!("{index:03}.txt")), [])
                .expect("create content source");
        }
        let error = collect_entries(directory.path(), &default_content_config())
            .expect_err("max files plus one must fail");
        assert!(error.to_string().contains("configured limit"));
    }
    #[cfg(unix)]
    #[test]
    fn bounded_bundle_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("target.tar");
        fs::write(&target, []).expect("write target");
        let link = directory.path().join("link.tar");
        symlink(&target, &link).expect("create symlink");
        let error =
            read_content_file_bounded(&link, defaults::content::MAX_BUNDLE_BYTES, "test bundle")
                .expect_err("symlink must fail closed");
        assert!(error.to_string().contains("direct regular file"));
    }
}
