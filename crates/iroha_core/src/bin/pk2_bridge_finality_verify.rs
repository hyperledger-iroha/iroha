//! Verify a challenged PK2 native finality capture against independently selected trust material.
//!
//! The canonical checkpoint, signed genesis, exact node and ordered committee are selected by
//! the caller. Native BLS and required paired-Pasta certificates are verified before emitting a
//! diagnostic receipt. A receipt is not a checkpoint, and genesis has no invented quorum certificate.
use iroha_core::{
    sumeragi::{attestation::NativePastaVerifier, crypto::core_key},
    validate_genesis_block,
};
use iroha_crypto::{Algorithm, Hash, PublicKey};
use iroha_data_model::{
    Encode as _, NetworkId,
    account::AccountId,
    block::{
        SignedBlock,
        consensus_v2::{ConsensusMode, is_valid_committee_size},
    },
    sumeragi::PROTOCOL_VERSION,
    sumeragi_finality::{
        MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityAttestation, SumeragiFinalityCheckpoint,
        SumeragiFinalityVerifier, VerifiedSumeragiBlock,
    },
};
use iroha_model_base::{chain::ChainId, peer::PeerId};
use iroha_sumeragi::{crypto::verify_attestations, message::Qc, types::Committee};
use norito::{JsonDeserialize, JsonSerialize};
use sha2::{Digest, Sha256};
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
const EXPECTATIONS_SCHEMA_VERSION: u8 = 1;
const RECEIPT_SCHEMA_VERSION: u8 = 1;
const PK2_CHAIN_ID: &str = "cbdc16";
const MAX_ATTESTATION_BYTES: u64 = 96 * 1024 * 1024;
const MAX_EXPECTATIONS_BYTES: u64 = 64 * 1024;
const CHALLENGE_BYTES: u64 = 32;
/// Exact sealed-source identity supplied by the artifact builder and retained in release binaries.
const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");
const HELP: &str = "\
Verify one challenged native PK2 finality capture.

Usage:
  pk2_bridge_finality_verify --attestation <attestation.json> \\
    --signed-genesis <genesis.signed.nrt> --trusted-checkpoint <checkpoint.nrt> \\
    --expected-roster <expected-roster.json> --challenge <64-lowercase-hex>

Each path accepts a mutually exclusive -fd form. --challenge-fd reads exactly 32 raw bytes.
Every inherited descriptor must be distinct, >= 3, and refer to one bounded regular file.

The strict expected-roster JSON contains schema_version=1, chain_id=\"cbdc16\",
network_id, protocol_version=8, consensus_mode=\"npos\", expected_node_key,
validator_keys (exact ordered BLS keys), min_signers, expected_height (>=2),
genesis_public_key, signed_genesis_sha256 and trusted_checkpoint_sha256.
The binary requires its compiled protocol revision and exact sealed executable source identity.

Select the complete checkpoint independently of the response. The requested proof must name
its retained tip or the immediate successor; gaps and retired formats are errors. Required
paired-Pasta attestations and the exact native BLS quorum are verified. Receipt hashes are
diagnostics; future verification requires the original authenticated checkpoint/proof history.
";
#[derive(Debug)]
struct Cli {
    attestation: InputSource,
    signed_genesis: InputSource,
    trusted_checkpoint: InputSource,
    expected_roster: InputSource,
    challenge: ChallengeSource,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputSource {
    Path(PathBuf),
    InheritedFd(i32),
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChallengeSource {
    Inline(Box<[u8; 32]>),
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
    expected_height: u64,
    genesis_public_key: String,
    signed_genesis_sha256: String,
    trusted_checkpoint_sha256: String,
}
#[derive(Debug, Clone, JsonSerialize)]
struct VerificationReceipt {
    schema_version: u8,
    status: String,
    chain_id: String,
    protocol_version: u16,
    network_id: NetworkId,
    expected_node_key: String,
    node_fingerprint: String,
    build_fingerprint: String,
    config_fingerprint: String,
    genesis_public_key: String,
    challenge: String,
    genesis_block_hash: String,
    height: u64,
    block_hash: String,
    core_hash: String,
    result: String,
    context_id: String,
    epoch: u64,
    authority_generation: String,
    validator_keys: Vec<String>,
    min_signers: u32,
    signer_indices: Vec<u32>,
    attestation_sha256: String,
    signed_genesis_sha256: String,
    trusted_checkpoint_sha256: String,
}
fn main() {
    let _ = std::hint::black_box(BUILD_SOURCE_ID);
    if let Err(error) = run() {
        eprintln!("PK2 native finality verification failed: {error}");
        process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let cli = parse_args(env::args().skip(1))?;
    println!("{}", run_cli(compiled_build_identity()?, cli)?);
    Ok(())
}
fn run_cli(
    build_identity: iroha_core::release_identity::BuildIdentity,
    cli: Cli,
) -> Result<String, String> {
    validate_distinct_inherited_fds(&cli)?;
    let challenge = read_challenge_source(&cli.challenge)?;
    let attestation = read_bounded_source(&cli.attestation, MAX_ATTESTATION_BYTES, "attestation")?;
    let genesis = read_bounded_source(
        &cli.signed_genesis,
        iroha_genesis::SIGNED_GENESIS_MAX_BYTES_V1 as u64,
        "signed genesis",
    )?;
    let checkpoint = read_bounded_source(
        &cli.trusted_checkpoint,
        MAX_FINALITY_CHECKPOINT_BYTES as u64,
        "trusted checkpoint",
    )?;
    let expectations = read_bounded_source(
        &cli.expected_roster,
        MAX_EXPECTATIONS_BYTES,
        "expected roster",
    )?;
    let receipt = verify_json_inputs(
        build_identity,
        &attestation,
        &genesis,
        &checkpoint,
        &expectations,
        challenge,
    )?;
    norito::json::to_json(&receipt).map_err(|error| error.to_string())
}
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let (
        mut attestation,
        mut signed_genesis,
        mut trusted_checkpoint,
        mut expected_roster,
        mut challenge,
    ) = (None, None, None, None, None);
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            println!("{HELP}");
            process::exit(0);
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {argument}"))?;
        if argument == "--challenge" || argument == "--challenge-fd" {
            let parsed = if argument == "--challenge" {
                ChallengeSource::Inline(Box::new(parse_challenge(&value)?))
            } else {
                ChallengeSource::InheritedFd(parse_inherited_fd(&value, "challenge")?)
            };
            if challenge.replace(parsed).is_some() {
                return Err("duplicate challenge input".into());
            }
            continue;
        }
        let (slot, label, fd) = match argument.as_str() {
            "--attestation" => (&mut attestation, "attestation", false),
            "--attestation-fd" => (&mut attestation, "attestation", true),
            "--signed-genesis" => (&mut signed_genesis, "signed genesis", false),
            "--signed-genesis-fd" => (&mut signed_genesis, "signed genesis", true),
            "--trusted-checkpoint" => (&mut trusted_checkpoint, "trusted checkpoint", false),
            "--trusted-checkpoint-fd" => (&mut trusted_checkpoint, "trusted checkpoint", true),
            "--expected-roster" => (&mut expected_roster, "expected roster", false),
            "--expected-roster-fd" => (&mut expected_roster, "expected roster", true),
            _ => return Err(format!("unknown argument {argument}")),
        };
        let source = if fd {
            InputSource::InheritedFd(parse_inherited_fd(&value, label)?)
        } else {
            InputSource::Path(PathBuf::from(value))
        };
        set_input_source(slot, source, label)?;
    }
    let cli = Cli {
        attestation: attestation.ok_or("missing required attestation input")?,
        signed_genesis: signed_genesis.ok_or("missing required signed-genesis input")?,
        trusted_checkpoint: trusted_checkpoint
            .ok_or("missing required trusted-checkpoint input")?,
        expected_roster: expected_roster.ok_or("missing required expected-roster input")?,
        challenge: challenge.ok_or("missing required challenge input")?,
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
        ChallengeSource::Inline(challenge) => Ok(**challenge),
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
    if let ChallengeSource::InheritedFd(fd) = &cli.challenge {
        seen.insert(*fd);
    }
    for source in [
        &cli.attestation,
        &cli.signed_genesis,
        &cli.trusted_checkpoint,
        &cli.expected_roster,
    ] {
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
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| format!("{label} inherited fd {fd} length does not fit usize"))?;
    let mut bytes = Vec::with_capacity(capacity.saturating_add(1));
    (&mut file)
        .take(metadata.len().saturating_add(1))
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
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| format!("{label} {} length does not fit usize", path.display()))?;
    let mut bytes = Vec::with_capacity(capacity.saturating_add(1));
    let mut file = file;
    (&mut file)
        .take(metadata.len().saturating_add(1))
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
#[expect(
    clippy::too_many_lines,
    reason = "independent input pins and native certificate checks form one ordered fail-closed verification"
)]
fn verify_json_inputs(
    build_identity: iroha_core::release_identity::BuildIdentity,
    attestation_json: &[u8],
    signed_genesis: &[u8],
    checkpoint_bytes: &[u8],
    expectations_json: &[u8],
    challenge: [u8; 32],
) -> Result<VerificationReceipt, String> {
    if attestation_json.len() as u64 > MAX_ATTESTATION_BYTES
        || expectations_json.len() as u64 > MAX_EXPECTATIONS_BYTES
    {
        return Err("JSON input exceeds the verifier bound".into());
    }
    let attestation: SumeragiFinalityAttestation = norito::json::from_slice(attestation_json)
        .map_err(|e| format!("invalid native attestation JSON: {e}"))?;
    let expectations: ExpectedRosterDocument = norito::json::from_slice(expectations_json)
        .map_err(|e| format!("invalid expected-roster JSON: {e}"))?;
    validate_expectations(&expectations)?;
    build_identity
        .release_source_commit()
        .map_err(|e| e.to_string())?;
    attestation
        .verify()
        .map_err(|e| format!("invalid reporting-node attestation: {e}"))?;
    let body = &attestation.body;
    if body.challenge != challenge || challenge == [0; 32] {
        return Err("attestation differs from caller challenge".into());
    }
    if body.network_id != expectations.network_id {
        return Err("attestation network differs".into());
    }
    if body.node_id
        != parse_expected_peer_key(&expectations.expected_node_key, "expected_node_key")?
    {
        return Err("attestation reporting node differs".into());
    }
    if body.build_fingerprint != build_identity.build_fingerprint() {
        return Err("attestation build fingerprint differs from this sealed executable".into());
    }
    if body.status.protocol_version != PROTOCOL_VERSION
        || body.status.config_fingerprint != body.config_fingerprint
        || body.config_fingerprint.as_ref().iter().all(|b| *b == 0)
    {
        return Err("attestation protocol or configuration fingerprint differs".into());
    }
    if !body.status.is_signing() || body.status.is_halted() {
        return Err("reporting node is not an anchored active validator".into());
    }
    if body.finality_proof.height() != expectations.expected_height {
        return Err("attestation durable height differs from requested height".into());
    }
    if sha256_hex(signed_genesis) != expectations.signed_genesis_sha256 {
        return Err("signed genesis SHA-256 differs".into());
    }
    if sha256_hex(checkpoint_bytes) != expectations.trusted_checkpoint_sha256 {
        return Err("trusted checkpoint SHA-256 differs".into());
    }
    let genesis_key = parse_expected_genesis_public_key(&expectations.genesis_public_key)?;
    let genesis = decode_validate_signed_genesis(signed_genesis, &genesis_key)?;
    if NetworkId::from_genesis_hash(genesis.hash()) != expectations.network_id
        || body.genesis_block_hash != genesis.hash()
    {
        return Err("signed genesis network or block hash differs".into());
    }
    // This checks the signed root and result-only frame shape. H1 execution is never asserted
    // to have a quorum certificate or used as a standalone monetary/finality capability.
    let mut genesis_verifier = SumeragiFinalityVerifier::new(
        &genesis,
        expectations.chain_id.as_str(),
        body.genesis_finality_proof.committee.clone(),
    )
    .map_err(|e| format!("invalid selected genesis: {e}"))?;
    genesis_verifier
        .verify(&body.genesis_finality_proof)
        .map_err(|e| format!("invalid genesis frame: {e}"))?;
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(checkpoint_bytes)
        .map_err(|e| e.to_string())?;
    let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        &expectations.network_id,
        expectations.chain_id.as_str(),
    )
    .map_err(|e| format!("invalid selected checkpoint: {e}"))?;
    if body.status.instance != verifier.instance().0 {
        return Err("native status instance differs".into());
    }
    // A compact checkpoint retains H1 only when adjacent. If retained, the attested H1 result
    // must equal that original decision. Higher checkpoints confer no detached H1-result claim.
    if checkpoint.height() <= 2 {
        verifier
            .verify_retained_decision(&body.genesis_finality_proof)
            .map_err(|e| format!("genesis result differs from selected checkpoint: {e}"))?;
    }
    let verified = if body.finality_proof.height() == checkpoint.height() {
        verifier.verify_same_decision(checkpoint.tip(), &body.finality_proof)
    } else {
        verifier.verify(&body.finality_proof)
    }
    .map_err(|e| format!("native finality verification failed: {e}"))?;
    let schedule = &verified.commitment().schedule.current;
    if schedule.mode != ConsensusMode::Npos {
        return Err("authenticated schedule is not NPoS".into());
    }
    let ordered: Vec<_> = schedule
        .committee
        .iter()
        .map(|v| v.validator.to_string())
        .collect();
    if ordered != expectations.validator_keys {
        return Err("authenticated ordered committee differs".into());
    }
    let qc = verify_native_attestations(&verified, verifier.instance(), expectations.network_id)?;
    if qc.signers.count_ones() != expectations.min_signers as usize {
        return Err("native signer count differs from exact quorum".into());
    }
    Ok(VerificationReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        status: "validated".into(),
        chain_id: expectations.chain_id.to_string(),
        protocol_version: PROTOCOL_VERSION,
        network_id: expectations.network_id,
        expected_node_key: expectations.expected_node_key,
        node_fingerprint: hex::encode(body.node_fingerprint.as_ref()),
        build_fingerprint: hex::encode(body.build_fingerprint.as_ref()),
        config_fingerprint: hex::encode(body.config_fingerprint.as_ref()),
        genesis_public_key: genesis_key.to_string(),
        challenge: hex::encode(challenge),
        genesis_block_hash: hex::encode(genesis.hash().as_ref()),
        height: verified.height(),
        block_hash: hex::encode(verified.header().hash().as_ref()),
        core_hash: hex::encode(verified.core_hash().0),
        result: hex::encode(verified.result().0),
        context_id: hex::encode(verified.context_id().as_ref()),
        epoch: schedule.authorization.epoch,
        authority_generation: hex::encode(
            schedule
                .authority
                .authority_id()
                .map_err(|e| e.to_string())?,
        ),
        validator_keys: ordered,
        min_signers: expectations.min_signers,
        signer_indices: qc.signers.ones().collect(),
        attestation_sha256: sha256_hex(attestation_json),
        signed_genesis_sha256: expectations.signed_genesis_sha256,
        trusted_checkpoint_sha256: expectations.trusted_checkpoint_sha256,
    })
}
fn verify_native_attestations(
    verified: &VerifiedSumeragiBlock,
    instance: iroha_sumeragi::types::Hash32,
    network: NetworkId,
) -> Result<Qc, String> {
    let certificate = verified
        .block()
        .commit_certificate()
        .ok_or("native certificate missing")?;
    let qc: Qc = norito::decode_canonical(certificate.commit_qc()).map_err(|e| e.to_string())?;
    let keys = verified
        .commitment()
        .schedule
        .current
        .committee
        .iter()
        .map(|v| core_key(v.validator.public_key()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let committee = Committee::new(keys).map_err(|e| e.to_string())?;
    if qc
        .attestation_witness
        .as_ref()
        .is_some_and(|witness| witness.as_slice() != certificate.result_preimage())
    {
        return Err("paired-Pasta witness differs from exact native execution".into());
    }
    verify_attestations(
        &NativePastaVerifier::new(instance, network),
        &committee,
        &qc,
    )
    .map_err(|e| format!("native paired-Pasta verification failed: {e:?}"))?;
    Ok(qc)
}
fn validate_expectations(expectations: &ExpectedRosterDocument) -> Result<(), String> {
    if expectations.schema_version != EXPECTATIONS_SCHEMA_VERSION {
        return Err("unsupported expected-roster schema".into());
    }
    if expectations.chain_id.as_str() != PK2_CHAIN_ID {
        return Err("expected-roster chain is not PK2 cbdc16".into());
    }
    if expectations.protocol_version != PROTOCOL_VERSION {
        return Err("expected-roster protocol differs".into());
    }
    if expectations.consensus_mode != "npos" {
        return Err("expected-roster mode must be npos".into());
    }
    if expectations.expected_height < 2 {
        return Err("native PK2 execution finality requires height >= 2".into());
    }
    let count = expectations.validator_keys.len();
    if !is_valid_committee_size(count)
        || expectations.min_signers as usize != 2 * ((count - 1) / 3) + 1
    {
        return Err("expected committee must be exactly 3f+1 and quorum exactly 2f+1".into());
    }
    let validators = parse_expected_validator_keys(&expectations.validator_keys)?;
    if !validators.contains(&parse_expected_peer_key(
        &expectations.expected_node_key,
        "expected_node_key",
    )?) {
        return Err("expected reporting node is not in ordered committee".into());
    }
    for (value, label) in [
        (&expectations.signed_genesis_sha256, "signed_genesis_sha256"),
        (
            &expectations.trusted_checkpoint_sha256,
            "trusted_checkpoint_sha256",
        ),
    ] {
        validate_sha256_literal(value, label)?;
    }
    parse_expected_genesis_public_key(&expectations.genesis_public_key)?;
    Ok(())
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
fn decode_validate_signed_genesis(bytes: &[u8], key: &PublicKey) -> Result<SignedBlock, String> {
    let block = iroha_genesis::decode_signed_genesis(bytes)
        .map_err(|e| format!("invalid signed genesis: {e}"))?;
    if block.encode_wire().map_err(|e| e.to_string())? != bytes {
        return Err("signed genesis is not canonical".into());
    }
    validate_genesis_block(&block, &AccountId::new(key.clone()))
        .map_err(|e| format!("signed genesis validation failed: {e}"))?;
    Ok(block)
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
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn compiled_build_identity() -> Result<iroha_core::release_identity::BuildIdentity, String> {
    iroha_core::compiled_build_identity!().map_err(|error| error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{KeyPair, Signature, SignatureOf, bls_normal_aggregate_signatures};
    use iroha_data_model::{
        block::{CommitCertificate, builder::BlockBuilder, decode_versioned_signed_block},
        isi::Log,
        level::Level,
        parameter::system::SumeragiConsensusMode,
        sumeragi::{SumeragiHaltReason, SumeragiStatus},
        sumeragi_finality::{
            ExecutionResultCommitment, SumeragiFinalityAttestationBody, SumeragiFinalityProof,
        },
        testing::native_finality::NativeFinalityFixture,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_sumeragi::types::{AggregateSignature, Bitmap};
    use std::time::Duration;
    #[cfg(unix)]
    use std::{io::Write as _, os::fd::AsRawFd as _};

    fn test_build_identity() -> iroha_core::release_identity::BuildIdentity {
        iroha_core::release_identity::BuildIdentity::from_compiled_parts(
            "test-executable",
            Some("1111111111111111111111111111111111111111"),
            None,
            None,
            None,
            None,
        )
        .unwrap()
    }
    #[derive(Clone)]
    struct Fixture {
        native: NativeFinalityFixture,
        attestation: SumeragiFinalityAttestation,
        expectations: ExpectedRosterDocument,
        node: KeyPair,
        signed_genesis: Vec<u8>,
        checkpoint: Vec<u8>,
    }
    fn advance(native: &mut NativeFinalityFixture) -> SumeragiFinalityProof {
        let wallet = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let mut tx = TransactionBuilder::new(
            native.network_id(),
            AccountId::new(wallet.public_key().clone()),
            FeePaymentIntent::authority(vec![], None),
        );
        tx.set_creation_time(Duration::from_millis(native.next_header().height().get()));
        let tx = tx
            .with_instructions([Log::new(Level::INFO, "PK2 protocol fixture".into())])
            .sign(wallet.private_key());
        let mut builder = BlockBuilder::new(native.next_header());
        builder.push_transaction(tx);
        let mut block = builder.build(BTreeSet::new());
        NativeFinalityFixture::install_network_results(&mut block, vec![Ok(Default::default())]);
        native.certify(block)
    }
    fn fixture() -> Fixture {
        let mut native =
            NativeFinalityFixture::start_with_mode(PK2_CHAIN_ID, SumeragiConsensusMode::Npos);
        let checkpoint = native.checkpoint().encode_canonical().unwrap();
        let signed_genesis = native.genesis().encode_wire().unwrap();
        let tip = advance(&mut native);
        let node = keys().remove(0);
        let node_id = PeerId::new(node.public_key().clone());
        let config = Hash::new(b"PK2 native fixture config");
        let body = SumeragiFinalityAttestationBody {
            challenge: [0x5c; 32],
            network_id: native.network_id(),
            node_fingerprint: Hash::new(node_id.encode()),
            node_id,
            build_fingerprint: test_build_identity().build_fingerprint(),
            config_fingerprint: config,
            genesis_block_hash: native.genesis().hash(),
            genesis_finality_proof: native.genesis_proof().clone(),
            status: SumeragiStatus {
                protocol_version: PROTOCOL_VERSION,
                config_fingerprint: config,
                beacon_horizon: None,
                instance: native.verifier().instance().0,
                height: 3,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: 2,
                applied_height: 2,
                awaiting: false,
                signer: Some(node.public_key().clone()),
                unanchored: false,
                abstaining: false,
                halted: None,
                footprint: Default::default(),
            },
            finality_proof: tip,
        };
        let signature =
            SignatureOf::try_from_hash(node.private_key(), body.signing_hash()).unwrap();
        let expectations = ExpectedRosterDocument {
            schema_version: 1,
            chain_id: PK2_CHAIN_ID.into(),
            network_id: native.network_id(),
            protocol_version: PROTOCOL_VERSION,
            consensus_mode: "npos".into(),
            expected_node_key: PeerId::new(node.public_key().clone()).to_string(),
            validator_keys: native
                .latest()
                .committee
                .iter()
                .map(|v| PeerId::new(v.public_key.clone()).to_string())
                .collect(),
            min_signers: 3,
            expected_height: 2,
            genesis_public_key: KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519)
                .public_key()
                .to_string(),
            signed_genesis_sha256: sha256_hex(&signed_genesis),
            trusted_checkpoint_sha256: sha256_hex(&checkpoint),
        };
        Fixture {
            native,
            attestation: SumeragiFinalityAttestation { body, signature },
            expectations,
            node,
            signed_genesis,
            checkpoint,
        }
    }
    fn keys() -> Vec<KeyPair> {
        let mut keys: Vec<_> = (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect();
        keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
        keys
    }
    fn resign(f: &mut Fixture) {
        f.attestation.signature =
            SignatureOf::try_from_hash(f.node.private_key(), f.attestation.body.signing_hash())
                .unwrap();
    }
    fn verify(f: &Fixture) -> Result<VerificationReceipt, String> {
        verify_with_identity(f, test_build_identity())
    }
    fn verify_with_identity(
        f: &Fixture,
        identity: iroha_core::release_identity::BuildIdentity,
    ) -> Result<VerificationReceipt, String> {
        verify_json_inputs(
            identity,
            norito::json::to_json(&f.attestation).unwrap().as_bytes(),
            &f.signed_genesis,
            &f.checkpoint,
            norito::json::to_json(&f.expectations).unwrap().as_bytes(),
            [0x5c; 32],
        )
    }
    fn replace_certificate(
        proof: &mut SumeragiFinalityProof,
        edit: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>, &mut Vec<u8>),
    ) {
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let cert = block.commit_certificate().unwrap();
        let (mut header, mut qc, mut result) = (
            cert.consensus_header().to_vec(),
            cert.commit_qc().to_vec(),
            cert.result_preimage().to_vec(),
        );
        edit(&mut header, &mut qc, &mut result);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            header, qc, result,
        )));
        proof.block_wire = block.encode_wire().unwrap();
    }
    #[test]
    fn valid_native_capture_binds_exact_inputs_and_durable_decision() {
        let f = fixture();
        let receipt = verify(&f).unwrap();
        assert_eq!(receipt.schema_version, 1);
        assert_eq!(receipt.height, 2);
        assert_eq!(receipt.min_signers, 3);
        assert_eq!(receipt.signer_indices, vec![0, 1, 2]);
        assert_eq!(receipt.validator_keys, f.expectations.validator_keys);
        assert_eq!(receipt.network_id, f.native.network_id());
        assert_eq!(receipt.challenge, hex::encode([0x5c; 32]));
        assert_eq!(receipt.signed_genesis_sha256, sha256_hex(&f.signed_genesis));
        assert_eq!(receipt.trusted_checkpoint_sha256, sha256_hex(&f.checkpoint));
        assert_eq!(
            receipt.block_hash,
            hex::encode(f.native.latest().block_header.hash().as_ref())
        );
        let verified = f
            .native
            .verifier()
            .verify_retained_decision(f.native.latest())
            .unwrap();
        assert_eq!(receipt.result, hex::encode(verified.result().0));
        assert_eq!(
            receipt.context_id,
            hex::encode(verified.context_id().as_ref())
        );
        let encoded = norito::json::to_json(&receipt).unwrap();
        assert!(!encoded.contains("genesis_commit_decision_id"));
        assert!(!encoded.contains("validator_powers"));
    }
    #[test]
    fn retained_tip_and_immediate_successor_verify_but_height_gaps_do_not() {
        let mut f = fixture();
        f.checkpoint = f.native.checkpoint().encode_canonical().unwrap();
        f.expectations.trusted_checkpoint_sha256 = sha256_hex(&f.checkpoint);
        assert!(verify(&f).is_ok());
        f.attestation.body.finality_proof = advance(&mut f.native);
        f.attestation.body.status.committed_height = 3;
        f.attestation.body.status.applied_height = 3;
        f.attestation.body.status.height = 4;
        f.expectations.expected_height = 3;
        resign(&mut f);
        assert!(verify(&f).is_ok());
        f.checkpoint = fixture().checkpoint;
        f.expectations.trusted_checkpoint_sha256 = sha256_hex(&f.checkpoint);
        assert!(verify(&f).unwrap_err().contains("immediately extend"));
    }
    #[test]
    fn rejects_different_and_development_executable_identity() {
        let f = fixture();
        for source in [
            "3333333333333333333333333333333333333333",
            "local-fast-build",
        ] {
            let identity = iroha_core::release_identity::BuildIdentity::from_compiled_parts(
                "test-executable",
                Some(source),
                None,
                None,
                None,
                None,
            )
            .unwrap();
            assert!(verify_with_identity(&f, identity).is_err());
        }
    }
    #[test]
    fn rejects_wrong_challenge_signature_node_build_and_configuration() {
        let f = fixture();
        let mut bad = f.clone();
        bad.attestation.body.challenge = [7; 32];
        resign(&mut bad);
        assert!(verify(&bad).unwrap_err().contains("challenge"));
        let mut bad = f.clone();
        bad.attestation.signature = SignatureOf::try_from_hash(
            KeyPair::from_seed(vec![99; 32], Algorithm::BlsNormal).private_key(),
            bad.attestation.body.signing_hash(),
        )
        .unwrap();
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        bad.expectations.expected_node_key = bad.expectations.validator_keys[1].clone();
        assert!(verify(&bad).unwrap_err().contains("node"));
        let mut bad = f.clone();
        bad.attestation.body.build_fingerprint = Hash::new(b"other build");
        resign(&mut bad);
        assert!(verify(&bad).unwrap_err().contains("build"));
        let mut bad = f.clone();
        bad.attestation.body.status.config_fingerprint = Hash::new(b"other config");
        resign(&mut bad);
        assert!(verify(&bad).unwrap_err().contains("configuration"));
        let mut bad = f;
        bad.attestation.body.status.protocol_version = 4;
        resign(&mut bad);
        assert!(verify(&bad).unwrap_err().contains("protocol"));
    }
    #[test]
    fn rejects_wrong_genesis_key_digest_network_and_executed_root() {
        let f = fixture();
        let mut bad = f.clone();
        bad.expectations.genesis_public_key = KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519)
            .public_key()
            .to_string();
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        bad.signed_genesis[0] ^= 1;
        assert!(verify(&bad).unwrap_err().contains("SHA-256"));
        let mut bad = f.clone();
        bad.expectations.network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"wrong network")),
        );
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        replace_certificate(
            &mut bad.attestation.body.genesis_finality_proof,
            |_, _, result| {
                let mut commitment = ExecutionResultCommitment::decode(result).unwrap();
                commitment.execution.post_state_root = Hash::new(b"changed root");
                *result = commitment.preimage().unwrap();
            },
        );
        resign(&mut bad);
        assert!(verify(&bad).is_err());
        let mut bad = f;
        replace_certificate(&mut bad.attestation.body.finality_proof, |_, _, result| {
            let mut commitment = ExecutionResultCommitment::decode(result).unwrap();
            commitment.execution.post_state_root = Hash::new(b"changed root");
            *result = commitment.preimage().unwrap();
        });
        resign(&mut bad);
        assert!(verify(&bad).is_err());
    }
    #[test]
    fn rejects_forged_genesis_certificate_and_tip_bls_signature() {
        let f = fixture();
        let mut bad = f.clone();
        let cert = decode_versioned_signed_block(&f.attestation.body.finality_proof.block_wire)
            .unwrap()
            .commit_certificate()
            .unwrap()
            .commit_qc()
            .to_vec();
        replace_certificate(
            &mut bad.attestation.body.genesis_finality_proof,
            |_, qc, _| *qc = cert,
        );
        resign(&mut bad);
        assert!(verify(&bad).is_err());
        let mut bad = f;
        replace_certificate(&mut bad.attestation.body.finality_proof, |_, encoded, _| {
            let mut qc: Qc = norito::decode_canonical(encoded).unwrap();
            qc.agg_sig.0[0] ^= 0x80;
            *encoded = norito::encode_canonical(&qc).unwrap();
        });
        resign(&mut bad);
        assert!(verify(&bad).is_err());
    }
    #[test]
    fn rejects_proof_chosen_roster_threshold_order_and_nonmember_node() {
        let f = fixture();
        let mut bad = f.clone();
        bad.expectations.validator_keys.swap(0, 1);
        assert!(verify(&bad).unwrap_err().contains("ordered committee"));
        let mut bad = f.clone();
        bad.expectations.min_signers = 2;
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        bad.expectations
            .validator_keys
            .push(bad.expectations.validator_keys[0].clone());
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        bad.expectations.validator_keys[1] = bad.expectations.validator_keys[0].clone();
        assert!(verify(&bad).is_err());
        let mut bad = f;
        bad.expectations.expected_node_key = PeerId::new(
            KeyPair::from_seed(vec![99; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
        )
        .to_string();
        assert!(verify(&bad).is_err());
    }
    #[test]
    fn diagnostic_decision_id_ignores_valid_reproposal_round_and_signer_representation() {
        let f = fixture();
        let expected = verify(&f).unwrap();
        let mut changed = f;
        replace_certificate(
            &mut changed.attestation.body.finality_proof,
            |_, encoded, _| {
                let mut qc: Qc = norito::decode_canonical(encoded).unwrap();
                qc.view += 7;
                qc.signers = Bitmap::from_indices(4, [1, 2, 3]).unwrap();
                let shares: Vec<_> = keys()[1..]
                    .iter()
                    .map(|key| Signature::try_new(key.private_key(), &qc.preimage()).unwrap())
                    .collect();
                let refs: Vec<_> = shares.iter().map(Signature::payload).collect();
                qc.agg_sig = AggregateSignature(
                    bls_normal_aggregate_signatures(&refs)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                *encoded = norito::encode_canonical(&qc).unwrap();
            },
        );
        resign(&mut changed);
        let result = verify(&changed).unwrap();
        assert_eq!(expected.context_id, result.context_id);
        assert_eq!(expected.result, result.result);
        assert_eq!(result.signer_indices, vec![1, 2, 3]);
    }
    #[test]
    fn rejects_stale_height_summary_missing_certificate_and_genesis_execution_claim() {
        let f = fixture();
        let mut bad = f.clone();
        bad.attestation.body.status.committed_height = 3;
        resign(&mut bad);
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        let mut block =
            decode_versioned_signed_block(&bad.attestation.body.finality_proof.block_wire).unwrap();
        block.set_commit_certificate(None);
        bad.attestation.body.finality_proof.block_wire = block.encode_wire().unwrap();
        resign(&mut bad);
        assert!(verify(&bad).is_err());
        let mut bad = f;
        bad.attestation.body.finality_proof = bad.attestation.body.genesis_finality_proof.clone();
        bad.attestation.body.status.committed_height = 1;
        bad.attestation.body.status.applied_height = 1;
        bad.expectations.expected_height = 1;
        resign(&mut bad);
        assert!(verify(&bad).unwrap_err().contains("height >= 2"));
    }
    #[test]
    fn rejects_halted_unanchored_abstaining_or_observer_node() {
        let f = fixture();
        for index in 0..4 {
            let mut bad = f.clone();
            let status = &mut bad.attestation.body.status;
            match index {
                0 => status.halted = Some(SumeragiHaltReason::DriverAnomaly),
                1 => status.unanchored = true,
                2 => status.abstaining = true,
                _ => status.signer = None,
            }
            resign(&mut bad);
            assert!(verify(&bad).is_err());
        }
    }
    #[test]
    fn checkpoint_is_canonical_bounded_and_independently_pinned() {
        let f = fixture();
        let mut bad = f.clone();
        bad.checkpoint.push(0);
        assert!(verify(&bad).unwrap_err().contains("SHA-256"));
        bad.expectations.trusted_checkpoint_sha256 = sha256_hex(&bad.checkpoint);
        assert!(verify(&bad).is_err());
        let mut bad = f.clone();
        bad.checkpoint = NativeFinalityFixture::new()
            .checkpoint()
            .encode_canonical()
            .unwrap();
        bad.expectations.trusted_checkpoint_sha256 = sha256_hex(&bad.checkpoint);
        assert!(verify(&bad).is_err());
        let mut bad = f;
        bad.checkpoint = vec![];
        bad.expectations.trusted_checkpoint_sha256 = sha256_hex(&bad.checkpoint);
        assert!(verify(&bad).is_err());
        assert!(
            SumeragiFinalityCheckpoint::decode_canonical(&vec![
                0;
                MAX_FINALITY_CHECKPOINT_BYTES + 1
            ])
            .is_err()
        );
    }
    #[test]
    fn strict_json_and_retired_modes_are_rejected() {
        let f = fixture();
        let expected = norito::json::to_json(&f.expectations).unwrap();
        let hostile = expected.strip_suffix('}').unwrap().to_owned() + ",\"legacy_fallback\":true}";
        assert!(
            verify_json_inputs(
                test_build_identity(),
                norito::json::to_json(&f.attestation).unwrap().as_bytes(),
                &f.signed_genesis,
                &f.checkpoint,
                hostile.as_bytes(),
                [0x5c; 32]
            )
            .unwrap_err()
            .contains("expected-roster JSON")
        );
        for option in [
            "--status",
            "--status-fd",
            "--proof",
            "--proof-fd",
            "--trusted-context-id",
        ] {
            assert!(
                parse_args([option.into(), "7".into()])
                    .unwrap_err()
                    .contains("unknown argument")
            );
        }
        for schema in [2, 3, 4] {
            let mut bad = f.clone();
            bad.expectations.schema_version = schema;
            assert!(verify(&bad).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn inherited_fd_mode_verifies_five_distinct_inputs_and_rejects_aliases() {
        let f = fixture();
        let attestation = norito::json::to_json(&f.attestation).unwrap();
        let expected = norito::json::to_json(&f.expectations).unwrap();
        let payloads = [
            attestation.as_bytes(),
            f.signed_genesis.as_slice(),
            f.checkpoint.as_slice(),
            expected.as_bytes(),
            &[0x5c; 32],
        ];
        let mut files = Vec::new();
        for bytes in payloads {
            let mut file = tempfile::tempfile().unwrap();
            file.write_all(bytes).unwrap();
            file.flush().unwrap();
            files.push(file);
        }
        let args = |fds: &[i32]| {
            [
                "--attestation-fd",
                "--signed-genesis-fd",
                "--trusted-checkpoint-fd",
                "--expected-roster-fd",
                "--challenge-fd",
            ]
            .into_iter()
            .zip(fds)
            .flat_map(|(label, fd)| [label.to_owned(), fd.to_string()])
            .collect::<Vec<_>>()
        };
        let mut fds: Vec<_> = files.iter().map(|file| file.as_raw_fd()).collect();
        let receipt = run_cli(test_build_identity(), parse_args(args(&fds)).unwrap()).unwrap();
        assert!(receipt.contains("\"status\":\"validated\""));
        assert!(receipt.contains(&hex::encode([0x5c; 32])));
        fds[2] = fds[4];
        assert!(parse_args(args(&fds)).unwrap_err().contains("reused"));
        assert!(parse_inherited_fd("2", "test").is_err());
        assert!(read_bounded_fd(files[0].as_raw_fd(), 1, "test").is_err());
    }
    #[test]
    fn challenge_and_input_custody_reject_noncanonical_or_changed_inputs() {
        assert!(parse_challenge(&"00".repeat(32)).is_err());
        assert!(parse_challenge(&"AB".repeat(32)).is_err());
        assert!(parse_challenge("01").is_err());
        assert_eq!(parse_challenge(&"5c".repeat(32)).unwrap(), [0x5c; 32]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input");
        fs::write(&path, b"one").unwrap();
        assert_eq!(read_bounded(&path, 3, "test").unwrap(), b"one");
        assert!(read_bounded(&path, 2, "test").is_err());
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_bounded(&link, 3, "test").is_err());
        }
        let mut slot = Some(InputSource::Path(path));
        assert!(set_input_source(&mut slot, InputSource::InheritedFd(9), "test").is_err());
    }
    #[test]
    fn release_binary_references_exact_source_marker() {
        let embedded = std::hint::black_box(BUILD_SOURCE_ID);
        assert_eq!(embedded, option_env!("IROHA_GIT_COMMIT_HASH"));
        if let Some(source) = embedded {
            assert!(!source.is_empty());
            assert!(source.is_ascii());
        }
    }
}
