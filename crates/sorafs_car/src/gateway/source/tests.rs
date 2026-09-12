//! Real loopback HTTP scope: native payload verification, not governance or HTTPS deployment.
use super::*;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::SigningKey;
use sorafs_manifest::{ManifestBuilder, StreamTokenBodyV1, StreamTokenV1};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::Mutex,
    thread,
};

fn limits() -> GatewaySourceLimitsV1 {
    GatewaySourceLimitsV1 {
        max_payload_bytes: 4 * 1024 * 1024,
        max_files: 16,
        max_chunks: 16,
        max_page_bytes: 64 * 1024,
        max_pages: 8,
        page_entries: 2,
    }
}
fn fixture() -> (ManifestV1, CarBuildPlan, Vec<u8>) {
    let temp = tempfile::tempdir().expect("directory");
    let root = temp.path().canonicalize().expect("canonical temp");
    for index in 0..5 {
        std::fs::write(
            root.join(format!("{index}.txt")),
            format!("SORA 空 {index}\n"),
        )
        .expect("file");
    }
    let (plan, payload) = CarBuildPlan::from_directory(&root).expect("native plan");
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload.as_slice(), &mut io::sink())
        .expect("CAR");
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(sorafs_manifest::DagCodecId(stats.dag_codec))
        .chunking_profile(sorafs_manifest::ChunkingProfileV1::from_descriptor(
            sorafs_manifest::chunker_registry::lookup_by_handle("sorafs.sf1@1.0.0")
                .expect("profile"),
        ))
        .chunk_digest_sha3_256(crate::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(crate::compute_por_root(&payload, &plan).expect("PoR"))
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(sorafs_manifest::PinPolicy {
            retention_epoch: 100,
            ..sorafs_manifest::PinPolicy::default()
        })
        .build()
        .expect("manifest");
    (manifest, plan, payload)
}
macro_rules! object {
    ($($key:literal: $value:expr),* $(,)?) => {{
        let mut map = json::Map::new();
        $(map.insert($key.to_owned(), json::to_value(&$value).expect("JSON fixture field"));)*
        Value::Object(map)
    }};
}
fn responses(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    payload: &[u8],
) -> BTreeMap<String, Vec<u8>> {
    let id = hex::encode(manifest.digest().expect("digest").as_bytes());
    let mut responses = BTreeMap::new();
    responses.insert(format!("/v1/sorafs/storage/manifest/{id}"), json::to_vec(&object! {
        "manifest_b64": STANDARD.encode(manifest.encode().expect("encode")), "manifest_digest_hex": id.clone(),
        "payload_digest_hex": plan.payload_digest.to_hex().to_string(), "content_length": plan.content_length,
        "chunk_count": plan.chunks.len(), "chunk_profile_handle": "sorafs.sf1@1.0.0"
    }).expect("manifest JSON"));
    for offset in (0..plan.files.len().max(plan.chunks.len())).step_by(2) {
        let chunks = plan.chunks.iter().enumerate().skip(offset).take(2).map(|(index, chunk)| object! {
            "chunk_index": index, "offset": chunk.offset, "length": chunk.length, "digest_blake3": hex::encode(chunk.digest)
        }).collect::<Vec<_>>();
        let mut file_offset = 0;
        let files = plan.files.iter().enumerate().filter_map(|(index, file)| {
            let entry = object! { "path": file.path.clone(), "offset": file_offset, "size": file.size,
                "first_chunk": file.first_chunk, "chunk_count": file.chunk_count };
            file_offset += file.size;
            (index >= offset && index < offset + 2).then_some(entry)
        }).collect::<Vec<_>>();
        let digests = plan
            .chunks
            .iter()
            .skip(offset)
            .take(2)
            .map(|chunk| hex::encode(chunk.digest))
            .collect::<Vec<_>>();
        let page_plan = object! {
            "chunk_count": plan.chunks.len(), "offset": offset, "returned_chunk_count": chunks.len(), "limit": 2_u64,
            "truncated_chunks": offset + chunks.len() < plan.chunks.len(), "content_length": plan.content_length,
            "payload_digest_blake3": plan.payload_digest.to_hex().to_string(), "chunk_profile_handle": "sorafs.sf1@1.0.0",
            "file_count": plan.files.len(), "returned_file_count": files.len(), "truncated_files": offset + files.len() < plan.files.len(),
            "files": files, "chunk_digest_count": plan.chunks.len(), "returned_chunk_digest_count": digests.len(),
            "truncated_chunk_digests": offset + digests.len() < plan.chunks.len(), "chunk_digests_blake3": digests, "chunks": chunks
        };
        responses.insert(
            format!("/v1/sorafs/storage/plan/{id}?offset={offset}&limit=2"),
            json::to_vec(&object! { "manifest_id_hex": id.clone(), "plan": page_plan })
                .expect("plan JSON"),
        );
    }
    for chunk in &plan.chunks {
        responses.insert(
            format!(
                "/v1/sorafs/storage/chunk/{id}/{}",
                hex::encode(chunk.digest)
            ),
            payload[chunk.offset as usize..chunk.offset as usize + chunk.length as usize].to_vec(),
        );
    }
    responses
}
struct LoopbackEngine {
    client: Arc<ReqwestEngine>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
    responses: Arc<BTreeMap<String, Vec<u8>>>,
    mode: u8,
}
impl HttpEngine for LoopbackEngine {
    fn get(&self, request: HttpRequest) -> HttpFuture {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
        let address = listener.local_addr().expect("address");
        let key = match request.url.query() {
            Some(query) => format!("{}?{query}", request.url.path()),
            None => request.url.path().to_owned(),
        };
        let body = self.responses.get(&key).cloned().unwrap_or_default();
        let mode = self.mode;
        self.workers.lock().unwrap().push(thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut incoming = [0; 4096]; let _ = stream.read(&mut incoming);
            let length = body.len() + usize::from(mode == 2);
            let status = if mode == 1 { "302 Found" } else { "200 OK" };
            let head = format!("HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\nLocation: http://127.0.0.1:1/forbidden\r\n\r\n");
            if stream.write_all(head.as_bytes()).is_ok() { let _ = stream.write_all(&body); }
        }));
        let client = Arc::clone(&self.client);
        let url = Url::parse(&format!("http://{address}{key}")).unwrap();
        client.get(HttpRequest {
            url,
            headers: request.headers,
            max_response_bytes: request.max_response_bytes,
        })
    }
}
impl Drop for LoopbackEngine {
    fn drop(&mut self) {
        for worker in self.workers.get_mut().unwrap().drain(..) {
            worker.join().expect("server");
        }
    }
}
fn context(
    manifest: &ManifestV1,
    responses: BTreeMap<String, Vec<u8>>,
    mode: u8,
) -> GatewayFetchContext {
    let key = SigningKey::from_bytes(&[0x42; 32]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let token = StreamTokenV1::sign(
        StreamTokenBodyV1 {
            token_id: "0123456789abcdef0123456789abcdef".into(),
            manifest_cid: manifest.root_cid.clone(),
            provider_id: [0x12; 32],
            profile_handle: "sorafs.sf1@1.0.0".into(),
            max_streams: 1,
            ttl_epoch: now + 60,
            rate_limit_bytes: 4 * 1024 * 1024,
            issued_at: now,
            requests_per_minute: 120,
            token_pk_version: 1,
        },
        &key,
    )
    .unwrap();
    let client = Client::builder()
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let engine = Arc::new(LoopbackEngine {
        client: Arc::new(ReqwestEngine::new(client)),
        workers: Mutex::new(Vec::new()),
        responses: Arc::new(responses),
        mode,
    });
    GatewayFetchContext::build_with_engine(
        GatewayFetchConfig {
            manifest_id_hex: hex::encode(manifest.digest().unwrap().as_bytes()),
            chunker_handle: "sorafs.sf1@1.0.0".into(),
            manifest_envelope_b64: None,
            client_id: None,
            expected_manifest_cid_hex: Some(hex::encode(&manifest.root_cid)),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
        },
        [GatewayProviderInput {
            name: "source".into(),
            provider_id_hex: hex::encode([0x12; 32]),
            gateway_public_key_hex: hex::encode(key.verifying_key().to_bytes()),
            base_url: "https://provider.example/".into(),
            stream_token_b64: STANDARD.encode(norito::encode_canonical(&token).unwrap()),
            privacy_events_url: None,
        }],
        engine,
    )
    .unwrap()
}
#[tokio::test]
async fn real_http_native_multifile_payload_verifies_complete_pagination_and_commitments() {
    let (manifest, plan, payload) = fixture();
    let context = context(&manifest, responses(&manifest, &plan, &payload), 0);
    let metadata = context
        .fetch_manifest()
        .await
        .expect("native manifest response");
    assert_eq!(metadata.manifest, manifest);
    assert_eq!(
        context
            .fetch_bound_plan_v1(&manifest, limits())
            .await
            .expect("complete native plan"),
        plan
    );
    let (observed, observed_plan, observed_payload) = context
        .fetch_verified_payload_v1(&manifest, limits())
        .await
        .expect("verified HTTP native payload")
        .into_parts();
    assert_eq!(observed, manifest);
    assert_eq!(observed_plan, plan);
    assert_eq!(observed_payload, payload);
}
#[tokio::test]
async fn real_http_source_rejects_redirects_truncated_body_and_changed_chunk() {
    let (manifest, plan, payload) = fixture();
    for mode in [1, 2] {
        assert!(
            context(&manifest, responses(&manifest, &plan, &payload), mode)
                .fetch_verified_payload_v1(&manifest, limits())
                .await
                .is_err()
        );
    }
    let mut changed = responses(&manifest, &plan, &payload);
    let chunk = changed
        .iter_mut()
        .find(|(path, _)| path.contains("/chunk/"))
        .unwrap()
        .1;
    chunk[0] ^= 1;
    assert!(matches!(
        context(&manifest, changed, 0)
            .fetch_verified_payload_v1(&manifest, limits())
            .await,
        Err(GatewaySourceErrorV1::ContentRejected)
    ));
}
#[tokio::test]
async fn real_http_source_rejects_changed_later_page_and_manifest_substitution() {
    let (manifest, plan, payload) = fixture();
    for field in [
        "offset",
        "content_length",
        "chunk_count",
        "file_count",
        "returned_file_count",
        "limit",
    ] {
        let mut changed = responses(&manifest, &plan, &payload);
        let page = changed
            .iter_mut()
            .find(|(path, _)| path.contains("offset=2"))
            .unwrap()
            .1;
        let mut value: Value = json::from_slice(page).unwrap();
        value
            .get_mut("plan")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), Value::from(999_u64));
        *page = json::to_vec(&value).unwrap();
        assert!(
            context(&manifest, changed, 0)
                .fetch_verified_payload_v1(&manifest, limits())
                .await
                .is_err(),
            "{field}"
        );
    }
    let mut other = manifest.clone();
    other.por_root[0] ^= 1;
    assert!(
        context(&manifest, responses(&manifest, &plan, &payload), 0)
            .fetch_verified_payload_v1(&other, limits())
            .await
            .is_err()
    );
}
#[test]
fn source_limits_reject_zero_excessive_and_incomplete_page_budgets() {
    limits().validate().unwrap();
    for altered in [
        GatewaySourceLimitsV1 {
            max_payload_bytes: 0,
            ..limits()
        },
        GatewaySourceLimitsV1 {
            max_payload_bytes: u64::MAX,
            ..limits()
        },
        GatewaySourceLimitsV1 {
            page_entries: 0,
            ..limits()
        },
        GatewaySourceLimitsV1 {
            max_pages: 1,
            ..limits()
        },
        GatewaySourceLimitsV1 {
            max_page_bytes: usize::MAX,
            ..limits()
        },
    ] {
        assert!(altered.validate().is_err());
    }
}

#[test]
fn source_tls_root_pins_are_required_and_bounded_before_dns() {
    let config = GatewayFetchConfig {
        manifest_id_hex: "11".repeat(32),
        chunker_handle: "sorafs.sf1@1.0.0".to_owned(),
        manifest_envelope_b64: None,
        client_id: None,
        expected_manifest_cid_hex: None,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: None,
    };
    for roots in [
        Vec::new(),
        vec![Vec::new()],
        vec![vec![0; 16 * 1024 + 1]],
        vec![vec![1]; 5],
    ] {
        assert!(matches!(
            GatewayFetchContext::new_with_pinned_tls_roots(
                config.clone(),
                [],
                Duration::from_secs(1),
                Duration::from_secs(2),
                &roots
            ),
            Err(GatewayBuildError::InvalidPinnedTlsRoots)
        ));
    }
}
