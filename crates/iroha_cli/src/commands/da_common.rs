//! Shared data-availability ingest helpers reused by the Taikai tooling and the
//! standalone `iroha da` commands.

use std::{path::Path, str::FromStr};

use eyre::{Result, WrapErr, eyre};
use iroha::{
    config::Config,
    da::{DaManifestBundle, DaSamplingPlan},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    da::{
        ingest::DaIngestReceipt,
        manifest::DaManifestV1,
        types::{
            BlobClass, ExtraMetadata, FecScheme, MetadataEncryption, MetadataEntry,
            MetadataVisibility,
        },
    },
    sorafs::pin_registry::StorageClass,
};
use iroha_torii_shared::da::sampling::build_sampling_plan;
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
    pub(super) sampling_plan: Option<Value>,
    pub(super) sampling_plan_typed: Option<DaSamplingPlan>,
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

    pub(super) fn fetch(
        &self,
        ticket_hex: &str,
        block_hash_hex: Option<&str>,
    ) -> Result<DaManifestFetchBundle> {
        let suffix = block_hash_hex.map_or_else(
            || ticket_hex.to_owned(),
            |hash| format!("{ticket_hex}?block_hash={hash}"),
        );
        let url = self
            .endpoint
            .join(&suffix)
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
        let manifest = parsed
            .decode_manifest()
            .map_err(|err| eyre!("failed to decode embedded manifest: {err}"))?;
        if let (Some(plan), Some(block_hex)) = (parsed.sampling_plan.as_ref(), block_hash_hex) {
            let block_hash = Hash::from_str(block_hex)
                .map_err(|err| eyre!("invalid block hash `{block_hex}`: {err}"))?;
            validate_sampling_plan(&manifest, plan, &block_hash)?;
        }

        let manifest_bytes = parsed.manifest_bytes.clone();
        let manifest_json = parsed.manifest_json.clone();
        let chunk_plan = parsed.chunk_plan.clone();
        let sampling_plan = value.get("sampling_plan").cloned();
        let sampling_plan_typed = parsed.sampling_plan;
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
            sampling_plan,
            sampling_plan_typed,
        })
    }
}

fn validate_sampling_plan(
    manifest: &DaManifestV1,
    plan: &DaSamplingPlan,
    block_hash: &Hash,
) -> Result<()> {
    let expected = build_sampling_plan(manifest, block_hash);
    if plan.assignment_hash != expected.assignment_hash {
        return Err(eyre!(
            "sampling plan assignment hash does not match block/manifest"
        ));
    }
    if plan.sample_window != expected.sample_window {
        return Err(eyre!(
            "sampling plan window {} differs from expected {}",
            plan.sample_window,
            expected.sample_window
        ));
    }
    let sample_seed = plan
        .sample_seed
        .ok_or_else(|| eyre!("sampling plan missing sample_seed"))?;
    if sample_seed != expected.sample_seed {
        return Err(eyre!(
            "sampling plan seed does not match deterministic sampler"
        ));
    }
    let expected_samples: Vec<_> = expected
        .samples
        .iter()
        .map(|sample| iroha::da::DaSampledChunk {
            index: sample.chunk_index,
            role: sample.role,
            group: sample.group_id,
        })
        .collect();
    if plan.samples != expected_samples {
        return Err(eyre!(
            "sampling plan samples differ from deterministic sampler ({} vs {})",
            plan.samples.len(),
            expected.samples.len()
        ));
    }
    Ok(())
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
    use iroha_data_model::{
        da::{
            manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
            types::{
                BlobClass, BlobCodec, BlobDigest, ErasureProfile, ExtraMetadata, RetentionPolicy,
                StorageTicketId,
            },
        },
        nexus::LaneId,
    };
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

    #[test]
    fn sampling_plan_validation_detects_mismatch() {
        let manifest = sample_manifest(8);
        let block_hash = Hash::new(b"deterministic-sampling");
        let expected = build_sampling_plan(&manifest, &block_hash);
        let mut plan = DaSamplingPlan {
            assignment_hash: expected.assignment_hash,
            sample_window: expected.sample_window,
            sample_seed: Some(expected.sample_seed),
            samples: expected
                .samples
                .iter()
                .map(|sample| iroha::da::DaSampledChunk {
                    index: sample.chunk_index,
                    role: sample.role,
                    group: sample.group_id,
                })
                .collect(),
        };
        validate_sampling_plan(&manifest, &plan, &block_hash).expect("plan must validate");

        plan.assignment_hash = BlobDigest::new([0xAA; 32]);
        let outcome = validate_sampling_plan(&manifest, &plan, &block_hash);
        assert!(outcome.is_err(), "mismatched plan must fail validation");
    }

    fn sample_manifest(chunk_count: u32) -> DaManifestV1 {
        let chunks = (0..chunk_count)
            .map(|idx| {
                ChunkCommitment::new_with_role(
                    idx,
                    u64::from(idx) * 1024,
                    1024,
                    BlobDigest::new(
                        [u8::try_from(idx).expect("chunk index fits in digest byte"); 32],
                    ),
                    if idx % 2 == 0 {
                        ChunkRole::Data
                    } else {
                        ChunkRole::GlobalParity
                    },
                    idx / 2,
                )
            })
            .collect();
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x01; 32]),
            lane_id: LaneId::new(1),
            epoch: 1,
            blob_class: BlobClass::NexusLaneSidecar,
            codec: BlobCodec::new("test/binary"),
            blob_hash: BlobDigest::new([0x02; 32]),
            chunk_root: BlobDigest::new([0x03; 32]),
            storage_ticket: StorageTicketId::new([0x04; 32]),
            total_size: u64::from(chunk_count) * 1024,
            chunk_size: 1024,
            total_stripes: chunk_count,
            shards_per_stripe: 4,
            erasure_profile: ErasureProfile::default(),
            retention_policy: RetentionPolicy::default(),
            rent_quote: iroha_data_model::da::types::DaRentQuote::default(),
            chunks,
            ipa_commitment: BlobDigest::new([0x05; 32]),
            metadata: ExtraMetadata::default(),
            issued_at_unix: 1_701_000_000,
        }
    }
}
