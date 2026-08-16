#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_throttles_metrics_requests() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        queue_size: 2,
        ..TelemetryStatus::default()
    };
    let body = encode_status_payload(&status);
    let sumeragi = sample_sumeragi_status_wire();
    let sumeragi_body = encode_sumeragi_status_payload(&sumeragi);
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(sumeragi_body.clone());
    });
    let metrics_mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200)
            .body("queue_size 2\nsumeragi_tx_queue_depth 1");
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let options = StatusStreamOptions::new(Duration::from_millis(10))
        .with_metrics_poll_interval(Some(Duration::from_secs(60)));
    let stream = ManagedStatusStream::spawn_with_options(&handle, "status-peer", client, options);
    let mut receiver = stream.subscribe();
    let first_event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("first snapshot");
    let first_metrics_present = matches!(
        first_event,
        Ok(StatusStreamEvent::Snapshot {
            metrics: Some(_),
            metrics_error: None,
            ..
        })
    );
    assert!(first_metrics_present, "first poll must fetch metrics");
    let second_event = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("second snapshot");
    let second_metrics_present = matches!(
        second_event,
        Ok(StatusStreamEvent::Snapshot {
            metrics: Some(_),
            ..
        })
    );
    assert!(
        second_metrics_present,
        "cached metrics should be propagated between polls"
    );
    assert_eq!(metrics_mock.calls(), 1);
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_can_disable_metrics_polling() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        queue_size: 4,
        ..TelemetryStatus::default()
    };
    let body = encode_status_payload(&status);
    let sumeragi = sample_sumeragi_status_wire();
    let sumeragi_body = encode_sumeragi_status_payload(&sumeragi);
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(sumeragi_body.clone());
    });
    let metrics_mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200).body("queue_size 4");
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let options = StatusStreamOptions::new(Duration::from_millis(10))
        .with_metrics_poll_interval(Some(Duration::ZERO));
    let stream = ManagedStatusStream::spawn_with_options(&handle, "status-peer", client, options);
    let mut receiver = stream.subscribe();
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("snapshot without metrics")
    {
        Ok(StatusStreamEvent::Snapshot {
            metrics,
            metrics_error,
            ..
        }) => {
            assert!(metrics.is_none());
            assert!(metrics_error.is_none());
        }
        other => panic!("expected snapshot without metrics, got {other:?}"),
    }
    assert_eq!(metrics_mock.calls(), 0);
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_status_stream_reports_metrics_failures() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        queue_size: 5,
        ..TelemetryStatus::default()
    };
    let body = encode_status_payload(&status);
    let sumeragi = sample_sumeragi_status_wire();
    let sumeragi_body = encode_sumeragi_status_payload(&sumeragi);
    server.mock(|when, then| {
        when.method(GET).path("/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(sumeragi_body.clone());
    });
    server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(503);
    });
    let client = operator_test_client(server.url("/"));
    let handle = tokio::runtime::Handle::current();
    let stream =
        ManagedStatusStream::spawn(&handle, "status-peer", client, Duration::from_millis(10));
    let mut receiver = stream.subscribe();
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("receive metrics snapshot")
    {
        Ok(StatusStreamEvent::Snapshot {
            snapshot,
            sumeragi,
            metrics,
            metrics_error,
            ..
        }) => {
            assert_eq!(snapshot.status.queue_size, 5);
            assert!(sumeragi.is_some());
            assert!(metrics.is_none(), "metrics snapshot should be absent");
            let error = metrics_error.expect("metrics error should be reported");
            assert_eq!(error.kind, ToriiErrorKind::UnexpectedStatus);
        }
        other => panic!("expected snapshot with metrics error, got {other:?}"),
    }
    stream.abort();
    sleep(Duration::from_millis(10)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn wait_for_ready_retries_until_status_returns_ok() {
    let status = TelemetryStatus {
        queue_size: 9,
        ..TelemetryStatus::default()
    };
    let ok_body = encode_status_payload(&status);
    let Some((addr, shutdown, handle)) =
        spawn_status_stub(vec![(503, Vec::new()), (200, ok_body.clone())])
    else {
        return;
    };
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let options = ReadinessOptions::new(Duration::from_millis(400))
        .with_poll_interval(Duration::from_millis(20));
    let snapshot = client
        .wait_for_ready(options)
        .await
        .expect("readiness snapshot");
    assert_eq!(snapshot.status.queue_size, 9);
    let _ = shutdown.send(());
    let _ = handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn all_managed_peers_genesis_waits_for_the_lagging_peer() {
    let committed = TelemetryStatus {
        blocks: 1,
        ..TelemetryStatus::default()
    };
    let zero_height = TelemetryStatus {
        blocks: 0,
        ..TelemetryStatus::default()
    };
    let Some((ready_addr, ready_shutdown, ready_handle)) =
        spawn_status_stub(vec![(200, encode_status_payload(&committed))])
    else {
        return;
    };
    let Some((lagging_addr, lagging_shutdown, lagging_handle)) = spawn_status_stub(vec![
        (200, encode_status_payload(&zero_height)),
        (200, encode_status_payload(&committed)),
    ]) else {
        let _ = ready_shutdown.send(());
        let _ = ready_handle.join();
        return;
    };
    let peers = vec![
        (
            "peer0".to_owned(),
            ToriiClient::new(format!("http://{ready_addr}")).expect("ready client"),
        ),
        (
            "peer1".to_owned(),
            ToriiClient::new(format!("http://{lagging_addr}")).expect("lagging client"),
        ),
    ];
    let options = ReadinessOptions::new(Duration::from_millis(400))
        .with_poll_interval(Duration::from_millis(20));
    let mut snapshots = wait_for_all_managed_peers_genesis(peers, options)
        .await
        .expect("all managed peers committed genesis");
    snapshots.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    assert_eq!(
        snapshots
            .iter()
            .map(|(alias, snapshot)| (alias.as_str(), snapshot.status.blocks))
            .collect::<Vec<_>>(),
        vec![("peer0", 1), ("peer1", 1)]
    );
    let _ = ready_shutdown.send(());
    let _ = lagging_shutdown.send(());
    let _ = ready_handle.join();
    let _ = lagging_handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn all_managed_peers_genesis_reports_each_lagging_alias_and_endpoint() {
    let committed = TelemetryStatus {
        blocks: 1,
        ..TelemetryStatus::default()
    };
    let zero_height = TelemetryStatus {
        blocks: 0,
        ..TelemetryStatus::default()
    };
    let Some((ready_addr, ready_shutdown, ready_handle)) =
        spawn_status_stub(vec![(200, encode_status_payload(&committed))])
    else {
        return;
    };
    let Some((lagging_addr, lagging_shutdown, lagging_handle)) =
        spawn_status_stub(vec![(200, encode_status_payload(&zero_height))])
    else {
        let _ = ready_shutdown.send(());
        let _ = ready_handle.join();
        return;
    };
    let lagging_url = format!("http://{lagging_addr}");
    let peers = vec![
        (
            "peer-ready".to_owned(),
            ToriiClient::new(format!("http://{ready_addr}")).expect("ready client"),
        ),
        (
            "peer-lagging".to_owned(),
            ToriiClient::new(&lagging_url).expect("lagging client"),
        ),
    ];
    let options = ReadinessOptions::new(Duration::from_millis(120))
        .with_poll_interval(Duration::from_millis(15));
    let error = wait_for_all_managed_peers_genesis(peers, options)
        .await
        .expect_err("lagging managed peer must fail the topology-wide gate");
    let failures = error.failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].alias, "peer-lagging");
    assert_eq!(failures[0].base_url, format!("{lagging_url}/"));
    assert_eq!(failures[0].error.kind, ToriiErrorKind::Timeout);
    let message = error.to_string();
    assert!(message.contains("peer-lagging"), "{message}");
    assert!(message.contains(&lagging_url), "{message}");
    assert!(message.contains("zero committed blocks"), "{message}");
    let _ = ready_shutdown.send(());
    let _ = lagging_shutdown.send(());
    let _ = ready_handle.join();
    let _ = lagging_handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn wait_for_ready_times_out_when_status_never_recovers() {
    let Some((addr, shutdown, handle)) = spawn_status_stub(vec![(503, Vec::new())]) else {
        return;
    };
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let options = ReadinessOptions::new(Duration::from_millis(120))
        .with_poll_interval(Duration::from_millis(15));
    match client.wait_for_ready(options).await {
        Ok(_) => panic!("expected readiness error"),
        Err(ToriiError::UnexpectedStatus { status, .. }) => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(other) => panic!("unexpected readiness error: {other:?}"),
    }
    let _ = shutdown.send(());
    let _ = handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn wait_for_genesis_commit_retries_zero_height_until_committed() {
    let zero_height = TelemetryStatus {
        blocks: 0,
        ..TelemetryStatus::default()
    };
    let committed = TelemetryStatus {
        blocks: 1,
        ..TelemetryStatus::default()
    };
    let Some((addr, shutdown, handle)) = spawn_status_stub(vec![
        (200, encode_status_payload(&zero_height)),
        (200, encode_status_payload(&committed)),
    ]) else {
        return;
    };
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let options = ReadinessOptions::new(Duration::from_millis(400))
        .with_poll_interval(Duration::from_millis(20));
    let snapshot = client
        .wait_for_genesis_commit(options)
        .await
        .expect("committed genesis snapshot");
    assert_eq!(snapshot.status.blocks, 1);
    let _ = shutdown.send(());
    let _ = handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn wait_for_genesis_commit_times_out_at_persistent_zero_height() {
    let zero_height = TelemetryStatus {
        blocks: 0,
        ..TelemetryStatus::default()
    };
    let Some((addr, shutdown, handle)) =
        spawn_status_stub(vec![(200, encode_status_payload(&zero_height))])
    else {
        return;
    };
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let options = ReadinessOptions::new(Duration::from_millis(120))
        .with_poll_interval(Duration::from_millis(15));
    match client.wait_for_genesis_commit(options).await {
        Ok(_) => panic!("expected genesis commitment timeout"),
        Err(ToriiError::Timeout { context }) => {
            assert!(context.contains("zero committed blocks"));
        }
        Err(other) => panic!("unexpected genesis readiness error: {other:?}"),
    }
    let _ = shutdown.send(());
    let _ = handle.join();
}
#[tokio::test(flavor = "current_thread")]
async fn managed_block_stream_emits_alias_on_error() {
    let handle = tokio::runtime::Handle::current();
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = {
        let attempts = attempts.clone();
        move || {
            let attempts = attempts.clone();
            async move {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(ToriiError::UnexpectedStatus {
                        status: StatusCode::SERVICE_UNAVAILABLE,
                        reject_code: None,
                        message: None,
                    })
                } else {
                    let (sender, _) = broadcast::channel(4);
                    let task = tokio::spawn(async {});
                    Ok(WsSubscription {
                        sender,
                        handle: task,
                    })
                }
            }
        }
    };
    let stream = ManagedBlockStream::spawn_with_factory(&handle, "alias-error", factory);
    assert_eq!(stream.alias(), "alias-error");
    let mut receiver = stream.subscribe();
    tokio::task::yield_now().await;
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("decode error event produced")
        .expect("decode error value")
    {
        BlockStreamEvent::DecodeError { .. } => {}
        other => panic!("expected decode error, got {other:?}"),
    }
    match timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("alias notice event produced")
        .expect("alias notice value")
    {
        BlockStreamEvent::Text { text } => {
            assert!(text.contains("alias-error"), "alias missing in {text}");
        }
        other => panic!("expected alias text notice, got {other:?}"),
    }
    sleep(INITIAL_BACKOFF).await;
    tokio::task::yield_now().await;
    stream.abort();
    sleep(Duration::from_millis(20)).await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn managed_block_stream_abort_stops_worker() {
    let handle = tokio::runtime::Handle::current();
    let stream = ManagedBlockStream::spawn_with_factory(&handle, "abort-peer", || async {
        let (sender, _) = broadcast::channel(1);
        let task = tokio::spawn(async {});
        Ok(WsSubscription {
            sender,
            handle: task,
        })
    });
    assert_eq!(stream.alias(), "abort-peer");
    stream.abort();
    tokio::task::yield_now().await;
    assert!(stream.is_finished());
}
#[tokio::test(flavor = "current_thread")]
async fn submit_signed_transaction_posts_versioned_bytes() {
    let listener = match handle_bind_result(
        TcpListener::bind("127.0.0.1:0").await,
        "bind transaction listener",
    ) {
        Some(listener) => listener,
        None => return,
    };
    let addr = listener.local_addr().expect("listener address");
    let recorded = Arc::new(AsyncMutex::new(None::<(String, Vec<u8>)>));
    let server_task = {
        let recorded = Arc::clone(&recorded);
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut header_bytes = Vec::new();
                loop {
                    match socket.read_u8().await {
                        Ok(byte) => {
                            header_bytes.push(byte);
                            if header_bytes.ends_with(b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => return,
                    }
                }
                let header_str = String::from_utf8_lossy(&header_bytes);
                let request_line = header_str.lines().next().unwrap_or_default().to_string();
                let content_length = header_str
                    .lines()
                    .find_map(|line| {
                        let mut parts = line.splitn(2, ':');
                        let name = parts.next()?.trim().to_ascii_lowercase();
                        if name == "content-length" {
                            parts.next()?.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; content_length];
                if socket.read_exact(&mut body).await.is_err() {
                    return;
                }
                {
                    let mut guard = recorded.lock().await;
                    *guard = Some((request_line, body));
                }
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                    .await;
            }
        })
    };
    let keypair = KeyPair::random();
    let tx = TransactionBuilder::new(
        test_network_id(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(iter::empty::<InstructionBox>())
    .sign(keypair.private_key());
    let versioned = tx.encode_versioned();
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    client
        .submit_signed_transaction(&tx)
        .await
        .expect("submit transaction");
    server_task.await.expect("server task finished");
    let guard = recorded.lock().await;
    let (request_line, body) = guard.clone().expect("captured request");
    assert!(
        request_line.starts_with("POST /v1/pipeline/transactions"),
        "unexpected request line: {request_line}"
    );
    assert_eq!(body, versioned);
}
#[tokio::test(flavor = "current_thread")]
async fn execute_query_decodes_response() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let query_output = QueryOutput::new(
        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
        0,
        None,
    );
    let encoded = norito::to_bytes(&query_output).expect("encode query output");
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(200).body(encoded.clone());
    });
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let client = ToriiClient::new_for_network(server.url("/"), test_network_id()).expect("client");
    let signed_query = client
        .sign_query(
            QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            )),
            account_id,
            &keypair,
        )
        .expect("sign query");
    let response = client
        .execute_query(&signed_query)
        .await
        .expect("decode query output");
    mock.assert();
    let (_, remaining_items, continue_cursor) = response.into_parts();
    assert_eq!(remaining_items, 0);
    assert!(continue_cursor.is_none());
}
#[test]
fn sign_query_requires_an_exact_network_identity() {
    let client = ToriiClient::new("http://127.0.0.1:8080").expect("client");
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let error = match client.sign_query(
        QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
            FindExecutorDataModel,
        )),
        account_id,
        &keypair,
    ) {
        Ok(_) => panic!("a general Torii client must not invent signed-query lineage"),
        Err(error) => error,
    };
    assert!(matches!(error, ToriiError::SignedQueryContext(_)));
}
#[test]
fn sign_query_binds_lineage_freshness_and_one_shot_nonce() {
    let network_id = test_network_id();
    let client = ToriiClient::new_for_network("http://127.0.0.1:8080", network_id)
        .expect("network-bound client");
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let signed = client
        .sign_query(
            QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            )),
            account_id,
            &keypair,
        )
        .expect("sign query");
    assert_eq!(client.network_id(), Some(network_id));
    assert_eq!(signed.payload.network_id, network_id);
    assert!(signed.payload.creation_time_ms > 0);
    assert_eq!(signed.payload.time_to_live_ms.get(), 100_000);
    assert_ne!(signed.payload.nonce, [0_u8; 32]);
    signed
        .verify_signature()
        .expect("signature covers every replay-context field");
}
#[tokio::test(flavor = "current_thread")]
async fn execute_query_returns_unexpected_status() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(500);
    });
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let client = ToriiClient::new_for_network(server.url("/"), test_network_id()).expect("client");
    let signed_query = client
        .sign_query(
            QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            )),
            account_id,
            &keypair,
        )
        .expect("sign query");
    let err = client
        .execute_query(&signed_query)
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn execute_query_reports_decode_error() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/query");
        then.status(200).body(vec![0, 1, 2, 3]);
    });
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let client = ToriiClient::new_for_network(server.url("/"), test_network_id()).expect("client");
    let signed_query = client
        .sign_query(
            QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            )),
            account_id,
            &keypair,
        )
        .expect("sign query");
    let err = client
        .execute_query(&signed_query)
        .await
        .expect_err("malformed payload should error");
    mock.assert();
    matches!(err, ToriiError::Decode(_));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_decodes_norito_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        build: Default::default(),
        peers: 2,
        blocks: 5,
        blocks_non_empty: 3,
        commit_time_ms: 42,
        da_reschedule_total: 0,
        txs_approved: 7,
        txs_rejected: 1,
        last_rejection_at_ms: None,
        txs_rejected_recent_5m: 0,
        uptime: Uptime(Duration::from_secs(123)),
        view_changes: 0,
        queue_size: 4,
        ..TelemetryStatus::default()
    };
    let encoded = norito::codec::encode_adaptive(&status);
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(encoded.clone());
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let decoded = client.fetch_status().await.expect("status");
    mock.assert();
    assert_eq!(decoded.blocks, status.blocks);
    assert_eq!(decoded.queue_size, status.queue_size);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_decodes_framed_norito_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = TelemetryStatus {
        build: Default::default(),
        peers: 2,
        blocks: 5,
        blocks_non_empty: 3,
        commit_time_ms: 42,
        da_reschedule_total: 0,
        txs_approved: 7,
        txs_rejected: 1,
        last_rejection_at_ms: None,
        txs_rejected_recent_5m: 0,
        uptime: Uptime(Duration::from_secs(123)),
        view_changes: 0,
        queue_size: 4,
        ..TelemetryStatus::default()
    };
    let encoded = norito::to_bytes(&status).expect("encode framed status");
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(encoded.clone());
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let decoded = client.fetch_status().await.expect("status");
    mock.assert();
    assert_eq!(decoded.blocks, status.blocks);
    assert_eq!(decoded.queue_size, status.queue_size);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_snapshot_tracks_metrics_across_calls() {
    let listener = match handle_bind_result(
        TcpListener::bind("127.0.0.1:0").await,
        "bind status listener",
    ) {
        Some(listener) => listener,
        None => return,
    };
    let addr = listener.local_addr().expect("listener address");
    let initial = TelemetryStatus {
        build: Default::default(),
        peers: 2,
        blocks: 10,
        blocks_non_empty: 8,
        commit_time_ms: 45,
        da_reschedule_total: 2,
        txs_approved: 5,
        txs_rejected: 1,
        last_rejection_at_ms: None,
        txs_rejected_recent_5m: 0,
        uptime: Uptime(Duration::from_secs(5)),
        view_changes: 0,
        queue_size: 4,
        ..TelemetryStatus::default()
    };
    let updated = TelemetryStatus {
        build: Default::default(),
        peers: 3,
        blocks: 11,
        blocks_non_empty: 9,
        commit_time_ms: 120,
        da_reschedule_total: 5,
        txs_approved: 9,
        txs_rejected: 3,
        last_rejection_at_ms: Some(7_000),
        txs_rejected_recent_5m: 3,
        uptime: Uptime(Duration::from_secs(7)),
        view_changes: 2,
        queue_size: 9,
        ..TelemetryStatus::default()
    };
    let responses = vec![
        norito::codec::encode_adaptive(&initial),
        norito::codec::encode_adaptive(&updated),
    ];
    let server_task = tokio::spawn(async move {
        for payload in responses {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut header_bytes = Vec::new();
                loop {
                    match socket.read_u8().await {
                        Ok(byte) => {
                            header_bytes.push(byte);
                            if header_bytes.ends_with(b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => return,
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {NORITO_MIME_TYPE}\r\nConnection: close\r\n\r\n",
                    payload.len()
                )
                .into_bytes();
                if socket.write_all(&response).await.is_err() {
                    return;
                }
                if socket.write_all(&payload).await.is_err() {
                    return;
                }
            }
        }
    });
    let client = ToriiClient::new(format!("http://{addr}")).expect("client");
    let first = client
        .fetch_status_snapshot()
        .await
        .expect("first snapshot");
    assert_eq!(first.status.queue_size, initial.queue_size);
    assert_eq!(first.metrics.queue_delta, 0);
    assert_eq!(first.metrics.da_reschedule_delta, 0);
    assert_eq!(first.metrics.tx_approved_delta, 0);
    assert_eq!(first.metrics.tx_rejected_delta, 0);
    assert_eq!(first.metrics.view_change_delta, 0);
    let second = client
        .fetch_status_snapshot()
        .await
        .expect("second snapshot");
    assert_eq!(second.status.queue_size, updated.queue_size);
    assert_eq!(
        second.metrics.queue_delta,
        updated.queue_size as i64 - initial.queue_size as i64
    );
    assert_eq!(
        second.metrics.da_reschedule_delta,
        updated.da_reschedule_total - initial.da_reschedule_total
    );
    assert_eq!(
        second.metrics.tx_approved_delta,
        updated.txs_approved - initial.txs_approved
    );
    assert_eq!(
        second.metrics.tx_rejected_delta,
        updated.txs_rejected - initial.txs_rejected
    );
    assert_eq!(
        second.metrics.view_change_delta,
        updated.view_changes - initial.view_changes
    );
    server_task.await.expect("server task finished");
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_reports_decode_error_for_invalid_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200).body(vec![0, 1, 2, 3, 4]);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_status()
        .await
        .expect_err("invalid payload should fail");
    mock.assert();
    matches!(err, ToriiError::Decode(_));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_sumeragi_status_decodes_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let status = sample_sumeragi_status_wire();
    let mut encoded = Vec::new();
    norito::core::to_bytes_in(&status, &mut encoded).expect("encode framed status");
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sumeragi/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(encoded.clone());
    });
    let client = operator_test_client(server.url("/"));
    let decoded = client
        .fetch_sumeragi_status()
        .await
        .expect("status payload");
    mock.assert();
    assert_eq!(decoded, status);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_sumeragi_status_rejects_semantically_invalid_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mut status = sample_sumeragi_status_wire();
    status.phase = iroha_data_model::block::consensus_v2::SumeragiV2StatusPhase::Commit;
    assert!(status.validate().is_err(), "fixture must be invalid");
    let mut encoded = Vec::new();
    norito::core::to_bytes_in(&status, &mut encoded).expect("encode framed status");
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/sumeragi/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(encoded.clone());
    });
    let client = operator_test_client(server.url("/"));
    let err = client
        .fetch_sumeragi_status()
        .await
        .expect_err("invalid status invariants must fail");
    mock.assert();
    assert!(matches!(err, ToriiError::Decode(_)));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_sumeragi_status_reports_unexpected_status() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/sumeragi/status");
        then.status(503);
    });
    let client = operator_test_client(server.url("/"));
    let err = client
        .fetch_sumeragi_status()
        .await
        .expect_err("non-success status should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_status_returns_unexpected_status_on_non_success() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/status")
            .header("accept", NORITO_MIME_TYPE);
        then.status(502);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_status()
        .await
        .expect_err("non-success response should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::BAD_GATEWAY);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_configuration_reports_decode_error() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/configuration");
        then.status(200)
            .header("content-type", "application/json")
            .body(&b"{not-json}"[..]);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_configuration()
        .await
        .expect_err("malformed json should error");
    mock.assert();
    matches!(err, ToriiError::Decode(_));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_metrics_returns_unexpected_status_on_error() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(503);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_metrics()
        .await
        .expect_err("non-success response should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_configuration_returns_json() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/configuration");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"chain_id":"mochi"}"#);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let value = client.fetch_configuration().await.expect("config");
    mock.assert();
    assert_eq!(value["chain_id"].as_str(), Some("mochi"));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_lane_lifecycle_status_returns_valid_norito() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let expected = lifecycle_status(true);
    let body = norito::to_bytes(&expected).expect("encode lifecycle status");
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/nexus/lifecycle")
            .header("accept", NORITO_MIME_TYPE);
        then.status(200)
            .header("content-type", NORITO_MIME_TYPE)
            .body(body);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let actual = client
        .fetch_lane_lifecycle_status()
        .await
        .expect("lifecycle status");
    mock.assert();
    assert_eq!(actual, expected);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_lane_lifecycle_status_reports_unexpected_status() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/nexus/lifecycle");
        then.status(503);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let err = client
        .fetch_lane_lifecycle_status()
        .await
        .expect_err("non-success response should error");
    mock.assert();
    match err {
        ToriiError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected UnexpectedStatus, got {other:?}"),
    }
}
#[test]
fn lane_lifecycle_transaction_binds_status_and_requires_permission() {
    let status = lifecycle_status(true);
    let network_id = test_network_id();
    let alice = crate::compose::development_signing_authorities()
        .iter()
        .find(|signer| signer.allows_permission(InstructionPermission::SetParameters))
        .expect("development CanSetParameters signer");
    let transaction = build_lane_lifecycle_transaction(
        network_id,
        alice,
        &status,
        LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![iroha_data_model::nexus::LaneId::SINGLE],
        },
    )
    .expect("build signed lifecycle transaction");
    transaction
        .verify_signature()
        .expect("lifecycle signature verifies");
    assert_eq!(transaction.network_id(), Some(&network_id));
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        transaction.instructions()
    else {
        panic!("expected instruction executable");
    };
    let set_parameter = instructions[0]
        .as_any()
        .downcast_ref::<SetParameter>()
        .expect("SetParameter instruction");
    let Parameter::Custom(custom) = set_parameter.inner() else {
        panic!("expected custom lifecycle parameter");
    };
    let payload = LaneLifecycleParameterV1::from_custom_parameter(custom)
        .expect("decode lifecycle parameter")
        .expect("matching lifecycle parameter");
    assert_eq!(payload.expected_catalog_hash, status.catalog_hash);
    let bob = crate::compose::development_signing_authorities()
        .iter()
        .find(|signer| !signer.allows_permission(InstructionPermission::SetParameters))
        .expect("restricted development signer");
    let error = build_lane_lifecycle_transaction(
        test_network_id(),
        bob,
        &status,
        LaneLifecyclePlan::default(),
    )
    .expect_err("signer without CanSetParameters must be rejected locally");
    assert!(error.to_string().contains("CanSetParameters"));
}
#[test]
fn lane_lifecycle_transaction_rejects_forged_status_hash() {
    let mut status = lifecycle_status(true);
    status.catalog_hash = Hash::prehashed([0xCC; Hash::LENGTH]);
    let signer = crate::compose::development_signing_authorities()
        .first()
        .expect("development signer");
    let error = build_lane_lifecycle_transaction(
        test_network_id(),
        signer,
        &status,
        LaneLifecyclePlan::default(),
    )
    .expect_err("forged status hash must fail closed");
    assert!(error.to_string().contains("catalog hash mismatch"));
}
#[tokio::test(flavor = "current_thread")]
async fn lane_lifecycle_rejects_network_id_different_from_client() {
    let configured_network_id = test_network_id();
    let supplied_network_id = NetworkId::from_genesis_hash(HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::prehashed([0x42; Hash::LENGTH])
    ));
    let client =
        ToriiClient::new_for_network("http://127.0.0.1:9", configured_network_id).expect("client");
    let signer = crate::compose::development_signing_authorities()
        .first()
        .expect("development signer");
    let error = client
        .apply_lane_lifecycle(supplied_network_id, signer, LaneLifecyclePlan::default())
        .await
        .expect_err("mismatched exact network identity must fail before I/O");
    assert!(matches!(error, ToriiError::SignedQueryContext(_)));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_metrics_returns_text() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let metrics_body = "queue_size 3";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200).body(metrics_body);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let body = client.fetch_metrics().await.expect("metrics");
    mock.assert();
    assert_eq!(body, metrics_body);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_metrics_snapshot_parses_values() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let metrics_body = "\
queue_size 4
view_changes 2
sumeragi_tx_queue_depth 5
state_tiered_hot_entries 10
";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/metrics");
        then.status(200).body(metrics_body);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let snapshot = client
        .fetch_metrics_snapshot()
        .await
        .expect("metrics snapshot");
    mock.assert();
    assert_eq!(snapshot.queue_size, Some(4.0));
    assert_eq!(snapshot.view_changes, Some(2.0));
    assert_eq!(snapshot.sumeragi_tx_queue_depth, Some(5.0));
    assert_eq!(snapshot.state_tiered_hot_entries, Some(10.0));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_block_parses_payload() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = r#"{
  "hash":"aa00bb11",
  "height":7,
  "created_at":"2026-01-01T00:00:00Z",
  "prev_block_hash":"cc22dd33",
  "transactions_hash":"ee44ff55",
  "transactions_rejected":1,
  "transactions_total":2
}"#;
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/blocks/7");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let record = client
        .fetch_block(7)
        .await
        .expect("request")
        .expect("record");
    mock.assert();
    assert_eq!(record.height, 7);
    assert_eq!(record.prev_block_hash.as_deref(), Some("cc22dd33"));
    assert_eq!(record.transactions_total, 2);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_block_returns_none_on_404() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let mock = server.mock(|when, then| {
        when.method(GET).path("/v1/blocks/99");
        then.status(404);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let record = client.fetch_block(99).await.expect("request");
    mock.assert();
    assert!(record.is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_blocks_page_supports_query_params() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = r#"{
  "pagination":{"page":1,"per_page":2,"total_pages":2,"total_items":4},
  "items":[
{
  "hash":"aa00bb11",
  "height":5,
  "created_at":"2026-01-01T00:00:00Z",
  "transactions_rejected":0,
  "transactions_total":1
},
{
  "block_hash":"cc22dd33",
  "height":"6",
  "createdAt":"2026-01-01T01:00:00Z",
  "transactionsRejected":"1",
  "transactionsTotal":"3"
}
  ]
}"#;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/blocks")
            .query_param("offset_height", "5")
            .query_param("limit", "2");
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_blocks_page(ExplorerBlocksQuery {
            offset_height: Some(5),
            limit: Some(2),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.pagination.per_page, 2);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].hash, "aa00bb11");
    assert_eq!(page.items[1].hash, "cc22dd33");
    assert_eq!(page.items[1].transactions_rejected, 1);
}
#[test]
fn metrics_parser_ignores_comments_and_labels() {
    let body = r#"
# HELP queue_size number of txs
queue_size 3
view_changes 1
accounts{domain="wonderland"} 10
sumeragi_tx_queue_capacity 64
state_tiered_cold_entries 2
"#;
    let now = Instant::now();
    let snapshot = ToriiMetricsSnapshot::from_prometheus(now, body);
    assert_eq!(snapshot.queue_size, Some(3.0));
    assert_eq!(snapshot.view_changes, Some(1.0));
    assert_eq!(snapshot.sumeragi_tx_queue_capacity, Some(64.0));
    assert_eq!(snapshot.state_tiered_cold_entries, Some(2.0));
    assert!(snapshot.state_tiered_hot_entries.is_none());
}
#[test]
fn status_metrics_report_activity_deltas() {
    let previous = TelemetryStatus {
        queue_size: 4,
        txs_approved: 1,
        txs_rejected: 0,
        da_reschedule_total: 2,
        view_changes: 3,
        blocks: 10,
        blocks_non_empty: 9,
        ..TelemetryStatus::default()
    };
    let current = TelemetryStatus {
        commit_time_ms: 25,
        queue_size: 7,
        txs_approved: 4,
        txs_rejected: 2,
        da_reschedule_total: 5,
        view_changes: 4,
        blocks: 12,
        blocks_non_empty: 11,
        ..TelemetryStatus::default()
    };
    let metrics = StatusMetrics::from_samples(Some(&previous), &current);
    assert_eq!(metrics.commit_latency_ms, 25);
    assert_eq!(metrics.queue_delta, 3);
    assert_eq!(metrics.tx_approved_delta, 3);
    assert_eq!(metrics.tx_rejected_delta, 2);
    assert_eq!(metrics.da_reschedule_delta, 3);
    assert_eq!(metrics.view_change_delta, 1);
    assert_eq!(metrics.block_delta, 2);
    assert_eq!(metrics.blocks_non_empty_delta, 2);
    assert_eq!(metrics.sample_interval_ms, 0);
    assert!(metrics.has_activity());
}
#[test]
fn status_metrics_report_idle_when_snapshots_match() {
    let snapshot = TelemetryStatus {
        queue_size: 2,
        txs_approved: 5,
        txs_rejected: 1,
        da_reschedule_total: 1,
        view_changes: 0,
        ..TelemetryStatus::default()
    };
    let metrics = StatusMetrics::from_samples(Some(&snapshot), &snapshot);
    assert_eq!(metrics.commit_latency_ms, snapshot.commit_time_ms);
    assert_eq!(metrics.queue_delta, 0);
    assert_eq!(metrics.tx_approved_delta, 0);
    assert_eq!(metrics.tx_rejected_delta, 0);
    assert_eq!(metrics.da_reschedule_delta, 0);
    assert_eq!(metrics.view_change_delta, 0);
    assert_eq!(metrics.block_delta, 0);
    assert_eq!(metrics.blocks_non_empty_delta, 0);
    assert_eq!(metrics.sample_interval_ms, 0);
    assert!(!metrics.has_activity());
}
#[test]
fn status_state_records_sample_interval_and_block_delta() {
    let mut state = StatusState::default();
    let now = Instant::now();
    let first = TelemetryStatus {
        blocks: 1,
        ..TelemetryStatus::default()
    };
    let first_metrics = state.record(now, &first);
    assert_eq!(first_metrics.block_delta, 0);
    assert_eq!(first_metrics.sample_interval_ms, 0);
    let later = now + Duration::from_millis(150);
    let second = TelemetryStatus {
        blocks: 3,
        blocks_non_empty: 2,
        ..TelemetryStatus::default()
    };
    let second_metrics = state.record(later, &second);
    assert_eq!(second_metrics.block_delta, 2);
    assert_eq!(second_metrics.blocks_non_empty_delta, 2);
    assert_eq!(second_metrics.sample_interval_ms, 150);
}
#[tokio::test(flavor = "current_thread")]
async fn status_monitor_streams_snapshots() {
    let counter = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
        let counter = Arc::clone(&counter);
        move || {
            let counter = Arc::clone(&counter);
            async move {
                let value = counter.fetch_add(1, Ordering::SeqCst) as u64 + 1;
                let status = TelemetryStatus {
                    queue_size: value,
                    ..TelemetryStatus::default()
                };
                let metrics = StatusMetrics::from_samples(None, &status);
                Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("monitor emitted value")
        .expect("channel open");
    let state = receiver.borrow().clone();
    assert!(state.has_snapshot(), "monitor should publish snapshots");
    assert!(
        state.last_success_at.is_some(),
        "monitor should record last success timestamp"
    );
    assert_eq!(
        state
            .last_snapshot
            .as_ref()
            .expect("snapshot available")
            .status
            .queue_size,
        1
    );
    assert!(state.last_error.is_none());
    monitor.stop();
}
#[tokio::test(flavor = "current_thread")]
async fn status_monitor_records_errors_and_clears_on_success() {
    let step = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
        let step = Arc::clone(&step);
        move || {
            let step = Arc::clone(&step);
            async move {
                let attempt = step.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    Err(ToriiError::UnexpectedStatus {
                        status: StatusCode::SERVICE_UNAVAILABLE,
                        reject_code: None,
                        message: None,
                    })
                } else {
                    let status = TelemetryStatus {
                        queue_size: attempt as u64,
                        ..TelemetryStatus::default()
                    };
                    let metrics = StatusMetrics::from_samples(None, &status);
                    Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
                }
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("first update")
        .expect("channel open");
    let first = receiver.borrow().clone();
    assert!(first.last_error.is_some());
    assert!(!first.has_snapshot());
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("second update")
        .expect("channel open");
    let second = receiver.borrow().clone();
    assert!(second.last_error.is_none());
    assert_eq!(
        second
            .last_snapshot
            .as_ref()
            .expect("snapshot available")
            .status
            .queue_size,
        1
    );
    monitor.stop();
}
#[tokio::test(flavor = "current_thread")]
async fn status_monitor_tracks_last_success_and_exposes_age() {
    let step = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiStatusMonitor::spawn(Duration::from_millis(5), {
        let step = Arc::clone(&step);
        move || {
            let step = Arc::clone(&step);
            async move {
                let attempt = step.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    let status = TelemetryStatus {
                        queue_size: 7,
                        ..TelemetryStatus::default()
                    };
                    let metrics = StatusMetrics::from_samples(None, &status);
                    Ok(ToriiStatusSnapshot::new(Instant::now(), status, metrics))
                } else {
                    Err(ToriiError::UnexpectedStatus {
                        status: StatusCode::BAD_GATEWAY,
                        reject_code: None,
                        message: None,
                    })
                }
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("first update")
        .expect("channel open");
    let first = receiver.borrow().clone();
    let first_timestamp = first
        .last_success_at
        .expect("first poll should record success timestamp");
    let first_age = first
        .last_success_age()
        .expect("first poll should expose success age");
    assert!(
        first_age >= Duration::ZERO,
        "age should be present for successful poll"
    );
    assert!(first.last_error.is_none(), "first poll should succeed");
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("second update")
        .expect("channel open");
    let second = receiver.borrow().clone();
    assert_eq!(
        second.last_success_at,
        Some(first_timestamp),
        "error should not clear last success timestamp"
    );
    assert_eq!(
        second.consecutive_failures, 1,
        "first error should increment failure counter"
    );
    assert!(
        second.last_success_age().is_some(),
        "age should remain available after errors"
    );
    assert!(second.last_error.is_some(), "second poll should fail");
    monitor.stop();
}
#[tokio::test(flavor = "current_thread")]
async fn metrics_monitor_streams_snapshots() {
    let counter = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
        let counter = Arc::clone(&counter);
        move || {
            let counter = Arc::clone(&counter);
            async move {
                let value = counter.fetch_add(1, Ordering::SeqCst) as f64 + 1.0;
                let snapshot = ToriiMetricsSnapshot::from_prometheus(
                    Instant::now(),
                    &format!("queue_size {value}\n"),
                );
                Ok(snapshot)
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("monitor emitted value")
        .expect("channel open");
    let state = receiver.borrow().clone();
    assert!(state.has_snapshot(), "monitor should publish snapshots");
    assert!(
        state.last_success_at.is_some(),
        "monitor should record last success timestamp"
    );
    assert!(
        matches!(
            state
                .last_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.queue_size),
            Some(value) if (value - 1.0).abs() < f64::EPSILON
        ),
        "monitor should retain queue gauge"
    );
    assert!(state.last_error.is_none());
    monitor.stop();
}
#[tokio::test(flavor = "current_thread")]
async fn metrics_monitor_records_errors_and_clears_on_success() {
    let step = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
        let step = Arc::clone(&step);
        move || {
            let step = Arc::clone(&step);
            async move {
                let attempt = step.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    Err(ToriiError::UnexpectedStatus {
                        status: StatusCode::GATEWAY_TIMEOUT,
                        reject_code: None,
                        message: None,
                    })
                } else {
                    Ok(ToriiMetricsSnapshot::from_prometheus(
                        Instant::now(),
                        "queue_size 4\n",
                    ))
                }
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("first update")
        .expect("channel open");
    let first = receiver.borrow().clone();
    assert!(first.last_error.is_some());
    assert!(!first.has_snapshot());
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("second update")
        .expect("channel open");
    let second = receiver.borrow().clone();
    assert!(second.last_error.is_none());
    assert!(
        matches!(
            second
                .last_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.queue_size),
            Some(value) if (value - 4.0).abs() < f64::EPSILON
        ),
        "successful poll should publish snapshot"
    );
    monitor.stop();
}
#[tokio::test(flavor = "current_thread")]
async fn metrics_monitor_tracks_last_success_and_exposes_age() {
    let step = Arc::new(AtomicUsize::new(0));
    let monitor = ToriiMetricsMonitor::spawn(Duration::from_millis(5), {
        let step = Arc::clone(&step);
        move || {
            let step = Arc::clone(&step);
            async move {
                let attempt = step.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    Ok(ToriiMetricsSnapshot::from_prometheus(
                        Instant::now(),
                        "queue_size 2\n",
                    ))
                } else {
                    Err(ToriiError::UnexpectedStatus {
                        status: StatusCode::BAD_GATEWAY,
                        reject_code: None,
                        message: None,
                    })
                }
            }
        }
    });
    let mut receiver = monitor.subscribe();
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("first update")
        .expect("channel open");
    let first = receiver.borrow().clone();
    let first_timestamp = first
        .last_success_at
        .expect("poll should record success timestamp");
    assert!(
        first.last_success_age().is_some(),
        "successful poll should expose age helper"
    );
    assert!(first.last_error.is_none());
    timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("second update")
        .expect("channel open");
    let second = receiver.borrow().clone();
    assert!(
        second.last_success_at == Some(first_timestamp),
        "failed poll should retain last_success_at"
    );
    assert!(
        second.last_error.is_some(),
        "failed poll should surface the error"
    );
    monitor.stop();
}
#[test]
fn metrics_snapshot_queue_utilization_reports_ratio() {
    let mut snapshot = empty_metrics_snapshot();
    snapshot.sumeragi_tx_queue_depth = Some(5.0);
    snapshot.sumeragi_tx_queue_capacity = Some(10.0);
    assert_eq!(
        snapshot.queue_utilization(),
        Some(0.5),
        "expected half-full queue to report 0.5 utilisation"
    );
    snapshot.sumeragi_tx_queue_depth = Some(12.0);
    assert_eq!(
        snapshot.queue_utilization(),
        Some(1.0),
        "ratio should clamp when depth exceeds capacity"
    );
    snapshot.sumeragi_tx_queue_capacity = Some(0.0);
    assert!(
        snapshot.queue_utilization().is_none(),
        "zero capacity should skip utilisation computation"
    );
}
#[test]
fn metrics_snapshot_queue_saturation_interprets_flags() {
    let mut snapshot = empty_metrics_snapshot();
    snapshot.sumeragi_tx_queue_saturated = Some(0.0);
    assert_eq!(snapshot.queue_saturation_flag(), Some(false));
    snapshot.sumeragi_tx_queue_saturated = Some(1.0);
    assert_eq!(snapshot.queue_saturation_flag(), Some(true));
    snapshot.sumeragi_tx_queue_saturated = Some(0.5);
    assert!(
        snapshot.queue_saturation_flag().is_none(),
        "non-binary values should bubble up as indeterminate"
    );
}
#[test]
fn metrics_snapshot_cold_entry_ratio_handles_missing_totals() {
    let mut snapshot = empty_metrics_snapshot();
    snapshot.state_tiered_hot_entries = Some(75.0);
    snapshot.state_tiered_cold_entries = Some(25.0);
    assert_eq!(
        snapshot.cold_entry_ratio(),
        Some(0.25),
        "cold tier share should reflect proportional occupancy"
    );
    snapshot.state_tiered_hot_entries = Some(0.0);
    snapshot.state_tiered_cold_entries = Some(0.0);
    assert!(
        snapshot.cold_entry_ratio().is_none(),
        "zero totals should skip ratio computation"
    );
}
#[test]
fn explorer_block_record_parses_camel_case_fields() {
    let value = norito::json!({
        "blockHash":"1122aabb",
        "height":"9",
        "createdAt":"2026-02-10T00:00:00Z",
        "transactionsRejected":"0",
        "transactionsTotal":"3"
    });
    let record = ExplorerBlockRecord::from_json(&value).expect("record");
    assert_eq!(record.hash, "1122aabb");
    assert_eq!(record.height, 9);
    assert!(record.prev_block_hash.is_none());
    assert_eq!(record.transactions_total, 3);
}
#[test]
fn explorer_blocks_page_errors_when_items_invalid() {
    let value = norito::json!({
        "pagination":{"page":1,"per_page":1,"total_pages":0,"total_items":0},
        "items":{"block_hash":"aa"}
    });
    let err = ExplorerBlocksPage::from_json(&value).expect_err("expected failure");
    assert!(matches!(err, ToriiError::Decode(_)));
}
#[test]
fn explorer_account_record_decodes_payload() {
    let value = norito::json!({
        "id": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        "i105_address": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        "network_prefix": 42,
        "metadata": { "role": "admin" },
        "owned_domains": 2,
        "owned_assets": 5,
        "owned_nfts": 1
    });
    let record = ExplorerAccountRecord::from_json(&value).expect("record");
    assert_eq!(
        record.id,
        "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
    );
    assert_eq!(
        record.i105_address,
        "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
    );
    assert_eq!(record.network_prefix, 42);
    assert_eq!(record.metadata, norito::json!({ "role": "admin" }));
    assert_eq!(record.owned_domains, 2);
    assert_eq!(record.owned_assets, 5);
    assert_eq!(record.owned_nfts, 1);
}
#[test]
fn explorer_accounts_page_decodes_entries() {
    let value = norito::json!({
        "pagination": {
            "limit": 10,
            "next_cursor": null,
            "has_more": false
        },
        "items": [
            {
                "id": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                "i105_address": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                "network_prefix": 1,
                "metadata": {},
                "owned_domains": 0,
                "owned_assets": 0,
                "owned_nfts": 0
            }
        ]
    });
    let page = ExplorerAccountsPage::from_json(&value).expect("page");
    assert_eq!(page.pagination.limit, 10);
    assert!(!page.pagination.has_more);
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].id,
        "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
    );
}
#[test]
fn explorer_domain_record_decodes_payload() {
    let value = norito::json!({
        "id": "sora",
        "logo": "https://example/logo.svg",
        "metadata": { "tier": "p0" },
        "owned_by": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "accounts": 5,
        "assets": 3,
        "nfts": 1
    });
    let record = ExplorerDomainRecord::from_json(&value).expect("record");
    assert_eq!(record.id, "sora");
    assert_eq!(record.logo.as_deref(), Some("https://example/logo.svg"));
    assert_eq!(
        record.owned_by,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
    assert_eq!(record.accounts, 5);
    assert_eq!(record.assets, 3);
    assert_eq!(record.nfts, 1);
}
#[test]
fn explorer_domains_page_validates_entries() {
    let value = norito::json!({
        "pagination":{"limit":10,"next_cursor":null,"has_more":false},
        "items":[{ "id":"sora","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","accounts":1,"assets":0,"nfts":0 }]
    });
    let page = ExplorerDomainsPage::from_json(&value).expect("page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "sora");
}
#[test]
fn explorer_asset_definition_record_decodes_payload() {
    let value = norito::json!({
        "id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        "mintable": "Infinitely",
        "logo": null,
        "metadata": { "decimals": 2 },
        "owned_by": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "assets": 10
    });
    let record = ExplorerAssetDefinitionRecord::from_json(&value).expect("record");
    assert_eq!(record.id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    assert_eq!(record.mintable, "Infinitely");
    assert_eq!(record.assets, 10);
    assert_eq!(
        record.owned_by,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
}
#[test]
fn explorer_asset_definition_page_validates_entries() {
    let value = norito::json!({
        "pagination":{"limit":5,"next_cursor":null,"has_more":false},
        "items":[
            {"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","mintable":"Infinitely","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","assets":2}
        ]
    });
    let page = ExplorerAssetDefinitionsPage::from_json(&value).expect("page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
}
#[test]
fn explorer_asset_record_decodes_payload() {
    let value = norito::json!({
        "id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        "definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        "account_id": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "value": "10.0"
    });
    let record = ExplorerAssetRecord::from_json(&value).expect("record");
    assert_eq!(record.id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    assert_eq!(
        record.account_id,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
    assert_eq!(record.value, "10.0");
}
#[test]
fn explorer_assets_page_validates_entries() {
    let value = norito::json!({
        "pagination":{"limit":10,"next_cursor":null,"has_more":false},
        "items":[
            {"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","account_id":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","value":"10"}
        ]
    });
    let page = ExplorerAssetsPage::from_json(&value).expect("page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].definition_id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
}
#[test]
fn explorer_nft_record_decodes_payload() {
    let value = norito::json!({
        "id": "art#gallery",
        "owned_by": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "metadata": { "uri": "ipfs://cid" }
    });
    let record = ExplorerNftRecord::from_json(&value).expect("record");
    assert_eq!(record.id, "art#gallery");
    assert_eq!(
        record.owned_by,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
    assert_eq!(record.metadata, norito::json!({ "uri": "ipfs://cid" }));
}
#[test]
fn explorer_nfts_page_validates_entries() {
    let value = norito::json!({
        "pagination":{"limit":1,"next_cursor":null,"has_more":false},
        "items":[{"id":"art#gallery","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","metadata":{}}]
    });
    let page = ExplorerNftsPage::from_json(&value).expect("page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].owned_by,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_accounts_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination": {
            "limit": 25,
            "next_cursor": "Y3Vyc29yLTI",
            "has_more": true
        },
        "items": [
            {
                "id": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                "i105_address": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                "network_prefix": 1,
                "metadata": { "owned_assets": 4 },
                "owned_domains": 0,
                "owned_assets": 4,
                "owned_nfts": 0
            }
        ]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/accounts")
            .query_param("cursor", "Y3Vyc29yLTE")
            .query_param("limit", "25")
            .query_param("domain", "sora")
            .query_param("with_asset", "usd#sora");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize mock body"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_accounts_page(ExplorerAccountsQuery {
            cursor: Some("Y3Vyc29yLTE".into()),
            limit: Some(25),
            domain: Some("sora".into()),
            with_asset: Some("usd#sora".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.pagination.limit, 25);
    assert_eq!(page.pagination.next_cursor.as_deref(), Some("Y3Vyc29yLTI"));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].owned_assets, 4);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_domains_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination":{"limit":10,"next_cursor":null,"has_more":false},
        "items":[{"id":"sora","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","accounts":1,"assets":0,"nfts":0}]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/domains")
            .query_param("limit", "10")
            .query_param(
                "owned_by",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            );
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_domains_page(ExplorerDomainsQuery {
            cursor: None,
            limit: Some(10),
            owned_by: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "sora");
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_asset_definitions_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination":{"limit":5,"next_cursor":"Y3Vyc29yLTI","has_more":true},
        "items":[{"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","mintable":"Infinitely","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","assets":7}]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/asset-definitions")
            .query_param("cursor", "Y3Vyc29yLTE")
            .query_param("limit", "5")
            .query_param("domain", "sora")
            .query_param(
                "owned_by",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            );
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_asset_definitions_page(ExplorerAssetDefinitionsQuery {
            cursor: Some("Y3Vyc29yLTE".into()),
            limit: Some(5),
            domain: Some("sora".into()),
            owned_by: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert!(page.pagination.has_more);
    assert_eq!(page.items[0].id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_assets_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination":{"limit":50,"next_cursor":null,"has_more":false},
        "items":[{"id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","account_id":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","value":"10"}]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/assets")
            .query_param("limit", "50")
            .query_param(
                "owned_by",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            )
            .query_param("definition", "usd#sora");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_assets_page(ExplorerAssetsQuery {
            cursor: None,
            limit: Some(50),
            owned_by: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
            definition: Some("usd#sora".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].account_id,
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_nfts_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination":{"limit":5,"next_cursor":"Y3Vyc29yLTI","has_more":true},
        "items":[{"id":"art#gallery","owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","metadata":{}}]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/nfts")
            .query_param("cursor", "Y3Vyc29yLTE")
            .query_param("limit", "5")
            .query_param(
                "owned_by",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            )
            .query_param("domain", "gallery");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_nfts_page(ExplorerNftsQuery {
            cursor: Some("Y3Vyc29yLTE".into()),
            limit: Some(5),
            owned_by: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
            domain: Some("gallery".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert!(page.pagination.has_more);
    assert_eq!(page.items[0].id, "art#gallery");
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_explorer_rwas_page_applies_filters() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "pagination":{"limit":5,"next_cursor":null,"has_more":false},
        "items":[{
            "id":"warehouse#commodities",
            "owned_by":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            "quantity":"10",
            "held_quantity":"2",
            "primary_reference":"vault-cert-1",
            "status":"active",
            "is_frozen":false,
            "metadata":{"origin":"AE"},
            "parents":[{"rwa":"source#commodities","quantity":"10"}]
        }]
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/explorer/rwas")
            .query_param("cursor", "Y3Vyc29yLTE")
            .query_param("limit", "5")
            .query_param(
                "owned_by",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            )
            .query_param("domain", "commodities");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .fetch_explorer_rwas_page(ExplorerRwasQuery {
            cursor: Some("Y3Vyc29yLTE".into()),
            limit: Some(5),
            owned_by: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
            domain: Some("commodities".into()),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.items[0].id, "warehouse#commodities");
    assert_eq!(page.items[0].held_quantity, "2");
    assert_eq!(page.items[0].parents[0].rwa, "source#commodities");
}
#[test]
fn explorer_cursor_metadata_rejects_inconsistent_continuation() {
    let value = norito::json!({
        "limit": 25,
        "next_cursor": null,
        "has_more": true
    });
    let error = ExplorerCursorMeta::from_json(&value, "cursor metadata")
        .expect_err("inconsistent metadata must fail");
    assert!(
        error
            .to_string()
            .contains("has_more must match next_cursor")
    );
}
#[test]
fn explorer_cursor_rejects_noncanonical_trailing_bits() {
    assert_eq!(
        validate_explorer_cursor("AA", "cursor").expect("canonical cursor"),
        "AA"
    );
    let error = validate_explorer_cursor("AB", "cursor")
        .expect_err("non-zero unused base64url bits must fail");
    assert!(error.to_string().contains("canonical base64url"));
}
#[test]
fn explorer_world_pages_reject_unknown_fields_and_oversized_items() {
    macro_rules! assert_all_world_pages_reject {
        ($value:expr, $needle:literal) => {{
            let error = ExplorerAccountsPage::from_json(&$value)
                .expect_err("accounts page must fail closed");
            assert!(error.to_string().contains($needle));
            let error =
                ExplorerDomainsPage::from_json(&$value).expect_err("domains page must fail closed");
            assert!(error.to_string().contains($needle));
            let error = ExplorerAssetDefinitionsPage::from_json(&$value)
                .expect_err("asset definitions page must fail closed");
            assert!(error.to_string().contains($needle));
            let error =
                ExplorerAssetsPage::from_json(&$value).expect_err("assets page must fail closed");
            assert!(error.to_string().contains($needle));
            let error =
                ExplorerNftsPage::from_json(&$value).expect_err("NFTs page must fail closed");
            assert!(error.to_string().contains($needle));
            let error =
                ExplorerRwasPage::from_json(&$value).expect_err("RWAs page must fail closed");
            assert!(error.to_string().contains($needle));
        }};
    }
    let unexpected_page_field = norito::json!({
        "pagination": {"limit": 1, "next_cursor": null, "has_more": false},
        "items": [],
        "page": 1
    });
    assert_all_world_pages_reject!(unexpected_page_field, "must contain exactly");
    let unexpected_metadata_field = norito::json!({
        "pagination": {
            "limit": 1,
            "next_cursor": null,
            "has_more": false,
            "total_items": 0
        },
        "items": []
    });
    assert_all_world_pages_reject!(unexpected_metadata_field, "must contain exactly");
    let oversized_items = norito::json!({
        "pagination": {"limit": 1, "next_cursor": null, "has_more": false},
        "items": [{}, {}]
    });
    assert_all_world_pages_reject!(oversized_items, "must contain at most 1 entries");
}
#[tokio::test(flavor = "current_thread")]
async fn explorer_cursor_query_rejects_invalid_bounds_before_http() {
    let client = ToriiClient::new("http://127.0.0.1:9").expect("client");
    let limit_error = client
        .fetch_explorer_domains_page(ExplorerDomainsQuery {
            cursor: None,
            limit: Some(101),
            owned_by: None,
        })
        .await
        .expect_err("oversized limit must fail locally");
    assert!(limit_error.to_string().contains("between 1 and 100"));
    let cursor_error = client
        .fetch_explorer_domains_page(ExplorerDomainsQuery {
            cursor: Some("padded==".to_owned()),
            limit: Some(25),
            owned_by: None,
        })
        .await
        .expect_err("padded cursor must fail locally");
    assert!(cursor_error.to_string().contains("canonical base64url"));
    let trailing_bits_error = client
        .fetch_explorer_domains_page(ExplorerDomainsQuery {
            cursor: Some("AB".to_owned()),
            limit: Some(25),
            owned_by: None,
        })
        .await
        .expect_err("cursor with non-zero unused bits must fail locally");
    assert!(
        trailing_bits_error
            .to_string()
            .contains("canonical base64url")
    );
}
#[test]
fn parse_pipeline_smoke_status_accepts_approved_height() {
    let value = norito::json!({
        "hash": "abcd",
        "status": {
            "kind": "Approved",
            "block_height": 7
        },
        "scope": "local",
        "resolved_from": "cache"
    });
    let status = parse_pipeline_smoke_status(&value)
        .expect("status")
        .expect("terminal status");
    assert_eq!(status, SmokeTransactionStatus::Committed(7));
}
#[test]
fn parse_pipeline_smoke_status_reports_rejection_reason() {
    let value = norito::json!({
        "hash": "abcd",
        "status": {
            "kind": "Rejected",
            "rejection_reason": { "Validation": "TooComplex" }
        },
        "scope": "local",
        "resolved_from": "cache"
    });
    let status = parse_pipeline_smoke_status(&value)
        .expect("status")
        .expect("terminal status");
    match status {
        SmokeTransactionStatus::Rejected(reason) => {
            assert!(reason.contains("TooComplex"), "reason was `{reason}`");
        }
        other => panic!("expected rejection, got {other:?}"),
    }
}
#[test]
fn parse_pipeline_smoke_status_preserves_queued_reconciliation_evidence() {
    let value = norito::json!({
        "hash": "abcd",
        "status": { "kind": "Queued" },
        "scope": "local",
        "resolved_from": "cache"
    });
    let status = parse_pipeline_smoke_status(&value)
        .expect("status")
        .expect("queued status remains observable");
    assert_eq!(status, SmokeTransactionStatus::Queued);
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_smoke_transaction_status_uses_pipeline_status() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let body = norito::json!({
        "hash": "abcd",
        "status": { "kind": "Committed", "block_height": 9 },
        "scope": "local",
        "resolved_from": "cache"
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/pipeline/transactions/status")
            .query_param("hash", "abcd");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let status = client
        .fetch_smoke_transaction_status("abcd")
        .await
        .expect("status")
        .expect("terminal status");
    mock.assert();
    assert_eq!(status, SmokeTransactionStatus::Committed(9));
}
#[tokio::test(flavor = "current_thread")]
async fn fetch_smoke_transaction_status_falls_back_to_explorer() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let pipeline = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/pipeline/transactions/status")
            .query_param("hash", "abcd");
        then.status(404);
    });
    let explorer = server.mock(|when, then| {
        when.method(GET).path("/v1/explorer/transactions/abcd");
        then.status(200).body(
            norito::json::to_string(&norito::json!({
                "hash": "abcd",
                "status": "Committed",
                "block": 12
            }))
            .expect("serialize"),
        );
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let status = client
        .fetch_smoke_transaction_status("abcd")
        .await
        .expect("status")
        .expect("terminal status");
    pipeline.assert();
    explorer.assert();
    assert_eq!(status, SmokeTransactionStatus::Committed(12));
}
#[tokio::test(flavor = "current_thread")]
async fn list_triggers_parses_results() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let payload = norito::json!({
        "items": [
            {
                "id": "daily-airdrop",
                "action": { "Mint": { "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "account_id": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", "value": "5" } },
                "metadata": { "cron": "0 0 * * *" }
            }
        ],
        "total": 7
    });
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/v1/triggers")
            .query_param("namespace", "core")
            .query_param(
                "authority",
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            )
            .query_param("limit", "5")
            .query_param("offset", "10");
        then.status(200)
            .body(norito::json::to_string(&payload).expect("serialize payload"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let page = client
        .list_triggers(TriggerListQuery {
            namespace: Some(" core ".into()),
            authority: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
            limit: Some(5),
            offset: Some(10),
        })
        .await
        .expect("page");
    mock.assert();
    assert_eq!(page.total, 7);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "daily-airdrop");
    assert_eq!(
        page.items[0].metadata,
        norito::json!({ "cron": "0 0 * * *" })
    );
}
#[tokio::test(flavor = "current_thread")]
async fn get_trigger_supports_missing() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let not_found = server.mock(|when, then| {
        when.method(GET).path("/v1/triggers/missing");
        then.status(404);
    });
    let body = norito::json!({
        "id": "mint-hook",
        "action": { "Register": { "Account": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D" } },
        "metadata": {}
    });
    let found = server.mock(|when, then| {
        when.method(GET).path("/v1/triggers/mint-hook");
        then.status(200)
            .body(norito::json::to_string(&body).expect("serialize body"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    assert!(
        client
            .get_trigger("missing")
            .await
            .expect("request")
            .is_none()
    );
    not_found.assert();
    let record = client
        .get_trigger("mint-hook")
        .await
        .expect("request")
        .expect("record");
    found.assert();
    assert_eq!(record.id, "mint-hook");
    assert_eq!(record.action, body.get("action").unwrap().clone());
}
#[tokio::test(flavor = "current_thread")]
async fn register_trigger_posts_json() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let request = norito::json!({
        "id": "hook",
        "action": { "Mint": { "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "account_id": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", "value": "42" } },
        "metadata": { "note": "demo" }
    });
    let response = request.clone();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/v1/triggers")
            .header("content-type", "application/json")
            .body(norito::json::to_string(&request).expect("serialize request"));
        then.status(200)
            .body(norito::json::to_string(&response).expect("serialize payload"));
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    let record = client.register_trigger(&request).await.expect("request");
    mock.assert();
    assert_eq!(record.id, "hook");
    assert_eq!(record.action, response.get("action").unwrap().clone());
}
#[tokio::test(flavor = "current_thread")]
async fn delete_trigger_reports_outcome() {
    let Some(server) = try_start_mock_server() else {
        return;
    };
    let deleted = server.mock(|when, then| {
        when.method(DELETE).path("/v1/triggers/hook");
        then.status(204);
    });
    let missing = server.mock(|when, then| {
        when.method(DELETE).path("/v1/triggers/missing");
        then.status(404);
    });
    let client = ToriiClient::new(server.url("/")).expect("client");
    assert!(client.delete_trigger("hook").await.expect("delete"));
    deleted.assert();
    assert!(!client.delete_trigger("missing").await.expect("delete"));
    missing.assert();
}
#[test]
fn account_deleted_summary_mentions_account_id() {
    let event_box = EventBox::Data(DataEvent::from(AccountEvent::Deleted(ALICE_ID.clone())).into());
    let summary = EventSummary::from_event(&event_box);
    assert_eq!(summary.label, "Account deleted");
    let detail = summary.detail.expect("detail");
    let alice_literal = ALICE_ID.to_string();
    assert!(
        detail.contains(&alice_literal),
        "detail `{detail}` should mention {alice_literal}"
    );
}
#[test]
fn asset_transfer_summaries_cover_direct_and_batch_events() {
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("valid domain"),
        "rose".parse().expect("valid asset name"),
    );
    let source = AssetId::new(definition.clone(), ALICE_ID.clone());
    let destination = AssetId::new(definition, BOB_ID.clone());
    let amount = Quantity::from(5_u32);
    let direct = AssetEvent::Transferred(AssetTransferred {
        source: source.clone(),
        destination: destination.clone(),
        amount: amount.clone(),
    });
    let (label, detail) = asset_event_summary(&direct);
    assert_eq!(label, "Asset transferred");
    assert!(detail.contains(&source.to_string()));
    assert!(detail.contains(&destination.to_string()));
    assert!(detail.contains(&amount.to_string()));
    let batch = AssetEvent::BatchTransferOutcome(AssetBatchTransferOutcome {
        leg_index: 2,
        leg_id: "leg-2".to_owned(),
        asset: source.clone(),
        destination: BOB_ID.clone(),
        amount: amount.clone(),
        status: AssetBatchTransferLegStatus::Applied,
    });
    let (label, detail) = asset_event_summary(&batch);
    assert_eq!(label, "Asset batch transfer leg");
    assert!(detail.contains("leg_index=2"));
    assert!(detail.contains("leg_id=leg-2"));
    assert!(detail.contains(&source.to_string()));
    assert!(detail.contains(&BOB_ID.to_string()));
    assert!(detail.contains(&amount.to_string()));
    assert!(detail.contains("status=Applied"));
}
#[test]
fn account_controller_replaced_summary_mentions_old_and_new_controllers() {
    let event_box = EventBox::Data(
        DataEvent::from(AccountEvent::ControllerReplaced(
            AccountControllerReplaced {
                account: ALICE_ID.clone(),
                previous_account: BOB_ID.clone(),
                previous_controller: iroha_data_model::account::AccountController::single(
                    BOB_KEYPAIR.public_key().clone(),
                ),
                new_controller: iroha_data_model::account::AccountController::single(
                    ALICE_KEYPAIR.public_key().clone(),
                ),
            },
        ))
        .into(),
    );
    let summary = EventSummary::from_event(&event_box);
    assert_eq!(summary.label, "Account controller replaced");
    let detail = summary.detail.expect("detail");
    assert!(
        detail.contains(&ALICE_ID.to_string()) && detail.contains(&BOB_ID.to_string()),
        "detail `{detail}` should mention both account ids"
    );
    assert!(
        detail.contains("previous_controller=single") && detail.contains("new_controller=single"),
        "detail `{detail}` should mention both controller summaries"
    );
}
#[test]
fn account_recovery_policy_summary_mentions_alias_and_quorum() {
    let alias = iroha_data_model::account::AccountAlias::domainless(
        "primary".parse().expect("valid alias label"),
        iroha_data_model::nexus::DataSpaceId::new(7),
    );
    let policy = iroha_data_model::account::AccountRecoveryPolicy::new(
        vec![iroha_data_model::account::RecoveryGuardian::new(
            BOB_ID.clone(),
            1,
        )],
        1,
        std::num::NonZeroU64::new(60_000).expect("non-zero timelock"),
    )
    .expect("valid recovery policy");
    let event_box = EventBox::Data(
        DataEvent::from(AccountEvent::Recovery(AccountRecoveryEvent::PolicySet(
            AccountRecoveryPolicySet {
                account: ALICE_ID.clone(),
                alias,
                policy,
            },
        )))
        .into(),
    );
    let summary = EventSummary::from_event(&event_box);
    assert_eq!(summary.label, "Account recovery policy set");
    let detail = summary.detail.expect("detail");
    assert!(
        detail.contains(&ALICE_ID.to_string())
            && detail.contains("label=primary")
            && detail.contains("quorum=1")
            && detail.contains("timelock_ms=60000"),
        "detail `{detail}` should summarize the recovery policy"
    );
}
fn empty_metrics_snapshot() -> ToriiMetricsSnapshot {
    ToriiMetricsSnapshot {
        timestamp: Instant::now(),
        queue_size: None,
        view_changes: None,
        sumeragi_tx_queue_depth: None,
        sumeragi_tx_queue_capacity: None,
        sumeragi_tx_queue_saturated: None,
        state_tiered_hot_entries: None,
        state_tiered_cold_entries: None,
        state_tiered_cold_bytes: None,
        uptime_since_genesis_ms: None,
    }
}
