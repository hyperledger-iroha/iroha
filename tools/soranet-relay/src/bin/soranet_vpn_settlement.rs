//! Operator helper for SoraNet VPN settlement artifacts.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Parser, ValueEnum};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{NetworkId, account::AccountAddress, alias_setup::AccountAliasName};
use iroha_primitives::numeric::Quantity;
use norito::{
    DecodeLimits,
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use sha2::{Digest as _, Sha256};
use soranet_relay::{
    config::{read_bounded_direct_regular_file, read_bounded_private_regular_file},
    runtime::VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_tungstenite::tungstenite::http::uri::Authority;

const HEADER_ACCOUNT: &str = "X-Iroha-Account";
const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
const HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
const HEADER_NONCE: &str = "X-Iroha-Nonce";
const RECEIPT_METHOD: &str = "POST";
// This is the exact POST route registered by `iroha_torii` and advertised by
// `iroha_torii_shared::route_catalog`; settlement artifacts may not redirect it.
const RECEIPT_PATH: &str = "/v1/vpn/receipts";
// Keep these private producer-side preflight limits aligned with the canonical
// V1 verifier/client limits in `iroha::client` without pulling that full crate
// into the relay tool.
const CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1: usize = 64;
const CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1: usize = 64 * 1024;
const CANONICAL_REQUEST_MAX_METHOD_BYTES_V1: usize = 32;
const CANONICAL_REQUEST_MAX_PATH_BYTES_V1: usize = 64 * 1024;
const CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1: usize = 36 * 1024;
const CANONICAL_REQUEST_MAX_NONCE_BYTES_V1: usize = 256;
const CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1: usize = Algorithm::MlDsa.signature_payload_len();
const TORII_ROOT_MAX_BYTES_V1: usize = 4 * 1024;
// `NetworkId` is one typed 32-byte hash. The envelope bound applies JSON's
// worst-case six-byte string escape expansion to every bounded string source:
// body, origin, target, account, signature, timestamp, nonce, and fixed header
// text. The structural allowance covers field/object/array punctuation.
const NETWORK_ID_JSON_MAX_BYTES_V1: usize = 128;
const SIGNED_SETTLEMENT_JSON_STRUCTURE_BYTES_V1: usize = 4 * 1024;
const SIGNED_SETTLEMENT_JSON_MAX_BYTES_V1: usize = SIGNED_SETTLEMENT_JSON_STRUCTURE_BYTES_V1
    + 6 * (NETWORK_ID_JSON_MAX_BYTES_V1
        + RECEIPT_METHOD.len()
        + TORII_ROOT_MAX_BYTES_V1
        + 2 * RECEIPT_PATH.len()
        + VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1
        + CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
        + CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1.div_ceil(3) * 4
        + 20
        + CANONICAL_REQUEST_MAX_NONCE_BYTES_V1
        + 256);
const PRIVATE_KEY_SEED_FILE_MAX_BYTES_V1: usize = 128;
const VPN_SETTLEMENT_JSON_MAX_FIELD_BYTES_V1: usize = 32 * 1024;
const VPN_SETTLEMENT_JSON_MAX_TOTAL_STRING_BYTES_V1: usize = 60 * 1024;
const VPN_SETTLEMENT_JSON_MAX_SEQUENCE_ELEMENTS_V1: usize = 32;
const VPN_SETTLEMENT_JSON_MAX_TOTAL_ELEMENTS_V1: usize = 64;
const VPN_SETTLEMENT_JSON_MAX_ALLOCATED_BYTES_V1: usize = 256 * 1024;
const VPN_SETTLEMENT_JSON_MAX_DEPTH_V1: usize = 4;
const VPN_SETTLEMENT_JSON_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    VPN_SETTLEMENT_JSON_MAX_SEQUENCE_ELEMENTS_V1,
    VPN_SETTLEMENT_JSON_MAX_FIELD_BYTES_V1,
    VPN_SETTLEMENT_JSON_MAX_TOTAL_ELEMENTS_V1,
    VPN_SETTLEMENT_JSON_MAX_ALLOCATED_BYTES_V1,
    VPN_SETTLEMENT_JSON_MAX_DEPTH_V1,
);
const fn vpn_settlement_json_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
        VPN_SETTLEMENT_JSON_MAX_TOTAL_ELEMENTS_V1 + 1,
        VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
        VPN_SETTLEMENT_JSON_MAX_FIELD_BYTES_V1,
        VPN_SETTLEMENT_JSON_MAX_TOTAL_STRING_BYTES_V1,
        VPN_SETTLEMENT_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        0,
        VPN_SETTLEMENT_JSON_MAX_TOTAL_ELEMENTS_V1,
        VPN_SETTLEMENT_JSON_MAX_TOTAL_ELEMENTS_V1,
        VPN_SETTLEMENT_JSON_MAX_DEPTH_V1,
    )
}
#[derive(Debug, Parser)]
#[command(
    name = "soranet-vpn-settlement",
    version,
    about = "Sign SoraNet VPN settlement artifacts for Torii receipt submission"
)]
struct Cli {
    /// Path to a relay-spooled VPN settlement artifact JSON file.
    #[arg(long)]
    artifact: PathBuf,
    /// Exact canonical I105 operator account or printable ASCII alias.
    ///
    /// I105 is emitted in X-Iroha-Account as lowercase canonical address hex.
    #[arg(long)]
    account_id: String,
    /// Exact genesis-derived network identity used by Torii request authentication.
    #[arg(long)]
    network_id: NetworkId,
    /// Runtime-only file containing the hex-encoded 32-byte Ed25519 private seed.
    ///
    /// The file must be a direct regular file; symlinks are rejected. The seed
    /// is never accepted through argv or included in output. Unix files must
    /// have no group or other permission bits (for example, mode 0600).
    #[arg(long)]
    private_key_seed_file: PathBuf,
    /// Optional Torii root used when rendering curl output.
    #[arg(long)]
    torii_root: Option<String>,
    /// Override the request timestamp in milliseconds.
    #[arg(long)]
    timestamp_ms: Option<u64>,
    /// Override the request nonce. Must be unique for the operator account.
    #[arg(long)]
    nonce: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    output: OutputFormat,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Curl,
}
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct VpnSettlementSubmitRequestArtifact {
    relay_receipt_hex: String,
    client_voucher_hex: String,
    lease_id_hex: String,
}
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
struct VpnSettlementSpoolRecord {
    version: u8,
    generated_at_ms: u64,
    session_id_hex: String,
    quote_id_hex: String,
    payment_tx_hash_hex: String,
    earned_fee: Quantity,
    torii_receipt_path: String,
    submit_receipt_request: VpnSettlementSubmitRequestArtifact,
}
#[derive(Debug, Clone, JsonSerialize)]
struct SignedHeader {
    name: String,
    value: String,
}
#[derive(Debug, Clone, JsonSerialize)]
struct SignedSettlementRequest {
    network_id: NetworkId,
    method: String,
    url: Option<String>,
    path: String,
    body: String,
    headers: Vec<SignedHeader>,
}
fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("soranet-vpn-settlement error: {error}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let artifact = read_artifact(&cli.artifact)?;
    validate_artifact_receipt_path(&artifact)?;
    let torii_root = cli
        .torii_root
        .as_deref()
        .map(validate_torii_root)
        .transpose()?;
    let timestamp_ms = match cli.timestamp_ms {
        Some(timestamp_ms) => timestamp_ms,
        None => unix_time_ms()?,
    };
    let nonce = match cli.nonce {
        Some(nonce) => nonce,
        None => default_nonce(&artifact, timestamp_ms)?,
    };
    validate_canonical_request_inputs(RECEIPT_METHOD, RECEIPT_PATH, "", &cli.account_id, &nonce)?;
    validate_request_freshness(timestamp_ms, &nonce)?;
    let seed = read_seed_file(&cli.private_key_seed_file)?;
    let signed_result = sign_artifact(
        &artifact,
        &cli.account_id,
        &cli.network_id,
        seed.expose(),
        torii_root.as_deref(),
        timestamp_ms,
        nonce.as_str(),
    );
    let signed = signed_result?;
    match cli.output {
        OutputFormat::Json => {
            println!("{}", render_json(&signed)?);
        }
        OutputFormat::Curl => {
            println!("{}", render_curl(&signed)?);
        }
    }
    Ok(())
}
fn read_artifact(path: &Path) -> Result<VpnSettlementSpoolRecord, Box<dyn Error>> {
    let bytes = read_bounded_direct_regular_file(
        path,
        VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
        "VPN settlement spool record",
    )?;
    json::preflight_slice(&bytes, vpn_settlement_json_preflight_limits_v1())?;
    Ok(norito::with_decode_limits_scope(
        VPN_SETTLEMENT_JSON_DECODE_LIMITS_V1,
        || json::from_slice(&bytes),
    )?)
}
fn validate_artifact_receipt_path(
    artifact: &VpnSettlementSpoolRecord,
) -> Result<(), Box<dyn Error>> {
    if artifact.version != 1 {
        return Err("settlement artifact version must be exactly 1".into());
    }
    if artifact.torii_receipt_path != RECEIPT_PATH {
        return Err(
            format!("settlement artifact receipt path must be exactly {RECEIPT_PATH}").into(),
        );
    }
    Ok(())
}
fn unix_time_ms() -> Result<u64, Box<dyn Error>> {
    unix_time_ms_from(SystemTime::now())
}

fn unix_time_ms_from(now: SystemTime) -> Result<u64, Box<dyn Error>> {
    let millis = now
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system timestamp does not fit in u64 milliseconds".into())
}
fn decimal_len(mut value: u64) -> usize {
    let mut length = 1;
    while value >= 10 {
        value /= 10;
        length += 1;
    }
    length
}

fn push_decimal(mut value: u64, output: &mut Vec<u8>) {
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + u8::try_from(value % 10).expect("decimal digit fits in u8");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.extend_from_slice(&digits[start..]);
}

fn default_nonce(
    artifact: &VpnSettlementSpoolRecord,
    timestamp_ms: u64,
) -> Result<String, Box<dyn Error>> {
    const PREFIX: &[u8] = b"vpn-settle:";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let session_hash = Sha256::digest(artifact.session_id_hex.as_bytes());
    let length = PREFIX
        .len()
        .checked_add(
            session_hash
                .len()
                .checked_mul(2)
                .ok_or("nonce length overflow")?,
        )
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(decimal_len(timestamp_ms)))
        .ok_or("nonce length overflow")?;
    if length > CANONICAL_REQUEST_MAX_NONCE_BYTES_V1 {
        return Err("default nonce exceeds the canonical V1 limit".into());
    }
    let mut nonce = Vec::new();
    nonce
        .try_reserve_exact(length)
        .map_err(|error| format!("failed to reserve {length} nonce bytes: {error}"))?;
    nonce.extend_from_slice(PREFIX);
    for byte in session_hash {
        nonce.push(HEX[usize::from(byte >> 4)]);
        nonce.push(HEX[usize::from(byte & 0x0f)]);
    }
    nonce.push(b':');
    push_decimal(timestamp_ms, &mut nonce);
    debug_assert_eq!(nonce.len(), length);
    String::from_utf8(nonce).map_err(Into::into)
}

struct SecretSeed([u8; 32]);

impl fmt::Debug for SecretSeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretSeed(<redacted>)")
    }
}

impl SecretSeed {
    fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    fn clear(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

impl Drop for SecretSeed {
    fn drop(&mut self) {
        self.clear();
    }
}

fn decode_seed(raw: &str) -> Result<SecretSeed, Box<dyn Error>> {
    let normalized = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if normalized.len() != 64 {
        return Err("private key seed must contain exactly 64 hex characters".into());
    }
    let mut seed = SecretSeed([0_u8; 32]);
    hex::decode_to_slice(normalized, &mut seed.0)?;
    Ok(seed)
}

fn read_seed_file(path: &Path) -> Result<SecretSeed, Box<dyn Error>> {
    if !path.is_absolute() {
        return Err("private key seed file must use an absolute runtime-only path".into());
    }
    let mut bytes = read_bounded_private_regular_file(
        path,
        PRIVATE_KEY_SEED_FILE_MAX_BYTES_V1,
        "VPN settlement private seed",
    )?;
    let result = match std::str::from_utf8(&bytes) {
        Ok(raw) => decode_seed(raw),
        Err(error) => Err(format!("private key seed file is not UTF-8: {error}").into()),
    };
    bytes.clear();
    result
}
fn request_body(record: &VpnSettlementSpoolRecord) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(json::to_json_bounded(
        &record.submit_receipt_request,
        VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
    )?
    .into_bytes())
}
fn validate_canonical_request_target(
    method: &str,
    path: &str,
    raw_query: &str,
) -> Result<(), Box<dyn Error>> {
    if method.len() > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 {
        return Err(format!(
            "canonical request method exceeds the V1 limit of {CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} bytes"
        )
        .into());
    }
    if path.len() > CANONICAL_REQUEST_MAX_PATH_BYTES_V1 {
        return Err(format!(
            "canonical request path exceeds the V1 limit of {CANONICAL_REQUEST_MAX_PATH_BYTES_V1} bytes"
        )
        .into());
    }
    if raw_query.len() > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 {
        return Err(format!(
            "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1} raw bytes"
        )
        .into());
    }
    let pair_count = raw_query
        .as_bytes()
        .split(|byte| *byte == b'&')
        .filter(|pair| !pair.is_empty())
        .take(CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1.saturating_add(1))
        .count();
    if pair_count > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 {
        return Err(format!(
            "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs"
        )
        .into());
    }
    Ok(())
}

fn validate_canonical_request_inputs(
    method: &str,
    path: &str,
    raw_query: &str,
    account_id: &str,
    nonce: &str,
) -> Result<(), Box<dyn Error>> {
    validate_canonical_request_target(method, path, raw_query)?;
    if account_id.is_empty()
        || account_id.len() > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
        || account_id.trim() != account_id
    {
        return Err(format!(
            "account id must be exact and within the canonical V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
        )
        .into());
    }
    if nonce.is_empty()
        || nonce.len() > CANONICAL_REQUEST_MAX_NONCE_BYTES_V1
        || !nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err(format!(
            "nonce must be printable ASCII within the canonical V1 limit of {CANONICAL_REQUEST_MAX_NONCE_BYTES_V1} bytes"
        )
        .into());
    }
    Ok(())
}

fn validate_request_freshness(timestamp_ms: u64, nonce: &str) -> Result<(), Box<dyn Error>> {
    if timestamp_ms == 0 {
        return Err("canonical request timestamp must be positive".into());
    }
    if nonce.is_empty()
        || nonce.len() > CANONICAL_REQUEST_MAX_NONCE_BYTES_V1
        || !nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("invalid canonical request nonce".into());
    }
    Ok(())
}

fn canonical_network_request_signature_message(
    network_id: &NetworkId,
    method: &str,
    path: &str,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    const DOMAIN: &[u8] = b"iroha.app.request.network.v1\0";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    validate_canonical_request_target(method, path, "")?;
    validate_request_freshness(timestamp_ms, nonce)?;
    let length = DOMAIN
        .len()
        .checked_add(network_id.as_bytes().len())
        .and_then(|length| length.checked_add(method.len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(path.len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(1 + 64))
        .and_then(|length| length.checked_add(1 + decimal_len(timestamp_ms) + 1))
        .and_then(|length| length.checked_add(nonce.len()))
        .ok_or("canonical request byte length exceeds platform capacity")?;
    let mut message = Vec::new();
    message
        .try_reserve_exact(length)
        .map_err(|error| format!("failed to reserve {length} canonical request bytes: {error}"))?;
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(network_id.as_bytes());
    message.extend(method.bytes().map(|byte| byte.to_ascii_uppercase()));
    message.push(b'\n');
    message.extend_from_slice(path.as_bytes());
    message.push(b'\n');
    // The pinned receipt target has no query.
    message.push(b'\n');
    for byte in Sha256::digest(body) {
        message.push(HEX[usize::from(byte >> 4)]);
        message.push(HEX[usize::from(byte & 0x0f)]);
    }
    message.push(b'\n');
    push_decimal(timestamp_ms, &mut message);
    message.push(b'\n');
    message.extend_from_slice(nonce.as_bytes());
    debug_assert_eq!(message.len(), length);
    Ok(message)
}
/// Render an exact account input into the strict ASCII auth-header form.
fn canonical_account_header_value(
    account_id: &str,
    signing_public_key: &iroha_crypto::PublicKey,
) -> Result<String, Box<dyn Error>> {
    if account_id.is_empty()
        || account_id.len() > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
        || account_id.trim() != account_id
    {
        return Err("account id must be exact, non-empty, and bounded".into());
    }
    let value = match AccountAddress::parse_encoded(account_id, None) {
        Ok(address) => {
            let parsed_account = address.to_account_id().map_err(|error| {
                format!("failed to decode canonical account controller: {error}")
            })?;
            if parsed_account.try_signatory() != Some(signing_public_key) {
                return Err(
                    "canonical I105 account controller does not match the settlement signing key"
                        .into(),
                );
            }
            address
                .canonical_hex()
                .map_err(|err| format!("failed to encode canonical account header: {err}").into())
        }
        Err(_) if account_id.bytes().all(|byte| byte.is_ascii_graphic()) => {
            let alias = account_id
                .parse::<AccountAliasName>()
                .map_err(|error| format!("invalid canonical account alias: {error}"))?;
            if alias.canonical_text() != account_id {
                return Err("account alias must use its exact canonical text".into());
            }
            try_clone_string(account_id, "account header")
        }
        Err(_) => Err("account id must be a canonical I105 account or account alias".into()),
    }?;
    if value.len() > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 {
        return Err(format!(
            "canonical account header exceeds the V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
        )
        .into());
    }
    Ok(value)
}

fn try_clone_string(value: &str, context: &str) -> Result<String, Box<dyn Error>> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|error| {
        format!(
            "failed to reserve {} bytes for {context}: {error}",
            value.len()
        )
    })?;
    output.push_str(value);
    Ok(output)
}

fn canonical_signature_header_value(signature: &Signature) -> Result<String, Box<dyn Error>> {
    let payload = signature.payload();
    if payload.is_empty()
        || payload.len() > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
        || payload.iter().all(|byte| *byte == 0)
    {
        return Err("invalid canonical request signature".into());
    }
    let encoded_len = payload
        .len()
        .checked_add(2)
        .map(|length| length / 3)
        .and_then(|length| length.checked_mul(4))
        .ok_or("canonical signature header length exceeds platform capacity")?;
    let mut encoded = Vec::new();
    encoded.try_reserve_exact(encoded_len).map_err(|error| {
        format!("failed to reserve {encoded_len} signature header bytes: {error}")
    })?;
    encoded.resize(encoded_len, 0);
    let written = BASE64_STANDARD
        .encode_slice(payload, &mut encoded)
        .map_err(|_| "failed to encode canonical signature header")?;
    if written != encoded_len {
        return Err("canonical signature header length mismatch".into());
    }
    String::from_utf8(encoded).map_err(Into::into)
}

fn timestamp_header_value(timestamp_ms: u64) -> Result<String, Box<dyn Error>> {
    let length = decimal_len(timestamp_ms);
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|error| format!("failed to reserve {length} timestamp bytes: {error}"))?;
    push_decimal(timestamp_ms, &mut bytes);
    String::from_utf8(bytes).map_err(Into::into)
}

fn signed_header(name: &str, value: String) -> Result<SignedHeader, Box<dyn Error>> {
    Ok(SignedHeader {
        name: try_clone_string(name, "header name")?,
        value,
    })
}

fn render_json(signed: &SignedSettlementRequest) -> Result<String, Box<dyn Error>> {
    render_json_with_limit(signed, SIGNED_SETTLEMENT_JSON_MAX_BYTES_V1)
}

fn render_json_with_limit(
    signed: &SignedSettlementRequest,
    maximum: usize,
) -> Result<String, Box<dyn Error>> {
    json::to_json_bounded(signed, maximum).map_err(Into::into)
}

fn validate_torii_root(root: &str) -> Result<String, Box<dyn Error>> {
    if root.is_empty()
        || root.len() > TORII_ROOT_MAX_BYTES_V1
        || root.trim() != root
        || !root.is_ascii()
    {
        return Err("Torii root must be exact, non-empty ASCII".into());
    }
    let (scheme, authority_raw) = root
        .split_once("://")
        .ok_or("Torii root must be an absolute http or https URL")?;
    if scheme != "http" && scheme != "https" {
        return Err("Torii root scheme must be http or https".into());
    }
    let authority_raw = authority_raw.strip_suffix('/').unwrap_or(authority_raw);
    if authority_raw.is_empty()
        || authority_raw.contains(['/', '?', '#', '@'])
        || Authority::from_str(authority_raw).is_err()
    {
        return Err(
            "Torii root must contain only a valid authority, with no credentials, base path, query, or fragment"
                .into(),
        );
    }
    let length = scheme
        .len()
        .checked_add(3)
        .and_then(|length| length.checked_add(authority_raw.len()))
        .ok_or("Torii root length exceeds platform capacity")?;
    let mut canonical = String::new();
    canonical
        .try_reserve_exact(length)
        .map_err(|error| format!("failed to reserve {length} Torii root bytes: {error}"))?;
    canonical.push_str(scheme);
    canonical.push_str("://");
    canonical.push_str(authority_raw);
    Ok(canonical)
}

fn request_url(root: &str) -> Result<String, Box<dyn Error>> {
    let length = root
        .len()
        .checked_add(RECEIPT_PATH.len())
        .ok_or("request URL length exceeds platform capacity")?;
    let mut url = String::new();
    url.try_reserve_exact(length)
        .map_err(|error| format!("failed to reserve {length} request URL bytes: {error}"))?;
    url.push_str(root);
    url.push_str(RECEIPT_PATH);
    Ok(url)
}
fn sign_artifact(
    artifact: &VpnSettlementSpoolRecord,
    account_id: &str,
    network_id: &NetworkId,
    seed: &[u8; 32],
    torii_root: Option<&str>,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<SignedSettlementRequest, Box<dyn Error>> {
    validate_artifact_receipt_path(artifact)?;
    validate_canonical_request_inputs(RECEIPT_METHOD, RECEIPT_PATH, "", account_id, nonce)?;
    validate_request_freshness(timestamp_ms, nonce)?;
    let torii_root = torii_root.map(validate_torii_root).transpose()?;
    let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .map_err(|err| format!("failed to derive settlement signing key: {err}"))?;
    let account_header = canonical_account_header_value(account_id, key_pair.public_key())?;
    let body = request_body(artifact)?;
    let message = canonical_network_request_signature_message(
        network_id,
        RECEIPT_METHOD,
        RECEIPT_PATH,
        &body,
        timestamp_ms,
        nonce,
    )?;
    let signature = Signature::try_new(key_pair.private_key(), &message)?;
    let signature_header = canonical_signature_header_value(&signature)?;
    let timestamp_header = timestamp_header_value(timestamp_ms)?;
    let nonce_header = try_clone_string(nonce, "nonce header")?;
    let body = String::from_utf8(body)?;
    let url = torii_root.as_deref().map(request_url).transpose()?;
    let mut headers = Vec::new();
    headers
        .try_reserve_exact(5)
        .map_err(|error| format!("failed to reserve signed request headers: {error}"))?;
    headers.push(signed_header(
        "Content-Type",
        try_clone_string("application/json", "content type header")?,
    )?);
    headers.push(signed_header(HEADER_ACCOUNT, account_header)?);
    headers.push(signed_header(HEADER_SIGNATURE, signature_header)?);
    headers.push(signed_header(HEADER_TIMESTAMP_MS, timestamp_header)?);
    headers.push(signed_header(HEADER_NONCE, nonce_header)?);
    Ok(SignedSettlementRequest {
        network_id: *network_id,
        method: try_clone_string(RECEIPT_METHOD, "request method")?,
        url,
        path: try_clone_string(RECEIPT_PATH, "request path")?,
        body,
        headers,
    })
}
fn render_curl(signed: &SignedSettlementRequest) -> Result<String, Box<dyn Error>> {
    const PREFIX: &str = "curl --disable --config /dev/null --silent --show-error --request ";
    const OPTIONS: &str = " --no-location --no-location-trusted --max-redirs 0 --retry 0 --proto '=http,https' --proto-redir -all --url ";
    const HEADER_PREFIX: &str = " \\\n  -H ";
    const BODY_PREFIX: &str = " \\\n  --data-binary ";
    let url = signed
        .url
        .as_deref()
        .ok_or("curl output requires --torii-root so the request URL can be rendered")?;
    let mut capacity = PREFIX
        .len()
        .checked_add(signed.method.len())
        .and_then(|length| length.checked_add(OPTIONS.len()))
        .and_then(|length| {
            shell_quoted_parts_max_len(&[url]).and_then(|url| length.checked_add(url))
        })
        .ok_or("curl command length exceeds platform capacity")?;
    for header in &signed.headers {
        capacity = capacity
            .checked_add(HEADER_PREFIX.len())
            .and_then(|length| {
                shell_quoted_parts_max_len(&[&header.name, ": ", &header.value])
                    .and_then(|header| length.checked_add(header))
            })
            .ok_or("curl command length exceeds platform capacity")?;
    }
    capacity = capacity
        .checked_add(BODY_PREFIX.len())
        .and_then(|length| {
            shell_quoted_parts_max_len(&[&signed.body]).and_then(|body| length.checked_add(body))
        })
        .ok_or("curl command length exceeds platform capacity")?;
    let mut command = String::new();
    command
        .try_reserve_exact(capacity)
        .map_err(|error| format!("failed to reserve {capacity} curl command bytes: {error}"))?;
    command.push_str(PREFIX);
    command.push_str(&signed.method);
    command.push_str(OPTIONS);
    push_shell_quoted_parts(&mut command, &[url]);
    for header in &signed.headers {
        command.push_str(" \\\n  -H ");
        push_shell_quoted_parts(&mut command, &[&header.name, ": ", &header.value]);
    }
    command.push_str(" \\\n  --data-binary ");
    push_shell_quoted_parts(&mut command, &[&signed.body]);
    Ok(command)
}
fn shell_quoted_parts_max_len(parts: &[&str]) -> Option<usize> {
    parts.iter().try_fold(2_usize, |length, part| {
        part.len()
            .checked_mul(4)
            .and_then(|bytes| length.checked_add(bytes))
    })
}

fn push_shell_quoted_parts(output: &mut String, parts: &[&str]) {
    output.push('\'');
    for part in parts {
        for segment in part.split_inclusive('\'') {
            if let Some(prefix) = segment.strip_suffix('\'') {
                output.push_str(prefix);
                output.push_str("'\\''");
            } else {
                output.push_str(segment);
            }
        }
    }
    output.push('\'');
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{account::AccountId, block::BlockHeader};
    use tempfile::tempdir;
    fn test_network_id(marker: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([marker; Hash::LENGTH]),
        ))
    }
    fn sample_record() -> VpnSettlementSpoolRecord {
        VpnSettlementSpoolRecord {
            version: 1,
            generated_at_ms: 1_700_000_000_000,
            session_id_hex: "aa".repeat(16),
            quote_id_hex: "11".repeat(32),
            payment_tx_hash_hex: "22".repeat(32),
            earned_fee: "0.000000055".parse().expect("canonical XOR quantity"),
            torii_receipt_path: RECEIPT_PATH.to_owned(),
            submit_receipt_request: VpnSettlementSubmitRequestArtifact {
                relay_receipt_hex: "33".repeat(4),
                client_voucher_hex: "44".repeat(4),
                lease_id_hex: "11".repeat(32),
            },
        }
    }
    fn test_key_pair(seed: [u8; 32]) -> KeyPair {
        KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
            .expect("derive settlement fixture key")
    }
    fn account_i105_for_seed(seed: [u8; 32]) -> String {
        let key_pair = test_key_pair(seed);
        AccountAddress::from_account_id(&AccountId::new(key_pair.public_key().clone()))
            .expect("single-key account address")
            .to_i105()
            .expect("canonical I105 account")
    }
    #[test]
    fn signs_spooled_artifact_with_verifiable_canonical_message() {
        let record = sample_record();
        let seed = [0x66; 32];
        let account_i105 = account_i105_for_seed(seed);
        let network_id = test_network_id(0x61);
        let signed = sign_artifact(
            &record,
            &account_i105,
            &network_id,
            &seed,
            Some("http://127.0.0.1:8080"),
            1_700_000_000_123,
            "nonce-1",
        )
        .expect("signed request");
        assert_eq!(signed.network_id, network_id);
        assert_eq!(signed.method, "POST");
        assert_eq!(
            signed.url.as_deref(),
            Some("http://127.0.0.1:8080/v1/vpn/receipts")
        );
        assert!(signed.body.contains("relay_receipt_hex"));
        let account_header = signed
            .headers
            .iter()
            .find(|header| header.name == HEADER_ACCOUNT)
            .expect("account header");
        let expected_account_header = AccountAddress::parse_encoded(&account_i105, None)
            .expect("canonical I105 account")
            .canonical_hex()
            .expect("canonical account hex");
        assert_eq!(account_header.value, expected_account_header);
        assert!(account_header.value.is_ascii());
        let signature_b64 = signed
            .headers
            .iter()
            .find(|header| header.name == HEADER_SIGNATURE)
            .expect("signature header")
            .value
            .clone();
        let signature = Signature::try_from_bytes(
            &BASE64_STANDARD
                .decode(signature_b64)
                .expect("base64 signature"),
        )
        .expect("settlement request signature is non-empty and nonzero");
        let message = canonical_network_request_signature_message(
            &network_id,
            RECEIPT_METHOD,
            RECEIPT_PATH,
            signed.body.as_bytes(),
            1_700_000_000_123,
            "nonce-1",
        )
        .expect("canonical signed message");
        let key_pair = test_key_pair(seed);
        signature
            .verify(key_pair.public_key(), &message)
            .expect("signature verifies");
        let foreign_message = canonical_network_request_signature_message(
            &test_network_id(0x62),
            RECEIPT_METHOD,
            RECEIPT_PATH,
            signed.body.as_bytes(),
            1_700_000_000_123,
            "nonce-1",
        )
        .expect("foreign canonical signed message");
        signature
            .verify(key_pair.public_key(), &foreign_message)
            .expect_err("same request must not verify for a different genesis network");
    }
    #[test]
    fn curl_output_contains_signed_headers_and_body() {
        let record = sample_record();
        let signed = sign_artifact(
            &record,
            "operator@taira",
            &test_network_id(0x63),
            &[0x11; 32],
            Some("https://torii.example"),
            7,
            "nonce-2",
        )
        .expect("signed request");
        let curl = render_curl(&signed).expect("curl output");
        assert!(curl.contains("https://torii.example/v1/vpn/receipts"));
        assert!(curl.contains("X-Iroha-Account: operator@taira"));
        assert!(curl.contains("X-Iroha-Nonce: nonce-2"));
        assert!(curl.contains("relay_receipt_hex"));
        assert!(curl.starts_with("curl --disable --config /dev/null "));
        assert!(curl.contains("--max-redirs 0"));
        assert!(curl.contains("--retry 0"));
        assert!(curl.contains("--no-location"));
        assert!(curl.contains("--no-location-trusted"));
        assert!(curl.contains("--proto-redir -all"));
    }
    #[test]
    fn json_output_is_compact_bounded_and_matches_canonical_encoding() {
        let signed = sign_artifact(
            &sample_record(),
            "operator@taira",
            &test_network_id(0x66),
            &[0x12; 32],
            Some("https://torii.example"),
            8,
            "nonce-json-output",
        )
        .expect("signed request");
        let expected = json::to_string(&signed).expect("canonical compact JSON");
        assert_eq!(render_json(&signed).expect("bounded JSON"), expected);
        assert_eq!(
            render_json_with_limit(&signed, expected.len()).expect("exact output bound"),
            expected
        );
        render_json_with_limit(&signed, expected.len().saturating_sub(1))
            .expect_err("one byte below exact output length must fail");
    }
    #[test]
    fn account_header_rejects_invalid_or_noncanonical_aliases() {
        let key_pair = test_key_pair([0x22; 32]);
        assert_eq!(
            canonical_account_header_value("operator@taira", key_pair.public_key())
                .expect("canonical alias"),
            "operator@taira"
        );
        for invalid in [
            "operator",
            " operator",
            "operator ",
            "operator alias",
            "operator\n",
            "Operator@Taira",
            "账户",
        ] {
            canonical_account_header_value(invalid, key_pair.public_key())
                .expect_err("only exact canonical account aliases are supported");
        }
    }
    #[test]
    fn i105_account_must_match_signing_key() {
        let record = sample_record();
        let account_i105 = account_i105_for_seed([0x31; 32]);
        sign_artifact(
            &record,
            &account_i105,
            &test_network_id(0x64),
            &[0x32; 32],
            None,
            9,
            "nonce-key-mismatch",
        )
        .expect_err("foreign I105 controller must fail before signing");
    }
    #[test]
    fn artifact_route_and_version_are_pinned() {
        let mut record = sample_record();
        validate_artifact_receipt_path(&record).expect("canonical artifact route");
        record.torii_receipt_path = "/v1/vpn/receipts?x=1".to_owned();
        validate_artifact_receipt_path(&record).expect_err("query-bearing route must fail");
        record.torii_receipt_path = RECEIPT_PATH.to_owned();
        record.version = 2;
        validate_artifact_receipt_path(&record).expect_err("unknown version must fail");
    }
    #[test]
    fn request_body_uses_bounded_canonical_json() {
        let record = sample_record();
        let expected = json::to_vec(&record.submit_receipt_request).expect("legacy canonical JSON");
        let actual = request_body(&record).expect("bounded canonical JSON");
        assert_eq!(actual, expected);
        assert_eq!(
            json::to_json_bounded(&record.submit_receipt_request, expected.len())
                .expect("exact JSON bound")
                .into_bytes(),
            expected
        );
        json::to_json_bounded(
            &record.submit_receipt_request,
            expected.len().saturating_sub(1),
        )
        .expect_err("one byte below exact JSON length must fail");
    }
    #[test]
    fn canonical_v1_caps_reject_max_plus_one_before_message_building() {
        let method = "M".repeat(CANONICAL_REQUEST_MAX_METHOD_BYTES_V1);
        let path = format!("/{}", "p".repeat(CANONICAL_REQUEST_MAX_PATH_BYTES_V1 - 1));
        let raw_query = "q".repeat(CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1);
        validate_canonical_request_target(&method, &path, &raw_query)
            .expect("exact method/path/query limits");
        assert!(
            validate_canonical_request_target(&format!("{method}M"), RECEIPT_PATH, "").is_err()
        );
        assert!(
            validate_canonical_request_target(RECEIPT_METHOD, &format!("{path}p"), "").is_err()
        );
        assert!(
            validate_canonical_request_target(
                RECEIPT_METHOD,
                RECEIPT_PATH,
                &format!("{raw_query}q")
            )
            .is_err()
        );
        let exact_pairs = std::iter::repeat_n("k=v", CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1)
            .collect::<Vec<_>>()
            .join("&");
        validate_canonical_request_target(RECEIPT_METHOD, RECEIPT_PATH, &exact_pairs)
            .expect("exact query-pair limit");
        let excessive_pairs = format!("{exact_pairs}&k=v");
        assert!(
            validate_canonical_request_target(RECEIPT_METHOD, RECEIPT_PATH, &excessive_pairs)
                .is_err()
        );

        let exact_account = "a".repeat(CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1);
        let exact_nonce = "n".repeat(CANONICAL_REQUEST_MAX_NONCE_BYTES_V1);
        validate_canonical_request_inputs(
            RECEIPT_METHOD,
            RECEIPT_PATH,
            "",
            &exact_account,
            &exact_nonce,
        )
        .expect("exact account and nonce limits");
        assert!(
            validate_canonical_request_inputs(
                RECEIPT_METHOD,
                RECEIPT_PATH,
                "",
                &format!("{exact_account}a"),
                "nonce",
            )
            .is_err()
        );
        assert!(
            validate_canonical_request_inputs(
                RECEIPT_METHOD,
                RECEIPT_PATH,
                "",
                "operator",
                &format!("{exact_nonce}n"),
            )
            .is_err()
        );
        assert!(
            validate_canonical_request_inputs(
                RECEIPT_METHOD,
                RECEIPT_PATH,
                "",
                "operator",
                "nonce with space",
            )
            .is_err()
        );
    }
    #[test]
    fn signature_header_enforces_v1_payload_cap() {
        let valid = Signature::from_bytes(&vec![0x11; CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1]);
        canonical_signature_header_value(&valid).expect("exact signature limit");
        let excessive =
            Signature::from_bytes(&vec![0x11; CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 + 1]);
        canonical_signature_header_value(&excessive).expect_err("max+1 signature must fail");
    }
    #[test]
    fn torii_root_accepts_only_an_http_origin() {
        assert_eq!(
            validate_torii_root("https://torii.example:443/").expect("valid origin"),
            "https://torii.example:443"
        );
        assert_eq!(
            request_url(&validate_torii_root("http://[::1]:8080").expect("IPv6 origin"))
                .expect("receipt URL"),
            "http://[::1]:8080/v1/vpn/receipts"
        );
        for invalid in [
            "https://torii.example/base",
            "https://torii.example?x=1",
            "https://torii.example/#fragment",
            "https://user@torii.example",
            "ftp://torii.example",
            " https://torii.example",
        ] {
            validate_torii_root(invalid).expect_err("non-origin Torii root must fail");
        }
        let exact_root = format!(
            "https://{}",
            "a".repeat(TORII_ROOT_MAX_BYTES_V1 - "https://".len())
        );
        assert_eq!(
            validate_torii_root(&exact_root).expect("exact root byte limit"),
            exact_root
        );
        let excessive_root = format!("{exact_root}a");
        validate_torii_root(&excessive_root).expect_err("max+1 root must fail before parsing");
    }
    #[test]
    fn generated_nonce_is_printable_and_bounded() {
        let mut record = sample_record();
        record.session_id_hex = "s".repeat(VPN_SETTLEMENT_JSON_MAX_FIELD_BYTES_V1);
        let nonce = default_nonce(&record, u64::MAX).expect("bounded nonce");
        assert!(nonce.len() <= CANONICAL_REQUEST_MAX_NONCE_BYTES_V1);
        assert!(nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte)));
    }
    #[test]
    fn zero_timestamp_is_rejected_before_signing() {
        validate_request_freshness(0, "nonce").expect_err("zero timestamp must fail");
        sign_artifact(
            &sample_record(),
            "operator@taira",
            &test_network_id(0x65),
            &[0x41; 32],
            None,
            0,
            "nonce-zero-timestamp",
        )
        .expect_err("producer must reject a zero timestamp");
    }
    #[test]
    fn pre_epoch_clock_is_rejected() {
        let before_epoch = UNIX_EPOCH
            .checked_sub(std::time::Duration::from_millis(1))
            .expect("representable pre-epoch timestamp");
        unix_time_ms_from(before_epoch).expect_err("pre-epoch clock must fail closed");
    }
    #[test]
    fn seed_decode_rejects_length_before_hex_allocation() {
        let mut seed = decode_seed(&"ab".repeat(32)).expect("valid seed");
        assert_eq!(seed.expose(), &[0xab; 32]);
        assert_eq!(format!("{seed:?}"), "SecretSeed(<redacted>)");
        seed.clear();
        assert_eq!(seed.expose(), &[0; 32]);
        let error = decode_seed(&"ab".repeat(33)).expect_err("max+1 seed must fail");
        assert!(error.to_string().contains("exactly 64"), "{error}");
    }
    #[test]
    fn seed_file_is_read_without_argv_material() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("seed.hex");
        std::fs::write(&path, format!("{}\n", "ab".repeat(32))).expect("write seed file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .expect("restrict seed permissions");
        }
        assert_eq!(
            read_seed_file(&path).expect("read seed").expose(),
            &[0xab; 32]
        );
        read_seed_file(Path::new("relative-seed.hex"))
            .expect_err("relative secret path must fail before file access");
    }
    #[cfg(unix)]
    #[test]
    fn seed_file_rejects_group_or_other_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("seed.hex");
        std::fs::write(&path, "ab".repeat(32)).expect("write seed file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640))
            .expect("set unsafe seed permissions");
        read_seed_file(&path).expect_err("group-readable seed must fail before read");
    }
    #[test]
    fn settlement_artifact_reader_accepts_exact_file_limit() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("settlement.json");
        let encoded = json::to_vec(&sample_record()).expect("encode sample record");
        assert!(encoded.len() < VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1);
        let mut exact = encoded;
        exact.resize(VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1, b' ');
        std::fs::write(&path, &exact).expect("write exact artifact");
        let loaded = read_artifact(&path).expect("exact artifact must load");
        assert_eq!(loaded.version, 1);
        exact.push(b' ');
        std::fs::write(&path, exact).expect("write oversized artifact");
        let error = read_artifact(&path).expect_err("max+1 artifact must fail before decode");
        assert!(error.to_string().contains("first-release limit"), "{error}");
    }
    #[cfg(unix)]
    #[test]
    fn settlement_artifact_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().expect("temporary directory");
        let target = directory.path().join("target.json");
        let link = directory.path().join("settlement.json");
        std::fs::write(
            &target,
            json::to_vec(&sample_record()).expect("encode sample record"),
        )
        .expect("write target");
        symlink(&target, &link).expect("create symlink");
        let error = read_artifact(&link).expect_err("symlink must fail before read");
        assert!(error.to_string().contains("direct regular file"), "{error}");
    }
}
