#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
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
    asset::AssetDefinitionId,
    domain::DomainId,
    nexus::{
        Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceCatalog,
        DataSpaceId, DataSpaceMetadata, ManifestEffect, ManifestEntry, ManifestVersion,
        UniversalAccountId,
    },
    peer::PeerId,
    transaction::Executable,
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
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let dataspace = DataSpaceId::new(11);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::space_directory"));
    let account_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(account_key.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
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
                asset: Some(AssetDefinitionId::new(
                    DomainId::try_new("bank", "universal").expect("domain id"),
                    "cbdc".parse().expect("asset definition name"),
                )),
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
            local_peer_id,
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

    let raw_uaid = uaid.to_string().trim_start_matches("uaid:").to_owned();

    // Bindings endpoint returns canonical account literals.
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

    // Raw 64-hex UAID paths are accepted and canonicalized in the response payload.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/space-directory/uaids/{raw_uaid}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("raw-hex bindings response");
    assert_eq!(resp.status(), StatusCode::OK);
    let raw_bindings = resp.into_body().collect().await.unwrap().to_bytes();
    let raw_bindings_doc: Value = json::from_slice(&raw_bindings).expect("raw bindings payload");
    assert_eq!(raw_bindings_doc["uaid"], Value::from(uaid.to_string()));
    assert_eq!(
        raw_bindings_doc["dataspaces"][0]["accounts"][0],
        Value::from(account_id.to_string())
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
    assert_eq!(filtered_doc["total"], Value::from(0));
    assert_eq!(
        filtered_doc["manifests"].as_array().unwrap().len(),
        0,
        "filter removes non-matching dataspaces"
    );

    // Configured dataspace with no explicit lane route still falls back to fanout filtering.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?dataspace={}",
                    dataspace.as_u64()
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let filtered_existing = resp.into_body().collect().await.unwrap().to_bytes();
    let filtered_existing_doc: Value =
        json::from_slice(&filtered_existing).expect("configured dataspace payload");
    assert_eq!(filtered_existing_doc["total"], Value::from(1));
    assert_eq!(
        filtered_existing_doc["manifests"].as_array().unwrap().len(),
        1,
        "configured dataspace filter preserves the matching manifest"
    );
    assert_eq!(
        filtered_existing_doc["manifests"][0]["manifest_hash"],
        Value::from(expected_hash.as_str())
    );

    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/space-directory/uaids/{raw_uaid}/manifests"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("raw-hex manifests response");
    assert_eq!(resp.status(), StatusCode::OK);
    let raw_manifests = resp.into_body().collect().await.unwrap().to_bytes();
    let raw_manifests_doc: Value = json::from_slice(&raw_manifests).expect("raw manifests payload");
    assert_eq!(raw_manifests_doc["uaid"], Value::from(uaid.to_string()));
    assert_eq!(raw_manifests_doc["total"], Value::from(1));
    assert_eq!(
        raw_manifests_doc["manifests"][0]["manifest_hash"],
        Value::from(expected_hash.as_str())
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

    // Invalid status values are rejected by the manifest query parser.
    let app = torii.api_router_for_tests();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?status=DefinitelyNotAStatus"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Build a revoked + second manifest world to test inactive/pagination.
    let cfg_rev = iroha_torii::test_utils::mk_minimal_root_cfg();
    let local_peer_id_rev = PeerId::new(cfg_rev.common.key_pair.public_key().clone());
    let mut world_revoked = World::default();
    fixtures::seed_peer(&mut world_revoked, local_peer_id_rev.clone());
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
            local_peer_id_rev,
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
async fn space_directory_get_routes_reject_invalid_uaid_literals() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
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
            local_peer_id,
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
                state,
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
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let bindings_resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri("/v1/space-directory/uaids/uaid:1234")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("bindings response");
    assert_eq!(bindings_resp.status(), StatusCode::BAD_REQUEST);

    let manifests_resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri("/v1/space-directory/uaids/uaid:1234/manifests")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("manifests response");
    assert_eq!(manifests_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn space_directory_bindings_route_returns_multiple_dataspaces_with_aliases() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let uaid = UniversalAccountId::from_hash(Hash::new(b"space-directory-bindings-multi"));
    let primary_dataspace = DataSpaceId::new(31);
    let secondary_dataspace = DataSpaceId::new(33);
    let primary_account = AccountId::new(KeyPair::random().public_key().clone());
    let secondary_account = AccountId::new(KeyPair::random().public_key().clone());
    let tertiary_account = AccountId::new(KeyPair::random().public_key().clone());

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    bindings.bind_account(primary_dataspace, secondary_account.clone());
    bindings.bind_account(primary_dataspace, primary_account.clone());
    bindings.bind_account(secondary_dataspace, tertiary_account.clone());
    world
        .uaid_dataspaces_mut_for_testing()
        .insert(uaid, bindings);

    let mut primary_record = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: primary_dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_096,
        expiry_epoch: Some(8_192),
        entries: Vec::new(),
    });
    primary_record.lifecycle.mark_activated(4_096);

    let mut secondary_record = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: secondary_dataspace,
        issued_ms: 1_762_723_200_100,
        activation_epoch: 4_097,
        expiry_epoch: Some(8_193),
        entries: Vec::new(),
    });
    secondary_record.lifecycle.mark_activated(4_097);

    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(primary_record);
    set.upsert(secondary_record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, kura.clone(), query);
    state.nexus.get_mut().dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: primary_dataspace,
            alias: "retail".into(),
            description: Some("Retail lane".into()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let state = Arc::new(state);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
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
            local_peer_id,
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
                state,
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
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/space-directory/uaids/{uaid}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("bindings response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = json::from_slice(&body).expect("bindings payload");
    assert_eq!(doc["uaid"], Value::from(uaid.to_string()));
    let dataspaces = doc["dataspaces"].as_array().expect("dataspaces array");
    assert_eq!(dataspaces.len(), 2);

    let primary = dataspaces
        .iter()
        .find(|entry| entry["dataspace_id"] == Value::from(primary_dataspace.as_u64()))
        .expect("primary dataspace entry");
    assert_eq!(primary["dataspace_alias"], Value::from("retail"));
    let mut actual_primary_accounts: Vec<_> = primary["accounts"]
        .as_array()
        .expect("primary accounts array")
        .iter()
        .map(|value| value.as_str().expect("primary account literal").to_owned())
        .collect();
    let mut expected_primary_accounts =
        vec![primary_account.to_string(), secondary_account.to_string()];
    actual_primary_accounts.sort_unstable();
    expected_primary_accounts.sort_unstable();
    assert_eq!(actual_primary_accounts, expected_primary_accounts);

    let secondary = dataspaces
        .iter()
        .find(|entry| entry["dataspace_id"] == Value::from(secondary_dataspace.as_u64()))
        .expect("secondary dataspace entry");
    assert!(
        secondary["dataspace_alias"].is_null(),
        "missing catalog alias should stay null in the public route payload",
    );
    assert_eq!(
        secondary["accounts"][0],
        Value::from(tertiary_account.to_string())
    );
}

#[tokio::test]
async fn space_directory_manifest_endpoint_keeps_prefilter_total_when_public_page_is_empty() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let uaid = UniversalAccountId::from_hash(Hash::new(b"space-directory-empty-page"));
    let active_dataspace = DataSpaceId::new(41);
    let revoked_dataspace = DataSpaceId::new(42);

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());

    let mut active_record = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: active_dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_096,
        expiry_epoch: Some(8_192),
        entries: Vec::new(),
    });
    active_record.lifecycle.mark_activated(4_096);

    let mut revoked_record = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace: revoked_dataspace,
        issued_ms: 1_762_723_200_100,
        activation_epoch: 4_097,
        expiry_epoch: Some(8_193),
        entries: Vec::new(),
    });
    revoked_record.lifecycle.mark_activated(4_097);
    revoked_record.lifecycle.mark_revoked(4_200, None);

    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(active_record);
    set.upsert(revoked_record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, kura.clone(), query);
    state.nexus.get_mut().dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: active_dataspace,
            alias: "active".into(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: revoked_dataspace,
            alias: "revoked".into(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let state = Arc::new(state);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
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
            local_peer_id,
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
                state,
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
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/space-directory/uaids/{uaid}/manifests?status=ACTIVE&limit=1&offset=1"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .expect("manifests response");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = json::from_slice(&body).expect("manifests payload");
    assert_eq!(doc["uaid"], Value::from(uaid.to_string()));
    assert_eq!(doc["total"], Value::from(2));
    assert_eq!(
        doc["manifests"].as_array().expect("manifests array").len(),
        0,
        "total should reflect the pre-status-filter set size even when pagination clears the page",
    );
}

#[tokio::test]
async fn space_directory_manifest_endpoint_keeps_null_revocation_reason_in_json() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let dataspace = DataSpaceId::new(21);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"space-directory-null-reason"));
    let account = AccountId::new(KeyPair::random().public_key().clone());

    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let mut bindings = iroha_core::nexus::space_directory::UaidDataspaceBindings::default();
    bindings.bind_account(dataspace, account.clone());
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
        entries: Vec::new(),
    };
    let mut record = SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(4_096);
    record.lifecycle.mark_revoked(4_200, None);
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    world
        .space_directory_manifests_mut_for_testing()
        .insert(uaid, set);

    let mut state = State::new_for_testing(world, kura.clone(), query);
    state.nexus.get_mut().dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace,
            alias: "archived".into(),
            description: Some("Archived dataspace".into()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let state = Arc::new(state);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
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
            local_peer_id,
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
                state,
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
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let resp = torii
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
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = json::from_slice(&body).expect("inactive payload");
    assert_eq!(doc["total"], Value::from(1));
    assert_eq!(doc["manifests"][0]["status"], Value::from("Revoked"));
    assert_eq!(
        doc["manifests"][0]["dataspace_alias"],
        Value::from("archived")
    );
    assert_eq!(
        doc["manifests"][0]["accounts"][0],
        Value::from(account.to_string())
    );
    assert!(
        doc["manifests"][0]["lifecycle"]["revocation"]["reason"].is_null(),
        "reasonless revocations should stay explicit nulls in route payloads",
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
                asset: Some(AssetDefinitionId::new(
                    DomainId::try_new("bank", "universal").expect("domain id"),
                    "cbdc".parse().expect("asset definition name"),
                )),
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
async fn manifest_publish_endpoint_applies_reason_only_to_entries_missing_notes() {
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
    let uaid = UniversalAccountId::from_hash(Hash::new(b"publish-manifest-reason"));
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid,
        dataspace,
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_096,
        expiry_epoch: Some(8_192),
        entries: vec![
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: Some(dataspace),
                    program: Some("cbdc.transfer".parse().unwrap()),
                    method: Some("transfer".parse().unwrap()),
                    asset: Some(AssetDefinitionId::new(
                        DomainId::try_new("bank", "universal").expect("domain id"),
                        "cbdc".parse().expect("asset definition name"),
                    )),
                    role: None,
                },
                effect: ManifestEffect::Allow(Allowance {
                    max_amount: Some(Numeric::from(500u64)),
                    window: AllowanceWindow::PerDay,
                }),
                notes: None,
            },
            ManifestEntry {
                scope: CapabilityScope {
                    dataspace: Some(dataspace),
                    program: Some("cbdc.transfer".parse().unwrap()),
                    method: Some("refund".parse().unwrap()),
                    asset: Some(AssetDefinitionId::new(
                        DomainId::try_new("bank", "universal").expect("domain id"),
                        "cbdc".parse().expect("asset definition name"),
                    )),
                    role: None,
                },
                effect: ManifestEffect::Allow(Allowance {
                    max_amount: Some(Numeric::from(100u64)),
                    window: AllowanceWindow::PerDay,
                }),
                notes: Some("keep existing".into()),
            },
        ],
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

    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).collect::<Vec<_>>()
    };
    assert_eq!(queued.len(), 1, "publish transaction queued");
    let tx = queued.first().expect("queued transaction");
    let external = tx.external().expect("queued tx should be external");
    let Executable::Instructions(instructions) = external.instructions() else {
        panic!("publish request should enqueue instruction transaction");
    };
    assert_eq!(instructions.len(), 1);
    let publish = instructions[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest>()
        .expect("queued instruction should be publish manifest");
    assert_eq!(
        publish.manifest.entries[0].notes.as_deref(),
        Some("QA publish trigger")
    );
    assert_eq!(
        publish.manifest.entries[1].notes.as_deref(),
        Some("keep existing")
    );
}

#[tokio::test]
async fn manifest_publish_endpoint_preserves_missing_notes_when_reason_is_omitted() {
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
    let dataspace = DataSpaceId::new(12);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"publish-manifest-no-reason"));
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
                asset: Some(AssetDefinitionId::new(
                    DomainId::try_new("bank", "universal").expect("domain id"),
                    "cbdc".parse().expect("asset definition name"),
                )),
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

    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).collect::<Vec<_>>()
    };
    assert_eq!(queued.len(), 1, "publish transaction queued");
    let tx = queued.first().expect("queued transaction");
    let external = tx.external().expect("queued tx should be external");
    let Executable::Instructions(instructions) = external.instructions() else {
        panic!("publish request should enqueue instruction transaction");
    };
    let publish = instructions[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest>()
        .expect("queued instruction should be publish manifest");
    assert!(
        publish.manifest.entries[0].notes.is_none(),
        "omitting reason should leave missing notes untouched",
    );
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

#[tokio::test]
async fn manifest_revoke_endpoint_canonicalizes_uaid_literal_in_queued_instruction() {
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
    let uaid_hash = iroha_crypto::Hash::new(b"space-directory-revoke-canonical");
    let expected_uaid = UniversalAccountId::from_hash(uaid_hash);
    let uaid_literal = format!(
        "  UaId:  {}  ",
        uaid_hash.as_ref().encode_hex::<String>().to_uppercase()
    );
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

    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).collect::<Vec<_>>()
    };
    assert_eq!(queued.len(), 1, "revoke transaction queued");
    let tx = queued.first().expect("queued transaction");
    let external = tx.external().expect("queued tx should be external");
    let Executable::Instructions(instructions) = external.instructions() else {
        panic!("revoke request should enqueue instruction transaction");
    };
    let revoke = instructions[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::space_directory::RevokeSpaceDirectoryManifest>()
        .expect("queued instruction should be revoke manifest");
    assert_eq!(revoke.uaid, expected_uaid);
    assert_eq!(revoke.dataspace, DataSpaceId::new(11));
    assert_eq!(revoke.revoked_epoch, 4096);
    assert_eq!(revoke.reason.as_deref(), Some("test emergency revoke"));
}

#[tokio::test]
async fn manifest_revoke_endpoint_accepts_raw_hex_uaid_without_reason() {
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
    let uaid_hash = iroha_crypto::Hash::new(b"space-directory-revoke-raw-no-reason");
    let expected_uaid = UniversalAccountId::from_hash(uaid_hash);
    let raw_hex = uaid_hash.as_ref().encode_hex::<String>();
    let value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("uaid", raw_hex),
        iroha_torii::json_entry("dataspace", 12u64),
        iroha_torii::json_entry("revoked_epoch", 8192u64),
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

    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).collect::<Vec<_>>()
    };
    assert_eq!(queued.len(), 1, "revoke transaction queued");
    let tx = queued.first().expect("queued transaction");
    let external = tx.external().expect("queued tx should be external");
    let Executable::Instructions(instructions) = external.instructions() else {
        panic!("revoke request should enqueue instruction transaction");
    };
    let revoke = instructions[0]
        .as_any()
        .downcast_ref::<iroha_data_model::isi::space_directory::RevokeSpaceDirectoryManifest>()
        .expect("queued instruction should be revoke manifest");
    assert_eq!(revoke.uaid, expected_uaid);
    assert_eq!(revoke.dataspace, DataSpaceId::new(12));
    assert_eq!(revoke.revoked_epoch, 8192);
    assert!(
        revoke.reason.is_none(),
        "omitted revoke reason should stay absent in the queued instruction",
    );
}

#[tokio::test]
async fn manifest_revoke_endpoint_rejects_invalid_uaid_before_queueing() {
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
    let value = iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("uaid", "uaid:1234"),
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
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        queue.queued_len(),
        0,
        "invalid UAID must not enqueue a transaction"
    );
    let queued = {
        let state_view = state.view();
        queue.all_transactions(&state_view).count()
    };
    assert_eq!(
        queued, 0,
        "invalid UAID must not leave pending transactions"
    );
}

#[tokio::test]
async fn api_router_registers_space_directory_manifest_mutation_routes() {
    let cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
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
            local_peer_id,
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
                queue.clone(),
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
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
                queue.clone(),
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };

    let creds = iroha_torii::test_utils::random_authority();
    let dataspace = DataSpaceId::new(11);
    let uaid = UniversalAccountId::from_hash(Hash::new(b"router-manifest"));
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
                asset: Some(AssetDefinitionId::new(
                    DomainId::try_new("bank", "universal").expect("domain id"),
                    "cbdc".parse().expect("asset definition name"),
                )),
                role: None,
            },
            effect: ManifestEffect::Allow(Allowance {
                max_amount: Some(Numeric::from(500u64)),
                window: AllowanceWindow::PerDay,
            }),
            notes: Some("router registration".into()),
        }],
    };
    let manifest_value = norito::json::to_value(&manifest).expect("manifest json");
    let publish_body = norito::json::to_json(&iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("manifest", manifest_value),
        iroha_torii::json_entry("reason", "router publish"),
    ]))
    .expect("serialize publish request");
    let publish_resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/space-directory/manifests")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(publish_body))
                .expect("publish request"),
        )
        .await
        .expect("publish response");
    assert_ne!(
        publish_resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "publish route must be registered on the Torii API router",
    );
    assert_ne!(
        publish_resp.status(),
        StatusCode::NOT_FOUND,
        "publish route must be registered on the Torii API router",
    );

    let uaid_literal = format!(
        "uaid:{}",
        Hash::new(b"router-revoke").as_ref().encode_hex::<String>()
    );
    let revoke_body = norito::json::to_json(&iroha_torii::json_object(vec![
        iroha_torii::json_entry("authority", creds.account.clone()),
        iroha_torii::json_entry("private_key", creds.private_key.to_string()),
        iroha_torii::json_entry("uaid", uaid_literal),
        iroha_torii::json_entry("dataspace", dataspace.as_u64()),
        iroha_torii::json_entry("revoked_epoch", 4096u64),
        iroha_torii::json_entry("reason", "router revoke"),
    ]))
    .expect("serialize revoke request");
    let revoke_resp = torii
        .api_router_for_tests()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/space-directory/manifests/revoke")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(revoke_body))
                .expect("revoke request"),
        )
        .await
        .expect("revoke response");
    assert_ne!(
        revoke_resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "revoke route must be registered on the Torii API router",
    );
    assert_ne!(
        revoke_resp.status(),
        StatusCode::NOT_FOUND,
        "revoke route must be registered on the Torii API router",
    );
}
