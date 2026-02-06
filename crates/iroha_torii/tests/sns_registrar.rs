#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "app_api")]
//! Integration tests exercising the SNS registrar API surface.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    metadata::Metadata,
    peer::PeerId,
    sns::{
        FreezeNameRequestV1, GovernanceHookV1, NameControllerV1, NameRecordV1, NameSelectorV1,
        NameStatus, PaymentProofV1, RegisterNameRequestV1, RegisterNameResponseV1,
        RenewNameRequestV1, TransferNameRequestV1, UpdateControllersRequestV1, fixtures,
    },
};
use iroha_primitives::json::Json;
use iroha_torii::{Torii, test_utils};
use norito::json::Value;
use tokio::sync::broadcast;
use tower::util::ServiceExt as _;

#[path = "fixtures.rs"]
mod torii_fixtures;

fn test_router() -> Router {
    let cfg = test_utils::mk_minimal_root_cfg();
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let local_peer_id = PeerId::new(cfg.common.key_pair.public_key().clone());
    let mut world = World::default();
    torii_fixtures::seed_peer(&mut world, local_peer_id.clone());
    let state = Arc::new(State::new_for_testing(world, kura.clone(), query));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = broadcast::channel(1).0;
    let queue = Arc::new(iroha_core::queue::Queue::from_config(
        queue_cfg,
        events_sender,
    ));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    #[cfg(feature = "telemetry")]
    let telemetry = {
        use iroha_core::telemetry as core_telemetry;
        let metrics = torii_fixtures::shared_metrics();
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
                broadcast::channel(1).0,
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
                broadcast::channel(1).0,
                LiveQueryStore::start_test(),
                kura,
                state,
                da_receipt_signer,
                iroha_torii::OnlinePeersProvider::new(peers_rx),
            )
        }
    };
    torii.api_router_for_tests()
}

fn sample_owner() -> AccountId {
    let domain = "wonderland".parse().expect("domain parses");
    let public_key: PublicKey =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
            .parse()
            .expect("key parses");
    AccountId::new(domain, public_key)
}

fn secondary_owner() -> AccountId {
    let domain = "wonderland".parse().expect("domain parses");
    let public_key: PublicKey =
        "ed0120C70416DC2D60D9AB2F0C6CED829837F1006DDED2DE794E9D5091A60663FA8C11"
            .parse()
            .expect("key parses");
    AccountId::new(domain, public_key)
}

fn controller_for(owner: &AccountId) -> NameControllerV1 {
    let address = AccountAddress::from_account_id(owner).expect("address encode");
    NameControllerV1::account(&address)
}

fn build_payment_with_amount(payer: AccountId, amount: u64) -> PaymentProofV1 {
    PaymentProofV1 {
        asset_id: "xor#sora".to_string(),
        gross_amount: amount,
        net_amount: amount,
        settlement_tx: Json::from("dummy-tx"),
        payer,
        signature: Json::from("dummy-signature"),
    }
}

fn build_payment_with(payer: AccountId) -> PaymentProofV1 {
    build_payment_with_amount(payer, 120)
}

fn build_register_request_with(label: &str, owner: &AccountId) -> RegisterNameRequestV1 {
    let selector = NameSelectorV1::new(0x0001, label).expect("selector");
    RegisterNameRequestV1 {
        selector,
        owner: owner.clone(),
        controllers: vec![controller_for(owner)],
        term_years: 1,
        pricing_class_hint: Some(0),
        payment: build_payment_with(owner.clone()),
        governance: None,
        metadata: Metadata::default(),
    }
}

fn build_register_request() -> RegisterNameRequestV1 {
    let owner = sample_owner();
    build_register_request_with("makoto", &owner)
}

async fn register_name(app: &Router, payload: RegisterNameRequestV1) -> RegisterNameResponseV1 {
    let body = norito::json::to_vec(&payload).expect("serialize register payload");
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .expect("register response");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    norito::json::from_slice(&bytes).expect("register response json")
}

#[tokio::test]
async fn sns_register_and_fetch_round_trip() {
    let app = test_router();
    let payload = build_register_request();
    let register_response = register_name(&app, payload).await;
    assert_eq!(
        register_response.name_record.selector.normalized_label(),
        "makoto"
    );
    assert!(matches!(
        register_response.name_record.status,
        NameStatus::Active
    ));

    let record_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/makoto.sora")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(record_resp.status(), StatusCode::OK);

    let policy_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sns/policies/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(policy_resp.status(), StatusCode::OK);
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sns_renew_transfer_and_freeze_flow() {
    let app = test_router();
    let owner = sample_owner();
    let register_response = register_name(&app, build_register_request_with("flow", &owner)).await;
    let initial_expiry = register_response.name_record.expires_at_ms;

    let renew = RenewNameRequestV1 {
        term_years: 1,
        payment: build_payment_with(owner.clone()),
    };
    let renew_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/flow.sora/renew")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&renew).expect("serialize renew"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(renew_resp.status(), StatusCode::OK);
    let renewed: NameRecordV1 =
        norito::json::from_slice(&renew_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode renew");
    assert!(
        renewed.expires_at_ms > initial_expiry,
        "renewal must extend expiry"
    );

    let new_owner = secondary_owner();
    let transfer = TransferNameRequestV1 {
        new_owner: new_owner.clone(),
        governance: GovernanceHookV1 {
            proposal_id: "proposal-1".into(),
            council_vote_hash: Json::from("council-hash"),
            dao_vote_hash: Json::from("dao-hash"),
            steward_ack: Json::from("steward-ack"),
            guardian_clearance: None,
        },
    };
    let transfer_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/flow.sora/transfer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&transfer).expect("serialize transfer"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(transfer_resp.status(), StatusCode::OK);
    let transferred: NameRecordV1 = norito::json::from_slice(
        &transfer_resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .expect("decode transfer");
    assert_eq!(transferred.owner, new_owner);

    let freeze_req = FreezeNameRequestV1 {
        reason: "court-order".into(),
        until_ms: transferred.expires_at_ms + 1_000,
        guardian_ticket: Json::from("ticket"),
    };
    let freeze_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/flow.sora/freeze")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&freeze_req).expect("serialize freeze"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(freeze_resp.status(), StatusCode::OK);
    let frozen: NameRecordV1 =
        norito::json::from_slice(&freeze_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode freeze");
    assert!(matches!(frozen.status, NameStatus::Frozen(_)));

    let unfreeze = GovernanceHookV1 {
        proposal_id: "proposal-1".into(),
        council_vote_hash: Json::from("council-hash"),
        dao_vote_hash: Json::from("dao-hash"),
        steward_ack: Json::from("steward-ack"),
        guardian_clearance: Some(Json::from("guardian")),
    };
    let unfreeze_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/flow.sora/freeze")
                .method("DELETE")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&unfreeze).expect("serialize unfreeze"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unfreeze_resp.status(), StatusCode::OK);
    let unfrozen: NameRecordV1 = norito::json::from_slice(
        &unfreeze_resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .expect("decode unfreeze");
    assert!(matches!(unfrozen.status, NameStatus::Active));
    assert_eq!(unfrozen.owner, new_owner);
}

#[tokio::test]
async fn sns_transfer_rejected_while_frozen() {
    let app = test_router();
    let owner = sample_owner();
    register_name(&app, build_register_request_with("frozenxfer", &owner)).await;

    let freeze_req = FreezeNameRequestV1 {
        reason: "audit".into(),
        until_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .saturating_add(60_000)
            .try_into()
            .unwrap_or(u64::MAX),
        guardian_ticket: Json::from("ticket"),
    };
    let freeze_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/frozenxfer.sora/freeze")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&freeze_req).expect("serialize freeze"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(freeze_resp.status(), StatusCode::OK);

    let transfer = TransferNameRequestV1 {
        new_owner: secondary_owner(),
        governance: GovernanceHookV1 {
            proposal_id: "proposal-2".into(),
            council_vote_hash: Json::from("council-hash"),
            dao_vote_hash: Json::from("dao-hash"),
            steward_ack: Json::from("steward-ack"),
            guardian_clearance: None,
        },
    };
    let transfer_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/frozenxfer.sora/transfer")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&transfer).expect("serialize transfer"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(transfer_resp.status(), StatusCode::CONFLICT);

    let record_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/frozenxfer.sora")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(record_resp.status(), StatusCode::OK);
    let record: NameRecordV1 =
        norito::json::from_slice(&record_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode record");
    assert!(
        matches!(record.status, NameStatus::Frozen(_)),
        "record should stay frozen after transfer attempt"
    );
}

#[tokio::test]
async fn sns_reserved_label_requires_steward() {
    let app = test_router();
    let reserved_owner = sample_owner();
    let reserved_attempt = build_register_request_with("treasury", &reserved_owner);
    let body = norito::json::to_vec(&reserved_attempt).expect("serialize reserved attempt");
    let conflict_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict_resp.status(), StatusCode::CONFLICT);

    let steward = fixtures::steward_account();
    let success = register_name(&app, build_register_request_with("treasury", &steward)).await;
    assert_eq!(success.name_record.owner, steward);
}

#[tokio::test]
async fn sns_governance_cases_round_trip() {
    let app = test_router();
    let payload = norito::json!({
        "status": "open",
        "note": "abuse report"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/governance/cases")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&payload).expect("serialize case"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let case: Value =
        norito::json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode case");
    let case_id = case
        .get("case_id")
        .and_then(Value::as_str)
        .expect("case id");

    let export_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sns/governance/cases?status=open&limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_resp.status(), StatusCode::OK);
    let export: Value =
        norito::json::from_slice(&export_resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode export");
    let cases = export.as_array().expect("array export");
    assert!(
        !cases.is_empty(),
        "case export should return at least one entry"
    );
    assert_eq!(
        cases[0].get("case_id").and_then(Value::as_str),
        Some(case_id)
    );
}

#[tokio::test]
async fn sns_update_controllers_round_trip() {
    let app = test_router();
    let owner = sample_owner();
    register_name(&app, build_register_request_with("controllers", &owner)).await;
    let secondary = secondary_owner();
    let payload = UpdateControllersRequestV1 {
        controllers: vec![controller_for(&owner), controller_for(&secondary)],
    };
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations/controllers.sora/controllers")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    norito::json::to_vec(&payload).expect("serialize controllers"),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated: NameRecordV1 =
        norito::json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes())
            .expect("decode controllers");
    assert_eq!(updated.controllers, payload.controllers);
}

#[tokio::test]
async fn sns_registration_rejects_insufficient_payment() {
    let app = test_router();
    let owner = sample_owner();
    let mut payload = build_register_request_with("underpay", &owner);
    payload.payment = build_payment_with_amount(owner.clone(), 1);
    let body = norito::json::to_vec(&payload).expect("serialize register payload");
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/sns/registrations")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .expect("register response");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
