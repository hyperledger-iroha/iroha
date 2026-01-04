//! Router-level tests for Sumeragi execution endpoints.
#![cfg(feature = "telemetry")]

use std::{collections::HashSet, sync::Arc};

use axum::http::{HeaderValue, StatusCode};
use http_body_util::BodyExt;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State as CoreState, World},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::BlockHeader, consensus::ExecutionQcRecord};
use nonzero_ext::nonzero;

fn seed_exec_state() -> (
    Arc<CoreState>,
    HashOf<BlockHeader>,
    iroha_crypto::Hash,
    ExecutionQcRecord,
) {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(CoreState::new_for_testing(World::default(), kura, query));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);

    let subject_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
    let exec_root = Hash::prehashed([0xB2; Hash::LENGTH]);
    let qc = ExecutionQcRecord {
        subject_block_hash: subject_hash,
        parent_state_root: Hash::prehashed([0xB1; Hash::LENGTH]),
        post_state_root: exec_root,
        height: 7,
        view: 3,
        epoch: 2,
        signers_bitmap: vec![0xAA, 0x0F],
        bls_aggregate_signature: vec![0xCC, 0xDD, 0xEE],
    };

    {
        let mut block = state.block(header);
        block
            .transactions
            .insert_block(HashSet::new(), nonzero!(1usize));
        block
            .world
            .exec_roots_mut_for_testing()
            .insert(subject_hash, exec_root);
        block
            .world
            .exec_qcs_mut_for_testing()
            .insert(subject_hash, qc.clone());
        block.commit().expect("commit exec state");
    }

    (state, subject_hash, exec_root, qc)
}

#[tokio::test]
async fn sumeragi_exec_root_endpoint_returns_exec_root() {
    let (state, subject_hash, exec_root, _) = seed_exec_state();
    let hash_hex = format!("{subject_hash}");

    let resp = iroha_torii::handle_v1_sumeragi_exec_root(
        axum::extract::State(state.clone()),
        axum::extract::Path(hash_hex.clone()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        payload
            .get("block_hash")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hash_hex
    );
    assert_eq!(
        payload
            .get("exec_root")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        format!("{exec_root}")
    );
}

#[tokio::test]
async fn sumeragi_exec_root_endpoint_returns_null_for_unknown_hash() {
    let (state, _, _, _) = seed_exec_state();
    let missing_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xF3; Hash::LENGTH]));
    let resp = iroha_torii::handle_v1_sumeragi_exec_root(
        axum::extract::State(state),
        axum::extract::Path(format!("{missing_hash}")),
        None,
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert!(
        payload
            .get("exec_root")
            .is_some_and(norito::json::Value::is_null)
    );
}

#[tokio::test]
async fn sumeragi_exec_qc_endpoint_returns_record() {
    let (state, subject_hash, exec_root, qc) = seed_exec_state();
    let hash_hex = format!("{subject_hash}");

    let resp = iroha_torii::handle_v1_sumeragi_exec_qc(
        axum::extract::State(state.clone()),
        axum::extract::Path(hash_hex.clone()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let payload: norito::json::Value = norito::json::from_slice(&body).unwrap();
    assert_eq!(
        payload
            .get("subject_block_hash")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hash_hex
    );
    assert_eq!(
        payload
            .get("post_state_root")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        format!("{exec_root}")
    );
    assert_eq!(
        payload
            .get("height")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        qc.height
    );
    assert_eq!(
        payload
            .get("view")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        qc.view
    );
    assert_eq!(
        payload
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .unwrap(),
        qc.epoch
    );
    assert_eq!(
        payload
            .get("signers_bitmap")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hex::encode(&qc.signers_bitmap)
    );
    assert_eq!(
        payload
            .get("bls_aggregate_signature")
            .and_then(norito::json::Value::as_str)
            .unwrap(),
        hex::encode(&qc.bls_aggregate_signature)
    );
}

#[tokio::test]
async fn sumeragi_exec_qc_endpoint_supports_norito_payload() {
    let (state, subject_hash, _, qc) = seed_exec_state();
    let hash_hex = format!("{subject_hash}");
    let resp = iroha_torii::handle_v1_sumeragi_exec_qc(
        axum::extract::State(state),
        axum::extract::Path(hash_hex),
        Some(HeaderValue::from_static("application/x-norito")),
    )
    .await
    .unwrap();
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok()),
        Some("application/x-norito")
    );
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    let decoded: Option<ExecutionQcRecord> =
        norito::decode_from_bytes(&bytes).expect("decode ExecutionQC Norito");
    assert_eq!(decoded.as_ref(), Some(&qc));
}
