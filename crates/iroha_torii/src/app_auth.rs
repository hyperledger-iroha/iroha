//! Canonical request signing helpers for app-facing HTTP endpoints.
//!
//! Clients may optionally attach:
//! - `X-Iroha-Account`: account id that authorises the request.
//! - `X-Iroha-Signature`: base64 signature over the canonical request bytes.
//!
//! The canonical request bytes are:
//! ```text
//! <UPPERCASE_METHOD>\n
//! <path>\n
//! <sorted_query_string>\n
//! <hex_sha256(body)>
//! ```
//! - Query parameters are parsed, percent-decoded (treating `+` as space), sorted
//!   by `(key, value)`, then re-encoded using `application/x-www-form-urlencoded`
//!   rules.
//! - The body hash is computed over the raw request body bytes.

use std::sync::Arc;

use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_core::state::{State as CoreState, WorldReadOnly};
use iroha_crypto::Signature;
use iroha_data_model::{
    ValidationFail,
    account::{AccountController, AccountId},
    query::{
        ErasedIterQuery, Query, QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
        dsl::{CompoundPredicate, HasProjection, PredicateMarker, SelectorMarker, SelectorTuple},
        error::{FindError, QueryExecutionFail},
        parameters::QueryParams,
    },
};
use sha2::{Digest as _, Sha256};

/// Header carrying the authorising account id.
pub const HEADER_ACCOUNT: &str = "X-Iroha-Account";
/// Header carrying the base64-encoded signature over the canonical request bytes.
pub const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
/// HTTP request types used for canonical signing.
pub use axum::http::{Method, Uri};

/// Canonicalise a raw query string by decoding, sorting, and re-encoding.
#[must_use]
pub fn canonical_query_string(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        serializer.append_pair(&k, &v);
    }
    serializer.finish()
}

/// Construct canonical request bytes for signing.
#[must_use]
pub fn canonical_request_message(method: &Method, uri: &Uri, body: &[u8]) -> Vec<u8> {
    let query = canonical_query_string(uri.query());
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash = hasher.finalize();
    format!(
        "{}\n{}\n{}\n{}",
        method.as_str().to_ascii_uppercase(),
        uri.path(),
        query,
        hex::encode(body_hash)
    )
    .into_bytes()
}

/// Encode a signature payload for use in `X-Iroha-Signature` headers.
#[must_use]
pub fn signature_header_value(signature: &Signature) -> String {
    BASE64_STANDARD.encode(signature.payload())
}

/// Validate an iterable query against the executor on behalf of `authority`.
pub fn validate_iter_query_for_authority<Q>(
    state: &Arc<CoreState>,
    authority: &AccountId,
    query: Q,
) -> Result<(), crate::Error>
where
    Q: Query + 'static,
    Q::Item:
        HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
    Q: norito::codec::Encode,
{
    use iroha_core::smartcontracts::isi::query::{QueryLimits, ValidQueryRequest};

    let payload = norito::codec::Encode::encode(&query);
    let qbox: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Q::Item>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        payload,
    ));
    let iter = QueryWithParams::new(&qbox, QueryParams::default());
    let request = QueryRequest::Start(iter);
    let limits = QueryLimits::new(crate::routing::app_query_limits().max_fetch_size);
    let world = state.world_view();
    let latest_block = state.latest_block_header_fast();
    ValidQueryRequest::validate_for_client_world_parts(
        request,
        authority,
        &world,
        latest_block,
        limits,
    )
    .map(|_| ())
    .map_err(crate::Error::Query)
}

/// Verify optional canonical request headers.
///
/// Returns `Ok(Some(account))` when a signature is present and valid, `Ok(None)` when
/// no signing headers are provided, and an error when headers are malformed or verification fails.
pub fn verify_canonical_request(
    state: &Arc<CoreState>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    expected_account: Option<&AccountId>,
) -> Result<Option<AccountId>, crate::Error> {
    let account_hdr = headers.get(HEADER_ACCOUNT);
    let signature_hdr = headers.get(HEADER_SIGNATURE);
    let (Some(account_hdr), Some(signature_hdr)) = (account_hdr, signature_hdr) else {
        // Require both or neither: treat partial presence as an error.
        return match (account_hdr, signature_hdr) {
            (None, None) => Ok(None),
            _ => Err(crate::Error::Query(ValidationFail::NotPermitted(
                "both X-Iroha-Account and X-Iroha-Signature must be set together".to_owned(),
            ))),
        };
    };

    let account_literal = account_hdr.to_str().map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "X-Iroha-Account must be valid UTF-8".to_owned(),
        ))
    })?;
    let account: AccountId = AccountId::parse_encoded(account_literal.trim())
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Account value".to_owned(),
            ))
        })?;

    if let Some(expected) = expected_account {
        if expected != &account {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "signed account does not match request path".to_owned(),
            )));
        }
    }

    let signature_b64 = signature_hdr.to_str().map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "X-Iroha-Signature must be valid UTF-8".to_owned(),
        ))
    })?;
    let signature_bytes = BASE64_STANDARD.decode(signature_b64.trim()).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "invalid base64 in X-Iroha-Signature".to_owned(),
        ))
    })?;
    let signature = Signature::from_bytes(&signature_bytes);
    let message = canonical_request_message(method, uri, body);

    let world = state.world_view();
    let account_entry = world.account(&account).map_err(|_| {
        crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(account.clone()),
        )))
    })?;

    let controller = account_entry.id.controller();
    let allowed_keys: Vec<_> = match controller {
        AccountController::Single(pk) => vec![pk.clone()],
        AccountController::Multisig(policy) => policy
            .members()
            .iter()
            .map(|member| member.public_key().clone())
            .collect(),
    };

    let ok = allowed_keys
        .iter()
        .any(|pk| signature.verify(pk, &message).is_ok());
    if !ok {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "query signature failed verification".to_owned(),
        )));
    }

    Ok(Some(account))
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use axum::http::Uri;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute as _,
        state::{State, StateReadOnly, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        Registrable, account::Account, domain::Domain, isi::Register, prelude::DomainId,
    };
    use nonzero_ext::nonzero;

    use super::*;

    const TEST_ACCOUNT_I105: &str = "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw";

    fn minimal_state_with_account(account: &AccountId) -> Arc<State> {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(account);
        let account_value = Account::new(account.to_account_id(domain_id)).build(account);
        Arc::new(State::new_for_testing(
            World::with([domain], [account_value], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }

    #[test]
    fn canonical_query_sorting_is_stable() {
        let raw = "b=2&a=3&b=1&space=a+b";
        let canonical = canonical_query_string(Some(raw));
        assert_eq!(canonical, "a=3&b=1&b=2&space=a+b");
    }

    #[test]
    fn canonical_message_includes_body_hash() {
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=5")
            .parse()
            .expect("uri");
        let msg = canonical_request_message(&Method::GET, &uri, b"{\"foo\":1}");
        let rendered = String::from_utf8(msg).expect("utf8");
        assert!(rendered.contains(&format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets")));
        assert!(rendered.contains("limit=5"));
        assert!(
            rendered.ends_with("37a76343c8e3c695feeaadfe52329673ff129c65f99f55ae6056c9254f4c481d")
        );
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let kp = KeyPair::random();
        let account: AccountId = AccountId::new(kp.public_key().clone());
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let message = canonical_request_message(&method, &uri, &[]);
        let signature = Signature::new(kp.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_ACCOUNT, account.to_string().parse().unwrap());
        headers.insert(
            HEADER_SIGNATURE,
            BASE64_STANDARD.encode(signature.payload()).parse().unwrap(),
        );

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                .expect("verify");
        assert_eq!(verified, Some(account));
    }

    #[test]
    fn verify_rejects_wrong_signature() {
        let kp = KeyPair::random();
        let account: AccountId = AccountId::new(kp.public_key().clone());
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let bad_sig = Signature::new(KeyPair::random().private_key(), b"forged");
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_ACCOUNT, account.to_string().parse().unwrap());
        headers.insert(
            HEADER_SIGNATURE,
            BASE64_STANDARD.encode(bad_sig.payload()).parse().unwrap(),
        );

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signature"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn verify_rejects_mismatched_path_account() {
        let kp = KeyPair::random();
        let account: AccountId = AccountId::new(kp.public_key().clone());
        let other: AccountId = AccountId::new(KeyPair::random().public_key().clone());
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let message = canonical_request_message(&method, &uri, &[]);
        let signature = Signature::new(kp.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_ACCOUNT, account.to_string().parse().unwrap());
        headers.insert(
            HEADER_SIGNATURE,
            BASE64_STANDARD.encode(signature.payload()).parse().unwrap(),
        );

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&other))
            .unwrap_err();
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signed account does not match request path"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
