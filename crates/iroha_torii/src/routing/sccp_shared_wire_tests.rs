//! Regression coverage for canonical SCCP frames exchanged by Torii and the SDK.

use super::*;
use iroha::http::{HttpTransport, TransportFuture, TransportRequest};
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Debug)]
struct ProducerResponses(Mutex<BTreeMap<&'static str, Vec<u8>>>);

impl HttpTransport for ProducerResponses {
    fn send_blocking(&self, request: TransportRequest) -> eyre::Result<http::Response<Vec<u8>>> {
        assert_eq!(request.method, http::Method::GET);
        assert!(request.headers.iter().any(|(name, value)| {
            name == http::header::ACCEPT && value == "application/x-norito"
        }));
        let body = self
            .0
            .lock()
            .expect("producer response lock")
            .remove(request.url.path())
            .expect("one request for each canonical SCCP endpoint");
        assert!(body.len() <= request.max_response_bytes);
        Ok(http::Response::builder()
            .status(200)
            .header(http::header::CONTENT_TYPE, "application/x-norito")
            .body(body)
            .expect("canonical SCCP HTTP response"))
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[test]
fn sdk_reads_canonical_torii_capability_and_recent_message_frames() {
    let state = CoreState::new_with_chain_for_testing(
        iroha_core::state::World::default(),
        iroha_core::kura::Kura::blank_kura_for_testing(),
        iroha_core::query::store::LiveQueryStore::start_test(),
        iroha_sccp::SCCP_TAIRA_CHAIN_ID_V1
            .parse()
            .expect("Taira chain id"),
    );
    let capabilities = sccp_capabilities_snapshot(&state);
    let recent = collect_recent_sccp_messages(
        &state,
        &SccpRecentWindowQuery {
            from: None,
            after_index: None,
            limit: Some(1),
        },
    )
    .expect("empty authoritative SCCP discovery snapshot");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("response collection runtime");
    let mut responses = BTreeMap::new();
    let mut different_identity_responses = BTreeMap::new();
    for (path, response, expected_name, rejected_name) in [
        (
            "/v1/sccp/capabilities",
            sccp_bundle_response_with_format(&capabilities, crate::utils::ResponseFormat::Norito),
            "iroha_torii::routing::SccpCapabilitiesDto",
            "iroha::client::SccpCapabilities",
        ),
        (
            "/v1/sccp/messages/recent",
            sccp_bundle_response_with_format(&recent, crate::utils::ResponseFormat::Norito),
            "iroha_torii::routing::SccpRecentMessagesDto",
            "iroha::client::SccpRecentMessages",
        ),
    ] {
        let response = response.expect("Torii SCCP response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = runtime
            .block_on(axum::body::to_bytes(response.into_body(), 1024 * 1024))
            .expect("bounded response body")
            .to_vec();
        let header =
            norito::core::Header::read(bytes.as_slice()).expect("canonical producer header");
        assert_eq!(
            header.schema,
            norito::core::schema_hash_for_name(expected_name)
        );
        let mut different_identity = bytes.clone();
        // Change only the fixed header's schema bytes, after magic and version.
        different_identity[6..22]
            .copy_from_slice(&norito::core::schema_hash_for_name(rejected_name));
        different_identity_responses.insert(path, different_identity);
        responses.insert(path, bytes);
    }
    drop(runtime);
    let transport = Arc::new(ProducerResponses(Mutex::new(responses)));
    let key_pair = checked_routing_fixture_keypair(
        0x6c,
        iroha_crypto::Algorithm::Ed25519,
        "derive canonical SCCP SDK fixture key",
    );
    let config = iroha::config::Config {
        chain: state.chain_id_ref().clone(),
        network_id: iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xa5; 32])),
        ),
        account: iroha_data_model::account::AccountId::new(key_pair.public_key().clone()),
        account_chain_discriminant: iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT,
        key_pair,
        basic_auth: None,
        torii_api_url: "http://127.0.0.1:8080/".parse().expect("fixture Torii URL"),
        torii_request_timeout: Duration::from_secs(1),
        transaction_ttl: Duration::from_secs(5),
        transaction_status_timeout: Duration::from_secs(5),
        transaction_add_nonce: false,
        sorafs_alias_cache: sorafs_manifest::alias_cache::AliasCachePolicy::new(
            Duration::from_secs(60),
            Duration::from_secs(30),
            Duration::from_secs(120),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(60),
            Duration::from_secs(60),
        ),
        sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::Canary,
    };
    let client = iroha::client::Client::builder(config)
        .http_transport(transport.clone())
        .build()
        .expect("canonical SCCP SDK client");
    assert_eq!(
        client
            .get_sccp_capabilities()
            .expect("decode Torii capabilities"),
        capabilities,
    );
    assert_eq!(
        client
            .get_sccp_recent_messages()
            .expect("decode Torii recent messages"),
        recent,
    );
    assert!(
        transport
            .0
            .lock()
            .expect("producer response lock")
            .is_empty()
    );
    *transport.0.lock().expect("producer response lock") = different_identity_responses;
    let error = client
        .get_sccp_capabilities()
        .expect_err("the retired SDK capability identity is not a wire variant");
    assert!(matches!(
        error.downcast_ref::<norito::Error>(),
        Some(norito::Error::SchemaMismatch)
    ));
    let error = client
        .get_sccp_recent_messages()
        .expect_err("the retired SDK recent-message identity is not a wire variant");
    assert!(matches!(
        error.downcast_ref::<norito::Error>(),
        Some(norito::Error::SchemaMismatch)
    ));
    assert!(
        transport
            .0
            .lock()
            .expect("producer response lock")
            .is_empty()
    );
}
