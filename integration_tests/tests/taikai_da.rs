#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Roadmap SN13-D coverage for Taikai DA ingest validation.

use std::path::Path;

use blake3::hash as blake3_hash;
use eyre::Result;
use integration_tests::sandbox::start_network_async_or_skip;
use iroha_config::parameters::actual::DaReplicationPolicy;
use iroha_config_base::toml::Writer;
use iroha_crypto::Signature;
use iroha_data_model::{
    da::{
        ingest::DaIngestRequest,
        types::{
            BlobClass, BlobCodec, BlobDigest, Compression, ErasureProfile, ExtraMetadata,
            GovernanceTag, MetadataEntry, MetadataVisibility, RetentionPolicy,
        },
    },
    nexus::LaneId,
    sorafs::pin_registry::StorageClass,
    taikai::TaikaiAvailabilityClass,
};
use iroha_test_network::{Network, NetworkBuilder};
use iroha_test_samples::ALICE_KEYPAIR;
use norito::json::{self, Value};
use reqwest::{Client, Response, StatusCode};
use tempfile::tempdir;
use toml::Value as TomlValue;

const META_EVENT_ID: &str = "taikai.event_id";
const META_STREAM_ID: &str = "taikai.stream_id";
const META_RENDITION_ID: &str = "taikai.rendition_id";
const META_TRACK_KIND: &str = "taikai.track.kind";
const META_TRACK_CODEC: &str = "taikai.track.codec";
const META_TRACK_BITRATE: &str = "taikai.track.bitrate_kbps";
const META_TRACK_RESOLUTION: &str = "taikai.track.resolution";
const META_TRACK_AUDIO_LAYOUT: &str = "taikai.track.audio_layout";
const META_SEGMENT_SEQUENCE: &str = "taikai.segment.sequence";
const META_SEGMENT_START: &str = "taikai.segment.start_pts";
const META_SEGMENT_DURATION: &str = "taikai.segment.duration";
const META_WALLCLOCK_MS: &str = "taikai.wallclock_unix_ms";
const META_TAIKAI_INGEST_LATENCY_MS: &str = "taikai.instrumentation.ingest_latency_ms";
const META_TAIKAI_LIVE_EDGE_DRIFT_MS: &str = "taikai.instrumentation.live_edge_drift_ms";
const META_TAIKAI_INGEST_NODE_ID: &str = "taikai.instrumentation.ingest_node_id";
const META_SSM: &str = "taikai.ssm";

const TEST_RESOLUTION: &str = "1920x1080";
const TEST_PAYLOAD: &[u8] = b"taikai-da-segment-fixture";

fn value_for(metadata: &ExtraMetadata, key: &str) -> String {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == key)
        .unwrap_or_else(|| panic!("missing metadata entry `{key}`"));
    String::from_utf8(entry.value.clone()).expect("utf8 metadata value")
}

#[tokio::test]
async fn taikai_video_segments_require_resolution_metadata() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_missing_resolution").await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_taikai_metadata();
    metadata.retain(|entry| entry.key != META_TRACK_RESOLUTION);
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_TRACK_RESOLUTION),
        "expected missing-resolution error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_segments_require_signing_manifest_metadata() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_missing_ssm").await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let metadata = base_taikai_metadata();
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_SSM),
        "expected missing-SSM error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_audio_segments_require_audio_layout_metadata() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_missing_audio_layout").await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_audio_metadata();
    metadata.retain(|entry| entry.key != META_TRACK_AUDIO_LAYOUT);
    metadata.retain(|entry| entry.key != META_TRACK_RESOLUTION);
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_TRACK_AUDIO_LAYOUT),
        "expected missing-audio-layout error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_ingest_latency_metadata_requires_integer() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_latency_metadata").await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_taikai_metadata();
    metadata.push(metadata_utf8(META_TAIKAI_INGEST_LATENCY_MS, "not-a-number"));
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_TAIKAI_INGEST_LATENCY_MS),
        "expected ingest-latency parse error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_live_edge_drift_metadata_requires_integer() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_drift_metadata").await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_taikai_metadata();
    metadata.push(metadata_utf8(META_TAIKAI_LIVE_EDGE_DRIFT_MS, "invalid"));
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_TAIKAI_LIVE_EDGE_DRIFT_MS),
        "expected live-edge drift parse error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_ingest_node_id_metadata_requires_utf8() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_node_id_metadata").await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_taikai_metadata();
    metadata.push(metadata_bytes(META_TAIKAI_INGEST_NODE_ID, vec![0xFF]));
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains(META_TAIKAI_INGEST_NODE_ID),
        "expected ingest-node-id UTF-8 error, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn taikai_ssm_payload_must_decode_into_manifest() -> Result<()> {
    let manifest_dir = tempdir()?;
    let replay_dir = tempdir()?;
    let manifest_path = manifest_dir.path().to_owned();
    let replay_path = replay_dir.path().to_owned();
    let builder = NetworkBuilder::new().with_config_layer(move |layer| {
        configure_da_spool(layer, &manifest_path, &replay_path);
    });
    let Some(network) = start_network_async_or_skip(builder, "taikai_ssm_decode").await? else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let mut metadata = base_taikai_metadata();
    metadata.push(metadata_bytes(META_SSM, vec![0xDE, 0xAD, 0xBE, 0xEF]));
    let request = build_taikai_request(metadata);

    let response = post_ingest(&network, &request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await?;
    assert!(
        body.contains("failed to decode signing manifest"),
        "expected SSM decode failure, got body: {body}"
    );

    network.shutdown().await;
    Ok(())
}

#[test]
fn taikai_replication_policy_uses_availability_overrides() {
    struct Expectation {
        availability: TaikaiAvailabilityClass,
        hot_retention_secs: u64,
        cold_retention_secs: u64,
        replicas: u16,
        storage_class: StorageClass,
        governance_tag: &'static str,
    }

    let policy = DaReplicationPolicy::default();
    let caller_policy = RetentionPolicy {
        hot_retention_secs: 1,
        cold_retention_secs: 2,
        required_replicas: 1,
        storage_class: StorageClass::Cold,
        governance_tag: GovernanceTag::new("da.intent.test"),
    };
    let cases = [
        Expectation {
            availability: TaikaiAvailabilityClass::Hot,
            hot_retention_secs: 24 * 60 * 60,
            cold_retention_secs: 14 * 24 * 60 * 60,
            replicas: 5,
            storage_class: StorageClass::Hot,
            governance_tag: "da.taikai.live",
        },
        Expectation {
            availability: TaikaiAvailabilityClass::Warm,
            hot_retention_secs: 6 * 60 * 60,
            cold_retention_secs: 30 * 24 * 60 * 60,
            replicas: 4,
            storage_class: StorageClass::Warm,
            governance_tag: "da.taikai.warm",
        },
        Expectation {
            availability: TaikaiAvailabilityClass::Cold,
            hot_retention_secs: 60 * 60,
            cold_retention_secs: 180 * 24 * 60 * 60,
            replicas: 3,
            storage_class: StorageClass::Cold,
            governance_tag: "da.taikai.archive",
        },
    ];

    for expectation in cases {
        let (enforced, mismatch) = policy.enforce(
            BlobClass::TaikaiSegment,
            Some(expectation.availability),
            &caller_policy,
        );
        assert!(
            mismatch,
            "caller overrides must be replaced for {:?} availability",
            expectation.availability
        );
        assert_eq!(enforced.hot_retention_secs, expectation.hot_retention_secs);
        assert_eq!(
            enforced.cold_retention_secs,
            expectation.cold_retention_secs
        );
        assert_eq!(enforced.required_replicas, expectation.replicas);
        assert_eq!(enforced.storage_class, expectation.storage_class);
        assert_eq!(enforced.governance_tag.0, expectation.governance_tag);
    }
}

#[test]
fn taikai_ingest_tags_cover_replication_and_proofs() {
    let payload = TEST_PAYLOAD;
    let payload_digest = BlobDigest::from_hash(blake3_hash(payload));
    let retention = RetentionPolicy {
        hot_retention_secs: 7_200,
        cold_retention_secs: 86_400,
        required_replicas: 5,
        storage_class: StorageClass::Hot,
        governance_tag: GovernanceTag::new("da.taikai.ops"),
    };
    let metadata = ExtraMetadata {
        items: base_taikai_metadata(),
    };
    let tagged = iroha_torii::compute_taikai_ingest_tags(
        metadata,
        Some(TaikaiAvailabilityClass::Warm),
        &retention,
        payload_digest,
        payload.len() as u64,
    )
    .expect("tagging succeeds");

    assert_eq!(value_for(&tagged, "taikai.availability_class"), "warm");
    assert_eq!(
        value_for(&tagged, "da.proof.tier"),
        "hot",
        "proof tier follows enforced storage class"
    );
    assert_eq!(value_for(&tagged, "taikai.replication.replicas"), "5");
    assert_eq!(
        value_for(&tagged, "taikai.replication.storage_class"),
        "hot"
    );
    assert_eq!(
        value_for(&tagged, "taikai.replication.hot_retention_secs"),
        "7200"
    );
    assert_eq!(
        value_for(&tagged, "taikai.replication.cold_retention_secs"),
        "86400"
    );
    assert_eq!(value_for(&tagged, "da.proof.pdp.sample_window"), "32");
    assert_eq!(value_for(&tagged, "da.proof.potr.sample_window"), "32");

    let cache_hint_entry = tagged
        .items
        .iter()
        .find(|entry| entry.key == "taikai.cache_hint")
        .expect("cache hint present");
    let cache_hint: Value = json::from_slice(&cache_hint_entry.value).expect("cache hint is JSON");
    let cache_hint = cache_hint.as_object().expect("cache hint object");
    assert_eq!(
        cache_hint
            .get("payload_blake3_hex")
            .and_then(Value::as_str)
            .expect("payload digest"),
        hex::encode(payload_digest.as_ref())
    );
}

async fn post_ingest(network: &Network, request: &DaIngestRequest) -> Result<Response> {
    let http = Client::new();
    let json_value = json::to_value(request)?;
    let request_body = json::to_string(&json_value)?;
    let url = network
        .client()
        .torii_url
        .join("/v2/da/ingest")
        .expect("compose DA ingest URL");
    let response = http
        .post(url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?;
    Ok(response)
}

fn build_taikai_request(metadata_entries: Vec<MetadataEntry>) -> DaIngestRequest {
    let payload = TEST_PAYLOAD.to_vec();
    let digest = BlobDigest::from_hash(blake3_hash(&payload));
    let submitter = ALICE_KEYPAIR.public_key().clone();
    let signature = Signature::new(ALICE_KEYPAIR.private_key(), &payload);

    DaIngestRequest {
        client_blob_id: digest,
        lane_id: LaneId::SINGLE,
        epoch: 7,
        sequence: 3,
        blob_class: BlobClass::TaikaiSegment,
        codec: BlobCodec("video/cmaf".into()),
        erasure_profile: ErasureProfile::default(),
        retention_policy: RetentionPolicy::default(),
        chunk_size: 1_024,
        total_size: payload.len() as u64,
        compression: Compression::Identity,
        metadata: ExtraMetadata {
            items: metadata_entries,
        },
        norito_manifest: None,
        payload,
        submitter,
        signature,
    }
}

fn base_taikai_metadata() -> Vec<MetadataEntry> {
    vec![
        metadata_utf8(META_EVENT_ID, "demo-event"),
        metadata_utf8(META_STREAM_ID, "demo-stream"),
        metadata_utf8(META_RENDITION_ID, "1080p-main"),
        metadata_utf8(META_TRACK_KIND, "video"),
        metadata_utf8(META_TRACK_CODEC, "avc-high"),
        metadata_utf8(META_TRACK_BITRATE, "4000"),
        metadata_utf8(META_TRACK_RESOLUTION, TEST_RESOLUTION),
        metadata_utf8(META_SEGMENT_SEQUENCE, "91"),
        metadata_utf8(META_SEGMENT_START, "180000000"),
        metadata_utf8(META_SEGMENT_DURATION, "2000000"),
        metadata_utf8(META_WALLCLOCK_MS, "1700000000123"),
    ]
}

fn base_audio_metadata() -> Vec<MetadataEntry> {
    let mut items = base_taikai_metadata();
    for entry in &mut items {
        if entry.key == META_TRACK_KIND {
            entry.value = b"audio".to_vec();
        } else if entry.key == META_TRACK_CODEC {
            entry.value = b"aac-lc".to_vec();
        }
    }
    items.retain(|entry| entry.key != META_TRACK_RESOLUTION);
    items.push(metadata_utf8(META_TRACK_AUDIO_LAYOUT, "stereo"));
    items
}

fn metadata_utf8(key: &str, value: impl Into<String>) -> MetadataEntry {
    MetadataEntry::new(key, value.into().into_bytes(), MetadataVisibility::Public)
}

fn metadata_bytes(key: &str, value: Vec<u8>) -> MetadataEntry {
    MetadataEntry::new(key, value, MetadataVisibility::Public)
}

fn configure_da_spool<'a>(layer: &'a mut Writer<'a>, manifest_dir: &Path, replay_dir: &Path) {
    layer
        .write(["nexus", "enabled"], true)
        .write(["sumeragi", "consensus_mode"], "npos")
        .write(
            ["torii", "da_ingest", "manifest_store_dir"],
            manifest_dir.display().to_string(),
        )
        .write(
            ["torii", "da_ingest", "replay_cache_store_dir"],
            replay_dir.display().to_string(),
        )
        .write(
            [
                "torii",
                "da_ingest",
                "replication_policy",
                "default_retention",
            ],
            TomlValue::Table(retention_table(
                6 * 60 * 60,
                30 * 24 * 60 * 60,
                3,
                "warm",
                "da.test.default",
            )),
        )
        .write(
            ["torii", "da_ingest", "replication_policy", "overrides"],
            default_replication_overrides(),
        )
        .write(
            [
                "torii",
                "da_ingest",
                "replication_policy",
                "taikai_availability",
            ],
            default_taikai_availability_overrides(),
        )
        .write(
            ["torii", "da_ingest", "telemetry_cluster_label"],
            "integration-taikai",
        );
}

fn retention_table(
    hot_retention_secs: i64,
    cold_retention_secs: i64,
    required_replicas: i64,
    storage_class: &str,
    governance_tag: &str,
) -> toml::value::Table {
    let mut retention = toml::value::Table::new();
    retention.insert(
        "hot_retention_secs".to_string(),
        TomlValue::Integer(hot_retention_secs),
    );
    retention.insert(
        "cold_retention_secs".to_string(),
        TomlValue::Integer(cold_retention_secs),
    );
    retention.insert(
        "required_replicas".to_string(),
        TomlValue::Integer(required_replicas),
    );
    retention.insert(
        "storage_class".to_string(),
        TomlValue::String(storage_class.to_string()),
    );
    retention.insert(
        "governance_tag".to_string(),
        TomlValue::String(governance_tag.to_string()),
    );
    retention
}

fn default_replication_overrides() -> TomlValue {
    fn entry(class: &str, retention: toml::value::Table) -> TomlValue {
        let mut table = toml::value::Table::new();
        table.insert("class".to_string(), TomlValue::String(class.to_string()));
        table.insert("retention".to_string(), TomlValue::Table(retention));
        TomlValue::Table(table)
    }

    TomlValue::Array(vec![
        entry(
            "taikai_segment",
            retention_table(24 * 60 * 60, 14 * 24 * 60 * 60, 5, "hot", "da.taikai.live"),
        ),
        entry(
            "nexus_lane_sidecar",
            retention_table(6 * 60 * 60, 7 * 24 * 60 * 60, 4, "warm", "da.sidecar"),
        ),
        entry(
            "governance_artifact",
            retention_table(12 * 60 * 60, 180 * 24 * 60 * 60, 3, "cold", "da.governance"),
        ),
    ])
}

fn default_taikai_availability_overrides() -> TomlValue {
    fn entry(
        availability_class: &str,
        hot_retention_secs: i64,
        cold_retention_secs: i64,
        required_replicas: i64,
        storage_class: &str,
        governance_tag: &str,
    ) -> TomlValue {
        let retention = retention_table(
            hot_retention_secs,
            cold_retention_secs,
            required_replicas,
            storage_class,
            governance_tag,
        );
        let mut table = toml::value::Table::new();
        table.insert(
            "availability_class".to_string(),
            TomlValue::String(availability_class.to_string()),
        );
        table.insert("retention".to_string(), TomlValue::Table(retention));

        TomlValue::Table(table)
    }

    TomlValue::Array(vec![
        entry(
            "hot",
            24 * 60 * 60,
            14 * 24 * 60 * 60,
            5,
            "hot",
            "da.taikai.live",
        ),
        entry(
            "warm",
            6 * 60 * 60,
            30 * 24 * 60 * 60,
            4,
            "warm",
            "da.taikai.warm",
        ),
        entry(
            "cold",
            60 * 60,
            180 * 24 * 60 * 60,
            3,
            "cold",
            "da.taikai.archive",
        ),
    ])
}
