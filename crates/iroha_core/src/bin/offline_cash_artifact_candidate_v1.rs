//! Generate one explicitly unauthenticated Offline Cash V1 artifact candidate.
//!
//! This developer binary never accepts or creates release-authority signing
//! keys. It publishes into a new directory atomically and emits metadata that
//! states which independent evidence and threshold attestation remain absent.

use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use iroha_core::zk::offline_cash_v1::{
    generate_offline_cash_artifacts_v1, offline_cash_artifact_file_name_v1,
    offline_cash_artifact_profile_digest_v1, offline_cash_artifact_protocol_digest_v1,
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_P256_V3_HALO2_K_V1, OFFLINE_CASH_WIRE_VERSION_V1,
    OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
};
use norito::JsonSerialize;
use sha2::{Digest as _, Sha256};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};

const CANDIDATE_JSON: &str = "offline_cash_artifact_manifest_candidate_v1.json";
const CANDIDATE_SHA256: &str = "offline_cash_artifact_manifest_candidate_v1.sha256";
const CANDIDATE_SCHEMA: &str = "iroha.offline_cash.artifact_manifest_candidate.v1";
const CANDIDATE_STATUS: &str = "unauthenticated_developer_candidate";
const RELEASE_STATUS: &str = "not_qualified_not_attested_not_promoted";

const HELP: &str = r#"Generate all 34 Offline Cash V1 developer artifact files.

Usage:
  offline_cash_artifact_candidate_v1 --out-dir <ABSOLUTE_NEW_DIRECTORY>

The output is an unauthenticated manifest candidate. The command never creates,
loads, or stores release-authority signing keys and cannot promote a release.
The output directory is mandatory, must be absolute, must not already exist,
and is committed by one final atomic no-replace rename from an owner-private
staging directory.
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Arguments {
    out_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedArguments {
    Help,
    Run(Arguments),
}

#[derive(Debug, JsonSerialize)]
struct CandidateArtifactV1 {
    ordinal: u32,
    role: String,
    file_name: String,
    byte_len: u64,
    sha256: String,
    protocol_sha256: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct ArtifactManifestCandidateV1 {
    schema: String,
    status: String,
    release_status: String,
    wire_version: u16,
    halo2_k: u32,
    p256_halo2_k: u32,
    profile_sha256: String,
    artifact_set_sha256: String,
    artifacts: Vec<CandidateArtifactV1>,
    absent_release_inputs: Vec<String>,
}

#[derive(Debug, JsonSerialize)]
struct PublicationOutcomeV1 {
    status: String,
    output_directory: String,
    candidate_sha256: String,
    authenticated_release: bool,
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ParsedArguments, String> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let mut out_dir = None;
    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            if out_dir.is_some() || arguments.next().is_some() {
                return Err("--help cannot be combined with other arguments".to_owned());
            }
            return Ok(ParsedArguments::Help);
        }
        if argument == "--out-dir" {
            if out_dir.is_some() {
                return Err("--out-dir may be supplied only once".to_owned());
            }
            let value = arguments
                .next()
                .ok_or_else(|| "--out-dir requires one path".to_owned())?;
            if value.is_empty() {
                return Err("--out-dir cannot be empty".to_owned());
            }
            out_dir = Some(PathBuf::from(value));
            continue;
        }
        return Err(format!("unknown argument: {}", argument.to_string_lossy()));
    }
    let out_dir = out_dir.ok_or_else(|| "--out-dir is required".to_owned())?;
    if !out_dir.is_absolute() {
        return Err("--out-dir must be an absolute path".to_owned());
    }
    Ok(ParsedArguments::Run(Arguments { out_dir }))
}

fn canonical_new_output(path: &Path) -> Result<(PathBuf, PathBuf), String> {
    if !path.is_absolute() {
        return Err("output directory must be absolute".to_owned());
    }
    let leaf = path
        .file_name()
        .ok_or_else(|| "output directory must have one final component".to_owned())?;
    if leaf
        .to_str()
        .is_none_or(|leaf| leaf.is_empty() || leaf.starts_with('.'))
    {
        return Err("output directory leaf must be visible, non-empty UTF-8".to_owned());
    }
    match fs::symlink_metadata(path) {
        Ok(_) => return Err("output directory already exists".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("failed to inspect output directory: {error}")),
    }
    let requested_parent = path
        .parent()
        .ok_or_else(|| "output directory has no parent".to_owned())?;
    let parent = requested_parent
        .canonicalize()
        .map_err(|error| format!("failed to resolve output parent: {error}"))?;
    if !parent
        .metadata()
        .map_err(|error| format!("failed to inspect output parent: {error}"))?
        .is_dir()
    {
        return Err("output parent is not a directory".to_owned());
    }
    let canonical_output = parent.join(leaf);
    if canonical_output != path {
        return Err("--out-dir must use its canonical parent path (no symlink or '..')".to_owned());
    }
    Ok((parent, canonical_output))
}

struct CandidateStagingDirectory {
    path: PathBuf,
    committed: bool,
}

impl CandidateStagingDirectory {
    fn create(parent: &Path, final_path: &Path) -> Result<Self, String> {
        let leaf = final_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "output directory leaf is not UTF-8".to_owned())?;
        let path = parent.join(format!(
            ".{leaf}.offline-cash-v1.{}.staging",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        builder.create(&path).map_err(|error| {
            format!("failed to create owner-private staging directory: {error}")
        })?;
        Ok(Self {
            path,
            committed: false,
        })
    }

    fn create_file(&self, name: &str) -> Result<File, String> {
        if name.is_empty() || name.contains('/') || name.contains("..") {
            return Err("candidate file name is unsafe".to_owned());
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        options
            .open(self.path.join(name))
            .map_err(|error| format!("failed to create candidate file {name}: {error}"))
    }

    fn write_file(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let mut file = self.create_file(name)?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write candidate file {name}: {error}"))?;
        file.flush()
            .map_err(|error| format!("failed to flush candidate file {name}: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync candidate file {name}: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("failed to inspect candidate file {name}: {error}"))?;
        if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
            return Err(format!("candidate file {name} has an inconsistent length"));
        }
        Ok(())
    }

    fn sync(&self) -> Result<(), String> {
        File::open(&self.path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("failed to sync candidate staging directory: {error}"))
    }

    fn commit(mut self, parent: &Path, final_path: &Path) -> Result<(), String> {
        match fs::symlink_metadata(final_path) {
            Ok(_) => return Err("output directory appeared before commit".to_owned()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to recheck output directory: {error}")),
        }
        self.sync()?;
        #[cfg(unix)]
        {
            let staging_name = self
                .path
                .strip_prefix(parent)
                .ok()
                .filter(|path| path.components().count() == 1)
                .ok_or_else(|| "staging directory escaped its canonical parent".to_owned())?;
            let final_name = final_path
                .strip_prefix(parent)
                .ok()
                .filter(|path| path.components().count() == 1)
                .ok_or_else(|| "final directory escaped its canonical parent".to_owned())?;
            let parent_directory = File::open(parent)
                .map_err(|error| format!("failed to pin candidate output parent: {error}"))?;
            rustix::fs::renameat_with(
                &parent_directory,
                staging_name,
                &parent_directory,
                final_name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                format!("failed to atomically publish new candidate directory: {error}")
            })?;
            self.committed = true;
            return parent_directory.sync_all().map_err(|error| {
                format!(
                    "candidate rename committed but parent-directory durability is uncertain: {error}"
                )
            });
        }
        #[cfg(not(unix))]
        {
            let _ = (parent, final_path);
            return Err(
                "atomic no-replace candidate publication is unsupported on this platform"
                    .to_owned(),
            );
        }
    }
}

impl Drop for CandidateStagingDirectory {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn artifact_role_name(role: OfflineCashArtifactRoleV1) -> String {
    offline_cash_artifact_file_name_v1(role)
        .strip_suffix(".bin")
        .expect("canonical artifact name has .bin suffix")
        .to_owned()
}

fn candidate_metadata(
    bindings: &[OfflineCashArtifactBindingV1],
    artifact_set_digest: [u8; 32],
) -> Result<ArtifactManifestCandidateV1, String> {
    if bindings.len() != OfflineCashArtifactRoleV1::ALL.len()
        || bindings
            .iter()
            .zip(OfflineCashArtifactRoleV1::ALL)
            .any(|(binding, expected)| binding.role != expected)
    {
        return Err("candidate bindings are not in exact 34-role order".to_owned());
    }
    let artifacts = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| CandidateArtifactV1 {
            ordinal: u32::try_from(index).expect("34-role ordinal fits u32"),
            role: artifact_role_name(binding.role),
            file_name: offline_cash_artifact_file_name_v1(binding.role).to_owned(),
            byte_len: binding.byte_len,
            sha256: hex::encode(binding.sha256),
            protocol_sha256: offline_cash_artifact_protocol_digest_v1(binding.role)
                .map(hex::encode),
        })
        .collect();
    Ok(ArtifactManifestCandidateV1 {
        schema: CANDIDATE_SCHEMA.to_owned(),
        status: CANDIDATE_STATUS.to_owned(),
        release_status: RELEASE_STATUS.to_owned(),
        wire_version: OFFLINE_CASH_WIRE_VERSION_V1,
        halo2_k: OFFLINE_CASH_HALO2_K_V1,
        p256_halo2_k: OFFLINE_CASH_P256_V3_HALO2_K_V1,
        profile_sha256: hex::encode(offline_cash_artifact_profile_digest_v1()),
        artifact_set_sha256: hex::encode(artifact_set_digest),
        artifacts,
        absent_release_inputs: vec![
            "reviewed_source_tree_and_cargo_lock_evidence".to_owned(),
            "four_validator_restart_replay_and_adversarial_receipts".to_owned(),
            "fuzz_benchmark_and_physical_device_evidence".to_owned(),
            "finalized_hardware_policy_digest".to_owned(),
            "release_authority_policy_and_threshold_attestation".to_owned(),
            "authenticated_release_id_and_promotion_receipt".to_owned(),
        ],
    })
}

fn run(arguments: Arguments) -> Result<PublicationOutcomeV1, Box<dyn Error>> {
    let (parent, final_path) = canonical_new_output(&arguments.out_dir)?;

    // Generate only after rejecting an existing or ambiguous final target. The
    // generator itself holds every large payload in owner-private unlinked
    // spools, so no partially populated candidate directory becomes visible.
    let generated = generate_offline_cash_artifacts_v1()?;
    let metadata = candidate_metadata(generated.bindings(), generated.artifact_set_digest())?;
    let mut metadata_json = norito::json::to_string_pretty(&metadata)?;
    metadata_json.push('\n');
    let metadata_sha256: [u8; 32] = Sha256::digest(metadata_json.as_bytes()).into();
    let metadata_sha256_text = format!("{}\n", hex::encode(metadata_sha256));

    let staging = CandidateStagingDirectory::create(&parent, &final_path)?;
    generated.emit_all(|artifact| {
        let name = offline_cash_artifact_file_name_v1(artifact.role());
        let mut file = staging.create_file(name)?;
        artifact.copy_to(&mut file)?;
        file.flush()
            .map_err(|error| format!("failed to flush {name}: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {name}: {error}"))?;
        let expected = artifact.binding().byte_len;
        let actual = file
            .metadata()
            .map_err(|error| format!("failed to inspect {name}: {error}"))?
            .len();
        if actual != expected {
            return Err(format!("published {name} length changed"));
        }
        Ok(())
    })?;
    staging.write_file(CANDIDATE_JSON, metadata_json.as_bytes())?;
    staging.write_file(CANDIDATE_SHA256, metadata_sha256_text.as_bytes())?;
    staging.commit(&parent, &final_path)?;

    Ok(PublicationOutcomeV1 {
        status: "committed_unauthenticated_candidate".to_owned(),
        output_directory: final_path.display().to_string(),
        candidate_sha256: hex::encode(metadata_sha256),
        authenticated_release: false,
    })
}

fn main() -> ExitCode {
    match parse_arguments(env::args_os()) {
        Ok(ParsedArguments::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(ParsedArguments::Run(arguments)) => match run(arguments) {
            Ok(outcome) => match norito::json::to_string(&outcome) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("candidate committed, but outcome encoding failed: {error}");
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                eprintln!("offline-cash artifact candidate generation failed: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn output_directory_is_explicit_absolute_and_unique() {
        assert_eq!(
            parse_arguments(argv(&["tool", "--help"])),
            Ok(ParsedArguments::Help)
        );
        assert!(parse_arguments(argv(&["tool"])).is_err());
        assert!(parse_arguments(argv(&["tool", "--out-dir", "relative"])).is_err());
        assert!(
            parse_arguments(argv(&[
                "tool",
                "--out-dir",
                "/tmp/one",
                "--out-dir",
                "/tmp/two"
            ]))
            .is_err()
        );
        assert!(parse_arguments(argv(&["tool", "--release-authority-key", "secret"])).is_err());
    }

    #[test]
    fn existing_output_is_rejected_before_staging() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let output = parent.path().join("candidate");
        fs::create_dir(&output).expect("existing output");
        assert!(canonical_new_output(&output).is_err());
    }

    #[test]
    fn staging_commit_is_atomic_and_does_not_replace_existing_target() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let output = parent.path().join("candidate");
        let staging = CandidateStagingDirectory::create(parent.path(), &output)
            .expect("owner-private staging");
        staging
            .write_file("probe", b"complete")
            .expect("staged file");
        staging
            .commit(parent.path(), &output)
            .expect("atomic directory commit");
        assert_eq!(
            fs::read(output.join("probe")).expect("committed file"),
            b"complete"
        );

        let replacement =
            CandidateStagingDirectory::create(parent.path(), &output).expect("second staging");
        assert!(replacement.commit(parent.path(), &output).is_err());
        assert_eq!(
            fs::read(output.join("probe")).expect("original file"),
            b"complete"
        );
    }

    #[test]
    fn candidate_metadata_is_deterministic_all_order_and_non_authorizing() {
        let bindings = OfflineCashArtifactRoleV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, role)| OfflineCashArtifactBindingV1 {
                role,
                sha256: [u8::try_from(index + 1).expect("small role index"); 32],
                byte_len: 1,
            })
            .collect::<Vec<_>>();
        let digest = [0xA5; 32];
        let first = candidate_metadata(&bindings, digest).expect("ordered metadata");
        let second = candidate_metadata(&bindings, digest).expect("ordered metadata");
        let first = norito::json::to_string_pretty(&first).expect("candidate JSON");
        let second = norito::json::to_string_pretty(&second).expect("candidate JSON");
        assert_eq!(first, second);
        assert!(first.contains(CANDIDATE_STATUS));
        assert!(first.contains(RELEASE_STATUS));
        assert!(!first.contains("private_key"));
        assert!(!first.contains("signature_bytes"));
        for (index, role) in OfflineCashArtifactRoleV1::ALL.into_iter().enumerate() {
            let name = artifact_role_name(role);
            let position = first.find(&format!("\"role\": \"{name}\"")).expect("role");
            if index != 0 {
                let previous = artifact_role_name(OfflineCashArtifactRoleV1::ALL[index - 1]);
                assert!(first.find(&format!("\"role\": \"{previous}\"")).unwrap() < position);
            }
        }
    }
}
