#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account onboarding tests.
#![cfg(feature = "app_api")]

use std::{num::NonZeroU64, sync::Arc};

use axum::http::Request;
use http::StatusCode;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    block::BlockHeader,
    domain::DomainId,
    nexus::DataSpaceId,
    peer::PeerId,
    permission::Permission,
    prelude::{Account, Domain, ExposedPrivateKey},
};
use iroha_executor_data_model::permission::nexus::CanPublishSpaceDirectoryManifest;
use iroha_torii::{Torii, json_entry, json_object};
use mv::storage::StorageReadOnly;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

#[tokio::test]
async fn accounts_onboard_publishes_global_manifest_and_binding() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let authority_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account =
        Account::new(authority_id.clone().to_account_id(domain_id.clone())).build(&authority_id);
    let mut world = World::with([domain], [authority_account], []);
    fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    {
        let height_u64 = u64::try_from(state.view().height())
            .unwrap_or(0)
            .saturating_add(1);
        let header = BlockHeader::new(
            NonZeroU64::new(height_u64).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.world_mut_for_testing().add_account_permission(
            &authority_id,
            Permission::from(CanPublishSpaceDirectoryManifest {
                dataspace: DataSpaceId::GLOBAL,
            }),
        );
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_domain: Some(domain_id.clone()),
    });

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
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

    let chain_id = iroha_data_model::ChainId::from("test-chain");
    let da_receipt_signer = cfg.common.key_pair.clone();
    let torii = {
        #[cfg(feature = "telemetry")]
        {
            Torii::new(
                chain_id.clone(),
                kiso,
                cfg.torii.clone(),
                queue.clone(),
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
                chain_id.clone(),
                kiso,
                cfg.torii.clone(),
                queue.clone(),
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
    let user_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let user_id = AccountId::new(user_kp.public_key().clone());
    let body = json_object(vec![
        json_entry("alias", "p2p-user"),
        json_entry("account_id", user_id.to_string()),
    ]);
    let body = norito::json::to_json(&body).expect("serialize onboarding request");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts/onboard")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .expect("onboarding response");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let expected_height = u64::try_from(state.view().height())
        .unwrap_or(0)
        .saturating_add(1);
    let applied = iroha_torii::test_utils::apply_queued_in_one_block(
        &state,
        &queue,
        &chain_id,
        expected_height,
    );
    assert!(applied > 0);

    let view = state.view();
    let account_entry = view
        .world()
        .account(&user_id)
        .expect("onboarded account exists");
    let uaid = account_entry
        .value()
        .uaid()
        .copied()
        .expect("UAID assigned");
    let bindings = view
        .world()
        .uaid_dataspaces()
        .get(&uaid)
        .expect("UAID bindings present");
    assert!(
        bindings.is_bound_to(DataSpaceId::GLOBAL, &user_id),
        "UAID should be bound to the global dataspace"
    );
    let manifest_set = view
        .world()
        .space_directory_manifests()
        .get(&uaid)
        .expect("manifest registry present");
    let record = manifest_set
        .get(&DataSpaceId::GLOBAL)
        .expect("global manifest present");
    assert!(record.is_active(), "global manifest should be active");
}
