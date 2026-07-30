//! Shared data-availability ingest helpers reused by the Taikai tooling and the
//! standalone `iroha da` commands.

use std::path::Path;

use eyre::{Result, WrapErr, eyre};
use iroha::{config::Config, da::DaManifestBundle};
use iroha_data_model::{
    da::{
        ingest::DaIngestReceipt,
        types::{
            BlobClass, ExtraMetadata, FecScheme, MetadataEncryption, MetadataEntry,
            MetadataVisibility,
        },
    },
    sorafs::pin_registry::StorageClass,
};
use norito::{
    decode_from_bytes,
    json::{Map, Value},
};
use reqwest::{
    blocking::Client as HttpClient,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use url::Url;

const HEADER_SORA_PDP_COMMITMENT: &str = "sora-pdp-commitment";

/// Blocking Torii publisher for `/v1/da/ingest`.
pub(super) struct DaPublisher {
    client: HttpClient,
    endpoint: Url,
    basic_auth: Option<(String, String)>,
}

/// Receipt bundle containing Norito bytes, rendered JSON, and the typed record.
pub(super) struct DaPublisherReceipt {
    pub(super) bytes: Vec<u8>,
    pub(super) json: String,
    pub(super) receipt: DaIngestReceipt,
    pub(super) pdp_commitment_header: Option<String>,
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
        let client = HttpClient::builder()
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
        })
    }

    /// Submit the encoded Norito payload to Torii and return the receipt bundle.
    pub(super) fn publish(&self, request_bytes: &[u8]) -> Result<DaPublisherReceipt> {
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "application/x-norito")
            .header(ACCEPT, "application/x-norito")
            .body(request_bytes.to_vec());
        if let Some((ref login, ref password)) = self.basic_auth {
            request = request.basic_auth(login, Some(password));
        }
        let response = request
            .send()
            .wrap_err("failed to submit DA ingest request to Torii")?;
        let status = response.status();
        let header_value = extract_pdp_header(response.headers())?;
        let bytes = response
            .bytes()
            .wrap_err("failed to read DA ingest response body")?
            .to_vec();
        if !status.is_success() {
            let preview = String::from_utf8_lossy(&bytes);
            return Err(eyre!(
                "Torii /v1/da/ingest responded with {}: {}",
                status,
                preview
            ));
        }
        let receipt: DaIngestReceipt = decode_from_bytes(&bytes)
            .map_err(|err| eyre!("failed to decode DA ingest receipt: {err}"))?;
        let json = norito::json::to_json_pretty(&receipt)
            .map_err(|err| eyre!("failed to render DA receipt JSON: {err}"))?;
        Ok(DaPublisherReceipt {
            bytes,
            json,
            receipt,
            pdp_commitment_header: header_value,
        })
    }
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
    use std::collections::BTreeMap;

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
}
