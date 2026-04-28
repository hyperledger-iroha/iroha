//! Build script that stages deterministic IVM sample bytecode for integration tests.
//!
//! The fixtures are versioned under `integration_tests/fixtures/ivm` and copied
//! into `crates/ivm/target/prebuilt/samples` so existing test helpers keep
//! working without compiling Kotodama/IVM logic in the build script.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

const SAMPLE_MANIFEST: &str = include_str!("../crates/ivm/prebuilt_samples.txt");

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

    let root = workspace_root();
    let fixtures_dir = root.join("integration_tests/fixtures/ivm");
    let prebuilt_dir = root.join("crates/ivm/target/prebuilt");
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

    let fallback = fs::read(sample_path(&fixtures_dir, "executor_with_admin")).unwrap_or_default();
    for name in sample_names {
        let source = sample_path(&fixtures_dir, name);
        let destination = sample_path(&samples_dir, name);

        match fs::read(&source) {
            Ok(bytes) => {
                write_file_if_changed(&destination, &bytes).unwrap_or_else(|err| {
                    panic!("failed to stage {}: {err}", destination.display())
                });
            }
            Err(_) if !fallback.is_empty() => {
                println!(
                    "cargo:warning=missing fixture {}; staging fallback placeholder",
                    source.display()
                );
                write_file_if_changed(&destination, &fallback).unwrap_or_else(|err| {
                    panic!("failed to write fallback {}: {err}", destination.display())
                });
            }
            Err(err) => {
                panic!(
                    "missing fixture {} and no fallback available: {err}",
                    source.display()
                );
            }
        }
    }
}
