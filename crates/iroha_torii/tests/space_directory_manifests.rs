//! Torii Space Directory manifest endpoint tests.
#![cfg(feature = "app_api")]

use std::sync::Arc;

use axum::{Router, http::Request, routing::post};
use hex::ToHex;
use http::StatusCode;
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    nexus::{
        Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceCatalog,
        DataSpaceId, DataSpaceMetadata, ManifestEffect, ManifestEntry, ManifestVersion,
        UniversalAccountId,
    },
};
use iroha_primitives::numeric::Numeric;
use iroha_torii::Torii;
use norito::json::{self, Value};
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn space_directory_manifest_endpoint_returns_records() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let dataspace = DataSpaceId::new(11);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::space_directory"));
    let account_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new("cbdc".parse().unwrap(), account_key.public_key().clone());
    let compressed_literal = {
        account_id
            .to_account_address()
            .and_then(|address| address.to_compressed_sora())
            .expect("compressed literal")
    };

    let mut world = World::default();
    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    bindings.bind_account(dataspace, account_id.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_096,
        expiry_epoch: Some(8_192),
        entries: vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.transfer".parse().unwrap()),
                method: Some("transfer".parse().unwrap()),
                asset: Some("cbdc#bank".parse().unwrap()),
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::from(500u64)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("Wholesale daily cap".into()),
        }],
    };

    let mut record = SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.activated_epoch = Some(4_096);
    let expected_hash = record.manifest_hash.as_ref().encode_hex::<String>();
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, kura.clone(), query);
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "cbdc".into(),
            description: Some("CBDC lane".into()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    state.nexus.get_mut().dataspace_catalog = dataspace_catalog;
    let state = Arc::new(state);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) =
            iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state.clone(),
            kura.clone(),
            queue.clone(),
            peers_rx.clone(),
            ts,
            false,
        )
        .0
    };
    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state.clone(),
                da_receipt_signer.clone(),
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain"),
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state.clone(),
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/space-directory/uaids/{uaid}/manifests"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = json::from_slice(&body).expect("manifest payload");
    assert_eq!(doc["uaid"], Value::from(uaid.to_string()));
    assert_eq!(doc["total"], Value::from(1));
    let entries = doc["manifests"].as_array().expect("array of manifests");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["dataspace_alias"], Value::from("cbdc"));
    assert_eq!(
        entries[0]["accounts"][0],
        Value::from(account_id.to_string())
    );
    assert_eq!(
        entries[0]["manifest_hash"],
        Value::from(expected_hash.as_str())
    );
    assert_eq!(
        entries[0]["accounts"][0],
        Value::from(account_id.to_string())
    );

    // Compressed address_format rewrites account literals.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?address_format=compressed"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = json::from_slice(&body).expect("manifest payload");
    let entries = doc["manifests"].as_array().expect("array of manifests");
    assert_eq!(
        entries[0]["accounts"][0],
        Value::from(compressed_literal.as_str())
    );

    // Bindings endpoint respects address_format.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/space-directory/uaids/{uaid}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("bindings response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bindings_default = resp.into_body().collect().await.unwrap().to_bytes();
    let bindings_doc: Value = json::from_slice(&bindings_default).expect("bindings payload");
    let dataspaces = bindings_doc["dataspaces"]
        .as_array()
        .expect("dataspaces array");
    assert_eq!(dataspaces.len(), 1);
    assert_eq!(
        dataspaces[0]["accounts"][0],
        Value::from(account_id.to_string())
    );

    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}?address_format=compressed"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("bindings compressed response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bindings_compressed = resp.into_body().collect().await.unwrap().to_bytes();
    let bindings_doc: Value =
        json::from_slice(&bindings_compressed).expect("bindings payload compressed");
    let dataspaces = bindings_doc["dataspaces"]
        .as_array()
        .expect("dataspaces array");
    assert_eq!(
        dataspaces[0]["accounts"][0],
        Value::from(compressed_literal.as_str())
    );

    // Dataspace filter excludes unknown ids.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?dataspace={}",
                    dataspace.as_u64() + 1
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let filtered = resp.into_body().collect().await.unwrap().to_bytes();
    let filtered_doc: Value = json::from_slice(&filtered).expect("filter payload");
    assert_eq!(
        filtered_doc["manifests"].as_array().unwrap().len(),
        0,
        "filter removes non-matching dataspaces"
    );

    // Status filter (active) yields the entry, limit/offset paginate.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?status=Active&limit=1"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let active = resp.into_body().collect().await.unwrap().to_bytes();
    let active_doc: Value = json::from_slice(&active).expect("active payload");
    assert_eq!(active_doc["total"], Value::from(1));
    assert_eq!(
        active_doc["manifests"].as_array().unwrap().len(),
        1,
        "status=Active returns bindings"
    );

    // Build a revoked + second manifest world to test inactive/pagination.
    let mut world_revoked = World::default();
    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    let dataspace_two = DataSpaceId::new(13);
    bindings.bind_account(dataspace, account_id.clone());
    bindings.bind_account(dataspace_two, account_id.clone());
    world_revoked
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let mut record_revoked = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 0,
        activation_epoch: 100,
        expiry_epoch: None,
        entries: Vec::new(),
    });
    record_revoked.lifecycle.activated_epoch = Some(100);
    record_revoked
        .lifecycle
        .mark_revoked(200, Some("test revoke".into()));

    let mut record_active = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: dataspace_two,
        issued_ms: 0,
        activation_epoch: 150,
        expiry_epoch: None,
        entries: Vec::new(),
    });
    record_active.lifecycle.activated_epoch = Some(150);

    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record_revoked);
    set.upsert(record_active);
    world_revoked
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let kura_rev = Kura::blank_kura_for_testing();
    let query_rev = LiveQueryStore::start_test();
    let mut state_revoked = State::new_for_testing(world_revoked, kura_rev.clone(), query_rev);
    state_revoked.nexus.get_mut().dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "cbdc".into(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: dataspace_two,
            alias: "retail".into(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let state_revoked = Arc::new(state_revoked);

    let cfg_rev = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso_rev, _child_rev) = KisoHandle::start(cfg_rev.clone());
    let queue_cfg_rev = iroha_config::parameters::actual::Queue::default();
    let events_sender_rev: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue_rev = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg_rev,
        events_sender_rev,
    ));
    let (revoked_peers_tx, revoked_peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = revoked_peers_tx;
    #[cfg(feature = "telemetry")]
    let telemetry_rev = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = fixtures::shared_metrics();
        let (_mh, ts) =
            iroha_primitives::time::TimeSource::new_mock(core::time::Duration::default());
        core_telemetry::start(
            metrics,
            state_revoked.clone(),
            kura_rev.clone(),
            queue_rev.clone(),
            revoked_peers_rx.clone(),
            ts,
            false,
        )
        .0
    };
    let da_receipt_signer_rev = cfg_rev.common.key_pair.clone();
    let torii_rev = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain-2"),
                kiso_rev,
                cfg_rev.torii.clone(),
                queue_rev,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura_rev,
                state_revoked.clone(),
                da_receipt_signer_rev.clone(),
                iroha_torii::OnlinePeersProvider::new(revoked_peers_rx),
                telemetry_rev,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                iroha_data_model::ChainId::from("test-chain-2"),
                kiso_rev,
                cfg_rev.torii.clone(),
                queue_rev,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura_rev,
                state_revoked.clone(),
                da_receipt_signer_rev,
                iroha_torii::OnlinePeersProvider::new(revoked_peers_rx),
            )
        }
    };

    // Inactive filter returns revoked manifest.
    let resp = torii_rev
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?status=Inactive"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("inactive response");
    assert_eq!(resp.status(), StatusCode::OK);
    let inactive = resp.into_body().collect().await.unwrap().to_bytes();
    let inactive_doc: Value = json::from_slice(&inactive).expect("inactive payload");
    assert_eq!(inactive_doc["total"], Value::from(2));
    assert_eq!(
        inactive_doc["manifests"].as_array().unwrap().len(),
        1,
        "inactive filter returns one entry"
    );
    assert_eq!(
        inactive_doc["manifests"][0]["status"],
        Value::from("Revoked")
    );

    // Active filter + pagination.
    let resp = torii_rev
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?status=Active&limit=1&offset=0"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("active response");
    assert_eq!(resp.status(), StatusCode::OK);
    let active = resp.into_body().collect().await.unwrap().to_bytes();
    let active_doc: Value = json::from_slice(&active).expect("active payload");
    assert_eq!(active_doc["total"], Value::from(2));
    assert_eq!(
        active_doc["manifests"].as_array().unwrap().len(),
        1,
        "pagination limits to 1 entry"
    );

    // limit=0 is treated as "no limit" even when multiple manifests exist.
    let resp = torii_rev
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?limit=0"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("no-limit response");
    assert_eq!(resp.status(), StatusCode::OK);
    let no_limit = resp.into_body().collect().await.unwrap().to_bytes();
    let no_limit_doc: Value = json::from_slice(&no_limit).expect("no-limit payload");
    assert_eq!(no_limit_doc["total"], Value::from(2));
    assert_eq!(
        no_limit_doc["manifests"].as_array().unwrap().len(),
        2,
        "limit=0 should return all matching manifests"
    );
}

#[tokio::test]
async fn manifest_publish_endpoint_enqueues_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "test-chain".parse().expect("chain id");
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let router = Router::new().route(
        "/v1/space-directory/manifests",
        post({
            let chain_id = Arc::new(chain_id.clone());
            let queue = queue.clone();
            let state = state.clone();
            let telemetry = telemetry.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::SpaceDirectoryManifestPublishDto>| {
                let chain_id = chain_id.clone();
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                async move {
                    iroha_torii::handle_post_space_directory_manifest_publish(
                        chain_id, queue, state, telemetry, req,
                    )
                    .await
                }
            }
        }),
    );

    let creds = iroha_torii::test_utils::random_authority();
    let dataspace = DataSpaceId::new(11);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"publish-manifest"));
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_096,
        expiry_epoch: Some(8_192),
        entries: vec![ManifestEntry {
            scope: CapabilityScope {
                dataspace: Some(dataspace),
                program: Some("cbdc.transfer".parse().unwrap()),
                method: Some("transfer".parse().unwrap()),
                asset: Some("cbdc#bank".parse().unwrap()),
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::from(500u64)),
                window: AllowanceWindow::PerDay,
            }),
            notes: None,
        }],
    };
    let manifest_value = norito::json::to_value(&manifest).expect("manifest json");
    let value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("manifest", manifest_value),
        iroha_torii::json_entry("reason", "QA publish trigger"),
    ]);
    let body = norito::json::to_json(&value).expect("serialize publish request");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/space-directory/manifests")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .expect("request");

    let resp = router
        .clone()
        .oneshot(req)
        .await
        .expect("publish response body");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(queue.queued_len(), 1, "publish transaction queued");
}

#[tokio::test]
async fn manifest_revoke_endpoint_enqueues_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(World::default(), kura, query));
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(8).0;
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let queue = Arc::new(Queue::from_config(queue_cfg, events));
    let chain_id: iroha_data_model::ChainId = "test-chain".parse().expect("chain id");
    #[cfg(feature = "telemetry")]
    let telemetry = iroha_torii::MaybeTelemetry::for_tests();
    #[cfg(not(feature = "telemetry"))]
    let telemetry = iroha_torii::MaybeTelemetry::disabled();
    let router = Router::new().route(
        "/v1/space-directory/manifests/revoke",
        post({
            let chain_id = Arc::new(chain_id.clone());
            let queue = queue.clone();
            let state = state.clone();
            let telemetry = telemetry.clone();
            move |req: iroha_torii::NoritoJson<iroha_torii::SpaceDirectoryManifestRevokeDto>| {
                let chain_id = chain_id.clone();
                let queue = queue.clone();
                let state = state.clone();
                let telemetry = telemetry.clone();
                async move {
                    iroha_torii::handle_post_space_directory_manifest_revoke(
                        chain_id, queue, state, telemetry, req,
                    )
                    .await
                }
            }
        }),
    );

    let creds = iroha_torii::test_utils::random_authority();
    let uaid_hash = iroha_crypto::Hash::new(b"space-directory-revoke");
    let uaid_literal = format!("uaid:{}", uaid_hash.as_ref().encode_hex::<String>());
    let value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("uaid", uaid_literal),
        iroha_torii::json_entry("dataspace", 11u64),
        iroha_torii::json_entry("revoked_epoch", 4096u64),
        iroha_torii::json_entry("reason", "test emergency revoke"),
    ]);
    let body = norito::json::to_json(&value).expect("serialize revoke request");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/space-directory/manifests/revoke")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .expect("request");

    let resp = router
        .clone()
        .oneshot(req)
        .await
        .expect("revoke response body");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(queue.queued_len(), 1, "revocation transaction queued");
}
