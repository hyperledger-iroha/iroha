//! Verify one PK2 bridge-finality proof against a live Sumeragi-v2 status snapshot.
//!
//! This binary is deliberately a narrow deployment boundary. It accepts only
//! the current typed JSON schemas, pins the PK2 chain, protocol, `NPoS` mode,
//! exact reporting-node identity, ordered validator keys, and count threshold,
//! and delegates the complete header/context/PoP/BLS verification to
//! `iroha_core::bridge`.

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process,
};

use iroha_core::{
    bridge::{FinalityProofVerificationConfig, verify_finality_proof},
    validate_genesis_block,
};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
use iroha_data_model::{
    ChainId, Encode as _, NetworkId,
    account::AccountId,
    block::{
        BlockHeader,
        consensus_v2::{
            BlockSubject, ConsensusMode, ExecutionCommitment, GlobalPhase, HeightContextId,
            PROTOCOL_VERSION, QuorumCertificateRef, SumeragiV2Status,
        },
        decode_framed_signed_block,
    },
    bridge::{BridgeFinalityAttestationV1, BridgeFinalityProof},
    peer::PeerId,
};
use norito::{JsonDeserialize, JsonSerialize};
use sha2::{Digest, Sha256};

const LEGACY_EXPECTATIONS_SCHEMA_VERSION: u8 = 2;
const LEGACY_RECEIPT_SCHEMA_VERSION: u8 = 3;
const ATTESTED_EXPECTATIONS_SCHEMA_VERSION: u8 = 3;
const ATTESTED_RECEIPT_SCHEMA_VERSION: u8 = 4;
const COMMIT_DECISION_ID_VERSION: u8 = 3;
const PK2_CHAIN_ID: &str = "cbdc16";
const MAX_STATUS_BYTES: u64 = 1024 * 1024;
const MAX_PROOF_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ATTESTATION_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXPECTATIONS_BYTES: u64 = 64 * 1024;
const MAX_SIGNED_GENESIS_BYTES: u64 = 256 * 1024 * 1024;
const CHALLENGE_BYTES: u64 = 32;
/// Exact sealed-source identity supplied by the artifact builder.
///
/// The explicit `black_box` use in `main` keeps this marker reachable in both
/// host and Linux release binaries without inventing source metadata locally.
const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");

const HELP: &str = "\
Verify one exact current-source PK2 bridge-finality proof.

Usage:
  pk2_bridge_finality_verify \\
    --attestation <bridge-finality-attestation.json> \\
    --signed-genesis <genesis.group.signed.nrt> \\
    --expected-roster <expected-roster.json> \\
    --challenge <64-lowercase-hex-iroha-hash>

Inherited-descriptor mode replaces each input path with its `-fd` form:
  --attestation-fd <fd> --signed-genesis-fd <fd> --expected-roster-fd <fd>
The challenge may be supplied without argv exposure as exactly 32 raw bytes
through --challenge-fd <fd>.

Legacy standalone mode remains available and is mutually exclusive:
  pk2_bridge_finality_verify \\
    --status <sumeragi-v2-status.json> \\
    --proof <bridge-finality-proof.json> \\
    --expected-roster <expected-roster.json>

The expected-roster document is strict JSON:
  {
    \"schema_version\": 3,
    \"chain_id\": \"cbdc16\",
    \"network_id\": \"<exact signed-genesis block-header hash>\",
    \"protocol_version\": 4,
    \"consensus_mode\": \"npos\",
    \"expected_node_key\": \"<this port's BLS public key>\",
    \"validator_keys\": [\"<BLS public key>\", \"...\"],
    \"min_signers\": 3,
    \"genesis_public_key\": \"<configured [genesis].public_key>\",
    \"signed_genesis_sha256\": \"<sha256 of exact sealed signed genesis bytes>\"
  }

The protocol_version example is illustrative; the binary requires the
PROTOCOL_VERSION compiled from the same current Iroha source.

On attested success, receipt schema 4 binds the reporting-node signature and
caller challenge, exact sealed signed-genesis SHA-256 and typed block hash,
height-one genesis CommitQC decision, and durable-tip CommitQC decision. Each
decision ID binds context, height, Commit phase, full block subject, and full
execution commitment while excluding the decision/re-proposal round, signers,
and aggregate representation.
";

#[derive(Debug)]
enum Cli {
    Attested {
        attestation: InputSource,
        signed_genesis: InputSource,
        expected_roster: InputSource,
        challenge: ChallengeSource,
    },
    Legacy {
        status: InputSource,
        proof: InputSource,
        expected_roster: InputSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputSource {
    Path(PathBuf),
    InheritedFd(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(variant_size_differences)]
enum ChallengeSource {
    Inline([u8; 32]),
    InheritedFd(i32),
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ExpectedRosterDocument {
    schema_version: u8,
    chain_id: ChainId,
    network_id: NetworkId,
    protocol_version: u16,
    consensus_mode: String,
    expected_node_key: String,
    validator_keys: Vec<String>,
    min_signers: u32,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AttestedExpectedRosterDocument {
    schema_version: u8,
    chain_id: ChainId,
    network_id: NetworkId,
    protocol_version: u16,
    consensus_mode: String,
    expected_node_key: String,
    validator_keys: Vec<String>,
    min_signers: u32,
    genesis_public_key: String,
    signed_genesis_sha256: String,
}

/// Stable projection of the fields which define one semantic `CommitQC` decision.
///
/// This deliberately mirrors `QuorumCertificateRef::same_commit_decision`: the
/// decision round, signer subset, and aggregate signature are absent, while
/// the complete subject and deterministic execution commitment remain bound.
#[derive(norito::Encode)]
struct SemanticCommitDecisionIdentity {
    identity_version: u8,
    context_id: HeightContextId,
    height: u64,
    phase: GlobalPhase,
    subject: BlockSubject,
    execution_commitment: ExecutionCommitment,
}

#[derive(Debug, Clone, JsonSerialize)]
struct VerificationReceipt {
    schema_version: u8,
    status: String,
    chain_id: String,
    protocol_version: u16,
    expected_node_key: String,
    node_fingerprint: String,
    height: u64,
    block_hash: String,
    height_context_id: String,
    commit_decision_id: String,
    validator_keys: Vec<String>,
    validator_powers: Vec<u64>,
    min_signers: u32,
    total_power: u64,
    signer_indices: Vec<u32>,
    signer_count: u32,
    signed_power: u64,
    proof_sha256: String,
    status_sha256: String,
}

#[derive(Debug, Clone, JsonSerialize)]
struct AttestedVerificationReceipt {
    schema_version: u8,
    status: String,
    chain_id: String,
    protocol_version: u16,
    expected_node_key: String,
    node_fingerprint: String,
    build_fingerprint: String,
    config_fingerprint: String,
    genesis_public_key: String,
    challenge: String,
    genesis_block_hash: String,
    genesis_payload_hash: String,
    genesis_executed_block_wire_hash: String,
    genesis_post_state_root: String,
    genesis_height_context_id: String,
    genesis_commit_decision_id: String,
    height: u64,
    block_hash: String,
    height_context_id: String,
    commit_decision_id: String,
    validator_keys: Vec<String>,
    validator_powers: Vec<u64>,
    min_signers: u32,
    total_power: u64,
    signer_indices: Vec<u32>,
    signer_count: u32,
    signed_power: u64,
    attestation_sha256: String,
    signed_genesis_sha256: String,
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "explicit hash suffixes distinguish three security-bound hash domains"
)]
struct ValidatedSignedGenesis {
    block_hash: HashOf<BlockHeader>,
    proposal_wire_hash: Hash,
    executed_block_wire_len: u64,
    executed_block_wire_hash: Hash,
}

fn main() {
    let _ = std::hint::black_box(BUILD_SOURCE_ID);
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("pk2 bridge finality verification failed: {error}");
            process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let cli = parse_args(env::args().skip(1))?;
    let json = run_cli(cli)?;
    println!("{json}");
    Ok(())
}

fn run_cli(cli: Cli) -> Result<String, String> {
    let json = match cli {
        Cli::Attested {
            attestation,
            signed_genesis,
            expected_roster,
            challenge,
        } => {
            let challenge = read_challenge_source(&challenge)?;
            let attestation_json =
                read_bounded_source(&attestation, MAX_ATTESTATION_BYTES, "attestation")?;
            let signed_genesis =
                read_bounded_source(&signed_genesis, MAX_SIGNED_GENESIS_BYTES, "signed genesis")?;
            let expectations_json =
                read_bounded_source(&expected_roster, MAX_EXPECTATIONS_BYTES, "expected roster")?;
            let receipt = verify_attested_json_inputs(
                &attestation_json,
                &signed_genesis,
                &expectations_json,
                challenge,
            )?;
            norito::json::to_json(&receipt)
        }
        Cli::Legacy {
            status,
            proof,
            expected_roster,
        } => {
            let status_json = read_bounded_source(&status, MAX_STATUS_BYTES, "status")?;
            let proof_json = read_bounded_source(&proof, MAX_PROOF_BYTES, "proof")?;
            let expectations_json =
                read_bounded_source(&expected_roster, MAX_EXPECTATIONS_BYTES, "expected roster")?;
            let receipt = verify_json_inputs(&status_json, &proof_json, &expectations_json)?;
            norito::json::to_json(&receipt)
        }
    }
    .map_err(|error| format!("failed to encode verification receipt: {error}"))?;
    Ok(json)
}

#[expect(
    clippy::too_many_lines,
    reason = "the mutually exclusive path/fd grammar and final descriptor-uniqueness audit form one ordered fail-closed parse"
)]
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let mut status = None;
    let mut proof = None;
    let mut attestation = None;
    let mut signed_genesis = None;
    let mut expected_roster = None;
    let mut challenge = None;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            println!("{HELP}");
            process::exit(0);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {argument}"))?;
        match argument.as_str() {
            "--status" => set_input_source(
                &mut status,
                InputSource::Path(PathBuf::from(value)),
                "status",
            )?,
            "--status-fd" => set_input_source(
                &mut status,
                InputSource::InheritedFd(parse_inherited_fd(&value, "status")?),
                "status",
            )?,
            "--proof" => {
                set_input_source(&mut proof, InputSource::Path(PathBuf::from(value)), "proof")?
            }
            "--proof-fd" => set_input_source(
                &mut proof,
                InputSource::InheritedFd(parse_inherited_fd(&value, "proof")?),
                "proof",
            )?,
            "--attestation" => set_input_source(
                &mut attestation,
                InputSource::Path(PathBuf::from(value)),
                "attestation",
            )?,
            "--attestation-fd" => set_input_source(
                &mut attestation,
                InputSource::InheritedFd(parse_inherited_fd(&value, "attestation")?),
                "attestation",
            )?,
            "--signed-genesis" => set_input_source(
                &mut signed_genesis,
                InputSource::Path(PathBuf::from(value)),
                "signed genesis",
            )?,
            "--signed-genesis-fd" => set_input_source(
                &mut signed_genesis,
                InputSource::InheritedFd(parse_inherited_fd(&value, "signed genesis")?),
                "signed genesis",
            )?,
            "--expected-roster" => set_input_source(
                &mut expected_roster,
                InputSource::Path(PathBuf::from(value)),
                "expected roster",
            )?,
            "--expected-roster-fd" => set_input_source(
                &mut expected_roster,
                InputSource::InheritedFd(parse_inherited_fd(&value, "expected roster")?),
                "expected roster",
            )?,
            "--challenge" => {
                let parsed = parse_challenge(&value)?;
                if challenge.replace(ChallengeSource::Inline(parsed)).is_some() {
                    return Err("duplicate argument --challenge".to_owned());
                }
            }
            "--challenge-fd" => {
                let parsed = ChallengeSource::InheritedFd(parse_inherited_fd(&value, "challenge")?);
                if challenge.replace(parsed).is_some() {
                    return Err(
                        "duplicate challenge input; inline and inherited-fd forms are mutually exclusive"
                            .to_owned(),
                    );
                }
            }
            _ => return Err(format!("unknown argument {argument}")),
        }
    }

    let expected_roster =
        expected_roster.ok_or_else(|| "missing required expected-roster input".to_owned())?;
    let attested_mode = attestation.is_some() || signed_genesis.is_some() || challenge.is_some();
    let cli = if attested_mode {
        if status.is_some() || proof.is_some() {
            return Err(
                "attestation mode is mutually exclusive with legacy status/proof inputs".to_owned(),
            );
        }
        Cli::Attested {
            attestation: attestation
                .ok_or_else(|| "missing required attestation input".to_owned())?,
            signed_genesis: signed_genesis
                .ok_or_else(|| "missing required signed-genesis input".to_owned())?,
            expected_roster,
            challenge: challenge.ok_or_else(|| "missing required --challenge".to_owned())?,
        }
    } else {
        Cli::Legacy {
            status: status.ok_or_else(|| "missing required status input".to_owned())?,
            proof: proof.ok_or_else(|| "missing required proof input".to_owned())?,
            expected_roster,
        }
    };
    validate_distinct_inherited_fds(&cli)?;
    Ok(cli)
}

fn set_input_source(
    slot: &mut Option<InputSource>,
    source: InputSource,
    label: &str,
) -> Result<(), String> {
    if slot.replace(source).is_some() {
        return Err(format!(
            "duplicate {label} input; path and inherited-fd forms are mutually exclusive"
        ));
    }
    Ok(())
}

fn parse_inherited_fd(value: &str, label: &str) -> Result<i32, String> {
    let fd = value
        .parse::<i32>()
        .map_err(|_| format!("{label} inherited fd must be a decimal integer"))?;
    if fd < 3 {
        return Err(format!(
            "{label} inherited fd must be >= 3; standard streams are forbidden"
        ));
    }
    Ok(fd)
}

fn parse_challenge(value: &str) -> Result<[u8; 32], String> {
    if value.len() != Hash::LENGTH * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("challenge must be exactly 64 lowercase hexadecimal characters".to_owned());
    }
    let decoded = hex::decode(value)
        .map_err(|error| format!("challenge hexadecimal decoding failed: {error}"))?;
    let challenge: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "challenge must decode to exactly 32 bytes".to_owned())?;
    if challenge.iter().all(|byte| *byte == 0) {
        return Err("challenge must be non-zero".to_owned());
    }
    Ok(challenge)
}

fn read_challenge_source(source: &ChallengeSource) -> Result<[u8; 32], String> {
    match source {
        ChallengeSource::Inline(challenge) => Ok(*challenge),
        ChallengeSource::InheritedFd(fd) => {
            let raw = read_bounded_fd(*fd, CHALLENGE_BYTES, "challenge")?;
            if u64::try_from(raw.len()).unwrap_or(u64::MAX) != CHALLENGE_BYTES {
                return Err("challenge fd must contain exactly 32 raw bytes".to_owned());
            }
            let challenge: [u8; 32] = raw
                .try_into()
                .map_err(|_| "challenge fd must contain exactly 32 raw bytes".to_owned())?;
            if challenge.iter().all(|byte| *byte == 0) {
                return Err("challenge must be non-zero".to_owned());
            }
            Ok(challenge)
        }
    }
}

fn validate_distinct_inherited_fds(cli: &Cli) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let sources: Vec<&InputSource> = match cli {
        Cli::Attested {
            attestation,
            signed_genesis,
            expected_roster,
            challenge,
        } => {
            let sources = vec![attestation, signed_genesis, expected_roster];
            if let ChallengeSource::InheritedFd(fd) = challenge
                && !seen.insert(*fd)
            {
                return Err(format!(
                    "inherited fd {fd} is reused for multiple verifier inputs"
                ));
            }
            sources
        }
        Cli::Legacy {
            status,
            proof,
            expected_roster,
        } => vec![status, proof, expected_roster],
    };
    for source in sources {
        if let InputSource::InheritedFd(fd) = source
            && !seen.insert(*fd)
        {
            return Err(format!(
                "inherited fd {fd} is reused for multiple verifier inputs"
            ));
        }
    }
    Ok(())
}

fn read_bounded_source(
    source: &InputSource,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, String> {
    match source {
        InputSource::Path(path) => read_bounded(path, max_bytes, label),
        InputSource::InheritedFd(fd) => read_bounded_fd(*fd, max_bytes, label),
    }
}

#[cfg(unix)]
fn read_bounded_fd(fd: i32, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    if fd < 3 {
        return Err(format!(
            "{label} inherited fd must be >= 3; standard streams are forbidden"
        ));
    }
    // Opening the process-local descriptor alias duplicates the descriptor
    // without re-resolving an attacker-controlled filesystem pathname. All
    // custody checks and reads below operate on that one duplicated handle.
    #[cfg(target_os = "linux")]
    let aliases = [format!("/proc/self/fd/{fd}"), format!("/dev/fd/{fd}")];
    #[cfg(not(target_os = "linux"))]
    let aliases = [format!("/dev/fd/{fd}"), format!("/proc/self/fd/{fd}")];
    let mut last_error = None;
    let mut file = None;
    for alias in &aliases {
        match File::open(alias) {
            Ok(opened) => {
                file = Some(opened);
                break;
            }
            Err(error) => last_error = Some((alias, error)),
        }
    }
    let mut file = file.ok_or_else(|| {
        let detail = last_error.map_or_else(
            || "no process fd alias is available".to_owned(),
            |(alias, error)| format!("{alias}: {error}"),
        );
        format!("failed to duplicate {label} inherited fd {fd}: {detail}")
    })?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to fstat {label} inherited fd {fd}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!(
            "{label} inherited fd {fd} does not refer to a regular file"
        ));
    }
    #[cfg(unix)]
    if metadata.nlink() > 1 {
        return Err(format!(
            "{label} inherited fd {fd} has invalid link custody"
        ));
    }
    if metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(format!(
            "{label} inherited fd {fd} has invalid size {} bytes (expected 1..={max_bytes})",
            metadata.len()
        ));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind {label} inherited fd {fd}: {error}"))?;
    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {label} inherited fd {fd}: {error}"))?;
    let metadata_after = file.metadata().map_err(|error| {
        format!("failed to re-fstat {label} inherited fd {fd} after reading: {error}")
    })?;
    let bytes_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut changed = bytes_len != metadata.len() || metadata_after.len() != metadata.len();
    changed =
        changed || (metadata_after.dev(), metadata_after.ino()) != (metadata.dev(), metadata.ino());
    if bytes.is_empty() || bytes_len > max_bytes || changed {
        return Err(format!(
            "{label} inherited fd {fd} changed or had invalid size {} bytes while being read",
            bytes.len()
        ));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_bounded_fd(fd: i32, _max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    Err(format!(
        "{label} inherited fd {fd} is unsupported on this platform"
    ))
}

fn read_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    let lexical_before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if lexical_before.file_type().is_symlink() {
        return Err(format!("{label} {} must not be a symlink", path.display()));
    }
    let file = File::open(path)
        .map_err(|error| format!("failed to open {label} {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{label} {} is not a regular file", path.display()));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1
        || (metadata.dev(), metadata.ino()) != (lexical_before.dev(), lexical_before.ino())
    {
        return Err(format!("{label} {} has invalid custody", path.display()));
    }
    if metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(format!(
            "{label} {} has invalid size {} bytes (expected 1..={max_bytes})",
            path.display(),
            metadata.len()
        ));
    }
    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    let mut file = file;
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    let metadata_after = file
        .metadata()
        .map_err(|error| format!("failed to re-inspect {label} {}: {error}", path.display()))?;
    let lexical_after = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to re-inspect {label} {} after reading: {error}",
            path.display()
        )
    })?;
    let bytes_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let changed = bytes_len != metadata.len()
        || metadata_after.len() != metadata.len()
        || lexical_after.file_type().is_symlink();
    #[cfg(unix)]
    let changed = changed
        || (metadata_after.dev(), metadata_after.ino()) != (metadata.dev(), metadata.ino())
        || (lexical_after.dev(), lexical_after.ino()) != (metadata.dev(), metadata.ino());
    if bytes.is_empty() || bytes_len > max_bytes || changed {
        return Err(format!(
            "{label} {} changed or had invalid size {} bytes while being read",
            path.display(),
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn verify_json_inputs(
    status_json: &[u8],
    proof_json: &[u8],
    expectations_json: &[u8],
) -> Result<VerificationReceipt, String> {
    let status_text = std::str::from_utf8(status_json)
        .map_err(|error| format!("status is not UTF-8 JSON: {error}"))?;
    let proof_text = std::str::from_utf8(proof_json)
        .map_err(|error| format!("proof is not UTF-8 JSON: {error}"))?;
    let expectations_text = std::str::from_utf8(expectations_json)
        .map_err(|error| format!("expected roster is not UTF-8 JSON: {error}"))?;

    let status = norito::json::from_json::<SumeragiV2Status>(status_text)
        .map_err(|error| format!("invalid current SumeragiV2Status JSON: {error}"))?;
    let proof = norito::json::from_json::<BridgeFinalityProof>(proof_text)
        .map_err(|error| format!("invalid current BridgeFinalityProof JSON: {error}"))?;
    let expectations = norito::json::from_json::<ExpectedRosterDocument>(expectations_text)
        .map_err(|error| format!("invalid expected-roster JSON: {error}"))?;

    verify_decoded(
        &status,
        &proof,
        &expectations,
        sha256_hex(status_json),
        sha256_hex(proof_json),
    )
}

fn verify_attested_json_inputs(
    attestation_json: &[u8],
    signed_genesis: &[u8],
    expectations_json: &[u8],
    challenge: [u8; 32],
) -> Result<AttestedVerificationReceipt, String> {
    let attestation_text = std::str::from_utf8(attestation_json)
        .map_err(|error| format!("attestation is not UTF-8 JSON: {error}"))?;
    let expectations_text = std::str::from_utf8(expectations_json)
        .map_err(|error| format!("expected roster is not UTF-8 JSON: {error}"))?;
    let attestation = norito::json::from_json::<BridgeFinalityAttestationV1>(attestation_text)
        .map_err(|error| format!("invalid current BridgeFinalityAttestationV1 JSON: {error}"))?;
    let expectations = norito::json::from_json::<AttestedExpectedRosterDocument>(expectations_text)
        .map_err(|error| format!("invalid attested expected-roster JSON: {error}"))?;

    let legacy_expectations = validate_attested_expectations(&expectations)?;
    attestation
        .verify()
        .map_err(|error| format!("reporting-node finality attestation is invalid: {error}"))?;
    if attestation.body.challenge != challenge {
        return Err("attestation does not bind the exact caller challenge".to_owned());
    }
    if attestation.body.network_id != expectations.network_id {
        return Err("attestation network id does not match expected-roster network id".to_owned());
    }
    let expected_node =
        parse_expected_peer_key(&expectations.expected_node_key, "expected_node_key")?;
    if attestation.body.node_id != expected_node {
        return Err("attestation node_id does not match expected_node_key".to_owned());
    }

    let actual_signed_genesis_sha256 = sha256_hex(signed_genesis);
    if actual_signed_genesis_sha256 != expectations.signed_genesis_sha256 {
        return Err(format!(
            "signed genesis SHA-256 {} does not match expected {}",
            actual_signed_genesis_sha256, expectations.signed_genesis_sha256
        ));
    }
    let genesis_public_key = parse_expected_genesis_public_key(&expectations.genesis_public_key)?;
    let genesis = decode_validate_signed_genesis(signed_genesis, &genesis_public_key)?;
    let decoded_network_id = NetworkId::from_genesis_hash(genesis.block_hash);
    if expectations.network_id != decoded_network_id {
        return Err(format!(
            "expected-roster network id {} does not match decoded signed genesis {}",
            expectations.network_id, decoded_network_id
        ));
    }
    if attestation.body.genesis_block_hash != genesis.block_hash {
        return Err(format!(
            "attested genesis block hash {} does not match decoded signed genesis {}",
            hex::encode(attestation.body.genesis_block_hash.as_ref()),
            hex::encode(genesis.block_hash.as_ref())
        ));
    }

    let (genesis_height_context_id, genesis_commit_decision_id) = verify_genesis_finality_proof(
        &attestation.body.genesis_finality_proof,
        &legacy_expectations,
        &genesis,
    )?;
    let tip = verify_decoded(
        &attestation.body.status,
        &attestation.body.finality_proof,
        &legacy_expectations,
        String::new(),
        String::new(),
    )?;
    let genesis_execution = &attestation
        .body
        .genesis_finality_proof
        .finality_artifact
        .commit_qc
        .execution_commitment;

    Ok(AttestedVerificationReceipt {
        schema_version: ATTESTED_RECEIPT_SCHEMA_VERSION,
        status: "validated".to_owned(),
        chain_id: tip.chain_id,
        protocol_version: tip.protocol_version,
        expected_node_key: tip.expected_node_key,
        node_fingerprint: tip.node_fingerprint,
        build_fingerprint: hex::encode(attestation.body.status.build_fingerprint.as_ref()),
        config_fingerprint: hex::encode(attestation.body.status.config_fingerprint.as_ref()),
        genesis_public_key: genesis_public_key.to_string(),
        challenge: hex::encode(challenge),
        genesis_block_hash: hex::encode(genesis.block_hash.as_ref()),
        genesis_payload_hash: hex::encode(genesis.proposal_wire_hash.as_ref()),
        genesis_executed_block_wire_hash: hex::encode(genesis.executed_block_wire_hash.as_ref()),
        genesis_post_state_root: hex::encode(genesis_execution.post_state_root.as_ref()),
        genesis_height_context_id,
        genesis_commit_decision_id,
        height: tip.height,
        block_hash: tip.block_hash,
        height_context_id: tip.height_context_id,
        commit_decision_id: tip.commit_decision_id,
        validator_keys: tip.validator_keys,
        validator_powers: tip.validator_powers,
        min_signers: tip.min_signers,
        total_power: tip.total_power,
        signer_indices: tip.signer_indices,
        signer_count: tip.signer_count,
        signed_power: tip.signed_power,
        attestation_sha256: sha256_hex(attestation_json),
        signed_genesis_sha256: actual_signed_genesis_sha256,
    })
}

fn validate_attested_expectations(
    expectations: &AttestedExpectedRosterDocument,
) -> Result<ExpectedRosterDocument, String> {
    if expectations.schema_version != ATTESTED_EXPECTATIONS_SCHEMA_VERSION {
        return Err(format!(
            "attested expected-roster schema_version {} is unsupported; expected {ATTESTED_EXPECTATIONS_SCHEMA_VERSION}",
            expectations.schema_version
        ));
    }
    validate_sha256_literal(&expectations.signed_genesis_sha256, "signed_genesis_sha256")?;
    parse_expected_genesis_public_key(&expectations.genesis_public_key)?;
    let legacy = ExpectedRosterDocument {
        schema_version: LEGACY_EXPECTATIONS_SCHEMA_VERSION,
        chain_id: expectations.chain_id.clone(),
        network_id: expectations.network_id,
        protocol_version: expectations.protocol_version,
        consensus_mode: expectations.consensus_mode.clone(),
        expected_node_key: expectations.expected_node_key.clone(),
        validator_keys: expectations.validator_keys.clone(),
        min_signers: expectations.min_signers,
    };
    validate_expectations(&legacy)?;
    Ok(legacy)
}

fn validate_sha256_literal(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(format!(
            "{label} must be a non-zero 64-character lowercase SHA-256 hex digest"
        ));
    }
    Ok(())
}

fn parse_expected_genesis_public_key(value: &str) -> Result<PublicKey, String> {
    let public_key = value
        .parse::<PublicKey>()
        .map_err(|error| format!("genesis_public_key is invalid: {error}"))?;
    if public_key.to_string() != value {
        return Err("genesis_public_key is not in canonical PublicKey form".to_owned());
    }
    Ok(public_key)
}

fn decode_validate_signed_genesis(
    signed_genesis: &[u8],
    genesis_public_key: &PublicKey,
) -> Result<ValidatedSignedGenesis, String> {
    iroha_genesis::init_instruction_registry();
    let block = decode_framed_signed_block(signed_genesis)
        .map_err(|error| format!("signed genesis is not a current framed SignedBlock: {error}"))?;
    let canonical = block
        .encode_wire()
        .map_err(|error| format!("failed to re-encode decoded signed genesis: {error}"))?;
    if canonical != signed_genesis {
        return Err("signed genesis is not in exact canonical framed encoding".to_owned());
    }
    let genesis_account = AccountId::new(genesis_public_key.clone());
    validate_genesis_block(&block, &genesis_account)
        .map_err(|error| format!("signed genesis validation failed: {error}"))?;
    let proposal_wire_hash = block
        .canonical_proposal_wire_hash()
        .map_err(|error| format!("failed to hash signed genesis proposal wire: {error}"))?;
    let executed_block_wire_len = u64::try_from(canonical.len())
        .map_err(|_| "signed genesis executed wire length does not fit u64".to_owned())?;
    let executed_block_wire_hash = Hash::new(&canonical);
    Ok(ValidatedSignedGenesis {
        block_hash: block.hash(),
        proposal_wire_hash,
        executed_block_wire_len,
        executed_block_wire_hash,
    })
}

fn verify_genesis_finality_proof(
    proof: &BridgeFinalityProof,
    expectations: &ExpectedRosterDocument,
    genesis: &ValidatedSignedGenesis,
) -> Result<(String, String), String> {
    let artifact = &proof.finality_artifact;
    if artifact.height != 1 || proof.block_header.height().get() != 1 {
        return Err("genesis finality proof is not for height one".to_owned());
    }
    if artifact.block_hash != genesis.block_hash || proof.block_header.hash() != genesis.block_hash
    {
        return Err(
            "genesis finality proof does not authenticate decoded signed genesis".to_owned(),
        );
    }
    if artifact.subject.payload_hash != genesis.proposal_wire_hash {
        return Err(
            "genesis finality proof payload hash does not match signed genesis proposal wire"
                .to_owned(),
        );
    }
    if artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_len
        != genesis.executed_block_wire_len
        || artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash
            != genesis.executed_block_wire_hash
    {
        return Err(
            "genesis finality proof execution commitment does not match signed genesis executed wire"
                .to_owned(),
        );
    }
    if artifact.protocol_version != PROTOCOL_VERSION
        || artifact.height_context.protocol_version != PROTOCOL_VERSION
    {
        return Err(format!(
            "genesis finality proof does not use current protocol version {PROTOCOL_VERSION}"
        ));
    }
    if artifact.height_context.network_id != expectations.network_id
        || artifact.height_context.mode != ConsensusMode::Npos
    {
        return Err("genesis finality proof is not for the expected PK2 NPoS network".to_owned());
    }
    let expected_peers = parse_expected_validator_keys(&expectations.validator_keys)?;
    let actual_peers = artifact
        .height_context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    if actual_peers != expected_peers {
        return Err(
            "genesis finality proof validator roster does not exactly match expected validator key order/set"
                .to_owned(),
        );
    }
    if artifact.height_context.quorum.min_signers != expectations.min_signers {
        return Err("genesis finality proof min_signers does not match expectations".to_owned());
    }
    let verification = FinalityProofVerificationConfig {
        expected_network_id: &expectations.network_id,
        expected_height: Some(1),
        trusted_context_id: artifact.context_id(),
    };
    verify_finality_proof(proof, &verification)
        .map_err(|error| format!("cryptographic genesis finality verification failed: {error}"))?;
    Ok((
        hex::encode((artifact.context_id().0).as_ref()),
        semantic_commit_decision_id(&artifact.commit_qc.as_ref())?,
    ))
}

#[expect(
    clippy::too_many_lines,
    reason = "status binding, proof audit, quorum accounting, and cryptographic verification form one ordered fail-closed flow"
)]
fn verify_decoded(
    status: &SumeragiV2Status,
    proof: &BridgeFinalityProof,
    expectations: &ExpectedRosterDocument,
    status_sha256: String,
    proof_sha256: String,
) -> Result<VerificationReceipt, String> {
    validate_expectations(expectations)?;
    status
        .validate()
        .map_err(|error| format!("SumeragiV2Status structural validation failed: {error}"))?;
    if status.restart_required {
        return Err("SumeragiV2Status reports restart_required=true".to_owned());
    }
    if status.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "status protocol version {} is not current protocol version {PROTOCOL_VERSION}",
            status.protocol_version
        ));
    }
    let expected_build_fingerprint = compiled_build_fingerprint();
    if status.build_fingerprint != expected_build_fingerprint {
        return Err(format!(
            "status build_fingerprint {} does not match this current-source verifier build {}",
            hex::encode(status.build_fingerprint.as_ref()),
            hex::encode(expected_build_fingerprint.as_ref())
        ));
    }
    if status
        .config_fingerprint
        .as_ref()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("status config_fingerprint must be non-zero".to_owned());
    }
    if status.height_context.mode != ConsensusMode::Npos {
        return Err("status consensus mode is not current NPoS".to_owned());
    }
    let expected_node =
        parse_expected_peer_key(&expectations.expected_node_key, "expected_node_key")?;
    let expected_node_fingerprint = Hash::new(expected_node.encode());
    if status.node_fingerprint != expected_node_fingerprint {
        return Err(format!(
            "status node_fingerprint {} does not match expected_node_key {} fingerprint {}",
            hex::encode(status.node_fingerprint.as_ref()),
            expectations.expected_node_key,
            hex::encode(expected_node_fingerprint.as_ref()),
        ));
    }

    let artifact = &proof.finality_artifact;
    if artifact.height_context.network_id != expectations.network_id {
        return Err(format!(
            "proof network id {} does not match expected network {}",
            artifact.height_context.network_id, expectations.network_id
        ));
    }
    if artifact.protocol_version != PROTOCOL_VERSION
        || artifact.height_context.protocol_version != PROTOCOL_VERSION
    {
        return Err(format!(
            "proof does not use current protocol version {PROTOCOL_VERSION}"
        ));
    }
    if artifact.height_context.mode != ConsensusMode::Npos {
        return Err("proof consensus mode is not current NPoS".to_owned());
    }

    if status.last_committed_height != artifact.height {
        return Err(format!(
            "status last_committed_height {} does not match proof height {}",
            status.last_committed_height, artifact.height
        ));
    }
    let status_subject = status.last_committed_subject.as_ref().ok_or_else(|| {
        "status has no authenticated last_committed_subject; CommitQC is required at every height"
            .to_owned()
    })?;
    if status_subject != &artifact.subject {
        return Err("status last_committed_subject does not match proof subject".to_owned());
    }
    let status_commit = status.last_commit_qc.as_ref().ok_or_else(|| {
        "status has no authenticated last_commit_qc; CommitQC is required at every height"
            .to_owned()
    })?;
    if status_commit.certificate != artifact.commit_qc.as_ref() {
        return Err("status last_commit_qc certificate does not match proof CommitQC".to_owned());
    }

    let expected_peers = parse_expected_validator_keys(&expectations.validator_keys)?;
    let actual_peers = artifact
        .height_context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    if actual_peers != expected_peers {
        return Err(
            "proof validator roster does not exactly match expected validator key order/set"
                .to_owned(),
        );
    }

    let validator_count = u32::try_from(artifact.height_context.roster.len())
        .map_err(|_| "proof validator roster is too large".to_owned())?;
    if artifact.height_context.quorum.min_signers != expectations.min_signers {
        return Err(format!(
            "proof min_signers {} does not match expected {}",
            artifact.height_context.quorum.min_signers, expectations.min_signers
        ));
    }

    let mut total_power = 0_u64;
    let mut validator_keys = Vec::with_capacity(artifact.height_context.roster.len());
    let mut validator_powers = Vec::with_capacity(artifact.height_context.roster.len());
    for entry in &artifact.height_context.roster {
        if entry.power != 1 {
            return Err(format!(
                "validator {} does not have the required unit consensus vote",
                entry.validator
            ));
        }
        total_power = total_power
            .checked_add(entry.power)
            .ok_or_else(|| "validator total power overflows u64".to_owned())?;
        validator_keys.push(entry.validator.to_string());
        validator_powers.push(entry.power);
    }
    if artifact.height_context.quorum.total_power != total_power {
        return Err(format!(
            "proof quorum total_power {} does not match roster sum {total_power}",
            artifact.height_context.quorum.total_power
        ));
    }

    let signer_count = u32::try_from(artifact.commit_qc.signers.len())
        .map_err(|_| "proof signer list is too large".to_owned())?;
    let mut signed_power = 0_u64;
    for index in &artifact.commit_qc.signers {
        let position = usize::try_from(*index)
            .map_err(|_| format!("proof signer index {index} is not representable"))?;
        let entry = artifact
            .height_context
            .roster
            .get(position)
            .ok_or_else(|| format!("proof signer index {index} is outside the roster"))?;
        signed_power = signed_power
            .checked_add(entry.power)
            .ok_or_else(|| "proof signed power overflows u64".to_owned())?;
    }
    if signer_count < expectations.min_signers {
        return Err(format!(
            "proof signer count {signer_count} is below expected min_signers {}",
            expectations.min_signers
        ));
    }
    if u128::from(signed_power) * 3 <= u128::from(total_power) * 2 {
        return Err(format!(
            "proof signed power {signed_power} is not a strict two-thirds supermajority of {total_power}"
        ));
    }

    if status_commit.validator_count != validator_count
        || status_commit.signer_count != signer_count
        || status_commit.min_signers != expectations.min_signers
        || status_commit.signed_power != signed_power
        || status_commit.total_power != total_power
    {
        return Err(format!(
            "status CommitQC quorum summary does not exactly match proof: \
             status=validators:{}/signers:{}/min:{}/signed_power:{}/total_power:{} \
             proof=validators:{validator_count}/signers:{signer_count}/min:{}/signed_power:{signed_power}/total_power:{total_power}",
            status_commit.validator_count,
            status_commit.signer_count,
            status_commit.min_signers,
            status_commit.signed_power,
            status_commit.total_power,
            expectations.min_signers,
        ));
    }

    let trusted_context_id = status_commit.certificate.round.context_id;
    if artifact.context_id() != trusted_context_id {
        return Err(
            "proof height context id does not match the context authenticated by status last_commit_qc"
                .to_owned(),
        );
    }
    let verification = FinalityProofVerificationConfig {
        expected_network_id: &expectations.network_id,
        expected_height: Some(status.last_committed_height),
        trusted_context_id,
    };
    verify_finality_proof(proof, &verification)
        .map_err(|error| format!("cryptographic bridge finality verification failed: {error}"))?;
    let commit_decision_id = semantic_commit_decision_id(&artifact.commit_qc.as_ref())?;

    Ok(VerificationReceipt {
        schema_version: LEGACY_RECEIPT_SCHEMA_VERSION,
        status: "validated".to_owned(),
        chain_id: PK2_CHAIN_ID.to_owned(),
        protocol_version: PROTOCOL_VERSION,
        expected_node_key: expected_node.to_string(),
        node_fingerprint: hex::encode(status.node_fingerprint.as_ref()),
        height: artifact.height,
        block_hash: hex::encode(artifact.block_hash.as_ref()),
        height_context_id: hex::encode((artifact.context_id().0).as_ref()),
        commit_decision_id,
        validator_keys,
        validator_powers,
        min_signers: expectations.min_signers,
        total_power,
        signer_indices: artifact.commit_qc.signers.clone(),
        signer_count,
        signed_power,
        proof_sha256,
        status_sha256,
    })
}

fn validate_expectations(expectations: &ExpectedRosterDocument) -> Result<(), String> {
    if expectations.schema_version != LEGACY_EXPECTATIONS_SCHEMA_VERSION {
        return Err(format!(
            "expected-roster schema_version {} is unsupported; expected {LEGACY_EXPECTATIONS_SCHEMA_VERSION}",
            expectations.schema_version
        ));
    }
    if expectations.chain_id.as_str() != PK2_CHAIN_ID {
        return Err(format!(
            "expected-roster chain_id {} is not required PK2 chain {PK2_CHAIN_ID}",
            expectations.chain_id
        ));
    }
    if expectations.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "expected-roster protocol_version {} is not compiled current version {PROTOCOL_VERSION}",
            expectations.protocol_version
        ));
    }
    if expectations.consensus_mode != "npos" {
        return Err("expected-roster consensus_mode must be exactly `npos`".to_owned());
    }
    let count = u32::try_from(expectations.validator_keys.len())
        .map_err(|_| "expected validator key list is too large".to_owned())?;
    let canonical_min_signers = (count != 0).then(|| u64::from(count) * 2 / 3 + 1);
    if canonical_min_signers != Some(u64::from(expectations.min_signers)) {
        return Err(format!(
            "expected min_signers {} is not the canonical strict two-thirds count threshold for {count} validators",
            expectations.min_signers
        ));
    }
    let validators = parse_expected_validator_keys(&expectations.validator_keys)?;
    let expected_node =
        parse_expected_peer_key(&expectations.expected_node_key, "expected_node_key")?;
    if !validators.contains(&expected_node) {
        return Err("expected_node_key is not present in validator_keys".to_owned());
    }
    Ok(())
}

fn parse_expected_validator_keys(keys: &[String]) -> Result<Vec<PeerId>, String> {
    let mut peers = Vec::with_capacity(keys.len());
    let mut unique = BTreeSet::new();
    for (index, key) in keys.iter().enumerate() {
        let peer = parse_expected_peer_key(key, &format!("expected validator key {index}"))?;
        if !unique.insert(peer.clone()) {
            return Err(format!("expected validator key {index} is duplicated"));
        }
        peers.push(peer);
    }
    Ok(peers)
}

fn parse_expected_peer_key(key: &str, label: &str) -> Result<PeerId, String> {
    let peer = key
        .parse::<PeerId>()
        .map_err(|error| format!("{label} is invalid: {error}"))?;
    let algorithm = peer
        .public_key()
        .try_algorithm()
        .map_err(|error| format!("{label} is invalid: {error}"))?;
    if algorithm != Algorithm::BlsNormal {
        return Err(format!(
            "{label} uses {algorithm:?}; current finality requires BlsNormal"
        ));
    }
    if peer.to_string() != key {
        return Err(format!("{label} is not in canonical PeerId form"));
    }
    Ok(peer)
}

fn semantic_commit_decision_id(certificate: &QuorumCertificateRef) -> Result<String, String> {
    if certificate.phase != GlobalPhase::Commit {
        return Err("semantic commit decision identity requires a CommitQC".to_owned());
    }
    let identity = SemanticCommitDecisionIdentity {
        identity_version: COMMIT_DECISION_ID_VERSION,
        context_id: certificate.round.context_id,
        height: certificate.round.height,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
    };
    Ok(hex::encode(Hash::new(identity.encode()).as_ref()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn compiled_build_fingerprint() -> Hash {
    let mut preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    preimage.extend_from_slice(
        option_env!("GIT_COMMIT_HASH")
            .unwrap_or("unknown")
            .as_bytes(),
    );
    Hash::new(preimage)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{io::Write as _, os::fd::AsRawFd as _};

    use iroha_crypto::{Hash, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        account::AccountId,
        block::{
            SignedBlock,
            consensus_v2::{
                BlockSubject, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding,
                QuorumCertificate, SumeragiV2BodyState, SumeragiV2CommitQcStatus,
                SumeragiV2HeightContextStatus, SumeragiV2LivenessStatus, SumeragiV2StatusPhase,
                ValidatorPower, finality::V2FinalityArtifact,
            },
        },
        bridge::{
            BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2,
            BridgeFinalityAttestationBodyV1,
        },
        transaction::signed::TransactionBuilder,
    };

    use super::*;

    struct Fixture {
        status: SumeragiV2Status,
        proof: BridgeFinalityProof,
        expectations: ExpectedRosterDocument,
        attestation: BridgeFinalityAttestationV1,
        node_signer: KeyPair,
        genesis_public_key: PublicKey,
        signed_genesis: Vec<u8>,
        challenge: [u8; 32],
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture constructs one cryptographically linked genesis, context, quorum certificate, status, proof, and attestation"
    )]
    fn fixture() -> Fixture {
        let mut keys = (0..4)
            .map(|_| {
                KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("generate checked BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let powers = [1, 1, 1, 1];
        let roster = keys
            .iter()
            .zip(powers)
            .map(|(key, power)| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power,
            })
            .collect::<Vec<_>>();
        let quorum = DualQuorum::from_roster(&roster).expect("canonical quorum");
        let genesis_signer = KeyPair::try_random().expect("generate genesis signer");
        let genesis_public_key = genesis_signer.public_key().clone();
        let genesis_account = AccountId::new(genesis_public_key.clone());
        let genesis_transaction = TransactionBuilder::new_genesis(
            genesis_account,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(genesis_signer.private_key());
        let genesis_block = SignedBlock::genesis(
            vec![genesis_transaction],
            genesis_signer.private_key(),
            None,
            None,
        );
        let header = genesis_block.header();
        let network_id = NetworkId::from_genesis_hash(header.hash());
        let genesis_payload_hash = genesis_block
            .canonical_proposal_wire_hash()
            .expect("genesis proposal wire hash");
        let signed_genesis = genesis_block.encode_wire().expect("signed genesis wire");
        let genesis_executed_wire_len =
            u64::try_from(signed_genesis.len()).expect("genesis wire length fits u64");
        let genesis_executed_wire_hash = Hash::new(&signed_genesis);
        let context = HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            roster,
            quorum,
            nexus_amx_context_hash: Hash::new(b"pk2 finality verifier test nexus context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: header.hash(),
            payload_hash: genesis_payload_hash,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"parent state"),
            Hash::new(b"post state"),
            Hash::new(b"ordinary writes"),
            genesis_executed_wire_len,
            genesis_executed_wire_hash,
        );
        let round = ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        };
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = commit_qc
            .signer_preimage(&context, 0)
            .expect("canonical vote preimage");
        let signatures = commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keys[usize::try_from(*index).expect("signer index")].private_key(),
                    &preimage,
                )
                .expect("sign vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate signatures");
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let artifact = V2FinalityArtifact::new(context.clone(), subject, commit_qc.clone(), pops);
        let signed_power = commit_qc
            .signers
            .iter()
            .map(|index| context.roster[usize::try_from(*index).expect("signer index")].power)
            .sum();
        let expected_node = context.roster[0].validator.clone();
        let status = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(expected_node.encode()),
            build_fingerprint: compiled_build_fingerprint(),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: context.id(),
            height: 1,
            view: 0,
            phase: SumeragiV2StatusPhase::PendingApply,
            leader: context.leader(0),
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Applied,
            pending_persistence_id: None,
            last_committed_height: 1,
            last_committed_subject: Some(subject),
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                quorum: context.quorum,
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate: commit_qc.as_ref(),
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                signer_count: u32::try_from(commit_qc.signers.len()).expect("signer count"),
                min_signers: context.quorum.min_signers,
                signed_power,
                total_power: context.quorum.total_power,
            }),
            liveness: SumeragiV2LivenessStatus::default(),
        };
        let expectations = ExpectedRosterDocument {
            schema_version: LEGACY_EXPECTATIONS_SCHEMA_VERSION,
            chain_id: PK2_CHAIN_ID.into(),
            network_id,
            protocol_version: PROTOCOL_VERSION,
            consensus_mode: "npos".to_owned(),
            expected_node_key: expected_node.to_string(),
            validator_keys: context
                .roster
                .iter()
                .map(|entry| entry.validator.to_string())
                .collect(),
            min_signers: context.quorum.min_signers,
        };
        let proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: header,
            finality_artifact: artifact,
        };
        let challenge = [0xA5; 32];
        let node_signer = keys[0].clone();
        let body = BridgeFinalityAttestationBodyV1 {
            version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
            challenge,
            network_id,
            node_id: expected_node,
            node_fingerprint: status.node_fingerprint,
            genesis_block_hash: proof.block_header.hash(),
            genesis_finality_proof: proof.clone(),
            status: status.clone(),
            finality_proof: proof.clone(),
        };
        let signature = SignatureOf::try_from_hash(node_signer.private_key(), body.signing_hash())
            .expect("sign node attestation");
        let attestation = BridgeFinalityAttestationV1 { body, signature };
        Fixture {
            status,
            proof,
            expectations,
            attestation,
            node_signer,
            genesis_public_key,
            signed_genesis,
            challenge,
        }
    }

    fn verify_fixture(fixture: &Fixture) -> Result<VerificationReceipt, String> {
        let status = norito::json::to_json(&fixture.status).expect("encode status");
        let proof = norito::json::to_json(&fixture.proof).expect("encode proof");
        let expectations =
            norito::json::to_json(&fixture.expectations).expect("encode expectations");
        verify_json_inputs(status.as_bytes(), proof.as_bytes(), expectations.as_bytes())
    }

    fn attested_expectations(fixture: &Fixture) -> AttestedExpectedRosterDocument {
        AttestedExpectedRosterDocument {
            schema_version: ATTESTED_EXPECTATIONS_SCHEMA_VERSION,
            chain_id: fixture.expectations.chain_id.clone(),
            network_id: fixture.expectations.network_id,
            protocol_version: fixture.expectations.protocol_version,
            consensus_mode: fixture.expectations.consensus_mode.clone(),
            expected_node_key: fixture.expectations.expected_node_key.clone(),
            validator_keys: fixture.expectations.validator_keys.clone(),
            min_signers: fixture.expectations.min_signers,
            genesis_public_key: fixture.genesis_public_key.to_string(),
            signed_genesis_sha256: sha256_hex(&fixture.signed_genesis),
        }
    }

    fn verify_attested_fixture(
        fixture: &Fixture,
        expectations: &AttestedExpectedRosterDocument,
        challenge: [u8; 32],
    ) -> Result<AttestedVerificationReceipt, String> {
        let attestation = norito::json::to_json(&fixture.attestation).expect("encode attestation");
        let expectations =
            norito::json::to_json(expectations).expect("encode attested expectations");
        verify_attested_json_inputs(
            attestation.as_bytes(),
            &fixture.signed_genesis,
            expectations.as_bytes(),
            challenge,
        )
    }

    fn resign_attestation(fixture: &mut Fixture) {
        fixture.attestation.signature = SignatureOf::try_from_hash(
            fixture.node_signer.private_key(),
            fixture.attestation.body.signing_hash(),
        )
        .expect("re-sign adversarial attestation");
    }

    fn mirror_height_one_genesis_proof_into_tip(fixture: &mut Fixture) {
        fixture.attestation.body.finality_proof =
            fixture.attestation.body.genesis_finality_proof.clone();
        let artifact = &fixture.attestation.body.finality_proof.finality_artifact;
        fixture.attestation.body.status.last_committed_subject = Some(artifact.subject);
        fixture
            .attestation
            .body
            .status
            .last_commit_qc
            .as_mut()
            .expect("height-one commit status")
            .certificate = artifact.commit_qc.as_ref();
    }

    #[test]
    fn valid_attested_mode_binds_signed_genesis_and_both_commit_decisions() {
        let fixture = fixture();
        let expectations = attested_expectations(&fixture);
        let receipt = verify_attested_fixture(&fixture, &expectations, fixture.challenge)
            .expect("verify attested proof");

        assert_eq!(receipt.schema_version, ATTESTED_RECEIPT_SCHEMA_VERSION);
        assert_eq!(receipt.status, "validated");
        assert_eq!(receipt.challenge, hex::encode(fixture.challenge));
        assert_eq!(
            receipt.genesis_public_key,
            fixture.genesis_public_key.to_string()
        );
        assert_eq!(
            receipt.signed_genesis_sha256,
            sha256_hex(&fixture.signed_genesis)
        );
        assert_eq!(
            receipt.genesis_block_hash,
            hex::encode(fixture.proof.block_header.hash().as_ref())
        );
        assert_eq!(
            receipt.genesis_payload_hash,
            hex::encode(
                fixture
                    .proof
                    .finality_artifact
                    .subject
                    .payload_hash
                    .as_ref()
            )
        );
        assert_eq!(
            receipt.genesis_executed_block_wire_hash,
            hex::encode(
                fixture
                    .proof
                    .finality_artifact
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_hash
                    .as_ref()
            )
        );
        assert_eq!(
            receipt.genesis_post_state_root,
            hex::encode(
                fixture
                    .proof
                    .finality_artifact
                    .commit_qc
                    .execution_commitment
                    .post_state_root
                    .as_ref()
            )
        );
        assert_eq!(receipt.height, 1);
        assert_eq!(receipt.block_hash, receipt.genesis_block_hash);
        assert_eq!(
            receipt.build_fingerprint,
            hex::encode(compiled_build_fingerprint().as_ref())
        );
        assert_eq!(
            receipt.config_fingerprint,
            hex::encode(fixture.status.config_fingerprint.as_ref())
        );
        assert_eq!(receipt.attestation_sha256.len(), 64);
    }

    #[test]
    fn attested_mode_rejects_wrong_challenge_signature_node_and_build() {
        let fixture = fixture();
        let expectations = attested_expectations(&fixture);
        assert!(
            verify_attested_fixture(&fixture, &expectations, [0xB4; 32])
                .expect_err("reject wrong challenge")
                .contains("exact caller challenge")
        );

        let mut forged_body = self::fixture();
        forged_body.attestation.body.challenge = [0xC3; 32];
        assert!(
            verify_attested_fixture(
                &forged_body,
                &attested_expectations(&forged_body),
                [0xC3; 32]
            )
            .expect_err("reject unsigned body change")
            .contains("node signature is invalid")
        );

        let mut wrong_node = attested_expectations(&fixture);
        wrong_node.expected_node_key = wrong_node.validator_keys[1].clone();
        assert!(
            verify_attested_fixture(&fixture, &wrong_node, fixture.challenge)
                .expect_err("reject another expected node")
                .contains("node_id does not match")
        );

        let mut wrong_network = attested_expectations(&fixture);
        wrong_network.network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"substituted PK2 expected network",
            )));
        assert!(
            verify_attested_fixture(&fixture, &wrong_network, fixture.challenge)
                .expect_err("reject another expected network")
                .contains("attestation network id does not match")
        );

        let mut wrong_genesis_binding = self::fixture();
        wrong_genesis_binding.attestation.body.network_id = wrong_network.network_id;
        resign_attestation(&mut wrong_genesis_binding);
        let mut wrong_genesis_expectations = attested_expectations(&wrong_genesis_binding);
        wrong_genesis_expectations.network_id = wrong_network.network_id;
        assert!(
            verify_attested_fixture(
                &wrong_genesis_binding,
                &wrong_genesis_expectations,
                wrong_genesis_binding.challenge,
            )
            .expect_err("reject a network identity not derived from the signed genesis")
            .contains("does not match decoded signed genesis")
        );

        let mut wrong_build = self::fixture();
        wrong_build.attestation.body.status.build_fingerprint = Hash::new(b"other build");
        resign_attestation(&mut wrong_build);
        assert!(
            verify_attested_fixture(
                &wrong_build,
                &attested_expectations(&wrong_build),
                wrong_build.challenge
            )
            .expect_err("reject another build")
            .contains("does not match this current-source verifier build")
        );
    }

    #[test]
    fn attested_mode_rejects_wrong_genesis_key_sha_and_execution_root() {
        let fixture = fixture();
        let mut wrong_key = attested_expectations(&fixture);
        wrong_key.genesis_public_key = KeyPair::try_random()
            .expect("alternate genesis key")
            .public_key()
            .to_string();
        assert!(
            verify_attested_fixture(&fixture, &wrong_key, fixture.challenge)
                .expect_err("reject wrong genesis signer")
                .contains("signed genesis validation failed")
        );

        let mut wrong_sha = attested_expectations(&fixture);
        wrong_sha.signed_genesis_sha256 = "11".repeat(32);
        assert!(
            verify_attested_fixture(&fixture, &wrong_sha, fixture.challenge)
                .expect_err("reject wrong signed genesis sha")
                .contains("signed genesis SHA-256")
        );

        let mut substituted_execution = self::fixture();
        substituted_execution
            .attestation
            .body
            .genesis_finality_proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash = Hash::new(b"substituted executed genesis wire");
        mirror_height_one_genesis_proof_into_tip(&mut substituted_execution);
        resign_attestation(&mut substituted_execution);
        assert!(
            verify_attested_fixture(
                &substituted_execution,
                &attested_expectations(&substituted_execution),
                substituted_execution.challenge
            )
            .expect_err("reject substituted genesis execution root")
            .contains("execution commitment does not match signed genesis executed wire")
        );
    }

    #[test]
    fn attested_mode_rejects_forged_genesis_and_tip_qcs() {
        let mut forged_genesis_qc = fixture();
        forged_genesis_qc
            .attestation
            .body
            .genesis_finality_proof
            .finality_artifact
            .commit_qc
            .aggregate_signature[0] ^= 0x80;
        mirror_height_one_genesis_proof_into_tip(&mut forged_genesis_qc);
        resign_attestation(&mut forged_genesis_qc);
        assert!(
            verify_attested_fixture(
                &forged_genesis_qc,
                &attested_expectations(&forged_genesis_qc),
                forged_genesis_qc.challenge
            )
            .expect_err("reject forged genesis QC")
            .contains("cryptographic genesis finality verification failed")
        );

        let mut forged_tip_qc = fixture();
        forged_tip_qc
            .attestation
            .body
            .finality_proof
            .finality_artifact
            .commit_qc
            .aggregate_signature[0] ^= 0x80;
        resign_attestation(&mut forged_tip_qc);
        assert!(
            verify_attested_fixture(
                &forged_tip_qc,
                &attested_expectations(&forged_tip_qc),
                forged_tip_qc.challenge
            )
            .expect_err("reject forged tip QC")
            .contains("height-one genesis and tip proofs do not match exactly")
        );
    }

    #[cfg(unix)]
    #[test]
    fn inherited_fd_mode_executes_full_attested_verification_over_four_distinct_handles() {
        let payload = b"descriptor-pinned verifier input";
        let mut file = tempfile::tempfile().expect("anonymous verifier input");
        file.write_all(payload).expect("write verifier input");
        file.flush().expect("flush verifier input");
        assert_eq!(
            read_bounded_fd(file.as_raw_fd(), 1024, "test input").expect("read inherited fd"),
            payload
        );

        let challenge = [0x5C; 32];
        let mut challenge_file = tempfile::tempfile().expect("anonymous challenge input");
        challenge_file
            .write_all(&challenge)
            .expect("write raw challenge");
        challenge_file.flush().expect("flush challenge");
        assert_eq!(
            read_challenge_source(&ChallengeSource::InheritedFd(challenge_file.as_raw_fd(),))
                .expect("read exact raw challenge"),
            challenge
        );

        let cli = parse_args([
            "--attestation-fd".to_owned(),
            "7".to_owned(),
            "--signed-genesis-fd".to_owned(),
            "8".to_owned(),
            "--expected-roster-fd".to_owned(),
            "9".to_owned(),
            "--challenge-fd".to_owned(),
            "10".to_owned(),
        ])
        .expect("parse attested inherited-fd mode");
        assert!(matches!(
            cli,
            Cli::Attested {
                attestation: InputSource::InheritedFd(7),
                signed_genesis: InputSource::InheritedFd(8),
                expected_roster: InputSource::InheritedFd(9),
                challenge: ChallengeSource::InheritedFd(10),
            }
        ));

        let fixture = fixture();
        let attestation_json =
            norito::json::to_json(&fixture.attestation).expect("encode attestation");
        let expectations_json = norito::json::to_json(&attested_expectations(&fixture))
            .expect("encode attested expectations");
        let mut attestation_file = tempfile::tempfile().expect("anonymous attestation input");
        attestation_file
            .write_all(attestation_json.as_bytes())
            .expect("write attestation input");
        attestation_file.flush().expect("flush attestation input");
        let mut genesis_file = tempfile::tempfile().expect("anonymous signed-genesis input");
        genesis_file
            .write_all(&fixture.signed_genesis)
            .expect("write signed genesis input");
        genesis_file.flush().expect("flush signed genesis input");
        let mut roster_file = tempfile::tempfile().expect("anonymous expected-roster input");
        roster_file
            .write_all(expectations_json.as_bytes())
            .expect("write expected roster input");
        roster_file.flush().expect("flush expected roster input");
        let mut bound_challenge_file =
            tempfile::tempfile().expect("anonymous bound challenge input");
        bound_challenge_file
            .write_all(&fixture.challenge)
            .expect("write bound challenge input");
        bound_challenge_file
            .flush()
            .expect("flush bound challenge input");
        let receipt_json = run_cli(Cli::Attested {
            attestation: InputSource::InheritedFd(attestation_file.as_raw_fd()),
            signed_genesis: InputSource::InheritedFd(genesis_file.as_raw_fd()),
            expected_roster: InputSource::InheritedFd(roster_file.as_raw_fd()),
            challenge: ChallengeSource::InheritedFd(bound_challenge_file.as_raw_fd()),
        })
        .expect("execute attested verifier over all four inherited descriptors");
        assert!(receipt_json.contains("\"schema_version\":4"));
        assert!(receipt_json.contains("\"status\":\"validated\""));
        assert!(receipt_json.contains(&hex::encode(fixture.challenge)));

        assert!(
            parse_args([
                "--attestation-fd".to_owned(),
                "7".to_owned(),
                "--signed-genesis-fd".to_owned(),
                "7".to_owned(),
                "--expected-roster-fd".to_owned(),
                "9".to_owned(),
                "--challenge-fd".to_owned(),
                "10".to_owned(),
            ])
            .expect_err("reject reused inherited fd")
            .contains("reused")
        );
    }

    #[test]
    fn valid_current_height_one_produces_bound_receipt() {
        let fixture = fixture();
        let receipt = verify_fixture(&fixture).expect("verify current proof");

        assert_eq!(receipt.schema_version, LEGACY_RECEIPT_SCHEMA_VERSION);
        assert_eq!(receipt.status, "validated");
        assert_eq!(receipt.chain_id, PK2_CHAIN_ID);
        assert_eq!(receipt.protocol_version, PROTOCOL_VERSION);
        assert_eq!(
            receipt.expected_node_key,
            fixture.expectations.expected_node_key
        );
        assert_eq!(
            receipt.node_fingerprint,
            hex::encode(fixture.status.node_fingerprint.as_ref())
        );
        assert_eq!(receipt.height, 1);
        assert_eq!(
            receipt.commit_decision_id,
            semantic_commit_decision_id(&fixture.proof.finality_artifact.commit_qc.as_ref())
                .expect("semantic commit decision id")
        );
        assert_eq!(receipt.validator_keys.len(), 4);
        assert_eq!(receipt.validator_powers, vec![1, 1, 1, 1]);
        assert_eq!(receipt.signer_indices, vec![0, 1, 2]);
        assert_eq!(receipt.signer_count, 3);
        assert_eq!(receipt.min_signers, 3);
        assert_eq!(receipt.total_power, 4);
        assert_eq!(receipt.signed_power, 3);
        assert_eq!(receipt.status_sha256.len(), 64);
        assert_eq!(receipt.proof_sha256.len(), 64);
    }

    #[test]
    fn rejects_proof_chosen_roster_and_threshold() {
        let mut wrong_key = fixture();
        wrong_key.expectations.validator_keys.swap(0, 1);
        assert!(
            verify_fixture(&wrong_key)
                .expect_err("reject wrong roster")
                .contains("expected validator key order/set")
        );

        let mut wrong_threshold = fixture();
        wrong_threshold.expectations.min_signers = 4;
        assert!(
            verify_fixture(&wrong_threshold)
                .expect_err("reject noncanonical threshold")
                .contains("canonical strict two-thirds")
        );
    }

    #[test]
    fn rejects_status_from_another_node_or_nonmember_expectation() {
        let mut wrong_status_node = fixture();
        let other_node = wrong_status_node
            .proof
            .finality_artifact
            .height_context
            .roster[1]
            .validator
            .clone();
        wrong_status_node.status.node_fingerprint = Hash::new(other_node.encode());
        assert!(
            verify_fixture(&wrong_status_node)
                .expect_err("reject status from another node")
                .contains("does not match expected_node_key")
        );

        let mut nonmember = fixture();
        nonmember.expectations.expected_node_key = PeerId::new(
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("generate nonmember BLS key")
                .public_key()
                .clone(),
        )
        .to_string();
        assert!(
            verify_fixture(&nonmember)
                .expect_err("reject nonmember node expectation")
                .contains("expected_node_key is not present in validator_keys")
        );
    }

    #[test]
    fn semantic_commit_decision_id_ignores_reproposal_round_and_qc_representation() {
        let fixture = fixture();
        let baseline = fixture.proof.finality_artifact.commit_qc.as_ref();

        let mut alternate_qc = fixture.proof.finality_artifact.commit_qc.clone();
        alternate_qc.round.view = alternate_qc.round.view.saturating_add(1);
        alternate_qc.proposal_round = alternate_qc.round;
        alternate_qc.signers = vec![0, 1, 3];
        alternate_qc.aggregate_signature = vec![0xAA, 0xBB];
        let alternate = alternate_qc.as_ref();
        assert!(baseline.same_commit_decision(alternate));
        assert_eq!(
            semantic_commit_decision_id(&baseline).expect("baseline decision id"),
            semantic_commit_decision_id(&alternate).expect("alternate decision id"),
        );

        let mut different_execution = baseline;
        different_execution.execution_commitment.post_state_root =
            Hash::new(b"different deterministic post state");
        assert!(!baseline.same_commit_decision(different_execution));
        assert_ne!(
            semantic_commit_decision_id(&baseline).expect("baseline decision id"),
            semantic_commit_decision_id(&different_execution)
                .expect("different execution decision id"),
        );

        let mut different_subject = baseline;
        different_subject.subject.payload_hash = Hash::new(b"different payload");
        assert!(!baseline.same_commit_decision(different_subject));
        assert_ne!(
            semantic_commit_decision_id(&baseline).expect("baseline decision id"),
            semantic_commit_decision_id(&different_subject).expect("different subject decision id"),
        );

        let mut reproposed = baseline;
        reproposed.round.view = reproposed.round.view.saturating_add(1);
        reproposed.proposal_round.view = reproposed.proposal_round.view.saturating_add(1);
        assert!(baseline.same_commit_decision(reproposed));
        assert_eq!(
            semantic_commit_decision_id(&baseline).expect("baseline decision id"),
            semantic_commit_decision_id(&reproposed).expect("re-proposed decision id"),
        );

        let mut prepare = baseline;
        prepare.phase = GlobalPhase::Prepare;
        assert!(
            semantic_commit_decision_id(&prepare)
                .expect_err("PrepareQC is not a semantic commit decision")
                .contains("requires a CommitQC")
        );
    }

    #[test]
    fn rejects_status_summary_or_missing_commit_at_height_one() {
        let mut wrong_power = fixture();
        wrong_power
            .status
            .last_commit_qc
            .as_mut()
            .expect("commit summary")
            .signed_power = 4;
        assert!(
            verify_fixture(&wrong_power)
                .expect_err("reject mismatched status power")
                .contains("quorum summary does not exactly match")
        );

        let mut missing_commit = fixture();
        missing_commit.status.phase = SumeragiV2StatusPhase::AwaitingProposal;
        missing_commit.status.body_state = SumeragiV2BodyState::Missing;
        missing_commit.status.height = 2;
        missing_commit.status.last_committed_height = 0;
        missing_commit.status.last_committed_subject = None;
        missing_commit.status.last_commit_qc = None;
        assert!(
            verify_fixture(&missing_commit)
                .expect_err("reject missing height-one commit")
                .contains("last_committed_height")
        );
    }

    #[test]
    fn rejects_invalid_aggregate_signature_and_restart_required() {
        let mut forged = fixture();
        forged.proof.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x80;
        assert!(
            verify_fixture(&forged)
                .expect_err("reject forged aggregate")
                .contains("cryptographic bridge finality verification failed")
        );

        let mut stopped = fixture();
        stopped.status.restart_required = true;
        assert!(
            verify_fixture(&stopped)
                .expect_err("reject fail-stopped node")
                .contains("restart_required=true")
        );
    }

    #[test]
    fn strict_expectations_json_rejects_unknown_fields() {
        let fixture = fixture();
        let status = norito::json::to_json(&fixture.status).expect("encode status");
        let proof = norito::json::to_json(&fixture.proof).expect("encode proof");
        let expectations =
            norito::json::to_json(&fixture.expectations).expect("encode expectations");
        let hostile = expectations.strip_suffix('}').expect("object").to_owned()
            + ",\"legacy_fallback\":true}";

        assert!(
            verify_json_inputs(status.as_bytes(), proof.as_bytes(), hostile.as_bytes())
                .expect_err("reject unknown expectation")
                .contains("invalid expected-roster JSON")
        );
    }

    #[test]
    fn release_binary_references_exact_source_marker() {
        let embedded = std::hint::black_box(BUILD_SOURCE_ID);
        assert_eq!(embedded, option_env!("IROHA_GIT_COMMIT_HASH"));
        if let Some(source_id) = embedded {
            assert!(!source_id.is_empty());
            assert!(source_id.is_ascii());
        }
    }
}
