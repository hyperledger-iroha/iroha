//! Deterministic V1 `CancelAssetLock` fixture generation and verification.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use norito::{codec::encode_with_header_flags, json};

use crate::{escrow::EscrowId, isi::escrow::CancelAssetLock};

const ESCROW_ID_PREIMAGE: &str = "sorafs-appeal-cancel-asset-lock-v1";
const MAX_FIXTURE_COUNT: usize = 16;
const MAX_FIXTURE_PATH_BYTES: usize = 240;
const MAX_FIXTURE_BYTES: u64 = 1 << 20;
const MAX_OUTPUT_PATH_BYTES: usize = 4 << 10;
const MAX_OUTPUT_PATH_COMPONENTS: usize = 64;
const MAX_TEMP_ATTEMPTS: u64 = 32;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(norito::derive::NoritoSerialize)]
struct LegacyCancelAssetLock {
    escrow_id: EscrowId,
}

/// Return the repository's checked-in appeal-finance fixture directory.
#[must_use]
pub fn default_output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("iroha_data_model must remain two levels below the repository root")
        .join("fixtures/sorafs_manifest/appeal_finance")
}

/// Render every canonical positive and negative fixture without writing files.
pub fn render_fixtures() -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let escrow_id = EscrowId::new(Hash::new(ESCROW_ID_PREIMAGE));
    let valid = CancelAssetLock::new(escrow_id, Quantity::from(20_u64));
    let zero = CancelAssetLock::new(escrow_id, Quantity::zero());

    let valid_json = json::to_value(&valid)?;
    let zero_json = json::to_value(&zero)?;

    let mut missing_expected = valid_json.clone();
    object_mut(&mut missing_expected)?
        .remove("expected_remaining_amount")
        .ok_or("canonical CancelAssetLock JSON lacks expected_remaining_amount")?;

    let mut noncanonical_quantity = valid_json.clone();
    object_mut(&mut noncanonical_quantity)?.insert(
        "expected_remaining_amount".to_owned(),
        json::Value::String("20.0".to_owned()),
    );

    let valid_norito = norito::to_bytes(&valid)?;
    let zero_norito = norito::to_bytes(&zero)?;
    let (legacy_payload, flags) = encode_with_header_flags(&LegacyCancelAssetLock { escrow_id });
    let legacy_norito =
        norito::core::frame_bare_with_header_flags::<CancelAssetLock>(&legacy_payload, flags)?;
    let mut trailing_norito = valid_norito.clone();
    trailing_norito.push(0);

    Ok(BTreeMap::from([
        (
            PathBuf::from("cancel_asset_lock_v1.json"),
            pretty_json_bytes(&valid_json)?,
        ),
        (PathBuf::from("cancel_asset_lock_v1.to"), valid_norito),
        (
            PathBuf::from("negative/cancel_asset_lock_legacy_missing_expected_v1.json"),
            pretty_json_bytes(&missing_expected)?,
        ),
        (
            PathBuf::from("negative/cancel_asset_lock_legacy_missing_expected_v1.to"),
            legacy_norito,
        ),
        (
            PathBuf::from("negative/cancel_asset_lock_noncanonical_quantity_v1.json"),
            pretty_json_bytes(&noncanonical_quantity)?,
        ),
        (
            PathBuf::from("negative/cancel_asset_lock_trailing_bytes_v1.to"),
            trailing_norito,
        ),
        (
            PathBuf::from("negative/cancel_asset_lock_zero_expected_v1.json"),
            pretty_json_bytes(&zero_json)?,
        ),
        (
            PathBuf::from("negative/cancel_asset_lock_zero_expected_v1.to"),
            zero_norito,
        ),
    ]))
}

/// Write the exact fixture map beneath `output_dir`.
pub fn write_fixtures(
    output_dir: &Path,
    fixtures: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    validate_fixture_map(fixtures)?;
    ensure_real_directory(output_dir)?;
    let expected_paths = fixtures.keys().cloned().collect::<BTreeSet<_>>();
    let existing_paths = scan_fixture_paths(output_dir, &expected_paths)?;
    if !existing_paths.is_subset(&expected_paths) {
        return Err("CancelAssetLock fixture directory contains unexpected entries".into());
    }
    for (relative, bytes) in fixtures {
        let path = output_dir.join(relative);
        let parent = path
            .parent()
            .ok_or("CancelAssetLock fixture target must have a parent")?;
        ensure_real_directory(parent)?;
        atomic_write_regular_file(&path, bytes)?;
    }
    let actual_paths = scan_fixture_paths(output_dir, &expected_paths)?;
    if actual_paths != expected_paths {
        return Err(
            "CancelAssetLock fixture publication did not produce the exact path set".into(),
        );
    }
    Ok(())
}

/// Verify exact paths and bytes beneath `output_dir`.
pub fn check_fixtures(
    output_dir: &Path,
    fixtures: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    validate_fixture_map(fixtures)?;
    ensure_existing_real_directory(output_dir)?;
    let expected_paths = fixtures.keys().cloned().collect::<BTreeSet<_>>();
    let actual_paths = scan_fixture_paths(output_dir, &expected_paths)?;
    if actual_paths != expected_paths {
        return Err(format!(
            "CancelAssetLock fixture paths differ (expected={expected_paths:?}, actual={actual_paths:?})"
        )
        .into());
    }
    for (relative, expected) in fixtures {
        let actual = read_regular_file(&output_dir.join(relative))?;
        if actual != *expected {
            return Err(format!(
                "CancelAssetLock fixture `{}` differs from deterministic generation",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn validate_fixture_map(fixtures: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), Box<dyn Error>> {
    if fixtures.is_empty() {
        return Err("CancelAssetLock fixture map must not be empty".into());
    }
    if fixtures.len() > MAX_FIXTURE_COUNT {
        return Err("CancelAssetLock fixture map exceeds the entry bound".into());
    }
    for (relative, bytes) in fixtures {
        validate_relative_fixture_path(relative)?;
        if u64::try_from(bytes.len())? > MAX_FIXTURE_BYTES {
            return Err(format!(
                "CancelAssetLock fixture `{}` exceeds the byte bound",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn validate_relative_fixture_path(path: &Path) -> Result<(), Box<dyn Error>> {
    use std::path::Component;

    if path.as_os_str().len() > MAX_FIXTURE_PATH_BYTES || path.is_absolute() {
        return Err(format!(
            "CancelAssetLock fixture path `{}` must be bounded and relative",
            path.display()
        )
        .into());
    }
    let mut components = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => components += 1,
            _ => {
                return Err(format!(
                    "CancelAssetLock fixture path `{}` contains traversal or a prefix",
                    path.display()
                )
                .into());
            }
        }
    }
    if components == 0
        || components > 3
        || !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("json" | "to")
        )
    {
        return Err(format!(
            "CancelAssetLock fixture path `{}` is outside the closed layout",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn object_mut(value: &mut json::Value) -> Result<&mut json::Map, Box<dyn Error>> {
    match value {
        json::Value::Object(map) => Ok(map),
        _ => Err("CancelAssetLock JSON must be an object".into()),
    }
}

fn pretty_json_bytes(value: &json::Value) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(format!("{}\n", json::to_json_pretty(value)?).into_bytes())
}

fn scan_fixture_paths(
    root: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<BTreeSet<PathBuf>, Box<dyn Error>> {
    fn visit(
        root: &Path,
        directory: &Path,
        expected_paths: &BTreeSet<PathBuf>,
        expected_directories: &BTreeSet<PathBuf>,
        paths: &mut BTreeSet<PathBuf>,
    ) -> Result<(), Box<dyn Error>> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "CancelAssetLock fixture entry `{}` must not be a symlink",
                    relative.display()
                )
                .into());
            }
            if metadata.is_dir() {
                if !expected_directories.contains(&relative) {
                    return Err(format!(
                        "CancelAssetLock fixture directory contains unexpected entry `{}`",
                        relative.display()
                    )
                    .into());
                }
                visit(root, &path, expected_paths, expected_directories, paths)?;
            } else if metadata.is_file() {
                ensure_single_hard_link(&metadata, &path)?;
                if relative == Path::new("README.md") {
                    continue;
                }
                if !expected_paths.contains(&relative) {
                    return Err(format!(
                        "CancelAssetLock fixture directory contains unexpected entry `{}`",
                        relative.display()
                    )
                    .into());
                }
                paths.insert(relative);
            } else {
                return Err(format!(
                    "CancelAssetLock fixture entry `{}` must be a regular file or directory",
                    relative.display()
                )
                .into());
            }
        }
        Ok(())
    }

    let mut expected_directories = BTreeSet::new();
    for path in expected_paths {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            expected_directories.insert(parent.to_path_buf());
        }
    }
    let mut paths = BTreeSet::new();
    visit(
        root,
        root,
        expected_paths,
        &expected_directories,
        &mut paths,
    )?;
    Ok(paths)
}

fn ensure_real_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    validate_directory_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => ensure_existing_real_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or("CancelAssetLock fixture directory must have a parent")?;
            ensure_real_directory(parent)?;
            match fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            let metadata = fs::symlink_metadata(path)?;
            ensure_directory_metadata(&metadata, path)
        }
        Err(error) => Err(error.into()),
    }
}

fn ensure_existing_real_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    validate_directory_path(path)?;
    if let Some(parent) = path.parent()
        && parent != path
        && !parent.as_os_str().is_empty()
    {
        ensure_existing_real_directory(parent)?;
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure_directory_metadata(&metadata, path)
}

fn validate_directory_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.as_os_str().len() > MAX_OUTPUT_PATH_BYTES {
        return Err("CancelAssetLock fixture output path exceeds the byte bound".into());
    }
    let mut count = 0usize;
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(format!(
                "CancelAssetLock fixture output `{}` contains parent traversal",
                path.display()
            )
            .into());
        }
        count += 1;
    }
    if count > MAX_OUTPUT_PATH_COMPONENTS {
        return Err("CancelAssetLock fixture output path exceeds the component bound".into());
    }
    Ok(())
}

fn ensure_directory_metadata(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "CancelAssetLock fixture directory `{}` must be a real directory",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn atomic_write_regular_file(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .ok_or("CancelAssetLock fixture target must have a parent")?;
    let parent_before = fs::symlink_metadata(parent)?;
    ensure_directory_metadata(&parent_before, parent)?;
    validate_existing_target(path)?;

    let (mut temporary, temporary_path) = create_temporary_file(parent)?;
    let mut cleanup = TemporaryFileGuard::new(temporary_path.clone());
    temporary.write_all(bytes)?;
    temporary.sync_all()?;
    let temporary_metadata = temporary.metadata()?;
    if !temporary_metadata.is_file() {
        return Err("CancelAssetLock fixture temporary target must be a regular file".into());
    }
    ensure_single_hard_link(&temporary_metadata, &temporary_path)?;
    drop(temporary);

    let parent_current = fs::symlink_metadata(parent)?;
    ensure_same_directory(&parent_before, &parent_current, parent)?;
    validate_existing_target(path)?;
    fs::rename(&temporary_path, path)?;
    cleanup.disarm();

    let published = fs::symlink_metadata(path)?;
    if published.file_type().is_symlink() || !published.is_file() {
        return Err(format!(
            "published CancelAssetLock fixture `{}` must be a regular non-symlink file",
            path.display()
        )
        .into());
    }
    ensure_single_hard_link(&published, path)?;
    sync_directory(parent)?;
    Ok(())
}

fn validate_existing_target(path: &Path) -> Result<(), Box<dyn Error>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "CancelAssetLock fixture target `{}` must be a regular non-symlink file",
                    path.display()
                )
                .into());
            }
            ensure_single_hard_link(&metadata, path)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "CancelAssetLock fixture `{}` must be a regular non-symlink file",
            path.display()
        )
        .into());
    }
    ensure_single_hard_link(&before, path)?;
    if before.len() > MAX_FIXTURE_BYTES {
        return Err(format!(
            "CancelAssetLock fixture `{}` exceeds the byte bound",
            path.display()
        )
        .into());
    }

    let file = File::open(path)?;
    let opened = file.metadata()?;
    ensure_same_file(&before, &opened, path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len())?);
    file.take(MAX_FIXTURE_BYTES + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > MAX_FIXTURE_BYTES {
        return Err(format!(
            "CancelAssetLock fixture `{}` exceeds the byte bound",
            path.display()
        )
        .into());
    }

    let after = fs::symlink_metadata(path)?;
    if after.file_type().is_symlink() || !after.is_file() {
        return Err(format!(
            "CancelAssetLock fixture `{}` changed type while reading",
            path.display()
        )
        .into());
    }
    ensure_single_hard_link(&after, path)?;
    ensure_same_file(&before, &after, path)?;
    Ok(bytes)
}

fn create_temporary_file(parent: &Path) -> Result<(File, PathBuf), Box<dyn Error>> {
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".cancel_asset_lock_fixtures.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("could not allocate a unique CancelAssetLock fixture temporary file".into())
}

struct TemporaryFileGuard {
    path: Option<PathBuf>,
}

impl TemporaryFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(unix)]
fn ensure_single_hard_link(metadata: &fs::Metadata, path: &Path) -> Result<(), Box<dyn Error>> {
    if metadata.nlink() != 1 {
        return Err(format!(
            "CancelAssetLock fixture `{}` must have exactly one hard link",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_single_hard_link(_metadata: &fs::Metadata, _path: &Path) -> Result<(), Box<dyn Error>> {
    Ok(())
}

#[cfg(unix)]
fn ensure_same_file(
    before: &fs::Metadata,
    after: &fs::Metadata,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len() {
        return Err(format!(
            "CancelAssetLock fixture `{}` changed while reading",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_same_file(
    before: &fs::Metadata,
    after: &fs::Metadata,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(format!(
            "CancelAssetLock fixture `{}` changed while reading",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_same_directory(
    before: &fs::Metadata,
    after: &fs::Metadata,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    ensure_directory_metadata(after, path)?;
    if before.dev() != after.dev() || before.ino() != after.ino() {
        return Err(format!(
            "CancelAssetLock fixture directory `{}` changed during publication",
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_same_directory(
    _before: &fs::Metadata,
    after: &fs::Metadata,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    ensure_directory_metadata(after, path)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), Box<dyn Error>> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_cancel_asset_lock_fixtures_match_generation() {
        check_fixtures(
            &default_output_dir(),
            &render_fixtures().expect("render fixture bytes"),
        )
        .expect("checked-in fixtures must be byte-identical");
    }

    #[test]
    fn fixture_publication_is_atomic_and_path_closed() {
        let temp = tempfile::tempdir().expect("create fixture tempdir");
        let root = temp
            .path()
            .canonicalize()
            .expect("canonicalize fixture tempdir")
            .join("appeal_finance");
        let fixtures = render_fixtures().expect("render fixture bytes");

        write_fixtures(&root, &fixtures).expect("publish fixture set");
        check_fixtures(&root, &fixtures).expect("verify published fixture set");
        assert!(
            fs::read_dir(&root)
                .expect("scan fixture root")
                .all(|entry| !entry
                    .expect("read fixture entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".tmp")),
            "atomic publication must not leave temporary files"
        );

        fs::write(root.join("unreviewed.to"), b"NRT0").expect("write unexpected fixture control");
        let error = write_fixtures(&root, &fixtures)
            .expect_err("unexpected fixture entries must fail closed");
        assert!(error.to_string().contains("unexpected entry"));
    }

    #[test]
    fn fixture_map_rejects_parent_traversal() {
        let temp = tempfile::tempdir().expect("create fixture tempdir");
        let root = temp
            .path()
            .canonicalize()
            .expect("canonicalize fixture tempdir");
        let fixtures = BTreeMap::from([(PathBuf::from("../escape.to"), b"NRT0".to_vec())]);

        let error = write_fixtures(&root, &fixtures)
            .expect_err("parent-traversal fixture paths must fail closed");
        assert!(error.to_string().contains("traversal"));
        assert!(
            !root
                .parent()
                .expect("tempdir parent")
                .join("escape.to")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn fixture_publication_rejects_symlink_target_and_parent() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("create fixture tempdir");
        let canonical_temp = temp
            .path()
            .canonicalize()
            .expect("canonicalize fixture tempdir");
        let root = canonical_temp.join("appeal_finance");
        let outside = canonical_temp.join("outside");
        fs::create_dir(&root).expect("create fixture root");
        fs::create_dir(&outside).expect("create outside directory");
        let outside_file = outside.join("outside.to");
        fs::write(&outside_file, b"outside").expect("write outside fixture");
        symlink(&outside_file, root.join("cancel_asset_lock_v1.to"))
            .expect("create target symlink");

        let fixtures = render_fixtures().expect("render fixture bytes");
        let error = write_fixtures(&root, &fixtures).expect_err("target symlink must fail closed");
        assert!(error.to_string().contains("symlink"));
        assert_eq!(
            fs::read(&outside_file).expect("read outside fixture"),
            b"outside"
        );

        fs::remove_file(root.join("cancel_asset_lock_v1.to"))
            .expect("remove target symlink control");
        symlink(&outside, root.join("negative")).expect("create parent symlink");
        let error = write_fixtures(&root, &fixtures).expect_err("parent symlink must fail closed");
        assert!(error.to_string().contains("symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn fixture_publication_and_check_reject_hardlinked_targets() {
        let temp = tempfile::tempdir().expect("create fixture tempdir");
        let root = temp
            .path()
            .canonicalize()
            .expect("canonicalize fixture tempdir")
            .join("appeal_finance");
        let fixtures = render_fixtures().expect("render fixture bytes");
        write_fixtures(&root, &fixtures).expect("publish fixture set");

        let target = root.join("cancel_asset_lock_v1.to");
        let alias = root
            .parent()
            .expect("fixture root parent")
            .join("cancel_asset_lock_alias.to");
        fs::hard_link(&target, &alias).expect("create hardlink control");

        let error =
            check_fixtures(&root, &fixtures).expect_err("hardlinked fixture must fail check");
        assert!(error.to_string().contains("exactly one hard link"));
        let error =
            write_fixtures(&root, &fixtures).expect_err("hardlinked fixture must fail write");
        assert!(error.to_string().contains("exactly one hard link"));
    }
}
