//! Functions and types to make queries to the Iroha peer.
#![allow(clippy::result_large_err)]
use crate::{
    client::{APPLICATION_NORITO, Client, QueryResult, ResponseReport, join_torii_url},
    crypto::{HashOf, KeyPair},
    data_model::{
        NetworkId, ValidationFail,
        account::AccountId,
        query::{
            CommittedTransaction, CommittedTxFilters, Query, QueryOutput, QueryRequest,
            QueryResponse, QueryWithParams, SingularQuery, SingularQueryBox,
            SingularQueryOutputBox,
            builder::{QueryBuilder, QueryExecutor},
            dsl::{CompoundPredicate, SelectorTuple},
            error::QueryExecutionFail,
            parameters::{DEFAULT_FETCH_SIZE, ForwardCursor, MAX_FETCH_SIZE, QueryParams},
            transaction::prelude::FindTransactions,
        },
        transaction::TransactionEntrypoint,
    },
    http::{Method as HttpMethod, RequestBuilder},
    http_default::DefaultRequestBuilder,
};
use eyre::{Report, Result, eyre};
use http::{StatusCode, header::CONTENT_TYPE};
use iroha_data_model::query::QueryOutputBatchBoxTuple;
use iroha_torii_shared::{PipelineTransactionDetailsResponse, uri as torii_uri};
use iroha_version::codec::EncodeVersioned;
use norito::{codec::Encode as _, json};
use std::{
    collections::HashMap,
    fmt::Debug,
    num::NonZeroU64,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

const TRANSACTION_DETAILS_RESPONSE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct ClientQueryRequestHead {
    torii_url: Url,
    headers: HashMap<String, String>,
    network_id: NetworkId,
    account_id: AccountId,
    key_pair: KeyPair,
    request_timeout: Duration,
    accept_header: &'static str,
}
impl ClientQueryRequestHead {
    #[cfg(test)]
    fn assemble(&self, query: QueryRequest) -> Result<DefaultRequestBuilder, QueryError> {
        let body = self.sign_and_encode(query)?;
        Ok(self.assemble_body(body))
    }
    fn assemble_body(&self, body: Vec<u8>) -> DefaultRequestBuilder {
        DefaultRequestBuilder::new(
            HttpMethod::POST,
            join_torii_url(&self.torii_url, torii_uri::QUERY),
        )
        .headers(self.headers.clone())
        .header("Content-Type", APPLICATION_NORITO)
        // Prefer canonical Norito responses to avoid JSON decoding drift between
        // client/server versions.
        .header("Accept", self.accept_header)
        .timeout(self.request_timeout)
        .body(body)
    }
    fn assemble_body_with_accept(
        &self,
        body: Vec<u8>,
        accept: &'static str,
    ) -> DefaultRequestBuilder {
        DefaultRequestBuilder::new(
            HttpMethod::POST,
            join_torii_url(&self.torii_url, torii_uri::QUERY),
        )
        .headers(self.headers.clone())
        .header("Content-Type", APPLICATION_NORITO)
        .header("Accept", accept)
        .timeout(self.request_timeout)
        .body(body)
    }
    fn assemble_canonical_norito_body_at(
        &self,
        body: Vec<u8>,
        path: &'static str,
    ) -> DefaultRequestBuilder {
        let mut headers = self.headers.clone();
        headers.retain(|name, _| {
            !name.eq_ignore_ascii_case("accept") && !name.eq_ignore_ascii_case("content-type")
        });
        DefaultRequestBuilder::new(HttpMethod::POST, join_torii_url(&self.torii_url, path))
            .headers(headers)
            .header("Content-Type", APPLICATION_NORITO)
            .header("Accept", APPLICATION_NORITO)
            .timeout(self.request_timeout)
            .body(body)
    }
    fn sign_and_encode(&self, query: QueryRequest) -> Result<Vec<u8>, QueryError> {
        let creation_time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| QueryError::Other(eyre!("system clock precedes Unix epoch: {error}")))?
            .as_millis()
            .try_into()
            .map_err(|_| QueryError::Other(eyre!("query creation time exceeds u64")))?;
        let time_to_live_ms = NonZeroU64::new(
            crate::config::DEFAULT_QUERY_TIME_TO_LIVE
                .as_millis()
                .try_into()
                .map_err(|_| QueryError::Other(eyre!("query TTL exceeds u64")))?,
        )
        .ok_or_else(|| QueryError::Other(eyre!("query TTL must be nonzero")))?;
        let mut nonce = [0_u8; 32];
        for _ in 0..16 {
            rand::rand_core::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut nonce)
                .map_err(|error| QueryError::Other(eyre!("query nonce OS RNG failed: {error}")))?;
            if nonce != [0_u8; 32] {
                break;
            }
        }
        if nonce == [0_u8; 32] {
            return Err(QueryError::Other(eyre!(
                "query nonce OS RNG repeatedly returned the forbidden all-zero value"
            )));
        }
        let with_auth = query.with_authority(
            self.network_id,
            self.account_id.clone(),
            creation_time_ms,
            time_to_live_ms,
            nonce,
        );
        let query = with_auth
            .try_sign(&self.key_pair)
            .map_err(|err| QueryError::Other(eyre!("failed to sign query request: {err}")))?;
        Ok(query.encode_versioned())
    }
}
/// Decode a raw response from the node's query endpoint
fn decode_query_response(resp: &http::Response<Vec<u8>>) -> QueryResult<QueryResponse> {
    match resp.status() {
        StatusCode::OK => {
            let body = resp.body();
            let is_json = resp
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|ct| {
                    let media_type = ct.split(';').next().map(str::trim).unwrap_or_default();
                    let media_type_lower = media_type.to_ascii_lowercase();
                    media_type.eq_ignore_ascii_case("application/json")
                        || (media_type_lower.starts_with("application/")
                            && media_type_lower.ends_with("+json"))
                });
            if is_json {
                return json::from_slice::<QueryResponse>(body).map_err(|error| {
                    eyre!("Failed to decode JSON query response: {error}").into()
                });
            }
            decode_query_response_body(body)
        }
        StatusCode::BAD_REQUEST
        | StatusCode::UNAUTHORIZED
        | StatusCode::FORBIDDEN
        | StatusCode::GONE
        | StatusCode::NOT_FOUND
        | StatusCode::UNPROCESSABLE_ENTITY => {
            let body = resp.body();
            match norito::decode_from_bytes::<ValidationFail>(body) {
                Ok(fail) => Err(QueryError::Validation(fail)),
                Err(decode_err) => {
                    if resp.status() == StatusCode::GONE {
                        return Err(QueryError::Validation(ValidationFail::QueryFailed(
                            QueryExecutionFail::Expired,
                        )));
                    }
                    if resp.status() == StatusCode::NOT_FOUND {
                        return Err(QueryError::Validation(ValidationFail::QueryFailed(
                            QueryExecutionFail::NotFound,
                        )));
                    }
                    let report = ResponseReport::with_msg("Query failed", resp).map_or_else(
                        |_| {
                            Report::new(decode_err).wrap_err(
                                "Failed to decode response from Iroha. \
                                Response is neither a `ValidationFail` encoded value nor a valid utf-8 string error response. \
                                You are likely using a version of the client library that is incompatible with the version of the peer software",
                            )
                        },
                        Into::into,
                    );
                    Err(QueryError::Other(report))
                }
            }
        }
        _ => Err(ResponseReport::with_msg("Unexpected query response", resp)
            .unwrap_or_else(core::convert::identity)
            .into()),
    }
}
/// Decode `QueryResponse` from a canonical Norito byte body.
fn decode_query_response_body(body: &[u8]) -> QueryResult<QueryResponse> {
    norito::decode_from_bytes::<QueryResponse>(body).map_err(|error| {
        Report::new(error)
            .wrap_err(
                "Failed to decode response from Iroha. You are likely using a version of the client library that is incompatible with the version of the peer software",
            )
            .into()
    })
}
fn send_once<F>(mut make_request: F) -> Result<http::Response<Vec<u8>>, QueryError>
where
    F: FnMut() -> Result<DefaultRequestBuilder, QueryError>,
{
    make_request().and_then(|builder| {
        builder
            .build()
            .map_err(QueryError::from)
            .and_then(|request| request.send().map_err(QueryError::from))
    })
}
/// Send a signed query exactly once and decode its response.
///
/// A transport or response-decode failure is deliberately ambiguous: the node may already have
/// consumed and executed the signed nonce. Retrying the same bytes would violate one-shot request
/// semantics, so callers receive the error and may issue a newly signed query if appropriate.
fn send_once_and_decode<F, D, T>(make_request: F, decode: D) -> Result<T, QueryError>
where
    F: FnMut() -> Result<DefaultRequestBuilder, QueryError>,
    D: Fn(&http::Response<Vec<u8>>) -> QueryResult<T>,
{
    let response = send_once(make_request)?;
    decode(&response)
}
fn decode_singular_query_response(
    resp: &http::Response<Vec<u8>>,
) -> QueryResult<SingularQueryOutputBox> {
    let QueryResponse::Singular(resp) = decode_query_response(resp)? else {
        return Err(eyre!(
            "Got unexpected type of query response from the node (expected singular)"
        )
        .into());
    };
    Ok(resp)
}
fn decode_iterable_query_response(resp: &http::Response<Vec<u8>>) -> QueryResult<QueryOutput> {
    let QueryResponse::Iterable(resp) = decode_query_response(resp)? else {
        return Err(eyre!(
            "Got unexpected type of query response from the node (expected iterable)"
        )
        .into());
    };
    Ok(resp)
}
/// Ensure the requested fetch size respects client-side limits.
fn validate_fetch_size(fetch_size: NonZeroU64) -> QueryResult<()> {
    if fetch_size > MAX_FETCH_SIZE {
        return Err(ValidationFail::QueryFailed(QueryExecutionFail::FetchSizeTooBig).into());
    }
    Ok(())
}

fn exact_transaction_details_query(
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> QueryWithParams {
    let query = FindTransactions::new();
    let predicate = CompoundPredicate::from_filters(CommittedTxFilters {
        entry_eq: Some(entrypoint_hash),
        ..CommittedTxFilters::default()
    });
    QueryWithParams {
        query: (),
        query_payload: query.dyn_encode(),
        item: query.query_item_kind(),
        predicate_bytes: predicate.encode(),
        selector_bytes: SelectorTuple::<CommittedTransaction>::default().encode(),
        params: QueryParams::default(),
    }
}
/// An iterable query cursor for use in the client
#[derive(Debug)]
pub struct QueryCursor {
    // instead of storing iroha client itself, we store the base URL and headers required to make a request
    //   along with the account id and key pair to sign the request.
    // this removes the need to either keep a reference or use an Arc, but breaks abstraction a little
    request_head: ClientQueryRequestHead,
    cursor: ForwardCursor,
}
impl QueryCursor {
    /// Return the underlying Iroha forward cursor.
    pub fn forward_cursor(&self) -> &ForwardCursor {
        &self.cursor
    }
}
/// Different errors as a result of query response handling
#[derive(Debug, thiserror::Error, displaydoc::Display)]
pub enum QueryError {
    /// Query validation error
    Validation(#[from] ValidationFail),
    /// Iterable query response has an invalid batch shape: {0}
    ResponseShape(#[from] iroha_data_model::query::builder::TypedBatchDowncastError),
    /// Other error
    Other(#[from] eyre::Error),
}
impl From<ResponseReport> for QueryError {
    #[inline]
    fn from(ResponseReport(err): ResponseReport) -> Self {
        Self::Other(err)
    }
}
impl QueryExecutor for Client {
    type Cursor = QueryCursor;
    type Error = QueryError;
    fn execute_singular_query(
        &self,
        query: SingularQueryBox,
    ) -> Result<SingularQueryOutputBox, Self::Error> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let is_parameters_query = matches!(query, SingularQueryBox::FindParameters(_));
        let request_head = self.get_query_request_head();
        let request = QueryRequest::Singular(query);
        let body = request_head.sign_and_encode(request)?;
        let make_request = || {
            if is_parameters_query {
                Ok(request_head.assemble_body_with_accept(body.clone(), "application/json"))
            } else {
                Ok(request_head.assemble_body(body.clone()))
            }
        };
        send_once_and_decode(make_request, decode_singular_query_response)
    }
    fn start_query(
        &self,
        query: QueryWithParams,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let requested_fetch_size = query
            .params
            .fetch_size
            .fetch_size
            .unwrap_or(DEFAULT_FETCH_SIZE);
        validate_fetch_size(requested_fetch_size)?;
        let request_head = self.get_query_request_head();
        let request = QueryRequest::Start(query);
        let body = request_head.sign_and_encode(request)?;
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        let response = send_once_and_decode(make_request, decode_iterable_query_response)?;
        let (batch, remaining_items, _has_more, cursor) = response.into_parts_with_count_mode();
        let cursor = cursor.map(|cursor| QueryCursor {
            request_head,
            cursor,
        });
        Ok((batch, remaining_items, cursor))
    }
    fn continue_query(
        cursor: Self::Cursor,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error> {
        let QueryCursor {
            request_head,
            cursor,
        } = cursor;
        let request = QueryRequest::Continue(cursor);
        let body = request_head.sign_and_encode(request)?;
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        let response = send_once_and_decode(make_request, decode_iterable_query_response)?;
        let (batch, remaining_items, _has_more, cursor) = response.into_parts_with_count_mode();
        let cursor = cursor.map(|cursor| QueryCursor {
            request_head,
            cursor,
        });
        Ok((batch, remaining_items, cursor))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::query::{SignedQuery, executor::prelude::FindExecutorDataModel};
    use iroha_version::codec::DecodeVersioned as _;
    use std::sync::Arc;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked query fixture keypair")
    }
    #[test]
    fn assemble_binds_network_freshness_and_one_shot_nonce() {
        let network_id =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([1; iroha_crypto::Hash::LENGTH]),
            ));
        let head = ClientQueryRequestHead {
            torii_url: Url::parse("http://127.0.0.1:8080").expect("url"),
            headers: HashMap::new(),
            network_id,
            account_id: iroha_test_samples::ALICE_ID.clone(),
            key_pair: checked_random_keypair(),
            request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            accept_header: APPLICATION_NORITO,
        };
        let req = head
            .assemble(QueryRequest::Singular(
                SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            ))
            .expect("sign query request")
            .build()
            .expect("request build");
        crate::http_default::with_send_hook(
            Arc::new(move |snapshot| {
                let accept = snapshot
                    .headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
                    .map(|(_, value)| value.as_str())
                    .expect("accept header");
                assert_eq!(accept, APPLICATION_NORITO);
                let signed = SignedQuery::decode_all_versioned(&snapshot.body)
                    .expect("decode signed query request");
                assert_eq!(signed.payload.network_id, network_id);
                assert!(signed.payload.creation_time_ms > 0);
                assert_eq!(
                    signed.payload.time_to_live_ms,
                    NonZeroU64::new(
                        crate::config::DEFAULT_QUERY_TIME_TO_LIVE
                            .as_millis()
                            .try_into()
                            .expect("default query TTL fits u64"),
                    )
                    .expect("default query TTL is nonzero")
                );
                assert_ne!(signed.payload.nonce, [0_u8; 32]);
                Ok(http::Response::new(Vec::new()))
            }),
            || {
                let _ = req.send();
            },
        );
    }
    #[test]
    fn validate_fetch_size_rejects_over_max() {
        let over = MAX_FETCH_SIZE.checked_add(1).expect("nonzero add");
        let err = super::validate_fetch_size(over).expect_err("should reject oversized fetch size");
        assert!(matches!(
            err,
            QueryError::Validation(ValidationFail::QueryFailed(
                QueryExecutionFail::FetchSizeTooBig
            ))
        ));
    }
    #[test]
    fn validate_fetch_size_accepts_limits() {
        assert!(super::validate_fetch_size(MAX_FETCH_SIZE).is_ok());
        assert!(super::validate_fetch_size(DEFAULT_FETCH_SIZE).is_ok());
    }
    #[test]
    fn garbled_not_found_is_treated_as_missing() {
        let resp = http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(vec![0xff, 0x00, 0x01])
            .expect("response");
        let err = super::decode_query_response(&resp).expect_err("expected validation error");
        assert!(matches!(
            err,
            QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
        ));
    }
    #[test]
    fn garbled_gone_is_treated_as_expired() {
        let resp = http::Response::builder()
            .status(StatusCode::GONE)
            .body(b"query_validation_failed: The stored cursor has expired".to_vec())
            .expect("response");
        let err = super::decode_query_response(&resp).expect_err("expected validation error");
        assert!(matches!(
            err,
            QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Expired))
        ));
    }
}
impl Client {
    /// Fetch the exact committed transaction selected by its entrypoint hash.
    ///
    /// This uses the dedicated authenticated transaction-details route with the one canonical
    /// `FindTransactions` equality predicate accepted by Torii. The response must be canonical
    /// bounded Norito and must repeat the requested entrypoint hash, a self-consistent entrypoint
    /// and result hash. Both successful and rejected committed results are returned so callers can
    /// authenticate an exact rejection reason instead of treating a public pipeline-status label
    /// as sufficient evidence.
    ///
    /// # Errors
    ///
    /// Returns an error if request binding or signing fails, Torii rejects the query, the response
    /// violates the strict transport/codec contract, or any requested hash/result binding differs.
    pub fn get_transaction_details(
        &self,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<PipelineTransactionDetailsResponse, QueryError> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let request_head = self.get_query_request_head();
        let request = QueryRequest::Start(exact_transaction_details_query(entrypoint_hash));
        let body = request_head.sign_and_encode(request)?;
        let make_request = || {
            Ok(request_head
                .assemble_canonical_norito_body_at(body.clone(), torii_uri::TRANSACTION_DETAILS)
                .max_response_bytes(TRANSACTION_DETAILS_RESPONSE_MAX_BYTES))
        };
        let response = send_once(make_request)?;
        if response.body().len() > TRANSACTION_DETAILS_RESPONSE_MAX_BYTES {
            return Err(QueryError::Other(eyre!(
                "transaction-details response exceeds {} bytes",
                TRANSACTION_DETAILS_RESPONSE_MAX_BYTES
            )));
        }
        if response.status() != StatusCode::OK {
            return match decode_query_response(&response) {
                Err(error) => Err(error),
                Ok(_) => Err(QueryError::Other(eyre!(
                    "transaction-details endpoint returned an unexpected query response"
                ))),
            };
        }
        let details: PipelineTransactionDetailsResponse = Client::decode_canonical_norito_response(
            &response,
            TRANSACTION_DETAILS_RESPONSE_MAX_BYTES,
            "Failed to get exact transaction details",
        )
        .map_err(QueryError::from)?;
        let expected_hash = entrypoint_hash.to_string();
        let transaction = &details.transaction;
        if details.block_height == 0
            || details.hash != expected_hash
            || transaction.entrypoint_hash() != &entrypoint_hash
            || transaction.entrypoint().hash() != entrypoint_hash
            || transaction.result_hash() != &transaction.result().hash()
            || transaction.entrypoint_proof().leaf_index()
                != transaction.result_proof().leaf_index()
        {
            return Err(QueryError::Other(eyre!(
                "transaction-details response is not a committed result for the requested entrypoint/result hash"
            )));
        }
        Ok(details)
    }

    /// Fetch the exact successfully committed transaction selected by its entrypoint hash.
    ///
    /// This compatibility wrapper applies the historical success-only constraint after the
    /// authenticated, canonical details response has been validated. Call
    /// [`Self::get_transaction_details`] when a committed rejection and its typed reason are the
    /// expected terminal result.
    ///
    /// # Errors
    ///
    /// Returns every error from [`Self::get_transaction_details`] and rejects a committed
    /// transaction whose execution result is an error.
    pub fn get_successful_transaction_details(
        &self,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    ) -> Result<PipelineTransactionDetailsResponse, QueryError> {
        let details = self.get_transaction_details(entrypoint_hash)?;
        if details.transaction.result().is_err() {
            return Err(QueryError::Other(eyre!(
                "transaction-details response contains a rejected transaction result"
            )));
        }
        Ok(details)
    }

    /// Bind, sign, encode, and execute an arbitrary raw query request.
    ///
    /// The client supplies the configured network identity and authority plus a fresh creation
    /// time, nonzero lifetime, and operating-system nonce. This is the canonical boundary for
    /// callers that construct a dynamic [`QueryRequest`] rather than using a typed query builder.
    ///
    /// # Errors
    /// Returns an error if request binding or signing fails, the HTTP request
    /// fails, or the server rejects the query.
    pub fn execute_query_request(
        &self,
        request: QueryRequest,
    ) -> Result<iroha_data_model::query::QueryResponse, QueryError> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let request_head = self.get_query_request_head();
        let body = request_head.sign_and_encode(request)?;
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        send_once_and_decode(make_request, decode_query_response)
    }
    /// Execute an arbitrary `SignedQuery` (already signed and Norito-encoded) against the `/query` endpoint.
    /// Returns a typed `QueryResponse` which may be singular or iterable.
    /// # Errors
    /// Returns an error if the HTTP request fails or the server returns a non-OK response.
    pub fn execute_signed_query_raw(
        &self,
        body: &[u8],
    ) -> Result<iroha_data_model::query::QueryResponse, QueryError> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let make_request = || {
            Ok(DefaultRequestBuilder::new(
                HttpMethod::POST,
                join_torii_url(&self.torii_url, torii_uri::QUERY),
            )
            .headers(self.headers.clone())
            .header("Content-Type", APPLICATION_NORITO)
            .header("Accept", self.wire_format_preference.accept_header())
            .timeout(self.torii_request_timeout)
            .body(body.to_owned()))
        };
        send_once_and_decode(make_request, decode_query_response)
    }
}
impl Client {
    /// Get a [`ClientQueryRequestHead`] - an object that can be used to make queries independently of the client.
    ///
    /// You probably do not want to use it directly, but rather use [`Client::query_single`] or [`Client::query`].
    fn get_query_request_head(&self) -> ClientQueryRequestHead {
        ClientQueryRequestHead {
            torii_url: self.torii_url.clone(),
            headers: self.headers.clone(),
            network_id: self.network_id,
            account_id: self.account.clone(),
            key_pair: self.key_pair.clone(),
            request_timeout: self.torii_request_timeout,
            accept_header: self.wire_format_preference.accept_header(),
        }
    }
    /// Execute a singular query and return the result
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub fn query_single<Q>(&self, query: Q) -> Result<Q::Output, QueryError>
    where
        Q: SingularQuery,
        SingularQueryBox: From<Q>,
        Q::Output: TryFrom<SingularQueryOutputBox>,
        <Q::Output as TryFrom<SingularQueryOutputBox>>::Error: Debug,
    {
        let query = SingularQueryBox::from(query);
        let result = self.execute_singular_query(query)?;
        Ok(result
            .try_into()
            .expect("BUG: iroha returned unexpected type in singular query"))
    }
    /// Build an iterable query and return a builder object
    pub fn query<Q>(&self, query: Q) -> QueryBuilder<'_, Self, Q, Q::Item>
    where
        Q: Query,
    {
        QueryBuilder::new(self, query)
    }
    /// Make a request to continue an iterable query with the provided raw [`ForwardCursor`]
    ///
    /// You probably do not want to use this function, but rather use the [`Self::query`] method to make a query and iterate over its results.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub fn raw_continue_iterable_query(
        &self,
        cursor: ForwardCursor,
    ) -> Result<QueryResponse, QueryError> {
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let request_head = self.get_query_request_head();
        let request = QueryRequest::Continue(cursor);
        let body = request_head.sign_and_encode(request)?;
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        let response = send_once_and_decode(make_request, decode_query_response)?;
        Ok(response)
    }
}
#[cfg(test)]
mod query_errors_handling {
    use super::*;
    use crate::{
        client::{APPLICATION_NORITO, DataModelCompatibility, DataModelCompatibilityError},
        data_model::ValidationFail,
        http::StatusCode as HttpStatusCode,
        http_default::{RequestSnapshot, with_send_hook},
    };
    use http::Response;
    use iroha_config::parameters::actual::SorafsRolloutPhase;
    use iroha_data_model::{
        ChainId,
        query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse, SignedQuery,
        },
    };
    use iroha_test_samples::gen_account_in;
    use iroha_version::codec::DecodeVersioned as _;
    use norito::codec::{Decode, Encode};
    use sorafs_manifest::alias_cache::AliasCachePolicy;
    use sorafs_orchestrator::AnonymityPolicy;
    use std::{
        collections::HashMap,
        num::NonZeroU64,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };
    use url::Url;
    #[test]
    fn certain_errors() -> Result<()> {
        let responses = vec![(StatusCode::UNPROCESSABLE_ENTITY, ValidationFail::TooComplex)];
        for (status_code, err) in responses {
            let body = norito::to_bytes(&err)?;
            let resp = Response::builder().status(status_code).body(body)?;
            match decode_query_response(&resp) {
                Err(QueryError::Validation(actual)) => {
                    // PartialEq isn't implemented, so asserting by encoded repr
                    assert_eq!(actual.encode(), err.encode());
                }
                x => return Err(eyre!("Wrong output for {:?}: {:?}", (status_code, err), x)),
            }
        }
        Ok(())
    }
    #[test]
    fn indeterminate() -> Result<()> {
        let response = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::<u8>::new())?;
        match decode_query_response(&response) {
            Err(QueryError::Other(_)) => Ok(()),
            x => Err(eyre!("Expected indeterminate, found: {:?}", x)),
        }
    }
    #[test]
    fn malformed_iterable_response_error_remains_typed() {
        let error = QueryError::from(
            iroha_data_model::query::builder::TypedBatchDowncastError::WrongType { column: 2 },
        );
        assert!(matches!(
            error,
            QueryError::ResponseShape(
                iroha_data_model::query::builder::TypedBatchDowncastError::WrongType { column: 2 }
            )
        ));
    }
    #[test]
    fn signed_query_transport_never_retries_ambiguous_decode_failure() {
        let sends = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&sends);
        with_mock_http(
            move |_| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(vec![0xFF])
                    .expect("malformed response"))
            },
            || {
                let make_request = || {
                    Ok(DefaultRequestBuilder::new(
                        HttpMethod::POST,
                        Url::parse("http://localhost:8080/query").expect("query URL"),
                    )
                    .body(vec![0xA5]))
                };
                send_once_and_decode(make_request, decode_query_response)
                    .expect_err("malformed response must be reported without retry");
            },
        );
        assert_eq!(sends.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn norito_body_with_json_content_type_errors_cleanly() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
            remaining_items: Some(0),
            has_more: false,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "application/json")
            .body(norito::to_bytes(&expected)?)?;
        match decode_query_response(&response) {
            Err(QueryError::Other(_)) => Ok(()),
            other => Err(eyre!("expected strict JSON decode failure, got {other:?}")),
        }
    }
    #[test]
    fn json_body_decodes_iterable_response() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
            remaining_items: Some(0),
            has_more: false,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "application/json")
            .body(norito::json::to_vec(&expected)?)?;
        let decoded = decode_query_response(&response)?;
        assert_eq!(decoded, expected);
        Ok(())
    }
    #[test]
    fn text_json_is_not_a_supported_response_media_type() -> Result<()> {
        let payload = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
            remaining_items: Some(0),
            has_more: false,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "text/json")
            .body(norito::json::to_vec(&payload)?)?;
        assert!(
            matches!(decode_query_response(&response), Err(QueryError::Other(_))),
            "the retired text/json alias must not select JSON decoding"
        );
        Ok(())
    }
    #[test]
    fn json_body_reports_decode_errors_with_json_context() -> Result<()> {
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "application/json")
            .body(vec![0_u8, 1, 2, 3])?;
        match decode_query_response(&response) {
            Err(QueryError::Other(inner)) => {
                let messages: Vec<String> = inner.chain().map(ToString::to_string).collect();
                assert!(
                    messages
                        .iter()
                        .any(|message| message.contains("Failed to decode JSON query response")),
                    "error message should mention JSON decode failure: {messages:?}"
                );
            }
            other => panic!("decode must fail with QueryError::Other, got {other:?}"),
        }
        Ok(())
    }
    #[test]
    fn missing_content_type_defaults_to_norito_decode() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
            remaining_items: Some(0),
            has_more: false,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .body(norito::to_bytes(&expected)?)?;
        let decoded = decode_query_response(&response)?;
        assert_eq!(decoded, expected);
        Ok(())
    }
    #[test]
    fn empty_ok_body_errors_cleanly() -> Result<()> {
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .body(Vec::<u8>::new())?;
        match decode_query_response(&response) {
            Err(QueryError::Other(_)) => Ok(()),
            other => Err(eyre!("expected Other error for empty body, got {other:?}")),
        }
    }
    #[test]
    fn non_ok_garbage_body_errors_cleanly() -> Result<()> {
        let response = Response::builder()
            .status(HttpStatusCode::INTERNAL_SERVER_ERROR)
            .body(vec![1_u8, 2, 3, 4])?;
        match decode_query_response(&response) {
            Err(QueryError::Other(_)) => Ok(()),
            other => Err(eyre!(
                "expected Other error for garbage body, got {other:?}"
            )),
        }
    }
    #[test]
    fn validation_fail_with_json_content_type_is_parsed() -> Result<()> {
        let body = norito::to_bytes(&ValidationFail::TooComplex)?;
        let response = Response::builder()
            .status(HttpStatusCode::UNPROCESSABLE_ENTITY)
            .header("content-type", "application/json")
            .body(body)?;
        match decode_query_response(&response) {
            Err(QueryError::Validation(v)) => {
                assert_eq!(v.encode(), ValidationFail::TooComplex.encode());
                Ok(())
            }
            other => Err(eyre!("expected Validation error, got {other:?}")),
        }
    }
    #[test]
    fn validation_fail_with_norito_header_is_parsed() -> Result<()> {
        let body = norito::to_bytes(&ValidationFail::TooComplex)?;
        let response = Response::builder()
            .status(HttpStatusCode::UNPROCESSABLE_ENTITY)
            .header("content-type", APPLICATION_NORITO)
            .body(body)?;
        match decode_query_response(&response) {
            Err(QueryError::Validation(v)) => {
                assert_eq!(v.encode(), ValidationFail::TooComplex.encode());
                Ok(())
            }
            other => Err(eyre!("expected Validation error, got {other:?}")),
        }
    }
    #[test]
    fn query_request_head_sets_accept_header() {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let head = ClientQueryRequestHead {
            torii_url: Url::parse("http://localhost:8080").expect("torii url"),
            headers: HashMap::new(),
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([2; iroha_crypto::Hash::LENGTH]),
            )),
            account_id,
            key_pair,
            request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            accept_header: APPLICATION_NORITO,
        };
        let cursor = ForwardCursor {
            query: "cursor".into(),
            cursor: NonZeroU64::new(1).expect("cursor"),
            gas_budget: None,
        };
        let query_request = QueryRequest::Continue(cursor);
        let observed = Arc::new(AtomicBool::new(false));
        let observed_clone = Arc::clone(&observed);
        with_mock_http(
            move |snapshot| {
                observed_clone.store(true, Ordering::Relaxed);
                assert_accept_header(&snapshot, APPLICATION_NORITO);
                Ok(ok_empty_response())
            },
            move || {
                head.assemble(query_request)
                    .expect("sign query request")
                    .build()
                    .expect("request")
                    .send()
                    .expect("send");
            },
        );
        assert!(
            observed.load(Ordering::Relaxed),
            "send hook was not triggered"
        );
    }
    #[test]
    fn execute_signed_query_raw_sets_accept_header() {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let client = Client {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            network_id: crate::client::test_network_id(),
            torii_url: Url::parse("http://localhost:8081").expect("torii url"),
            key_pair: key_pair.clone(),
            transaction_ttl: Some(Duration::from_secs(5)),
            transaction_status_timeout: Duration::from_secs(5),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            account: account_id,
            headers: HashMap::new(),
            operator_key_pair: None,
            add_transaction_nonce: false,
            alias_cache_policy: sample_alias_policy(),
            default_anonymity_policy: AnonymityPolicy::GuardPq,
            rollout_phase: SorafsRolloutPhase::Default,
            data_model_compatibility: Arc::new(Mutex::new(DataModelCompatibility::Compatible)),
            wire_format_preference: crate::client::WireFormatPreference::default(),
        };
        let encoded_response = norito::to_bytes(&QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(Vec::new())),
            remaining_items: Some(0),
            has_more: false,
            continue_cursor: None,
        }))
        .expect("encode query response");
        let observed = Arc::new(AtomicBool::new(false));
        let observed_clone = Arc::clone(&observed);
        with_mock_http(
            move |snapshot| {
                observed_clone.store(true, Ordering::Relaxed);
                assert_accept_header(
                    &snapshot,
                    crate::client::WireFormatPreference::default().accept_header(),
                );
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded_response.clone())
                    .expect("response"))
            },
            || {
                let response = client.execute_signed_query_raw(&[]).expect("execute query");
                assert!(matches!(response, QueryResponse::Iterable(_)));
            },
        );
        assert!(
            observed.load(Ordering::Relaxed),
            "send hook was not triggered"
        );
    }
    #[test]
    fn execute_signed_query_raw_rejects_incompatible_data_model_version_before_query_request() {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let client = Client {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            network_id: crate::client::test_network_id(),
            torii_url: Url::parse("http://localhost:8081").expect("torii url"),
            key_pair,
            transaction_ttl: Some(Duration::from_secs(5)),
            transaction_status_timeout: Duration::from_secs(5),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            account: account_id,
            headers: HashMap::new(),
            operator_key_pair: None,
            add_transaction_nonce: false,
            alias_cache_policy: sample_alias_policy(),
            default_anonymity_policy: AnonymityPolicy::GuardPq,
            rollout_phase: SorafsRolloutPhase::Default,
            data_model_compatibility: Arc::new(Mutex::new(DataModelCompatibility::Unchecked)),
            wire_format_preference: crate::client::WireFormatPreference::default(),
        };
        let query_seen = Arc::new(AtomicBool::new(false));
        let query_seen_clone = Arc::clone(&query_seen);
        let mismatched_version = crate::data_model::DATA_MODEL_VERSION + 1;
        let capabilities_body =
            format!(r#"{{"data_model_version":{mismatched_version}}}"#).into_bytes();
        with_mock_http(
            move |snapshot| match snapshot.url.path() {
                "/v1/node/capabilities" => Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", "application/json")
                    .body(capabilities_body.clone())
                    .expect("capabilities response")),
                p if p == torii_uri::QUERY => {
                    query_seen_clone.store(true, Ordering::Relaxed);
                    Ok(ok_empty_response())
                }
                path => Err(eyre!("unexpected request path: {path}")),
            },
            || {
                let err = client
                    .execute_signed_query_raw(&[])
                    .expect_err("compatibility mismatch must fail");
                let QueryError::Other(report) = err else {
                    panic!("expected QueryError::Other");
                };
                let incompat = report
                    .downcast_ref::<DataModelCompatibilityError>()
                    .expect("compatibility error");
                assert!(matches!(
                    incompat,
                    DataModelCompatibilityError::Mismatch {
                        expected,
                        actual,
                    } if *expected == crate::data_model::DATA_MODEL_VERSION && *actual == mismatched_version
                ));
            },
        );
        assert!(
            !query_seen.load(Ordering::Relaxed),
            "query request must not be sent after compatibility mismatch"
        );
    }
    fn compatible_client_with_conflicting_wire_headers() -> Client {
        let (account_id, key_pair) = gen_account_in("wonderland");
        Client {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            network_id: crate::client::test_network_id(),
            torii_url: Url::parse("http://localhost:8081").expect("torii url"),
            key_pair,
            transaction_ttl: Some(Duration::from_secs(5)),
            transaction_status_timeout: Duration::from_secs(5),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            account: account_id,
            headers: HashMap::from([
                ("Accept".to_owned(), "application/json".to_owned()),
                ("Content-Type".to_owned(), "application/json".to_owned()),
            ]),
            operator_key_pair: None,
            add_transaction_nonce: false,
            alias_cache_policy: sample_alias_policy(),
            default_anonymity_policy: AnonymityPolicy::GuardPq,
            rollout_phase: SorafsRolloutPhase::Default,
            data_model_compatibility: Arc::new(Mutex::new(DataModelCompatibility::Compatible)),
            wire_format_preference: crate::client::WireFormatPreference::default(),
        }
    }
    fn successful_transaction_details_fixture() -> (
        HashOf<TransactionEntrypoint>,
        PipelineTransactionDetailsResponse,
    ) {
        use crate::crypto::MerkleProof;
        use iroha_data_model::transaction::{
            DataTriggerSequence, TransactionBuilder, TransactionResult,
        };
        let (authority, key_pair) = gen_account_in("wonderland");
        let signed = TransactionBuilder::new(
            crate::client::test_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(key_pair.private_key())
        .expect("sign transaction-details fixture");
        let entrypoint = TransactionEntrypoint::External(signed);
        let entrypoint_hash = entrypoint.hash();
        let result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let transaction = CommittedTransaction {
            block_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0x77; iroha_crypto::Hash::LENGTH],
            )),
            entrypoint_hash,
            entrypoint_proof: MerkleProof::from_audit_path(0, Vec::new()),
            entrypoint,
            result_hash: result.hash(),
            result_proof: MerkleProof::from_audit_path(0, Vec::new()),
            result,
            merge_inclusion: None,
        };
        (
            entrypoint_hash,
            PipelineTransactionDetailsResponse {
                hash: entrypoint_hash.to_string(),
                block_height: 1,
                transaction,
                trigger_completions: Vec::new(),
            },
        )
    }
    fn assert_exact_transaction_details_query(
        query: &QueryWithParams,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    ) {
        let find = FindTransactions::new();
        assert_eq!(query.query_payload, find.dyn_encode());
        assert_eq!(query.item, find.query_item_kind());
        assert_eq!(query.params, QueryParams::default());
        let mut predicate_cursor = std::io::Cursor::new(query.predicate_bytes.as_slice());
        let predicate = CompoundPredicate::<CommittedTransaction>::decode(&mut predicate_cursor)
            .expect("decode transaction-details predicate");
        assert_eq!(
            usize::try_from(predicate_cursor.position()).expect("predicate cursor position"),
            query.predicate_bytes.len(),
            "predicate must not contain trailing bytes"
        );
        assert_eq!(
            predicate.committed_tx_filters(),
            Some(CommittedTxFilters {
                entry_eq: Some(entrypoint_hash),
                ..CommittedTxFilters::default()
            })
        );
        let mut selector_cursor = std::io::Cursor::new(query.selector_bytes.as_slice());
        let selector = SelectorTuple::<CommittedTransaction>::decode(&mut selector_cursor)
            .expect("decode transaction-details selector");
        assert_eq!(
            usize::try_from(selector_cursor.position()).expect("selector cursor position"),
            query.selector_bytes.len(),
            "selector must not contain trailing bytes"
        );
        assert_eq!(selector, SelectorTuple::<CommittedTransaction>::default());
    }
    #[test]
    fn transaction_details_reader_uses_exact_signed_query_and_transport_contract() {
        let client = compatible_client_with_conflicting_wire_headers();
        let (entrypoint_hash, details) = successful_transaction_details_fixture();
        let encoded = norito::to_bytes(&details).expect("encode transaction-details response");
        let expected_hash = entrypoint_hash;
        let actual = with_mock_http(
            move |snapshot| {
                assert_eq!(snapshot.method, HttpMethod::POST);
                assert_eq!(snapshot.url.path(), torii_uri::TRANSACTION_DETAILS);
                assert!(snapshot.url.query().is_none());
                assert_eq!(
                    snapshot.max_response_bytes,
                    TRANSACTION_DETAILS_RESPONSE_MAX_BYTES
                );
                for (name, value) in [
                    ("accept", APPLICATION_NORITO),
                    ("content-type", APPLICATION_NORITO),
                ] {
                    let matching = snapshot
                        .headers
                        .iter()
                        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
                        .collect::<Vec<_>>();
                    assert_eq!(matching.len(), 1, "expected one {name} header");
                    assert_eq!(matching[0].1, value);
                }
                let signed = SignedQuery::decode_all_versioned(&snapshot.body)
                    .expect("decode signed transaction-details query");
                let QueryRequest::Start(query) = signed.request() else {
                    panic!("transaction-details request must be a query start");
                };
                assert_exact_transaction_details_query(query, expected_hash);
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("transaction-details response"))
            },
            || client.get_successful_transaction_details(entrypoint_hash),
        )
        .expect("exact transaction-details lookup");
        assert_eq!(actual, details);
    }
    #[test]
    fn transaction_details_reader_preserves_authenticated_rejection_reason() {
        use iroha_data_model::transaction::{TransactionResult, error::TransactionRejectionReason};

        let client = compatible_client_with_conflicting_wire_headers();
        let (entrypoint_hash, mut details) = successful_transaction_details_fixture();
        let reason = TransactionRejectionReason::Validation(ValidationFail::InternalError(
            "privacy action rejected by committed native execution".to_owned(),
        ));
        details.transaction.result = TransactionResult::new(Err(reason.clone()));
        details.transaction.result_hash = details.transaction.result.hash();
        let encoded = norito::to_bytes(&details).expect("encode rejected transaction details");
        let actual = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("rejected transaction-details response"))
            },
            || client.get_transaction_details(entrypoint_hash),
        )
        .expect("authenticated rejection details must remain readable");
        assert_eq!(actual.transaction.result().as_ref().unwrap_err(), &reason);

        let client = compatible_client_with_conflicting_wire_headers();
        let encoded = norito::to_bytes(&details).expect("encode rejected transaction details");
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("rejected transaction-details response"))
            },
            || client.get_successful_transaction_details(entrypoint_hash),
        )
        .expect_err("success-only compatibility reader must still reject failed execution");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("rejected transaction result"),
            "unexpected compatibility-reader error: {diagnostic}"
        );
    }
    #[test]
    fn transaction_details_reader_rejects_noncanonical_or_non_norito_success() {
        let client = compatible_client_with_conflicting_wire_headers();
        let (entrypoint_hash, details) = successful_transaction_details_fixture();
        let mut trailing = norito::to_bytes(&details).expect("encode transaction-details response");
        trailing.push(0);
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(trailing.clone())
                    .expect("trailing response"))
            },
            || client.get_successful_transaction_details(entrypoint_hash),
        )
        .expect_err("trailing bytes must be rejected");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("canonical Norito"),
            "unexpected trailing-byte error: {diagnostic}"
        );

        let client = compatible_client_with_conflicting_wire_headers();
        let encoded = norito::to_bytes(&details).expect("encode transaction-details response");
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", "application/json")
                    .body(encoded.clone())
                    .expect("wrong-media response"))
            },
            || client.get_successful_transaction_details(entrypoint_hash),
        )
        .expect_err("non-Norito success must be rejected");
        assert!(format!("{error:?}").contains("invalid content-type"));

        let client = compatible_client_with_conflicting_wire_headers();
        let (proof_hash, mut mismatched_proof) = successful_transaction_details_fixture();
        mismatched_proof.transaction.result_proof =
            crate::crypto::MerkleProof::from_audit_path(1, Vec::new());
        let encoded =
            norito::to_bytes(&mismatched_proof).expect("encode mismatched result proof index");
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("mismatched result proof response"))
            },
            || client.get_transaction_details(proof_hash),
        )
        .expect_err("entrypoint/result proof leaf indexes must match");
        assert!(format!("{error:?}").contains("entrypoint/result hash"));

        let client = compatible_client_with_conflicting_wire_headers();
        let mut mismatched = details;
        mismatched.transaction.result_hash = HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0x93; iroha_crypto::Hash::LENGTH]),
        );
        let encoded = norito::to_bytes(&mismatched).expect("encode mismatched result hash");
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("mismatched result response"))
            },
            || client.get_successful_transaction_details(entrypoint_hash),
        )
        .expect_err("result hash mismatch must be rejected");
        assert!(format!("{error:?}").contains("entrypoint/result hash"));

        let client = compatible_client_with_conflicting_wire_headers();
        let (_, mut uncommitted) = successful_transaction_details_fixture();
        uncommitted.block_height = 0;
        let encoded = norito::to_bytes(&uncommitted).expect("encode height-zero details");
        let error = with_mock_http(
            move |_| {
                Ok(Response::builder()
                    .status(HttpStatusCode::OK)
                    .header("content-type", APPLICATION_NORITO)
                    .body(encoded.clone())
                    .expect("height-zero response"))
            },
            || client.get_transaction_details(entrypoint_hash),
        )
        .expect_err("height zero must not authenticate as a committed result");
        assert!(format!("{error:?}").contains("not a committed result"));
    }
    fn with_mock_http<R>(
        responder: impl Fn(RequestSnapshot) -> Result<Response<Vec<u8>>> + Send + Sync + 'static,
        f: impl FnOnce() -> R,
    ) -> R {
        with_send_hook(Arc::new(responder), f)
    }
    fn ok_empty_response() -> Response<Vec<u8>> {
        Response::builder()
            .status(HttpStatusCode::OK)
            .body(Vec::new())
            .expect("response")
    }
    fn assert_accept_header(snapshot: &RequestSnapshot, expected: &str) {
        let header = snapshot
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
            .map(|(_, value)| value.as_str());
        assert_eq!(
            header,
            Some(expected),
            "request must declare expected Accept header; got {:?}",
            snapshot.headers
        );
    }
    fn sample_alias_policy() -> AliasCachePolicy {
        AliasCachePolicy::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
    }
}
