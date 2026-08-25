//! Build script that stages deterministic IVM sample bytecode for integration tests.
//!
//! The fixtures are versioned under `integration_tests/fixtures/ivm`. Normal
//! test builds copy them into `crates/ivm/target/prebuilt/samples` so existing
//! helpers keep working without compiling Kotodama/IVM logic here. Sealed
//! release replays redirect those derived bytes below the external Cargo target
//! and never write into their read-only source mirror.
use std::{
    env::{self, VarError},
    fs,
    path::{Component, Path, PathBuf},
};
const SAMPLE_MANIFEST: &str = include_str!("../crates/ivm/prebuilt_samples.txt");
const READ_ONLY_SOURCE_ENV: &str = "IROHA_TEST_PREBUILD_READ_ONLY_SOURCE";
fn prebuilt_sample_names() -> Vec<&'static str> {
    SAMPLE_MANIFEST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .expect("integration_tests must be in workspace root")
}
fn sample_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name).with_extension("to")
}
fn prebuilt_dir(root: &Path) -> PathBuf {
    match env::var(READ_ONLY_SOURCE_ENV) {
        Err(VarError::NotPresent) => root.join("crates/ivm/target/prebuilt"),
        Ok(value) if value == "1" => {
            let target_dir = PathBuf::from(
                env::var_os("CARGO_TARGET_DIR")
                    .expect("read-only source prebuild requires CARGO_TARGET_DIR"),
            );
            assert!(
                target_dir.is_absolute()
                    && !target_dir.components().any(|component| {
                        matches!(component, Component::CurDir | Component::ParentDir)
                    }),
                "read-only source prebuild requires an absolute normalized CARGO_TARGET_DIR"
            );
            target_dir.join("integration-tests-prebuilt")
        }
        Ok(_) => panic!("{READ_ONLY_SOURCE_ENV} must be exactly 1 when present"),
        Err(VarError::NotUnicode(_)) => panic!("{READ_ONLY_SOURCE_ENV} must be valid Unicode"),
    }
}
fn write_file_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../crates/ivm/prebuilt_samples.txt");
    println!("cargo:rerun-if-env-changed={READ_ONLY_SOURCE_ENV}");
    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    let root = workspace_root();
    let fixtures_dir = root.join("integration_tests/fixtures/ivm");
    let prebuilt_dir = prebuilt_dir(&root);
    let samples_dir = prebuilt_dir.join("samples");
    if let Ok(entries) = fs::read_dir(&fixtures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "to") {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {
        "Release"
    } else {
        "Debug"
    };
    let config = format!("profile = \"{profile}\"\n");
    write_file_if_changed(&prebuilt_dir.join("build_config.toml"), config.as_bytes())
        .expect("failed to write build config");
    fs::create_dir_all(&samples_dir).expect("failed to create prebuilt samples directory");
    let mut sample_names = prebuilt_sample_names();
    if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")
        .ok()
        .as_deref()
        == Some("1")
    {
        sample_names.push("default_executor");
    }
    for name in sample_names {
        let source = sample_path(&fixtures_dir, name);
        let destination = sample_path(&samples_dir, name);
        match fs::read(&source) {
            Ok(bytes) => {
                write_file_if_changed(&destination, &bytes).unwrap_or_else(|err| {
                    panic!("failed to stage {}: {err}", destination.display())
                });
            }
            Err(err) => {
                panic!("missing canonical fixture {}: {err}", source.display());
            }
        }
    }
}
