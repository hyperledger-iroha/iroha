//! Shared data-availability ingest helpers reused by the Taikai tooling and the
//! standalone `iroha da` commands.
use eyre::{Result, WrapErr, eyre};
use iroha::{
    client::{
        canonical_network_request_signature_message, canonical_request_account_header_value,
        canonical_request_signature_header_value, canonical_request_timestamp_header_value,
    },
    config::Config,
    crypto::{KeyPair, Signature},
    da::DaManifestBundle,
    http::Method as HttpMethod,
};
use iroha_data_model::{
    NetworkId,
    da::{
        ingest::{DaIngestReceipt, DaIngestRequest, DaPinScopeV1},
        types::{
            BlobClass, ExtraMetadata, FecScheme, MetadataEncryption, MetadataEntry,
            MetadataVisibility,
        },
    },
    prelude::AccountId,
    sorafs::pin_registry::StorageClass,
};
use norito::{
    decode_from_bytes,
    json::{Map, Value},
    to_bytes,
};
use reqwest::{
    blocking::Client as HttpClient,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use std::{
    io::Read as _,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
const DA_INGEST_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;
const HEADER_SORA_PDP_COMMITMENT: &str = "sora-pdp-commitment";
const HEADER_IROHA_ACCOUNT: &str = "x-iroha-account";
const HEADER_IROHA_SIGNATURE: &str = "x-iroha-signature";
const HEADER_IROHA_TIMESTAMP_MS: &str = "x-iroha-timestamp-ms";
const HEADER_IROHA_NONCE: &str = "x-iroha-nonce";
/// Blocking Torii publisher for `/v1/da/ingest`.
pub(super) struct DaPublisher {
    client: HttpClient,
    endpoint: Url,
    basic_auth: Option<(String, String)>,
    network_id: NetworkId,
    account: AccountId,
    key_pair: KeyPair,
}
/// Receipt bundle containing Norito bytes, rendered JSON, and the typed record.
pub(super) struct DaPublisherReceipt {
    pub(super) status: String,
    pub(super) duplicate: bool,
    pub(super) bytes: Vec<u8>,
    pub(super) json: String,
    pub(super) receipt: DaIngestReceipt,
    pub(super) pin_scope: Option<DaPinScopeV1>,
    pub(super) submitted_request: DaIngestRequest,
    pub(super) pdp_commitment_header: Option<String>,
}
#[derive(Debug, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
struct DaIngestResponsePayload {
    status: String,
    duplicate: bool,
    receipt: Option<DaIngestReceipt>,
    pin_scope: Option<DaPinScopeV1>,
}
/// Blocking Torii fetcher for `/v1/da/manifests/{ticket}`.
pub(super) struct DaManifestFetcher {
    client: HttpClient,
    endpoint: Url,
    basic_auth: Option<(String, String)>,
}
/// Response bundle returned by [`DaManifestFetcher`].
pub(super) struct DaManifestFetchBundle {
    pub(super) manifest_bytes: Vec<u8>,
    pub(super) manifest_json: Value,
    pub(super) chunk_plan: Value,
    pub(super) storage_ticket_hex: String,
    pub(super) manifest_hash_hex: String,
    pub(super) blob_hash_hex: String,
}
impl DaPublisher {
    /// Build a publisher using CLI config (Torii URL + basic auth).
    pub(super) fn new(config: &Config, endpoint_override: Option<&str>) -> Result<Self> {
        let endpoint = if let Some(url) = endpoint_override {
            Url::parse(url).map_err(|err| eyre!("invalid DA endpoint `{url}`: {err}"))?
        } else {
            config
                .torii_api_url
                .join("v1/da/ingest")
                .wrap_err("failed to derive /v1/da/ingest from torii_api_url")?
        };
        let mut client_builder = HttpClient::builder().redirect(reqwest::redirect::Policy::none());
        if !config.torii_request_timeout.is_zero() {
            client_builder = client_builder.timeout(config.torii_request_timeout);
        }
        let client = client_builder
            .build()
            .wrap_err("failed to build HTTP client for DA ingest")?;
        let basic_auth = config.basic_auth.as_ref().map(|auth| {
            (
                auth.web_login.as_str().to_owned(),
                auth.password.expose_secret().to_owned(),
            )
        });
        Ok(Self {
            client,
            endpoint,
            basic_auth,
            network_id: config.network_id,
            account: config.account.clone(),
            key_pair: config.key_pair.clone(),
        })
    }
    /// Submit the encoded request to Torii and return the finalized receipt bundle.
    ///
    /// The on-disk request format remains Norito. The HTTP route is JSON-only, so the
    /// request is decoded and rendered as canonical Norito JSON for transport. If Torii
    /// returns a durable pin scope, this method signs that exact scope and retries once.
    pub(super) fn publish(&self, request_bytes: &[u8]) -> Result<DaPublisherReceipt> {
        let request: DaIngestRequest = decode_from_bytes(request_bytes)
            .map_err(|err| eyre!("failed to decode DA ingest request: {err}"))?;
        if request.network_id != self.network_id || request.owner != self.account {
            return Err(eyre!(
                "DA ingest request network or owner does not match the configured canonical HTTP signer"
            ));
        }
        request
            .verify_signatures()
            .map_err(|err| eyre!("DA ingest request signatures are invalid: {err}"))?;
        let first = self.publish_once(&request)?;
        validate_da_ingest_response(&request, &first)?;
        let (response, submitted_request) = match first.status.as_str() {
            "accepted" => (first, request),
            "pending_pin_authorization" => {
                let scope = first.pin_scope.as_ref().ok_or_else(|| {
                    eyre!("Torii returned `pending_pin_authorization` without a DA pin scope")
                })?;
                let mut authorized_request = request.clone();
                authorized_request
                    .try_add_pin_scope_signature(scope, &self.key_pair)
                    .map_err(|err| eyre!("failed to authorize DA pin scope: {err}"))?;
                let retried = self.publish_once(&authorized_request)?;
                validate_da_ingest_response(&authorized_request, &retried)?;
                if retried.status != "accepted" && retried.status != "pending_pin_authorization" {
                    return Err(eyre!(
                        "Torii returned unexpected DA ingest status `{}` after pin authorization",
                        retried.status
                    ));
                }
                if retried.pin_scope != first.pin_scope
                    || retried.receipt != first.receipt
                    || retried.pdp_commitment_header != first.pdp_commitment_header
                {
                    return Err(eyre!(
                        "Torii changed the durable DA receipt, pin scope, or commitment header during authorization"
                    ));
                }
                (retried, authorized_request)
            }
            status => {
                return Err(eyre!(
                    "Torii returned unexpected DA ingest status `{status}`"
                ));
            }
        };
        let receipt = response
            .receipt
            .ok_or_else(|| eyre!("Torii DA ingest response is missing its receipt"))?;
        let bytes = to_bytes(&receipt)
            .map_err(|err| eyre!("failed to encode DA ingest receipt as Norito: {err}"))?;
        let json = norito::json::to_json_pretty(&receipt)
            .map_err(|err| eyre!("failed to render DA receipt JSON: {err}"))?;
        Ok(DaPublisherReceipt {
            status: response.status,
            duplicate: response.duplicate,
            bytes,
            json,
            receipt,
            pin_scope: response.pin_scope,
            submitted_request,
            pdp_commitment_header: response.pdp_commitment_header,
        })
    }

    fn publish_once(&self, request_body: &DaIngestRequest) -> Result<DaIngestResponse> {
        let request_json = norito::json::to_vec(request_body)
            .map_err(|err| eyre!("failed to encode DA ingest request JSON: {err}"))?;
        let auth = self.canonical_auth_headers(&request_json)?;
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .header(HEADER_IROHA_ACCOUNT, auth.account)
            .header(HEADER_IROHA_SIGNATURE, auth.signature)
            .header(HEADER_IROHA_TIMESTAMP_MS, auth.timestamp_ms)
            .header(HEADER_IROHA_NONCE, auth.nonce)
            .body(request_json);
        if let Some((ref login, ref password)) = self.basic_auth {
            request = request.basic_auth(login, Some(password));
        }
        let mut response = request
            .send()
            .wrap_err("failed to submit DA ingest request to Torii")?;
        let status = response.status();
        let pdp_commitment_header = extract_pdp_header(response.headers())?;
        if response
            .content_length()
            .is_some_and(|length| length > DA_INGEST_RESPONSE_MAX_BYTES_V1 as u64)
        {
            return Err(eyre!(
                "Torii DA ingest response exceeds the first-release {}-byte limit",
                DA_INGEST_RESPONSE_MAX_BYTES_V1
            ));
        }
        let read_limit = u64::try_from(DA_INGEST_RESPONSE_MAX_BYTES_V1)
            .expect("DA ingest response limit fits u64")
            .saturating_add(1);
        let mut bytes = Vec::new();
        response
            .take(read_limit)
            .read_to_end(&mut bytes)
            .wrap_err("failed to read DA ingest response body")?;
        if bytes.len() > DA_INGEST_RESPONSE_MAX_BYTES_V1 {
            return Err(eyre!(
                "Torii DA ingest response exceeds the first-release {}-byte limit",
                DA_INGEST_RESPONSE_MAX_BYTES_V1
            ));
        }
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&bytes);
            return Err(eyre!(
                "Torii /v1/da/ingest responded with {}: {}",
                status,
                preview
            ));
        }
        let payload: DaIngestResponsePayload = norito::json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to decode DA ingest response JSON: {err}"))?;
        Ok(DaIngestResponse {
            status: payload.status,
            duplicate: payload.duplicate,
            receipt: payload.receipt,
            pin_scope: payload.pin_scope,
            pdp_commitment_header,
        })
    }

    fn canonical_auth_headers(&self, body: &[u8]) -> Result<CanonicalAuthHeaders> {
        use rand::{rand_core::TryRngCore as _, rngs::OsRng};

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("DA canonical request clock precedes the Unix epoch")?
            .as_millis()
            .try_into()
            .wrap_err("DA canonical request timestamp exceeds u64 milliseconds")?;
        let mut nonce_bytes = [0_u8; 16];
        OsRng
            .try_fill_bytes(&mut nonce_bytes)
            .map_err(|err| eyre!("DA canonical request nonce OS RNG failed: {err}"))?;
        let nonce = hex::encode(nonce_bytes);
        let message = canonical_network_request_signature_message(
            &self.network_id,
            &HttpMethod::POST,
            &self.endpoint,
            body,
            timestamp_ms,
            &nonce,
        )
        .wrap_err("failed to build canonical DA request authentication message")?;
        let signature = Signature::try_new(self.key_pair.private_key(), &message)
            .wrap_err("failed to sign canonical DA request headers")?;
        Ok(CanonicalAuthHeaders {
            account: canonical_request_account_header_value(&self.account)
                .wrap_err("failed to encode canonical DA request account")?,
            signature: canonical_request_signature_header_value(&signature)
                .wrap_err("failed to encode canonical DA request signature")?,
            timestamp_ms: canonical_request_timestamp_header_value(timestamp_ms)
                .wrap_err("failed to encode canonical DA request timestamp")?,
            nonce,
        })
    }
}
struct CanonicalAuthHeaders {
    account: String,
    signature: String,
    timestamp_ms: String,
    nonce: String,
}
struct DaIngestResponse {
    status: String,
    duplicate: bool,
    receipt: Option<DaIngestReceipt>,
    pin_scope: Option<DaPinScopeV1>,
    pdp_commitment_header: Option<String>,
}
fn validate_da_ingest_response(
    request: &DaIngestRequest,
    response: &DaIngestResponse,
) -> Result<()> {
    if response.status != "accepted" && response.status != "pending_pin_authorization" {
        return Err(eyre!(
            "Torii returned unexpected DA ingest status `{}`",
            response.status
        ));
    }
    let receipt = response
        .receipt
        .as_ref()
        .ok_or_else(|| eyre!("Torii DA ingest response is missing its receipt"))?;
    let scope = response
        .pin_scope
        .as_ref()
        .ok_or_else(|| eyre!("Torii DA ingest response is missing its durable pin scope"))?;
    if !scope.matches_authorization(&request.authorization()) {
        return Err(eyre!(
            "Torii DA pin scope does not match the submitted request authorization"
        ));
    }
    if receipt.client_blob_id != request.client_blob_id
        || receipt.lane_id != request.lane_id
        || receipt.epoch != request.epoch
        || receipt.blob_hash != request.payload_hash
        || receipt.storage_ticket != scope.storage_ticket
        || receipt.manifest_hash.as_bytes() != scope.manifest_hash.as_bytes()
    {
        return Err(eyre!(
            "Torii DA receipt does not match the submitted request and durable pin scope"
        ));
    }
    Ok(())
}
fn extract_pdp_header(headers: &reqwest::header::HeaderMap) -> Result<Option<String>> {
    headers
        .get(HEADER_SORA_PDP_COMMITMENT)
        .map_or_else(|| Ok(None), |value| parse_header_value(value).map(Some))
}
fn parse_header_value(value: &HeaderValue) -> Result<String> {
    value
        .to_str()
        .map(|raw| raw.trim().to_string())
        .map_err(|err| eyre!("invalid {HEADER_SORA_PDP_COMMITMENT} header: {err}"))
}
impl DaManifestFetcher {
    pub(super) fn new(config: &Config, endpoint_override: Option<&str>) -> Result<Self> {
        let endpoint = if let Some(url) = endpoint_override {
            Url::parse(url).map_err(|err| eyre!("invalid DA manifest endpoint `{url}`: {err}"))?
        } else {
            config
                .torii_api_url
                .join("v1/da/manifests/")
                .wrap_err("failed to derive /v1/da/manifests from torii_api_url")?
        };
        let client = HttpClient::builder()
            .build()
            .wrap_err("failed to build HTTP client for DA manifest fetch")?;
        let basic_auth = config.basic_auth.as_ref().map(|auth| {
            (
                auth.web_login.as_str().to_owned(),
                auth.password.expose_secret().to_owned(),
            )
        });
        Ok(Self {
            client,
            endpoint,
            basic_auth,
        })
    }
    pub(super) fn fetch(&self, ticket_hex: &str) -> Result<DaManifestFetchBundle> {
        let url = self
            .endpoint
            .join(ticket_hex)
            .wrap_err("failed to build DA manifest fetch URL")?;
        let mut request = self.client.get(url).header(ACCEPT, "application/json");
        if let Some((ref login, ref password)) = self.basic_auth {
            request = request.basic_auth(login, Some(password));
        }
        let response = request
            .send()
            .wrap_err("failed to fetch DA manifest from Torii")?;
        let status = response.status();
        let bytes = response
            .bytes()
            .wrap_err("failed to read DA manifest response body")?
            .to_vec();
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&bytes);
            return Err(eyre!(
                "Torii /v1/da/manifests responded with {}: {}",
                status,
                preview
            ));
        }
        let value: Value = norito::json::from_slice(&bytes)
            .map_err(|err| eyre!("failed to parse DA manifest response: {err}"))?;
        let parsed = DaManifestBundle::from_json(&value)
            .map_err(|err| eyre!("failed to decode DA manifest bundle: {err}"))?;
        let manifest_bytes = parsed.manifest_bytes.clone();
        let manifest_json = parsed.manifest_json.clone();
        let chunk_plan = parsed.chunk_plan.clone();
        let storage_ticket_hex = parsed.storage_ticket_hex;
        let manifest_hash_hex = parsed.manifest_hash_hex;
        let blob_hash_hex = parsed.blob_hash_hex;
        Ok(DaManifestFetchBundle {
            manifest_bytes,
            manifest_json,
            chunk_plan,
            storage_ticket_hex,
            manifest_hash_hex,
            blob_hash_hex,
        })
    }
}
/// Convert a JSON metadata map into the Norito `ExtraMetadata` structure.
pub(super) fn metadata_map_to_extra(map: &Map) -> Result<ExtraMetadata> {
    let mut items = Vec::with_capacity(map.len());
    for (key, value) in map {
        let str_value = value
            .as_str()
            .ok_or_else(|| eyre!("metadata entry `{key}` must be a string"))?;
        items.push(MetadataEntry {
            key: key.clone(),
            value: str_value.as_bytes().to_vec(),
            visibility: MetadataVisibility::Public,
            encryption: MetadataEncryption::None,
        });
    }
    Ok(ExtraMetadata { items })
}
/// Parse a blob-class label (supports aliases such as `taikai`).
pub(super) fn parse_blob_class(label: &str) -> Result<BlobClass> {
    match label.to_ascii_lowercase().as_str() {
        "taikai_segment" | "taikai" => Ok(BlobClass::TaikaiSegment),
        "nexus_lane_sidecar" | "sidecar" => Ok(BlobClass::NexusLaneSidecar),
        "governance_artifact" | "governance" => Ok(BlobClass::GovernanceArtifact),
        other if other.starts_with("custom:") => {
            let suffix = other.trim_start_matches("custom:");
            let value = suffix
                .parse::<u16>()
                .map_err(|err| eyre!("invalid custom blob class `{label}`: {err}"))?;
            Ok(BlobClass::Custom(value))
        }
        other => Err(eyre!("unsupported blob class `{other}`")),
    }
}
/// Parse an erasure FEC scheme identifier.
pub(super) fn parse_fec_scheme(label: &str) -> Result<FecScheme> {
    match label.to_ascii_lowercase().as_str() {
        "rs12_10" | "rs12-10" => Ok(FecScheme::Rs12_10),
        "rswin14_10" | "rswin14-10" => Ok(FecScheme::RsWin14_10),
        "rs18_14" | "rs18-14" => Ok(FecScheme::Rs18_14),
        other if other.starts_with("custom:") => {
            let suffix = other.trim_start_matches("custom:");
            let value = suffix
                .parse::<u16>()
                .map_err(|err| eyre!("invalid custom FEC scheme `{label}`: {err}"))?;
            Ok(FecScheme::Custom(value))
        }
        other => Err(eyre!("unsupported FEC scheme `{other}`")),
    }
}
/// Parse a storage-class alias into the Norito enum.
pub(super) fn parse_storage_class(label: &str) -> Result<StorageClass> {
    match label.to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        other => Err(eyre!("unsupported storage class `{other}`")),
    }
}
/// Load a Norito JSON file containing simple metadata entries.
pub(super) fn load_metadata_from_path(path: &Path) -> Result<ExtraMetadata> {
    let bytes = std::fs::read(path)
        .wrap_err_with(|| format!("failed to read metadata JSON `{}`", path.display()))?;
    let value: Value = norito::json::from_slice(&bytes)
        .map_err(|err| eyre!("failed to parse metadata JSON `{}`: {err}", path.display()))?;
    let map = value
        .as_object()
        .ok_or_else(|| eyre!("metadata JSON `{}` must be an object", path.display()))?;
    metadata_map_to_extra(map)
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD as Base64Standard};
    use iroha::{
        crypto::{Algorithm, Hash, HashOf, Signature},
        da::{DaIngestParams, build_da_request},
    };
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        da::{
            ingest::DaStripeLayout,
            types::{BlobDigest, DaRentQuote, StorageTicketId},
        },
        prelude::AccountId,
        sorafs::pin_registry::ManifestDigest,
    };
    use std::{
        collections::BTreeMap,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
    };

    fn publisher_request_fixture() -> (KeyPair, DaIngestRequest) {
        let key_pair = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519)
            .expect("derive deterministic DA publisher key");
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; 32])),
        );
        let request = build_da_request(
            network_id,
            AccountId::new(key_pair.public_key().clone()),
            vec![0xA1, 0xB2, 0xC3],
            &DaIngestParams::default(),
            ExtraMetadata::default(),
            &key_pair,
            None,
        )
        .expect("build deterministic DA publisher request");
        (key_pair, request)
    }

    fn receipt_for_request(request: &DaIngestRequest) -> DaIngestReceipt {
        let operator = KeyPair::try_from_seed(vec![0x43; 32], Algorithm::Ed25519)
            .expect("derive deterministic receipt operator key");
        DaIngestReceipt {
            client_blob_id: request.client_blob_id,
            lane_id: request.lane_id,
            epoch: request.epoch,
            blob_hash: request.payload_hash,
            chunk_root: BlobDigest::new([0x44; 32]),
            manifest_hash: BlobDigest::new([0x45; 32]),
            storage_ticket: StorageTicketId::new([0x46; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            },
            queued_at_unix: 1,
            rent_quote: DaRentQuote::default(),
            operator_signature: Signature::try_new(operator.private_key(), b"receipt")
                .expect("sign deterministic receipt fixture"),
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 4096];
        let (header_end, body_len) = loop {
            let read = stream.read(&mut chunk).expect("read mock HTTP request");
            assert_ne!(read, 0, "request ended before its headers");
            bytes.extend_from_slice(&chunk[..read]);
            let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let body_len = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("content length"))
                })
                .expect("mock request content length");
            break (header_end + 4, body_len);
        };
        while bytes.len() < header_end + body_len {
            let read = stream.read(&mut chunk).expect("read mock HTTP body");
            assert_ne!(read, 0, "request ended before its body");
            bytes.extend_from_slice(&chunk[..read]);
        }
        bytes
    }

    fn spawn_ingest_server(
        response_bodies: Vec<Vec<u8>>,
    ) -> (Url, thread::JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock DA ingest server");
        let address = listener.local_addr().expect("mock server address");
        let handle = thread::spawn(move || {
            let mut requests = Vec::with_capacity(response_bodies.len());
            for body in response_bodies {
                let (mut stream, _) = listener.accept().expect("accept DA ingest request");
                requests.push(read_http_request(&mut stream));
                write!(
                    stream,
                    "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write mock response headers");
                stream.write_all(&body).expect("write mock response body");
            }
            requests
        });
        (
            Url::parse(&format!("http://{address}/v1/da/ingest")).expect("mock endpoint URL"),
            handle,
        )
    }

    fn request_body(raw_request: &[u8]) -> &[u8] {
        let body_start = raw_request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP header terminator")
            + 4;
        &raw_request[body_start..]
    }

    fn request_header<'a>(raw_request: &'a [u8], expected_name: &str) -> &'a str {
        let header_end = raw_request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP header terminator");
        std::str::from_utf8(&raw_request[..header_end])
            .expect("ASCII HTTP headers")
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case(expected_name)
                    .then(|| value.trim())
            })
            .unwrap_or_else(|| panic!("missing {expected_name} request header"))
    }
    #[test]
    fn parse_blob_class_supports_aliases() {
        assert!(matches!(
            parse_blob_class("taikai_segment").expect("alias"),
            BlobClass::TaikaiSegment
        ));
        assert!(matches!(
            parse_blob_class("custom:42").expect("custom"),
            BlobClass::Custom(42)
        ));
        assert!(parse_blob_class("unknown").is_err());
    }
    #[test]
    fn metadata_map_conversion_preserves_entries() {
        let mut map = Map::new();
        map.insert("da.stream".into(), Value::from("demo"));
        map.insert("codec".into(), Value::from("av1-main"));
        let extra = metadata_map_to_extra(&map).expect("metadata");
        assert_eq!(extra.items.len(), 2);
        let by_key: BTreeMap<_, _> = extra
            .items
            .iter()
            .map(|entry| (entry.key.clone(), entry.value.clone()))
            .collect();
        assert_eq!(
            by_key.get("da.stream").map(Vec::as_slice),
            Some(&b"demo"[..])
        );
        assert_eq!(
            by_key.get("codec").map(Vec::as_slice),
            Some(&b"av1-main"[..])
        );
    }
    #[test]
    fn storage_class_aliases_supported() {
        assert!(matches!(
            parse_storage_class("HOT").expect("hot"),
            StorageClass::Hot
        ));
        assert!(matches!(
            parse_storage_class("warm").expect("warm"),
            StorageClass::Warm
        ));
        assert!(parse_storage_class("unknown").is_err());
    }

    #[test]
    fn publisher_uses_json_and_authorizes_pin_scope_once() {
        let (key_pair, request) = publisher_request_fixture();
        let receipt = receipt_for_request(&request);
        let scope = DaPinScopeV1::new(
            &request.authorization(),
            receipt.storage_ticket,
            ManifestDigest::new(*receipt.manifest_hash.as_bytes()),
            None,
        );
        let pending = norito::json::to_vec(&DaIngestResponsePayload {
            status: "pending_pin_authorization".to_owned(),
            duplicate: false,
            receipt: Some(receipt.clone()),
            pin_scope: Some(scope.clone()),
        })
        .expect("encode pending response");
        let accepted = norito::json::to_vec(&DaIngestResponsePayload {
            status: "accepted".to_owned(),
            duplicate: true,
            receipt: Some(receipt),
            pin_scope: Some(scope.clone()),
        })
        .expect("encode accepted response");
        let (endpoint, server) = spawn_ingest_server(vec![pending, accepted]);
        let auth_endpoint = endpoint.clone();
        let network_id = request.network_id;
        let account = request.owner.clone();
        let public_key = key_pair.public_key().clone();
        let publisher = DaPublisher {
            client: HttpClient::builder()
                .build()
                .expect("build mock HTTP client"),
            endpoint,
            basic_auth: Some(("alice".to_owned(), "secret".to_owned())),
            network_id,
            account: account.clone(),
            key_pair,
        };

        let initial_bytes = to_bytes(&request).expect("encode initial request");
        let result = publisher
            .publish(&initial_bytes)
            .expect("authorize and finalize DA pin scope");
        let requests = server.join().expect("mock server thread");

        assert_eq!(requests.len(), 2, "publisher must retry exactly once");
        let mut nonces = Vec::new();
        for raw in &requests {
            let headers = String::from_utf8_lossy(raw).to_ascii_lowercase();
            assert!(headers.contains("content-type: application/json"));
            assert!(headers.contains("accept: application/json"));
            assert!(headers.contains("authorization: basic ywxpy2u6c2vjcmv0"));
            assert_eq!(
                request_header(raw, HEADER_IROHA_ACCOUNT),
                canonical_request_account_header_value(&account).expect("canonical account")
            );
            let timestamp_ms = request_header(raw, HEADER_IROHA_TIMESTAMP_MS)
                .parse::<u64>()
                .expect("canonical timestamp");
            let nonce = request_header(raw, HEADER_IROHA_NONCE);
            nonces.push(nonce.to_owned());
            let message = canonical_network_request_signature_message(
                &network_id,
                &HttpMethod::POST,
                &auth_endpoint,
                request_body(raw),
                timestamp_ms,
                nonce,
            )
            .expect("rebuild canonical request message");
            let signature_bytes = Base64Standard
                .decode(request_header(raw, HEADER_IROHA_SIGNATURE))
                .expect("decode canonical signature header");
            let signature = Signature::try_from_bytes(&signature_bytes)
                .expect("decode canonical request signature");
            signature
                .verify(&public_key, &message)
                .expect("verify canonical request signature against exact JSON body");
        }
        assert_ne!(nonces[0], nonces[1], "each attempt needs a fresh nonce");
        let first: DaIngestRequest =
            norito::json::from_slice(request_body(&requests[0])).expect("decode prepare request");
        let finalized: DaIngestRequest =
            norito::json::from_slice(request_body(&requests[1])).expect("decode final request");
        assert!(first.pin_scope_signatures.is_empty());
        assert_eq!(first.signing_digest(), finalized.signing_digest());
        assert_eq!(finalized.pin_scope_signatures.len(), 1);
        let witness = &finalized.pin_scope_signatures[0];
        witness
            .signature
            .verify(&witness.signer, &scope.signing_digest())
            .expect("verify exact pin-scope witness");
        assert_eq!(result.status, "accepted");
        assert!(result.duplicate);
        assert_eq!(result.pin_scope, Some(scope));
        assert_eq!(result.submitted_request, finalized);
    }

    #[test]
    fn publisher_rejects_oversized_ingest_response_before_reading_body() {
        let (key_pair, request) = publisher_request_fixture();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind oversized-response server");
        let address = listener.local_addr().expect("oversized-response address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept DA ingest request");
            let _ = read_http_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                DA_INGEST_RESPONSE_MAX_BYTES_V1 + 1
            )
            .expect("write oversized response headers");
        });
        let endpoint =
            Url::parse(&format!("http://{address}/v1/da/ingest")).expect("mock endpoint URL");
        let publisher = DaPublisher {
            client: HttpClient::builder()
                .build()
                .expect("build mock HTTP client"),
            endpoint,
            basic_auth: None,
            network_id: request.network_id,
            account: request.owner.clone(),
            key_pair,
        };

        let error = publisher
            .publish(&to_bytes(&request).expect("encode initial request"))
            .err()
            .expect("oversized response must fail before body allocation");
        server.join().expect("oversized-response server");
        assert!(error.to_string().contains("262144-byte limit"));
    }
}
