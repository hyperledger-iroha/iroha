//! Exercise the case's actual SDK read with scripted Torii responses.

use super::*;
use iroha::http::{HttpTransport, Method, Response, TransportFuture, TransportRequest};
use iroha_torii_shared::{ErrorDetails, ErrorEnvelope};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
struct CommitteeTransport {
    responses: Mutex<VecDeque<Response<Vec<u8>>>>,
    timeouts: Mutex<Vec<Duration>>,
}

impl HttpTransport for CommitteeTransport {
    fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
        panic!("the SDK committee GET uses asynchronous dispatch")
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            assert_eq!(request.method, Method::GET);
            assert_eq!(request.url.path(), "/v1/sccp/committees/2");
            assert!(request.url.query().is_none() && request.body.is_empty());
            self.timeouts.lock().unwrap().push(request.timeout.unwrap());
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected extra request"))
        })
    }
}

fn client_for(responses: Vec<Response<Vec<u8>>>) -> Result<(Client, Arc<CommitteeTransport>)> {
    let config = iroha::config::Config {
        chain: "sccp-committee-read-control".into(),
        network_id: NetworkId::from_genesis_hash(iroha::crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"sccp-committee-read-control"),
        )),
        key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
        account: ALICE_ID.clone(),
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        http_transport_policy: iroha::config::HttpTransportPolicy::Ordinary,
        torii_api_url: "http://committee.invalid/".parse()?,
        torii_request_timeout: Duration::from_secs(10),
        basic_auth: None,
        api_token: None,
        transaction_add_nonce: false,
        transaction_ttl: Duration::from_secs(5),
        transaction_status_timeout: Duration::from_secs(10),
        sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
        sorafs_anonymity_policy: Default::default(),
        sorafs_rollout_phase: Default::default(),
    };
    let transport = Arc::new(CommitteeTransport {
        responses: Mutex::new(responses.into()),
        timeouts: Mutex::default(),
    });
    Ok((
        Client::with_http_transport(config, transport.clone())?,
        transport,
    ))
}

fn refusal(status: u16, code: &str, message: &str) -> Result<Response<Vec<u8>>> {
    let envelope = ErrorEnvelope::new(code, message).with_details(ErrorDetails {
        retry_after_seconds: Some(1),
        ..Default::default()
    });
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Retry-After", "1")
        .body(norito::json::to_vec(&envelope)?)?)
}

#[test]
fn generation_two_busy_repeats_only_its_get_within_the_original_deadline() -> Result<()> {
    let generation = SccpGenerationV1 {
        generation: 2,
        committee_root: [1; 32],
        keys: vec![[2; 48]],
        members: vec![None],
        cause: SccpGenerationCauseV1::KeyChange,
        start_height: 129,
        start_timestamp_ms: 5,
        start_lc: 2,
        validity_ms: 100,
        deadline_ms: 105,
        grace_ms: 10,
        progress_cap_ms: 3,
        liable_until_lc: 112,
        dest_cleared_lc: None,
        end: None,
        liable: true,
    };
    let success = Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(norito::json::to_vec(&generation)?)?;
    let (client, transport) = client_for(vec![
        refusal(
            503,
            "sccp_state_busy",
            "The SCCP committed state is temporarily busy.",
        )?,
        success,
    ])?;
    assert_eq!(read_sccp_committee(&client, 2)?, generation);
    let timeouts = transport.timeouts.lock().unwrap();
    assert_eq!(timeouts.len(), 2);
    assert!(timeouts[0] <= client.client().torii_request_timeout());
    assert!(timeouts[1] > Duration::ZERO && timeouts[1] < timeouts[0]);
    Ok(())
}

#[test]
fn missing_pruned_and_unavailable_generations_do_not_repeat_the_get() -> Result<()> {
    for (status, code) in [
        (404, "sccp_not_found"),
        (410, "sccp_pruned"),
        (503, "sccp_unavailable"),
        (503, "sccp_capacity"),
    ] {
        let response = refusal(status, code, "retained source refused the read")?;
        let body = response.body().clone();
        let (client, transport) = client_for(vec![response])?;
        let failure = read_sccp_committee(&client, 2).unwrap_err();
        assert!(
            matches!(failure.downcast_ref::<iroha::Error>(), Some(iroha::Error::Http {
            operation: "sccp.committee.read", status: actual, body: retained, ..
        }) if *actual == status && *retained == body)
        );
        assert_eq!(transport.timeouts.lock().unwrap().len(), 1);
    }
    Ok(())
}
