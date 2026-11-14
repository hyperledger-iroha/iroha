//! Actual operator middleware, bounded artifact responses, original source failure and memory drain.
use super::*;
use iroha_core::snapshot::transfer::{CheckpointTransferV1, transport::TransferLimits};
use std::num::{NonZeroU64, NonZeroUsize};
use tower::ServiceExt as _;

fn fixture() -> (tempfile::TempDir, SharedAppState) {
    let directory = tempfile::tempdir().unwrap();
    // Operator routes validate the API token only after native execution established the
    // listener's authenticated root scope, so the fixture executes its signed genesis first.
    let mut app = crate::tests_runtime_handlers::mk_native_app_state_for_tests();
    let root = iroha_fs::OwnerDirectory::open(directory.path()).unwrap();
    // These opaque artifact bytes exercise transport only, never native State acceptance.
    for (parents, names) in [
        (vec!["kura"], vec!["lane_geometry_journal.norito"]),
        (
            vec!["snapshots", "checkpoints", "2-3"],
            vec![
                "snapshot.data",
                "snapshot.sha256",
                "snapshot.sig",
                "snapshot.fast.norito",
                "snapshot.merkle.json",
            ],
        ),
        (
            vec!["kura", "blocks", "canonical"],
            vec![
                "blocks.data",
                "blocks.index",
                "blocks.hashes",
                "blocks.count.norito",
            ],
        ),
        (
            vec!["kura", "checkpoint_prefix"],
            vec![
                "manifest.norito",
                "bootstrap.data",
                "bootstrap.indices",
                "bootstrap.hashes",
                "bootstrap.count",
                "00000000000000000002.norito",
                "00000000000000000003.norito",
            ],
        ),
    ] {
        let mut parent = iroha_fs::OwnerDirectory::open(root.path()).unwrap();
        for name in parents {
            parent = parent.ensure_child(name).unwrap();
        }
        for name in names {
            parent
                .write_private_atomic(
                    name,
                    b"original bounded artifact bytes",
                    iroha_fs::PublishMode::CreateNew,
                )
                .unwrap();
        }
    }
    let marker = CheckpointTransferV1 {
        version: 1,
        chain_id: app.state.chain_id_ref().clone(),
        network_id: *app.state.network_id_ref(),
        height: 2,
        block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([1; 32])),
        published_state_root: iroha_data_model::sumeragi_finality::KeyedStateRoot([2; 32]),
        snapshot_verification_key: app.da_receipt_signer.public_key().clone(),
    };
    root.write_private_atomic(
        "checkpoint-transfer.json",
        &norito::json::to_vec(&marker).unwrap(),
        iroha_fs::PublishMode::CreateNew,
    )
    .unwrap();
    let source = ExportedCheckpoint::open(
        root.path(),
        app.state.chain_id_ref(),
        *app.state.network_id_ref(),
        TransferLimits {
            max_artifacts: NonZeroUsize::new(32).unwrap(),
            max_total_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
            max_artifact_bytes: NonZeroU64::new(16 * 1024).unwrap(),
            chunk_bytes: NonZeroUsize::new(1024).unwrap(),
        },
        &AllocationBudget::new(1024 * 1024),
    )
    .unwrap();
    Arc::get_mut(&mut app).unwrap().runtime_checkpoint_package =
        Some(Arc::new(parking_lot::Mutex::new(source)));
    (directory, app)
}
fn router(app: &SharedAppState) -> axum::Router {
    use iroha_torii_shared::route_catalog::runtime_governance as routes;
    const ROUTES: &[route_catalog::RouteDescriptor] = &[
        routes::RUNTIME_CHECKPOINT_PACKAGE,
        routes::RUNTIME_CHECKPOINT_ARTIFACT,
        routes::RUNTIME_CHECKPOINT_RANGE,
        routes::RUNTIME_CHECKPOINT_FINISH,
    ];
    let mut builder = RouterBuilder::new(
        app.clone(),
        RouteCatalog::new(ROUTES),
        compiled_route_features(),
    )
    .unwrap();
    builder.route(
        &routes::RUNTIME_CHECKPOINT_PACKAGE,
        catalog_get(descriptor).authenticated_operator(app.clone()),
    );
    builder.route(
        &routes::RUNTIME_CHECKPOINT_ARTIFACT,
        catalog_post(artifact)
            .layer(DefaultBodyLimit::max(REQUEST_BYTES))
            .authenticated_operator(app.clone()),
    );
    builder.route(
        &routes::RUNTIME_CHECKPOINT_RANGE,
        catalog_post(range)
            .layer(DefaultBodyLimit::max(REQUEST_BYTES))
            .authenticated_operator(app.clone()),
    );
    builder.route(
        &routes::RUNTIME_CHECKPOINT_FINISH,
        catalog_post(finish)
            .layer(DefaultBodyLimit::max(REQUEST_BYTES))
            .authenticated_operator(app.clone()),
    );
    builder.finish().unwrap().0.with_state(app.clone())
}
fn request(
    app: &SharedAppState,
    method: Method,
    path: &str,
    bytes: Vec<u8>,
    signed: bool,
) -> axum::http::Request<Body> {
    let uri: crate::Uri = path.parse().unwrap();
    let headers = signed.then(|| {
        operator_signatures::signed_request_headers(
            &app.da_receipt_signer,
            app.state.network_id_ref(),
            &method,
            &uri,
            &bytes,
        )
        .unwrap()
    });
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ACCEPT, "application/json")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .extension(crate::loopback_connect_info())
        .body(Body::from(bytes))
        .unwrap();
    if let Some(headers) = headers {
        request.headers_mut().extend(headers);
    }
    request
}
#[tokio::test]
async fn runtime_checkpoint_routes_authenticate_bound_inventory_and_retain_response_memory() {
    let _guard = crate::test_utils::TestDataDirGuard::new();
    let (_directory, app) = fixture();
    let router = router(&app);
    let unsigned = router
        .clone()
        .oneshot(request(
            &app,
            Method::GET,
            "/v1/runtime/checkpoint/package",
            vec![],
            false,
        ))
        .await
        .unwrap();
    assert_eq!(unsigned.status(), StatusCode::UNAUTHORIZED);
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::GET,
            "/v1/runtime/checkpoint/package",
            vec![],
            true,
        ))
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), METADATA_BYTES)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let descriptor: transport::PackageDescriptorV1 = norito::json::from_slice(&body).unwrap();
    let inventory = descriptor.inventory_sha256;
    let artifact_request = norito::json::to_vec(&ArtifactRequest {
        inventory_sha256: inventory,
        index: 0,
    })
    .unwrap();
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::POST,
            "/v1/runtime/checkpoint/artifact",
            artifact_request,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let artifact: transport::ArtifactV1 = norito::json::from_slice(
        &axum::body::to_bytes(response.into_body(), METADATA_BYTES)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        artifact.length,
        b"original bounded artifact bytes".len() as u64
    );
    let available = app.query_fanout_inflight.available_bytes();
    let range_request = norito::json::to_vec(&RangeRequest {
        inventory_sha256: inventory,
        index: 0,
        offset: 9,
        length: 7,
    })
    .unwrap();
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::POST,
            "/v1/runtime/checkpoint/range",
            range_request,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[axum::http::header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert!(
        app.query_fanout_inflight.available_bytes() < available,
        "original pool credit lives through response drain"
    );
    assert_eq!(
        &axum::body::to_bytes(response.into_body(), 7).await.unwrap()[..],
        b"bounded"
    );
    assert_eq!(app.query_fanout_inflight.available_bytes(), available);
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::POST,
            "/v1/runtime/checkpoint/finish",
            norito::json::to_vec(&FinishRequest {
                inventory_sha256: inventory,
            })
            .unwrap(),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    for (digest, index, offset, length) in [
        (inventory, 0, 0, 0),
        (inventory, 0, 0, 1025),
        (inventory, u64::MAX, 0, 1),
        (inventory, 0, u64::MAX, 1),
        ([0; 32], 0, 0, 1),
    ] {
        let body = norito::json::to_vec(&RangeRequest {
            inventory_sha256: digest,
            index,
            offset,
            length,
        })
        .unwrap();
        let response = router
            .clone()
            .oneshot(request(
                &app,
                Method::POST,
                "/v1/runtime/checkpoint/range",
                body,
                true,
            ))
            .await
            .unwrap();
        assert!(!response.status().is_success());
    }
    let oversize = vec![b' '; REQUEST_BYTES + 1];
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::POST,
            "/v1/runtime/checkpoint/range",
            oversize,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let package = app.runtime_checkpoint_package.as_ref().unwrap();
    let locked = package.lock();
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::GET,
            "/v1/runtime/checkpoint/package",
            vec![],
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    drop(locked);
    let response = router
        .clone()
        .oneshot(request(
            &app,
            Method::GET,
            "/v1/runtime/checkpoint/package",
            vec![],
            true,
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "same source retries after lock release"
    );
}

#[test]
fn runtime_checkpoint_request_shape_rejects_paths_and_bounds_before_allocation() {
    let mut request = norito::json::to_string(&ArtifactRequest {
        inventory_sha256: [0; 32],
        index: 0,
    })
    .unwrap();
    request.pop();
    request.push_str(r#","path":"/etc/passwd"}"#);
    assert!(norito::json::from_str::<ArtifactRequest>(&request).is_err());
    for length in [0, u32::MAX] {
        assert!(
            Work::Range(RangeRequest {
                inventory_sha256: [0; 32],
                index: 0,
                offset: 0,
                length
            })
            .response_bytes(usize::MAX)
            .is_err()
        );
    }
    assert!(
        Work::Range(RangeRequest {
            inventory_sha256: [0; 32],
            index: 0,
            offset: 0,
            length: 1024
        })
        .response_bytes(1023)
        .is_err()
    );
}

#[test]
fn runtime_checkpoint_source_rechecks_authentic_current_floor_and_independent_pins() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let _guard = crate::test_utils::TestDataDirGuard::new();
            let (_directory, mut app) = fixture();
            let mut descriptor = app.runtime_checkpoint_package.as_ref().unwrap().lock().descriptor().unwrap();
            let mut chain = iroha_core::sumeragi::test_chain::CertifiedTestChain::start(
                iroha_core::sumeragi::test_chain::TestChainConfig::new(iroha_core::state::World::new(), 1_000)
            ).unwrap();
            chain.commit(Vec::new());
            chain.commit(Vec::new());
            descriptor.chain_id = chain.state().chain_id_ref().clone();
            descriptor.network_id = *chain.state().network_id_ref();
            let inner = Arc::get_mut(&mut app).unwrap();
            inner.state = chain.state().clone();
            inner.kura = chain.kura().clone();
            validate_source(&app, &descriptor).unwrap();
            let mut foreign = descriptor.clone();
            foreign.chain_id = "foreign-checkpoint".parse().unwrap();
            assert!(matches!(validate_source(&app, &foreign), Err(WorkError::SourcePin)));
            foreign = descriptor.clone();
            foreign.network_id = iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xD1;32])));
            assert!(matches!(validate_source(&app, &foreign), Err(WorkError::SourcePin)));
            let floor = |height| iroha_core::state::upgrade_checkpoint::UpgradeCheckpointV1 {
                height,
                block_hash: chain.kura().get_durable_block_hash(NonZeroUsize::new(height as usize).unwrap()).unwrap(),
            };
            chain.kura().record_history_boundary(&floor(2)).unwrap();
            validate_source(&app, &descriptor).unwrap();
            chain.kura().record_history_boundary(&floor(3)).unwrap();
            assert!(matches!(validate_source(&app, &descriptor), Err(WorkError::HistoricalFloor)), "the same previously allowed package is refused after the authentic floor advances");
        });
    }).unwrap().join().unwrap();
}

#[test]
fn runtime_checkpoint_requests_decode_from_norito_and_json() {
    // NoritoJson admits either body format, so every request body carries both codecs.
    let range = RangeRequest {
        inventory_sha256: [7; 32],
        index: 3,
        offset: 4096,
        length: 512,
    };
    let decoded: RangeRequest =
        norito::decode_from_bytes(&norito::to_bytes(&range).expect("range Norito")).expect("range");
    assert_eq!(
        (
            decoded.inventory_sha256,
            decoded.index,
            decoded.offset,
            decoded.length
        ),
        ([7; 32], 3, 4096, 512)
    );
    let from_json: RangeRequest =
        norito::json::from_str(&norito::json::to_json(&range).expect("range JSON")).expect("range");
    assert_eq!((from_json.offset, from_json.length), (4096, 512));
    let artifact = ArtifactRequest {
        inventory_sha256: [1; 32],
        index: 9,
    };
    let artifact: ArtifactRequest =
        norito::decode_from_bytes(&norito::to_bytes(&artifact).expect("artifact Norito"))
            .expect("artifact");
    assert_eq!((artifact.inventory_sha256, artifact.index), ([1; 32], 9));
    let finish = FinishRequest {
        inventory_sha256: [2; 32],
    };
    let finish: FinishRequest =
        norito::decode_from_bytes(&norito::to_bytes(&finish).expect("finish Norito"))
            .expect("finish");
    assert_eq!(finish.inventory_sha256, [2; 32]);
}
