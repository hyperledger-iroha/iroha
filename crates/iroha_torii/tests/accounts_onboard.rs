#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii account onboarding tests.
#![cfg(feature = "app_api")]

use std::{num::NonZeroU64, str::FromStr, sync::Arc};

use axum::{extract::connect_info::ConnectInfo, http::Request};
use http::StatusCode;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    block::BlockHeader,
    domain::DomainId,
    name::Name,
    nexus::{DataSpaceId, UniversalAccountId},
    peer::PeerId,
    permission::Permission,
    prelude::{Account, Domain, ExposedPrivateKey, Mint},
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use iroha_executor_data_model::permission::nexus::CanPublishSpaceDirectoryManifest;
use iroha_torii::{Torii, json_entry, json_object};
use mv::storage::StorageReadOnly;
use tower::ServiceExt as _;

#[path = "fixtures.rs"]
mod fixtures;

fn checked_onboard_ed25519_key_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked account onboarding Ed25519 fixture keypair")
}

#[test]
fn accounts_onboard_ed25519_fixture_uses_checked_key_generation() {
    let key_pair = checked_onboard_ed25519_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("fixture account onboarding public key has a valid algorithm");

    assert_eq!(algorithm, Algorithm::Ed25519);
}

async fn post_account_onboarding_for_validation(
    body: norito::json::Value,
) -> (StatusCode, norito::json::Value) {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("genesis domain id");
    let authority_kp = checked_onboard_ed25519_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let genesis_domain = Domain::new(genesis_domain_id).build(&authority_id);
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let payment_definition = AssetDefinition::numeric(payment_asset_definition_id)
        .with_name("xor".to_owned())
        .build(&authority_id);
    let mut world = World::with(
        [genesis_domain, domain],
        [authority_account],
        [payment_definition],
    );
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
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id,
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_permissions: Vec::new(),
        fee_sponsor_account: None,
        alias_lease_term_years: 1,
        alias_auto_renew_enabled: false,
        alias_auto_renew_retry_backoff_ms: 86_400_000,
        alias_auto_renew_max_failures: 5,
        alias_auto_renew_subscription_domain: None,
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
                chain_id,
                kiso,
                cfg.torii.clone(),
                queue,
                tokio::sync::broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
                telemetry,
                true,
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            Torii::new(
                chain_id,
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

    let body = norito::json::to_json(&body).expect("serialize onboarding request");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/accounts/onboard")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header(axum::http::header::ACCEPT, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));

    let resp = torii
        .api_router_for_tests()
        .oneshot(req)
        .await
        .expect("onboarding response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let payload = norito::json::from_slice(&bytes).unwrap_or_else(|err| {
        panic!(
            "decode onboarding error response: {err}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    });
    (status, payload)
}

#[tokio::test]
async fn accounts_onboard_rejects_invalid_uaid_contract() {
    let account_id = AccountId::new(checked_onboard_ed25519_key_fixture().public_key().clone());
    let public_key_hex = "1111111111111111111111111111111111111111111111111111111111111111";
    let uaid = UniversalAccountId::from_hash(Hash::new(b"accounts-onboard::validation"));

    let cases = [
        (
            "missing_uaid",
            norito::json!({
                "alias": "invalid-missing-uaid@universal",
                "account_id": (account_id.to_string())
            }),
        ),
        (
            "raw_identity_not_allowed",
            norito::json!({
                "alias": "invalid-raw-identity@universal",
                "account_id": (account_id.to_string()),
                "uaid": (uaid.to_string()),
                "identity": { "email": "alice@example.test" }
            }),
        ),
        (
            "ambiguous_account_material",
            norito::json!({
                "alias": "invalid-ambiguous@universal",
                "account_id": (account_id.to_string()),
                "public_key_hex": public_key_hex,
                "uaid": (uaid.to_string())
            }),
        ),
        (
            "invalid_identity_commitment",
            norito::json!({
                "alias": "invalid-commitment@universal",
                "account_id": (account_id.to_string()),
                "uaid": (uaid.to_string()),
                "identity_commitment_hex": "abcd"
            }),
        ),
    ];

    for (expected_code, body) in cases {
        let (status, payload) = post_account_onboarding_for_validation(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "payload: {payload:?}");
        assert_eq!(
            payload
                .as_object()
                .and_then(|payload| payload.get("error_code"))
                .and_then(norito::json::Value::as_str),
            Some(expected_code)
        );
    }
}

#[tokio::test]
async fn accounts_onboard_publishes_global_manifest_and_binding() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("genesis domain id");
    let authority_kp = checked_onboard_ed25519_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let genesis_domain = Domain::new(genesis_domain_id).build(&authority_id);
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
        .with_name("xor".to_owned())
        .build(&authority_id);
    let mut world = World::with(
        [genesis_domain, domain],
        [authority_account],
        [payment_definition],
    );
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
                dataspace: DataSpaceId::UNIVERSAL,
            }),
        );
        stx.world_mut_for_testing().add_account_permission(
            &authority_id,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        Mint::asset_numeric(
            10_000_u64,
            AssetId::of(payment_asset_definition_id.clone(), authority_id.clone()),
        )
        .execute(&authority_id, &mut stx)
        .expect("mint onboarding payment balance");
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_permissions: Vec::new(),
        fee_sponsor_account: None,
        alias_lease_term_years: 1,
        alias_auto_renew_enabled: true,
        alias_auto_renew_retry_backoff_ms: 86_400_000,
        alias_auto_renew_max_failures: 5,
        alias_auto_renew_subscription_domain: Some(domain_id.clone()),
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
    let user_kp = checked_onboard_ed25519_key_fixture();
    let user_id = AccountId::new(user_kp.public_key().clone());
    let expected_uaid = UniversalAccountId::from_hash(Hash::new(b"accounts-onboard::p2p-user"));
    let body = json_object(vec![
        json_entry("alias", "p2p-user@universal"),
        json_entry("account_id", user_id.to_string()),
        json_entry("uaid", expected_uaid.to_string()),
    ]);
    let body = norito::json::to_json(&body).expect("serialize onboarding request");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/accounts/onboard")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));

    let resp = app.clone().oneshot(req).await.expect("onboarding response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let onboarding_payload: norito::json::Value =
        norito::json::from_slice(&bytes).expect("decode onboarding response");
    let lease_payload = onboarding_payload
        .as_object()
        .and_then(|map| map.get("lease"))
        .and_then(norito::json::Value::as_object)
        .expect("response includes lease block");
    assert_eq!(
        lease_payload
            .get("alias")
            .and_then(norito::json::Value::as_str),
        Some("p2p-user@universal")
    );
    assert_eq!(
        lease_payload
            .get("auto_renew_enabled")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );

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
    assert_eq!(uaid, expected_uaid);
    let bindings = view
        .world()
        .uaid_dataspaces()
        .get(&uaid)
        .expect("UAID bindings present");
    assert!(
        bindings.is_bound_to(DataSpaceId::UNIVERSAL, &user_id),
        "UAID should be bound to the universal dataspace"
    );
    let manifest_set = view
        .world()
        .space_directory_manifests()
        .get(&uaid)
        .expect("manifest registry present");
    let record = manifest_set
        .get(&DataSpaceId::UNIVERSAL)
        .expect("global manifest present");
    assert!(record.is_active(), "global manifest should be active");
    let lease = iroha_core::sns::get_name_record(
        view.world(),
        &view.nexus.dataspace_catalog,
        iroha_core::sns::SnsNamespace::AccountAlias,
        "p2p-user@universal",
        0,
    )
    .expect("alias lease should exist");
    assert_eq!(
        lease.owner, user_id,
        "onboarding must create the alias lease"
    );
    let auto_renew_nft_count = view
        .world()
        .nfts_iter()
        .filter(|nft| {
            let subscription_key =
                Name::from_str(iroha_data_model::subscription::SUBSCRIPTION_METADATA_KEY)
                    .expect("subscription metadata key");
            nft.owned_by == user_id && nft.content.get(&subscription_key).is_some()
        })
        .count();
    assert_eq!(
        auto_renew_nft_count, 1,
        "onboarding should create exactly one alias auto-renew subscription"
    );

    let aliases_req = Request::builder()
        .method("GET")
        .uri(format!("/v1/accounts/{user_id}/aliases"))
        .body(axum::body::Body::empty())
        .unwrap();
    let aliases_resp = app
        .clone()
        .oneshot(aliases_req)
        .await
        .expect("aliases response");
    assert_eq!(aliases_resp.status(), StatusCode::OK);
    let aliases_bytes = axum::body::to_bytes(aliases_resp.into_body(), usize::MAX)
        .await
        .expect("read aliases response");
    let aliases_payload: norito::json::Value =
        norito::json::from_slice(&aliases_bytes).expect("decode aliases response");
    let alias_item = aliases_payload
        .as_object()
        .and_then(|map| map.get("items"))
        .and_then(norito::json::Value::as_array)
        .and_then(|items| items.first())
        .and_then(norito::json::Value::as_object)
        .expect("aliases response includes one item");
    assert_eq!(
        alias_item
            .get("alias")
            .and_then(norito::json::Value::as_str),
        Some("p2p-user@universal")
    );
    assert_eq!(
        alias_item
            .get("auto_renew_enabled")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        alias_item
            .get("subscription_status")
            .and_then(norito::json::Value::as_str),
        Some("active")
    );
}

#[tokio::test]
async fn accounts_onboard_multisig_registers_multisig_account() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("genesis domain id");
    let authority_kp = checked_onboard_ed25519_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let genesis_domain = Domain::new(genesis_domain_id).build(&authority_id);
    let domain = Domain::new(domain_id.clone()).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
        .with_name("xor".to_owned())
        .build(&authority_id);
    let mut world = World::with(
        [genesis_domain, domain],
        [authority_account],
        [payment_definition],
    );
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
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        Mint::asset_numeric(
            10_000_u64,
            AssetId::of(payment_asset_definition_id.clone(), authority_id.clone()),
        )
        .execute(&authority_id, &mut stx)
        .expect("mint onboarding payment balance");
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_permissions: Vec::new(),
        fee_sponsor_account: None,
        alias_lease_term_years: 1,
        alias_auto_renew_enabled: true,
        alias_auto_renew_retry_backoff_ms: 86_400_000,
        alias_auto_renew_max_failures: 5,
        alias_auto_renew_subscription_domain: Some(domain_id.clone()),
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
    let signer_a = AccountId::new(checked_onboard_ed25519_key_fixture().public_key().clone());
    let signer_b = AccountId::new(checked_onboard_ed25519_key_fixture().public_key().clone());
    let body = json_object(vec![
        json_entry("alias", "multisig-company@universal"),
        json_entry("required_signers", 2_u64),
        json_entry(
            "member_account_ids",
            vec![signer_a.to_string(), signer_b.to_string()],
        ),
    ]);
    let body = norito::json::to_json(&body).expect("serialize multisig onboarding request");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/accounts/onboard/multisig")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));

    let resp = app
        .clone()
        .oneshot(req)
        .await
        .expect("multisig onboarding response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let payload: norito::json::Value =
        norito::json::from_slice(&bytes).expect("decode response json");
    let multisig_id = payload
        .as_object()
        .and_then(|map| map.get("account_id"))
        .and_then(norito::json::Value::as_str)
        .expect("response includes account_id")
        .to_string();

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

    let multisig_id = AccountId::parse_encoded(&multisig_id)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("parse multisig account id");
    let view = state.view();
    assert!(
        view.world().account(&multisig_id).is_ok(),
        "multisig account should be registered"
    );
    let lease = iroha_core::sns::get_name_record(
        view.world(),
        &view.nexus.dataspace_catalog,
        iroha_core::sns::SnsNamespace::AccountAlias,
        "multisig-company@universal",
        0,
    )
    .expect("multisig alias lease should exist");
    assert_eq!(
        lease.owner, multisig_id,
        "multisig onboarding must create the alias lease"
    );
}

#[tokio::test]
async fn accounts_onboard_succeeds_without_auto_renew_subscription_domain_when_disabled() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("genesis domain id");
    let authority_kp = checked_onboard_ed25519_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let genesis_domain = Domain::new(genesis_domain_id).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
        .with_name("xor".to_owned())
        .build(&authority_id);
    let mut world = World::with([genesis_domain], [authority_account], [payment_definition]);
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
                dataspace: DataSpaceId::UNIVERSAL,
            }),
        );
        stx.world_mut_for_testing().add_account_permission(
            &authority_id,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        Mint::asset_numeric(
            10_000_u64,
            AssetId::of(payment_asset_definition_id.clone(), authority_id.clone()),
        )
        .execute(&authority_id, &mut stx)
        .expect("mint onboarding payment balance");
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_permissions: Vec::new(),
        fee_sponsor_account: None,
        alias_lease_term_years: 1,
        alias_auto_renew_enabled: false,
        alias_auto_renew_retry_backoff_ms: 86_400_000,
        alias_auto_renew_max_failures: 5,
        alias_auto_renew_subscription_domain: None,
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
    let user_kp = checked_onboard_ed25519_key_fixture();
    let user_id = AccountId::new(user_kp.public_key().clone());
    let expected_uaid = UniversalAccountId::from_hash(Hash::new(b"accounts-onboard::no-renew"));
    let body = json_object(vec![
        json_entry("alias", "no-renew@universal"),
        json_entry("account_id", user_id.to_string()),
        json_entry("uaid", expected_uaid.to_string()),
    ]);
    let body = norito::json::to_json(&body).expect("serialize onboarding request");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/accounts/onboard")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));

    let resp = app.clone().oneshot(req).await.expect("onboarding response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let onboarding_payload: norito::json::Value =
        norito::json::from_slice(&bytes).expect("decode onboarding response");
    let lease_payload = onboarding_payload
        .as_object()
        .and_then(|map| map.get("lease"))
        .and_then(norito::json::Value::as_object)
        .expect("response includes lease block");
    assert_eq!(
        lease_payload
            .get("auto_renew_enabled")
            .and_then(norito::json::Value::as_bool),
        Some(false)
    );

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
        .expect("onboarded account should exist");
    assert_eq!(account_entry.value().uaid().copied(), Some(expected_uaid));
    let lease = iroha_core::sns::get_name_record(
        view.world(),
        &view.nexus.dataspace_catalog,
        iroha_core::sns::SnsNamespace::AccountAlias,
        "no-renew@universal",
        0,
    )
    .expect("alias lease should exist");
    assert_eq!(
        lease.owner, user_id,
        "onboarding must create the alias lease"
    );
    let auto_renew_nft_count = view
        .world()
        .nfts_iter()
        .filter(|nft| {
            let subscription_key =
                Name::from_str(iroha_data_model::subscription::SUBSCRIPTION_METADATA_KEY)
                    .expect("subscription metadata key");
            nft.owned_by == user_id && nft.content.get(&subscription_key).is_some()
        })
        .count();
    assert_eq!(
        auto_renew_nft_count, 0,
        "onboarding should not create an auto-renew subscription when disabled"
    );
}

#[tokio::test]
async fn accounts_onboard_multisig_succeeds_without_auto_renew_subscription_domain_when_disabled() {
    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());

    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("genesis domain id");
    let authority_kp = checked_onboard_ed25519_key_fixture();
    let authority_id = AccountId::new(authority_kp.public_key().clone());
    let genesis_domain = Domain::new(genesis_domain_id).build(&authority_id);
    let authority_account = Account::new(authority_id.clone()).build(&authority_id);
    let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        .parse()
        .expect("payment asset definition id");
    let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
        .with_name("xor".to_owned())
        .build(&authority_id);
    let mut world = World::with([genesis_domain], [authority_account], [payment_definition]);
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
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        Mint::asset_numeric(
            10_000_u64,
            AssetId::of(payment_asset_definition_id.clone(), authority_id.clone()),
        )
        .execute(&authority_id, &mut stx)
        .expect("mint onboarding payment balance");
        stx.apply();
        block.commit().expect("commit should persist permission");
    }

    cfg.torii.onboarding = Some(iroha_config::parameters::actual::ToriiOnboarding {
        authority: authority_id.clone(),
        private_key: ExposedPrivateKey(authority_kp.private_key().clone()),
        allowed_permissions: Vec::new(),
        fee_sponsor_account: None,
        alias_lease_term_years: 1,
        alias_auto_renew_enabled: false,
        alias_auto_renew_retry_backoff_ms: 86_400_000,
        alias_auto_renew_max_failures: 5,
        alias_auto_renew_subscription_domain: None,
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
    let signer_a = AccountId::new(checked_onboard_ed25519_key_fixture().public_key().clone());
    let signer_b = AccountId::new(checked_onboard_ed25519_key_fixture().public_key().clone());
    let body = json_object(vec![
        json_entry("alias", "no-renew-multisig@universal"),
        json_entry("required_signers", 2_u64),
        json_entry(
            "member_account_ids",
            vec![signer_a.to_string(), signer_b.to_string()],
        ),
    ]);
    let body = norito::json::to_json(&body).expect("serialize multisig onboarding request");
    let mut req = Request::builder()
        .method("POST")
        .uri("/v1/accounts/onboard/multisig")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));

    let resp = app
        .clone()
        .oneshot(req)
        .await
        .expect("multisig onboarding response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read response body");
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let payload: norito::json::Value =
        norito::json::from_slice(&bytes).expect("decode response json");
    let multisig_id = payload
        .as_object()
        .and_then(|map| map.get("account_id"))
        .and_then(norito::json::Value::as_str)
        .expect("response includes account_id")
        .to_string();

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

    let multisig_id = AccountId::parse_encoded(&multisig_id)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("parse multisig account id");
    let view = state.view();
    assert!(
        view.world().account(&multisig_id).is_ok(),
        "multisig account should be registered"
    );
    let lease = iroha_core::sns::get_name_record(
        view.world(),
        &view.nexus.dataspace_catalog,
        iroha_core::sns::SnsNamespace::AccountAlias,
        "no-renew-multisig@universal",
        0,
    )
    .expect("multisig alias lease should exist");
    assert_eq!(
        lease.owner, multisig_id,
        "multisig onboarding must create the alias lease"
    );
    let auto_renew_nft_count = view
        .world()
        .nfts_iter()
        .filter(|nft| {
            let subscription_key =
                Name::from_str(iroha_data_model::subscription::SUBSCRIPTION_METADATA_KEY)
                    .expect("subscription metadata key");
            nft.owned_by == multisig_id && nft.content.get(&subscription_key).is_some()
        })
        .count();
    assert_eq!(
        auto_renew_nft_count, 0,
        "multisig onboarding should not create an auto-renew subscription when disabled"
    );
}
