//! Canonical, node-independent admission and create-only manifest publication.
//!
//! Build with
//! `cargo build -p ivm_artifact_admission --features dev-tools --bin ivm_artifact_admit`.
//! This development tool is not part of the default workspace build; it needs
//! no node configuration, network, or signing material.

use std::{ffi::OsString, path::PathBuf, process::ExitCode};

use iroha_fs::{OwnerDirectory, PublishMode};
use ivm_artifact_admission::{MAX_CONTRACT_IMAGE_BYTES, verify_contract_artifact};

const USAGE: &str = "Usage: ivm_artifact_admit --code-file <artifact.to> --out <absent-manifest.json>\n\
                    Admit one complete V1 contract with the canonical verifier and publish its\n\
                    Norito JSON manifest. The output parent must exist; existing files are rejected.";

#[derive(Debug, PartialEq, Eq)]
struct Inputs {
    code_file: PathBuf,
    out: PathBuf,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Option<Inputs>, String> {
    let mut args = args.into_iter();
    let mut code_file = None;
    let mut out = None;
    while let Some(option) = args.next() {
        if option == "--help" && code_file.is_none() && out.is_none() {
            return if args.next().is_none() {
                Ok(None)
            } else {
                Err("--help must be the only argument".into())
            };
        }
        let slot = if option == "--code-file" {
            &mut code_file
        } else if option == "--out" {
            &mut out
        } else {
            return Err(format!("unknown argument: {}", option.to_string_lossy()));
        };
        if slot.is_some() {
            return Err(format!("duplicate argument: {}", option.to_string_lossy()));
        }
        let value = args
            .next()
            .filter(|value| !value.is_empty() && !value.to_string_lossy().starts_with('-'))
            .ok_or_else(|| format!("{} requires a path", option.to_string_lossy()))?;
        *slot = Some(PathBuf::from(value));
    }
    Ok(Some(Inputs {
        code_file: code_file.ok_or("--code-file is required")?,
        out: out.ok_or("--out is required")?,
    }))
}

fn admit(inputs: &Inputs) -> Result<(), String> {
    let maximum = usize::try_from(MAX_CONTRACT_IMAGE_BYTES)
        .ok()
        .and_then(|image| image.checked_add(ivm_abi::metadata::HEADER_SIZE))
        .ok_or("complete contract artifact bound cannot be represented on this host")?;
    let artifact = iroha_fs::read_regular(&inputs.code_file, maximum)
        .map_err(|error| format!("cannot retain contract input: {error}"))?;
    let verified = verify_contract_artifact(&artifact)
        .map_err(|error| format!("contract admission failed: {error}"))?;
    let manifest = norito::json::to_json_pretty(&verified.manifest)
        .map_err(|error| format!("cannot encode canonical manifest: {error}"))?;
    let name = inputs
        .out
        .file_name()
        .ok_or("output path has no file name")?;
    let parent = inputs
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let directory = OwnerDirectory::open(parent)
        .map_err(|error| format!("cannot retain manifest output directory: {error}"))?;
    directory
        .write_atomic(name, manifest.as_bytes(), PublishMode::CreateNew)
        .map_err(|error| format!("cannot create manifest: {error}"))
}

fn main() -> ExitCode {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(None) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(inputs)) => match admit(&inputs) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("error: {error}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "ivm_artifact_admit/tests.rs"]
mod tests;
