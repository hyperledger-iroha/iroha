#[cfg(all(feature = "connect", feature = "app_api"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anonymous_ballot_http_reads_open_records_and_admits_the_genuine_envelope() {
    use super::{catalog_get, catalog_post, router::builder::RouterBuilder};
    use axum::{extract::DefaultBodyLimit, middleware::from_fn_with_state};
    use iroha_core::{
        governance::parliament_anonymous::AnonymousParliamentAttemptStateV1,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_data_model::governance::{
        anonymous_ballot::{CanonicalAnonymousBallotEnvelopeV1, ParliamentBallotBoardRecordV1},
        types::AssignmentId,
    };
    use iroha_torii_shared::route_catalog::{RouteCatalog, runtime_governance as routes};

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/governance/anonymous_ballot_http_v1");
    let record = |name: &str| std::fs::read(dir.join(name)).expect("public reducer fixture");
    let attempt_bytes = record("attempt.nrt");
    let board_bytes = record("board.nrt");
    let attempt: AnonymousParliamentAttemptStateV1 =
        norito::decode_from_bytes(&attempt_bytes).unwrap();
    let board: ParliamentBallotBoardRecordV1 = norito::decode_from_bytes(&board_bytes).unwrap();
    let registrations: Vec<(
        AssignmentId,
        iroha_data_model::governance::anonymous_ballot::ParliamentCredentialRecordV1,
    )> = norito::decode_from_bytes(&record("registrations.nrt")).unwrap();
    let envelope =
        CanonicalAnonymousBallotEnvelopeV1::from_canonical_bytes(record("ballot.nrt")).unwrap();
    let ballot = envelope.envelope().unwrap();
    let id = hex::encode(attempt.attempt().id.as_bytes());
    let scope = hex::encode(ballot.ballot.scope.as_bytes());
    assert!(board.opening.is_some() && board.casting_clock.is_none() && board.entries.is_empty());

    // The socket's global authority comes from original signed genesis. The reducer vector
    // uses this very network. Only this component fixture installs its candidate rows.
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
    assert_eq!(chain.network_id(), ballot.network_id);
    assert_eq!(chain.network_id(), board.context.body_context.network_id);
    iroha_core::runtime::activate_additions_for_testing(
        &chain.state().world,
        "iroha.governance.parliament.anonymous.v1",
    )
    .unwrap();
    chain
        .state()
        .world
        .install_parliament_opening_for_testing(attempt, board, registrations);
    let mut app = mk_native_app_state_for_tests();
    let writable = Arc::get_mut(&mut app).expect("isolated service fixture");
    writable.state = Arc::clone(chain.state());
    writable.kura = Arc::clone(chain.kura());
    writable.chain_id = Arc::new(chain.state().chain_id.clone());
    let mut builder = RouterBuilder::new(
        Arc::clone(&app),
        RouteCatalog::new(&[
            routes::GOV_ANONYMOUS_BALLOT_SUBMIT,
            routes::GOV_ANONYMOUS_BALLOT_BOARD_READ,
            routes::GOV_ANONYMOUS_BALLOT_ATTEMPT_READ,
        ]),
        super::compiled_route_features(),
    )
    .unwrap();
    builder.route(
        &routes::GOV_ANONYMOUS_BALLOT_SUBMIT,
        catalog_post(super::handler_gov_anonymous_ballot_submit)
            .layer(DefaultBodyLimit::max(app.anonymous_ballot_max_body_bytes))
            .unauthenticated(),
    );
    builder.route(
        &routes::GOV_ANONYMOUS_BALLOT_BOARD_READ,
        catalog_get(super::handler_gov_anonymous_ballot_board_read),
    );
    builder.route(
        &routes::GOV_ANONYMOUS_BALLOT_ATTEMPT_READ,
        catalog_get(super::handler_gov_anonymous_parliament_attempt_read),
    );
    let (router, manifest) = builder.finish().unwrap();
    let router = router
        .with_state(Arc::clone(&app))
        .layer(from_fn_with_state(
            Arc::clone(&app),
            super::enforce_app_routed_read_http_admission,
        ))
        .layer(from_fn_with_state(
            manifest.route_index(),
            super::attach_matched_route_metadata,
        ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for (suffix, expected) in [
        (
            format!("/v1/gov/anonymous-ballots/attempts/{id}"),
            attempt_bytes,
        ),
        (
            format!("/v1/gov/anonymous-ballots/boards/{scope}"),
            board_bytes,
        ),
    ] {
        let response = client
            .get(format!("{endpoint}{suffix}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.headers()["content-type"],
            crate::utils::NORITO_MIME_TYPE
        );
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            expected.as_slice()
        );
    }
    let submit = format!("{endpoint}{}", routes::GOV_ANONYMOUS_BALLOT_SUBMIT.path());
    for _ in 0..2 {
        let response = client
            .post(&submit)
            .header("content-type", crate::utils::NORITO_MIME_TYPE)
            .body(envelope.as_bytes().to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.bytes().await.unwrap().is_empty());
        assert_eq!(app.queue.active_len(), 1);
    }
    shutdown.send(()).unwrap();
    server.await.unwrap();
}
