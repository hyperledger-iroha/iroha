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

    fn client(transport: Arc<StatusTransport>) -> iroha::client::Client {
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
}
