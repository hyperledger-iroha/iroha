//! Authenticated operator commands for the SoraFS PDP provider protocol.
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::NetworkId;
use norito::{
    json::{Map, Value, from_slice, to_string_pretty, to_vec},
    to_bytes,
};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    redirect::Policy,
};
use sha2::{Digest as _, Sha256};
use sorafs_manifest::{
    PdpChallengeV1, PdpCommitmentV1, PdpProofV1,
    pdp::{
        PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1, PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
        PDP_PROOF_MAX_CANONICAL_BYTES_V1,
    },
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
use std::{
    ffi::CString,
    io::Write,
    os::{
        fd::{AsRawFd as _, FromRawFd as _},
        unix::ffi::OsStrExt as _,
    },
    path::Component,
};
use url::Url;
const ROUTE_ENQUEUE: &str = "/v1/sorafs/pdp/challenge";
const ROUTE_NEXT: &str = "/v1/sorafs/pdp/next";
const ROUTE_SUBMIT: &str = "/v1/sorafs/pdp/proof";
const ROUTE_STATUS: &str = "/v1/sorafs/pdp/status";
const ROUTE_EXPORT: &str = "/v1/sorafs/pdp/export";
const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";
const OPERATOR_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha.operator.http-request.network.v1\0";
const RESPONSE_MAX_BYTES: u64 = 4 * 1024 * 1024;
const STATUS_EXPORT_DEFAULT_RECORDS: u32 = 100;
const STATUS_EXPORT_MAX_RECORDS: u32 = 1_000;
const OPERATOR_KEY_CONTEXT: &str = "sorafs_cli pdp operator signing";
struct CommonArgs {
    torii_url: String,
    network_id: NetworkId,
    operator_key_path: PathBuf,
}
struct OperatorAuth {
    network_id: NetworkId,
    key_pair: KeyPair,
}
struct ResponseBytes {
    status: StatusCode,
    content_type: Option<bool>,
    content_length: Option<u64>,
    body: Vec<u8>,
}
struct SubmittedProofBinding {
    digest: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    epoch_id: u64,
}
struct ValidatedStatus {
    sequence: u64,
    challenge_id: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    epoch_id: u64,
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
type OutputFileIdentity = (u64, u64);
#[cfg(unix)]
type InputFileIdentity = (u64, u64);
#[cfg(windows)]
type InputFileIdentity = (u32, u64);
#[cfg(not(any(unix, windows)))]
type InputFileIdentity = ();
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
type OutputParentIdentity = (u64, u64, u32, u32, u32);
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
struct OutputParentSnapshot {
    path: PathBuf,
    identity: OutputParentIdentity,
}
pub(super) fn run(mut raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(usage());
    }
    let operation = raw_args.remove(0);
    match operation.as_str() {
        "enqueue" => enqueue(raw_args),
        "next" => next(raw_args),
        "submit" => submit(raw_args),
        "status" => status(raw_args),
        "export" => export(raw_args),
        _ => Err(usage()),
    }
}
fn usage() -> String {
    "Usage:
  sorafs_cli pdp enqueue --torii-url=HTTPS_ORIGIN --network-id=GENESIS_HASH --operator-private-key-file=PATH --commitment=PATH --challenge=PATH --expected-epoch-id=N
  sorafs_cli pdp next --torii-url=HTTPS_ORIGIN --network-id=GENESIS_HASH --operator-private-key-file=PATH --provider-id-hex=HEX32 --challenge-out=PATH
  sorafs_cli pdp submit --torii-url=HTTPS_ORIGIN --network-id=GENESIS_HASH --operator-private-key-file=PATH --challenge-id-hex=HEX32 --proof=PATH
  sorafs_cli pdp status --torii-url=HTTPS_ORIGIN --network-id=GENESIS_HASH --operator-private-key-file=PATH --challenge-id-hex=HEX32
  sorafs_cli pdp export --torii-url=HTTPS_ORIGIN --network-id=GENESIS_HASH --operator-private-key-file=PATH --out=PATH [--after-sequence=N] [--limit=1..1000]"
        .to_owned()
}
fn enqueue(raw_args: Vec<String>) -> Result<(), String> {
    let mut options = parse_options(
        raw_args,
        "enqueue",
        &[
            "torii-url",
            "network-id",
            "operator-private-key-file",
            "commitment",
            "challenge",
            "expected-epoch-id",
        ],
    )?;
    let common = take_common(&mut options, "enqueue")?;
    let commitment_path = PathBuf::from(take_required(&mut options, "commitment", "enqueue")?);
    let challenge_path = PathBuf::from(take_required(&mut options, "challenge", "enqueue")?);
    let expected_epoch_id = super::parse_u64_arg(
        "--expected-epoch-id",
        &take_required(&mut options, "expected-epoch-id", "enqueue")?,
        "sorafs_cli pdp enqueue",
    )?;
    if expected_epoch_id == 0 {
        return Err("`--expected-epoch-id` must be non-zero".to_owned());
    }
    ensure_consumed(options, "enqueue")?;
    let commitment = load_commitment(&commitment_path)?;
    let (challenge, challenge_bytes) = load_challenge(&challenge_path)?;
    validate_challenge_binding(&commitment, &challenge)?;
    if challenge.epoch_id != expected_epoch_id {
        return Err(
            "`--expected-epoch-id` does not match the canonical challenge epoch".to_owned(),
        );
    }
    let commitment_bytes = to_bytes(&commitment)
        .map_err(|_| "failed to encode the validated PDP commitment".to_owned())?;
    let mut request = Map::new();
    request.insert(
        "commitment_b64".into(),
        Value::from(BASE64_STANDARD.encode(commitment_bytes)),
    );
    request.insert(
        "challenge_b64".into(),
        Value::from(BASE64_STANDARD.encode(challenge_bytes)),
    );
    request.insert("expected_epoch_id".into(), Value::from(expected_epoch_id));
    let response = send_request(&common, ROUTE_ENQUEUE, Value::Object(request))?;
    let value = require_json_ok(response, "PDP challenge enqueue")?;
    validate_enqueue_response(&value, &challenge)?;
    emit_json(&value)
}
fn next(raw_args: Vec<String>) -> Result<(), String> {
    let mut options = parse_options(
        raw_args,
        "next",
        &[
            "torii-url",
            "network-id",
            "operator-private-key-file",
            "provider-id-hex",
            "challenge-out",
        ],
    )?;
    let common = take_common(&mut options, "next")?;
    let provider_id = take_hex32(&mut options, "provider-id-hex", "next")?;
    let challenge_out = PathBuf::from(take_required(&mut options, "challenge-out", "next")?);
    ensure_consumed(options, "next")?;
    validate_new_output_path(&challenge_out, "`--challenge-out`")?;
    let mut request = Map::new();
    request.insert(
        "provider_id_hex".into(),
        Value::from(hex::encode(provider_id)),
    );
    let response = send_request(&common, ROUTE_NEXT, Value::Object(request))?;
    if response.status == StatusCode::NO_CONTENT {
        if !response.body.is_empty()
            || response.content_type.is_some()
            || response.content_length.is_some_and(|length| length != 0)
        {
            return Err("Torii PDP next response used a non-canonical 204 projection".to_owned());
        }
        let mut summary = Map::new();
        summary.insert("result".into(), Value::from("empty"));
        return emit_json(&Value::Object(summary));
    }
    let value = require_json_ok(response, "PDP next challenge")?;
    let challenge_bytes = validate_next_response(&value, &provider_id)?;
    write_create_new(&challenge_out, &challenge_bytes, "PDP challenge output")?;
    let object = value
        .as_object()
        .ok_or_else(|| "validated PDP next response stopped being an object".to_owned())?;
    let mut summary = Map::new();
    summary.insert("result".into(), Value::from("challenge"));
    summary.insert(
        "sequence".into(),
        object
            .get("sequence")
            .cloned()
            .ok_or_else(|| "validated PDP next response lost `sequence`".to_owned())?,
    );
    summary.insert(
        "challenge_id_hex".into(),
        object
            .get("challenge_id_hex")
            .cloned()
            .ok_or_else(|| "validated PDP next response lost `challenge_id_hex`".to_owned())?,
    );
    summary.insert(
        "enqueued_at_unix".into(),
        object
            .get("enqueued_at_unix")
            .cloned()
            .ok_or_else(|| "validated PDP next response lost `enqueued_at_unix`".to_owned())?,
    );
    summary.insert(
        "challenge_out".into(),
        Value::from(challenge_out.display().to_string()),
    );
    emit_json(&Value::Object(summary))
}
fn submit(raw_args: Vec<String>) -> Result<(), String> {
    let mut options = parse_options(
        raw_args,
        "submit",
        &[
            "torii-url",
            "network-id",
            "operator-private-key-file",
            "challenge-id-hex",
            "proof",
        ],
    )?;
    let common = take_common(&mut options, "submit")?;
    let challenge_id = take_hex32(&mut options, "challenge-id-hex", "submit")?;
    let proof_path = PathBuf::from(take_required(&mut options, "proof", "submit")?);
    ensure_consumed(options, "submit")?;
    let (proof, proof_bytes) = load_proof(&proof_path)?;
    if proof.challenge_id != challenge_id {
        return Err("`--challenge-id-hex` does not match the canonical PDP proof".to_owned());
    }
    let proof_binding = SubmittedProofBinding {
        digest: proof
            .proof_digest()
            .map_err(|_| "failed to derive the submitted PDP proof digest".to_owned())?,
        manifest_digest: proof.manifest_digest,
        provider_id: proof.provider_id,
        epoch_id: proof.epoch_id,
    };
    let mut request = Map::new();
    request.insert(
        "challenge_id_hex".into(),
        Value::from(hex::encode(challenge_id)),
    );
    request.insert(
        "proof_b64".into(),
        Value::from(BASE64_STANDARD.encode(proof_bytes)),
    );
    let value = require_json_ok(
        send_request(&common, ROUTE_SUBMIT, Value::Object(request))?,
        "PDP proof submission",
    )?;
    validate_status_response(&value, Some(&challenge_id), Some(&proof_binding))?;
    emit_json(&value)
}
fn status(raw_args: Vec<String>) -> Result<(), String> {
    let mut options = parse_options(
        raw_args,
        "status",
        &[
            "torii-url",
            "network-id",
            "operator-private-key-file",
            "challenge-id-hex",
        ],
    )?;
    let common = take_common(&mut options, "status")?;
    let challenge_id = take_hex32(&mut options, "challenge-id-hex", "status")?;
    ensure_consumed(options, "status")?;
    let mut request = Map::new();
    request.insert(
        "challenge_id_hex".into(),
        Value::from(hex::encode(challenge_id)),
    );
    let value = require_json_ok(
        send_request(&common, ROUTE_STATUS, Value::Object(request))?,
        "PDP challenge status",
    )?;
    validate_status_response(&value, Some(&challenge_id), None)?;
    emit_json(&value)
}
fn export(raw_args: Vec<String>) -> Result<(), String> {
    let mut options = parse_options(
        raw_args,
        "export",
        &[
            "torii-url",
            "network-id",
            "operator-private-key-file",
            "after-sequence",
            "limit",
            "out",
        ],
    )?;
    let common = take_common(&mut options, "export")?;
    let after_sequence = options
        .remove("after-sequence")
        .map(|raw| super::parse_u64_arg("--after-sequence", &raw, "sorafs_cli pdp export"))
        .transpose()?
        .unwrap_or(0);
    let limit = options
        .remove("limit")
        .map(|raw| super::parse_u32_arg("--limit", &raw, "sorafs_cli pdp export"))
        .transpose()?
        .unwrap_or(STATUS_EXPORT_DEFAULT_RECORDS);
    if !(1..=STATUS_EXPORT_MAX_RECORDS).contains(&limit) {
        return Err(format!(
            "`--limit` must be between 1 and {STATUS_EXPORT_MAX_RECORDS}"
        ));
    }
    let out = PathBuf::from(take_required(&mut options, "out", "export")?);
    ensure_consumed(options, "export")?;
    validate_new_output_path(&out, "`--out`")?;
    let mut request = Map::new();
    request.insert("after_sequence".into(), Value::from(after_sequence));
    request.insert("limit".into(), Value::from(limit));
    let value = require_json_ok(
        send_request(&common, ROUTE_EXPORT, Value::Object(request))?,
        "PDP status export",
    )?;
    let item_count = validate_export_response(&value, after_sequence, limit)?;
    let next_sequence = value
        .get("next_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| "validated PDP export lost `next_sequence`".to_owned())?;
    let mut rendered = to_string_pretty(&value)
        .map_err(|_| "failed to render validated PDP export JSON".to_owned())?
        .into_bytes();
    rendered.push(b'\n');
    write_create_new(&out, &rendered, "PDP status export")?;
    let mut summary = Map::new();
    summary.insert("item_count".into(), Value::from(item_count as u64));
    summary.insert("next_sequence".into(), Value::from(next_sequence));
    summary.insert("out".into(), Value::from(out.display().to_string()));
    emit_json(&Value::Object(summary))
}
fn parse_options(
    raw_args: Vec<String>,
    operation: &str,
    allowed: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    let mut options = BTreeMap::new();
    for argument in raw_args {
        let option = argument.strip_prefix("--").ok_or_else(|| {
            format!("unexpected positional argument for `sorafs_cli pdp {operation}`")
        })?;
        let (name, value) = option
            .split_once('=')
            .ok_or_else(|| format!("option `--{option}` must use the exact `--name=value` form"))?;
        if !allowed.contains(name) {
            return Err(format!(
                "unsupported option `--{name}` for `sorafs_cli pdp {operation}`"
            ));
        }
        if value.is_empty() {
            return Err(format!("`--{name}` must not be empty"));
        }
        if options.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!(
                "duplicate `--{name}` for `sorafs_cli pdp {operation}`"
            ));
        }
    }
    Ok(options)
}
fn take_common(
    options: &mut BTreeMap<String, String>,
    operation: &str,
) -> Result<CommonArgs, String> {
    let torii_url = take_required(options, "torii-url", operation)?;
    let network_literal = take_required(options, "network-id", operation)?;
    let network_id = network_literal
        .parse::<NetworkId>()
        .map_err(|_| "`--network-id` must be a canonical genesis hash".to_owned())?;
    if network_id.to_string() != network_literal {
        return Err("`--network-id` must use its exact canonical text form".to_owned());
    }
    let operator_key_path = PathBuf::from(take_required(
        options,
        "operator-private-key-file",
        operation,
    )?);
    Ok(CommonArgs {
        torii_url,
        network_id,
        operator_key_path,
    })
}
fn take_required(
    options: &mut BTreeMap<String, String>,
    name: &str,
    operation: &str,
) -> Result<String, String> {
    options
        .remove(name)
        .ok_or_else(|| format!("missing `--{name}` for `sorafs_cli pdp {operation}`"))
}
fn take_hex32(
    options: &mut BTreeMap<String, String>,
    name: &str,
    operation: &str,
) -> Result<[u8; 32], String> {
    let value = take_required(options, name, operation)?;
    let bytes = super::parse_fixed_hex_bytes::<32>(&value, name)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("`--{name}` must be non-zero"));
    }
    Ok(bytes)
}
fn ensure_consumed(options: BTreeMap<String, String>, operation: &str) -> Result<(), String> {
    if options.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "internal option parser left unconsumed fields for `sorafs_cli pdp {operation}`"
        ))
    }
}
fn pdp_decode_limits(maximum: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        maximum.max(1),
        maximum,
        maximum,
        maximum.saturating_mul(4),
        64,
    )
}
fn load_commitment(path: &Path) -> Result<PdpCommitmentV1, String> {
    let maximum = u64::try_from(PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1)
        .map_err(|_| "PDP commitment byte cap does not fit u64".to_owned())?;
    let bytes = read_input_file_bounded(path, maximum, "PDP commitment")?;
    let value: PdpCommitmentV1 = norito::decode_from_bytes_with_limits(
        &bytes,
        pdp_decode_limits(PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1),
    )
    .map_err(|_| "PDP commitment is not valid bounded Norito".to_owned())?;
    value
        .validate()
        .map_err(|_| "PDP commitment failed V1 validation".to_owned())?;
    require_canonical_norito(&bytes, &value, "PDP commitment")?;
    Ok(value)
}
fn load_challenge(path: &Path) -> Result<(PdpChallengeV1, Vec<u8>), String> {
    let maximum = u64::try_from(PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1)
        .map_err(|_| "PDP challenge byte cap does not fit u64".to_owned())?;
    let bytes = read_input_file_bounded(path, maximum, "PDP challenge")?;
    let value = decode_challenge_bytes(&bytes)?;
    Ok((value, bytes))
}
fn decode_challenge_bytes(bytes: &[u8]) -> Result<PdpChallengeV1, String> {
    if bytes.is_empty() || bytes.len() > PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1 {
        return Err("PDP challenge exceeds its canonical byte bounds".to_owned());
    }
    let value: PdpChallengeV1 = norito::decode_from_bytes_with_limits(
        bytes,
        pdp_decode_limits(PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1),
    )
    .map_err(|_| "PDP challenge is not valid bounded Norito".to_owned())?;
    value
        .validate()
        .map_err(|_| "PDP challenge failed V1 validation".to_owned())?;
    require_canonical_norito(bytes, &value, "PDP challenge")?;
    Ok(value)
}
fn load_proof(path: &Path) -> Result<(PdpProofV1, Vec<u8>), String> {
    let maximum = u64::try_from(PDP_PROOF_MAX_CANONICAL_BYTES_V1)
        .map_err(|_| "PDP proof byte cap does not fit u64".to_owned())?;
    let bytes = read_input_file_bounded(path, maximum, "PDP proof")?;
    let value: PdpProofV1 = norito::decode_from_bytes_with_limits(
        &bytes,
        pdp_decode_limits(PDP_PROOF_MAX_CANONICAL_BYTES_V1),
    )
    .map_err(|_| "PDP proof is not valid bounded Norito".to_owned())?;
    value
        .validate()
        .map_err(|_| "PDP proof failed V1 validation".to_owned())?;
    value
        .verify_signature()
        .map_err(|_| "PDP proof signature verification failed".to_owned())?;
    require_canonical_norito(&bytes, &value, "PDP proof")?;
    Ok((value, bytes))
}
fn read_input_file_bounded(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
    validate_input_metadata(&before, maximum, label, path)?;
    let expected_len =
        usize::try_from(before.len()).map_err(|_| format!("{label} length does not fit usize"))?;
    let mut file = open_input_file(path, label)?;
    let opened = file
        .metadata()
        .map_err(|error| format!("failed to inspect opened {label}: {error}"))?;
    validate_input_metadata(&opened, maximum, label, path)?;
    if !input_metadata_unchanged(&before, &opened) {
        return Err(format!("{label} changed between inspection and open"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|error| format!("failed to reserve bounded {label} storage: {error}"))?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            format!("{label} changed length while being read")
        } else {
            format!("failed to read {label}: {error}")
        }
    })?;
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|error| format!("failed to finish reading {label}: {error}"))?
        != 0
    {
        return Err(format!("{label} changed length while being read"));
    }
    let after_file = file
        .metadata()
        .map_err(|error| format!("failed to re-inspect opened {label}: {error}"))?;
    let after_path = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to re-inspect {label}: {error}"))?;
    validate_input_metadata(&after_file, maximum, label, path)?;
    validate_input_metadata(&after_path, maximum, label, path)?;
    if !input_metadata_unchanged(&opened, &after_file)
        || !input_metadata_unchanged(&opened, &after_path)
        || after_file.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(format!("{label} changed while being read"));
    }
    Ok(bytes)
}
fn validate_input_metadata(
    metadata: &fs::Metadata,
    maximum: u64,
    label: &str,
    path: &Path,
) -> Result<(), String> {
    if input_file_is_indirect(metadata)
        || !metadata.file_type().is_file()
        || input_file_identity(metadata).is_none()
    {
        return Err(format!(
            "{label} `{}` must be a regular non-symlink file with a stable identity",
            path.display()
        ));
    }
    if !input_file_is_single_link(metadata) {
        return Err(format!(
            "{label} `{}` must have exactly one hard link",
            path.display()
        ));
    }
    #[cfg(unix)]
    if metadata.mode() & 0o022 != 0 {
        return Err(format!(
            "{label} `{}` must not grant group or world write permission",
            path.display()
        ));
    }
    if metadata.len() == 0 || metadata.len() > maximum {
        return Err(format!(
            "{label} `{}` must contain between 1 and {maximum} bytes",
            path.display()
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn input_file_identity(metadata: &fs::Metadata) -> Option<InputFileIdentity> {
    Some((metadata.dev(), metadata.ino()))
}
#[cfg(windows)]
fn input_file_identity(metadata: &fs::Metadata) -> Option<InputFileIdentity> {
    Some((metadata.volume_serial_number()?, metadata.file_index()?))
}
#[cfg(not(any(unix, windows)))]
fn input_file_identity(_metadata: &fs::Metadata) -> Option<InputFileIdentity> {
    None
}
#[cfg(windows)]
fn input_file_is_reparse_point(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn input_file_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
fn input_file_is_indirect(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || input_file_is_reparse_point(metadata)
}
#[cfg(unix)]
fn input_file_is_single_link(metadata: &fs::Metadata) -> bool {
    metadata.nlink() == 1
}
#[cfg(windows)]
fn input_file_is_single_link(metadata: &fs::Metadata) -> bool {
    metadata.number_of_links() == Some(1)
}
#[cfg(not(any(unix, windows)))]
fn input_file_is_single_link(_metadata: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn input_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    input_file_identity(left) == input_file_identity(right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
}
#[cfg(windows)]
fn input_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    input_file_identity(left).is_some()
        && input_file_identity(left) == input_file_identity(right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}
#[cfg(not(any(unix, windows)))]
fn input_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn open_input_file(path: &Path, label: &str) -> Result<fs::File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    super::set_no_follow_flag(&mut options);
    options
        .open(path)
        .map_err(|error| format!("failed to securely open {label}: {error}"))
}
#[cfg(windows)]
fn open_input_file(path: &Path, label: &str) -> Result<fs::File, String> {
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(0x0000_0001)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options
        .open(path)
        .map_err(|error| format!("failed to securely open {label}: {error}"))
}
#[cfg(not(any(unix, windows)))]
fn open_input_file(_path: &Path, label: &str) -> Result<fs::File, String> {
    Err(format!(
        "this platform does not expose the stable file identity required for {label}"
    ))
}
fn require_canonical_norito<T>(bytes: &[u8], value: &T, label: &str) -> Result<(), String>
where
    T: norito::core::NoritoSerialize,
{
    let canonical =
        to_bytes(value).map_err(|_| format!("failed to canonically re-encode {label}"))?;
    if canonical != bytes {
        return Err(format!("{label} is not canonically encoded"));
    }
    Ok(())
}
fn validate_challenge_binding(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
) -> Result<(), String> {
    let commitment_digest = commitment
        .commitment_digest()
        .map_err(|_| "failed to derive the PDP commitment digest".to_owned())?;
    if challenge.commitment_digest != commitment_digest
        || challenge.manifest_digest != commitment.manifest_digest
        || challenge.chunk_profile != commitment.chunk_profile
        || challenge.samples.len() > usize::from(commitment.sample_window)
        || commitment.sealed_at > challenge.issued_at_unix
    {
        return Err("PDP challenge is not bound to the supplied commitment".to_owned());
    }
    Ok(())
}
fn endpoint(torii_url: &str, route: &str) -> Result<Url, String> {
    if torii_url.is_empty() || torii_url.trim() != torii_url {
        return Err("`--torii-url` must be an exact canonical URL without padding".to_owned());
    }
    let parsed =
        Url::parse(torii_url).map_err(|_| "`--torii-url` must be a valid URL".to_owned())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "`--torii-url` must include a host".to_owned())?;
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && is_loopback) {
        return Err(
            "`--torii-url` must use HTTPS; HTTP is permitted only for loopback fixtures".to_owned(),
        );
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("`--torii-url` must not include userinfo".to_owned());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("`--torii-url` must not include a query or fragment".to_owned());
    }
    if parsed.port() == Some(0) {
        return Err("`--torii-url` must not use port zero".to_owned());
    }
    let canonical_origin = parsed.origin().ascii_serialization();
    let canonical_origin_with_slash = format!("{canonical_origin}/");
    if parsed.path() != "/"
        || (torii_url != canonical_origin && torii_url != canonical_origin_with_slash)
    {
        return Err(
            "`--torii-url` must be an exact canonical bare origin without a path prefix".to_owned(),
        );
    }
    parsed
        .join(route.trim_start_matches('/'))
        .map_err(|_| "failed to build the PDP endpoint URL".to_owned())
}
fn load_auth(common: &CommonArgs) -> Result<OperatorAuth, String> {
    let private_key =
        super::load_reputation_auth_private_key(&common.operator_key_path, OPERATOR_KEY_CONTEXT)
            .map_err(|error| error.replace("reputation authentication", "PDP operator"))?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|_| "failed to derive the PDP operator public key".to_owned())?;
    if key_pair.public_key().try_algorithm() != Ok(Algorithm::Ed25519) {
        return Err("PDP operator signing requires an Ed25519 private key".to_owned());
    }
    Ok(OperatorAuth {
        network_id: common.network_id,
        key_pair,
    })
}
fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(Policy::none())
        .referer(false)
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .no_proxy()
        .build()
        .map_err(|_| "failed to construct the hardened PDP HTTP client".to_owned())
}
fn send_request(common: &CommonArgs, route: &str, request: Value) -> Result<ResponseBytes, String> {
    let endpoint = endpoint(&common.torii_url, route)?;
    if endpoint.path() != route || endpoint.query().is_some() {
        return Err("constructed PDP endpoint is not the exact catalog route".to_owned());
    }
    let body = to_vec(&request).map_err(|_| "failed to encode the PDP request JSON".to_owned())?;
    let auth = load_auth(common)?;
    let headers = signed_headers(&auth, route, &body)?;
    let response = http_client()?
        .post(endpoint)
        .header(ACCEPT, "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .header(CONTENT_TYPE, "application/json")
        .header(HEADER_OPERATOR_PUBLIC_KEY, headers.public_key)
        .header(HEADER_OPERATOR_TIMESTAMP_MS, headers.timestamp_ms)
        .header(HEADER_OPERATOR_NONCE, headers.nonce)
        .header(HEADER_OPERATOR_SIGNATURE, headers.signature)
        .body(body)
        .send()
        .map_err(|_| "authenticated PDP request failed".to_owned())?;
    read_response(response)
}
struct SignedHeaders {
    public_key: String,
    timestamp_ms: String,
    nonce: String,
    signature: String,
}
fn signed_headers(auth: &OperatorAuth, route: &str, body: &[u8]) -> Result<SignedHeaders, String> {
    use rand::rand_core::TryRngCore as _;
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch; cannot sign PDP request".to_owned())?;
    let timestamp_ms = u64::try_from(elapsed.as_millis())
        .map_err(|_| "system clock does not fit the PDP operator timestamp".to_owned())?;
    let mut nonce_bytes = [0_u8; 12];
    rand::rngs::OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|_| "OS RNG failed while signing PDP request".to_owned())?;
    let nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
    let canonical_request = format!("POST\n{route}\n\n{}", hex::encode(Sha256::digest(body)));
    let mut message = Vec::with_capacity(
        OPERATOR_SIGNATURE_DOMAIN_V1.len()
            + auth.network_id.as_bytes().len()
            + canonical_request.len()
            + nonce.len()
            + 32,
    );
    message.extend_from_slice(OPERATOR_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(auth.network_id.as_bytes());
    message.extend_from_slice(canonical_request.as_bytes());
    message.push(b'\n');
    message.extend_from_slice(timestamp_ms.to_string().as_bytes());
    message.push(b'\n');
    message.extend_from_slice(nonce.as_bytes());
    let signature = Signature::try_new(auth.key_pair.private_key(), &message)
        .map_err(|_| "failed to sign the PDP operator request".to_owned())?;
    Ok(SignedHeaders {
        public_key: auth.key_pair.public_key().to_string(),
        timestamp_ms: timestamp_ms.to_string(),
        nonce,
        signature: BASE64_STANDARD.encode(signature.payload()),
    })
}
fn read_response(response: Response) -> Result<ResponseBytes, String> {
    let status = response.status();
    let content_type = {
        let mut values = response.headers().get_all(CONTENT_TYPE).iter();
        match values.next() {
            None => None,
            Some(value) => Some(value.as_bytes() == b"application/json" && values.next().is_none()),
        }
    };
    let mut encodings = response.headers().get_all(CONTENT_ENCODING).iter();
    let identity_only = match encodings.next() {
        None => true,
        Some(value) => value.as_bytes() == b"identity" && encodings.next().is_none(),
    };
    if !identity_only {
        return Err("Torii PDP response must use identity content encoding".to_owned());
    }
    let mut lengths = response.headers().get_all(CONTENT_LENGTH).iter();
    let content_length = lengths
        .next()
        .map(|value| parse_content_length(value.as_bytes()))
        .transpose()?;
    if lengths.next().is_some() {
        return Err(
            "Torii PDP response must not contain duplicate Content-Length headers".to_owned(),
        );
    }
    if content_length.is_some_and(|length| length > RESPONSE_MAX_BYTES) {
        return Err(format!(
            "Torii PDP response declared more than {RESPONSE_MAX_BYTES} bytes"
        ));
    }
    let capacity = usize::try_from(content_length.unwrap_or(0))
        .map_err(|_| "Torii PDP response length does not fit usize".to_owned())?;
    let mut body = Vec::new();
    body.try_reserve_exact(capacity)
        .map_err(|_| "failed to reserve bounded Torii PDP response storage".to_owned())?;
    response
        .take(RESPONSE_MAX_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|_| "failed to read the bounded Torii PDP response".to_owned())?;
    let body_len = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if body_len > RESPONSE_MAX_BYTES {
        return Err(format!(
            "Torii PDP response exceeded {RESPONSE_MAX_BYTES} bytes"
        ));
    }
    if content_length.is_some_and(|length| length != body_len) {
        return Err("Torii PDP response body length did not match Content-Length".to_owned());
    }
    Ok(ResponseBytes {
        status,
        content_type,
        content_length,
        body,
    })
}
fn parse_content_length(raw: &[u8]) -> Result<u64, String> {
    if raw.is_empty()
        || !raw.iter().all(u8::is_ascii_digit)
        || raw.len() > 1 && raw.starts_with(b"0")
    {
        return Err(
            "Torii PDP response Content-Length must be canonical unsigned decimal".to_owned(),
        );
    }
    std::str::from_utf8(raw)
        .map_err(|_| "Torii PDP response Content-Length must be ASCII".to_owned())?
        .parse::<u64>()
        .map_err(|_| "Torii PDP response Content-Length does not fit u64".to_owned())
}
fn require_json_ok(response: ResponseBytes, context: &str) -> Result<Value, String> {
    if response.status != StatusCode::OK {
        return Err(format!(
            "Torii {context} endpoint returned {}",
            response.status
        ));
    }
    if response.content_type != Some(true) {
        return Err(format!(
            "Torii {context} response must use canonical Content-Type application/json"
        ));
    }
    from_slice(&response.body).map_err(|_| format!("Torii {context} response is not valid JSON"))
}
fn validate_enqueue_response(value: &Value, challenge: &PdpChallengeV1) -> Result<(), String> {
    let object = exact_object(
        value,
        &["result", "sequence", "challenge_id_hex"],
        &[],
        "PDP enqueue response",
    )?;
    let result = required_string(object, "result", "PDP enqueue response")?;
    if !matches!(result, "inserted" | "existing") {
        return Err("PDP enqueue response has an unknown `result`".to_owned());
    }
    required_nonzero_u64(object, "sequence", "PDP enqueue response")?;
    let challenge_id = required_hex32(object, "challenge_id_hex", "PDP enqueue response")?;
    if challenge_id != challenge.challenge_id {
        return Err("PDP enqueue response challenge ID does not match the request".to_owned());
    }
    Ok(())
}
fn validate_next_response(value: &Value, provider_id: &[u8; 32]) -> Result<Vec<u8>, String> {
    let object = exact_object(
        value,
        &[
            "sequence",
            "challenge_id_hex",
            "challenge_b64",
            "enqueued_at_unix",
        ],
        &[],
        "PDP next response",
    )?;
    required_nonzero_u64(object, "sequence", "PDP next response")?;
    required_nonzero_u64(object, "enqueued_at_unix", "PDP next response")?;
    let challenge_id = required_hex32(object, "challenge_id_hex", "PDP next response")?;
    let encoded = required_string(object, "challenge_b64", "PDP next response")?;
    let bytes = decode_canonical_base64(
        encoded,
        PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
        "PDP next response `challenge_b64`",
    )?;
    let challenge = decode_challenge_bytes(&bytes)?;
    let enqueued_at_unix = object
        .get("enqueued_at_unix")
        .and_then(Value::as_u64)
        .ok_or_else(|| "PDP next response `enqueued_at_unix` must be a u64".to_owned())?;
    if challenge.challenge_id != challenge_id
        || &challenge.provider_id != provider_id
        || enqueued_at_unix > challenge.response_deadline_unix
    {
        return Err("PDP next response challenge binding does not match the request".to_owned());
    }
    Ok(bytes)
}
fn validate_status_response(
    value: &Value,
    expected_id: Option<&[u8; 32]>,
    submitted_proof: Option<&SubmittedProofBinding>,
) -> Result<ValidatedStatus, String> {
    let object = exact_object(
        value,
        &[
            "sequence",
            "challenge_id_hex",
            "manifest_digest_hex",
            "provider_id_hex",
            "epoch_id",
            "lifecycle",
        ],
        &[
            "response_deadline_unix",
            "proof_digest_hex",
            "decision",
            "rejection_reason",
        ],
        "PDP status response",
    )?;
    let sequence = required_nonzero_u64(object, "sequence", "PDP status response")?;
    let challenge_id = required_hex32(object, "challenge_id_hex", "PDP status response")?;
    let manifest_digest = required_hex32(object, "manifest_digest_hex", "PDP status response")?;
    let provider_id = required_hex32(object, "provider_id_hex", "PDP status response")?;
    let epoch_id = required_nonzero_u64(object, "epoch_id", "PDP status response")?;
    if expected_id.is_some_and(|expected| expected != &challenge_id) {
        return Err("PDP status response challenge ID does not match the request".to_owned());
    }
    let lifecycle = required_string(object, "lifecycle", "PDP status response")?;
    let deadline = optional_nonzero_u64(object, "response_deadline_unix", "PDP status response")?;
    let has_proof = object.contains_key("proof_digest_hex");
    let proof_digest = if has_proof {
        Some(required_hex32(
            object,
            "proof_digest_hex",
            "PDP status response",
        )?)
    } else {
        None
    };
    if submitted_proof.is_some_and(|expected| {
        proof_digest
            .as_ref()
            .is_some_and(|observed| observed != &expected.digest)
    }) {
        return Err("PDP status response proof digest does not match the submission".to_owned());
    }
    let decision = optional_string(object, "decision", "PDP status response")?;
    let rejection = optional_string(object, "rejection_reason", "PDP status response")?;
    if submitted_proof.is_some() && lifecycle != "terminal" {
        return Err("PDP proof submission response must be terminal".to_owned());
    }
    match lifecycle {
        "pending"
            if deadline.is_some() && decision.is_none() && rejection.is_none() && !has_proof => {}
        "handoff_pending" if deadline.is_some() && decision.is_some() => {}
        "terminal" if deadline.is_none() && decision.is_some() => {}
        "pending" | "handoff_pending" | "terminal" => {
            return Err("PDP status response has an inconsistent lifecycle projection".to_owned());
        }
        _ => return Err("PDP status response has an unknown lifecycle".to_owned()),
    }
    match decision {
        None if rejection.is_none() && !has_proof => {}
        Some("accepted") if rejection.is_none() && has_proof => {
            if submitted_proof.is_some_and(|expected| {
                manifest_digest != expected.manifest_digest
                    || provider_id != expected.provider_id
                    || epoch_id != expected.epoch_id
            }) {
                return Err(
                    "accepted PDP submission response does not match the submitted proof scope"
                        .to_owned(),
                );
            }
        }
        Some("rejected") => match rejection {
            Some("submission_late" | "future_timestamp") if has_proof => {}
            Some(
                "deadline_expired"
                | "admission_revoked"
                | "admission_inactive"
                | "storage_unavailable",
            ) if !has_proof => {}
            Some("invalid_proof") => {}
            _ => {
                return Err(
                    "PDP status response has an invalid proof/rejection projection".to_owned(),
                );
            }
        },
        _ => return Err("PDP status response has an invalid terminal decision".to_owned()),
    }
    if submitted_proof.is_some()
        && !matches!(
            (decision, rejection, has_proof),
            (Some("accepted"), None, true)
                | (
                    Some("rejected"),
                    Some("submission_late" | "future_timestamp"),
                    true
                )
                | (Some("rejected"), Some("invalid_proof"), _)
                | (Some("rejected"), Some("admission_revoked"), false)
        )
    {
        return Err("PDP proof submission response has an invalid terminal outcome".to_owned());
    }
    Ok(ValidatedStatus {
        sequence,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id,
    })
}
fn validate_export_response(
    value: &Value,
    after_sequence: u64,
    limit: u32,
) -> Result<usize, String> {
    let object = exact_object(
        value,
        &["items", "next_sequence"],
        &[],
        "PDP export response",
    )?;
    let items = object
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "PDP export response `items` must be an array".to_owned())?;
    let limit =
        usize::try_from(limit).map_err(|_| "PDP export limit does not fit usize".to_owned())?;
    if items.len() > limit || items.len() > STATUS_EXPORT_MAX_RECORDS as usize {
        return Err("PDP export response exceeded the requested record limit".to_owned());
    }
    let mut previous = after_sequence;
    let mut challenge_ids = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    for item in items {
        let status = validate_status_response(item, None, None)?;
        if status.sequence <= previous {
            return Err("PDP export response sequences are not strictly increasing".to_owned());
        }
        if !challenge_ids.insert(status.challenge_id)
            || !scopes.insert((status.provider_id, status.manifest_digest, status.epoch_id))
        {
            return Err(
                "PDP export response contains duplicate challenge IDs or provider scopes"
                    .to_owned(),
            );
        }
        previous = status.sequence;
    }
    let next_sequence = object
        .get("next_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| "PDP export response `next_sequence` must be a u64".to_owned())?;
    if next_sequence != previous {
        return Err("PDP export response `next_sequence` does not match its final item".to_owned());
    }
    Ok(items.len())
}
fn exact_object<'a>(
    value: &'a Value,
    required: &[&str],
    optional: &[&str],
    context: &str,
) -> Result<&'a Map, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be a JSON object"))?;
    for field in required {
        if !object.contains_key(*field) {
            return Err(format!("{context} is missing `{field}`"));
        }
    }
    if let Some(field) = object
        .keys()
        .find(|field| !required.contains(&field.as_str()) && !optional.contains(&field.as_str()))
    {
        return Err(format!("{context} contains unknown field `{field}`"));
    }
    Ok(object)
}
fn required_string<'a>(object: &'a Map, field: &str, context: &str) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} `{field}` must be a string"))
}
fn optional_string<'a>(
    object: &'a Map,
    field: &str,
    context: &str,
) -> Result<Option<&'a str>, String> {
    object
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{context} `{field}` must be a string"))
        })
        .transpose()
}
fn required_nonzero_u64(object: &Map, field: &str, context: &str) -> Result<u64, String> {
    let value = object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} `{field}` must be a u64"))?;
    if value == 0 {
        return Err(format!("{context} `{field}` must be non-zero"));
    }
    Ok(value)
}
fn optional_nonzero_u64(object: &Map, field: &str, context: &str) -> Result<Option<u64>, String> {
    object
        .get(field)
        .map(|value| {
            let value = value
                .as_u64()
                .ok_or_else(|| format!("{context} `{field}` must be a u64"))?;
            if value == 0 {
                return Err(format!("{context} `{field}` must be non-zero"));
            }
            Ok(value)
        })
        .transpose()
}
fn required_hex32(object: &Map, field: &str, context: &str) -> Result<[u8; 32], String> {
    let literal = required_string(object, field, context)?;
    let bytes = super::parse_fixed_hex_bytes::<32>(literal, field)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("{context} `{field}` must be non-zero"));
    }
    Ok(bytes)
}
fn decode_canonical_base64(
    encoded: &str,
    maximum: usize,
    context: &str,
) -> Result<Vec<u8>, String> {
    let encoded_maximum = maximum
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| format!("{context} bound overflowed"))?;
    if encoded.is_empty() || encoded.len() > encoded_maximum {
        return Err(format!("{context} exceeds its encoded byte bound"));
    }
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| format!("{context} is not standard padded base64"))?;
    if bytes.is_empty() || bytes.len() > maximum || BASE64_STANDARD.encode(&bytes) != encoded {
        return Err(format!("{context} is not canonical padded base64"));
    }
    Ok(bytes)
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_new_output_path(path: &Path, field: &str) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(format!(
            "{field} path must not contain `.` or `..` components"
        ));
    }
    super::validate_output_path(path)?;
    let outcome = match fs::symlink_metadata(path) {
        Ok(_) => Err(format!(
            "{field} `{}` already exists; PDP outputs never clobber files",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect {field} `{}`: {error}",
            path.display()
        )),
    };
    outcome?;
    validate_existing_output_parents(path, field)
}
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
)))]
fn validate_new_output_path(_path: &Path, field: &str) -> Result<(), String> {
    Err(format!(
        "{field} output is unavailable because this platform lacks private descriptor-relative creation"
    ))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_existing_output_parents(path: &Path, label: &str) -> Result<(), String> {
    let absolute = absolute_output_path(path, label)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| format!("{label} must have a parent directory"))?;
    for (depth, ancestor) in std::iter::once(parent)
        .chain(parent.ancestors().skip(1))
        .enumerate()
    {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                validate_output_parent_metadata(&metadata, ancestor, label)?;
                if depth == 0 {
                    validate_private_output_parent_metadata(&metadata, ancestor, label)?;
                }
                if output_parent_identity(&metadata).is_none() {
                    return Err(format!(
                        "{label} parent `{}` has no stable platform identity",
                        ancestor.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "{label} parent `{}` must already exist as a private directory",
                    ancestor.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "failed to inspect {label} parent `{}`: {error}",
                    ancestor.display()
                ));
            }
        }
    }
    Ok(())
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn absolute_output_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| format!("failed to resolve {label} path: {error}"))
    }
}
#[cfg(any(target_os = "android", target_os = "linux"))]
const OUTPUT_PARENT_OPEN_FLAGS: i32 = 0o200000 | 0o400000 | 0o2000000;
#[cfg(any(target_os = "android", target_os = "linux"))]
const OUTPUT_CREATE_FLAGS: i32 = 0x1 | 0o100 | 0o200 | 0o400000 | 0o2000000;
#[cfg(any(target_os = "android", target_os = "linux"))]
const OUTPUT_REOPEN_FLAGS: i32 = 0o400000 | 0o2000000;
#[cfg(any(target_os = "ios", target_os = "macos"))]
const OUTPUT_PARENT_OPEN_FLAGS: i32 = 0x100000 | 0x100 | 0x1000000;
#[cfg(any(target_os = "ios", target_os = "macos"))]
const OUTPUT_CREATE_FLAGS: i32 = 0x1 | 0x200 | 0x800 | 0x100 | 0x1000000;
#[cfg(any(target_os = "ios", target_os = "macos"))]
const OUTPUT_REOPEN_FLAGS: i32 = 0x100 | 0x1000000;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
unsafe extern "C" {
    fn geteuid() -> u32;
    fn openat(directory: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn unlinkat(directory: i32, path: *const std::ffi::c_char, flags: i32) -> i32;
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn open_output_parent(snapshot: &OutputParentSnapshot, label: &str) -> Result<fs::File, String> {
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(OUTPUT_PARENT_OPEN_FLAGS);
    let directory = options.open(&snapshot.path).map_err(|error| {
        format!(
            "failed to securely open {label} parent `{}`: {error}",
            snapshot.path.display()
        )
    })?;
    validate_open_output_parent(&directory, snapshot, label)?;
    Ok(directory)
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_open_output_parent(
    directory: &fs::File,
    snapshot: &OutputParentSnapshot,
    label: &str,
) -> Result<(), String> {
    let metadata = directory.metadata().map_err(|error| {
        format!(
            "failed to inspect opened {label} parent `{}`: {error}",
            snapshot.path.display()
        )
    })?;
    validate_output_parent_metadata(&metadata, &snapshot.path, label)?;
    validate_private_output_parent_metadata(&metadata, &snapshot.path, label)?;
    if output_parent_identity(&metadata).as_ref() != Some(&snapshot.identity) {
        return Err(format!(
            "{label} parent `{}` changed before descriptor-relative creation",
            snapshot.path.display()
        ));
    }
    Ok(())
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn cleanup_created_at_then_fail(
    parent: &fs::File,
    file_name: &CString,
    expected_identity: Option<OutputFileIdentity>,
    error: String,
) -> Result<(), String> {
    match remove_created_file_at(parent, file_name, expected_identity) {
        Ok(()) => Err(error),
        Err(cleanup_error) => Err(format!("{error}; {cleanup_error}")),
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn remove_created_file_at(
    parent: &fs::File,
    file_name: &CString,
    expected_identity: Option<OutputFileIdentity>,
) -> Result<(), String> {
    let expected_identity = expected_identity
        .ok_or_else(|| "refusing unverified cleanup of created PDP output".to_owned())?;
    let raw_fd = unsafe {
        // SAFETY: `file_name` is NUL-terminated and `parent` is a live directory
        // descriptor retained from before creation.
        openat(
            parent.as_raw_fd(),
            file_name.as_ptr(),
            OUTPUT_REOPEN_FLAGS,
            0,
        )
    };
    if raw_fd < 0 {
        return Err(format!(
            "failed to re-open created PDP output for identity-guarded cleanup: {}",
            std::io::Error::last_os_error()
        ));
    }
    let linked_file = unsafe {
        // SAFETY: `openat` returned a new owned descriptor on success.
        fs::File::from_raw_fd(raw_fd)
    };
    let metadata = linked_file
        .metadata()
        .map_err(|error| format!("failed to inspect created PDP output before cleanup: {error}"))?;
    if output_file_is_indirect(&metadata)
        || !metadata.is_file()
        || output_file_identity(&metadata) != Some(expected_identity)
        || has_multiple_links(&metadata)
    {
        return Err("refusing cleanup because the created PDP output identity changed".to_owned());
    }
    // The retained parent is current-EUID-owned mode 0700. Processes sharing
    // that EUID are explicitly inside the operator trust boundary; other
    // principals cannot swap this leaf between the identity check and unlink.
    let result = unsafe {
        // SAFETY: the descriptor and NUL-terminated leaf name are live, and
        // the relative entry was identity-checked immediately above.
        unlinkat(parent.as_raw_fd(), file_name.as_ptr(), 0)
    };
    if result != 0 {
        return Err(format!(
            "failed to remove created PDP output through its parent descriptor: {}",
            std::io::Error::last_os_error()
        ));
    }
    parent
        .sync_all()
        .map_err(|error| format!("failed to sync PDP output cleanup: {error}"))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn write_create_new(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    validate_new_output_path(path, label)?;
    let (path, parent_snapshots) = prepare_output_path(path, label)?;
    validate_output_parent_snapshots(&parent_snapshots, label)?;
    let parent_snapshot = parent_snapshots
        .first()
        .ok_or_else(|| format!("{label} has no parent snapshot"))?;
    let parent = open_output_parent(parent_snapshot, label)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{label} must name a file"))?;
    let file_name = CString::new(file_name.as_bytes())
        .map_err(|_| format!("{label} file name contains a NUL byte"))?;
    let raw_fd = unsafe {
        // SAFETY: `file_name` is NUL-terminated, the held descriptor names the
        // validated parent, and all flags/mode values are native constants.
        openat(
            parent.as_raw_fd(),
            file_name.as_ptr(),
            OUTPUT_CREATE_FLAGS,
            0o600,
        )
    };
    if raw_fd < 0 {
        return Err(format!(
            "failed to create {label} `{}`: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe {
        // SAFETY: `openat` returned a new owned descriptor on success.
        fs::File::from_raw_fd(raw_fd)
    };
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect created {label} `{}`: {error}",
            path.display()
        )
    })?;
    let identity = output_file_identity(&opened);
    if output_file_is_indirect(&opened)
        || !opened.is_file()
        || opened.len() != 0
        || has_multiple_links(&opened)
        || output_file_permissions_are_unsafe(&opened)
        || identity.is_none()
    {
        drop(file);
        return cleanup_created_at_then_fail(
            &parent,
            &file_name,
            identity,
            format!(
                "created {label} `{}` is not a unique regular file",
                path.display()
            ),
        );
    }
    let identity = identity.expect("supported Unix output identity is present");
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        let written = file.metadata()?;
        if output_file_identity(&written) != Some(identity)
            || written.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            || has_multiple_links(&written)
            || output_file_permissions_are_unsafe(&written)
        {
            return Err(std::io::Error::other("created output identity changed"));
        }
        Ok(())
    })()
    .map_err(|error| format!("failed to write {label} `{}`: {error}", path.display()));
    drop(file);
    if let Err(error) = result {
        return cleanup_created_at_then_fail(&parent, &file_name, Some(identity), error);
    }
    let after = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return cleanup_created_at_then_fail(
                &parent,
                &file_name,
                Some(identity),
                format!("failed to re-inspect {label} `{}`: {error}", path.display()),
            );
        }
    };
    if output_file_is_indirect(&after)
        || !after.is_file()
        || output_file_identity(&after) != Some(identity)
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || has_multiple_links(&after)
        || output_file_permissions_are_unsafe(&after)
    {
        return cleanup_created_at_then_fail(
            &parent,
            &file_name,
            Some(identity),
            format!("{label} `{}` changed while being written", path.display()),
        );
    }
    if let Err(error) = validate_output_parent_snapshots(&parent_snapshots, label)
        .and_then(|()| validate_open_output_parent(&parent, parent_snapshot, label))
        .and_then(|()| {
            parent
                .sync_all()
                .map_err(|error| format!("failed to sync {label} parent directory: {error}"))
        })
    {
        return cleanup_created_at_then_fail(&parent, &file_name, Some(identity), error);
    }
    Ok(())
}
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
)))]
fn write_create_new(_path: &Path, _bytes: &[u8], label: &str) -> Result<(), String> {
    Err(format!(
        "{label} output is unavailable because this platform lacks private descriptor-relative creation"
    ))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn prepare_output_path(
    path: &Path,
    label: &str,
) -> Result<(PathBuf, Vec<OutputParentSnapshot>), String> {
    let absolute = absolute_output_path(path, label)?;
    if absolute.file_name().is_none() {
        return Err(format!("{label} must name a file"));
    }
    validate_new_output_path(&absolute, label)?;
    let snapshots = snapshot_output_parents(&absolute, label)?;
    Ok((absolute, snapshots))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn snapshot_output_parents(path: &Path, label: &str) -> Result<Vec<OutputParentSnapshot>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{label} must have a parent directory"))?;
    let mut snapshots = Vec::new();
    for (depth, ancestor) in std::iter::once(parent)
        .chain(parent.ancestors().skip(1))
        .enumerate()
    {
        let metadata = fs::symlink_metadata(ancestor).map_err(|error| {
            format!(
                "failed to inspect {label} parent `{}`: {error}",
                ancestor.display()
            )
        })?;
        validate_output_parent_metadata(&metadata, ancestor, label)?;
        if depth == 0 {
            validate_private_output_parent_metadata(&metadata, ancestor, label)?;
        }
        let identity = output_parent_identity(&metadata).ok_or_else(|| {
            format!(
                "{label} parent `{}` has no stable platform identity",
                ancestor.display()
            )
        })?;
        snapshots.push(OutputParentSnapshot {
            path: ancestor.to_owned(),
            identity,
        });
    }
    Ok(snapshots)
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_output_parent_snapshots(
    snapshots: &[OutputParentSnapshot],
    label: &str,
) -> Result<(), String> {
    for (depth, snapshot) in snapshots.iter().enumerate() {
        let metadata = fs::symlink_metadata(&snapshot.path).map_err(|error| {
            format!(
                "failed to re-inspect {label} parent `{}`: {error}",
                snapshot.path.display()
            )
        })?;
        validate_output_parent_metadata(&metadata, &snapshot.path, label)?;
        if depth == 0 {
            validate_private_output_parent_metadata(&metadata, &snapshot.path, label)?;
        }
        if output_parent_identity(&metadata).as_ref() != Some(&snapshot.identity) {
            return Err(format!(
                "{label} parent `{}` changed while writing",
                snapshot.path.display()
            ));
        }
    }
    Ok(())
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_output_parent_metadata(
    metadata: &fs::Metadata,
    path: &Path,
    label: &str,
) -> Result<(), String> {
    if output_file_is_indirect(metadata) || !metadata.is_dir() {
        return Err(format!(
            "{label} parent `{}` must be a real directory",
            path.display()
        ));
    }
    if metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0 {
        return Err(format!(
            "{label} parent `{}` must not be group/world writable unless sticky",
            path.display()
        ));
    }
    Ok(())
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn validate_private_output_parent_metadata(
    metadata: &fs::Metadata,
    path: &Path,
    label: &str,
) -> Result<(), String> {
    let effective_uid = unsafe {
        // SAFETY: `geteuid` has no arguments or memory-safety preconditions.
        geteuid()
    };
    if metadata.uid() != effective_uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(format!(
            "{label} immediate parent `{}` must be current-EUID-owned mode 0700",
            path.display()
        ));
    }
    Ok(())
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn has_multiple_links(metadata: &fs::Metadata) -> bool {
    metadata.nlink() != 1
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn output_file_identity(metadata: &fs::Metadata) -> Option<OutputFileIdentity> {
    Some((metadata.dev(), metadata.ino()))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn output_parent_identity(metadata: &fs::Metadata) -> Option<OutputParentIdentity> {
    Some((
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    ))
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn output_file_is_indirect(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn output_file_permissions_are_unsafe(metadata: &fs::Metadata) -> bool {
    metadata.mode() & 0o077 != 0
}
fn emit_json(value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|_| "failed to render validated PDP response JSON".to_owned())?;
    println!("{rendered}");
    Ok(())
}
