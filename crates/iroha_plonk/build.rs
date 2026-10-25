//! Bind closed built-in source authority to the actual local mathematical inputs.

#[path = "build_support/source_identity.rs"]
mod source_identity;

use sha2::{Digest as _, Sha256};
use std::{fs, path::Path};

const LOCAL_CONTEXT: &str = "CARGO_FAST_NATIVE_SOURCE_CONTEXT";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn watch_digest(snapshot: &source_identity::Snapshot) -> String {
    let mut hash = Sha256::new();
    for path in &snapshot.watched_paths {
        let bytes = path.as_os_str().as_encoded_bytes();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hex(&hash.finalize())
}

fn local_stamp(root: &Path, snapshot: &source_identity::Snapshot) -> String {
    format!(
        "iroha-plonk-local-source-v1\nroot={}\ndigest={}\nwatches={}\n",
        hex(root.as_os_str().as_encoded_bytes()),
        hex(&snapshot.digest),
        watch_digest(snapshot),
    )
}

/// Validate the wrapper's local build context; its digest never replaces our census.
#[cfg(unix)]
fn checked_local_context(
    out_dir: &Path,
    root: &Path,
    snapshot: &source_identity::Snapshot,
) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::MetadataExt as _;

    let value = std::env::var_os(LOCAL_CONTEXT)?;
    assert!(
        std::env::var("VERGEN_GIT_SHA").as_deref() == Ok("local-fast-build")
            && std::env::var_os("IROHA_GIT_COMMIT_HASH").is_none(),
        "local source preflight is only valid for unsealed local-fast-build commands"
    );
    let path = std::path::PathBuf::from(value);
    let parent = path.parent().expect("local source stamp parent");
    let owner = fs::metadata(out_dir).expect("Cargo output owner").uid();
    let directory = fs::symlink_metadata(parent).expect("local source stamp directory");
    let metadata = fs::symlink_metadata(&path).expect("local source stamp");
    assert!(
        path.is_absolute()
            && path.file_name().is_some_and(|name| name == "snapshot-v1")
            && parent
                .file_name()
                .is_some_and(|name| name == ".iroha-local-source")
            && fs::canonicalize(&path).ok().as_deref() == Some(path.as_path())
            && directory.is_dir()
            && directory.uid() == owner
            && directory.mode() & 0o777 == 0o700
            && metadata.is_file()
            && metadata.uid() == owner
            && metadata.nlink() == 1
            && metadata.mode() & 0o777 == 0o600
            && metadata.len() < 16 * 1024,
        "unsafe local source preflight stamp"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read local source stamp"),
        local_stamp(root, snapshot),
        "local source preflight differs from the authoritative build-script census"
    );
    Some(path)
}

#[cfg(not(unix))]
fn checked_local_context(
    _out_dir: &Path,
    _root: &Path,
    _snapshot: &source_identity::Snapshot,
) -> Option<std::path::PathBuf> {
    assert!(
        std::env::var_os(LOCAL_CONTEXT).is_none(),
        "local source preflight is only supported on Unix"
    );
    None
}

/// Export this already compiled host executable as the exact maintained evaluator.
#[cfg(unix)]
fn probe_receipt(root: &Path, snapshot: &source_identity::Snapshot) -> String {
    let executable = std::env::current_exe()
        .and_then(fs::canonicalize)
        .expect("current source evaluator");
    let contents = format!(
        "iroha-plonk-source-probe-v1\nroot={}\nexecutable={}\nexecutable_sha256={}\nbuild_script_sha256={}\nhelper_sha256={}\ndigest={}\nwatches={}\n",
        hex(root.as_os_str().as_encoded_bytes()),
        hex(executable.as_os_str().as_encoded_bytes()),
        hex(&Sha256::digest(
            fs::read(&executable).expect("source evaluator bytes")
        )),
        hex(&Sha256::digest(include_bytes!("build.rs"))),
        hex(&Sha256::digest(include_bytes!(
            "build_support/source_identity.rs"
        ))),
        hex(&snapshot.digest),
        watch_digest(snapshot),
    );
    // Keep local coordination metadata inside the existing generated file.
    // Direct builds retain the exact canonical source and OUT_DIR inventory.
    contents
        .lines()
        .map(|line| format!("// {line}\n"))
        .collect()
}

#[cfg(not(unix))]
fn probe_receipt(_root: &Path, _snapshot: &source_identity::Snapshot) -> String {
    String::new()
}

fn conservative_rescan(out_dir: &Path) {
    let rescan = out_dir.join("native-source-rescan-required");
    match fs::remove_file(&rescan) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => panic!("cannot keep source-rescan input absent: {error}"),
    }
    println!("cargo:rerun-if-changed={}", rescan.display());
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if !arguments.is_empty() {
        assert!(
            arguments.len() == 2 && arguments[0] == "--source-snapshot",
            "usage: build-script-build --source-snapshot <source-root>"
        );
        let root = fs::canonicalize(&arguments[1]).expect("source snapshot root");
        let snapshot = source_identity::snapshot(&root).expect("complete native source census");
        print!("{}", local_stamp(&root, &snapshot));
        return;
    }
    let out_dir =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    let local_build = std::env::var("VERGEN_GIT_SHA").as_deref() == Ok("local-fast-build")
        && std::env::var_os("IROHA_GIT_COMMIT_HASH").is_none();
    if local_build {
        println!("cargo:rerun-if-env-changed={LOCAL_CONTEXT}");
        println!("cargo:rerun-if-env-changed=VERGEN_GIT_SHA");
        println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
    }
    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"),
    );
    let root = manifest.parent().and_then(std::path::Path::parent);
    let snapshot = root
        .ok_or_else(|| "missing workspace root".to_owned())
        .and_then(source_identity::snapshot);
    let mut local_probe = String::new();
    let (digest, complete) = match snapshot {
        Ok(snapshot) => {
            let root = root
                .expect("complete source root")
                .canonicalize()
                .expect("source root");
            let context = checked_local_context(&out_dir, &root, &snapshot);
            if let Some(path) = &context {
                println!("cargo:rerun-if-changed={}", path.display());
            } else {
                // Direct Cargo retains a complete rescan on every invocation.
                // The local wrapper can instead rescan before and after Cargo,
                // updating its stable stamp only when the complete census changes.
                conservative_rescan(&out_dir);
            }
            for path in &snapshot.watched_paths {
                if context.is_some() {
                    // Preserve the complete audit census without making a
                    // same-byte touch or index-only staging operation dirty.
                    // The local preflight owns membership/content invalidation.
                    println!("cargo:iroha-plonk-source-input={}", path.display());
                } else {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
            if local_build {
                local_probe = probe_receipt(&root, &snapshot);
            }
            (snapshot.digest, true)
        }
        Err(error) => {
            assert!(
                std::env::var_os(LOCAL_CONTEXT).is_none(),
                "local source preflight cannot admit an incomplete source census: {error}"
            );
            conservative_rescan(&out_dir);
            // A published generic crate or incomplete checkout must still compile
            // so strict generation/import can run. It gains no built-in authority.
            println!("cargo:warning=closed source registry disabled: {error}");
            println!(
                "cargo:rerun-if-changed={}",
                manifest.join("build.rs").display()
            );
            if let Some(root) = root {
                println!(
                    "cargo:rerun-if-changed={}",
                    root.join(".git/index").display()
                );
                println!(
                    "cargo:rerun-if-changed={}",
                    root.join("Cargo.toml").display()
                );
            }
            ([0; 32], false)
        }
    };
    let read = |name| {
        println!("cargo:rerun-if-env-changed={name}");
        std::env::var(name).unwrap_or_default()
    };
    let arch = read("CARGO_CFG_TARGET_ARCH");
    let os = read("CARGO_CFG_TARGET_OS");
    let width = read("CARGO_CFG_TARGET_POINTER_WIDTH");
    let endian = read("CARGO_CFG_TARGET_ENDIAN");
    let target = read("TARGET");
    let supported = source_identity::supported_target(&arch, &os, &width, &endian);
    let source = format!(
        "// Generated from actual workspace inputs, not an environment digest.\n\
         pub(super) const CURRENT_NATIVE_SOURCE_DIGEST: [u8; 32] = {digest:?};\n\
         pub(super) const NATIVE_SOURCE_CLOSURE_COMPLETE: bool = {complete};\n\
         pub(super) const BUILTIN_TARGET_SUPPORTED: bool = {supported};\n\
         #[allow(dead_code)]\n\
         pub(super) const NATIVE_SOURCE_TARGET: &str = {target:?};\n\
         {local_probe}"
    );
    let output = out_dir.join("native_source_identity.rs");
    if std::fs::read_to_string(&output).ok().as_deref() != Some(&source) {
        std::fs::write(output, source).expect("write native source identity");
    }
}
