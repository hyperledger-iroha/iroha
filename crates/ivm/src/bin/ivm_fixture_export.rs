//! Check or atomically publish all generator-owned repository IVM fixtures.
//!
//! Usage:
//! `cargo run --locked -p ivm --bin ivm_fixture_export -- --check`
//! `cargo run --locked -p ivm --bin ivm_fixture_export -- --write`

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use ivm::prebuilt_fixtures::{
    SYNTHETIC_EXECUTOR_FIXTURES, build_default_executor_program,
    build_synthetic_executor_program,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
}

fn parse_mode_from(arguments: &[String]) -> Result<Mode, String> {
    match arguments {
        [argument] if argument == "--check" => Ok(Mode::Check),
        [argument] if argument == "--write" => Ok(Mode::Write),
        _ => Err("expected exactly one of --check or --write".to_owned()),
    }
}

fn parse_mode() -> Result<Mode, String> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    parse_mode_from(&arguments)
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("IVM crate belongs to the workspace")
        .to_path_buf()
}

fn publish(path: &Path, expected: &[u8], mode: Mode) -> Result<(), String> {
    if fs::read(path).ok().as_deref() == Some(expected) {
        eprintln!("fresh {}", path.display());
        return Ok(());
    }
    if mode == Mode::Check {
        return Err(format!("stale or missing generated fixture {}", path.display()));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("fixture has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create {}: {error}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("fixture name is not UTF-8: {}", path.display()))?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", process::id()));
    fs::write(&temporary, expected)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!(
            "atomically replace {} with {}: {error}",
            path.display(),
            temporary.display()
        )
    })?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

fn main() -> Result<(), String> {
    let mode = parse_mode()?;
    let root = repository_root();

    publish(
        &root.join("defaults/executor.to"),
        &build_default_executor_program(),
        mode,
    )?;
    for (tag, name) in SYNTHETIC_EXECUTOR_FIXTURES.iter().enumerate() {
        let tag = u8::try_from(tag).expect("synthetic fixture inventory fits u8");
        publish(
            &root
                .join("integration_tests/fixtures/ivm")
                .join(name)
                .with_extension("to"),
            &build_synthetic_executor_program(tag),
            mode,
        )?;
    }

    let stage = root
        .join("target/ivm-fixture-export")
        .join(process::id().to_string());
    if stage.exists() {
        fs::remove_dir_all(&stage)
            .map_err(|error| format!("clear staging directory {}: {error}", stage.display()))?;
    }
    ivm::predecoder_fixtures::generate_predecoder_mixed_fixtures(&stage)
        .map_err(|error| format!("generate staged predecoder fixtures: {error}"))?;
    let destination = root.join("crates/ivm/tests/fixtures/predecoder/mixed");
    for relative in [
        Path::new("code.bin"),
        Path::new("decoded.json"),
        Path::new("index.json"),
        Path::new("artifacts/artifact_v1_1_mode00_vlen0_cycles0_abi1.to"),
        Path::new("artifacts/artifact_v1_1_mode03_vlen8_cycles1000_abi1.to"),
    ] {
        let expected = fs::read(stage.join(relative))
            .map_err(|error| format!("read staged {}: {error}", relative.display()))?;
        publish(&destination.join(relative), &expected, mode)?;
    }
    fs::remove_dir_all(&stage)
        .map_err(|error| format!("remove staging directory {}: {error}", stage.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_requires_an_explicit_non_mutating_or_mutating_mode() {
        assert_eq!(
            parse_mode_from(&["--check".to_owned()]),
            Ok(Mode::Check)
        );
        assert_eq!(
            parse_mode_from(&["--write".to_owned()]),
            Ok(Mode::Write)
        );
        assert!(parse_mode_from(&[]).is_err());
        assert!(parse_mode_from(&["--write".to_owned(), "extra".to_owned()]).is_err());
    }

    #[test]
    fn publish_is_checkable_idempotent_and_replaces_stale_bytes() {
        let directory = env::temp_dir().join(format!(
            "ivm-fixture-export-test-{}-{}",
            process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let path = directory.join("fixture.to");
        let _ = fs::remove_dir_all(&directory);

        assert!(publish(&path, b"canonical", Mode::Check).is_err());
        publish(&path, b"canonical", Mode::Write).expect("publish fixture");
        publish(&path, b"canonical", Mode::Check).expect("fresh fixture passes check");
        fs::write(&path, b"stale").expect("make fixture stale");
        assert!(publish(&path, b"canonical", Mode::Check).is_err());
        publish(&path, b"canonical", Mode::Write).expect("replace stale fixture");
        assert_eq!(fs::read(&path).expect("read fixture"), b"canonical");

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn repository_root_contains_the_ivm_crate() {
        assert!(repository_root().join("crates/ivm/Cargo.toml").is_file());
    }
}
