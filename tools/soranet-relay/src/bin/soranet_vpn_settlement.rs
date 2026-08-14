//! Operator helper for SoraNet VPN settlement artifacts.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Parser, ValueEnum};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{NetworkId, account::AccountAddress};
use iroha_primitives::numeric::Quantity;
use norito::{
    DecodeLimits,
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use sha2::{Digest as _, Sha256};
use soranet_relay::{
    config::read_bounded_direct_regular_file, runtime::VPN_SETTLEMENT_SPOOL_MAX_BYTES_V1,
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const HEADER_ACCOUNT: &str = "X-Iroha-Account";
const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
const HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
const HEADER_NONCE: &str = "X-Iroha-Nonce";
#[cfg(test)]
const DEFAULT_PATH: &str = "/v1/vpn/receipts";
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
    /// Runtime-only hex-encoded 32-byte Ed25519 private seed.
    #[arg(long)]
    private_key_seed_hex: String,
    /// Optional Torii root used when rendering curl output.
    #[arg(long)]
    torii_root: Option<String>,
    /// Override the Torii receipt path.
    #[arg(long)]
    path: Option<String>,
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
    let path = cli
        .path
        .as_deref()
        .unwrap_or(artifact.torii_receipt_path.as_str());
    let path = normalize_path(path)?;
    let timestamp_ms = cli.timestamp_ms.unwrap_or_else(unix_time_ms);
    let nonce = cli
        .nonce
        .unwrap_or_else(|| default_nonce(&artifact, timestamp_ms));
    let seed = decode_seed(&cli.private_key_seed_hex)?;
    let signed = sign_artifact(
        &artifact,
        &cli.account_id,
        &cli.network_id,
        &seed,
        path.as_str(),
        cli.torii_root.as_deref(),
        timestamp_ms,
        nonce.as_str(),
    )?;
    match cli.output {
        OutputFormat::Json => {
            println!("{}", json::to_string_pretty(&signed)?);
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
fn normalize_path(path: &str) -> Result<String, Box<dyn Error>> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Torii receipt path must not be empty".into());
    }
    if trimmed.contains('?') {
        return Err("Torii receipt path must not contain a query string".into());
    }
    if trimmed.starts_with('/') {
        Ok(trimmed.to_owned())
    } else {
        Ok(format!("/{trimmed}"))
    }
}
fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}
fn default_nonce(artifact: &VpnSettlementSpoolRecord, timestamp_ms: u64) -> String {
    format!("vpn-settle:{}:{timestamp_ms}", artifact.session_id_hex)
}
fn decode_seed(raw: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let normalized = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if normalized.len() != 64 {
        return Err("private key seed must contain exactly 64 hex characters".into());
    }
    let mut seed = [0_u8; 32];
    hex::decode_to_slice(normalized, &mut seed)?;
    Ok(seed)
}
fn request_body(record: &VpnSettlementSpoolRecord) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(json::to_vec(&record.submit_receipt_request)?)
}
fn canonical_request_message(method: &str, path: &str, body: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash = hasher.finalize();
    format!(
        "{}\n{}\n\n{}",
        method.to_ascii_uppercase(),
        path,
        hex::encode(body_hash)
    )
    .into_bytes()
}
fn canonical_network_request_signature_message(
    network_id: &NetworkId,
    method: &str,
    path: &str,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    const DOMAIN: &[u8] = b"iroha.app.request.network.v1\0";
    let request = canonical_request_message(method, path, body);
    let mut message =
        Vec::with_capacity(DOMAIN.len() + network_id.as_bytes().len() + request.len());
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(network_id.as_bytes());
    message.extend_from_slice(&request);
    message.push(b'\n');
    message.extend_from_slice(timestamp_ms.to_string().as_bytes());
    message.push(b'\n');
    message.extend_from_slice(nonce.as_bytes());
    message
}
/// Render an exact account input into the strict ASCII auth-header form.
fn canonical_account_header_value(account_id: &str) -> Result<String, Box<dyn Error>> {
    if account_id.is_empty() || account_id.trim() != account_id {
        return Err("account id must be exact and non-empty".into());
    }
    match AccountAddress::parse_encoded(account_id, None) {
        Ok(address) => address
            .canonical_hex()
            .map_err(|err| format!("failed to encode canonical account header: {err}").into()),
        Err(_) if account_id.bytes().all(|byte| byte.is_ascii_graphic()) => {
            Ok(account_id.to_owned())
        }
        Err(_) => Err(
            "account id must be a canonical I105 account or printable ASCII account alias".into(),
        ),
    }
}
fn sign_artifact(
    artifact: &VpnSettlementSpoolRecord,
    account_id: &str,
    network_id: &NetworkId,
    seed: &[u8; 32],
    path: &str,
    torii_root: Option<&str>,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<SignedSettlementRequest, Box<dyn Error>> {
    let body = request_body(artifact)?;
    let account_header = canonical_account_header_value(account_id)?;
    let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .map_err(|err| format!("failed to derive settlement signing key: {err}"))?;
    let message = canonical_network_request_signature_message(
        network_id,
        "POST",
        path,
        &body,
        timestamp_ms,
        nonce,
    );
    let signature = Signature::try_new(key_pair.private_key(), &message)?;
    let body = String::from_utf8(body)?;
    let url = torii_root.map(|root| request_url(root, path));
    Ok(SignedSettlementRequest {
        network_id: *network_id,
        method: "POST".to_owned(),
        url,
        path: path.to_owned(),
        body,
        headers: vec![
            SignedHeader {
                name: "Content-Type".to_owned(),
                value: "application/json".to_owned(),
            },
            SignedHeader {
                name: HEADER_ACCOUNT.to_owned(),
                value: account_header,
            },
            SignedHeader {
                name: HEADER_SIGNATURE.to_owned(),
                value: BASE64_STANDARD.encode(signature.payload()),
            },
            SignedHeader {
                name: HEADER_TIMESTAMP_MS.to_owned(),
                value: timestamp_ms.to_string(),
            },
            SignedHeader {
                name: HEADER_NONCE.to_owned(),
                value: nonce.to_owned(),
            },
        ],
    })
}
fn request_url(root: &str, path: &str) -> String {
    format!("{}{}", root.trim_end_matches('/'), path)
}
fn render_curl(signed: &SignedSettlementRequest) -> Result<String, Box<dyn Error>> {
    let url = signed
        .url
        .as_deref()
        .ok_or("curl output requires --torii-root so the request URL can be rendered")?;
    let mut command = format!("curl -sS -X {} {}", signed.method, shell_quote(url));
    for header in &signed.headers {
        command.push_str(" \\\n  -H ");
        command.push_str(&shell_quote(&format!("{}: {}", header.name, header.value)));
    }
    command.push_str(" \\\n  --data-binary ");
    command.push_str(&shell_quote(&signed.body));
    Ok(command)
}
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;
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
            torii_receipt_path: DEFAULT_PATH.to_owned(),
            submit_receipt_request: VpnSettlementSubmitRequestArtifact {
                relay_receipt_hex: "33".repeat(4),
                client_voucher_hex: "44".repeat(4),
                lease_id_hex: "11".repeat(32),
            },
        }
    }
    #[test]
    fn signs_spooled_artifact_with_verifiable_canonical_message() {
        const ACCOUNT_I105: &str = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
        let record = sample_record();
        let seed = [0x66; 32];
        let network_id = test_network_id(0x61);
        let signed = sign_artifact(
            &record,
            ACCOUNT_I105,
            &network_id,
            &seed,
            DEFAULT_PATH,
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
        let expected_account_header = AccountAddress::parse_encoded(ACCOUNT_I105, None)
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
            "POST",
            DEFAULT_PATH,
            signed.body.as_bytes(),
            1_700_000_000_123,
            "nonce-1",
        );
        let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
            .expect("derive settlement fixture key");
        signature
            .verify(key_pair.public_key(), &message)
            .expect("signature verifies");
        let foreign_message = canonical_network_request_signature_message(
            &test_network_id(0x62),
            "POST",
            DEFAULT_PATH,
            signed.body.as_bytes(),
            1_700_000_000_123,
            "nonce-1",
        );
        signature
            .verify(key_pair.public_key(), &foreign_message)
            .expect_err("same request must not verify for a different genesis network");
    }
    #[test]
    fn curl_output_contains_signed_headers_and_body() {
        let record = sample_record();
        let signed = sign_artifact(
            &record,
            "operator",
            &test_network_id(0x63),
            &[0x11; 32],
            DEFAULT_PATH,
            Some("https://torii.example"),
            7,
            "nonce-2",
        )
        .expect("signed request");
        let curl = render_curl(&signed).expect("curl output");
        assert!(curl.contains("https://torii.example/v1/vpn/receipts"));
        assert!(curl.contains("X-Iroha-Account: operator"));
        assert!(curl.contains("X-Iroha-Nonce: nonce-2"));
        assert!(curl.contains("relay_receipt_hex"));
    }
    #[test]
    fn account_header_rejects_inexact_or_non_ascii_aliases() {
        for invalid in [
            " operator",
            "operator ",
            "operator alias",
            "operator\n",
            "账户",
        ] {
            canonical_account_header_value(invalid)
                .expect_err("only exact printable ASCII aliases are supported");
        }
    }
    #[test]
    fn normalize_path_rejects_query_strings() {
        assert!(normalize_path("/v1/vpn/receipts?x=1").is_err());
        assert_eq!(
            normalize_path("v1/vpn/receipts").expect("normalized"),
            DEFAULT_PATH
        );
    }
    #[test]
    fn seed_decode_rejects_length_before_hex_allocation() {
        assert_eq!(
            decode_seed(&"ab".repeat(32)).expect("valid seed"),
            [0xab; 32]
        );
        let error = decode_seed(&"ab".repeat(33)).expect_err("max+1 seed must fail");
        assert!(error.to_string().contains("exactly 64"), "{error}");
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
