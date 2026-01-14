//! Deploy a static HTML page to `SoraFS` storage, verify fetch, and derive public
//! DNS settings (including delegation placeholders) for a regular internet zone
//! pointing at the `SoraDNS` gateway.

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
const PLACEHOLDER_NS_HOSTS: [&str; 2] = ["ns1.soranet.example.", "ns2.soranet.example."];
const PLACEHOLDER_DS_KEY_TAG: u16 = 2371;
const PLACEHOLDER_DS_ALGO: u8 = 13;
const PLACEHOLDER_DS_DIGEST_TYPE: u8 = 2;
const PLACEHOLDER_DS_DIGEST_HEX: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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

    fn alias(hostname: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            hostname: hostname.into(),
            record_type: "ALIAS",
            values,
            ttl_secs: DNS_TTL_SECS,
        }
    }

    fn ns(hostname: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            hostname: hostname.into(),
            record_type: "NS",
            values,
            ttl_secs: DNS_TTL_SECS,
        }
    }

    fn ds(hostname: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            hostname: hostname.into(),
            record_type: "DS",
            values,
            ttl_secs: DNS_TTL_SECS,
        }
    }
}

/// Minimal authoritative DNS stub for public-zone CNAME/ALIAS checks in tests.
struct ReferenceAuthoritativeDns {
    cname_records: HashMap<String, Vec<String>>,
    alias_records: HashMap<String, Vec<String>>,
}

impl ReferenceAuthoritativeDns {
    fn new(records: &[DnsRecord]) -> Self {
        let mut cname_records = HashMap::new();
        let mut alias_records = HashMap::new();
        for record in records {
            match record.record_type {
                "CNAME" => {
                    cname_records.insert(normalize_host(&record.hostname), record.values.clone());
                }
                "ALIAS" => {
                    alias_records.insert(normalize_host(&record.hostname), record.values.clone());
                }
                _ => {}
            }
        }
        Self {
            cname_records,
            alias_records,
        }
    }

    fn resolve_alias_or_cname(&self, hostname: &str) -> Option<&[String]> {
        let key = normalize_host(hostname);
        self.alias_records
            .get(&key)
            .or_else(|| self.cname_records.get(&key))
            .map(Vec::as_slice)
    }
}

fn normalize_host(hostname: &str) -> String {
    hostname.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn ds_placeholder_value() -> String {
    format!(
        "{PLACEHOLDER_DS_KEY_TAG} {PLACEHOLDER_DS_ALGO} {PLACEHOLDER_DS_DIGEST_TYPE} {PLACEHOLDER_DS_DIGEST_HEX}"
    )
}

fn delegation_records_for_zone(zone: &str) -> Vec<DnsRecord> {
    vec![
        DnsRecord::ns(
            zone.to_string(),
            PLACEHOLDER_NS_HOSTS
                .iter()
                .copied()
                .map(str::to_string)
                .collect(),
        ),
        DnsRecord::ds(zone.to_string(), vec![ds_placeholder_value()]),
    ]
}

// Regular DNS deployments use ALIAS/ANAME for apex/TLDs and CNAME for subdomains.
fn public_dns_settings_for_domain(bindings: &GatewayHostBindings, zone: &str) -> Vec<DnsRecord> {
    let pretty = bindings.pretty_host().to_string();
    let normalized = bindings.normalized_name().to_string();
    let mut records = Vec::new();
    records.push(DnsRecord::alias(zone.to_string(), vec![pretty.clone()]));
    if normalized != zone {
        records.push(DnsRecord::cname(normalized, vec![pretty]));
    }
    records.extend(delegation_records_for_zone(zone));
    records
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

#[test]
fn public_dns_settings_include_delegation_placeholders() {
    let bindings = derive_gateway_hosts("sora-nexus.sora").expect("derive hosts");
    let zone = "sora";
    let records = public_dns_settings_for_domain(&bindings, zone);

    let alias_records: Vec<_> = records
        .iter()
        .filter(|record| record.record_type == "ALIAS")
        .collect();
    let cname_records: Vec<_> = records
        .iter()
        .filter(|record| record.record_type == "CNAME")
        .collect();
    let ns_records: Vec<_> = records
        .iter()
        .filter(|record| record.record_type == "NS")
        .collect();
    let ds_records: Vec<_> = records
        .iter()
        .filter(|record| record.record_type == "DS")
        .collect();

    assert_eq!(alias_records.len(), 1, "expected apex ALIAS record");
    assert_eq!(alias_records[0].hostname, zone);
    assert_eq!(
        alias_records[0].values,
        vec![bindings.pretty_host().to_string()]
    );
    assert_eq!(cname_records.len(), 1, "expected subdomain CNAME record");
    assert_eq!(cname_records[0].hostname, bindings.normalized_name());
    assert_eq!(
        cname_records[0].values,
        vec![bindings.pretty_host().to_string()]
    );
    assert_eq!(ns_records.len(), 1, "expected NS delegation record");
    assert_eq!(ns_records[0].hostname, zone);
    assert_eq!(
        ns_records[0].values,
        PLACEHOLDER_NS_HOSTS
            .iter()
            .copied()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(ds_records.len(), 1, "expected DS delegation record");
    assert_eq!(ds_records[0].hostname, zone);
    assert_eq!(ds_records[0].values, vec![ds_placeholder_value()]);
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
    let zone = "sora";
    let bindings = derive_gateway_hosts(fqdn)?;
    let dns_records = public_dns_settings_for_domain(&bindings, zone);
    let cname_count = dns_records
        .iter()
        .filter(|record| record.record_type == "CNAME")
        .count();
    let alias_count = dns_records
        .iter()
        .filter(|record| record.record_type == "ALIAS")
        .count();
    assert_eq!(cname_count, 1, "expected one CNAME record");
    assert_eq!(alias_count, 1, "expected one ALIAS record");
    assert!(
        dns_records
            .iter()
            .any(|record| record.record_type == "NS" || record.record_type == "DS"),
        "dns records should include NS/DS delegation placeholders"
    );
    let gateway = ReferenceAuthoritativeDns::new(&dns_records);
    let pretty_host = bindings.pretty_host().to_string();
    assert_eq!(
        gateway.resolve_alias_or_cname(fqdn),
        Some([pretty_host.clone()].as_slice()),
        "domain CNAME should point at the pretty host"
    );
    assert_eq!(
        gateway.resolve_alias_or_cname(zone),
        Some([pretty_host.clone()].as_slice()),
        "apex ALIAS should point at the pretty host"
    );
    assert!(
        gateway
            .resolve_alias_or_cname(bindings.canonical_host())
            .is_none(),
        "canonical host is managed under gw.sora.id, not the public zone"
    );

    network.shutdown().await;
    Ok(())
}
