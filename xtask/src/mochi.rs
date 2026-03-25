use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use norito::json::{self, Map, Value};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::workspace_root;

#[derive(Debug, Clone)]
pub(crate) struct MochiBundleResult {
    pub target: String,
    pub profile: String,
    pub bundle_name: String,
    pub output_root: PathBuf,
    pub bundle_root: PathBuf,
    pub manifest_path: PathBuf,
    pub archive_path: Option<PathBuf>,
}

pub(crate) fn bundle_mochi(
    output_root: &Path,
    profile: &str,
    archive: bool,
    kagami_override: Option<&Path>,
) -> Result<MochiBundleResult, Box<dyn Error>> {
    build_mochi_ui(profile)?;

    if !output_root.exists() {
        fs::create_dir_all(output_root)?;
    }

    let host = format!("{}-{}", env::consts::OS, env::consts::ARCH);
    let bundle_name = format!("mochi-{host}-{profile}");
    let bundle_root = output_root.join(&bundle_name);

    if bundle_root.exists() {
        fs::remove_dir_all(&bundle_root)?;
    }

    fs::create_dir_all(bundle_root.join("bin"))?;
    fs::create_dir_all(bundle_root.join("config"))?;
    fs::create_dir_all(bundle_root.join("docs"))?;

    let binary_path = resolve_binary_path(profile)?;
    let binary_target = bundle_root
        .join("bin")
        .join(format!("mochi{}", env::consts::EXE_SUFFIX));
    fs::copy(binary_path, &binary_target)?;

    let kagami_path = resolve_kagami_path(profile, kagami_override)?;
    let kagami_target = bundle_root
        .join("bin")
        .join(format!("kagami{}", env::consts::EXE_SUFFIX));
    fs::copy(kagami_path, &kagami_target)?;

    copy_into_bundle("LICENSE", &bundle_root.join("LICENSE"))?;
    copy_into_bundle(
        "mochi/BUNDLE_README.md",
        &bundle_root.join("docs/README.md"),
    )?;
    copy_into_bundle(
        "mochi/sample-config.toml",
        &bundle_root.join("config/sample.toml"),
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

    Ok(MochiBundleResult {
        target: host,
        profile: profile.to_owned(),
        bundle_name,
        output_root: output_root.to_path_buf(),
        bundle_root,
        manifest_path,
        archive_path,
    })
}

pub(crate) fn run_bundle_smoke(result: &MochiBundleResult) -> Result<(), Box<dyn Error>> {
    let mochi_bin = result
        .bundle_root
        .join("bin")
        .join(format!("mochi{}", env::consts::EXE_SUFFIX));
    if !mochi_bin.exists() {
        return Err(format!("missing mochi binary at {}", mochi_bin.display()).into());
    }

    let status = Command::new(&mochi_bin)
        .arg("--help")
        .env("MOCHI_DATA_ROOT", result.bundle_root.join(".smoke"))
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{}` --help exited with status {:?}",
            mochi_bin.display(),
            status
        )
        .into())
    }
}

pub(crate) fn update_bundle_matrix(
    result: &MochiBundleResult,
    matrix_path: &Path,
    smoke_passed: bool,
) -> Result<(), Box<dyn Error>> {
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
    entry.insert("generated_unix_ms".into(), Value::from(timestamp));
    entry.insert("smoke_passed".into(), Value::from(smoke_passed));
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

    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    for entry in WalkDir::new(source).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(source) else {
            continue;
        };
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

fn build_mochi_ui(profile: &str) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command.arg("build");
    if profile == "release" {
        command.arg("--release");
    } else if profile != "debug" {
        command.args(["--profile", profile]);
    }
    command.args(["-p", "mochi-ui-egui"]);
    command.current_dir(workspace_root());
    let status = command.status()?;
    if !status.success() {
        return Err("cargo build -p mochi-ui-egui failed".into());
    }
    Ok(())
}

fn build_kagami(profile: &str) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command.arg("build");
    if profile == "release" {
        command.arg("--release");
    } else if profile != "debug" {
        command.args(["--profile", profile]);
    }
    command.args(["-p", "iroha_kagami"]);
    command.current_dir(workspace_root());
    let status = command.status()?;
    if !status.success() {
        return Err("cargo build -p iroha_kagami failed".into());
    }
    Ok(())
}

fn profile_directory(profile: &str) -> &str {
    match profile {
        "release" => "release",
        "debug" => "debug",
        other => other,
    }
}

fn resolve_binary_path(profile: &str) -> Result<PathBuf, Box<dyn Error>> {
    let profile_dir = profile_directory(profile);
    let binary = cargo_target_dir()
        .join(profile_dir)
        .join(format!("mochi{}", env::consts::EXE_SUFFIX));
    if !binary.exists() {
        return Err(format!("expected MOCHI binary at {}", binary.display()).into());
    }
    Ok(binary)
}

pub(crate) fn resolve_kagami_path(
    profile: &str,
    override_path: Option<&Path>,
) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = override_path {
        if !path.exists() {
            return Err(format!("kagami override {} does not exist", path.display()).into());
        }
        if !path.is_file() {
            return Err(format!("kagami override {} must point to a file", path.display()).into());
        }
        return Ok(path.to_path_buf());
    }

    let profile_dir = profile_directory(profile);
    let candidate = cargo_target_dir()
        .join(profile_dir)
        .join(format!("kagami{}", env::consts::EXE_SUFFIX));
    if candidate.exists() {
        return Ok(candidate);
    }

    build_kagami(profile)?;
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(format!(
            "expected kagami binary at {} after build",
            candidate.display()
        )
        .into())
    }
}

fn copy_into_bundle(source_rel: &str, destination: &Path) -> Result<(), Box<dyn Error>> {
    let source = workspace_root().join(source_rel);
    if !source.exists() {
        return Err(format!("missing bundle asset {source_rel}").into());
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
    for entry in WalkDir::new(bundle_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
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
    use super::create_archive;
    use std::{fs, process::Command};

    use tempfile::tempdir;

    #[test]
    fn create_archive_packages_bundle_directory() {
        let tempdir = tempdir().expect("tempdir");
        let output_root = tempdir.path();
        let bundle_name = "mochi-test-bundle";
        let bundle_root = output_root.join(bundle_name);
        fs::create_dir_all(bundle_root.join("bin")).expect("bundle dir");
        fs::write(bundle_root.join("bin").join("mochi"), b"binary").expect("bundle file");

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
            stdout
                .lines()
                .any(|line| line == format!("{bundle_name}/bin/mochi")),
            "archive listing did not include bundle payload: {stdout}"
        );
    }
}
