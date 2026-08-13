//! Generates deterministic PDP commitment, challenge, and proof fixtures.
//!
//! By default outputs are written to the checked-in PDP fixture directory.
//! `--output-dir PATH` targets one existing isolated PDP directory instead.
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File},
    path::{Component, Path, PathBuf},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use ed25519_dalek::SigningKey;
use hex::encode;
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, to_string_pretty},
};
use sorafs_manifest::{
    ChunkingProfileV1, ProfileId,
    pdp::{
        PDP_HOT_LEAF_SIZE_V1, PDP_PROOF_VERSION_V1, PDP_SEGMENT_SIZE_V1, PdpChallengeV1,
        PdpCommitmentV1, PdpEd25519SignatureV1, PdpHotLeafProofV1, PdpMerkleTreeV1, PdpProofLeafV1,
        PdpProofV1, PdpSampleV1, sign_pdp_proof_ed25519_v1,
    },
    validate_pdp_challenge_bytes, validate_pdp_challenge_proof_bytes,
    validate_pdp_commitment_challenge_proof_bytes, validate_pdp_proof_bytes,
};
const VALIDATION_GENERATED_AT: u64 = 123;
const DEFAULT_FIXTURE_DIR: &str = "fixtures/sorafs_manifest/pdp";
const MAX_OUTPUT_PATH_BYTES: usize = 4 << 10;
const MAX_OUTPUT_PATH_COMPONENTS: usize = 64;
#[derive(Debug, Eq, PartialEq)]
struct Args {
    fixture_dir: PathBuf,
    help: bool,
}
struct BoundOutputDirectory {
    display_path: PathBuf,
    canonical_path: PathBuf,
    handle: File,
}
impl BoundOutputDirectory {
    fn open(path: &Path, label: &str) -> Result<Self, Box<dyn Error>> {
        require_real_directory_ancestry(path, label)?;
        let before = fs::symlink_metadata(path)
            .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(format!("{label} must be an existing non-symlink directory").into());
        }
        let handle = File::open(path)
            .map_err(|error| format!("failed to open {label} `{}`: {error}", path.display()))?;
        let opened = handle.metadata().map_err(|error| {
            format!(
                "failed to inspect opened {label} `{}`: {error}",
                path.display()
            )
        })?;
        let after = fs::symlink_metadata(path).map_err(|error| {
            format!(
                "failed to reinspect {label} `{}` after opening: {error}",
                path.display()
            )
        })?;
        require_real_directory_ancestry(path, label)?;
        if after.file_type().is_symlink()
            || !after.is_dir()
            || !same_directory_identity(&before, &opened)
            || !same_directory_identity(&before, &after)
        {
            return Err(format!("{label} changed identity while it was bound").into());
        }
        let canonical_path = fs::canonicalize(path).map_err(|error| {
            format!(
                "failed to canonicalize {label} `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Self {
            display_path: path.to_path_buf(),
            canonical_path,
            handle,
        })
    }
    fn verify(&self, label: &str) -> Result<(), Box<dyn Error>> {
        require_real_directory_ancestry(&self.display_path, label)?;
        let lexical = fs::symlink_metadata(&self.display_path).map_err(|error| {
            format!(
                "failed to reinspect {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        let opened = self.handle.metadata().map_err(|error| {
            format!(
                "failed to reinspect bound {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        let canonical = fs::canonicalize(&self.display_path).map_err(|error| {
            format!(
                "failed to recanonicalize {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        if lexical.file_type().is_symlink()
            || !lexical.is_dir()
            || !same_directory_identity(&lexical, &opened)
            || canonical != self.canonical_path
        {
            return Err(format!("{label} changed identity during fixture generation").into());
        }
        Ok(())
    }
}
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Args, Box<dyn Error>> {
    const USAGE: &str = "usage: generate_pdp_fixtures [--output-dir PATH]";
    let args = args.into_iter().collect::<Vec<_>>();
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        return Ok(Args {
            fixture_dir: PathBuf::from(DEFAULT_FIXTURE_DIR),
            help: true,
        });
    }
    if args
        .iter()
        .any(|argument| matches!(argument.to_str(), Some("--help" | "-h")))
    {
        return Err("`--help` must be used by itself".into());
    }
    let mut fixture_dir = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--output-dir") => {
                if fixture_dir.is_some() {
                    return Err("`--output-dir` may be specified only once".into());
                }
                let value = args
                    .next()
                    .ok_or("`--output-dir` requires a separate path argument")?;
                let value_text = value
                    .to_str()
                    .ok_or("`--output-dir` path must be valid UTF-8")?;
                if value_text.is_empty() {
                    return Err("`--output-dir` path must not be empty".into());
                }
                if value_text.starts_with('-') {
                    return Err("`--output-dir` path must not be ambiguous with an option".into());
                }
                if value_text
                    .split(['/', '\\'])
                    .any(|component| component == ".")
                {
                    return Err("`--output-dir` path must not contain `.` components".into());
                }
                if value_text
                    .split(['/', '\\'])
                    .any(|component| component == "..")
                {
                    return Err("`--output-dir` path must not contain `..` components".into());
                }
                let path = PathBuf::from(value);
                validate_output_dir(&path)?;
                fixture_dir = Some(path);
            }
            Some(other) => {
                return Err(format!("{USAGE}; unrecognized argument `{other}`").into());
            }
            None => return Err(format!("{USAGE}; arguments must be valid UTF-8").into()),
        }
    }
    Ok(Args {
        fixture_dir: fixture_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_FIXTURE_DIR)),
        help: false,
    })
}
fn validate_output_dir(path: &Path) -> Result<(), Box<dyn Error>> {
    let path_text = path
        .to_str()
        .ok_or("`--output-dir` path must be valid UTF-8")?;
    if path_text.len() > MAX_OUTPUT_PATH_BYTES {
        return Err(
            format!("`--output-dir` path exceeds the {MAX_OUTPUT_PATH_BYTES}-byte bound").into(),
        );
    }
    let mut has_normal_component = false;
    let mut component_count = 0_usize;
    for component in path.components() {
        component_count = component_count
            .checked_add(1)
            .ok_or("`--output-dir` path component count overflowed")?;
        match component {
            Component::Normal(_) => has_normal_component = true,
            Component::CurDir => {
                return Err("`--output-dir` path must not contain `.` components".into());
            }
            Component::ParentDir => {
                return Err("`--output-dir` path must not contain `..` components".into());
            }
            Component::Prefix(_) | Component::RootDir => {}
        }
    }
    if component_count > MAX_OUTPUT_PATH_COMPONENTS {
        return Err(format!(
            "`--output-dir` path exceeds the {MAX_OUTPUT_PATH_COMPONENTS}-component bound"
        )
        .into());
    }
    if !has_normal_component {
        return Err("`--output-dir` must name a bounded PDP fixture directory".into());
    }
    Ok(())
}
fn require_real_directory_ancestry(path: &Path, label: &str) -> Result<(), Box<dyn Error>> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "failed to inspect {label} ancestry `{}`: {error}",
                current.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{label} ancestry must not contain a symbolic link: {}",
                current.display()
            )
            .into());
        }
        if !metadata.is_dir() {
            return Err(format!(
                "{label} ancestry must contain directories only: {}",
                current.display()
            )
            .into());
        }
    }
    Ok(())
}
#[cfg(unix)]
fn same_directory_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_dir() && right.is_dir() && left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_directory_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_dir()
        && right.is_dir()
        && left.created().ok() == right.created().ok()
        && left.modified().ok() == right.modified().ok()
}
fn print_usage() {
    println!(
        "Usage: generate_pdp_fixtures [--output-dir PATH]\n\
         \n\
         Generate the canonical SoraFS PDP commitment, challenge, proof, and\n\
         negative fixtures. PATH must be an existing bounded non-symlink PDP\n\
         fixture directory with an existing real `negative` child directory."
    );
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args_os().skip(1))?;
    if args.help {
        print_usage();
        return Ok(());
    }
    let fixture_binding =
        BoundOutputDirectory::open(&args.fixture_dir, "PDP fixture output directory")?;
    let fixture_dir = fixture_binding.display_path.clone();
    let negative_dir = fixture_dir.join("negative");
    let negative_binding =
        BoundOutputDirectory::open(&negative_dir, "negative PDP fixture output directory")?;
    let manifest_digest = [0x42; 32];
    let provider_id = [0x10; 32];
    let chunk_profile = chunk_profile()?;
    let payload =
        deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize + PDP_HOT_LEAF_SIZE_V1 as usize + 37);
    let tree = PdpMerkleTreeV1::from_bytes(&payload)?;
    let commitment = PdpCommitmentV1::from_tree(
        &tree,
        manifest_digest,
        chunk_profile.clone(),
        4,
        1_700_000_000,
    )?;
    let samples = vec![
        PdpSampleV1 {
            segment_index: 0,
            hot_leaf_indices: vec![0, 3, 7],
        },
        PdpSampleV1 {
            segment_index: 1,
            hot_leaf_indices: vec![0, 1],
        },
    ];
    let challenge = PdpChallengeV1::new(
        commitment.commitment_digest()?,
        manifest_digest,
        provider_id,
        chunk_profile,
        digest("sorafs.pdp.challenge.seed"),
        1_700_000,
        5_432_101,
        1_700_000_010,
        1_700_000_360,
        samples,
    )?;
    let signing_key = SigningKey::from_bytes(&[0x21; 32]);
    let proof = sign_pdp_proof_ed25519_v1(
        PdpProofV1 {
            version: PDP_PROOF_VERSION_V1,
            commitment_digest: challenge.commitment_digest,
            challenge_id: challenge.challenge_id,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            epoch_id: challenge.epoch_id,
            proof_leaves: tree.prove_samples(&challenge.samples, &payload)?,
            issued_at_unix: 1_700_000_050,
            signature: PdpEd25519SignatureV1 {
                public_key: [0; 32],
                signature: [0; 64],
            },
        },
        &signing_key,
    )?;
    write_norito_pair(
        &fixture_dir.join("commitment_v1"),
        &commitment,
        commitment_json(&commitment),
    )?;
    write_norito_pair(
        &fixture_dir.join("challenge_v1"),
        &challenge,
        challenge_json(&challenge),
    )?;
    write_norito_pair(&fixture_dir.join("proof_v1"), &proof, proof_json(&proof))?;
    let commitment_bytes = norito::to_bytes(&commitment)?;
    let challenge_bytes = norito::to_bytes(&challenge)?;
    let proof_bytes = norito::to_bytes(&proof)?;
    let bundle_outcome = validate_pdp_commitment_challenge_proof_bytes(
        &commitment_bytes,
        &challenge_bytes,
        &proof_bytes,
        "commitment_v1.to",
        "challenge_v1.to",
        "proof_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &fixture_dir.join("bundle_validation_outcome_v1.json"),
        &bundle_outcome,
        true,
        "SFS-PDP-DIAG-000",
    )?;
    let mut duplicate_hot_leaf_challenge = challenge.clone();
    duplicate_hot_leaf_challenge.samples[0].hot_leaf_indices = vec![0, 0];
    assert!(duplicate_hot_leaf_challenge.validate().is_err());
    write_norito_pair(
        &negative_dir.join("duplicate_hot_leaf_challenge_v1"),
        &duplicate_hot_leaf_challenge,
        challenge_json(&duplicate_hot_leaf_challenge),
    )?;
    let duplicate_hot_leaf_challenge_bytes = norito::to_bytes(&duplicate_hot_leaf_challenge)?;
    let duplicate_hot_leaf_outcome = validate_pdp_challenge_bytes(
        &duplicate_hot_leaf_challenge_bytes,
        "duplicate_hot_leaf_challenge_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join("duplicate_hot_leaf_challenge_validation_outcome_v1.json"),
        &duplicate_hot_leaf_outcome,
        false,
        "SFS-PDP-001",
    )?;
    let mut missing_signature_proof = proof.clone();
    missing_signature_proof.signature.signature = [0; 64];
    assert!(missing_signature_proof.validate().is_err());
    write_norito_pair(
        &negative_dir.join("missing_signature_proof_v1"),
        &missing_signature_proof,
        proof_json(&missing_signature_proof),
    )?;
    let missing_signature_proof_bytes = norito::to_bytes(&missing_signature_proof)?;
    let missing_signature_outcome = validate_pdp_proof_bytes(
        &missing_signature_proof_bytes,
        "missing_signature_proof_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join("missing_signature_proof_validation_outcome_v1.json"),
        &missing_signature_outcome,
        false,
        "SFS-SIG-008",
    )?;
    let mut missing_segment_path_proof = proof.clone();
    missing_segment_path_proof.proof_leaves[0]
        .segment_merkle_path
        .clear();
    let missing_segment_path_proof =
        sign_pdp_proof_ed25519_v1(missing_segment_path_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("missing_segment_path_proof_v1"),
        &missing_segment_path_proof,
        proof_json(&missing_segment_path_proof),
    )?;
    write_bundle_negative_outcome(
        &negative_dir,
        "missing_segment_path_proof_v1",
        &commitment_bytes,
        &challenge_bytes,
        &missing_segment_path_proof,
        "SFS-PDP-001",
    )?;
    let mut missing_hot_leaf_path_proof = proof.clone();
    missing_hot_leaf_path_proof.proof_leaves[0].hot_leaves[0]
        .segment_hot_merkle_path
        .clear();
    missing_hot_leaf_path_proof.proof_leaves[0].hot_leaves[0]
        .global_hot_merkle_path
        .clear();
    let missing_hot_leaf_path_proof =
        sign_pdp_proof_ed25519_v1(missing_hot_leaf_path_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("missing_hot_leaf_path_proof_v1"),
        &missing_hot_leaf_path_proof,
        proof_json(&missing_hot_leaf_path_proof),
    )?;
    write_bundle_negative_outcome(
        &negative_dir,
        "missing_hot_leaf_path_proof_v1",
        &commitment_bytes,
        &challenge_bytes,
        &missing_hot_leaf_path_proof,
        "SFS-PDP-001",
    )?;
    let mut late_proof = proof.clone();
    late_proof.issued_at_unix = challenge.response_deadline_unix + 1;
    let late_proof = sign_pdp_proof_ed25519_v1(late_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("late_proof_v1"),
        &late_proof,
        proof_json(&late_proof),
    )?;
    write_pair_negative_outcome(
        &negative_dir,
        "late_proof_v1",
        &challenge_bytes,
        &late_proof,
        "SFS-POL-002",
    )?;
    let mut wrong_provider_proof = proof.clone();
    wrong_provider_proof.provider_id = [0x88; 32];
    let wrong_provider_proof = sign_pdp_proof_ed25519_v1(wrong_provider_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("wrong_provider_proof_v1"),
        &wrong_provider_proof,
        proof_json(&wrong_provider_proof),
    )?;
    write_pair_negative_outcome(
        &negative_dir,
        "wrong_provider_proof_v1",
        &challenge_bytes,
        &wrong_provider_proof,
        "SFS-PDP-003",
    )?;
    let mut wrong_manifest_proof = proof.clone();
    wrong_manifest_proof.manifest_digest = [0x77; 32];
    let wrong_manifest_proof = sign_pdp_proof_ed25519_v1(wrong_manifest_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("wrong_manifest_proof_v1"),
        &wrong_manifest_proof,
        proof_json(&wrong_manifest_proof),
    )?;
    write_pair_negative_outcome(
        &negative_dir,
        "wrong_manifest_proof_v1",
        &challenge_bytes,
        &wrong_manifest_proof,
        "SFS-PDP-003",
    )?;
    let mut wrong_path_proof = proof;
    wrong_path_proof.proof_leaves[0].segment_merkle_path[0][0] ^= 0x01;
    let wrong_path_proof = sign_pdp_proof_ed25519_v1(wrong_path_proof, &signing_key)?;
    write_norito_pair(
        &negative_dir.join("wrong_path_proof_v1"),
        &wrong_path_proof,
        proof_json(&wrong_path_proof),
    )?;
    write_bundle_negative_outcome(
        &negative_dir,
        "wrong_path_proof_v1",
        &commitment_bytes,
        &challenge_bytes,
        &wrong_path_proof,
        "SFS-PDP-003",
    )?;
    negative_binding.verify("negative PDP fixture output directory")?;
    fixture_binding.verify("PDP fixture output directory")
}
fn chunk_profile() -> Result<ChunkingProfileV1, Box<dyn Error>> {
    let descriptor = sorafs_manifest::chunker_registry::lookup(ProfileId(1))
        .ok_or_else(|| std::io::Error::other("canonical SF1 chunking profile is not registered"))?;
    Ok(ChunkingProfileV1::from_descriptor(descriptor))
}
fn digest(label: &str) -> [u8; 32] {
    *blake3::hash(label.as_bytes()).as_bytes()
}
fn deterministic_payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index.wrapping_mul(131).wrapping_add(17)) % 251) as u8)
        .collect()
}
fn write_pair_negative_outcome(
    negative_dir: &Path,
    proof_name: &str,
    challenge_bytes: &[u8],
    proof: &PdpProofV1,
    expected_code: &str,
) -> Result<(), Box<dyn Error>> {
    let scenario_name = proof_name.strip_suffix("_v1").unwrap_or(proof_name);
    let proof_bytes = norito::to_bytes(proof)?;
    let outcome = validate_pdp_challenge_proof_bytes(
        challenge_bytes,
        &proof_bytes,
        "challenge_v1.to",
        format!("{proof_name}.to"),
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join(format!("{scenario_name}_validation_outcome_v1.json")),
        &outcome,
        false,
        expected_code,
    )
}
fn write_bundle_negative_outcome(
    negative_dir: &Path,
    proof_name: &str,
    commitment_bytes: &[u8],
    challenge_bytes: &[u8],
    proof: &PdpProofV1,
    expected_code: &str,
) -> Result<(), Box<dyn Error>> {
    let scenario_name = proof_name.strip_suffix("_v1").unwrap_or(proof_name);
    let proof_bytes = norito::to_bytes(proof)?;
    let outcome = validate_pdp_commitment_challenge_proof_bytes(
        commitment_bytes,
        challenge_bytes,
        &proof_bytes,
        "commitment_v1.to",
        "challenge_v1.to",
        format!("{proof_name}.to"),
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join(format!("{scenario_name}_validation_outcome_v1.json")),
        &outcome,
        false,
        expected_code,
    )
}
fn write_expected_outcome(
    path: &Path,
    outcome: &sorafs_manifest::ValidationOutcomeV1,
    expected_ok: bool,
    expected_code: &str,
) -> Result<(), Box<dyn Error>> {
    if outcome.is_ok() != expected_ok || outcome.code != expected_code {
        return Err(format!(
            "generated PDP outcome returned status_ok={} code={}, expected status_ok={expected_ok} code={expected_code}",
            outcome.is_ok(),
            outcome.code,
        )
        .into());
    }
    write_fixture_file(path, format!("{}\n", to_string_pretty(outcome)?).as_bytes())
}
fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    mut json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    write_fixture_file(&base_path.with_extension("to"), &bytes)?;
    if let Value::Object(map) = &mut json_value {
        map.insert("norito_bytes_hex".to_owned(), Value::from(encode(&bytes)));
    }
    let json = to_string_pretty(&json_value)?;
    write_fixture_file(&base_path.with_extension("json"), json.as_bytes())?;
    Ok(())
}
fn write_fixture_file(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("fixture output `{}` must have a parent", path.display()))?;
    require_real_directory_ancestry(parent, "PDP fixture output parent")?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "fixture output `{}` must be a regular non-symlink file",
                    path.display()
                )
                .into());
            }
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(format!(
                    "fixture output `{}` must have exactly one hard link",
                    path.display()
                )
                .into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::write(path, bytes)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "fixture output `{}` changed type while it was written",
            path.display()
        )
        .into());
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(format!(
            "fixture output `{}` changed hard-link count while it was written",
            path.display()
        )
        .into());
    }
    Ok(())
}
fn commitment_json(commitment: &PdpCommitmentV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(commitment.version));
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(commitment.manifest_digest)),
    );
    map.insert(
        "chunk_profile".to_owned(),
        chunk_profile_json(&commitment.chunk_profile),
    );
    map.insert(
        "payload_len".to_owned(),
        Value::from(commitment.payload_len),
    );
    map.insert(
        "hot_leaf_size".to_owned(),
        Value::from(commitment.hot_leaf_size),
    );
    map.insert(
        "segment_size".to_owned(),
        Value::from(commitment.segment_size),
    );
    map.insert(
        "hot_leaf_count".to_owned(),
        Value::from(commitment.hot_leaf_count),
    );
    map.insert(
        "segment_count".to_owned(),
        Value::from(commitment.segment_count),
    );
    map.insert(
        "commitment_root_hot_hex".to_owned(),
        Value::from(encode(commitment.commitment_root_hot)),
    );
    map.insert(
        "commitment_root_segment_hex".to_owned(),
        Value::from(encode(commitment.commitment_root_segment)),
    );
    map.insert(
        "hash_algorithm".to_owned(),
        Value::from(commitment.hash_algorithm.as_str()),
    );
    map.insert(
        "hot_tree_height".to_owned(),
        Value::from(commitment.hot_tree_height),
    );
    map.insert(
        "segment_tree_height".to_owned(),
        Value::from(commitment.segment_tree_height),
    );
    map.insert(
        "sample_window".to_owned(),
        Value::from(commitment.sample_window),
    );
    map.insert("sealed_at".to_owned(), Value::from(commitment.sealed_at));
    Value::Object(map)
}
fn challenge_json(challenge: &PdpChallengeV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(challenge.version));
    map.insert(
        "challenge_id_hex".to_owned(),
        Value::from(encode(challenge.challenge_id)),
    );
    map.insert(
        "commitment_digest_hex".to_owned(),
        Value::from(encode(challenge.commitment_digest)),
    );
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(challenge.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".to_owned(),
        Value::from(encode(challenge.provider_id)),
    );
    map.insert(
        "chunk_profile".to_owned(),
        chunk_profile_json(&challenge.chunk_profile),
    );
    map.insert("seed_hex".to_owned(), Value::from(encode(challenge.seed)));
    map.insert("epoch_id".to_owned(), Value::from(challenge.epoch_id));
    map.insert("drand_round".to_owned(), Value::from(challenge.drand_round));
    map.insert(
        "issued_at_unix".to_owned(),
        Value::from(challenge.issued_at_unix),
    );
    map.insert(
        "response_deadline_unix".to_owned(),
        Value::from(challenge.response_deadline_unix),
    );
    map.insert(
        "samples".to_owned(),
        Value::Array(
            challenge
                .samples
                .iter()
                .map(sample_json)
                .collect::<Vec<_>>(),
        ),
    );
    Value::Object(map)
}
fn proof_json(proof: &PdpProofV1) -> Value {
    let mut map = Map::new();
    map.insert("version".to_owned(), Value::from(proof.version));
    map.insert(
        "commitment_digest_hex".to_owned(),
        Value::from(encode(proof.commitment_digest)),
    );
    map.insert(
        "challenge_id_hex".to_owned(),
        Value::from(encode(proof.challenge_id)),
    );
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(encode(proof.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".to_owned(),
        Value::from(encode(proof.provider_id)),
    );
    map.insert("epoch_id".to_owned(), Value::from(proof.epoch_id));
    map.insert(
        "proof_leaves".to_owned(),
        Value::Array(proof.proof_leaves.iter().map(proof_leaf_json).collect()),
    );
    map.insert(
        "signer_public_key_hex".to_owned(),
        Value::from(encode(proof.signature.public_key)),
    );
    map.insert(
        "signature_hex".to_owned(),
        Value::from(encode(proof.signature.signature)),
    );
    map.insert(
        "issued_at_unix".to_owned(),
        Value::from(proof.issued_at_unix),
    );
    Value::Object(map)
}
fn chunk_profile_json(profile: &ChunkingProfileV1) -> Value {
    let mut map = Map::new();
    map.insert("profile_id".to_owned(), Value::from(profile.profile_id.0));
    map.insert(
        "namespace".to_owned(),
        Value::from(profile.namespace.clone()),
    );
    map.insert("name".to_owned(), Value::from(profile.name.clone()));
    map.insert("semver".to_owned(), Value::from(profile.semver.clone()));
    map.insert("min_size".to_owned(), Value::from(profile.min_size));
    map.insert("target_size".to_owned(), Value::from(profile.target_size));
    map.insert("max_size".to_owned(), Value::from(profile.max_size));
    map.insert("break_mask".to_owned(), Value::from(profile.break_mask));
    map.insert(
        "multihash_code".to_owned(),
        Value::from(profile.multihash_code),
    );
    map.insert(
        "aliases".to_owned(),
        Value::Array(
            profile
                .aliases
                .iter()
                .map(|alias| Value::from(alias.clone()))
                .collect(),
        ),
    );
    Value::Object(map)
}
fn sample_json(sample: &PdpSampleV1) -> Value {
    let mut map = Map::new();
    map.insert(
        "segment_index".to_owned(),
        Value::from(sample.segment_index),
    );
    map.insert(
        "hot_leaf_indices".to_owned(),
        Value::Array(
            sample
                .hot_leaf_indices
                .iter()
                .map(|index| Value::from(*index))
                .collect(),
        ),
    );
    Value::Object(map)
}
fn proof_leaf_json(leaf: &PdpProofLeafV1) -> Value {
    let mut map = Map::new();
    map.insert("segment_index".to_owned(), Value::from(leaf.segment_index));
    map.insert(
        "segment_offset".to_owned(),
        Value::from(leaf.segment_offset),
    );
    map.insert(
        "segment_length".to_owned(),
        Value::from(leaf.segment_length),
    );
    map.insert(
        "segment_merkle_path_hex".to_owned(),
        Value::Array(
            leaf.segment_merkle_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    map.insert(
        "hot_leaves".to_owned(),
        Value::Array(leaf.hot_leaves.iter().map(hot_leaf_json).collect()),
    );
    Value::Object(map)
}
fn hot_leaf_json(leaf: &PdpHotLeafProofV1) -> Value {
    let mut map = Map::new();
    map.insert("leaf_index".to_owned(), Value::from(leaf.leaf_index));
    map.insert("leaf_offset".to_owned(), Value::from(leaf.leaf_offset));
    map.insert("leaf_length".to_owned(), Value::from(leaf.leaf_length));
    map.insert(
        "leaf_bytes_hex".to_owned(),
        Value::from(encode(&leaf.leaf_bytes)),
    );
    map.insert(
        "segment_hot_merkle_path_hex".to_owned(),
        Value::Array(
            leaf.segment_hot_merkle_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    map.insert(
        "global_hot_merkle_path_hex".to_owned(),
        Value::Array(
            leaf.global_hot_merkle_path
                .iter()
                .map(|node| Value::from(encode(node)))
                .collect(),
        ),
    );
    Value::Object(map)
}
