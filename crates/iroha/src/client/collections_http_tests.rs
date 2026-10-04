//! Collection reads: canonical bodies, optional signing, cursor following and typed errors.

use super::{
    Client, HEADER_ACCOUNT, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP_MS, HEADER_WITNESS,
    capability_test_support::AsyncOnlyTransport,
    collections::MAX_PAGE_RESPONSE_BYTES,
    evidence_http_tests::{base_url, client_with_base_url},
};
use crate::{
    ApiError, ApiErrorDetails, Error, blocking,
    collections::{
        Collection, CollectionReader, ListQuery, Page, Pager, SortKey, StreamExt as _,
        TryStreamExt as _, field,
    },
    data_model::account::{AccountAddress, AccountId, MultisigMember, MultisigPolicy},
    http::{Method, Response, TransportRequest},
    http_default::RequestSnapshot,
};
use iroha_torii_shared::ErrorEnvelope;
use norito::json::{self, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, atomic::AtomicUsize},
    time::Duration,
};

type Requests = Arc<Mutex<Vec<TransportRequest>>>;

/// Serve `responses` in order from an async-only transport (blocking sends panic).
fn attach(client: &Client, responses: Vec<Response<Vec<u8>>>) -> (Client, Requests) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let queue = Mutex::new(VecDeque::from(responses));
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(move |_| {
            queue
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| eyre::eyre!("unexpected extra collection request"))
        }),
        requests: requests.clone(),
        completed: Arc::new(AtomicUsize::new(0)),
        delay: Duration::ZERO,
    });
    let client = client
        .to_builder()
        .http_transport(transport)
        .build()
        .expect("collection test client");
    (client, requests)
}

fn public_client(responses: Vec<Response<Vec<u8>>>) -> (Client, Requests) {
    attach(&client_with_base_url(base_url()), responses)
}

fn response(status: u16, media_type: Option<&str>, body: &str) -> Response<Vec<u8>> {
    let mut builder = Response::builder().status(status);
    if let Some(media_type) = media_type {
        builder = builder.header("Content-Type", media_type);
    }
    builder.body(body.as_bytes().to_vec()).unwrap()
}

fn page(items: &[&str], next_cursor: Option<&str>, total: Option<u64>) -> Response<Vec<u8>> {
    let page = Page {
        items: items.iter().map(|id| row(id)).collect(),
        next_cursor: next_cursor.map(ToOwned::to_owned),
        total,
    };
    response(
        200,
        Some("application/json"),
        &json::to_json(&page).expect("page JSON"),
    )
}

fn row(id: &str) -> Value {
    json::parse_value(&format!(r#"{{"id": "{id}", "metadata": {{"tier": 1}}}}"#)).unwrap()
}

fn ids(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| row.get("id").and_then(Value::as_str).unwrap().to_owned())
        .collect()
}

fn header<'a>(request: &'a TransportRequest, name: &str) -> Vec<&'a str> {
    request
        .headers
        .iter()
        .filter(|(header, _)| header.as_str().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.to_str().unwrap())
        .collect()
}

fn body(request: &TransportRequest) -> Value {
    json::parse_value(std::str::from_utf8(&request.body).unwrap()).unwrap()
}

fn assert_unsigned(request: &TransportRequest) {
    for name in [
        HEADER_ACCOUNT,
        HEADER_SIGNATURE,
        HEADER_TIMESTAMP_MS,
        HEADER_NONCE,
        HEADER_WITNESS,
    ] {
        assert!(header(request, name).is_empty(), "public read sent {name}");
    }
}

fn sample_query() -> ListQuery {
    ListQuery::new()
        .filter(field("owned_by").eq("alice") & field("metadata.tier").gte(1))
        .sort_by(SortKey::desc("id"))
        .select(["id", "metadata.tier"])
        .limit(2)
        .include_total()
}

#[tokio::test(flavor = "current_thread")]
async fn public_page_posts_the_canonical_body_without_account_authentication() {
    fn require_send(_: impl Send) {}

    let (client, requests) = public_client(vec![page(&["a", "b"], Some("c1"), Some(3))]);
    let query = sample_query();
    require_send(client.list_page(&Collection::AssetDefinitions, &query));
    let page = client
        .list_page(&Collection::AssetDefinitions, &query)
        .await
        .expect("page");
    assert_eq!(ids(&page.items), ["a", "b"]);
    assert_eq!(page.next_cursor.as_deref(), Some("c1"));
    assert_eq!(page.total, Some(3));
    assert!(page.has_more());

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "one page read is one request");
    let request = &requests[0];
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.url.path(), "/v1/assets/definitions/query");
    assert!(request.url.query().is_none());
    assert_eq!(body(request), query.to_json_value());
    assert_eq!(
        request.body,
        json::to_vec(&query.to_json_value()).unwrap(),
        "the body is the canonical serialization"
    );
    assert_eq!(header(request, "content-type"), ["application/json"]);
    assert_eq!(header(request, "accept"), ["application/json"]);
    assert_eq!(request.max_response_bytes, MAX_PAGE_RESPONSE_BYTES);
    assert_eq!(request.timeout, Some(client.torii_request_timeout));
    assert_unsigned(request);
}

#[tokio::test(flavor = "current_thread")]
async fn account_page_carries_a_verifiable_canonical_signature() {
    let (client, requests) = public_client(vec![page(&["a"], None, None)]);
    let account = client.account_client().expect("account context");
    let target = AccountId::new(client.key_pair.public_key().clone());
    let collection = Collection::AccountAssets(target.clone());
    let page = account
        .list_page(&collection, &ListQuery::new())
        .await
        .expect("signed page");
    assert_eq!(ids(&page.items), ["a"]);
    assert!(!page.has_more());

    let requests = requests.lock().unwrap();
    let request = &requests[0];
    let literal = AccountAddress::from_account_id(&target)
        .and_then(|address| address.to_i105_for_discriminant(client.account_chain_discriminant))
        .unwrap();
    let mut expected = client.torii_url.clone();
    expected
        .path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .extend(["v1", "accounts", literal.as_str(), "assets", "query"]);
    assert_eq!(request.url, expected);
    assert_eq!(body(request), json::parse_value("{}").unwrap());
    super::tests::assert_canonical_account_signed_request(&client, &RequestSnapshot::from(request));
}

#[tokio::test(flavor = "current_thread")]
async fn multisig_member_reads_stay_public() {
    let base = client_with_base_url(base_url());
    let mut builder = base.to_builder();
    builder.account = AccountId::new_multisig(
        MultisigPolicy::new(
            1,
            vec![MultisigMember::new(builder.key_pair.public_key().clone(), 1).unwrap()],
        )
        .unwrap(),
    );
    let (client, requests) = attach(
        &builder.build().expect("multisig member context"),
        vec![page(&[], None, None)],
    );
    let account = client.account_client().expect("member context");
    account
        .list_page(&Collection::Domains, &ListQuery::new())
        .await
        .expect("public page");
    assert_unsigned(&requests.lock().unwrap()[0]);
}

#[tokio::test(flavor = "current_thread")]
async fn list_streams_every_page_lazily_and_reuses_the_query() {
    let (client, requests) = public_client(vec![
        page(&["a", "b"], Some("c1"), Some(5)),
        page(&[], Some("c2"), None),
        page(&["c", "d", "e"], None, None),
    ]);
    let query = sample_query();
    let mut rows = client.list(Collection::Accounts, query.clone());
    assert_eq!(rows.collection(), &Collection::Accounts);
    assert!(
        requests.lock().unwrap().is_empty(),
        "creating a stream sends nothing"
    );
    assert_eq!(rows.next().await.unwrap().unwrap(), row("a"));
    assert_eq!(
        requests.lock().unwrap().len(),
        1,
        "one page per buffer refill"
    );
    assert_eq!(rows.total(), Some(5));
    let rest: Vec<_> = rows.by_ref().try_collect().await.expect("remaining rows");
    assert_eq!(ids(&rest), ["b", "c", "d", "e"]);
    assert!(rows.next().await.is_none());
    assert_eq!(rows.total(), Some(5), "later pages without totals keep it");

    let requests = requests.lock().unwrap();
    let cursors: Vec<_> = requests
        .iter()
        .map(|request| body(request).get("cursor").cloned())
        .collect();
    assert_eq!(
        cursors,
        [None, Some(Value::from("c1")), Some(Value::from("c2"))]
    );
    for (request, cursor) in requests.iter().zip([None, Some("c1"), Some("c2")]) {
        let mut expected = query.clone();
        expected.cursor = cursor.map(ToOwned::to_owned);
        assert_eq!(body(request), expected.to_json_value());
        assert_eq!(request.url.path(), "/v1/accounts/query");
    }
}

fn transaction(hash: &str, block_height: u64, block_index: u64) -> Value {
    json::parse_value(&format!(
        r#"{{"entrypoint_hash": "{hash}", "block_height": {block_height},
            "block_index": {block_index}, "block_hash": "b{block_height}",
            "authority": null, "timestamp_ms": null, "entrypoint_kind": "External",
            "result_ok": true, "asset_ids": [], "asset_definition_ids": [], "metadata": {{}}}}"#
    ))
    .unwrap()
}

fn history_page(rows: Vec<Value>, next_cursor: Option<&str>) -> Response<Vec<u8>> {
    let page = Page {
        items: rows,
        next_cursor: next_cursor.map(ToOwned::to_owned),
        total: None,
    };
    response(
        200,
        Some("application/json"),
        &json::to_json(&page).expect("page JSON"),
    )
}

fn hashes(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            row.get("entrypoint_hash")
                .and_then(Value::as_str)
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn history_reads_follow_short_and_empty_pages_until_the_cursor_ends() {
    let (client, requests) = public_client(vec![
        history_page(vec![transaction("t4", 1500, 1)], Some("h1500_i1")),
        history_page(Vec::new(), Some("h1400_i0")),
        history_page(
            vec![transaction("t3", 1300, 4), transaction("t2", 1200, 0)],
            None,
        ),
    ]);
    let query = ListQuery::new()
        .filter(field("block_height").gte(1_200) & field("result_ok").eq(true))
        .limit(3);
    let rows: Vec<Value> = client
        .list(Collection::Transactions, query.clone())
        .try_collect()
        .await
        .expect("history rows");
    assert_eq!(hashes(&rows), ["t4", "t3", "t2"]);

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "short and empty pages are not the end");
    for (request, cursor) in requests
        .iter()
        .zip([None, Some("h1500_i1"), Some("h1400_i0")])
    {
        assert_eq!(request.url.path(), "/v1/transactions/query");
        let mut expected = query.clone();
        expected.cursor = cursor.map(ToOwned::to_owned);
        assert_eq!(body(request), expected.to_json_value());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn history_restrictions_are_rejected_before_dispatch() {
    let (client, requests) = public_client(Vec::new());
    let account = client.account_client().expect("account context");
    let target = AccountId::new(client.key_pair.public_key().clone());
    let error = account
        .list_page(
            &Collection::AccountTransactions(target),
            &ListQuery::new().include_total(),
        )
        .await
        .expect_err("history total");
    assert!(
        matches!(
            &error,
            Error::InvalidListQuery {
                operation: "collections.account_transactions",
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(error.code(), Some("invalid_include_total"));
    let error = client
        .list_page(
            &Collection::Transactions,
            &ListQuery::new().sort_by(SortKey::asc("block_height")),
        )
        .await
        .expect_err("history sort");
    assert_eq!(error.code(), Some("invalid_sort"));
    let mut rows = client.list(Collection::Transactions, ListQuery::new().include_total());
    assert_eq!(
        rows.next()
            .await
            .expect("validation error")
            .expect_err("rejected")
            .code(),
        Some("invalid_include_total")
    );
    assert!(rows.next().await.is_none());
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn list_ends_after_the_first_error() {
    let (client, requests) = public_client(vec![
        page(&["a"], Some("c1"), None),
        response(
            400,
            Some("application/json"),
            r#"{"code": "invalid_cursor", "message": "invalid `cursor`: stale", "details": {"field": "cursor"}}"#,
        ),
    ]);
    let mut rows = client.list(Collection::Domains, ListQuery::new());
    assert_eq!(rows.next().await.unwrap().unwrap(), row("a"));
    let error = rows.next().await.unwrap().expect_err("second page fails");
    assert_eq!(error.code(), Some("invalid_cursor"));
    assert!(rows.next().await.is_none());
    assert!(futures_util::stream::FusedStream::is_terminated(&rows));
    assert_eq!(
        requests.lock().unwrap().len(),
        2,
        "no request after an error"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_that_does_not_advance_is_rejected() {
    let (client, _) = public_client(vec![
        page(&[], Some("same"), None),
        page(&[], Some("same"), None),
    ]);
    let error = client
        .list(Collection::Nfts, ListQuery::new())
        .try_collect::<Vec<_>>()
        .await
        .expect_err("stalled cursor");
    let Error::ResponseBinding { operation, field } = &error else {
        panic!("unexpected collection error: {error:?}");
    };
    assert_eq!(*operation, "collections.nfts");
    assert_eq!(*field, "next_cursor");
}

#[tokio::test(flavor = "current_thread")]
async fn torii_error_envelopes_are_typed() {
    let (client, _) = public_client(vec![{
        let mut response = response(
            400,
            Some("application/json; charset=utf-8"),
            r#"{"code": "invalid_filter",
                "message": "invalid `filter`: unknown field `colour`",
                "details": {"field": "filter", "expected": "id, owned_by", "actual": "colour",
                            "hint": "use one of the listed fields", "column": 7}}"#,
        );
        response
            .headers_mut()
            .insert("x-iroha-reject-code", "query_rejected".parse().unwrap());
        response
            .headers_mut()
            .insert("retry-after", "3".parse().unwrap());
        response
    }]);
    let error = client
        .list_page(&Collection::Rwas, &ListQuery::new())
        .await
        .expect_err("rejected");
    assert_eq!(error.code(), Some("invalid_filter"));
    assert_eq!(error.http_status(), Some(400));
    let Error::Api { operation, error } = &error else {
        panic!("expected a typed API error, got {error:?}");
    };
    assert_eq!(*operation, "collections.rwas");
    assert_eq!(error.status(), 400);
    assert_eq!(error.message(), "invalid `filter`: unknown field `colour`");
    assert_eq!(error.list_query_control(), Some("filter"));
    assert_eq!(error.reject_code(), Some("query_rejected"));
    assert_eq!(error.retry_after(), Some(Duration::from_secs(3)));
    let details = error.details().expect("details");
    assert_eq!(details.field(), Some("filter"));
    assert_eq!(details.expected(), Some("id, owned_by"));
    assert_eq!(details.actual(), Some("colour"));
    assert_eq!(details.hint(), Some("use one of the listed fields"));
    assert_eq!(details.get("column").and_then(Value::as_u64), Some(7));
    assert_eq!(
        Error::Api {
            operation,
            error: error.clone(),
        }
        .to_string(),
        "collections.rwas returned HTTP 400: invalid_filter: invalid `filter`: unknown field `colour`"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn norito_error_envelopes_are_typed() {
    let envelope = ErrorEnvelope::new("permission_denied", "restricted dataspace");
    let (client, _) = public_client(vec![
        Response::builder()
            .status(403)
            .header("Content-Type", "application/x-norito")
            .body(norito::to_bytes(&envelope).unwrap())
            .unwrap(),
    ]);
    let error = client
        .list_page(&Collection::RepoAgreements, &ListQuery::new())
        .await
        .expect_err("rejected");
    let api = error.api_error().expect("typed envelope");
    assert_eq!(api.status(), 403);
    assert_eq!(api.code(), "permission_denied");
    assert_eq!(api.message(), "restricted dataspace");
    assert!(api.details().is_none());
    assert_eq!(api.list_query_control(), None);
}

#[tokio::test(flavor = "current_thread")]
async fn bodies_without_an_envelope_keep_the_raw_http_error() {
    for (media_type, body) in [
        (Some("text/plain"), "bad gateway"),
        (Some("application/json"), r#"{"error": "not an envelope"}"#),
        (
            Some("application/json"),
            r#"{"code": 7, "message": "wrong types"}"#,
        ),
        (None, "<html>proxy</html>"),
    ] {
        let (client, _) = public_client(vec![response(502, media_type, body)]);
        let error = client
            .list_page(&Collection::Domains, &ListQuery::new())
            .await
            .expect_err("rejected");
        let Error::Http {
            operation,
            status,
            retry_after,
            body: response_body,
        } = &error
        else {
            panic!("unexpected collection error for {media_type:?} {body}: {error:?}");
        };
        assert_eq!(*operation, "collections.domains");
        assert_eq!(*status, 502);
        assert_eq!(*retry_after, None);
        assert_eq!(
            response_body.as_slice(),
            body.as_bytes(),
            "{media_type:?} {body}"
        );
        assert_eq!(error.code(), None);
        assert_eq!(error.http_status(), Some(502));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_pages_are_decode_errors() {
    for (media_type, body) in [
        ("application/json", r#"{"items": 5}"#),
        ("application/json", r#"{"items": [], "next_cursor": 1}"#),
        ("application/x-norito", r#"{"items": []}"#),
    ] {
        let (client, _) = public_client(vec![response(200, Some(media_type), body)]);
        let error = client
            .list_page(&Collection::Domains, &ListQuery::new())
            .await
            .expect_err("malformed page");
        assert!(
            matches!(
                error,
                Error::Decode {
                    operation: "collections.domains",
                    ..
                }
            ),
            "{body}: {error:?}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_queries_are_rejected_before_dispatch() {
    let (client, requests) = public_client(Vec::new());
    for (query, code) in [
        (ListQuery::new().limit(0), "invalid_limit"),
        (ListQuery::new().cursor("not a cursor"), "invalid_cursor"),
        (
            ListQuery::new()
                .sort_by(SortKey::asc("id"))
                .sort_by(SortKey::desc("id")),
            "invalid_sort",
        ),
    ] {
        let error = client
            .list_page(&Collection::Domains, &query)
            .await
            .expect_err("invalid query");
        assert!(
            matches!(
                &error,
                Error::InvalidListQuery {
                    operation: "collections.domains",
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(error.code(), Some(code));
    }
    assert!(requests.lock().unwrap().is_empty());
}

#[test]
fn pager_positions_requests_after_each_page() {
    let client = Arc::new(client_with_base_url(base_url()));
    let query = ListQuery::new().filter(field("id").ne("x")).limit(2);
    let mut pager = Pager::new(
        CollectionReader::Public(client),
        Collection::Domains,
        query.clone(),
    );
    assert!(!pager.is_finished());
    let first = pager.take_request().expect("first request");
    assert_eq!(first, query);
    pager
        .accept(
            &first,
            Page {
                items: vec![row("a")],
                next_cursor: Some("c1".to_owned()),
                total: Some(2),
            },
        )
        .expect("first page");
    assert_eq!(pager.buffered(), 1);
    assert_eq!(pager.pop(), Some(row("a")));
    let second = pager.take_request().expect("second request");
    assert_eq!(second.cursor.as_deref(), Some("c1"));
    assert_eq!(second.filter, query.filter);
    pager
        .accept(&second, Page::last(vec![row("b")]))
        .expect("last page");
    assert_eq!(pager.total(), Some(2));
    assert_eq!(pager.pop(), Some(row("b")));
    assert!(pager.take_request().is_none());
    assert!(pager.is_finished());
}

#[test]
fn api_error_details_tolerate_unknown_members() {
    let mut members = json::Map::new();
    members.insert("expected".to_owned(), Value::from(1_u64));
    members.insert("trace".to_owned(), Value::from("abc"));
    let error = ApiError::new(429, "query_capacity_exceeded", "busy")
        .with_details(ApiErrorDetails::new(members));
    let details = error.details().unwrap();
    assert_eq!(
        details.expected(),
        None,
        "non-string members have no text form"
    );
    assert_eq!(details.get("trace"), Some(&Value::from("abc")));
    assert_eq!(details.members().len(), 2);
    assert_eq!(error.list_query_control(), None);
    for (code, control) in [
        ("invalid_query", "query"),
        ("invalid_filter", "filter"),
        ("invalid_sort", "sort"),
        ("invalid_select", "select"),
        ("invalid_aggregate", "aggregate"),
        ("invalid_limit", "limit"),
        ("invalid_cursor", "cursor"),
        ("invalid_include_total", "include_total"),
    ] {
        assert_eq!(
            ApiError::new(400, code, "").list_query_control(),
            Some(control)
        );
    }
}

#[test]
fn blocking_facade_reads_pages_and_iterates_lazily() {
    let (client, requests) = public_client(vec![
        page(&["a"], Some("c1"), Some(2)),
        page(&["b"], None, None),
        page(&["only"], None, None),
    ]);
    let facade = blocking::Client::from_client(client.clone()).expect("blocking facade");
    let mut rows = facade.list(Collection::AssetDefinitions, ListQuery::new().limit(1));
    assert!(requests.lock().unwrap().is_empty(), "iterators are lazy");
    assert_eq!(rows.next().unwrap().unwrap(), row("a"));
    assert_eq!(rows.total(), Some(2));
    assert_eq!(rows.next().unwrap().unwrap(), row("b"));
    assert!(rows.next().is_none());
    assert!(rows.next().is_none());
    let page = facade
        .list_page(&Collection::AssetDefinitions, &ListQuery::new())
        .expect("blocking page");
    assert_eq!(ids(&page.items), ["only"]);

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    for request in requests.iter() {
        super::tests::assert_canonical_account_signed_request(
            &client,
            &RequestSnapshot::from(request),
        );
    }
}

#[test]
fn blocking_history_iteration_follows_empty_pages() {
    let (client, requests) = public_client(vec![
        history_page(Vec::new(), Some("h9_i0")),
        history_page(vec![transaction("t1", 9, 0)], None),
    ]);
    let facade = blocking::Client::from_client(client).expect("blocking facade");
    let rows = facade
        .list(Collection::Transactions, ListQuery::new())
        .collect::<crate::Result<Vec<_>>>()
        .expect("history rows");
    assert_eq!(hashes(&rows), ["t1"]);
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn blocking_reads_reject_entry_from_an_async_runtime() {
    let (client, requests) = public_client(Vec::new());
    let facade = blocking::Client::from_client(client).expect("blocking facade");
    let error = facade
        .list_page(&Collection::Domains, &ListQuery::new())
        .expect_err("async entry");
    assert!(matches!(error, Error::Blocking(_)), "{error:?}");
    let mut rows = facade.list(Collection::Domains, ListQuery::new());
    assert!(matches!(rows.next(), Some(Err(Error::Blocking(_)))));
    assert!(rows.next().is_none(), "the first error ends the iteration");
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn permission_subscription_and_manifest_collections_share_query_transport() {
    use crate::data_model::nexus::UniversalAccountId;
    let base = client_with_base_url(base_url());
    let uaid = UniversalAccountId::from_hash(iroha_crypto::Hash::new(b"manifest-collection"));
    for collection in [
        Collection::AccountPermissions(base.account.clone()),
        Collection::SubscriptionPlans,
        Collection::Subscriptions,
        Collection::UaidManifests(uaid),
    ] {
        let mut reply = page(&["row"], Some("following-page"), Some(2));
        reply.headers_mut().insert(
            "x-iroha-account-permission-semantics",
            "effective-v1".parse().unwrap(),
        );
        let (client, requests) = attach(&base, vec![reply]);
        let query = ListQuery::new().limit(1).include_total();
        let result = client.list_page(&collection, &query).await.unwrap();
        assert_eq!(result.next_cursor.as_deref(), Some("following-page"));
        assert_eq!(result.total, Some(2));
        let requests = requests.lock().unwrap();
        assert_eq!(requests[0].method, Method::POST);
        assert_eq!(
            requests[0].url,
            collection
                .query_url(&base.torii_url, client.account_chain_discriminant)
                .unwrap()
        );
        assert_eq!(body(&requests[0]), query.to_json_value());
        assert_unsigned(&requests[0]);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn permission_pages_reject_absent_wrong_or_duplicate_effective_semantics() {
    for semantics in [
        vec![],
        vec!["direct-only"],
        vec!["effective-v1", "effective-v1"],
    ] {
        let mut reply = page(&[], None, None);
        for value in semantics {
            reply.headers_mut().append(
                "x-iroha-account-permission-semantics",
                value.parse().unwrap(),
            );
        }
        let (client, requests) = public_client(vec![reply]);
        assert!(matches!(
            client
                .list_page(
                    &Collection::AccountPermissions(client.account.clone()),
                    &ListQuery::new(),
                )
                .await,
            Err(Error::Decode { .. })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}
