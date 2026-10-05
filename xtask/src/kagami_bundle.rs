//! Assemble the two matching native CLI programs without a desktop build dependency.

use crate::{network_profiles, workspace_root};
use iroha_deploy::managed::{InstalledRuntime, KagamiBundleLayout, admit_native_program};
use iroha_fs::{FileSnapshot, OwnerDirectory, PublishMode, RetainedFile};
use norito::json::{self, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const PROGRAMS: [&str; 2] = ["kagami", "iroha3d"];

pub(crate) fn validate_profile(profile: &str) -> Result<(), &'static str> {
    match profile {
        "debug" | "release" => Ok(()),
        _ => Err("kagami-bundle accepts only debug or release; local-release cannot be packaged"),
    }
}

fn build_args(profile: &str) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("build"),
        OsString::from("--locked"),
        OsString::from("--message-format=json-render-diagnostics"),
    ];
    if profile == "release" {
        args.push("--release".into());
    }
    for package in ["iroha_kagami", "irohad"] {
        args.extend([OsString::from("-p"), OsString::from(package)]);
    }
    for program in PROGRAMS {
        args.extend([OsString::from("--bin"), OsString::from(program)]);
    }
    args
}

/// Build both programs from this checkout in one locked native invocation, then publish once.
/// The inventory establishes local integrity, not authenticated release provenance.
pub(crate) fn bundle(
    output: &Path,
    profile: &str,
    profiles: Option<&Path>,
) -> Result<PathBuf, Box<dyn Error>> {
    bundle_at(&workspace_root(), output, profile, profiles)
}

fn bundle_at(
    source_root: &Path,
    output: &Path,
    profile: &str,
    profiles: Option<&Path>,
) -> Result<PathBuf, Box<dyn Error>> {
    validate_profile(profile)?;
    let profiles = network_profiles::select(source_root, profile, profiles)?;
    let package = output.join(format!(
        "kagami-{}-{}-{profile}",
        env::consts::OS,
        env::consts::ARCH,
    ));
    if package.try_exists()? {
        return Err("CLI package already exists; select a fresh --out directory".into());
    }
    // Reuse Cargo's active target and native jobserver; the daemon keeps its standard features.
    let mut child = Command::new("cargo")
        .args(build_args(profile))
        .current_dir(source_root)
        .stdout(Stdio::piped())
        .spawn()?;
    let stream = child
        .stdout
        .take()
        .ok_or("Cargo artifact stream is absent")?;
    let records = collect_programs(BufReader::new(stream));
    let status = child.wait()?;
    if !status.success() {
        return Err("building matching Kagami and iroha3d failed".into());
    }
    publish(&records?, &package, profile, profiles.as_ref())?;
    Ok(package)
}

fn collect_programs(mut stream: impl BufRead) -> Result<BTreeMap<String, PathBuf>, Box<dyn Error>> {
    let mut programs = BTreeMap::new();
    let mut failure = None;
    loop {
        let mut line = Vec::new();
        let count = stream
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        let record = (|| {
            if line.len() > 1024 * 1024 {
                return Err("Cargo message exceeds fixed byte bound".into());
            }
            let limits = norito::DecodeLimits::new(8192, 1024 * 1024, 8192, 4 * 1024 * 1024, 128);
            json::preflight_slice(
                &line,
                json::JsonPreflightLimits::from_decode_limits(1024 * 1024, limits),
            )?;
            let value: Value =
                norito::with_decode_limits_scope(limits, || json::from_slice(&line))?;
            if value.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
                return Ok(());
            }
            let target = value.get("target").ok_or("Cargo artifact has no target")?;
            let Some(name) = target
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| PROGRAMS.contains(name))
            else {
                return Ok(());
            };
            if target
                .get("kind")
                .and_then(Value::as_array)
                .is_none_or(|kind| kind.len() != 1 || kind[0].as_str() != Some("bin"))
            {
                return Err("CLI artifact is not the requested native binary target".into());
            }
            let path = PathBuf::from(
                value
                    .get("executable")
                    .and_then(Value::as_str)
                    .ok_or("Cargo CLI artifact has no executable")?,
            );
            if !path.is_absolute() || programs.insert(name.into(), path).is_some() {
                return Err(
                    "Cargo CLI artifact is duplicated or lacks an absolute executable path".into(),
                );
            }
            Ok::<(), Box<dyn Error>>(())
        })();
        if let Err(error) = record {
            failure.get_or_insert(error);
        }
        if failure.is_some() {
            // Drain the original pipe so Cargo can finish and be reaped even after rejection.
            io::copy(&mut stream, &mut io::sink())?;
            break;
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if programs.len() != PROGRAMS.len() {
        return Err("Cargo did not report both exact CLI executables".into());
    }
    Ok(programs)
}

fn digest(file: &mut RetainedFile) -> Result<String, Box<dyn Error>> {
    file.file_mut().seek(SeekFrom::Start(0))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut hash = Sha256::new();
    loop {
        let count = file.file_mut().read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    file.revalidate()?;
    Ok(hex::encode(hash.finalize()))
}

fn retain_output(
    path: &Path,
    expected_hash: String,
) -> Result<(RetainedFile, FileSnapshot, String), Box<dyn Error>> {
    let mut file = RetainedFile::open_regular(path)?;
    let snapshot = file.snapshot()?;
    if digest(&mut file)? != expected_hash || file.snapshot()? != snapshot {
        return Err("CLI package output changed during admission".into());
    }
    Ok((file, snapshot, expected_hash))
}

fn publish(
    source: &BTreeMap<String, PathBuf>,
    package: &Path,
    profile: &str,
    profiles: Option<&network_profiles::Selection>,
) -> Result<(), Box<dyn Error>> {
    publish_checked(source, package, profile, profiles, &mut || Ok(()))
}

fn publish_checked(
    source: &BTreeMap<String, PathBuf>,
    package: &Path,
    profile: &str,
    profiles: Option<&network_profiles::Selection>,
    before_publication: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    validate_profile(profile)?;
    network_profiles::require_for_profile(profile, profiles)?;
    let mut programs = PROGRAMS
        .into_iter()
        .map(|name| {
            let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
            let mut file = RetainedFile::open_regular(
                source.get(name).ok_or("missing exact Cargo executable")?,
            )?;
            let snapshot = file.snapshot()?;
            admit_native_program(&mut file)?;
            let hash = digest(&mut file)?;
            if file.snapshot()? != snapshot {
                return Err("CLI artifact changed during admission".into());
            }
            Ok((filename, file, snapshot, hash))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let profile_bytes = profiles.map(network_profiles::Selection::bytes);
    let parent =
        OwnerDirectory::open_or_create(package.parent().ok_or("CLI package has no parent")?)?;
    let staging = parent.create_private_child(format!(
        ".kagami-stage-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ))?;
    let runtime = staging.create_child("bin")?;
    let mut retained_outputs = Vec::new();
    for (filename, file, snapshot, hash) in &mut programs {
        let mut copied = runtime.open_append(filename.as_str())?;
        file.file_mut().seek(SeekFrom::Start(0))?;
        io::copy(file.file_mut(), &mut copied)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            copied.set_permissions(fs::Permissions::from_mode(0o700))?;
        }
        copied.sync_all()?;
        drop(copied);
        let mut copied = RetainedFile::open_regular(runtime.path().join(filename.as_str()))?;
        let copied_snapshot = copied.snapshot()?;
        admit_native_program(&mut copied)?;
        if digest(&mut copied)? != *hash
            || copied.snapshot()? != copied_snapshot
            || file.snapshot()? != *snapshot
            || digest(file)? != *hash
        {
            return Err("matching CLI input changed during package publication".into());
        }
        retained_outputs.push((copied, copied_snapshot, hash.clone()));
    }
    if let Some(bytes) = profile_bytes {
        runtime.write_atomic(
            iroha_deploy::bootstrap::NETWORK_PROFILES_FILENAME,
            bytes,
            PublishMode::CreateNew,
        )?;
        retained_outputs.push(retain_output(
            &runtime
                .path()
                .join(iroha_deploy::bootstrap::NETWORK_PROFILES_FILENAME),
            hex::encode(Sha256::digest(bytes)),
        )?);
    }
    if let Some(profiles) = profiles {
        profiles.verify_installed(&KagamiBundleLayout::profiles_path(staging.path()))?;
    }
    InstalledRuntime::from_directory(runtime.path())?;
    let manifest = inventory(staging.path(), profile, profiles)?;
    let manifest_bytes = json::to_vec(&manifest)?;
    staging.write_atomic("manifest.json", &manifest_bytes, PublishMode::CreateNew)?;
    retained_outputs.push(retain_output(
        &staging.path().join("manifest.json"),
        hex::encode(Sha256::digest(&manifest_bytes)),
    )?);
    runtime.sync()?;
    drop(runtime);
    staging.sync()?;
    before_publication()?;
    for (_, file, snapshot, hash) in &mut programs {
        if file.snapshot()? != *snapshot || digest(file)? != *hash {
            return Err("CLI artifact changed before atomic publication".into());
        }
    }
    for (file, snapshot, hash) in &mut retained_outputs {
        if file.snapshot()? != *snapshot || digest(file)? != *hash || file.snapshot()? != *snapshot
        {
            return Err("CLI package output changed before atomic publication".into());
        }
    }
    if let Some(profiles) = profiles {
        profiles.verify_installed(&KagamiBundleLayout::profiles_path(staging.path()))?;
    }
    if inventory(staging.path(), profile, profiles)? != manifest {
        return Err("CLI package contents changed before atomic publication".into());
    }
    staging.rename_to_sibling(
        package.file_name().ok_or("CLI package has no name")?,
        PublishMode::CreateNew,
    )?;
    Ok(())
}

fn inventory(
    package: &Path,
    profile: &str,
    profiles: Option<&network_profiles::Selection>,
) -> Result<Value, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(package).follow_root_links(false) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err("CLI inventory rejects indirect or nonregular entries".into());
        }
        // This manifest inventories the installed content, not its own encoding.
        // Its exact bytes are retained separately through the publication gate.
        if entry.path() == package.join("manifest.json") {
            continue;
        }
        let relative = entry.path().strip_prefix(package)?;
        let path = relative
            .to_str()
            .ok_or("CLI package path is not Unicode")?
            .replace('\\', "/");
        let mut file = RetainedFile::open_regular(entry.path())?;
        files.push((path, entry.metadata()?.len(), digest(&mut file)?));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let files = files
        .into_iter()
        .map(|(path, size, hash)| {
            Value::Object(Map::from([
                ("path".into(), Value::String(path)),
                ("size".into(), Value::from(size)),
                ("sha256".into(), Value::String(hash)),
            ]))
        })
        .collect();
    Ok(Value::Object(Map::from([
        (
            "schema".into(),
            Value::String("iroha.kagami-bundle.v1".into()),
        ),
        ("profile".into(), Value::String(profile.into())),
        (
            "network_profiles".into(),
            profiles.map_or(Value::Null, network_profiles::Selection::provenance),
        ),
        (
            "target".into(),
            Value::String(format!("{}-{}", env::consts::OS, env::consts::ARCH)),
        ),
        ("files".into(), Value::Array(files)),
    ])))
}

#[cfg(test)]
mod tests;
