//! Package externally generated Pasta-cycle material into the ABI-18/V3 release format.
//!
//! This command does not generate cryptographic keys or validator rosters. It
//! frames six reviewed Pasta inputs, validates one canonical finality-roster
//! artifact, and writes the exact release manifest consumed by deploy tooling.
//! This is an unsigned staging step: it validates content and records evidence
//! digests, but it does not authenticate release signatures. The same typed
//! manifest is published as canonical JSON for operators and canonical Norito
//! for native consumers; downstream release loading must require semantic
//! equality between both representations. Production
//! readiness remains governed by the fail-closed native capability record and
//! a separately authenticated manifest/release envelope.

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use iroha_core::zk::kagemusha_v2::{
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
    KagemushaRecursiveSpendPastaCycleArtifactsV3,
};
use iroha_data_model::{
    ChainId,
    asset::AssetDefinitionId,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3, KAGEMUSHA_RECURSIVE_SPEND_MODE_V2,
        KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3,
        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3,
        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3,
        KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3,
        KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2, KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2, KagemushaPastaCycleArtifactKindV3,
        KagemushaPastaCycleArtifactV3, KagemushaPastaCycleParityV1,
        KagemushaPastaCycleProofProfileV1, KagemushaRecursiveSpendArtifactManifestV3,
        KagemushaTopUpFinalityRosterArtifactReferenceV2, KagemushaTopUpFinalityRosterArtifactV2,
    },
};
use sha2::{Digest, Sha256};

const HELP: &str = "\
Package reviewed Pasta-cycle material into one Kagemusha ABI-18/V3 bundle.

Usage:
  cargo run -p iroha_core --bin kagemusha_recursive_spend_v3_bundle -- \\
    --out-dir <new-directory> \\
    --chain-id <chain> --asset-definition-id <asset> --asset-scale <u32> \\
    --generation <id> --parameter-generation <id> --source-commit <40-lower-hex> \\
    --activation-height <u64> --withdrawal-height <u64> \\
    --benchmark-evidence-sha256 <64-lower-hex> \\
    --cryptographic-review-sha256 <64-lower-hex> \\
    --release-attestation-sha256 <64-lower-hex> \\
    --transition-parameters <file> --transition-proving-key <file> \\
    --transition-verifying-key <file> --state-parameters <file> \\
    --state-proving-key <file> --state-verifying-key <file> \\
    --topup-finality-roster <canonical-norito-file>

The output directory must not already exist. Each source must be a non-empty
regular file within its role-specific bound. The command writes six KRV3KEY
files, the exact validated finality-roster archive, manifest.norito, its
manifest.norito.sha256 content identifier, and manifest.json in an owner-only
staging directory. After every staged byte is read back and verified, the
complete directory is promoted atomically without replacing an existing path.
The publication corridor currently requires Unix directory durability and
fails closed on unsupported targets. This command does not authenticate the
evidence or sign the manifest.
";

const REQUIRED_OPTIONS: &[&str] = &[
    "out-dir",
    "chain-id",
    "asset-definition-id",
    "asset-scale",
    "generation",
    "parameter-generation",
    "source-commit",
    "activation-height",
    "withdrawal-height",
    "benchmark-evidence-sha256",
    "cryptographic-review-sha256",
    "release-attestation-sha256",
    "transition-parameters",
    "transition-proving-key",
    "transition-verifying-key",
    "state-parameters",
    "state-proving-key",
    "state-verifying-key",
    "topup-finality-roster",
];

const MANIFEST_JSON_FILE_NAME: &str = "manifest.json";
const MANIFEST_NORITO_FILE_NAME: &str = "manifest.norito";
const MANIFEST_NORITO_SHA256_FILE_NAME: &str = "manifest.norito.sha256";

#[derive(Clone, Copy)]
struct InputSpec {
    option: &'static str,
    file_name: &'static str,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV3,
}

const INPUTS: &[InputSpec] = &[
    InputSpec {
        option: "transition-parameters",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PARAMETERS_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::TransitionEq,
        kind: KagemushaPastaCycleArtifactKindV3::Parameters,
    },
    InputSpec {
        option: "transition-proving-key",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROVING_KEY_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::TransitionEq,
        kind: KagemushaPastaCycleArtifactKindV3::ProvingKey,
    },
    InputSpec {
        option: "transition-verifying-key",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_VERIFYING_KEY_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::TransitionEq,
        kind: KagemushaPastaCycleArtifactKindV3::VerifyingKey,
    },
    InputSpec {
        option: "state-parameters",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STATE_PARAMETERS_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::StateEp,
        kind: KagemushaPastaCycleArtifactKindV3::Parameters,
    },
    InputSpec {
        option: "state-proving-key",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STATE_PROVING_KEY_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::StateEp,
        kind: KagemushaPastaCycleArtifactKindV3::ProvingKey,
    },
    InputSpec {
        option: "state-verifying-key",
        file_name: KAGEMUSHA_RECURSIVE_SPEND_STATE_VERIFYING_KEY_FILE_NAME_V3,
        parity: KagemushaPastaCycleParityV1::StateEp,
        kind: KagemushaPastaCycleArtifactKindV3::VerifyingKey,
    },
];

#[cfg(any(target_os = "macos", target_os = "ios"))]
const INPUT_OPEN_FLAGS: i32 = 0x2000_0000 | 0x0000_0004;
#[cfg(any(target_os = "linux", target_os = "android"))]
const INPUT_OPEN_FLAGS: i32 = 0x0002_0000 | 0x0000_0800;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const INPUT_OPEN_FLAGS: i32 = 0x0000_0100 | 0x0000_0004;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("the V3 bundle packager requires defined O_NOFOLLOW and O_NONBLOCK flags");
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(not(unix))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct FileSnapshot {
    length: u64,
    modified: Option<std::time::SystemTime>,
}

impl FileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                length: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                length: metadata.len(),
                modified: metadata.modified().ok(),
            }
        }
    }

    #[cfg(unix)]
    fn hardlink_identity(self) -> (u64, u64) {
        (self.device, self.inode)
    }
}

struct OpenedInput {
    file: File,
    path: PathBuf,
    snapshot: FileSnapshot,
    size_bytes: u64,
    sha256: [u8; 32],
}

struct BundleMetadata {
    chain_id: ChainId,
    asset: AssetDefinitionId,
    asset_scale: u32,
    generation: String,
    parameter_generation: String,
    source_commit: String,
    activation_height: u64,
    withdrawal_height: u64,
    benchmark_evidence_sha256: [u8; 32],
    cryptographic_review_sha256: [u8; 32],
    release_attestation_sha256: [u8; 32],
}

struct PreparedKeyInput {
    spec: InputSpec,
    input: OpenedInput,
    header_bytes: Vec<u8>,
    total_size: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse_options(env::args().skip(1))?;
    if options.contains_key("help") {
        print!("{HELP}");
        return Ok(());
    }
    for option in REQUIRED_OPTIONS {
        if !options.contains_key(*option) {
            return Err(format!("missing required option --{option}\n\n{HELP}").into());
        }
    }
    build_bundle(&options)
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut options = BTreeMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            if !options.is_empty() || arguments.next().is_some() {
                return Err("--help must be the only argument".into());
            }
            options.insert("help".to_owned(), String::new());
            return Ok(options);
        }
        let option = argument
            .strip_prefix("--")
            .filter(|value| REQUIRED_OPTIONS.contains(value))
            .ok_or_else(|| format!("unknown argument `{argument}`"))?;
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for --{option}"))?;
        if value.starts_with("--") || value.is_empty() {
            return Err(format!("invalid empty value for --{option}").into());
        }
        if options.insert(option.to_owned(), value).is_some() {
            return Err(format!("duplicate option --{option}").into());
        }
    }
    Ok(options)
}

fn required<'a>(options: &'a BTreeMap<String, String>, name: &str) -> &'a str {
    options
        .get(name)
        .expect("required options checked")
        .as_str()
}

fn canonical_unsigned_decimal(value: &str) -> bool {
    value == "0"
        || value.as_bytes().split_first().is_some_and(|(first, rest)| {
            matches!(first, b'1'..=b'9') && rest.iter().all(u8::is_ascii_digit)
        })
}

fn parse_u32(options: &BTreeMap<String, String>, name: &str) -> Result<u32, Box<dyn Error>> {
    let value = required(options, name);
    if !canonical_unsigned_decimal(value) {
        return Err(format!("--{name} must be a canonical unsigned decimal").into());
    }
    value
        .parse::<u32>()
        .map_err(|error| format!("--{name} must fit u32: {error}").into())
}

fn parse_u64(options: &BTreeMap<String, String>, name: &str) -> Result<u64, Box<dyn Error>> {
    let value = required(options, name);
    if !canonical_unsigned_decimal(value) {
        return Err(format!("--{name} must be a canonical unsigned decimal").into());
    }
    value
        .parse::<u64>()
        .map_err(|error| format!("--{name} must fit u64: {error}").into())
}

fn parse_digest(
    options: &BTreeMap<String, String>,
    name: &str,
) -> Result<[u8; 32], Box<dyn Error>> {
    let value = required(options, name);
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!("--{name} must be exact lowercase 32-byte hexadecimal").into());
    }
    let mut digest = [0; 32];
    hex::decode_to_slice(value, &mut digest)?;
    if digest == [0; 32] {
        return Err(format!("--{name} must not be all zero").into());
    }
    Ok(digest)
}

fn build_bundle(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    #[cfg(not(unix))]
    return Err(
        "Kagemusha V3 bundle publication requires Unix directory fsync and no-replace rename"
            .into(),
    );

    #[cfg(unix)]
    {
        build_bundle_unix(options)
    }
}

#[cfg(unix)]
fn build_bundle_unix(options: &BTreeMap<String, String>) -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(required(options, "out-dir"));
    if out_dir.exists() {
        return Err(format!("output directory already exists: {}", out_dir.display()).into());
    }
    let trusted_parent = TrustedOutputParent::open(&out_dir)?;
    let metadata = prepare_bundle_metadata(options)?;
    let evidence_digests = BTreeSet::from([
        metadata.benchmark_evidence_sha256,
        metadata.cryptographic_review_sha256,
        metadata.release_attestation_sha256,
    ]);
    if evidence_digests.len() != 3 {
        return Err("release evidence digests must be pairwise distinct".into());
    }

    let mut prepared_inputs = Vec::with_capacity(INPUTS.len());
    let mut payload_digests = evidence_digests;
    #[cfg(unix)]
    let mut file_identities = BTreeSet::new();
    for spec in INPUTS {
        let input = open_input(
            Path::new(required(options, spec.option)),
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3,
            "Pasta-cycle input",
        )?;
        #[cfg(unix)]
        if !file_identities.insert(input.snapshot.hardlink_identity()) {
            return Err(format!(
                "Pasta-cycle inputs must not alias one file: {}",
                input.path.display()
            )
            .into());
        }
        if !payload_digests.insert(input.sha256) {
            return Err(format!(
                "Pasta-cycle inputs and release evidence must have distinct payload digests: {}",
                input.path.display()
            )
            .into());
        }
        prepared_inputs.push(prepare_key_input(
            input,
            *spec,
            &metadata.generation,
            &metadata.parameter_generation,
        )?);
    }

    let mut roster_input = open_input(
        Path::new(required(options, "topup-finality-roster")),
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        "top-up finality roster",
    )?;
    #[cfg(unix)]
    if !file_identities.insert(roster_input.snapshot.hardlink_identity()) {
        return Err("top-up finality roster aliases a Pasta-cycle input".into());
    }
    if !payload_digests.insert(roster_input.sha256) {
        return Err("top-up finality roster aliases an input or release-evidence digest".into());
    }
    let (roster_bytes, topup_finality_roster_artifact) =
        prepare_topup_finality_roster(&mut roster_input, &metadata)?;

    let staging = tempfile::Builder::new()
        .prefix(".kagemusha-v3-staging-")
        .tempdir_in(&trusted_parent.path)?;
    let staging_name = staging
        .path()
        .file_name()
        .ok_or("temporary publication directory has no file name")?
        .to_owned();
    let staging_dir = PublicationDirectory::open_at(
        &trusted_parent.file,
        staging.path().to_owned(),
        &staging_name,
    )?;
    if let Err(error) = write_bundle(
        &staging_dir,
        metadata,
        prepared_inputs,
        &roster_bytes,
        topup_finality_roster_artifact,
    ) {
        return Err(format!(
            "bundle publication did not complete; no output was published at {}: {error}",
            out_dir.display()
        )
        .into());
    }
    staging_dir.verify_inventory()?;
    staging_dir.sync()?;
    trusted_parent.publish(&staging_name)?;
    let _published_staging_path = staging.keep();
    Ok(())
}

fn prepare_bundle_metadata(
    options: &BTreeMap<String, String>,
) -> Result<BundleMetadata, Box<dyn Error>> {
    let chain_id: ChainId = required(options, "chain-id").parse()?;
    if chain_id.as_str().is_empty()
        || chain_id.as_str().len() > 128
        || chain_id.as_str().trim() != chain_id.as_str()
        || chain_id.as_str().chars().any(char::is_control)
    {
        return Err("--chain-id must be exact non-empty text of at most 128 bytes".into());
    }
    let asset: AssetDefinitionId = required(options, "asset-definition-id").parse()?;
    let asset_scale = parse_u32(options, "asset-scale")?;
    if asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
        return Err(format!(
            "--asset-scale exceeds the Kagemusha bound {KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2}"
        )
        .into());
    }
    let generation = required(options, "generation").to_owned();
    let parameter_generation = required(options, "parameter-generation").to_owned();
    if !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&generation)
        || !iroha_data_model::offline::is_kagemusha_v3_portable_identifier(&parameter_generation)
    {
        return Err(
            "release and parameter generations must be canonical portable identifiers".into(),
        );
    }
    let source_commit = required(options, "source-commit").to_owned();
    if source_commit.len() != 40
        || !source_commit.bytes().any(|byte| byte != b'0')
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("--source-commit must be a nonzero lowercase 40-hex Git object id".into());
    }
    let activation_height = parse_u64(options, "activation-height")?;
    let withdrawal_height = parse_u64(options, "withdrawal-height")?;
    if activation_height == 0 || withdrawal_height <= activation_height {
        return Err("release heights must define a non-empty, nonzero activation window".into());
    }
    Ok(BundleMetadata {
        chain_id,
        asset,
        asset_scale,
        generation,
        parameter_generation,
        source_commit,
        activation_height,
        withdrawal_height,
        benchmark_evidence_sha256: parse_digest(options, "benchmark-evidence-sha256")?,
        cryptographic_review_sha256: parse_digest(options, "cryptographic-review-sha256")?,
        release_attestation_sha256: parse_digest(options, "release-attestation-sha256")?,
    })
}

fn write_bundle(
    publication: &PublicationDirectory,
    metadata: BundleMetadata,
    prepared_inputs: Vec<PreparedKeyInput>,
    roster_bytes: &[u8],
    topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV2,
) -> Result<(), Box<dyn Error>> {
    let mut transition_artifacts = Vec::with_capacity(3);
    let mut state_artifacts = Vec::with_capacity(3);
    let mut headers = Vec::with_capacity(prepared_inputs.len());
    for prepared in prepared_inputs {
        let (header, descriptor) = package_prepared_input(prepared, publication)?;
        match header.parity {
            KagemushaPastaCycleParityV1::TransitionEq => transition_artifacts.push(descriptor),
            KagemushaPastaCycleParityV1::StateEp => state_artifacts.push(descriptor),
        }
        headers.push(header);
    }
    let mut roster_output =
        publication.create_file(KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2)?;
    roster_output.write_all(roster_bytes)?;
    roster_output.sync_all()?;
    drop(roster_output);
    publication.verify_exact_file(
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2,
        roster_bytes,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
    )?;

    let manifest = KagemushaRecursiveSpendArtifactManifestV3 {
        schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3,
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
        mode: KAGEMUSHA_RECURSIVE_SPEND_MODE_V2.to_owned(),
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
        generation: metadata.generation,
        source_commit: metadata.source_commit,
        chain_id: metadata.chain_id,
        asset: metadata.asset,
        asset_scale: metadata.asset_scale,
        activation_height: metadata.activation_height,
        withdrawal_height: metadata.withdrawal_height,
        max_proof_bytes: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3,
        profiles: vec![
            KagemushaPastaCycleProofProfileV1 {
                parity: KagemushaPastaCycleParityV1::TransitionEq,
                circuit_id: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1.to_owned(),
                parameter_generation: metadata.parameter_generation.clone(),
                ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                artifacts: transition_artifacts,
            },
            KagemushaPastaCycleProofProfileV1 {
                parity: KagemushaPastaCycleParityV1::StateEp,
                circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1.to_owned(),
                parameter_generation: metadata.parameter_generation,
                ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
                artifacts: state_artifacts,
            },
        ],
        topup_finality_roster_artifact,
        benchmark_evidence_sha256: metadata.benchmark_evidence_sha256,
        cryptographic_review_sha256: metadata.cryptographic_review_sha256,
        release_attestation_sha256: metadata.release_attestation_sha256,
    };
    manifest
        .validate()
        .map_err(|error| io::Error::other(error.to_string()))?;
    for (header, descriptor) in headers.iter().zip(
        manifest
            .profiles
            .iter()
            .flat_map(|profile| &profile.artifacts),
    ) {
        header
            .validate_against_manifest(&manifest, descriptor)
            .map_err(io::Error::other)?;
    }

    let manifest_norito = norito::to_bytes(&manifest)?;
    let decoded_manifest: KagemushaRecursiveSpendArtifactManifestV3 =
        norito::decode_from_bytes(&manifest_norito)?;
    if decoded_manifest != manifest || norito::to_bytes(&decoded_manifest)? != manifest_norito {
        return Err("canonical Norito manifest round-trip changed its typed value or bytes".into());
    }

    let mut rendered = norito::json::to_string_pretty(&manifest)?;
    rendered.push('\n');
    let mut output = publication.create_file(MANIFEST_NORITO_FILE_NAME)?;
    output.write_all(&manifest_norito)?;
    output.sync_all()?;
    drop(output);

    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_norito).into();
    let manifest_sha256_text = format!("{}\n", hex::encode(manifest_sha256));
    let mut output = publication.create_file(MANIFEST_NORITO_SHA256_FILE_NAME)?;
    output.write_all(manifest_sha256_text.as_bytes())?;
    output.sync_all()?;
    drop(output);

    let mut output = publication.create_file(MANIFEST_JSON_FILE_NAME)?;
    output.write_all(rendered.as_bytes())?;
    output.sync_all()?;
    drop(output);
    publication.verify_exact_file(MANIFEST_NORITO_FILE_NAME, &manifest_norito, 1024 * 1024)?;
    publication.verify_exact_file(
        MANIFEST_NORITO_SHA256_FILE_NAME,
        manifest_sha256_text.as_bytes(),
        65,
    )?;
    publication.verify_exact_file(MANIFEST_JSON_FILE_NAME, rendered.as_bytes(), 1024 * 1024)?;
    Ok(())
}

fn prepare_topup_finality_roster(
    input: &mut OpenedInput,
    metadata: &BundleMetadata,
) -> Result<(Vec<u8>, KagemushaTopUpFinalityRosterArtifactReferenceV2), Box<dyn Error>> {
    let capacity = usize::try_from(input.size_bytes)?;
    let mut bytes = Vec::with_capacity(capacity);
    input.file.read_to_end(&mut bytes)?;
    if bytes.len() != capacity || <[u8; 32]>::from(Sha256::digest(&bytes)) != input.sha256 {
        return Err("top-up finality roster changed while it was being read".into());
    }
    input.rewind_and_verify()?;
    let roster: KagemushaTopUpFinalityRosterArtifactV2 = norito::decode_from_bytes(&bytes)?;
    roster
        .validate()
        .map_err(|error| io::Error::other(error.to_string()))?;
    if norito::to_bytes(&roster)? != bytes {
        return Err("top-up finality roster is not canonically encoded".into());
    }
    if roster.chain_id != metadata.chain_id || roster.artifact_generation != metadata.generation {
        return Err("top-up finality roster chain or generation mismatch".into());
    }
    let mut covered_until = metadata.activation_height;
    for window in &roster.windows {
        if window.withdraws_at_height <= covered_until {
            continue;
        }
        if window.activates_at_height > covered_until {
            return Err(format!(
                "top-up finality roster has a release-window gap at height {covered_until}"
            )
            .into());
        }
        covered_until = window.withdraws_at_height;
        if covered_until >= metadata.withdrawal_height {
            break;
        }
    }
    if covered_until < metadata.withdrawal_height {
        return Err("top-up finality roster does not cover the complete release window".into());
    }

    let descriptor = KagemushaTopUpFinalityRosterArtifactReferenceV2 {
        file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2.to_owned(),
        size_bytes: input.size_bytes,
        sha256: input.sha256,
        artifact_generation: roster.artifact_generation,
        circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
        purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
        artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
    };
    descriptor
        .validate()
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok((bytes, descriptor))
}

#[cfg(test)]
fn package_input(
    source_path: &Path,
    out_dir: &Path,
    spec: &InputSpec,
    generation: &str,
    parameter_generation: &str,
) -> Result<KagemushaPastaCycleArtifactV3, Box<dyn Error>> {
    let input = open_input(
        source_path,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3,
        "Pasta-cycle input",
    )?;
    let prepared = prepare_key_input(input, *spec, generation, parameter_generation)?;
    let publication = PublicationDirectory::open_existing(out_dir.to_owned())?;
    package_prepared_input(prepared, &publication).map(|(_, descriptor)| descriptor)
}

fn prepare_key_input(
    input: OpenedInput,
    spec: InputSpec,
    generation: &str,
    parameter_generation: &str,
) -> Result<PreparedKeyInput, Box<dyn Error>> {
    let circuit_id = match spec.parity {
        KagemushaPastaCycleParityV1::TransitionEq => {
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_EQ_CIRCUIT_ID_V1
        }
        KagemushaPastaCycleParityV1::StateEp => KAGEMUSHA_RECURSIVE_SPEND_STATE_EP_CIRCUIT_ID_V1,
    };
    let header = KagemushaRecursiveSpendPastaCycleArtifactsV3 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_VERSION_V3,
        manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3.to_owned(),
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V1.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V1.to_owned(),
        generation: generation.to_owned(),
        parity: spec.parity,
        circuit_id: circuit_id.to_owned(),
        parameter_generation: parameter_generation.to_owned(),
        ipa_k: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        kind: spec.kind,
        payload_size_bytes: input.size_bytes,
        payload_sha256: input.sha256,
    };
    header.validate_header().map_err(io::Error::other)?;
    let header_bytes = norito::to_bytes(&header)?;
    let header_len = u32::try_from(header_bytes.len())?;
    let total_size =
        u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3.len() + 4)?
            .checked_add(u64::from(header_len))
            .and_then(|size| size.checked_add(input.size_bytes))
            .ok_or_else(|| io::Error::other("V3 artifact size overflow"))?;
    if total_size > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3 {
        return Err(format!(
            "framed artifact exceeds the V3 bound: {}",
            input.path.display()
        )
        .into());
    }

    Ok(PreparedKeyInput {
        spec,
        input,
        header_bytes,
        total_size,
    })
}

fn package_prepared_input(
    mut prepared: PreparedKeyInput,
    publication: &PublicationDirectory,
) -> Result<
    (
        KagemushaRecursiveSpendPastaCycleArtifactsV3,
        KagemushaPastaCycleArtifactV3,
    ),
    Box<dyn Error>,
> {
    let header: KagemushaRecursiveSpendPastaCycleArtifactsV3 =
        norito::decode_from_bytes(&prepared.header_bytes)?;
    let mut output = publication.create_file(prepared.spec.file_name)?;
    let header_len_bytes = u32::try_from(prepared.header_bytes.len())?.to_le_bytes();
    for bytes in [
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3.as_slice(),
        header_len_bytes.as_slice(),
        prepared.header_bytes.as_slice(),
    ] {
        output.write_all(bytes)?;
    }

    let mut copied = 0_u64;
    let mut copy_hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = prepared.input.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(read)?)
            .ok_or_else(|| io::Error::other("input size overflow"))?;
        if copied > prepared.input.size_bytes {
            return Err(format!(
                "input changed while packaging: {}",
                prepared.input.path.display()
            )
            .into());
        }
        output.write_all(&buffer[..read])?;
        copy_hasher.update(&buffer[..read]);
    }
    let copied_digest: [u8; 32] = copy_hasher.finalize().into();
    if copied != prepared.input.size_bytes || copied_digest != prepared.input.sha256 {
        return Err(format!(
            "input changed while packaging: {}",
            prepared.input.path.display()
        )
        .into());
    }
    prepared.input.rewind_and_verify()?;
    output.sync_all()?;
    drop(output);

    let descriptor = publication.verify_artifact(
        prepared.spec,
        &prepared.header_bytes,
        prepared.total_size,
        prepared.input.size_bytes,
        prepared.input.sha256,
    )?;
    descriptor
        .validate()
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok((header, descriptor))
}

fn open_input(path: &Path, maximum: u64, label: &str) -> Result<OpenedInput, Box<dyn Error>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "{label} must be a non-symlink regular file: {}",
            path.display()
        )
        .into());
    }
    let mut options = File::options();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(INPUT_OPEN_FLAGS);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    let before_snapshot = FileSnapshot::from_metadata(&before);
    let opened_snapshot = FileSnapshot::from_metadata(&opened);
    let after_snapshot = FileSnapshot::from_metadata(&after);
    if before.file_type().is_symlink()
        || after.file_type().is_symlink()
        || !opened.is_file()
        || !after.is_file()
        || before_snapshot != opened_snapshot
        || opened_snapshot != after_snapshot
    {
        return Err(format!("{label} changed while it was opened: {}", path.display()).into());
    }
    if opened.len() == 0 || opened.len() > maximum {
        return Err(format!(
            "{label} size is outside its release bound: {}",
            path.display()
        )
        .into());
    }
    let sha256 = hash_open_file(&mut file, opened.len(), path)?;
    file.seek(SeekFrom::Start(0))?;
    let input = OpenedInput {
        file,
        path: path.to_path_buf(),
        snapshot: opened_snapshot,
        size_bytes: opened.len(),
        sha256,
    };
    input.verify_unchanged()?;
    Ok(input)
}

fn hash_open_file(
    input: &mut File,
    expected_size: u64,
    path: &Path,
) -> Result<[u8; 32], Box<dyn Error>> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read)?)
            .ok_or_else(|| io::Error::other("input size overflow"))?;
        if total > expected_size {
            return Err(format!("input changed while hashing: {}", path.display()).into());
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(format!("input changed while hashing: {}", path.display()).into());
    }
    let mut extra = [0_u8; 1];
    if input.read(&mut extra)? != 0 {
        return Err(format!("input grew while hashing: {}", path.display()).into());
    }
    Ok(hasher.finalize().into())
}

impl OpenedInput {
    fn rewind_and_verify(&mut self) -> Result<(), Box<dyn Error>> {
        self.file.seek(SeekFrom::Start(0))?;
        self.verify_unchanged()
    }

    fn verify_unchanged(&self) -> Result<(), Box<dyn Error>> {
        let opened = self.file.metadata()?;
        let current = fs::symlink_metadata(&self.path)?;
        if current.file_type().is_symlink()
            || !current.is_file()
            || FileSnapshot::from_metadata(&opened) != self.snapshot
            || FileSnapshot::from_metadata(&current) != self.snapshot
        {
            return Err(format!("input changed while packaging: {}", self.path.display()).into());
        }
        Ok(())
    }
}

#[cfg(unix)]
struct TrustedOutputParent {
    path: PathBuf,
    file: File,
    output_name: OsString,
}

#[cfg(unix)]
impl TrustedOutputParent {
    fn open(out_dir: &Path) -> Result<Self, Box<dyn Error>> {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let output_name = out_dir
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or("--out-dir must end in one directory name")?
            .to_owned();
        let parent = out_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = fs::canonicalize(parent)?;
        for ancestor in path.ancestors() {
            let metadata = fs::symlink_metadata(ancestor)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "output parent chain contains a non-directory: {}",
                    ancestor.display()
                )
                .into());
            }
            let mode = metadata.permissions().mode();
            if mode & 0o022 != 0 && mode & 0o1000 == 0 {
                return Err(format!(
                    "output parent chain is writable by another principal without sticky protection: {}",
                    ancestor.display()
                )
                .into());
            }
        }
        let final_path = path.join(&output_name);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => {
                return Err(
                    format!("output directory already exists: {}", final_path.display()).into(),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let file = File::open(&path)?;
        let opened = file.metadata()?;
        let current = fs::metadata(&path)?;
        if !opened.is_dir() || opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err("output parent changed while it was opened".into());
        }
        Ok(Self {
            path,
            file,
            output_name,
        })
    }

    fn publish(&self, staging_name: &OsStr) -> io::Result<()> {
        rustix::fs::renameat_with(
            &self.file,
            staging_name,
            &self.file,
            &self.output_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )?;
        self.file.sync_all()
    }
}

struct PublicationDirectory {
    path: PathBuf,
    file: File,
}

impl PublicationDirectory {
    #[cfg(unix)]
    fn open_at(parent: &File, path: PathBuf, name: &OsStr) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags};

        let file = File::from(rustix::fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        Self::validate(path, file)
    }

    #[cfg(test)]
    fn open_existing(path: PathBuf) -> io::Result<Self> {
        let before = fs::symlink_metadata(&path)?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(io::Error::other(
                "publication directory is not a real directory",
            ));
        }
        let file = File::open(&path)?;
        Self::validate(path, file)
    }

    fn validate(path: PathBuf, file: File) -> io::Result<Self> {
        let opened = file.metadata()?;
        if !opened.is_dir() {
            return Err(io::Error::other(
                "publication descriptor is not a directory",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            let current = fs::metadata(&path)?;
            if opened.dev() != current.dev()
                || opened.ino() != current.ino()
                || opened.permissions().mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "publication directory must be the same owner-private directory",
                ));
            }
        }
        Ok(Self { path, file })
    }

    fn create_file(&self, name: &str) -> io::Result<File> {
        validate_publication_file_name(name)?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};

            let file = File::from(rustix::fs::openat(
                &self.file,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?);
            verify_owner_private_regular_file(&file)?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            File::options()
                .write(true)
                .create_new(true)
                .open(self.path.join(name))
        }
    }

    fn open_file(&self, name: &str) -> io::Result<File> {
        validate_publication_file_name(name)?;
        #[cfg(unix)]
        {
            use rustix::fs::{Mode, OFlags};

            let file = File::from(rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            verify_owner_private_regular_file(&file)?;
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            File::open(self.path.join(name))
        }
    }

    fn verify_exact_file(&self, name: &str, expected: &[u8], maximum: u64) -> io::Result<()> {
        let mut file = self.open_file(name)?;
        let expected_len = u64::try_from(expected.len())
            .map_err(|_| io::Error::other("staged file length does not fit u64"))?;
        let metadata = file.metadata()?;
        if metadata.len() != expected_len || metadata.len() > maximum {
            return Err(io::Error::other(format!(
                "staged file has an unexpected length: {name}"
            )));
        }
        let capacity = usize::try_from(metadata.len())
            .map_err(|_| io::Error::other("staged file is too large for this host"))?;
        let mut actual = Vec::with_capacity(capacity);
        file.read_to_end(&mut actual)?;
        if actual != expected {
            return Err(io::Error::other(format!(
                "staged file changed after its write: {name}"
            )));
        }
        Ok(())
    }

    fn verify_artifact(
        &self,
        spec: InputSpec,
        expected_header: &[u8],
        expected_total_size: u64,
        expected_payload_size: u64,
        expected_payload_sha256: [u8; 32],
    ) -> Result<KagemushaPastaCycleArtifactV3, Box<dyn Error>> {
        let mut file = self.open_file(spec.file_name)?;
        let metadata = file.metadata()?;
        if metadata.len() != expected_total_size
            || metadata.len() > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3
        {
            return Err(format!(
                "staged artifact has an unexpected length: {}",
                spec.file_name
            )
            .into());
        }
        let mut magic = [0_u8; 8];
        let mut header_len_bytes = [0_u8; 4];
        file.read_exact(&mut magic)?;
        file.read_exact(&mut header_len_bytes)?;
        let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))?;
        if magic != *KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3
            || header_len == 0
            || header_len > 64 * 1024
            || header_len != expected_header.len()
        {
            return Err(format!("staged artifact has invalid framing: {}", spec.file_name).into());
        }
        let mut header_bytes = vec![0_u8; header_len];
        file.read_exact(&mut header_bytes)?;
        if header_bytes != expected_header {
            return Err(format!("staged artifact header changed: {}", spec.file_name).into());
        }
        let decoded: KagemushaRecursiveSpendPastaCycleArtifactsV3 =
            norito::decode_from_bytes(&header_bytes)?;
        if norito::to_bytes(&decoded)? != header_bytes {
            return Err(
                format!("staged artifact header is noncanonical: {}", spec.file_name).into(),
            );
        }
        decoded.validate_header().map_err(io::Error::other)?;
        if decoded.parity != spec.parity
            || decoded.kind != spec.kind
            || decoded.payload_size_bytes != expected_payload_size
            || decoded.payload_sha256 != expected_payload_sha256
        {
            return Err(
                format!("staged artifact header binding changed: {}", spec.file_name).into(),
            );
        }

        let mut framed_hasher = Sha256::new();
        framed_hasher.update(magic);
        framed_hasher.update(header_len_bytes);
        framed_hasher.update(&header_bytes);
        let mut payload_hasher = Sha256::new();
        let mut payload_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            payload_size = payload_size
                .checked_add(u64::try_from(read)?)
                .ok_or_else(|| io::Error::other("staged artifact payload length overflow"))?;
            if payload_size > expected_payload_size {
                return Err(format!("staged artifact payload grew: {}", spec.file_name).into());
            }
            framed_hasher.update(&buffer[..read]);
            payload_hasher.update(&buffer[..read]);
        }
        let payload_sha256: [u8; 32] = payload_hasher.finalize().into();
        if payload_size != expected_payload_size || payload_sha256 != expected_payload_sha256 {
            return Err(format!("staged artifact payload changed: {}", spec.file_name).into());
        }
        let descriptor = KagemushaPastaCycleArtifactV3 {
            kind: spec.kind,
            file_name: spec.file_name.to_owned(),
            size_bytes: metadata.len(),
            sha256: framed_hasher.finalize().into(),
            payload_size_bytes: payload_size,
            payload_sha256,
        };
        descriptor
            .validate()
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(descriptor)
    }

    fn verify_inventory(&self) -> io::Result<()> {
        let expected = INPUTS
            .iter()
            .map(|spec| spec.file_name)
            .chain([
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2,
                MANIFEST_NORITO_FILE_NAME,
                MANIFEST_NORITO_SHA256_FILE_NAME,
                MANIFEST_JSON_FILE_NAME,
            ])
            .collect::<BTreeSet<_>>();
        let mut actual = BTreeSet::new();
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("publication contains a non-UTF-8 file name"))?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() || !actual.insert(name) {
                return Err(io::Error::other(
                    "publication contains an invalid directory entry",
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                if metadata.nlink() != 1 {
                    return Err(io::Error::other(
                        "publication file has an external hard link",
                    ));
                }
            }
        }
        if actual != expected.into_iter().map(str::to_owned).collect() {
            return Err(io::Error::other(
                "publication file inventory is incomplete or excessive",
            ));
        }
        Ok(())
    }

    fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

fn validate_publication_file_name(name: &str) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(io::Error::other(
            "publication file name must be one normal component",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_owner_private_regular_file(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "publication file must be an owner-private regular file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        domain::DomainId,
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            KagemushaPastaCycleProofEnvelopeV1, KagemushaRecursiveSpendStateBoundaryV1,
            KagemushaTopUpFinalityRosterWindowV2,
        },
        peer::PeerId,
        proof::ProofBox,
    };

    use super::*;

    fn valid_options(root: &Path) -> BTreeMap<String, String> {
        let inputs = root.join("inputs");
        fs::create_dir(&inputs).expect("create input directory");

        let mut options = BTreeMap::from([
            (
                "out-dir".to_owned(),
                root.join("bundle").to_string_lossy().into_owned(),
            ),
            ("chain-id".to_owned(), "kagemusha-pasta-cycle".to_owned()),
            ("asset-scale".to_owned(), "2".to_owned()),
            ("generation".to_owned(), "release-generation-1".to_owned()),
            (
                "parameter-generation".to_owned(),
                "parameters-generation-1".to_owned(),
            ),
            (
                "source-commit".to_owned(),
                "0123456789abcdef0123456789abcdef01234567".to_owned(),
            ),
            ("activation-height".to_owned(), "100".to_owned()),
            ("withdrawal-height".to_owned(), "1000".to_owned()),
            ("benchmark-evidence-sha256".to_owned(), "11".repeat(32)),
            ("cryptographic-review-sha256".to_owned(), "22".repeat(32)),
            ("release-attestation-sha256".to_owned(), "33".repeat(32)),
        ]);
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "pasta".parse().expect("asset name"),
        );
        options.insert("asset-definition-id".to_owned(), asset.to_string());

        for (index, input) in INPUTS.iter().enumerate() {
            let path = inputs.join(input.option);
            fs::write(
                &path,
                vec![u8::try_from(index + 1).expect("small index"); index + 3],
            )
            .expect("write input");
            options.insert(input.option.to_owned(), path.to_string_lossy().into_owned());
        }
        let keypairs = (0..4)
            .map(|_| {
                KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("BLS finality-roster fixture key")
            })
            .collect::<Vec<_>>();
        let validator_set = keypairs
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_set_pops = keypairs
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS finality-roster fixture PoP")
                    .try_into()
                    .expect("96-byte BLS proof of possession")
            })
            .collect::<Vec<_>>();
        let validator_set_hash = HashOf::new(&validator_set);
        let roster = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id: ChainId::from("kagemusha-pasta-cycle"),
            artifact_generation: "release-generation-1".to_owned(),
            windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
                activates_at_height: 100,
                withdraws_at_height: 1_000,
                validator_set_hash: *validator_set_hash.as_ref(),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set,
                validator_set_pops,
            }],
        };
        let roster_path = inputs.join("topup-finality-roster.norito");
        fs::write(
            &roster_path,
            norito::to_bytes(&roster).expect("encode finality-roster fixture"),
        )
        .expect("write finality-roster fixture");
        options.insert(
            "topup-finality-roster".to_owned(),
            roster_path.to_string_lossy().into_owned(),
        );
        options
    }

    #[test]
    fn option_parser_rejects_ambiguous_or_duplicate_arguments() {
        assert!(parse_options(["--chain-id".to_owned(), "chain".to_owned()]).is_ok());
        for arguments in [
            vec!["--unknown".to_owned(), "value".to_owned()],
            vec!["--chain-id".to_owned()],
            vec!["--chain-id".to_owned(), "--asset-scale".to_owned()],
            vec![
                "--chain-id".to_owned(),
                "one".to_owned(),
                "--chain-id".to_owned(),
                "two".to_owned(),
            ],
            vec!["--help".to_owned(), "extra".to_owned()],
        ] {
            assert!(parse_options(arguments).is_err());
        }
        assert_eq!(
            parse_options(["--help".to_owned()])
                .expect("standalone help")
                .get("help"),
            Some(&String::new())
        );
    }

    #[test]
    fn numeric_options_require_canonical_unsigned_decimal() {
        for valid in ["0", "1", "4294967295"] {
            assert!(canonical_unsigned_decimal(valid), "{valid}");
        }
        for invalid in ["", "00", "01", "+1", "-1", " 1", "1 ", "1_0"] {
            assert!(!canonical_unsigned_decimal(invalid), "{invalid}");
        }

        let mut options = BTreeMap::from([("value".to_owned(), "4294967295".to_owned())]);
        assert_eq!(parse_u32(&options, "value").expect("u32 max"), u32::MAX);
        options.insert("value".to_owned(), "4294967296".to_owned());
        assert!(parse_u32(&options, "value").is_err());
        options.insert("value".to_owned(), "18446744073709551615".to_owned());
        assert_eq!(parse_u64(&options, "value").expect("u64 max"), u64::MAX);
        options.insert("value".to_owned(), "18446744073709551616".to_owned());
        assert!(parse_u64(&options, "value").is_err());
    }

    #[test]
    fn evidence_digests_are_exact_lowercase_nonzero_sha256_values() {
        let mut options = BTreeMap::from([("digest".to_owned(), "ab".repeat(32))]);
        assert_eq!(
            parse_digest(&options, "digest").expect("digest"),
            [0xab; 32]
        );
        for invalid in [
            "00".repeat(32),
            "AB".repeat(32),
            format!("0x{}", "ab".repeat(32)),
            "ab".repeat(31),
            format!("{}g0", "ab".repeat(31)),
        ] {
            options.insert("digest".to_owned(), invalid);
            assert!(parse_digest(&options, "digest").is_err());
        }
    }

    #[test]
    fn bundle_contains_exact_streamed_inputs_and_valid_manifest() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        build_bundle(&options).expect("build bundle");
        let out_dir = Path::new(required(&options, "out-dir"));

        let manifest_text =
            fs::read_to_string(out_dir.join("manifest.json")).expect("read generated manifest");
        assert!(manifest_text.ends_with('\n'));
        let manifest: KagemushaRecursiveSpendArtifactManifestV3 =
            norito::json::from_str(&manifest_text).expect("decode generated manifest");
        manifest.validate().expect("validate generated manifest");
        assert_eq!(manifest.mode, KAGEMUSHA_RECURSIVE_SPEND_MODE_V2);
        let manifest_norito =
            fs::read(out_dir.join(MANIFEST_NORITO_FILE_NAME)).expect("read Norito manifest");
        let manifest_from_norito: KagemushaRecursiveSpendArtifactManifestV3 =
            norito::decode_from_bytes(&manifest_norito).expect("decode Norito manifest");
        assert_eq!(manifest_from_norito, manifest);
        assert_eq!(
            norito::to_bytes(&manifest).expect("re-encode Norito manifest"),
            manifest_norito
        );
        let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_norito).into();
        assert_eq!(
            fs::read_to_string(out_dir.join(MANIFEST_NORITO_SHA256_FILE_NAME))
                .expect("read manifest SHA-256"),
            format!("{}\n", hex::encode(manifest_sha256))
        );

        let artifacts = manifest
            .profiles
            .iter()
            .flat_map(|profile| &profile.artifacts)
            .collect::<Vec<_>>();
        assert_eq!(artifacts.len(), INPUTS.len());
        let mut archive_digests = BTreeSet::new();
        for (spec, descriptor) in INPUTS.iter().zip(artifacts) {
            assert_eq!(descriptor.file_name, spec.file_name);
            let archive = fs::read(out_dir.join(spec.file_name)).expect("read archive");
            assert_eq!(descriptor.size_bytes, archive.len() as u64);
            let archive_digest: [u8; 32] = Sha256::digest(&archive).into();
            assert_eq!(descriptor.sha256, archive_digest);
            assert!(archive_digests.insert(archive_digest));
            assert_eq!(
                &archive[..KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3.len()],
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3
            );
            let header_len_offset = KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V3.len();
            let header_len = u32::from_le_bytes(
                archive[header_len_offset..header_len_offset + 4]
                    .try_into()
                    .expect("header length"),
            ) as usize;
            let header_start = header_len_offset + 4;
            let payload_start = header_start + header_len;
            let header: KagemushaRecursiveSpendPastaCycleArtifactsV3 =
                norito::decode_from_bytes(&archive[header_start..payload_start])
                    .expect("decode artifact header");
            header.validate_header().expect("validate artifact header");
            header
                .validate_against_manifest(&manifest, descriptor)
                .expect("bind artifact header to generated manifest");
            assert_eq!(header.parity, spec.parity);
            assert_eq!(header.kind, spec.kind);
            let source = fs::read(required(&options, spec.option)).expect("read source");
            assert_eq!(&archive[payload_start..], source);
            assert_eq!(header.payload_size_bytes, source.len() as u64);
            let payload_sha256: [u8; 32] = Sha256::digest(&source).into();
            assert_eq!(header.payload_sha256, payload_sha256);
            assert_eq!(descriptor.payload_size_bytes, source.len() as u64);
            assert_eq!(descriptor.payload_sha256, payload_sha256);
        }
        let roster_descriptor = &manifest.topup_finality_roster_artifact;
        roster_descriptor
            .validate()
            .expect("validate finality-roster descriptor");
        let roster_bytes = fs::read(out_dir.join(&roster_descriptor.file_name))
            .expect("read packaged finality roster");
        assert_eq!(
            roster_bytes,
            fs::read(required(&options, "topup-finality-roster"))
                .expect("read source finality roster")
        );
        assert_eq!(roster_descriptor.size_bytes, roster_bytes.len() as u64);
        assert_eq!(
            roster_descriptor.sha256,
            <[u8; 32]>::from(Sha256::digest(&roster_bytes))
        );
        assert_eq!(roster_descriptor.artifact_generation, manifest.generation);
        assert_eq!(
            roster_descriptor.circuit_id,
            KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2
        );
        assert_eq!(
            roster_descriptor.purpose,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2
        );
        assert_eq!(
            roster_descriptor.artifact_type,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2
        );
        assert_eq!(
            roster_descriptor.required_bridge_abi_version,
            KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3
        );
        assert!(archive_digests.insert(roster_descriptor.sha256));
        let roster: KagemushaTopUpFinalityRosterArtifactV2 =
            norito::decode_from_bytes(&roster_bytes).expect("decode packaged finality roster");
        roster
            .validate()
            .expect("validate packaged finality roster");
        assert_eq!(roster.chain_id, manifest.chain_id);
        assert_eq!(roster.artifact_generation, manifest.generation);

        for profile in &manifest.profiles {
            let envelope = KagemushaPastaCycleProofEnvelopeV1 {
                version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1,
                proof_backend: manifest.proof_backend.clone(),
                transcript_profile: manifest.transcript_profile.clone(),
                circuit_id: profile.circuit_id.clone(),
                parity: profile.parity,
                artifact_generation: manifest.generation.clone(),
                manifest_sha256,
                parameter_generation: profile.parameter_generation.clone(),
                verifier_key_sha256: profile.artifacts[2].payload_sha256,
                state_boundary: KagemushaRecursiveSpendStateBoundaryV1 {
                    layout_version: KAGEMUSHA_RECURSIVE_SPEND_STATE_BOUNDARY_VERSION_V1,
                    state_digest_limb0: 1,
                    state_digest_limb1: 2,
                    state_digest_limb2: 3,
                    state_digest_limb3: 4,
                },
                proof: ProofBox::new("halo2/ipa".into(), vec![0xA5]),
            };
            envelope
                .validate_against_manifest(&manifest)
                .expect("real framed verifier payload binds the envelope");
            let mut framed_digest_substitution = envelope;
            framed_digest_substitution.verifier_key_sha256 = profile.artifacts[2].sha256;
            assert!(
                framed_digest_substitution
                    .validate_against_manifest(&manifest)
                    .is_err(),
                "a framed-file digest must not substitute for the raw verifier-key digest"
            );
        }
        assert!(!out_dir.join("manifest.json.tmp").exists());
        assert!(!out_dir.join("manifest.norito.tmp").exists());
        assert!(!out_dir.join("manifest.norito.sha256.tmp").exists());
        assert!(
            fs::read_dir(out_dir)
                .expect("read output directory")
                .all(|entry| !entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp"))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            assert_eq!(
                fs::metadata(out_dir).expect("output metadata").mode() & 0o777,
                0o700
            );
            for entry in fs::read_dir(out_dir).expect("read output permissions") {
                let entry = entry.expect("output entry");
                assert_eq!(
                    entry.metadata().expect("file metadata").mode() & 0o777,
                    0o600,
                    "{} must remain owner-only",
                    entry.path().display()
                );
            }
        }
    }

    #[test]
    fn identical_inputs_produce_byte_identical_bundles() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        build_bundle(&options).expect("build first bundle");
        let first = PathBuf::from(required(&options, "out-dir"));

        let mut second_options = options.clone();
        let second = temporary.path().join("bundle-second");
        second_options.insert("out-dir".to_owned(), second.to_string_lossy().into_owned());
        build_bundle(&second_options).expect("build second bundle");

        let read_bundle = |path: &Path| {
            fs::read_dir(path)
                .expect("read bundle")
                .map(|entry| {
                    let entry = entry.expect("bundle entry");
                    (
                        entry.file_name(),
                        fs::read(entry.path()).expect("read bundle file"),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(read_bundle(&first), read_bundle(&second));
    }

    #[test]
    fn duplicate_paths_payloads_and_evidence_are_rejected_before_output_creation() {
        let assert_rejected = |options: &BTreeMap<String, String>| {
            let out_dir = Path::new(required(options, "out-dir"));
            assert!(build_bundle(options).is_err());
            assert!(!out_dir.exists());
        };

        let same_path_root = tempfile::tempdir().expect("same-path root");
        let mut same_path = valid_options(same_path_root.path());
        same_path.insert(
            INPUTS[1].option.to_owned(),
            required(&same_path, INPUTS[0].option).to_owned(),
        );
        assert_rejected(&same_path);

        let same_bytes_root = tempfile::tempdir().expect("same-bytes root");
        let same_bytes = valid_options(same_bytes_root.path());
        fs::copy(
            required(&same_bytes, INPUTS[0].option),
            required(&same_bytes, INPUTS[1].option),
        )
        .expect("copy duplicate payload");
        assert_rejected(&same_bytes);

        let evidence_root = tempfile::tempdir().expect("evidence root");
        let mut evidence_alias = valid_options(evidence_root.path());
        let payload = fs::read(required(&evidence_alias, INPUTS[0].option)).expect("payload");
        evidence_alias.insert(
            "benchmark-evidence-sha256".to_owned(),
            hex::encode(Sha256::digest(payload)),
        );
        assert_rejected(&evidence_alias);
    }

    #[test]
    fn invalid_release_metadata_is_rejected_before_output_creation() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        for (field, value) in [
            ("chain-id", ""),
            ("asset-scale", "29"),
            ("generation", "../release"),
            ("parameter-generation", "NUL"),
            ("source-commit", "0000000000000000000000000000000000000000"),
            ("activation-height", "0"),
            ("withdrawal-height", "100"),
        ] {
            let mut candidate = options.clone();
            candidate.insert(field.to_owned(), value.to_owned());
            assert!(build_bundle(&candidate).is_err(), "--{field}={value}");
            assert!(!Path::new(required(&candidate, "out-dir")).exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn hardlinked_inputs_are_rejected_before_output_creation() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        let first = Path::new(required(&options, INPUTS[0].option));
        let second = Path::new(required(&options, INPUTS[1].option));
        fs::remove_file(second).expect("remove second input");
        fs::hard_link(first, second).expect("create hardlink");
        let out_dir = Path::new(required(&options, "out-dir"));
        assert!(build_bundle(&options).is_err());
        assert!(!out_dir.exists());
    }

    #[test]
    fn preflight_failure_preserves_existing_paths_and_creates_no_output() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let mut options = valid_options(temporary.path());
        let missing = temporary.path().join("missing-input");
        options.insert(
            INPUTS[0].option.to_owned(),
            missing.to_string_lossy().into_owned(),
        );
        let out_dir = PathBuf::from(required(&options, "out-dir"));
        assert!(build_bundle(&options).is_err());
        assert!(!out_dir.exists());

        fs::create_dir(&out_dir).expect("create existing output");
        fs::write(out_dir.join("sentinel"), b"preserve").expect("write sentinel");
        assert!(build_bundle(&options).is_err());
        assert_eq!(
            fs::read(out_dir.join("sentinel")).expect("sentinel"),
            b"preserve"
        );
    }

    #[test]
    fn finality_roster_rejects_chain_generation_window_and_pop_substitution() {
        fn assert_rejected(mutate: impl FnOnce(&mut KagemushaTopUpFinalityRosterArtifactV2)) {
            let temporary = tempfile::tempdir().expect("temporary directory");
            let options = valid_options(temporary.path());
            let path = Path::new(required(&options, "topup-finality-roster"));
            let bytes = fs::read(path).expect("read roster fixture");
            let mut roster: KagemushaTopUpFinalityRosterArtifactV2 =
                norito::decode_from_bytes(&bytes).expect("decode roster fixture");
            mutate(&mut roster);
            fs::write(
                path,
                norito::to_bytes(&roster).expect("encode mutated roster"),
            )
            .expect("write mutated roster");
            let out_dir = PathBuf::from(required(&options, "out-dir"));
            assert!(build_bundle(&options).is_err());
            assert!(!out_dir.exists());
        }

        assert_rejected(|roster| roster.chain_id = ChainId::from("attacker-chain"));
        assert_rejected(|roster| roster.artifact_generation = "other-generation".to_owned());
        assert_rejected(|roster| roster.windows[0].activates_at_height = 101);
        assert_rejected(|roster| roster.windows[0].withdraws_at_height = 999);
        assert_rejected(|roster| {
            let mut second = roster.windows[0].clone();
            roster.windows[0].withdraws_at_height = 500;
            second.activates_at_height = 501;
            roster.windows.push(second);
        });
        assert_rejected(|roster| roster.windows[0].validator_set_pops[0][0] ^= 1);
    }

    #[test]
    fn finality_roster_rejects_noncanonical_or_trailing_bytes() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        let roster_path = Path::new(required(&options, "topup-finality-roster"));
        let mut bytes = fs::read(roster_path).expect("read roster fixture");
        bytes.push(0);
        fs::write(roster_path, bytes).expect("write noncanonical roster");

        let out_dir = PathBuf::from(required(&options, "out-dir"));
        assert!(build_bundle(&options).is_err());
        assert!(!out_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn package_input_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        let link = temporary.path().join("link");
        let output = temporary.path().join("output");
        fs::write(&source, b"payload").expect("write source");
        symlink(&source, &link).expect("create symlink");
        fs::create_dir(&output).expect("create output");
        assert!(
            package_input(
                &link,
                &output,
                &INPUTS[0],
                "release-generation-1",
                "parameters-generation-1",
            )
            .is_err()
        );
        assert!(fs::read_dir(output).expect("read output").next().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn package_finality_roster_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let options = valid_options(temporary.path());
        let source = Path::new(required(&options, "topup-finality-roster"));
        let link = temporary.path().join("roster-link");
        symlink(source, &link).expect("create symlink");
        assert!(
            open_input(
                &link,
                KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
                "top-up finality roster",
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_regular_fifo_input_is_rejected_without_blocking() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let fifo = temporary.path().join("input.fifo");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        assert!(
            open_input(
                &fifo,
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3,
                "Pasta-cycle input",
            )
            .is_err()
        );
    }
}
