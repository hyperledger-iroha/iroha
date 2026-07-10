use std::{collections::HashMap, fs, io::Write, path::PathBuf, sync::OnceLock};

use axum::{
    Router as AxumRouter, body,
    http::{Request, StatusCode},
};
use base64::Engine as _;
use norito::{
    decode_from_bytes,
    json::{self, Value},
};
use sorafs_car::CarBuildPlan;
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{ManifestV1, chunker_registry};
use sorafs_node::{
    NodeHandle,
    config::StorageConfig,
    gateway::{self, GatewayDataset, GatewayState, TokenPolicy},
};
use tempfile::{NamedTempFile, TempDir};
use tower::ServiceExt;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/sorafs_gateway/1.0.0")
}

fn chunk_profile_for_manifest(manifest: &ManifestV1) -> ChunkProfile {
    if let Some(descriptor) = chunker_registry::lookup(manifest.chunking.profile_id) {
        descriptor.profile
    } else {
        let min_size = usize::try_from(manifest.chunking.min_size).expect("min_size fits usize");
        let target_size =
            usize::try_from(manifest.chunking.target_size).expect("target_size fits usize");
        let max_size = usize::try_from(manifest.chunking.max_size).expect("max_size fits usize");
        assert!(
            min_size <= target_size && target_size <= max_size,
            "manifest chunk sizes must satisfy min <= target <= max"
        );
        ChunkProfile {
            min_size,
            target_size,
            max_size,
            break_mask: u64::from(manifest.chunking.break_mask),
        }
    }
}

fn signing_key_file() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp signing key");
    file.write_all(&[0x11u8; 32]).expect("write signing key");
    file
}

fn gateway_dataset_from_fixtures() -> GatewayDataset {
    let manifest_bytes =
        fs::read(fixture_dir().join("manifest_v1.to")).expect("read manifest fixture");
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
    let payload = fs::read(fixture_dir().join("payload.bin")).expect("read payload fixture");
    let profile = chunk_profile_for_manifest(&manifest);
    let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("build plan");
    let temp_dir = TempDir::new().expect("temp storage");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let signing_key = signing_key_file();
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(root.join("storage"))
        .stream_token_signing_key_path(Some(signing_key.path().to_path_buf()))
        .build();
    let node = NodeHandle::new(config);
    let mut reader = &payload[..];
    node.ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest manifest");
    let manifest_digest = manifest.digest().expect("manifest digest");
    let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());
    let provider_id = [0xAB; 32];
    GatewayDataset::load_from_storage_with_provider(&node, &manifest_digest_hex, provider_id)
        .expect("load storage-backed dataset")
}

#[test]
fn storage_backed_dataset_matches_manifest_fixture() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_bytes =
        fs::read(fixture_dir().join("manifest_v1.to")).expect("read manifest fixture");
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes).expect("decode manifest fixture");
    let manifest_digest = manifest.digest().expect("manifest digest");
    assert_eq!(
        dataset.manifest_id_hex(),
        hex::encode(manifest_digest.as_bytes())
    );
    assert_eq!(dataset.manifest().root_cid, manifest.root_cid);
}

#[test]
fn gateway_startup_requires_exact_canonical_operator_approval() {
    let dataset = gateway_dataset_from_fixtures();
    let approved = manifest_envelope_header(&dataset);
    let tampered = manifest_envelope_with_manifest_digest(
        &dataset,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );
    assert!(GatewayState::new(dataset, tampered).is_err());

    let dataset = gateway_dataset_from_fixtures();
    assert!(GatewayState::new(dataset, format!(" {approved}")).is_err());

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&approved)
        .expect("decode approval");
    let mut value: Value = json::from_slice(&decoded).expect("approval JSON");
    value
        .as_object_mut()
        .expect("approval object")
        .insert("unknown".into(), Value::from(true));
    let unknown_field_approval = base64::engine::general_purpose::STANDARD
        .encode(json::to_vec(&value).expect("approval JSON"));
    let dataset = gateway_dataset_from_fixtures();
    assert!(GatewayState::new(dataset, unknown_field_approval).is_err());
}

#[derive(Clone)]
struct CapabilityScenario {
    status: u16,
    error: String,
    reason: String,
    details: Option<Value>,
}

fn capability_refusal_scenarios() -> &'static HashMap<String, CapabilityScenario> {
    static CACHE: OnceLock<HashMap<String, CapabilityScenario>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/sorafs_gateway/capability_refusal/scenarios.json");
        let bytes = fs::read(&path).expect("read capability refusal scenarios");
        let value: Value =
            norito::json::from_slice(&bytes).expect("parse capability refusal scenarios");
        let array = value
            .as_array()
            .expect("capability refusal scenarios must be an array");
        let mut map = HashMap::new();
        for entry in array {
            let obj = entry
                .as_object()
                .expect("capability refusal scenario entries must be objects");
            let id = obj
                .get("id")
                .and_then(Value::as_str)
                .expect("capability refusal scenario missing id");
            let status = obj
                .get("status")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .expect("capability refusal scenario status must fit u16");
            let error = obj
                .get("error")
                .and_then(Value::as_str)
                .expect("capability refusal scenario missing error")
                .to_string();
            let reason = obj
                .get("reason")
                .and_then(Value::as_str)
                .expect("capability refusal scenario missing reason")
                .to_string();
            let details = obj.get("details").cloned();
            map.insert(
                id.to_string(),
                CapabilityScenario {
                    status,
                    error,
                    reason,
                    details,
                },
            );
        }
        map
    })
}

fn capability_scenario(id: &str) -> &'static CapabilityScenario {
    capability_refusal_scenarios()
        .get(id)
        .unwrap_or_else(|| panic!("capability scenario {id} not found"))
}

struct IssuedToken {
    token: String,
}

fn manifest_envelope_header(dataset: &GatewayDataset) -> String {
    let manifest_hex = dataset.manifest_id_hex().to_string();
    let provider_hex = dataset.provider_id_hex();
    let chunker = dataset.chunker_alias().to_string();
    let mut gar = json::Map::new();
    gar.insert("manifest_id_hex".into(), Value::from(manifest_hex.clone()));
    gar.insert("chunking_profile".into(), Value::from(chunker.clone()));
    gar.insert(
        "host_patterns".into(),
        Value::Array(vec![Value::from("gateway.test.invalid")]),
    );

    let mut admission = json::Map::new();
    admission.insert(
        "manifest_digest_hex".into(),
        Value::from(manifest_hex.clone()),
    );
    admission.insert("provider_id_hex".into(), Value::from(provider_hex.clone()));
    admission.insert("signature".into(), Value::from("test-signature"));
    admission.insert(
        "profile_version".into(),
        Value::from(dataset.profile_version().to_string()),
    );

    let mut root = json::Map::new();
    root.insert("manifest_digest_hex".into(), Value::from(manifest_hex));
    root.insert("chunking_profile".into(), Value::from(chunker));
    root.insert("provider_id_hex".into(), Value::from(provider_hex));
    root.insert("gar".into(), Value::Object(gar));
    root.insert("admission".into(), Value::Object(admission));

    let payload = Value::Object(root);
    let bytes = json::to_vec(&payload).expect("serialize manifest envelope");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn manifest_envelope_with_manifest_digest(
    dataset: &GatewayDataset,
    manifest_digest_hex: &str,
) -> String {
    let envelope_bytes = base64::engine::general_purpose::STANDARD
        .decode(manifest_envelope_header(dataset))
        .expect("decode manifest envelope");
    let mut envelope_value: Value =
        norito::json::from_slice(&envelope_bytes).expect("parse manifest envelope");
    if let Some(map) = envelope_value.as_object_mut() {
        map.insert(
            "manifest_digest_hex".into(),
            Value::from(manifest_digest_hex.to_string()),
        );
    }
    base64::engine::general_purpose::STANDARD
        .encode(norito::json::to_vec(&envelope_value).expect("encode manifest envelope"))
}

async fn issue_stream_token(
    app: &AxumRouter,
    manifest_envelope: &str,
) -> Result<IssuedToken, String> {
    let mut map = json::Map::new();
    map.insert(
        "manifest_envelope".into(),
        Value::from(manifest_envelope.to_string()),
    );
    map.insert(
        "capabilities".into(),
        Value::Array(vec![Value::from("sorafs.chunk-range.block")]),
    );
    let body_bytes = json::to_vec(&Value::Object(map)).map_err(|err| err.to_string())?;
    let request = Request::builder()
        .method("POST")
        .uri("/token")
        .header("content-type", "application/json")
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::from(body_bytes))
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .map_err(|err| err.to_string())?;
    if response.status() != StatusCode::OK {
        return Err(format!(
            "token issuance failed with status {}",
            response.status()
        ));
    }
    response
        .headers()
        .get("x-sorafs-stream-token")
        .and_then(|value| value.to_str().ok())
        .map(|value| IssuedToken {
            token: value.to_string(),
        })
        .ok_or_else(|| "token response missing X-SoraFS-Stream-Token header".to_string())
}

#[tokio::test]
async fn token_response_contains_real_signed_envelope_and_public_key() {
    let dataset = gateway_dataset_from_fixtures();
    let approved = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, approved.clone()).expect("gateway state");
    let app = gateway::router(state);

    let request_body = Value::Object(json::Map::from_iter([
        ("manifest_envelope".into(), Value::from(approved)),
        (
            "capabilities".into(),
            Value::Array(vec![Value::from("sorafs.chunk-range.block")]),
        ),
    ]));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/json")
                .header("X-SoraFS-Client", "client-alpha")
                .body(axum::body::Body::from(
                    json::to_vec(&request_body).expect("token body"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let header_token = response
        .headers()
        .get("x-sorafs-stream-token")
        .and_then(|value| value.to_str().ok())
        .expect("signed token header")
        .to_owned();
    let response_bytes = body::to_bytes(response.into_body(), 32 * 1024)
        .await
        .expect("response body");
    let response_value: Value = json::from_slice(&response_bytes).expect("response JSON");
    let response_map = response_value.as_object().expect("response object");
    assert_eq!(
        response_map.get("token").and_then(Value::as_str),
        Some(header_token.as_str())
    );
    let signature = response_map
        .get("signature")
        .and_then(Value::as_str)
        .expect("signature");
    let public_key = response_map
        .get("public_key")
        .and_then(Value::as_str)
        .expect("public key");
    assert_eq!(signature.len(), 128);
    assert_ne!(signature, "00".repeat(64));
    assert_eq!(public_key.len(), 64);
    assert_ne!(public_key, "00".repeat(32));

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(header_token)
        .expect("decode signed envelope");
    let envelope: Value = json::from_slice(&decoded).expect("signed envelope JSON");
    assert_eq!(
        envelope["signature_hex"].as_str(),
        Some(signature),
        "response signature must be the actual envelope signature"
    );
    assert_eq!(envelope["public_key_hex"].as_str(), Some(public_key));
}

#[tokio::test]
async fn token_endpoint_rejects_unknown_fields_invalid_clients_and_oversize_bodies() {
    let dataset = gateway_dataset_from_fixtures();
    let approved = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, approved.clone()).expect("gateway state");
    let app = gateway::router(state);

    let mut request_map = json::Map::from_iter([
        ("manifest_envelope".into(), Value::from(approved.clone())),
        (
            "capabilities".into(),
            Value::Array(vec![Value::from("sorafs.chunk-range.block")]),
        ),
    ]);
    request_map.insert("unknown".into(), Value::from(true));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/json")
                .header("X-SoraFS-Client", "client-alpha")
                .body(axum::body::Body::from(
                    json::to_vec(&Value::Object(request_map)).expect("body"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let valid_body = Value::Object(json::Map::from_iter([
        ("manifest_envelope".into(), Value::from(approved)),
        (
            "capabilities".into(),
            Value::Array(vec![Value::from("sorafs.chunk-range.block")]),
        ),
    ]));
    for client in ["Client-Uppercase", "client space", "client/control"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/token")
                    .header("content-type", "application/json")
                    .header("X-SoraFS-Client", client)
                    .body(axum::body::Body::from(
                        json::to_vec(&valid_body).expect("body"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "client={client}"
        );
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/token")
                .header("content-type", "application/json")
                .header("X-SoraFS-Client", "client-alpha")
                .body(axum::body::Body::from(vec![b'x'; 24 * 1024 + 1]))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn car_endpoint_serves_canonical_fixture() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let proof_digest = dataset.proof_digest_hex().to_string();
    let por_root = dataset.por_root_hex().to_string();
    let manifest_version = dataset.manifest().version;
    assert_eq!(manifest_version, 1, "fixture manifest version should be v1");
    let expected_car = std::fs::read(fixture_dir().join("gateway.car")).expect("read car fixture");
    let expected_len = expected_car.len();

    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);

    let nonce = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header("X-SoraFS-Nonce", nonce)
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/vnd.ipld.car")
    );
    assert_eq!(
        headers
            .get("content-length")
            .and_then(|value| value.to_str().ok()),
        Some(expected_len.to_string().as_str())
    );
    assert_eq!(
        headers
            .get("x-sorafs-chunker")
            .and_then(|value| value.to_str().ok()),
        Some(chunker_alias.as_str())
    );
    assert_eq!(
        headers
            .get("x-sorafs-proof-digest")
            .and_then(|value| value.to_str().ok()),
        Some(proof_digest.as_str())
    );
    assert_eq!(
        headers
            .get("x-sorafs-por-root")
            .and_then(|value| value.to_str().ok()),
        Some(por_root.as_str())
    );
    assert_eq!(
        headers
            .get("x-sorafs-nonce")
            .and_then(|value| value.to_str().ok()),
        Some(nonce)
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    assert_eq!(body_bytes.len(), expected_car.len());
    assert_eq!(body_bytes.as_ref(), expected_car.as_slice());
}

#[tokio::test]
async fn missing_manifest_envelope_is_rejected() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn head_manifest_envelope_is_required() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("HEAD")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        )
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn invalid_manifest_envelope_is_rejected() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let tampered = manifest_envelope_with_manifest_digest(
        &dataset,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );

    let approved = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, approved).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        )
        .header("X-SoraFS-Manifest-Envelope", tampered)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal body must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some("admission_mismatch")
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some("manifest envelope does not exactly match the operator-approved startup envelope")
    );
    assert!(obj.get("details").is_none());
}

#[tokio::test]
async fn car_range_serves_partial_chunk() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);

    let content_length = dataset.manifest().content_length;
    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk")
        .clone();
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");
    let stream_token = token_response.token;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Stream-Token", &stream_token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    let (parts, body) = response.into_parts();
    assert_eq!(parts.status, StatusCode::PARTIAL_CONTENT);
    let headers = parts.headers;
    let expected_content_range = format!("bytes {range_start}-{range_end}/{content_length}");
    assert_eq!(
        headers
            .get("content-range")
            .and_then(|value| value.to_str().ok()),
        Some(expected_content_range.as_str())
    );
    let expected_chunk_range = format!("start={range_start};end={range_end};chunks=1");
    assert_eq!(
        headers
            .get("x-sora-chunk-range")
            .and_then(|value| value.to_str().ok()),
        Some(expected_chunk_range.as_str())
    );
    assert_eq!(
        headers
            .get("accept-ranges")
            .and_then(|value| value.to_str().ok()),
        Some("bytes")
    );

    let body_bytes = body::to_bytes(body, usize::MAX)
        .await
        .expect("collect body");
    assert!(!body_bytes.is_empty(), "range body should not be empty");
    assert!(
        body_bytes.len() < content_length as usize,
        "range body must be smaller than full manifest"
    );
    let reported_length = headers
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .expect("content-length header present");
    assert_eq!(reported_length, body_bytes.len());
    assert!(
        body_bytes.len() >= usize::try_from(chunk.length).expect("chunk length fits in usize"),
        "range body should include chunk payload"
    );
}

#[tokio::test]
async fn range_request_rejects_token_tampering_and_wrong_client_binding() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_owned();
    let profile_version = dataset.profile_version().to_owned();
    let chunker_alias = dataset.chunker_alias().to_owned();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let chunk = dataset.plan().chunks.first().expect("chunk").clone();
    let range_end = chunk.offset + u64::from(chunk.length) - 1;
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue token")
        .token;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&token)
        .expect("decode token");
    let mut token_value: Value = json::from_slice(&decoded).expect("parse token");
    token_value
        .as_object_mut()
        .and_then(|map| map.get_mut("payload"))
        .and_then(Value::as_object_mut)
        .expect("token payload")
        .insert("client_id".into(), Value::from("client-evil"));
    let tampered = base64::engine::general_purpose::STANDARD
        .encode(json::to_vec(&token_value).expect("encode tampered token"));

    let request = |stream_token: &str, client_id: &str, nonce: &str| {
        Request::builder()
            .method("GET")
            .uri(format!("/car/{manifest_id}"))
            .header("X-SoraFS-Version", &profile_version)
            .header("X-SoraFS-Nonce", nonce)
            .header("X-SoraFS-Manifest-Envelope", &manifest_envelope)
            .header("X-SoraFS-Chunker", &chunker_alias)
            .header("Accept", "application/vnd.ipld.car; dag-scope=block")
            .header("Range", format!("bytes={}-{}", chunk.offset, range_end))
            .header("X-SoraFS-Stream-Token", stream_token)
            .header("X-SoraFS-Client", client_id)
            .body(axum::body::Body::empty())
            .expect("request")
    };

    let tampered_response = app
        .clone()
        .oneshot(request(
            &tampered,
            "client-evil",
            "8888888888888888888888888888888888888888888888888888888888888888",
        ))
        .await
        .expect("tampered response");
    assert_eq!(tampered_response.status(), StatusCode::PRECONDITION_FAILED);

    let wrong_client_response = app
        .oneshot(request(
            &token,
            "client-evil",
            "9999999999999999999999999999999999999999999999999999999999999999",
        ))
        .await
        .expect("wrong-client response");
    assert_eq!(
        wrong_client_response.status(),
        StatusCode::PRECONDITION_FAILED
    );
    let response_bytes = body::to_bytes(wrong_client_response.into_body(), 8 * 1024)
        .await
        .expect("error body");
    let response_value: Value = json::from_slice(&response_bytes).expect("error JSON");
    assert_eq!(response_value["error"].as_str(), Some("client_mismatch"));
}

#[tokio::test]
async fn stream_token_concurrency_is_enforced() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk")
        .clone();
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let policy = TokenPolicy {
        max_streams: 1,
        ..TokenPolicy::default()
    };
    let state = GatewayState::with_token_policy(dataset, manifest_envelope.clone(), policy)
        .expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");
    let stream_token = token_response.token;

    let build_request = |nonce: &str| {
        Request::builder()
            .method("GET")
            .uri(format!("/car/{manifest_id}"))
            .header("X-SoraFS-Version", &profile_version)
            .header("X-SoraFS-Nonce", nonce)
            .header("X-SoraFS-Manifest-Envelope", manifest_envelope.as_str())
            .header("X-SoraFS-Chunker", &chunker_alias)
            .header("Accept", "application/vnd.ipld.car; dag-scope=block")
            .header("Range", format!("bytes={range_start}-{range_end}"))
            .header("X-SoraFS-Stream-Token", &stream_token)
            .header("X-SoraFS-Client", "test-suite")
            .body(axum::body::Body::empty())
            .expect("request build")
    };

    let first_request =
        build_request("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let first_response = app
        .clone()
        .oneshot(first_request)
        .await
        .expect("first range request should succeed");
    assert_eq!(first_response.status(), StatusCode::PARTIAL_CONTENT);

    let second_request =
        build_request("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    let second_response = app
        .clone()
        .oneshot(second_request)
        .await
        .expect("second range request should complete");
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);

    drop(first_response);

    let third_request =
        build_request("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd");
    let third_response = app
        .clone()
        .oneshot(third_request)
        .await
        .expect("third range request should succeed after release");
    assert_eq!(third_response.status(), StatusCode::PARTIAL_CONTENT);
}

#[tokio::test]
async fn car_range_rejects_misaligned_offsets() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);

    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk");
    let range_start = chunk.offset + 1;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");
    let stream_token = token_response.token;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Stream-Token", &stream_token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

#[tokio::test]
async fn proof_endpoint_emits_json_payload() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let expected_digest = dataset.proof_digest_hex().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    dataset
        .verify_proof_for_testing()
        .expect("proof should validate");
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/proof/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(
        headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(
        headers
            .get("x-sorafs-proof-digest")
            .and_then(|value| value.to_str().ok()),
        Some(expected_digest.as_str())
    );
}

#[tokio::test]
async fn proof_request_requires_manifest_envelope() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/proof/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        )
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn token_issuance_respects_quota_limit() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let policy = TokenPolicy {
        requests_per_minute: Some(1),
        ..TokenPolicy::default()
    };
    let state = GatewayState::with_token_policy(dataset, manifest_envelope.clone(), policy)
        .expect("gateway state");
    let app = gateway::router(state);

    let mut body_map = json::Map::new();
    body_map.insert(
        "manifest_envelope".into(),
        Value::from(manifest_envelope.clone()),
    );
    body_map.insert(
        "capabilities".into(),
        Value::Array(vec![Value::from("sorafs.chunk-range.block")]),
    );
    let body_bytes = json::to_vec(&Value::Object(body_map)).expect("serialize request body");

    let make_request = || {
        Request::builder()
            .method("POST")
            .uri("/token")
            .header("content-type", "application/json")
            .header("X-SoraFS-Client", "quota-test")
            .body(axum::body::Body::from(body_bytes.clone()))
            .expect("request build")
    };

    let first_response = app
        .clone()
        .oneshot(make_request())
        .await
        .expect("first token issuance should succeed");
    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(
        first_response
            .headers()
            .get("x-sorafs-client-quota-remaining")
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );

    let second_response = app
        .clone()
        .oneshot(make_request())
        .await
        .expect("second token issuance should produce quota error");
    assert_eq!(second_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(
        second_response.headers().get("retry-after").is_some(),
        "quota error should include Retry-After header"
    );
    assert_eq!(
        second_response
            .headers()
            .get("x-sorafs-client")
            .and_then(|value| value.to_str().ok()),
        Some("quota-test")
    );
}

#[tokio::test]
async fn range_request_rejects_rate_limit_overflow() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk")
        .clone();
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");
    let policy = TokenPolicy {
        rate_limit_bytes: u64::from(chunk.length.saturating_sub(1)),
        ..TokenPolicy::default()
    };
    let state = GatewayState::with_token_policy(dataset, manifest_envelope.clone(), policy)
        .expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn range_request_without_client_header_is_rejected() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk")
        .clone();
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "2222222222222222222222222222222222222222222222222222222222222222",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
}

#[tokio::test]
async fn range_request_without_stream_token_is_rejected() {
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk")
        .clone();
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "3333333333333333333333333333333333333333333333333333333333333333",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    assert_eq!(
        value.get("error").and_then(Value::as_str),
        Some("missing_header")
    );
}

#[tokio::test]
async fn capability_refusal_unsupported_chunker_matches_fixture() {
    let scenario = capability_scenario("C1").clone();
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");
    let invalid_chunker = "sorafs.sf9@9.9.9";

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "3333333333333333333333333333333333333333333333333333333333333333",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", invalid_chunker)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", "bytes=0-0")
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let value: Value =
        norito::json::from_slice(&body_bytes).expect("parse capability refusal body");
    let obj = value
        .as_object()
        .expect("capability refusal body must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    if let (Some(expected_details), Some(Value::Object(actual_details))) =
        (&scenario.details, obj.get("details"))
    {
        if let Some(expected_profile) = expected_details
            .as_object()
            .and_then(|map| map.get("profile"))
            .and_then(Value::as_str)
        {
            assert_eq!(
                actual_details.get("profile").and_then(Value::as_str),
                Some(expected_profile)
            );
        }
        assert_eq!(
            actual_details
                .get("requested_profile")
                .and_then(Value::as_str),
            Some(invalid_chunker)
        );
    }
}

#[tokio::test]
async fn range_request_alias_mismatch_matches_fixture() {
    let scenario = capability_scenario("C2").clone();
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);

    let chunk = dataset
        .plan()
        .chunks
        .first()
        .expect("fixture must contain at least one chunk");
    let range_start = chunk.offset;
    let range_end = chunk
        .offset
        .checked_add(u64::from(chunk.length))
        .and_then(|value| value.checked_sub(1))
        .expect("valid chunk range");

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");
    let missing_alias = "sorafs.missing@1.0.0";

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "6666666666666666666666666666666666666666666666666666666666666666",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Range", format!("bytes={range_start}-{range_end}"))
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .header("X-SoraFS-Client", "test-suite")
        .header("Sora-Name", missing_alias)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal body must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    let details = obj
        .get("details")
        .and_then(Value::as_object)
        .expect("refusal must include details");
    assert_eq!(
        details.get("alias").and_then(Value::as_str),
        Some(missing_alias)
    );
}

#[tokio::test]
async fn token_issuance_rejects_unsupported_capability() {
    let scenario = capability_scenario("C4").clone();
    let dataset = gateway_dataset_from_fixtures();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);

    let mut body_map = json::Map::new();
    body_map.insert(
        "manifest_envelope".into(),
        Value::from(manifest_envelope.clone()),
    );
    body_map.insert(
        "capabilities".into(),
        Value::Array(vec![Value::from("vendor_reserved#0xff")]),
    );
    let body_bytes = json::to_vec(&Value::Object(body_map)).expect("serialize request body");

    let request = Request::builder()
        .method("POST")
        .uri("/token")
        .header("content-type", "application/json")
        .header("X-SoraFS-Client", "capability-test")
        .body(axum::body::Body::from(body_bytes))
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    let details = obj
        .get("details")
        .and_then(Value::as_object)
        .expect("refusal must include details");
    assert_eq!(
        details.get("capability").and_then(Value::as_str),
        Some("vendor_reserved#0xff")
    );
}

#[tokio::test]
async fn range_request_missing_dag_scope_header_matches_fixture() {
    let scenario = capability_scenario("C5").clone();
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "4444444444444444444444444444444444444444444444444444444444444444",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Range", "bytes=0-0")
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    let details = obj
        .get("details")
        .and_then(Value::as_object)
        .expect("refusal must include details");
    assert_eq!(
        details.get("header").and_then(Value::as_str),
        Some("Sora-Dag-Scope")
    );
}

#[tokio::test]
async fn range_request_rejects_gzip_compression_matches_fixture() {
    let scenario = capability_scenario("C6").clone();
    let dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let chunker_alias = dataset.chunker_alias().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);
    let token_response = issue_stream_token(&app, &manifest_envelope)
        .await
        .expect("issue stream token");

    let request = Request::builder()
        .method("GET")
        .uri(format!("/car/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "5555555555555555555555555555555555555555555555555555555555555555",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .header("X-SoraFS-Chunker", &chunker_alias)
        .header("Accept", "application/vnd.ipld.car; dag-scope=block")
        .header("Accept-Encoding", "gzip")
        .header("Range", "bytes=0-0")
        .header("X-SoraFS-Stream-Token", &token_response.token)
        .header("X-SoraFS-Client", "test-suite")
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    let details = obj
        .get("details")
        .and_then(Value::as_object)
        .expect("refusal must include details");
    assert_eq!(
        details.get("encoding").and_then(Value::as_str),
        Some("gzip")
    );
}

#[tokio::test]
async fn proof_endpoint_rejects_tampered_chunk_digest() {
    let scenario = capability_scenario("C7").clone();
    let mut dataset = gateway_dataset_from_fixtures();
    let manifest_id = dataset.manifest_id_hex().to_string();
    let profile_version = dataset.profile_version().to_string();
    let manifest_envelope = manifest_envelope_header(&dataset);
    let tampered_digest = [0xFF; 32];
    let proof = dataset.proof_mut_for_testing();
    assert!(
        !proof.samples.is_empty(),
        "fixture proof must contain at least one sample"
    );
    proof.samples[0].chunk_digest = tampered_digest;

    let state = GatewayState::new(dataset, manifest_envelope.clone()).expect("gateway state");
    let app = gateway::router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/proof/{manifest_id}"))
        .header("X-SoraFS-Version", &profile_version)
        .header(
            "X-SoraFS-Nonce",
            "7777777777777777777777777777777777777777777777777777777777777777",
        )
        .header("X-SoraFS-Manifest-Envelope", manifest_envelope)
        .body(axum::body::Body::empty())
        .expect("request build");

    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("gateway request should complete");
    assert_eq!(
        response.status(),
        StatusCode::from_u16(scenario.status).unwrap()
    );

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect refusal body");
    let value: Value = norito::json::from_slice(&body_bytes).expect("parse refusal body");
    let obj = value.as_object().expect("refusal must be object");
    assert_eq!(
        obj.get("error").and_then(Value::as_str),
        Some(scenario.error.as_str())
    );
    assert_eq!(
        obj.get("message").and_then(Value::as_str),
        Some(scenario.reason.as_str())
    );
    let details = obj
        .get("details")
        .and_then(Value::as_object)
        .expect("refusal must include details");
    assert_eq!(
        details.get("section").and_then(Value::as_str),
        Some("proof.chunks[0]")
    );
}
