//! Deadline-bound status observations for the real committee-transition fixture.
use eyre::{Result, WrapErr as _, ensure};
use std::time::{Duration, Instant};

const RETRY_DELAY: Duration = Duration::from_millis(150);

fn retry_delay(error: &iroha::Error, remaining: Duration) -> Option<Duration> {
    let iroha::Error::StatusUnavailable {
        reason:
            Some(iroha::StatusFailureReason::DeadlineElapsed | iroha::StatusFailureReason::StateBusy),
        retry_after,
    } = error
    else {
        return None;
    };
    let delay = retry_after.unwrap_or(RETRY_DELAY).max(RETRY_DELAY);
    (delay < remaining).then_some(delay)
}

/// Submit once within the same phase deadline as the surrounding height observations.
/// HTTP probes, fee quotes, dispatch and finality share this deadline; no write is retried.
pub(super) fn submit_until<T>(
    client: iroha::blocking::Client,
    deadline: Instant,
    submit: impl FnOnce(iroha::blocking::Client) -> Result<T>,
) -> Result<T> {
    ensure!(
        Instant::now() < deadline,
        "committee submission deadline elapsed"
    );
    let bounded =
        iroha::blocking::Client::from_client(client.client().with_request_deadline(deadline))?;
    // Preserve an unresolved dispatch or finality error with its exact transaction identity.
    let result = submit(bounded)?;
    ensure!(
        Instant::now() < deadline,
        "committee submission deadline elapsed"
    );
    Ok(result)
}

/// Submit once asynchronously under the original phase deadline.
/// Preserve the SDK's unresolved transaction identity when its finality wait expires.
pub(super) async fn submit_async_until<T, F, Fut>(
    client: iroha::client::Client,
    deadline: Instant,
    submit: F,
) -> Result<T>
where
    F: FnOnce(iroha::client::Client) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    ensure!(
        Instant::now() < deadline,
        "committee submission deadline elapsed"
    );
    let bounded = client.with_request_deadline(deadline);
    let result = submit(bounded).await?;
    ensure!(
        Instant::now() < deadline,
        "committee submission deadline elapsed"
    );
    Ok(result)
}

/// Read an authoritative applied height without extending the caller's deadline.
pub(super) async fn height_until(client: &iroha::client::Client, deadline: Instant) -> Result<u64> {
    tokio::time::timeout_at(deadline.into(), async {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            ensure!(!remaining.is_zero(), "committee status deadline elapsed");
            let mut builder = client.to_builder();
            builder.torii_request_timeout = builder.torii_request_timeout.min(remaining);
            match builder.build()?.status().get().await {
                Ok(status) => {
                    ensure!(
                        Instant::now() < deadline,
                        "committee status deadline elapsed"
                    );
                    return Ok(status.blocks);
                }
                Err(error) => {
                    let Some(delay) =
                        retry_delay(&error, deadline.saturating_duration_since(Instant::now()))
                    else {
                        return Err(error.into());
                    };
                    tokio::time::sleep(delay).await;
                }
            }
        }
    })
    .await
    .wrap_err("committee status exceeded its original deadline")?
}

/// Observe every voter concurrently within the same original phase deadline.
/// The exact bounded committee roster determines both concurrency and result order;
/// one failed or missing seat cannot be replaced by a smaller successful subset.
pub(super) async fn heights_until(
    clients: &[iroha::client::Client],
    deadline: Instant,
) -> Result<Vec<u64>> {
    ensure!(
        iroha_data_model::block::consensus::is_valid_committee_size(clients.len()),
        "committee status requires an exact bounded voter roster"
    );
    ensure!(
        Instant::now() < deadline,
        "committee status deadline elapsed"
    );
    futures_util::future::try_join_all(clients.iter().map(|client| height_until(client, deadline)))
        .await
}

/// Observe from a dedicated blocking worker with the same classified retry policy.
pub(super) fn height_until_blocking(
    client: &iroha::blocking::Client,
    deadline: Instant,
) -> Result<u64> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "committee status deadline elapsed");
        let mut builder = client.client().to_builder();
        builder.torii_request_timeout = builder.torii_request_timeout.min(remaining);
        let bounded = iroha::blocking::Client::from_client(builder.build()?)?;
        match bounded.status().get() {
            Ok(status) => {
                ensure!(
                    Instant::now() < deadline,
                    "committee status deadline elapsed"
                );
                return Ok(status.blocks);
            }
            Err(error) => {
                let Some(delay) =
                    retry_delay(&error, deadline.saturating_duration_since(Instant::now()))
                else {
                    return Err(error.into());
                };
                std::thread::sleep(delay);
            }
        }
    }
}

#[cfg(test)]
mod status_observation_tests {
    use super::*;
    use norito::json;

    async fn observe(
        client: &iroha::client::Client,
        deadline: Instant,
        blocking: bool,
    ) -> Result<u64> {
        if blocking {
            let client = iroha::blocking::Client::from_client(client.clone())?;
            iroha_test_network::read_on_dedicated_thread(move || {
                height_until_blocking(&client, deadline)
            })
            .await
        } else {
            height_until(client, deadline).await
        }
    }
    use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    #[derive(Debug)]
    struct StatusTransport {
        responses: Mutex<VecDeque<(u16, Vec<u8>, Option<&'static str>, Option<&'static str>)>>,
        request_budgets: Mutex<Vec<Duration>>,
        consume_request_deadline: bool,
    }

    impl HttpTransport for StatusTransport {
        fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
            panic!("status observation must use asynchronous reads")
        }

        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move {
                assert_eq!(request.method, iroha::http::Method::GET);
                assert_eq!(request.url.path(), "/status");
                assert!(
                    request.body.is_empty(),
                    "the observation must not submit work"
                );
                self.request_budgets
                    .lock()
                    .unwrap()
                    .push(request.timeout.unwrap());
                if self.consume_request_deadline {
                    // Finish synchronously after the deadline so timeout_at cannot
                    // intercept the response before this future returns Ready.
                    std::thread::sleep(request.timeout.unwrap() + Duration::from_millis(1));
                }
                let (status, body, retry_after, reason) = self
                    .responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected status retry");
                let mut response = Response::builder()
                    .status(status)
                    .header("content-type", "application/json");
                if let Some(retry_after) = retry_after {
                    response = response.header("retry-after", retry_after);
                }
                if let Some(reason) = reason {
                    response = response.header("x-iroha-reject-code", reason);
                }
                Ok(response.body(body)?)
            })
        }
    }

    fn client(transport: Arc<dyn HttpTransport>) -> iroha::client::Client {
        use iroha_crypto::{Hash, HashOf};
        let config = iroha::config::Config {
            chain: "status-observation-test".into(),
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH])),
            ),
            key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
            account: iroha_test_samples::ALICE_ID.clone(),
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            torii_api_url: "http://status-observation.invalid/".parse().unwrap(),
            torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            basic_auth: None,
            api_token: None,
            transaction_add_nonce: false,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(10),
            sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
            sorafs_anonymity_policy: Default::default(),
            sorafs_rollout_phase: Default::default(),
        };
        iroha::client::Client::builder(config)
            .http_transport(transport)
            .build()
            .unwrap()
    }

    fn transport(
        responses: impl IntoIterator<Item = (u16, Vec<u8>, Option<&'static str>, Option<&'static str>)>,
    ) -> Arc<StatusTransport> {
        Arc::new(StatusTransport {
            responses: Mutex::new(responses.into_iter().collect()),
            request_budgets: Mutex::new(Vec::new()),
            consume_request_deadline: false,
        })
    }

    #[derive(Debug)]
    struct PhaseTransport {
        requests: Mutex<Vec<(String, Duration)>>,
        budget: Duration,
    }

    impl HttpTransport for PhaseTransport {
        fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
            panic!("the blocking facade must use its owned asynchronous transport")
        }

        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move {
                assert_eq!(request.method, iroha::http::Method::GET);
                let timeout = request.timeout.expect("every phase dispatch has a budget");
                assert!(
                    timeout <= self.budget,
                    "a dispatch renewed the phase budget"
                );
                let path = request.url.path();
                self.requests
                    .lock()
                    .unwrap()
                    .push((path.to_owned(), timeout));
                let body = match path {
                    "/status" => json::to_vec(&iroha_torii_shared::status::Status {
                        blocks: 69,
                        ..Default::default()
                    })?,
                    "/v1/pipeline/transactions/status" => {
                        // The server accepted this exact request but never returns a response.
                        // Only propagation of the original phase deadline can finish the wait.
                        assert!(
                            request
                                .url
                                .query_pairs()
                                .any(|(key, value)| { key == "scope" && value == "local" })
                        );
                        return std::future::pending::<Result<Response<Vec<u8>>>>().await;
                    }
                    other => panic!("unexpected phase dispatch: {other}"),
                };
                Ok(Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(body)?)
            })
        }
    }

    #[test]
    fn submission_phase_deadline_bounds_prior_http_and_six_hundred_second_finality() {
        use iroha_crypto::{Hash, HashOf};
        let budget = Duration::from_secs(2);
        let transport = Arc::new(PhaseTransport {
            requests: Mutex::new(Vec::new()),
            budget,
        });
        let mut builder = client(transport.clone()).to_builder();
        builder.transaction_status_timeout = Duration::from_secs(600);
        let original = iroha::blocking::Client::from_client(builder.build().unwrap()).unwrap();
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x71; Hash::LENGTH]));
        let deadline = Instant::now() + budget;
        let error = submit_until(original.clone(), deadline, |bounded| {
            // Exercise a preceding SDK dispatch and the real finality poller on one clone.
            // A paid submit's capability/quote requests use this same transport boundary.
            assert_eq!(bounded.status().get()?.blocks, 69);
            bounded.wait_for_transaction_applied_local(
                hash,
                iroha::client::TransactionWaitOptions {
                    timeout: Duration::from_secs(600),
                    poll_interval: Duration::from_secs(60),
                },
            )
        })
        .expect_err("pending finality must stop at the phase deadline");
        assert!(
            format!("{error:#}").contains(&hash.to_string()),
            "the exact unresolved transaction identity must survive the deadline"
        );
        assert!(Instant::now().saturating_duration_since(deadline) < Duration::from_secs(2));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            2,
            "no polling read may run after the phase deadline"
        );
        assert_eq!(requests[0].0, "/status");
        assert_eq!(requests[1].0, "/v1/pipeline/transactions/status");
        assert!(
            requests[1].1 < requests[0].1,
            "finality must consume the remaining phase budget"
        );
        assert_eq!(
            original.client().to_builder().transaction_status_timeout,
            Duration::from_secs(600)
        );
    }

    #[tokio::test]
    async fn asynchronous_submission_retains_original_deadline_and_unresolved_identity() {
        use iroha_crypto::{Hash, HashOf};
        let budget = Duration::from_secs(2);
        let transport = Arc::new(PhaseTransport {
            requests: Mutex::new(Vec::new()),
            budget,
        });
        let mut builder = client(transport.clone()).to_builder();
        builder.transaction_status_timeout = Duration::from_secs(600);
        let original = builder.build().unwrap();
        let hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x72; Hash::LENGTH]));
        let deadline = Instant::now() + budget;
        let error = submit_async_until(original.clone(), deadline, |bounded| async move {
            assert_eq!(bounded.status().get().await?.blocks, 69);
            bounded
                .wait_until_transaction_applied_local(
                    hash,
                    iroha::client::TransactionWaitOptions {
                        timeout: Duration::from_secs(600),
                        poll_interval: Duration::from_secs(60),
                    },
                )
                .await
        })
        .await
        .expect_err("original finality deadline must finish the pending read");
        assert!(
            format!("{error:#}").contains(&hash.to_string()),
            "asynchronous progress must retain the original unresolved transaction identity: {error:#}"
        );
        assert!(Instant::now().saturating_duration_since(deadline) < Duration::from_secs(2));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2, "deadline cannot dispatch another read");
        assert_eq!(requests[0].0, "/status");
        assert_eq!(requests[1].0, "/v1/pipeline/transactions/status");
        assert!(requests[1].1 < requests[0].1);
        assert_eq!(
            original.to_builder().transaction_status_timeout,
            Duration::from_secs(600)
        );
    }

    #[tokio::test]
    async fn asynchronous_submission_refuses_expired_and_late_success_without_replacing_error() {
        let original = client(transport([]));
        let error = submit_async_until::<(), _, _>(original.clone(), Instant::now(), |_| async {
            panic!("expired progress cannot invoke submission")
        })
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("committee submission deadline elapsed")
        );
        let deadline = Instant::now() + Duration::from_millis(20);
        let error = submit_async_until(original.clone(), deadline, |_| async move {
            tokio::time::sleep_until(deadline.into()).await;
            Ok(69_u64)
        })
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("committee submission deadline elapsed")
        );
        #[derive(Debug)]
        struct OriginalDispatch;
        impl std::fmt::Display for OriginalDispatch {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("original unresolved asynchronous dispatch")
            }
        }
        impl std::error::Error for OriginalDispatch {}
        let deadline = Instant::now() + Duration::from_millis(20);
        let error = submit_async_until(original, deadline, |_| async move {
            tokio::time::sleep_until(deadline.into()).await;
            Err::<(), _>(OriginalDispatch.into())
        })
        .await
        .unwrap_err();
        assert!(error.downcast_ref::<OriginalDispatch>().is_some());
    }

    #[test]
    fn submission_phase_deadline_rejects_expired_work_before_dispatch() {
        let transport = transport([]);
        let original = iroha::blocking::Client::from_client(client(transport.clone())).unwrap();
        let error = submit_until(original, Instant::now(), |_| -> Result<()> {
            panic!("expired work must not invoke the submission closure")
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("committee submission deadline elapsed")
        );
        assert!(transport.request_budgets.lock().unwrap().is_empty());
    }

    #[test]
    fn submission_phase_deadline_rejects_late_success_and_preserves_submission_error() {
        let original = iroha::blocking::Client::from_client(client(transport([]))).unwrap();
        let deadline = Instant::now() + Duration::from_millis(20);
        let error = submit_until(original.clone(), deadline, |_| {
            std::thread::sleep(
                deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
            );
            Ok(69_u64)
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("committee submission deadline elapsed")
        );
        #[derive(Debug)]
        struct OriginalDispatch;
        impl std::fmt::Display for OriginalDispatch {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("original unresolved dispatch")
            }
        }
        impl std::error::Error for OriginalDispatch {}
        let error = submit_until(
            original,
            Instant::now() + Duration::from_secs(2),
            |_| -> Result<()> { Err(OriginalDispatch.into()) },
        )
        .unwrap_err();
        assert!(error.downcast_ref::<OriginalDispatch>().is_some());
    }

    #[tokio::test]
    async fn status_observation_rejects_ready_response_after_deadline() {
        for blocking in [false, true] {
            let status = iroha_torii_shared::status::Status {
                blocks: 4,
                ..Default::default()
            };
            let mut transport = transport([(200, json::to_vec(&status).unwrap(), None, None)]);
            Arc::get_mut(&mut transport)
                .unwrap()
                .consume_request_deadline = true;
            let client = client(transport.clone());
            let result = observe(
                &client,
                Instant::now() + Duration::from_millis(30),
                blocking,
            )
            .await;
            assert!(
                result.is_err(),
                "a late ready response cannot certify the phase height"
            );
            assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn status_observation_preserves_a_tighter_client_timeout() {
        for blocking in [false, true] {
            let status = iroha_torii_shared::status::Status {
                blocks: 4,
                ..Default::default()
            };
            let transport = transport([(200, json::to_vec(&status).unwrap(), None, None)]);
            let mut builder = client(transport.clone()).to_builder();
            let budget = Duration::from_millis(50);
            builder.torii_request_timeout = budget;
            let client = builder.build().unwrap();
            assert_eq!(
                observe(&client, Instant::now() + Duration::from_secs(5), blocking)
                    .await
                    .unwrap(),
                4
            );
            assert_eq!(*transport.request_budgets.lock().unwrap(), [budget]);
        }
    }

    #[tokio::test]
    async fn status_observation_retries_typed_busy_with_remaining_budget() {
        for blocking in [false, true] {
            for reason in [
                iroha::StatusFailureReason::DeadlineElapsed,
                iroha::StatusFailureReason::StateBusy,
            ] {
                let envelope = iroha_torii_shared::ErrorEnvelope::new(reason.code(), "busy");
                let status = iroha_torii_shared::status::Status {
                    blocks: 4,
                    ..Default::default()
                };
                let transport = transport([
                    (
                        503,
                        json::to_vec(&envelope).unwrap(),
                        None,
                        Some(reason.code()),
                    ),
                    (
                        503,
                        norito::to_bytes(&envelope).unwrap(),
                        None,
                        Some(reason.code()),
                    ),
                    (200, json::to_vec(&status).unwrap(), None, None),
                ]);
                let client = client(transport.clone());
                let budget = Duration::from_secs(5);
                let observed = observe(&client, Instant::now() + budget, blocking)
                    .await
                    .unwrap();
                assert_eq!(observed, 4);
                let budgets = transport.request_budgets.lock().unwrap();
                assert_eq!(budgets.len(), 3);
                assert!(budgets[0] <= budget);
                assert!(
                    budgets.windows(2).all(|pair| pair[1] < pair[0]),
                    "retries must not renew the caller's deadline"
                );
            }
        }
    }

    #[tokio::test]
    async fn status_observation_stops_at_original_deadline_during_retry_after() {
        for blocking in [false, true] {
            for reason in [
                iroha::StatusFailureReason::DeadlineElapsed,
                iroha::StatusFailureReason::StateBusy,
            ] {
                let envelope = iroha_torii_shared::ErrorEnvelope::new(reason.code(), "busy");
                let transport = transport([(
                    503,
                    json::to_vec(&envelope).unwrap(),
                    Some("60"),
                    Some(reason.code()),
                )]);
                let client = client(transport.clone());
                let result = tokio::time::timeout(
                    Duration::from_secs(2),
                    observe(
                        &client,
                        Instant::now() + Duration::from_millis(80),
                        blocking,
                    ),
                )
                .await
                .expect("Retry-After must remain bounded by the existing deadline");
                let error = result.unwrap_err();
                assert!(
                    matches!(error.downcast_ref::<iroha::Error>(), Some(iroha::Error::StatusUnavailable { reason: Some(actual), .. }) if *actual == reason)
                );
                assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
            }
        }
    }

    #[tokio::test]
    async fn status_observation_propagates_auth_other_service_and_decode_failures() {
        use iroha::StatusFailureReason;
        for blocking in [false, true] {
            for reason in [
                StatusFailureReason::Disabled,
                StatusFailureReason::MailboxUnavailable,
                StatusFailureReason::ActorClosed,
                StatusFailureReason::StateUnavailable,
                StatusFailureReason::CheckpointChanged,
                StatusFailureReason::MissingBlock,
                StatusFailureReason::JournalMismatch,
                StatusFailureReason::CounterOverflow,
                StatusFailureReason::CounterMismatch,
                StatusFailureReason::MetricsStale,
                StatusFailureReason::ProfileRestricted,
            ] {
                let transport = transport([(503, Vec::new(), None, Some(reason.code()))]);
                let client = client(transport.clone());
                let error = observe(&client, Instant::now() + Duration::from_secs(5), blocking)
                    .await
                    .unwrap_err();
                assert!(matches!(
                    error.downcast_ref::<iroha::Error>(),
                    Some(iroha::Error::StatusUnavailable { reason: Some(actual), .. }) if *actual == reason
                ));
                assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
            }
            for (status, body, reason) in [
                (401, b"unauthorized".to_vec(), None),
                // A recognized code at another HTTP status is not retry authority.
                (429, Vec::new(), Some("status_deadline_elapsed")),
                (429, Vec::new(), Some("status_state_busy")),
                (503, Vec::new(), Some("another_service_unavailable")),
                (503, Vec::new(), Some("status_metrics_unavailable")),
                // Only the SDK's typed header classification is authoritative.
                (
                    503,
                    br#"{"code":"status_deadline_elapsed","message":"busy"}"#.to_vec(),
                    None,
                ),
                (
                    503,
                    br#"{"code":"status_state_busy","message":"busy"}"#.to_vec(),
                    None,
                ),
                (503, b"malformed service error".to_vec(), None),
                (200, b"malformed status".to_vec(), None),
            ] {
                let transport = transport([(status, body, None, reason)]);
                let client = client(transport.clone());
                let error = observe(&client, Instant::now() + Duration::from_secs(5), blocking)
                    .await
                    .unwrap_err();
                match status {
                    200 => assert!(matches!(
                        error.downcast_ref::<iroha::Error>(),
                        Some(iroha::Error::Decode {
                            operation: "diagnostic.status",
                            ..
                        })
                    )),
                    503 => assert!(matches!(
                        error.downcast_ref::<iroha::Error>(),
                        Some(iroha::Error::StatusUnavailable { reason: None, .. })
                    )),
                    _ => assert!(matches!(
                        error.downcast_ref::<iroha::Error>(),
                        Some(iroha::Error::Http { operation: "diagnostic.status", status: actual, .. }) if *actual == status
                    )),
                }
                assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
            }
        }
    }
    #[derive(Debug)]
    struct ConcurrentStatusTransport {
        gate: Arc<tokio::sync::Barrier>,
        calls: Arc<Vec<std::sync::atomic::AtomicUsize>>,
        index: usize,
        budget: Duration,
        pending: bool,
        refusal: bool,
    }

    impl HttpTransport for ConcurrentStatusTransport {
        fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
            panic!("committee batch must use asynchronous reads")
        }

        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move {
                use std::sync::atomic::Ordering::SeqCst;
                assert_eq!(request.method, iroha::http::Method::GET);
                assert_eq!(request.url.path(), "/status");
                assert!(request.body.is_empty());
                assert!(request.timeout.unwrap() <= self.budget);
                assert_eq!(self.calls[self.index].fetch_add(1, SeqCst), 0);
                // A serialized implementation cannot pass this real transport
                // barrier: all voter requests must enter before any can complete.
                self.gate.wait().await;
                if self.pending {
                    return std::future::pending::<Result<Response<Vec<u8>>>>().await;
                }
                if self.refusal {
                    return Ok(Response::builder()
                        .status(503)
                        .header(
                            "x-iroha-reject-code",
                            iroha::StatusFailureReason::CheckpointChanged.code(),
                        )
                        .body(Vec::new())?);
                }
                tokio::time::sleep(Duration::from_millis(
                    u64::try_from(self.calls.len() - self.index).unwrap() * 2,
                ))
                .await;
                Ok(Response::builder()
                    .status(200)
                    .header("content-type", "application/json")
                    .body(json::to_vec(&iroha_torii_shared::status::Status {
                        blocks: 10 + u64::try_from(self.index).unwrap(),
                        ..Default::default()
                    })?)?)
            })
        }
    }

    #[tokio::test]
    async fn concurrent_status_batch_reads_every_seat_and_preserves_roster_order() {
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        for seats in [4, 7] {
            let budget = Duration::from_secs(2);
            let gate = Arc::new(tokio::sync::Barrier::new(seats));
            let calls = Arc::new((0..seats).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
            let clients = (0..seats)
                .map(|index| {
                    client(Arc::new(ConcurrentStatusTransport {
                        gate: Arc::clone(&gate),
                        calls: Arc::clone(&calls),
                        index,
                        budget,
                        pending: false,
                        refusal: false,
                    }))
                })
                .collect::<Vec<_>>();
            let heights = heights_until(&clients, Instant::now() + budget)
                .await
                .unwrap();
            assert_eq!(
                heights,
                (0..seats)
                    .map(|index| 10 + u64::try_from(index).unwrap())
                    .collect::<Vec<_>>()
            );
            assert!(calls.iter().all(|count| count.load(SeqCst) == 1));
        }
    }

    #[tokio::test]
    async fn concurrent_status_batch_keeps_original_deadline_and_exact_peer_refusal() {
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        for refuse in [false, true] {
            let budget = if refuse {
                Duration::from_secs(2)
            } else {
                Duration::from_millis(100)
            };
            let gate = Arc::new(tokio::sync::Barrier::new(4));
            let calls = Arc::new((0..4).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
            let clients = (0..4)
                .map(|index| {
                    client(Arc::new(ConcurrentStatusTransport {
                        gate: Arc::clone(&gate),
                        calls: Arc::clone(&calls),
                        index,
                        budget,
                        pending: index == 2 && !refuse,
                        refusal: index == 2 && refuse,
                    }))
                })
                .collect::<Vec<_>>();
            let deadline = Instant::now() + budget;
            let error = heights_until(&clients, deadline).await.unwrap_err();
            assert!(calls.iter().all(|count| count.load(SeqCst) == 1));
            if refuse {
                assert!(matches!(
                    error.downcast_ref::<iroha::Error>(),
                    Some(iroha::Error::StatusUnavailable {
                        reason: Some(iroha::StatusFailureReason::CheckpointChanged),
                        ..
                    })
                ));
            } else {
                // The SDK dispatch and outer phase timers share this deadline.
                // Either timer can win; preserve the actual typed timeout rather
                // than requiring the outer timer's display text.
                assert!(
                    matches!(
                        error.downcast_ref::<iroha::Error>(),
                        Some(iroha::Error::Timeout {
                            operation: "diagnostic.status",
                        })
                    ) || error
                        .downcast_ref::<tokio::time::error::Elapsed>()
                        .is_some(),
                    "unexpected pending status refusal: {error:#}"
                );
                assert!(Instant::now() >= deadline);
                assert!(Instant::now().duration_since(deadline) < Duration::from_secs(1));
            }
        }
    }

    #[tokio::test]
    async fn concurrent_status_batch_refuses_invalid_rosters_and_expired_deadline_before_dispatch()
    {
        let source = transport([]);
        for seats in [0, 3, 5, 32] {
            let clients = (0..seats)
                .map(|_| client(source.clone()))
                .collect::<Vec<_>>();
            let error = heights_until(&clients, Instant::now() + Duration::from_secs(1))
                .await
                .unwrap_err();
            assert!(error.to_string().contains("exact bounded voter roster"));
        }
        let clients = (0..4).map(|_| client(source.clone())).collect::<Vec<_>>();
        let error = heights_until(&clients, Instant::now()).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("committee status deadline elapsed")
        );
        assert!(source.request_budgets.lock().unwrap().is_empty());
    }
}
