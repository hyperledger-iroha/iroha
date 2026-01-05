//! Bridge finality endpoint pairs cleanly with the light-client verifier.

use std::{num::NonZeroU64, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use hyper::StatusCode;
use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
    sumeragi::status::{record_commit_certificate, reset_commit_certs_for_tests},
};
use iroha_crypto::{HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    ChainId,
    block::BlockHeader,
    bridge::{BridgeFinalityVerifier, BridgeFinalityVerifyError},
    consensus::{CommitCertificate, VALIDATOR_SET_HASH_VERSION_V1},
    peer::PeerId,
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use tower::ServiceExt as _;

struct CommitCertHistoryGuard;

impl CommitCertHistoryGuard {
    fn new() -> Self {
        reset_commit_certs_for_tests();
        Self
    }
}

impl Drop for CommitCertHistoryGuard {
    fn drop(&mut self) {
        reset_commit_certs_for_tests();
    }
}

#[tokio::test]
async fn bridge_finality_endpoint_roundtrips_into_verifier() {
    let _guard = CommitCertHistoryGuard::new();
    let cfg = test_utils::mk_minimal_root_cfg();
    let chain_id: ChainId = cfg.common.chain.clone();

    let kp = KeyPair::random();
    let peer_id = PeerId::from(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = iroha_data_model::block::builder::BlockBuilder::new(header)
        .build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));

    let validator_set = vec![peer_id.clone()];
    let validator_set_hash = HashOf::new(&validator_set);
    let cert = CommitCertificate {
        height: 1,
        block_hash,
        view: 0,
        epoch: 0,
        validator_set_hash,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        signatures: vec![iroha_data_model::block::BlockSignature::new(
            0,
            SignatureOf::from_hash(kp.private_key(), block_hash),
        )],
    };
    record_commit_certificate(cert);

    let queue_cfg = QueueConfig::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    drop(peers_tx);

    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        KisoHandle::mock(&cfg),
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );
    let app = torii.api_router_for_tests();

    let req = Request::builder()
        .uri("/v1/bridge/finality/1")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("response");
    if !cfg!(feature = "telemetry") {
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        return;
    }
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let proof: iroha_data_model::bridge::BridgeFinalityProof =
        norito::json::from_slice(&body).expect("decode proof");

    let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
        chain_id,
        validator_set_hash,
        VALIDATOR_SET_HASH_VERSION_V1,
        0,
    );

    if let Err(err) = verifier.verify(&proof) {
        panic!("verifier should accept endpoint proof, got {err:?}");
    }

    let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
        proof.chain_id.clone(),
        proof.commit_certificate.validator_set_hash,
        VALIDATOR_SET_HASH_VERSION_V1,
        1,
    );
    let err = verifier.verify(&proof).unwrap_err();
    assert!(matches!(
        err,
        BridgeFinalityVerifyError::UnexpectedEpoch { .. }
    ));
}
