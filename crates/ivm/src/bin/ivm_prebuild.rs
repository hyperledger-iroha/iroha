//! Stage deterministic IVM sample bytecode for integration tests.
//!
//! Compiler-owned samples are copied from their checked-in canonical outputs;
//! this tool never substitutes a placeholder for a missing contract.

use std::{env, fs, path::PathBuf};

const SAMPLE_MANIFEST: &str = include_str!("../../prebuilt_samples.txt");

fn prebuilt_sample_names() -> Vec<&'static str> {
    SAMPLE_MANIFEST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("ivm crate must belong to the workspace");
    let fixtures_dir = workspace_root.join("integration_tests/fixtures/ivm");
    let prebuilt_dir = crate_dir.join("target/prebuilt");
    let samples_dir = prebuilt_dir.join("samples");
    fs::create_dir_all(&samples_dir)?;

    let profile = if cfg!(debug_assertions) {
        "Debug"
    } else {
        "Release"
    };
    fs::write(
        prebuilt_dir.join("build_config.toml"),
        format!("profile = \"{profile}\"\n"),
    )?;

    for name in prebuilt_sample_names() {
        let source = fixtures_dir.join(name).with_extension("to");
        let bytes = fs::read(&source).map_err(|error| {
            format!(
                "canonical compiler-owned fixture {} is missing or unreadable: {error}",
                source.display()
            )
        })?;
        let output = samples_dir.join(name).with_extension("to");
        fs::write(&output, &bytes)?;
        eprintln!("wrote {} ({} bytes)", output.display(), bytes.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_manifest_has_no_unknown_or_duplicate_names() {
        let names = prebuilt_sample_names();
        let unique: std::collections::BTreeSet<_> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "sample names must be unique");
        assert!(names.contains(&"threshold_escrow"));
    }
}
