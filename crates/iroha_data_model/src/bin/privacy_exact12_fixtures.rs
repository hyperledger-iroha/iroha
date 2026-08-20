//! Generate or verify the closed Exact12 privacy conformance publication.
//!
//! Write mode accepts only an external absolute staging root. Check mode may
//! target the repository root. Both outputs are derived from the current typed
//! V1 transaction model; there is no legacy fixture decoder.

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_data_model::privacy::{
    privacy_exact12_fixture_bundle_bytes_v1, privacy_exact12_matrix_bytes_v1,
};

const MATRIX_PATH: &str = "fixtures/privacy/exact12_v1.tsv";
const BUNDLE_PATH: &str = "fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

fn main() -> Result<(), Box<dyn Error>> {
    let (mode, root) = parse_args(env::args().skip(1))?;
    let root = validate_root(mode, &root)?;
    let matrix = privacy_exact12_matrix_bytes_v1()?;
    let mut bundle = BASE64
        .encode(privacy_exact12_fixture_bundle_bytes_v1()?)
        .into_bytes();
    bundle.push(b'\n');
    for (relative, bytes) in [(MATRIX_PATH, matrix), (BUNDLE_PATH, bundle)] {
        let output = root.join(relative);
        match mode {
            Mode::Write => write_new(&root, &output, &bytes)?,
            Mode::Check => check_exact(&output, &bytes)?,
        }
    }
    Ok(())
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<(Mode, PathBuf), io::Error> {
    let mut mode = None;
    let mut root = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--write" | "--check" if mode.is_none() => {
                mode = Some(if argument == "--write" {
                    Mode::Write
                } else {
                    Mode::Check
                });
            }
            "--output-root" if root.is_none() => {
                root = Some(PathBuf::from(arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--output-root requires a path")
                })?));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: privacy_exact12_fixtures (--write|--check) --output-root <absolute-directory>",
                ));
            }
        }
    }
    Ok((
        mode.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "mode is required"))?,
        root.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "--output-root is required")
        })?,
    ))
}

fn validate_root(mode: Mode, requested: &Path) -> Result<PathBuf, io::Error> {
    if !requested.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output root must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(requested)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "output root must be an existing non-symlink directory",
        ));
    }
    let root = requested.canonicalize()?;
    if root.parent().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "filesystem root is not a valid output root",
        ));
    }
    if mode == Mode::Write {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("data-model crate is inside the workspace")
            .canonicalize()?;
        if root == workspace || root.starts_with(&workspace) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "write mode requires an external staging root",
            ));
        }
    }
    Ok(root)
}

fn write_new(root: &Path, output: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let relative = output
        .strip_prefix(root)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "output escapes staging root"))?;
    if relative.components().count() < 2 || output.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("refusing to replace staged output {}", output.display()),
        ));
    }
    let parent = output.parent().expect("owned output has a parent");
    fs::create_dir_all(parent)?;
    let temporary = output.with_extension("tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, output)?;
    Ok(())
}

fn check_exact(output: &Path, expected: &[u8]) -> Result<(), io::Error> {
    let actual = fs::read(output)?;
    if actual != expected {
        return Err(io::Error::other(format!(
            "Exact12 fixture is stale: {}",
            output.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_require_one_mode_and_absolute_root() {
        assert_eq!(
            parse_args(["--check", "--output-root", "/tmp/exact12"].map(str::to_owned))
                .expect("valid arguments"),
            (Mode::Check, PathBuf::from("/tmp/exact12"))
        );
        assert!(parse_args(["--write"].map(str::to_owned)).is_err());
        assert!(
            parse_args(["--write", "--check", "--output-root", "/tmp/exact12"].map(str::to_owned))
                .is_err()
        );
    }
}
