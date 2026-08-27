//! Export predecoder golden vectors (JSON/bin) for cross-implementation reuse.
//!
//! Writes a canonical 32-bit instruction stream, its decoded op list, and a
//! small set of header-variant artifacts under:
//!   `crates/ivm/tests/fixtures/predecoder/mixed/`
//!
//! Files produced:
//! - `code.bin`                 — raw instruction bytes (no header)
//! - `decoded.json`             — decoded ops: [{ pc, len, inst, inst_hex }]
//! - `index.json`               — list of artifact files and their metadata
//! - `artifacts/*.to`           — header + code artifacts for selected variants
//!
//! Usage:
//!   cargo run -p ivm --features dev-tools --bin ivm_predecoder_export
//!   cargo run -p ivm --features dev-tools --bin ivm_predecoder_export -- --out-dir /tmp/predecoder-fixtures
//!
//! Notes:
//! - The decoded op list is invariant across the header variants emitted here.
//! - JSON is produced via `norito::json` as per repo policy.
use std::path::PathBuf;
#[derive(Debug, PartialEq, Eq)]
struct Options {
    output_dir: PathBuf,
}
fn parse_options_from(
    arguments: impl IntoIterator<Item = String>,
    default_output_dir: PathBuf,
) -> Result<Options, Box<dyn std::error::Error>> {
    let mut output_dir = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--out-dir" if output_dir.is_none() => {
                let value = arguments
                    .next()
                    .ok_or("--out-dir requires a directory path")?;
                if value.is_empty() || value.starts_with('-') {
                    return Err("--out-dir requires a non-empty directory path".into());
                }
                output_dir = Some(PathBuf::from(value));
            }
            "--out-dir" => return Err("--out-dir was supplied more than once".into()),
            _ => {
                return Err(
                    format!("unknown argument `{argument}`; expected --out-dir <path>").into(),
                );
            }
        }
    }
    Ok(Options {
        output_dir: output_dir.unwrap_or(default_output_dir),
    })
}
fn parse_options() -> Result<Options, Box<dyn std::error::Error>> {
    parse_options_from(
        std::env::args().skip(1),
        ivm::predecoder_fixtures::default_predecoder_mixed_root(),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = parse_options()?.output_dir;
    ivm::predecoder_fixtures::generate_predecoder_mixed_fixtures(&root)?;
    eprintln!("wrote fixtures to {}", root.display());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_directory_defaults_to_the_live_fixture_path_and_can_be_staged() {
        let default = PathBuf::from("/workspace/predecoder");
        assert_eq!(
            parse_options_from(Vec::new(), default.clone()).expect("default options"),
            Options {
                output_dir: default,
            }
        );
        assert_eq!(
            parse_options_from(
                ["--out-dir".to_owned(), "/staged/predecoder".to_owned()],
                PathBuf::from("/workspace/predecoder"),
            )
            .expect("staged options"),
            Options {
                output_dir: PathBuf::from("/staged/predecoder"),
            }
        );
    }
    #[test]
    fn malformed_output_directory_options_are_rejected() {
        let default = PathBuf::from("/workspace/predecoder");
        for arguments in [
            vec!["--out-dir".to_owned()],
            vec!["--out-dir".to_owned(), String::new()],
            vec!["--out-dir".to_owned(), "--unknown".to_owned()],
            vec!["--out-dir".to_owned(), "-h".to_owned()],
            vec![
                "--out-dir".to_owned(),
                "/first".to_owned(),
                "--out-dir".to_owned(),
                "/second".to_owned(),
            ],
            vec!["--unknown".to_owned()],
        ] {
            assert!(parse_options_from(arguments, default.clone()).is_err());
        }
    }
}
