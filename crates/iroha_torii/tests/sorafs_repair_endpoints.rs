#![cfg(feature = "app_api")]

//! Contract tests for SoraFS repair worker endpoints.

use std::{num::NonZeroU64, str::FromStr, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use http_body_util::BodyExt as _;
use iroha_core::{
    EventsSender,
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    ChainId,
    account::{Account, AccountId},
    block::BlockHeader,
    domain::{Domain, DomainId},
    isi::{Grant, Register},
    permission::Permission,
    sorafs::capacity::ProviderId,
};
use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, json_entry, json_object};
use norito::json;
use sorafs_manifest::repair::{
    REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_WORKER_SIGNATURE_VERSION_V1,
    RepairCauseV1, RepairEvidenceV1, RepairReportV1, RepairTaskEventV1, RepairTaskRecordV1,
    RepairTaskStateV1, RepairTaskStatusV1, RepairTicketId, RepairWorkerActionV1,
    RepairWorkerSignaturePayloadV1,
};
use tower::ServiceExt as _;

fn repair_report(
    ticket: &str,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    submitted_at_unix: u64,
) -> RepairReportV1 {
    RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId(ticket.to_string()),
        auditor_account: "auditor#sora".to_string(),
        submitted_at_unix,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest,
            provider_id,
            por_history_id: None,
            cause: RepairCauseV1::Manual {
                reason: "test".to_string(),
            },
            evidence_json: None,
            notes: None,
        },
        notes: None,
    }
}

fn sign_worker_action(
    worker_key: &KeyPair,
    worker_id: &AccountId,
    ticket_id: &RepairTicketId,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    idempotency_key: &str,
    action: RepairWorkerActionV1,
) -> SignatureOf<RepairWorkerSignaturePayloadV1> {
    let payload = RepairWorkerSignaturePayloadV1 {
        version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
        ticket_id: ticket_id.clone(),
        manifest_digest,
        provider_id,
        worker_id: worker_id.to_string(),
        idempotency_key: idempotency_key.to_string(),
        action,
    };
    SignatureOf::new(worker_key.private_key(), &payload)
}

fn seed_worker_permission(state: &Arc<State>, worker_id: &AccountId, provider_id: [u8; 32]) {
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height should be non-zero"),
        None,
        None,
        None,
        1_700_000_000,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    Register::domain(Domain::new(worker_id.domain().clone()))
        .execute(worker_id, &mut tx)
        .expect("register domain");
    Register::account(Account::new(worker_id.clone()))
        .execute(worker_id, &mut tx)
        .expect("register account");
    let permission = Permission::from(CanOperateSorafsRepair {
        provider_id: ProviderId::new(provider_id),
    });
    Grant::account_permission(permission, worker_id.clone())
        .execute(worker_id, &mut tx)
        .expect("grant repair worker permission");
    tx.apply();
    block.commit().expect("commit worker setup");
}

fn decode_norito_base64(value: &json::Value) -> Vec<u8> {
    let encoded = value
        .get("norito_base64")
        .and_then(json::Value::as_str)
        .expect("norito_base64 payload");
    BASE64_STANDARD
        .decode(encoded)
        .expect("decode norito base64 payload")
}

fn decode_record_value(value: &json::Value) -> RepairTaskRecordV1 {
    let bytes = decode_norito_base64(value);
    norito::decode_from_bytes(&bytes).expect("decode repair task record")
}

fn decode_event_value(value: &json::Value) -> RepairTaskEventV1 {
    let bytes = decode_norito_base64(value);
    norito::decode_from_bytes(&bytes).expect("decode repair task event")
}

fn decode_record_body(body: &[u8]) -> RepairTaskRecordV1 {
    let value: json::Value = json::from_slice(body).expect("decode record json");
    decode_record_value(&value)
}

fn decode_snapshots_body(body: &[u8]) -> Vec<(RepairTaskRecordV1, Vec<RepairTaskEventV1>)> {
    let value: json::Value = json::from_slice(body).expect("decode snapshots json");
    let entries = value
        .as_array()
        .expect("repair status response should be an array");
    entries
        .iter()
        .map(|entry| {
            let record = decode_record_value(entry);
            let events = entry
                .get("events")
                .and_then(json::Value::as_array)
                .expect("events array");
            let events = events.iter().map(decode_event_value).collect();
            (record, events)
        })
        .collect()
}

fn assert_event_statuses(events: &[RepairTaskEventV1], expected: &[RepairTaskStatusV1]) {
    let statuses: Vec<_> = events.iter().map(|event| event.status).collect();
    assert_eq!(statuses, expected);
}

async fn post_json_with_status(
    app: &Router,
    uri: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("build request"),
        )
        .await
        .expect("post request");
    let status = resp.status();
    let body = resp.into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes()
        .to_vec();
    (status, body)
}

async fn post_json(app: &Router, uri: &str, body: Vec<u8>) -> Vec<u8> {
    let (status, body) = post_json_with_status(app, uri, body).await;
    assert_eq!(status, StatusCode::OK);
    body
}

async fn get_json(app: &Router, uri: &str) -> Vec<u8> {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("get request");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes()
        .to_vec()
}

async fn post_report(app: &Router, report: &RepairReportV1) -> RepairTaskRecordV1 {
    let body = json::to_vec(report).expect("encode repair report");
    let response = post_json(app, "/v1/sorafs/audit/repair/report", body).await;
    decode_record_body(&response)
}

async fn post_value(app: &Router, uri: &str, request: json::Value) -> RepairTaskRecordV1 {
    let body = json::to_vec(&request).expect("encode request");
    let response = post_json(app, uri, body).await;
    decode_record_body(&response)
}

async fn post_claim(app: &Router, request: json::Value) -> RepairTaskRecordV1 {
    post_value(app, "/v1/sorafs/audit/repair/claim", request).await
}

async fn post_heartbeat(app: &Router, request: json::Value) -> RepairTaskRecordV1 {
    post_value(app, "/v1/sorafs/audit/repair/heartbeat", request).await
}

async fn post_complete(app: &Router, request: json::Value) -> RepairTaskRecordV1 {
    post_value(app, "/v1/sorafs/audit/repair/complete", request).await
}

async fn post_fail(app: &Router, request: json::Value) -> RepairTaskRecordV1 {
    post_value(app, "/v1/sorafs/audit/repair/fail", request).await
}

async fn fetch_status(
    app: &Router,
    manifest_hex: &str,
) -> Vec<(RepairTaskRecordV1, Vec<RepairTaskEventV1>)> {
    let response = get_json(
        app,
        &format!("/v1/sorafs/audit/repair/status/{manifest_hex}"),
    )
    .await;
    decode_snapshots_body(&response)
}

#[tokio::test]
async fn sorafs_repair_worker_endpoints_drive_state() {
    let provider_id = [0x22; 32];
    let report_a = repair_report("REP-100", [0x11; 32], provider_id, 1_701_000_000);
    let report_b = repair_report("REP-101", [0x33; 32], provider_id, 1_701_000_100);

    let worker_key = KeyPair::random();
    let domain = DomainId::from_str("sora").expect("domain id");
    let worker_id = AccountId::new(domain, worker_key.public_key().clone());
    let worker_id_str = worker_id.to_string();

    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_repair.enabled = true;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    seed_worker_permission(&state, &worker_id, provider_id);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );
    let app = torii.api_router_for_tests();

    let record = post_report(&app, &report_a).await;
    assert!(matches!(record.state, RepairTaskStateV1::Queued(_)));
    let record = post_report(&app, &report_b).await;
    assert!(matches!(record.state, RepairTaskStateV1::Queued(_)));

    let manifest_a_hex = hex::encode(report_a.evidence.manifest_digest);
    let manifest_b_hex = hex::encode(report_b.evidence.manifest_digest);

    let claim_a_at = report_a.submitted_at_unix + 10;
    let claim_a_key = "claim-100";
    let claim_a_sig = sign_worker_action(
        &worker_key,
        &worker_id,
        &report_a.ticket_id,
        report_a.evidence.manifest_digest,
        provider_id,
        claim_a_key,
        RepairWorkerActionV1::Claim {
            claimed_at_unix: claim_a_at,
        },
    );
    let claim_a_req = json_object(vec![
        json_entry("ticket_id", report_a.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_a_hex.clone()),
        json_entry("worker_id", worker_id_str.clone()),
        json_entry("claimed_at_unix", claim_a_at),
        json_entry("idempotency_key", claim_a_key),
        json_entry("signature", claim_a_sig),
    ]);
    let claim_a_record = post_claim(&app, claim_a_req).await;
    match claim_a_record.state {
        RepairTaskStateV1::InProgress(state) => {
            assert_eq!(state.repair_agent.as_deref(), Some(worker_id_str.as_str()));
        }
        other => panic!("expected in_progress state, got {other:?}"),
    }

    let heartbeat_at = claim_a_at + 30;
    let heartbeat_key = "heartbeat-100";
    let heartbeat_sig = sign_worker_action(
        &worker_key,
        &worker_id,
        &report_a.ticket_id,
        report_a.evidence.manifest_digest,
        provider_id,
        heartbeat_key,
        RepairWorkerActionV1::Heartbeat {
            heartbeat_at_unix: heartbeat_at,
        },
    );
    let heartbeat_req = json_object(vec![
        json_entry("ticket_id", report_a.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_a_hex.clone()),
        json_entry("worker_id", worker_id_str.clone()),
        json_entry("heartbeat_at_unix", heartbeat_at),
        json_entry("idempotency_key", heartbeat_key),
        json_entry("signature", heartbeat_sig),
    ]);
    let heartbeat_record = post_heartbeat(&app, heartbeat_req).await;
    assert!(matches!(
        heartbeat_record.state,
        RepairTaskStateV1::InProgress(_)
    ));

    let completed_at = heartbeat_at + 30;
    let complete_key = "complete-100";
    let resolution_notes = Some("repaired".to_string());
    let complete_sig = sign_worker_action(
        &worker_key,
        &worker_id,
        &report_a.ticket_id,
        report_a.evidence.manifest_digest,
        provider_id,
        complete_key,
        RepairWorkerActionV1::Complete {
            completed_at_unix: completed_at,
            resolution_notes: resolution_notes.clone(),
        },
    );
    let complete_req = json_object(vec![
        json_entry("ticket_id", report_a.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_a_hex.clone()),
        json_entry("worker_id", worker_id_str.clone()),
        json_entry("completed_at_unix", completed_at),
        json_entry("resolution_notes", resolution_notes.clone()),
        json_entry("idempotency_key", complete_key),
        json_entry("signature", complete_sig),
    ]);
    let complete_record = post_complete(&app, complete_req).await;
    assert!(matches!(
        complete_record.state,
        RepairTaskStateV1::Completed(_)
    ));

    let claim_b_at = report_b.submitted_at_unix + 20;
    let claim_b_key = "claim-101";
    let claim_b_sig = sign_worker_action(
        &worker_key,
        &worker_id,
        &report_b.ticket_id,
        report_b.evidence.manifest_digest,
        provider_id,
        claim_b_key,
        RepairWorkerActionV1::Claim {
            claimed_at_unix: claim_b_at,
        },
    );
    let claim_b_req = json_object(vec![
        json_entry("ticket_id", report_b.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_b_hex.clone()),
        json_entry("worker_id", worker_id_str.clone()),
        json_entry("claimed_at_unix", claim_b_at),
        json_entry("idempotency_key", claim_b_key),
        json_entry("signature", claim_b_sig),
    ]);
    let claim_b_record = post_claim(&app, claim_b_req).await;
    assert!(matches!(
        claim_b_record.state,
        RepairTaskStateV1::InProgress(_)
    ));

    let failed_at = claim_b_at + 30;
    let fail_key = "fail-101";
    let fail_reason = "checksum_mismatch";
    let fail_sig = sign_worker_action(
        &worker_key,
        &worker_id,
        &report_b.ticket_id,
        report_b.evidence.manifest_digest,
        provider_id,
        fail_key,
        RepairWorkerActionV1::Fail {
            failed_at_unix: failed_at,
            reason: fail_reason.to_string(),
        },
    );
    let fail_req = json_object(vec![
        json_entry("ticket_id", report_b.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_b_hex.clone()),
        json_entry("worker_id", worker_id_str.clone()),
        json_entry("failed_at_unix", failed_at),
        json_entry("reason", fail_reason),
        json_entry("idempotency_key", fail_key),
        json_entry("signature", fail_sig),
    ]);
    let fail_record = post_fail(&app, fail_req).await;
    assert!(matches!(fail_record.state, RepairTaskStateV1::Failed(_)));

    let snapshots_a = fetch_status(&app, &manifest_a_hex).await;
    assert_eq!(snapshots_a.len(), 1);
    let (record, events) = &snapshots_a[0];
    assert_eq!(record.ticket_id, report_a.ticket_id);
    assert!(matches!(record.state, RepairTaskStateV1::Completed(_)));
    assert_event_statuses(
        events,
        &[
            RepairTaskStatusV1::Queued,
            RepairTaskStatusV1::InProgress,
            RepairTaskStatusV1::Completed,
        ],
    );

    let snapshots_b = fetch_status(&app, &manifest_b_hex).await;
    assert_eq!(snapshots_b.len(), 1);
    let (record, events) = &snapshots_b[0];
    assert_eq!(record.ticket_id, report_b.ticket_id);
    assert!(matches!(record.state, RepairTaskStateV1::Failed(_)));
    assert_event_statuses(
        events,
        &[
            RepairTaskStatusV1::Queued,
            RepairTaskStatusV1::InProgress,
            RepairTaskStatusV1::Failed,
        ],
    );
}

#[tokio::test]
async fn sorafs_repair_worker_rejects_invalid_signature() {
    let provider_id = [0x42; 32];
    let report = repair_report("REP-200", [0x21; 32], provider_id, 1_701_100_000);

    let worker_key = KeyPair::random();
    let bogus_key = KeyPair::random();
    let domain = DomainId::from_str("sora").expect("domain id");
    let worker_id = AccountId::new(domain, worker_key.public_key().clone());
    let worker_id_str = worker_id.to_string();

    let mut cfg = iroha_torii::test_utils::mk_minimal_root_cfg();
    cfg.torii.sorafs_repair.enabled = true;
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    seed_worker_permission(&state, &worker_id, provider_id);

    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender.clone()));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;
    let torii = Torii::new_with_handle(
        ChainId::from("test-chain"),
        kiso,
        cfg.torii.clone(),
        queue,
        events_sender,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );
    let app = torii.api_router_for_tests();

    let record = post_report(&app, &report).await;
    assert!(matches!(record.state, RepairTaskStateV1::Queued(_)));

    let manifest_hex = hex::encode(report.evidence.manifest_digest);
    let claimed_at = report.submitted_at_unix + 10;
    let claim_key = "claim-200";
    let bad_sig = sign_worker_action(
        &bogus_key,
        &worker_id,
        &report.ticket_id,
        report.evidence.manifest_digest,
        provider_id,
        claim_key,
        RepairWorkerActionV1::Claim {
            claimed_at_unix: claimed_at,
        },
    );
    let claim_req = json_object(vec![
        json_entry("ticket_id", report.ticket_id.clone()),
        json_entry("manifest_digest_hex", manifest_hex),
        json_entry("worker_id", worker_id_str),
        json_entry("claimed_at_unix", claimed_at),
        json_entry("idempotency_key", claim_key),
        json_entry("signature", bad_sig),
    ]);
    let body = json::to_vec(&claim_req).expect("encode request");
    let (status, _body) =
        post_json_with_status(&app, "/v1/sorafs/audit/repair/claim", body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
