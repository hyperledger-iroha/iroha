#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! App API canonical request auth smoke test: GET + POST endpoints accept
//! `X-Iroha-Account`/`X-Iroha-Signature` headers signed over the canonical
//! method/path/query/body envelope.

use eyre::Result;
use integration_tests::sandbox;
use iroha_crypto::Signature;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use iroha_torii::{
    HEADER_ACCOUNT, HEADER_SIGNATURE, Method, Uri, canonical_request_message,
    filter::{Pagination, QueryEnvelope},
    signature_header_value,
};
use norito::json::Value as JsonValue;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

#[test]
fn app_api_accepts_canonical_headers_for_get_and_post() -> Result<()> {
    let builder = NetworkBuilder::new();
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(app_api_accepts_canonical_headers_for_get_and_post),
    )?
    else {
        return Ok(());
    };

    let peer = network
        .peers()
        .first()
        .ok_or_else(|| eyre::eyre!("no peers available"))?;
    let http = reqwest::Client::new();
    let account_literal = ALICE_ID.to_string();

    // GET /v2/accounts/{account}/assets with canonical headers
    let assets_endpoint = format!(
        "{}/v2/accounts/{}/assets?limit=1",
        peer.torii_url(),
        account_literal
    );
    let assets_uri: Uri = assets_endpoint.parse()?;
    let assets_msg = canonical_request_message(&Method::GET, &assets_uri, &[]);
    let assets_sig = Signature::new(ALICE_KEYPAIR.private_key(), &assets_msg);
    let mut assets_headers = HeaderMap::new();
    assets_headers.insert(
        HeaderName::from_bytes(HEADER_ACCOUNT.as_bytes())?,
        HeaderValue::from_str(&account_literal)?,
    );
    assets_headers.insert(
        HeaderName::from_bytes(HEADER_SIGNATURE.as_bytes())?,
        HeaderValue::from_str(&signature_header_value(&assets_sig))?,
    );

    let assets_body: String = rt.block_on(async {
        http.get(&assets_endpoint)
            .headers(assets_headers)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    })?;
    let assets_json: JsonValue = norito::json::from_str(&assets_body)?;
    let total_assets = assets_json
        .get("total")
        .and_then(JsonValue::as_u64)
        .unwrap_or_default();
    assert!(
        total_assets > 0,
        "assets endpoint should accept canonical auth"
    );

    // POST /v2/accounts/{account}/transactions/query with canonical headers + body hash
    let envelope = QueryEnvelope {
        pagination: Pagination {
            limit: Some(4),
            offset: 0,
        },
        ..QueryEnvelope::default()
    };
    let body = norito::json::to_vec(&envelope)?;

    let tx_url = format!(
        "{}/v2/accounts/{}/transactions/query",
        peer.torii_url(),
        account_literal
    );
    let tx_uri: Uri = tx_url.parse()?;
    let tx_msg = canonical_request_message(&Method::POST, &tx_uri, &body);
    let tx_sig = Signature::new(ALICE_KEYPAIR.private_key(), &tx_msg);

    let mut tx_headers = HeaderMap::new();
    tx_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    tx_headers.insert(
        HeaderName::from_bytes(HEADER_ACCOUNT.as_bytes())?,
        HeaderValue::from_str(&account_literal)?,
    );
    tx_headers.insert(
        HeaderName::from_bytes(HEADER_SIGNATURE.as_bytes())?,
        HeaderValue::from_str(&signature_header_value(&tx_sig))?,
    );

    let tx_body: String = rt.block_on(async {
        http.post(&tx_url)
            .headers(tx_headers)
            .body(body)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    })?;
    let tx_json: JsonValue = norito::json::from_str(&tx_body)?;
    assert!(
        tx_json.get("items").is_some(),
        "transactions query should parse with canonical auth"
    );

    Ok(())
}
