//! Deploy a static HTML page to SoraFS storage, verify fetch, and derive public
//! DNS settings for a regular internet zone pointing at the SoraDNS gateway.

use std::collections::HashMap;

use base64::Engine as _;
use eyre::{Result, eyre};
use integration_tests::sandbox::start_network_async_or_skip;
use iroha_primitives::soradns::{GatewayHostBindings, derive_gateway_hosts};
use iroha_test_network::NetworkBuilder;
use norito::json::{self, Value};
use reqwest::{Client as HttpClient, Url};
use sorafs_car::{CarBuildPlan, CarWriter};
use sorafs_manifest::{
    DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy, StorageClass, chunker_registry,
};

const WELCOME_TEXT: &str = "welcome to SORA Nexus / Hyperledger Iroha 3";
const DNS_TTL_SECS: u32 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DnsRecord {
    hostname: String,
    record_type: &'static str,
    values: Vec<String>,
    ttl_secs: u32,
}

impl DnsRecord {
    fn cname(hostname: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            hostname: hostname.into(),
            record_type: "CNAME",
            values,
            ttl_secs: DNS_TTL_SECS,
        }
    }
}

/// Minimal authoritative DNS stub for public-zone CNAME checks in tests.
struct ReferenceAuthoritativeDns {
    cname_records: HashMap<String, Vec<String>>,
}

impl ReferenceAuthoritativeDns {
    fn new(records: &[DnsRecord]) -> Self {
        let mut cname_records = HashMap::new();
        for record in records {
            if record.record_type == "CNAME" {
                cname_records.insert(normalize_host(&record.hostname), record.values.clone());
            }
        }
        Self { cname_records }
    }

    fn resolve_cname(&self, hostname: &str) -> Option<&[String]> {
        self.cname_records
            .get(&normalize_host(hostname))
            .map(|values| values.as_slice())
    }
}

fn normalize_host(hostname: &str) -> String {
    hostname.trim().trim_end_matches('.').to_ascii_lowercase()
}

// Regular DNS deployments use CNAMEs for subdomains; apex/TLDs need ALIAS/ANAME or A/AAAA.
fn public_dns_settings_for_domain(bindings: &GatewayHostBindings) -> Vec<DnsRecord> {
    let pretty = bindings.pretty_host().to_string();
    vec![DnsRecord::cname(
        bindings.normalized_name().to_string(),
        vec![pretty],
    )]
}

fn build_manifest_and_plan(payload: &[u8]) -> Result<(sorafs_manifest::ManifestV1, CarBuildPlan)> {
    let descriptor = chunker_registry::default_descriptor();
    let plan = CarBuildPlan::single_file_with_profile(payload, descriptor.profile)?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, payload)?.write_to(&mut car_bytes)?;
    let root = stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| eyre!("car emission produced no root CID"))?;
    let manifest = ManifestBuilder::new()
        .root_cid(root)
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_registry(descriptor.id)
        .content_length(plan.content_length)
        .car_digest(stats.car_archive_digest.into())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs {
            council_signatures: Vec::new(),
        })
        .add_metadata("content-type", "text/html; charset=utf-8")
        .build()?;
    Ok((manifest, plan))
}

fn require_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| eyre!("expected string field `{key}` in response"))
}

fn require_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("expected u64 field `{key}` in response"))
}

async fn fetch_payload(
    http: &HttpClient,
    torii_url: &Url,
    manifest_id_hex: &str,
    length: u64,
) -> Result<Vec<u8>> {
    let fetch_url = torii_url.join("/v1/sorafs/storage/fetch")?;
    let request = norito::json!({
        "manifest_id_hex": manifest_id_hex,
        "offset": 0u64,
        "length": length,
    });
    let request_body = json::to_string(&request)?;
    let response = http
        .post(fetch_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(eyre!("storage fetch failed: {} {body}", status));
    }
    let bytes = response.bytes().await?;
    let value: Value = json::from_slice(&bytes)?;
    let data_b64 = require_string(&value, "data_b64")?;
    let payload = base64::engine::general_purpose::STANDARD.decode(data_b64.as_bytes())?;
    Ok(payload)
}

#[tokio::test]
async fn soranet_webpage_deploy_and_dns_settings() -> Result<()> {
    let builder = NetworkBuilder::new().with_config_layer(|layer| {
        layer.write(["sorafs", "storage", "enabled"], true);
    });
    let Some(network) =
        start_network_async_or_skip(builder, stringify!(soranet_webpage_deploy_and_dns_settings))
            .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head><body>{WELCOME_TEXT}</body></html>"
    );
    let payload = html.as_bytes().to_vec();
    let (manifest, plan) = build_manifest_and_plan(&payload)?;
    let manifest_bytes = manifest.encode()?;

    let client = network.client();
    let response = client.post_sorafs_storage_pin(&manifest_bytes, &payload)?;
    assert!(
        response.status().is_success(),
        "storage pin rejected: {}",
        response.status()
    );
    let body = response.body();
    let value: Value = json::from_slice(body)?;
    let manifest_id_hex = require_string(&value, "manifest_id_hex")?;
    let payload_digest_hex = require_string(&value, "payload_digest_hex")?;
    let content_length = require_u64(&value, "content_length")?;

    let expected_manifest_id = hex::encode(&manifest.root_cid);
    let expected_payload_digest = hex::encode(plan.payload_digest.as_bytes());
    assert_eq!(
        manifest_id_hex, expected_manifest_id,
        "manifest_id_hex should match root CID"
    );
    assert_eq!(
        payload_digest_hex, expected_payload_digest,
        "payload_digest_hex should match payload digest"
    );
    assert_eq!(
        content_length,
        payload.len() as u64,
        "content_length should match payload length"
    );

    let http = HttpClient::new();
    let fetched = fetch_payload(
        &http,
        &client.torii_url,
        &manifest_id_hex,
        payload.len() as u64,
    )
    .await?;
    assert_eq!(fetched, payload, "fetched payload mismatch");
    let fetched_str = String::from_utf8(fetched)?;
    assert!(
        fetched_str.contains(WELCOME_TEXT),
        "payload should contain the welcome string"
    );

    let fqdn = "sora-nexus.sora";
    let bindings = derive_gateway_hosts(fqdn)?;
    let dns_records = public_dns_settings_for_domain(&bindings);
    assert_eq!(dns_records.len(), 1, "expected one CNAME record");
    assert!(
        dns_records
            .iter()
            .all(|record| record.record_type == "CNAME"),
        "dns records should be CNAMEs"
    );
    let gateway = ReferenceAuthoritativeDns::new(&dns_records);
    let pretty_host = bindings.pretty_host().to_string();
    assert_eq!(
        gateway.resolve_cname(fqdn),
        Some([pretty_host.clone()].as_slice()),
        "domain CNAME should point at the pretty host"
    );
    assert!(
        gateway.resolve_cname(bindings.canonical_host()).is_none(),
        "canonical host is managed under gw.sora.id, not the public zone"
    );

    network.shutdown().await;
    Ok(())
}
