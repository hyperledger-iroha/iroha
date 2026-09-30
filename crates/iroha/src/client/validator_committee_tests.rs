// Canonical Nexus committee observations; finality authorization stays with its verifier.
mod validator_committee_capability {
    use super::*;
    use crate::{
        Error,
        client::capability_test_support::{AsyncOnlyTransport, GatedTransport, TransportGate},
    };
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn status_fixture() -> ValidatorCommitteeStatusV1 {
        use crate::crypto::{Algorithm, KeyPair};
        use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
        use iroha_data_model::{
            block::builder::BlockBuilder,
            sumeragi::finality::{NativeFinalityArtifact, NativeFinalityLimits},
        };
        // This is an HTTP carrier fixture, not a finality fixture: no QC is fabricated.
        // Authorization tests use the complete native journal consumer in Core.
        let key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let transaction = TransactionBuilder::new(
            test_network_id(),
            AccountId::new(key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(key.private_key())
        .unwrap();
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(HashOf::from_untyped_unchecked(Hash::new(
                b"transport predecessor",
            ))),
            None,
            10,
            0,
        ));
        builder.push_transaction(transaction);
        let block = builder
            .try_build_with_signature(0, key.private_key())
            .unwrap();
        let artifact = NativeFinalityArtifact::from_block(
            &block,
            NativeFinalityLimits {
                block_bytes: 16 * 1024 * 1024,
                journal_bytes: 16 * 1024 * 1024,
                block_count: 256,
                allocated_bytes: 64 * 1024 * 1024,
            },
        )
        .unwrap();
        ValidatorCommitteeStatusV1 {
            network_id: test_network_id(),
            target_epoch: 7,
            latest_finality: artifact,
            selected: None,
            candidate_keys: vec![],
            pending_beacon_session: None,
        }
    }

    fn attach(
        responder: impl Fn(&crate::http::TransportRequest) -> Result<Response<Vec<u8>>>
        + Send
        + Sync
        + 'static,
        delay: Duration,
        timeout: Duration,
    ) -> (
        Client,
        Arc<Mutex<Vec<crate::http::TransportRequest>>>,
        Arc<AtomicUsize>,
    ) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let completed = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(AsyncOnlyTransport {
            responder: Box::new(responder),
            requests: requests.clone(),
            completed: completed.clone(),
            delay,
        });
        let mut builder = client_with_base_url(base_url())
            .to_builder()
            .http_transport(transport);
        builder.network_id = test_network_id();
        builder.torii_request_timeout = timeout;
        (builder.build().unwrap(), requests, completed)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn canonical_observation_binds_route_target_and_preserves_executor_progress() {
        let status = status_fixture();
        let fixture = norito_response(StatusCode::OK, &status);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let completed = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(TransportGate::default());
        let transport = Arc::new(GatedTransport {
            inner: AsyncOnlyTransport {
                responder: Box::new(move |_| Ok(fixture.clone())),
                requests: requests.clone(),
                completed: completed.clone(),
                delay: Duration::ZERO,
            },
            gate: gate.clone(),
        });
        let mut builder = client_with_base_url(base_url())
            .to_builder()
            .http_transport(transport);
        builder.network_id = test_network_id();
        builder.torii_request_timeout = Duration::ZERO;
        let client = builder.build().unwrap();
        let nexus = client.nexus();
        fn require_send(_: impl Send) {}
        require_send(nexus.validator_committee(Some(7)));
        let operation = nexus.validator_committee(Some(7));
        tokio::pin!(operation);
        tokio::select! {
            early = &mut operation => panic!("operation completed before transport release: {early:?}"),
            () = gate.wait_until_entered() => {}
        }
        assert_eq!(
            completed.load(Ordering::SeqCst),
            0,
            "transport cannot finish before release"
        );
        gate.release();
        let result = operation.await;
        assert_eq!(result.unwrap(), status);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, HttpMethod::GET);
        assert_eq!(request.url.path(), "/v1/nexus/validator-committee");
        assert_eq!(request.url.query(), Some("target_epoch=7"));
        assert!(request.body.is_empty());
        assert_eq!(request.max_response_bytes, 16 * 1024 * 1024);
        let accepts: Vec<_> = request
            .headers
            .iter()
            .filter(|(name, _)| name == http::header::ACCEPT)
            .collect();
        assert_eq!(accepts.len(), 1);
        assert_eq!(accepts[0].1, APPLICATION_NORITO);
        assert_eq!(request.timeout, None);
    }

    #[tokio::test]
    async fn rejects_unbound_noncanonical_and_wrong_media_observations() {
        for mutation in 0..6 {
            let mut status = status_fixture();
            match mutation {
                0 => status.target_epoch = 8,
                1 => {
                    status.network_id = NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new(b"foreign committee network")),
                    )
                }
                4 => status.latest_finality.block_wire.push(0),
                5 => status.target_epoch = 0,
                _ => {}
            }
            let (client, _, _) = attach(
                move |_| {
                    Ok(match mutation {
                        2 => {
                            json_response(StatusCode::OK, &norito::json::to_json(&status).unwrap())
                        }
                        3 => empty_response(StatusCode::NOT_FOUND),
                        _ => norito_response(StatusCode::OK, &status),
                    })
                },
                Duration::ZERO,
                Duration::from_secs(1),
            );
            let error = client
                .nexus()
                .validator_committee(Some(7))
                .await
                .unwrap_err();
            assert!(
                match mutation {
                    0 | 1 | 5 => matches!(error, Error::ResponseBinding { .. }),
                    2 | 4 => matches!(error, Error::Decode { .. }),
                    3 => matches!(error, Error::Http { status: 404, .. }),
                    _ => unreachable!(),
                },
                "wrong error for mutation {mutation}: {error:?}"
            );
        }
    }

    #[tokio::test]
    async fn zero_target_epoch_is_rejected_before_dispatch() {
        let (client, requests, _) = attach(
            |_| Ok(empty_response(StatusCode::NOT_FOUND)),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client.nexus().validator_committee(Some(0)).await,
            Err(Error::InvalidRequest { .. })
        ));
        assert!(requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn omitted_epoch_remains_a_server_selected_observation() {
        let mut status = status_fixture();
        status.target_epoch = 1;
        let fixture = status.clone();
        let (client, requests, _) = attach(
            move |_| Ok(norito_response(StatusCode::OK, &fixture)),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(
            client.nexus().validator_committee(None).await.unwrap(),
            status
        );
        assert!(requests.lock().unwrap()[0].url.query().is_none());
    }

    #[tokio::test]
    async fn retains_http_error_body_and_retry_after_without_replay() {
        let (client, requests, _) = attach(
            |_| {
                Ok(Response::builder()
                    .status(429)
                    .header("Retry-After", "3")
                    .body(b"capacity".to_vec())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(
            client.nexus().validator_committee(None).await.unwrap_err(),
            Error::Http {
                operation: "nexus.validator_committee.read",
                status: 429,
                retry_after: Some(Duration::from_secs(3)),
                body: b"capacity".to_vec(),
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn injected_response_cannot_exceed_the_transport_bound() {
        let (client, requests, _) = attach(
            |_| {
                Ok(Response::builder()
                    .status(200)
                    .header("Content-Type", APPLICATION_NORITO)
                    .body(vec![0; 16 * 1024 * 1024 + 1])
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client.nexus().validator_committee(None).await.unwrap_err(),
            Error::ResponseTooLarge {
                maximum: 16_777_216,
                actual: Some(16_777_217)
            }
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn request_deadline_cancels_pending_transport_and_expired_context_never_sends() {
        let (client, requests, completed) = attach(
            |_| Ok(empty_response(StatusCode::NOT_FOUND)),
            Duration::from_secs(60),
            Duration::from_millis(10),
        );
        assert_eq!(
            client.nexus().validator_committee(None).await.unwrap_err(),
            Error::Timeout {
                operation: "nexus.validator_committee.read"
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 0);
        let client = client.with_request_deadline(std::time::Instant::now());
        assert_eq!(
            client.nexus().validator_committee(None).await.unwrap_err(),
            Error::Timeout {
                operation: "nexus.validator_committee.read"
            }
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn blocking_facade_rejects_async_entry_without_dispatch() {
        let (client, requests, _) = attach(
            |_| Ok(empty_response(StatusCode::NOT_FOUND)),
            Duration::ZERO,
            Duration::ZERO,
        );
        let client = crate::blocking::Client::from_client(client).unwrap();
        assert!(matches!(
            client.nexus().validator_committee(None),
            Err(Error::Blocking(_))
        ));
        assert!(requests.lock().unwrap().is_empty());
    }

    #[test]
    fn blocking_facade_reuses_its_owned_runtime_and_the_same_async_operation() {
        let status = status_fixture();
        let fixture = status.clone();
        let (client, requests, _) = attach(
            move |_| Ok(norito_response(StatusCode::OK, &fixture)),
            Duration::ZERO,
            Duration::ZERO,
        );
        let client = crate::blocking::Client::from_client(client).unwrap();
        for _ in 0..2 {
            assert_eq!(client.nexus().validator_committee(Some(7)).unwrap(), status);
        }
        assert_eq!(requests.lock().unwrap().len(), 2);
    }
}
