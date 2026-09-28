// Actual native finality committee status transport and closed public codecs.

fn native_committee_status_app(
    height: u64,
) -> (
    SharedAppState,
    iroha_data_model::sumeragi::finality::NativeFinalityArtifact,
) {
    use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::sumeragi::finality::{NativeFinalityArtifact, NativeFinalityLimits};
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(iroha_core::state::World::default(), 1))
            .expect("actual genesis");
    while chain.height() < height {
        chain.commit(Vec::new());
    }
    let block = chain.committed(height);
    let artifact = NativeFinalityArtifact::from_block(
        block.block(),
        NativeFinalityLimits {
            block_bytes: 32 * 1024 * 1024,
            journal_bytes: 64 * 1024 * 1024,
            block_count: 256,
            allocated_bytes: 128 * 1024 * 1024,
        },
    )
    .expect("bounded native source");
    let mut app = mk_app_state_for_tests();
    let app_mut = Arc::get_mut(&mut app).expect("unique test app");
    app_mut.state = chain.state().clone();
    app_mut.kura = chain.kura().clone();
    (app, artifact)
}

#[tokio::test]
async fn validator_committee_status_serves_exact_finality_and_closed_queries() {
    use crate::validator_committee::{CommitteeStatusQuery, handler_validator_committee_status};
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    let (app, artifact) = native_committee_status_app(2);
    let network_id = *app.state.network_id_ref();
    let peer = "127.0.0.1:12345".parse().unwrap();
    for accept in ["application/x-norito", "application/json"] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(axum::http::header::ACCEPT, accept.parse().unwrap());
        let response = handler_validator_committee_status(
            State(app.clone()),
            crate::NoritoQuery(CommitteeStatusQuery { target_epoch: None }),
            headers,
            axum::extract::ConnectInfo(peer),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status: ValidatorCommitteeStatusV1 = if accept == "application/x-norito" {
            norito::decode_from_bytes(&body).unwrap()
        } else {
            norito::json::from_slice(&body).unwrap()
        };
        assert_eq!(status.target_epoch, 1);
        assert_eq!(status.latest_finality, artifact);
        assert_eq!(status.network_id, network_id);
        assert_eq!(status.selected, None);
        assert!(status.candidate_keys.is_empty());
        assert_eq!(status.pending_beacon_session, None);
    }
    for query in [
        "unknown=2",
        "target_epoch=-1",
        "target_epoch=2&target_epoch=3",
    ] {
        use axum::extract::FromRequestParts as _;
        let (mut parts, _) = axum::http::Request::builder()
            .uri(format!("/v1/nexus/validator-committee?{query}"))
            .body(())
            .unwrap()
            .into_parts();
        assert!(
            crate::NoritoQuery::<CommitteeStatusQuery>::from_request_parts(&mut parts, &())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn validator_committee_status_rejects_missing_committed_finality() {
    use crate::validator_committee::{CommitteeStatusQuery, handler_validator_committee_status};
    let app = mk_app_state_for_tests();
    let block = make_empty_signed_block(1, None, 1);
    let header = block.header();
    let hash = store_block(&app, block);
    record_committed_block_hash_for_test(&app, header, hash);
    let result = handler_validator_committee_status(
        State(app),
        crate::NoritoQuery(CommitteeStatusQuery {
            target_epoch: Some(1),
        }),
        axum::http::HeaderMap::new(),
        axum::extract::ConnectInfo("127.0.0.1:12345".parse().unwrap()),
    )
    .await;
    assert!(
        result.is_err(),
        "raw committed State without exact durable finality is not status proof"
    );
}

#[tokio::test]
async fn signed_genesis_body_alone_does_not_authorize_committee_status_result() {
    use crate::validator_committee::{CommitteeStatusQuery, handler_validator_committee_status};
    let (app, _) = native_committee_status_app(1);
    assert!(
        handler_validator_committee_status(
            State(app),
            crate::NoritoQuery(CommitteeStatusQuery { target_epoch: None }),
            axum::http::HeaderMap::new(),
            axum::extract::ConnectInfo("127.0.0.1:12345".parse().unwrap()),
        )
        .await
        .is_err()
    );
}

#[test]
fn validator_committee_status_native_source_roundtrips_with_required_fields() {
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    let (app, artifact) = native_committee_status_app(2);
    let status = ValidatorCommitteeStatusV1 {
        network_id: *app.state.network_id_ref(),
        target_epoch: 1,
        latest_finality: artifact,
        selected: None,
        candidate_keys: vec![],
        pending_beacon_session: None,
    };
    let binary = norito::to_bytes(&status).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<ValidatorCommitteeStatusV1>(&binary).unwrap(),
        status
    );
    let json = norito::json::to_vec(&status).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeStatusV1>(&json).unwrap(),
        status
    );
    let mut missing = norito::json::to_value(&status).unwrap();
    missing
        .as_object_mut()
        .unwrap()
        .remove("pending_beacon_session");
    assert!(norito::json::from_value::<ValidatorCommitteeStatusV1>(missing).is_err());
    // The exact selecting-boundary mutation suite now exercises the shared production
    // validator in Core against a genuine signed 14-block native/Pasta prefix.
    // TODO: qualify a positive NPoS selected-state response on the real 4→7→4 network.
}
