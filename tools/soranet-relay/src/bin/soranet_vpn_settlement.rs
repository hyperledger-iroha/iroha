//! Operator helper for SoraNet VPN settlement artifacts.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::{Parser, ValueEnum};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::NetworkId;
use iroha_primitives::numeric::Quantity;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use sha2::{Digest as _, Sha256};

const HEADER_ACCOUNT: &str = "X-Iroha-Account";
const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
const HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
const HEADER_NONCE: &str = "X-Iroha-Nonce";
#[cfg(test)]
const DEFAULT_PATH: &str = "/v1/vpn/receipts";

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
    /// Operator account id to place in X-Iroha-Account.
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
    let bytes = fs::read(path)?;
    Ok(json::from_slice(&bytes)?)
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
    let decoded = hex::decode(normalized)?;
    let seed: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "private key seed must decode to 32 bytes")?;
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
                value: account_id.trim().to_owned(),
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
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::BlockHeader;

    use super::*;

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
        let record = sample_record();
        let seed = [0x66; 32];
        let network_id = test_network_id(0x61);
        let signed = sign_artifact(
            &record,
            "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
    fn normalize_path_rejects_query_strings() {
        assert!(normalize_path("/v1/vpn/receipts?x=1").is_err());
        assert_eq!(
            normalize_path("v1/vpn/receipts").expect("normalized"),
            DEFAULT_PATH
        );
    }
}
