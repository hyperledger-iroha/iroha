//! Build and inspect a complete native developer runtime bundle.

use crate::workspace_root;
use iroha_deploy::managed::{NativeBundleLayout, macos_info_plist};
use norito::json::{self, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;
mod developer_smoke;
mod network_profiles;
#[cfg(test)]
use iroha_deploy::bootstrap::InstalledNetworkProfiles;
pub(crate) mod latency;
const MOCHI_UI_MANIFEST_REL: &str = "mochi/mochi-ui-egui/Cargo.toml";
const MOCHI_BIN_NAME: &str = "mochi";
const MOCHI_HELP_HEADER: &str = "Usage: mochi [--workspace <DIRECTORY>]";
const RUNTIME_BINARIES: [&str; 3] = ["mochi", "kagami", "iroha3d"];
#[derive(Debug, Clone)]
pub(crate) struct MochiBundleResult {
    pub target: String,
    pub profile: String,
    pub bundle_name: String,
    pub output_root: PathBuf,
    pub bundle_root: PathBuf,
    pub manifest_path: PathBuf,
    pub archive_path: Option<PathBuf>,
    archive_sha256: Option<String>,
    network_profiles: Option<network_profiles::Selection>,
}
pub(crate) fn bundle_mochi(
    output_root: &Path,
    profile: &str,
    archive: bool,
    network_profiles: Option<&Path>,
) -> Result<MochiBundleResult, Box<dyn Error>> {
    validate_bundle_profile(profile)?;
    let network_profiles = network_profiles::select(&workspace_root(), profile, network_profiles)?;
    build_runtime(profile)?;
    if !output_root.exists() {
        fs::create_dir_all(output_root)?;
    }
    let host = format!("{}-{}", env::consts::OS, env::consts::ARCH);
    let bundle_name = format!("mochi-{host}-{profile}");
    let bundle_root = output_root.join(&bundle_name);
    if bundle_root.exists() {
        fs::remove_dir_all(&bundle_root)?;
    }
    fs::create_dir_all(NativeBundleLayout::current().runtime_directory(&bundle_root))?;
    fs::create_dir_all(bundle_root.join("docs"))?;
    copy_runtime_binaries(
        &cargo_target_dir().join(profile_directory(profile)),
        &bundle_root,
    )?;
    stage_application_metadata(&bundle_root)?;
    if let Some(profiles) = &network_profiles {
        profiles.stage(&bundle_root)?;
    }
    copy_into_bundle("LICENSE", &bundle_root.join("LICENSE"))?;
    copy_into_bundle(
        "mochi/BUNDLE_README.md",
        &bundle_root.join("docs/README.md"),
    )?;
    let manifest = generate_manifest_json(&bundle_root, profile)?;
    let manifest_path = bundle_root.join("manifest.json");
    let mut manifest_text = json::to_string_pretty(&manifest)?;
    manifest_text.push('\n');
    fs::write(&manifest_path, manifest_text)?;
    let archive_path = if archive {
        Some(create_archive(output_root, &bundle_name, &bundle_root)?)
    } else {
        None
    };
    let archive_sha256 = archive_path.as_deref().map(archive_digest).transpose()?;
    Ok(MochiBundleResult {
        target: host,
        profile: profile.to_owned(),
        bundle_name,
        output_root: output_root.to_path_buf(),
        bundle_root,
        manifest_path,
        archive_path,
        archive_sha256,
        network_profiles,
    })
}

/// Validate the exact release/development profile input before packaging effects.
pub(crate) fn validate_network_profile_input(
    profile: &str,
    supplied: Option<&Path>,
) -> Result<(), &'static str> {
    network_profiles::validate_input(profile, supplied)
}

/// Refuse the workspace's explicitly non-packaging profile before doing any work.
pub(crate) fn validate_bundle_profile(profile: &str) -> Result<(), &'static str> {
    if profile == "local-release" {
        return Err(
            "local-release is only for local runnable builds; it cannot package or qualify a developer runtime",
        );
    }
    Ok(())
}

#[cfg(test)]
fn load_network_profiles(
    source: Option<&Path>,
) -> Result<Option<network_profiles::Selection>, Box<dyn Error>> {
    network_profiles::select(&workspace_root(), "debug", source)
}

fn stage_application_metadata(bundle_root: &Path) -> Result<(), Box<dyn Error>> {
    if NativeBundleLayout::current() == NativeBundleLayout::MacOs {
        let source = fs::read_to_string(mochi_ui_manifest_path())?;
        let manifest: toml::Value = toml::from_str(&source)?;
        let version = manifest
            .get("package")
            .and_then(|package| package.get("version"))
            .and_then(toml::Value::as_str)
            .ok_or("Mochi package has no explicit release version")?;
        fs::create_dir_all(NativeBundleLayout::MacOs.resources_directory(bundle_root))?;
        fs::write(
            bundle_root.join("Mochi.app/Contents/Info.plist"),
            macos_info_plist(version)?,
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn stage_network_profiles(
    profiles: Option<&InstalledNetworkProfiles>,
    bundle_root: &Path,
) -> Result<(), Box<dyn Error>> {
    if let Some(profiles) = profiles {
        network_profiles::development(profiles)?.stage(bundle_root)?;
    }
    Ok(())
}

pub(crate) fn run_bundle_smoke(result: &MochiBundleResult) -> Result<(), Box<dyn Error>> {
    verify_bundle_profiles(result)?;
    if let Some(profiles) = &result.network_profiles {
        let kagami = NativeBundleLayout::current().executable(&result.bundle_root, "kagami");
        let mut probe = developer_smoke::Harness::new(&kagami)?;
        profiles.require_cli_names(&json::to_vec(&probe.network_names()?)?)?;
        verify_bundle_profiles(result)?;
    }
    let mochi_bin = NativeBundleLayout::current().executable(&result.bundle_root, "mochi");
    if !mochi_bin.exists() {
        return Err(format!("missing mochi binary at {}", mochi_bin.display()).into());
    }
    let output = Command::new(&mochi_bin).arg("--help").output()?;
    if !output.status.success() {
        Err(format!(
            "`{}` --help exited with status {:?}: {}",
            mochi_bin.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Err(reason) = validate_mochi_help_output(&stdout) {
            return Err(format!(
                "`{}` --help did not expose the packaged Mochi CLI: {reason}; stdout: {}",
                mochi_bin.display(),
                stdout.trim()
            )
            .into());
        }
        developer_smoke::run(
            &NativeBundleLayout::current().executable(&result.bundle_root, "kagami"),
        )
    }
}
fn verify_bundle_profiles(result: &MochiBundleResult) -> Result<(), Box<dyn Error>> {
    match (&result.archive_path, &result.archive_sha256) {
        (Some(path), Some(expected)) if archive_digest(path)? == *expected => {}
        (None, None) => {}
        _ => return Err("bundle archive differs from original packaging selection".into()),
    }
    match &result.network_profiles {
        Some(profiles) => profiles.verify_installed(&result.bundle_root),
        None if result.profile == "release" => {
            Err("release bundle has no retained Taira profile selection".into())
        }
        None => match fs::symlink_metadata(
            NativeBundleLayout::current().profiles_path(&result.bundle_root),
        ) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err("unexpected network profile in profile-free development bundle".into()),
        },
    }
}

fn validate_mochi_help_output(stdout: &str) -> Result<(), String> {
    if !stdout.contains(MOCHI_HELP_HEADER) {
        return Err(format!("missing `{MOCHI_HELP_HEADER}`"));
    }
    Ok(())
}
pub(crate) fn update_bundle_matrix(
    result: &MochiBundleResult,
    matrix_path: &Path,
    smoke_passed: bool,
) -> Result<(), Box<dyn Error>> {
    verify_bundle_profiles(result)?;
    if let Some(parent) = matrix_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let timestamp = u64::try_from(timestamp_ms).unwrap_or(u64::MAX);
    let mut root_map = if matrix_path.exists() {
        let raw = fs::read_to_string(matrix_path)?;
        let value: Value = json::from_str(&raw)?;
        value.as_object().cloned().unwrap_or_else(Map::new)
    } else {
        Map::new()
    };
    if !root_map.contains_key("generated_unix_ms") {
        root_map.insert("generated_unix_ms".into(), Value::from(timestamp));
    }
    root_map.insert("updated_unix_ms".into(), Value::from(timestamp));
    let mut entries = match root_map.remove("entries") {
        Some(Value::Array(entries)) => entries,
        Some(other) => {
            return Err(format!(
                "expected `entries` to be an array in {} but found {other:?}",
                matrix_path.display()
            )
            .into());
        }
        None => Vec::new(),
    };
    let target_value = Value::from(result.target.clone());
    let profile_value = Value::from(result.profile.clone());
    entries.retain(|entry| {
        entry
            .as_object()
            .map(|object| {
                let target_matches = object.get("target") == Some(&target_value);
                let profile_matches = object.get("profile") == Some(&profile_value);
                !(target_matches && profile_matches)
            })
            .unwrap_or(true)
    });
    let manifest_bytes = fs::read(&result.manifest_path)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let mut entry = Map::new();
    entry.insert("target".into(), Value::from(result.target.clone()));
    entry.insert("profile".into(), Value::from(result.profile.clone()));
    entry.insert("bundle".into(), Value::from(result.bundle_name.clone()));
    entry.insert("bundle_dir".into(), Value::from(result.bundle_name.clone()));
    entry.insert(
        "manifest".into(),
        Value::from(format!("{}/manifest.json", result.bundle_name)),
    );
    entry.insert("manifest_sha256".into(), Value::from(manifest_sha256));
    if let Some(archive) = &result.archive_path {
        if let Ok(relative) = archive.strip_prefix(&result.output_root) {
            entry.insert(
                "archive".into(),
                Value::from(relative.to_string_lossy().into_owned()),
            );
        } else if let Some(file_name) = archive.file_name() {
            entry.insert(
                "archive".into(),
                Value::from(file_name.to_string_lossy().into_owned()),
            );
        }
    }
    if let Some(digest) = &result.archive_sha256 {
        entry.insert("archive_sha256".into(), Value::from(digest.clone()));
    }
    entry.insert("generated_unix_ms".into(), Value::from(timestamp));
    entry.insert("smoke_passed".into(), Value::from(smoke_passed));
    entry.insert(
        "network_profiles".into(),
        result
            .network_profiles
            .as_ref()
            .map_or(Value::Null, network_profiles::Selection::provenance),
    );
    entry.insert(
        "qualification".into(),
        Value::from("local_native_diagnostic"),
    );
    entry.insert(
        "official_taira_attachment_qualified".into(),
        Value::from(false),
    );
    entries.push(Value::Object(entry));
    root_map.insert("entries".into(), Value::Array(entries));
    let mut text = json::to_string_pretty(&Value::Object(root_map))?;
    text.push('\n');
    fs::write(matrix_path, text)?;
    Ok(())
}
pub(crate) fn stage_bundle(
    result: &MochiBundleResult,
    stage_root: &Path,
) -> Result<(), Box<dyn Error>> {
    verify_bundle_profiles(result)?;
    fs::create_dir_all(stage_root)?;
    let staged_bundle_root = stage_root.join(&result.bundle_name);
    if staged_bundle_root.starts_with(&result.bundle_root)
        && staged_bundle_root != result.bundle_root
    {
        return Err(format!(
            "staging directory {} must not be inside the bundle root {}",
            staged_bundle_root.display(),
            result.bundle_root.display()
        )
        .into());
    }
    if staged_bundle_root != result.bundle_root {
        if staged_bundle_root.exists() {
            fs::remove_dir_all(&staged_bundle_root)?;
        }
        copy_directory(&result.bundle_root, &staged_bundle_root)?;
    }
    if let Some(archive) = &result.archive_path {
        if let Some(file_name) = archive.file_name() {
            let staged_archive = stage_root.join(file_name);
            if staged_archive != *archive {
                if staged_archive.exists() {
                    fs::remove_file(&staged_archive)?;
                }
                fs::copy(archive, staged_archive)?;
            }
        } else {
            return Err(format!(
                "archive {} is missing a file name segment",
                archive.display()
            )
            .into());
        }
    }
    verify_bundle_profiles(result)?;
    if let Some(profiles) = &result.network_profiles {
        profiles.verify_installed(&staged_bundle_root)?;
    }
    if let (Some(original), Some(expected)) = (&result.archive_path, &result.archive_sha256) {
        let staged = stage_root.join(original.file_name().ok_or("archive filename absent")?);
        if archive_digest(&staged)? != *expected {
            return Err("staged archive differs from original packaging selection".into());
        }
    }
    Ok(())
}
// Stream the existing bounded artifact hash owner; never allocate an entire release archive.
fn archive_digest(path: &Path) -> Result<String, Box<dyn Error>> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err("bundle archive must be a direct regular file".into());
    }
    let (digest, _) =
        iroha_crypto::sha256_reader_bounded(fs::File::open(path)?, 32 * 1024 * 1024 * 1024)?;
    Ok(hex::encode(digest))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    for entry in WalkDir::new(source).follow_root_links(false) {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_dir() && !entry.file_type().is_file() {
            return Err("bundle staging requires only direct files and directories".into());
        }
        let relative = path.strip_prefix(source)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}
fn build_runtime(profile: &str) -> Result<(), Box<dyn Error>> {
    validate_bundle_profile(profile)?;
    let mut command = Command::new("cargo");
    command.args(runtime_build_args(profile));
    command.current_dir(workspace_root());
    let status = command.status()?;
    if !status.success() {
        return Err("building the complete Mochi/Kagami/iroha3d runtime failed".into());
    }
    Ok(())
}
fn runtime_build_args(profile: &str) -> Vec<OsString> {
    let mut args = vec![OsString::from("build")];
    if profile == "release" {
        args.push(OsString::from("--release"));
    } else if profile != "debug" {
        args.extend([OsString::from("--profile"), OsString::from(profile)]);
    }
    args.extend([
        OsString::from("--locked"),
        OsString::from("-p"),
        OsString::from("mochi-ui"),
        OsString::from("-p"),
        OsString::from("iroha_kagami"),
        OsString::from("-p"),
        OsString::from("irohad"),
        OsString::from("--features"),
        OsString::from("mochi-ui/gui"),
        OsString::from("--bin"),
        OsString::from(MOCHI_BIN_NAME),
        OsString::from("--bin"),
        OsString::from("kagami"),
        OsString::from("--bin"),
        OsString::from("iroha3d"),
    ]);
    args
}
fn mochi_ui_manifest_path() -> PathBuf {
    workspace_root().join(MOCHI_UI_MANIFEST_REL)
}
fn profile_directory(profile: &str) -> &str {
    match profile {
        "release" => "release",
        "debug" => "debug",
        other => other,
    }
}
fn copy_runtime_binaries(source: &Path, bundle_root: &Path) -> Result<(), Box<dyn Error>> {
    // All executables must come from the preceding single Cargo invocation.
    // An arbitrary binary override can silently combine different protocols.
    for name in RUNTIME_BINARIES {
        let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
        let binary = source.join(&filename);
        let metadata = fs::symlink_metadata(&binary)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "runtime binary {} is not a direct regular file",
                binary.display()
            )
            .into());
        }
    }
    fs::create_dir_all(NativeBundleLayout::current().runtime_directory(&bundle_root))?;
    for name in RUNTIME_BINARIES {
        let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
        fs::copy(
            source.join(&filename),
            NativeBundleLayout::current()
                .runtime_directory(bundle_root)
                .join(filename),
        )?;
    }
    Ok(())
}
fn copy_into_bundle(source_rel: &str, destination: &Path) -> Result<(), Box<dyn Error>> {
    let source = workspace_root().join(source_rel);
    let metadata = fs::symlink_metadata(&source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("bundle asset {source_rel} must be a direct regular file").into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, destination)?;
    Ok(())
}
fn cargo_target_dir() -> PathBuf {
    if let Ok(dir) = env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            path
        } else {
            workspace_root().join(path)
        }
    } else {
        workspace_root().join("target")
    }
}
fn generate_manifest_json(bundle_root: &Path, profile: &str) -> Result<Value, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(bundle_root).follow_root_links(false) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err("bundle inventory requires only direct files and directories".into());
        }
        let path = entry.path();
        let relative = path.strip_prefix(bundle_root)?;
        let data = fs::read(path)?;
        files.push((
            relative.to_string_lossy().replace('\\', "/"),
            data.len() as u64,
            sha256_hex(&data),
        ));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let files: Vec<Value> = files
        .into_iter()
        .map(|(path, size, sha256)| {
            let mut entry = norito::json::Map::new();
            entry.insert("path".into(), Value::from(path));
            entry.insert("size".into(), Value::from(size));
            entry.insert("sha256".into(), Value::from(sha256));
            Value::Object(entry)
        })
        .collect();
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let generated_unix_ms = u64::try_from(timestamp_ms).unwrap_or(u64::MAX);
    let mut manifest = norito::json::Map::new();
    manifest.insert("generated_unix_ms".into(), Value::from(generated_unix_ms));
    manifest.insert(
        "target".into(),
        Value::from(format!("{}-{}", env::consts::OS, env::consts::ARCH)),
    );
    manifest.insert("profile".into(), Value::from(profile));
    manifest.insert("files".into(), Value::Array(files));
    Ok(Value::Object(manifest))
}
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
fn create_archive(
    output_root: &Path,
    bundle_name: &str,
    bundle_root: &Path,
) -> Result<PathBuf, Box<dyn Error>> {
    let archive_path = output_root.join(format!("{bundle_name}.tar.gz"));
    if archive_path.exists() {
        fs::remove_file(&archive_path)?;
    }
    let bundle_parent = bundle_root.parent().ok_or_else(|| {
        format!(
            "bundle root `{}` does not have a parent directory",
            bundle_root.display()
        )
    })?;
    if bundle_parent != output_root {
        return Err(format!(
            "bundle root `{}` must live under output root `{}`",
            bundle_root.display(),
            output_root.display()
        )
        .into());
    }
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(output_root)
        .arg(bundle_name)
        .status()?;
    if !status.success() {
        return Err(format!(
            "`tar -czf {}` exited with status {:?}",
            archive_path.display(),
            status
        )
        .into());
    }
    Ok(archive_path)
}
#[cfg(test)]
mod tests {
    use super::{
        MOCHI_BIN_NAME, MOCHI_HELP_HEADER, MOCHI_UI_MANIFEST_REL, RUNTIME_BINARIES, copy_directory,
        copy_runtime_binaries, create_archive, generate_manifest_json, mochi_ui_manifest_path,
        runtime_build_args, sha256_hex, stage_application_metadata, stage_network_profiles,
        validate_mochi_help_output,
    };
    use iroha_deploy::{
        bootstrap::InstalledNetworkProfiles,
        managed::{NativeBundleLayout, macos_info_plist},
    };
    use std::{env, ffi::OsString, fs, path::Path, process::Command};
    use tempfile::tempdir;
    #[test]
    fn local_release_is_rejected_before_build_profile_input_or_output_mutation() {
        let root = tempdir().unwrap();
        let existing = root.path().join("kept");
        fs::write(&existing, b"existing bundle").unwrap();
        let error = super::bundle_mochi(
            root.path(),
            "local-release",
            false,
            Some(&root.path().join("missing-network-profiles.nrt")),
        )
        .unwrap_err();
        assert!(error.to_string().contains("local-release"));
        assert_eq!(fs::read(&existing).unwrap(), b"existing bundle");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert!(super::build_runtime("local-release").is_err());
        assert!(super::validate_bundle_profile("release").is_ok());
        assert!(super::validate_bundle_profile("debug").is_ok());
    }
    #[test]
    fn mochi_ui_manifest_path_declares_packaged_binary() {
        let manifest_path = mochi_ui_manifest_path();
        assert!(
            manifest_path.ends_with(MOCHI_UI_MANIFEST_REL),
            "unexpected MOCHI UI manifest path: {}",
            manifest_path.display()
        );
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|_| panic!("missing manifest {}", manifest_path.display()));
        assert!(
            manifest.contains("[[bin]]")
                && manifest.contains(&format!("name = \"{MOCHI_BIN_NAME}\"")),
            "MOCHI UI manifest must declare the packaged `{MOCHI_BIN_NAME}` binary"
        );
    }
    #[test]
    fn mochi_ui_build_args_enable_gui_for_the_packaged_binary() {
        let args = runtime_build_args("debug");
        let expected_tail = [
            OsString::from("--features"),
            OsString::from("mochi-ui/gui"),
            OsString::from("--bin"),
            OsString::from(MOCHI_BIN_NAME),
            OsString::from("--bin"),
            OsString::from("kagami"),
            OsString::from("--bin"),
            OsString::from("iroha3d"),
        ];
        assert_eq!(args.first(), Some(&OsString::from("build")));
        assert_eq!(
            args.get(args.len() - expected_tail.len()..),
            Some(expected_tail.as_slice())
        );
    }
    #[test]
    fn mochi_ui_build_args_preserve_named_profiles() {
        let args = runtime_build_args("profiling");
        assert_eq!(
            args.get(0..3),
            Some(
                [
                    OsString::from("build"),
                    OsString::from("--profile"),
                    OsString::from("profiling"),
                ]
                .as_slice()
            )
        );
    }
    #[test]
    fn mochi_ui_build_args_preserve_release_profile() {
        let args = runtime_build_args("release");
        assert_eq!(
            args.get(0..2),
            Some([OsString::from("build"), OsString::from("--release")].as_slice())
        );
    }
    #[test]
    fn runtime_build_is_locked_and_includes_every_runtime_package() {
        let args = runtime_build_args("debug");
        assert!(args.contains(&OsString::from("--locked")));
        for package in ["mochi-ui", "iroha_kagami", "irohad"] {
            assert!(
                args.windows(2)
                    .any(|pair| pair == [OsString::from("-p"), OsString::from(package)])
            );
        }
    }
    #[test]
    fn runtime_bundle_contains_daemon_without_path_dependency() {
        let root = tempdir().expect("temporary root");
        let source = root.path().join("source");
        let bundle = root.path().join("bundle");
        fs::create_dir(&source).expect("source directory");
        for name in RUNTIME_BINARIES {
            fs::write(
                source.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
                name,
            )
            .expect("fixture binary");
        }
        copy_runtime_binaries(&source, &bundle).expect("complete runtime");
        for name in RUNTIME_BINARIES {
            assert_eq!(
                fs::read(NativeBundleLayout::current().executable(&bundle, name)).unwrap(),
                name.as_bytes()
            );
        }
        if NativeBundleLayout::current() == NativeBundleLayout::MacOs {
            assert!(!bundle.join("bin").exists());
        }
    }

    #[test]
    fn app_metadata_uses_the_desktop_package_version_and_enters_the_inventory() {
        let root = tempdir().unwrap();
        stage_application_metadata(root.path()).unwrap();
        let manifest = generate_manifest_json(root.path(), "release").unwrap();
        let files = manifest["files"].as_array().unwrap();
        if NativeBundleLayout::current() == NativeBundleLayout::MacOs {
            let source: toml::Value =
                toml::from_str(&fs::read_to_string(mochi_ui_manifest_path()).unwrap()).unwrap();
            let version = source["package"]["version"].as_str().unwrap();
            let expected = macos_info_plist(version).unwrap();
            assert_eq!(
                fs::read_to_string(root.path().join("Mochi.app/Contents/Info.plist")).unwrap(),
                expected
            );
            assert_eq!(files.len(), 1);
            assert_eq!(
                files[0]["path"].as_str(),
                Some("Mochi.app/Contents/Info.plist")
            );
            assert_eq!(
                files[0]["sha256"].as_str(),
                Some(sha256_hex(expected.as_bytes()).as_str())
            );
            assert!(
                NativeBundleLayout::MacOs
                    .resources_directory(root.path())
                    .is_dir()
            );
            assert!(!root.path().join("bin").exists());
        } else {
            assert!(files.is_empty());
        }
    }

    #[test]
    fn staging_and_inventory_propagate_missing_tree_errors() {
        let root = tempdir().unwrap();
        assert!(generate_manifest_json(&root.path().join("missing"), "release").is_err());
        assert!(
            copy_directory(
                &root.path().join("missing"),
                &root.path().join("destination")
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn staging_and_inventory_reject_indirect_files_and_directories() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("data"), b"not bundle custody").unwrap();
        for target in [outside.clone(), outside.join("data")] {
            symlink(target, source.join("indirect")).unwrap();
            assert!(generate_manifest_json(&source, "release").is_err());
            assert!(copy_directory(&source, &root.path().join("destination")).is_err());
            fs::remove_file(source.join("indirect")).unwrap();
        }
    }

    #[test]
    fn staged_bundle_has_complete_sorted_identical_file_inventory_after_relocation() {
        let root = tempdir().unwrap();
        let source = root.path().join("source");
        let bundle = root.path().join("bundle");
        fs::create_dir(&source).unwrap();
        for name in RUNTIME_BINARIES {
            fs::write(
                source.join(format!("{name}{}", env::consts::EXE_SUFFIX)),
                name,
            )
            .unwrap();
        }
        copy_runtime_binaries(&source, &bundle).unwrap();
        stage_application_metadata(&bundle).unwrap();
        stage_network_profiles(
            Some(&InstalledNetworkProfiles::new(Vec::new()).unwrap()),
            &bundle,
        )
        .unwrap();
        let before = generate_manifest_json(&bundle, "release").unwrap();
        let moved = root.path().join("relocated");
        copy_directory(&bundle, &moved).unwrap();
        fs::remove_dir_all(&bundle).unwrap();
        let after = generate_manifest_json(&moved, "release").unwrap();
        assert_eq!(before["files"], after["files"]);
        let names = after["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(names.len(), if cfg!(target_os = "macos") { 5 } else { 4 });
        let runtime = iroha_deploy::managed::InstalledRuntime::from_directory(
            &NativeBundleLayout::current().runtime_directory(&moved),
        )
        .unwrap();
        assert!(runtime.network_profiles().is_ok());
    }
    #[test]
    fn incomplete_runtime_is_rejected_before_any_binary_is_copied() {
        let root = tempdir().expect("temporary root");
        let source = root.path().join("source");
        let bundle = root.path().join("bundle");
        fs::create_dir(&source).unwrap();
        for name in ["mochi", "kagami"] {
            fs::write(
                source.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
                name,
            )
            .unwrap();
        }
        assert!(copy_runtime_binaries(&source, &bundle).is_err());
        assert!(!bundle.exists());
    }
    #[test]
    fn explicit_installed_profiles_are_canonical_and_covered_by_the_bundle_inventory() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_deploy::bootstrap::{InstalledNetworkProfile, InstalledNetworkProfiles};
        let root = tempdir().unwrap();
        let profiles = InstalledNetworkProfiles::new(vec![
            InstalledNetworkProfile::new(
                "fixture".into(),
                KeyPair::from_seed(vec![21; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
                7,
                "https://release.example/checkpoint.nrt".into(),
            )
            .unwrap(),
        ])
        .unwrap();
        let bytes = profiles.encode_installation().unwrap();
        let source = root.path().join("installer-selected.nrt");
        fs::write(&source, &bytes).unwrap();
        let loaded = super::load_network_profiles(Some(&source))
            .unwrap()
            .unwrap();
        let bundle = root.path().join("bundle");
        fs::create_dir_all(NativeBundleLayout::current().runtime_directory(&bundle)).unwrap();
        loaded.stage(&bundle).unwrap();
        assert_eq!(
            fs::read(NativeBundleLayout::current().profiles_path(&bundle)).unwrap(),
            bytes
        );
        let manifest = super::generate_manifest_json(&bundle, "test").unwrap();
        let entries = manifest.as_object().unwrap()["files"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        let entry = entries[0].as_object().unwrap();
        assert_eq!(
            entry["path"].as_str(),
            Some(
                NativeBundleLayout::current()
                    .profiles_path(Path::new(""))
                    .to_string_lossy()
                    .replace('\\', "/")
                    .as_str()
            )
        );
        assert_eq!(entry["size"].as_u64(), Some(bytes.len() as u64));
        assert_eq!(
            entry["sha256"].as_str(),
            Some(super::sha256_hex(&bytes).as_str())
        );
        assert!(loaded.require_cli_names(br#"["taira"]"#).is_err());
    }
    #[test]
    fn missing_profile_option_installs_no_authority_and_invalid_input_is_rejected() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        assert!(super::load_network_profiles(None).unwrap().is_none());
        super::stage_network_profiles(None, root.path()).unwrap();
        assert!(
            !NativeBundleLayout::current()
                .profiles_path(root.path())
                .exists()
        );
        let source = root.path().join("invalid.nrt");
        fs::write(&source, b"response-selected untrusted authority").unwrap();
        assert!(super::load_network_profiles(Some(&source)).is_err());
        assert!(super::load_network_profiles(Some(&root.path().join("missing.nrt"))).is_err());
        assert!(
            !NativeBundleLayout::current()
                .profiles_path(root.path())
                .exists()
        );
    }
    #[test]
    fn mochi_help_validation_accepts_workspace_desktop_usage() {
        let stdout = format!("{MOCHI_HELP_HEADER}\n");
        assert_eq!(validate_mochi_help_output(&stdout), Ok(()));
    }
    #[test]
    fn mochi_help_validation_rejects_default_feature_stub() {
        let error = validate_mochi_help_output(
            "MOCHI GUI is not enabled in the default workspace build.\n",
        )
        .expect_err("default-feature stub must not pass bundle smoke");
        assert_eq!(error, format!("missing `{MOCHI_HELP_HEADER}`"));
    }
    #[test]
    fn mochi_help_validation_rejects_retired_sandbox_usage() {
        let error = validate_mochi_help_output("MOCHI usage:\nmochi sandbox serve [options]")
            .expect_err("retired supervisor CLI must not pass bundle smoke");
        assert_eq!(error, format!("missing `{MOCHI_HELP_HEADER}`"));
    }
    #[test]
    fn create_archive_packages_bundle_directory() {
        let tempdir = tempdir().expect("tempdir");
        let output_root = tempdir.path();
        let bundle_name = "mochi-test-bundle";
        let bundle_root = output_root.join(bundle_name);
        fs::create_dir_all(NativeBundleLayout::current().runtime_directory(&bundle_root))
            .expect("bundle dir");
        fs::write(
            NativeBundleLayout::current().executable(&bundle_root, "mochi"),
            b"binary",
        )
        .expect("bundle file");
        let archive_path =
            create_archive(output_root, bundle_name, &bundle_root).expect("archive builds");
        assert!(archive_path.exists(), "archive should exist");
        let listing = Command::new("tar")
            .arg("-tzf")
            .arg(&archive_path)
            .output()
            .expect("archive listing");
        assert!(
            listing.status.success(),
            "archive listing should succeed: {:?}",
            listing.status
        );
        let stdout = String::from_utf8(listing.stdout).expect("utf8 listing");
        assert!(
            stdout.lines().any(|line| line
                == format!(
                    "{bundle_name}/{}",
                    NativeBundleLayout::current()
                        .executable(Path::new(""), "mochi")
                        .to_string_lossy()
                        .replace('\\', "/")
                )),
            "archive listing did not include bundle payload: {stdout}"
        );
    }
}
