// Actual client/Unix transport within an entered Tokio blocking worker. Signed software
// fixtures exercise transport composition only; they do not qualify hardware or an HTTP route.

async fn worker_fixture_after_ambiguous_sign() -> (Fixture, Vec<u8>) {
    tokio::task::spawn_blocking(|| {
        let mut fixture = fixture();
        let receipt = lose_one_sign(&mut fixture);
        (fixture, receipt)
    })
    .await
    .expect("bounded ambiguous-sign setup joins")
}

#[tokio::test(flavor = "current_thread")]
async fn exact_recovery_opens_its_real_read_session_from_an_entered_blocking_worker() {
    let (fixture, receipt) = worker_fixture_after_ambiguous_sign().await;
    let client = fixture.signer.clone();
    let expected_receipt = receipt.clone();
    let server = tokio::task::spawn_blocking(move || {
        let mut peer = accept_read_session(&fixture);
        let request = read_request(&mut peer, 1, OPERATION_STREAM_TOKEN_RECOVER_V1);
        assert_eq!(request.payload, signing_payload());
        write_response(&mut peer, &request, receipt);
        // The recovery client drops its one-purpose session after the reply.
        assert_eq!(peer.read(&mut [0_u8; 1]).unwrap(), 0);
        fixture
    });
    let recovering = tokio::task::spawn_blocking(move || {
        assert!(tokio::runtime::Handle::try_current().is_ok());
        client.recover(&expected(), &token_body())
    });
    let (reply, mut fixture) = tokio::time::timeout(Duration::from_secs(20), async {
        let reply = recovering
            .await
            .expect("real recovery worker joins")
            .unwrap();
        let fixture = server.await.expect("bounded real peer joins");
        (reply, fixture)
    })
    .await
    .expect("real recovery exchange finishes without nested-runtime panic");
    assert_eq!(reply.bytes(), expected_receipt);
    assert_fenced(&fixture.signer, &fixture.reconnect_listener, 3);
    assert_no_request_bytes(&mut fixture.peer);
}

#[tokio::test(flavor = "current_thread")]
async fn exact_observer_opens_its_real_read_session_from_an_entered_blocking_worker() {
    let (fixture, _) = worker_fixture_after_ambiguous_sign().await;
    let observer = StreamTokenObserverBrokerClient {
        session: Arc::clone(&fixture.signer.session),
        binding: fixture.signer.binding.clone(),
        metadata_digest: fixture.signer.metadata_digest,
        observer_handle: fixture
            .signer
            .binding
            .stream_token_hardware_binding
            .as_ref()
            .unwrap()
            .observer_handle()
            .to_owned(),
    };
    assert_ne!(observer.handle(), fixture.signer.handle());
    let query = stream_token_hardware_test_support::query();
    let expected_query = query.encode_canonical().unwrap();
    let observation = stream_token_hardware_test_support::state_claim(&query)
        .encode_canonical()
        .unwrap();
    let custody = vec![0xa1; SIGNER_CUSTODY_MAX_BYTES_V1];
    let response =
        StreamTokenObserverReplyV1::current(custody.clone(), observation.clone()).unwrap();
    let encoded = encode_stream_token_observer_reply(&query, &response).unwrap();
    let server = tokio::task::spawn_blocking(move || {
        let mut peer = accept_read_session(&fixture);
        let request = read_request(&mut peer, 1, OPERATION_STREAM_TOKEN_OBSERVE_V1);
        assert_eq!(request.payload, expected_query);
        write_response(&mut peer, &request, encoded);
        assert_eq!(peer.read(&mut [0_u8; 1]).unwrap(), 0);
        fixture
    });
    let observing = tokio::task::spawn_blocking(move || {
        assert!(tokio::runtime::Handle::try_current().is_ok());
        observer.observe(&query)
    });
    let (reply, mut fixture) = tokio::time::timeout(Duration::from_secs(20), async {
        let reply = observing
            .await
            .expect("real observer worker joins")
            .unwrap();
        let fixture = server.await.expect("bounded real peer joins");
        (reply, fixture)
    })
    .await
    .expect("real observer exchange finishes without nested-runtime panic");
    let (actual_custody, actual_observation) = reply.current_evidence().unwrap();
    assert_eq!(actual_custody, custody);
    assert_eq!(actual_observation, observation);
    assert!(reply.completed_observation().is_none());
    assert_fenced(&fixture.signer, &fixture.reconnect_listener, 3);
    assert_no_request_bytes(&mut fixture.peer);
}
