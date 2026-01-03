//! Functions and types to make queries to the Iroha peer.

use std::{
    collections::HashMap,
    fmt::{Debug, Write as _},
    num::{NonZeroU16, NonZeroU64},
    thread,
    time::Duration,
};

use eyre::{Report, Result, eyre};
use http::{StatusCode, header::CONTENT_TYPE};
use iroha_data_model::{
    executor::ExecutorDataModel,
    parameter::{
        Parameters, SmartContractParameters, SumeragiParameters, system::SumeragiConsensusMode,
    },
    proof::ProofRecord,
    query::QueryOutputBatchBoxTuple,
    smart_contract::manifest::ContractManifest,
};
use iroha_torii_shared::uri as torii_uri;
use iroha_version::codec::EncodeVersioned;
use norito::{
    codec::{DecodeAll, Error as NoritoDecodeError},
    json,
};
use reqwest::Error as ReqwestError;
use url::Url;

use crate::{
    client::{APPLICATION_NORITO, Client, QueryResult, ResponseReport, join_torii_url},
    crypto::KeyPair,
    data_model::{
        ValidationFail,
        account::AccountId,
        query::{
            Query, QueryOutput, QueryRequest, QueryResponse, QueryWithParams, SingularQuery,
            SingularQueryBox, SingularQueryOutputBox,
            builder::{QueryBuilder, QueryExecutor},
            error::QueryExecutionFail,
            parameters::{DEFAULT_FETCH_SIZE, ForwardCursor, MAX_FETCH_SIZE},
        },
    },
    http::{Method as HttpMethod, RequestBuilder},
    http_default::DefaultRequestBuilder,
};

#[derive(Debug)]
struct ClientQueryRequestHead {
    torii_url: Url,
    headers: HashMap<String, String>,
    account_id: AccountId,
    key_pair: KeyPair,
}

impl ClientQueryRequestHead {
    #[cfg(test)]
    fn assemble(&self, query: QueryRequest) -> DefaultRequestBuilder {
        let body = self.sign_and_encode(query);
        self.assemble_body(body)
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
        .header("Accept", APPLICATION_NORITO)
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
        .body(body)
    }

    fn sign_and_encode(&self, query: QueryRequest) -> Vec<u8> {
        let with_auth = query.with_authority(self.account_id.clone());
        let query = with_auth.sign(&self.key_pair);
        query.encode_versioned()
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
                .is_some_and(|ct| ct.starts_with("application/json"));
            if is_json {
                return match json::from_slice::<QueryResponse>(body) {
                    Ok(decoded) => Ok(decoded),
                    Err(json_err) => decode_query_response_body(body, Some(&json_err)),
                };
            }
            decode_query_response_body(body, None)
        }
        StatusCode::BAD_REQUEST
        | StatusCode::UNAUTHORIZED
        | StatusCode::FORBIDDEN
        | StatusCode::NOT_FOUND
        | StatusCode::UNPROCESSABLE_ENTITY => {
            let body = resp.body();
            let decoded = norito::decode_from_bytes::<ValidationFail>(body).or_else(|_| {
                let mut cursor = body.as_slice();
                ValidationFail::decode_all(&mut cursor)
            });
            match decoded {
                Ok(fail) => Err(QueryError::Validation(fail)),
                Err(decode_err) => {
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

/// Decode `QueryResponse` from a raw byte body, optionally noting a prior JSON decode failure.
fn decode_query_response_body(
    body: &[u8],
    json_err: Option<&json::Error>,
) -> QueryResult<QueryResponse> {
    if let Ok(res) = norito::decode_from_bytes::<QueryResponse>(body) {
        return Ok(res);
    }

    for offset in [4usize, 8, 12] {
        if offset < body.len()
            && let Ok(res) = norito::decode_from_bytes::<QueryResponse>(&body[offset..])
        {
            return Ok(res);
        }
    }

    let mut cursor = body;
    match QueryResponse::decode_all(&mut cursor) {
        Ok(res) => Ok(res),
        Err(primary_err) => {
            if let Some(fallback) = decode_query_response_fallback(body) {
                return Ok(fallback);
            }
            let mut msg = String::from(
                "Failed to decode response from Iroha. \
                 You are likely using a version of the client library \
                 that is incompatible with the version of the peer software",
            );
            if let Some(json_err) = json_err {
                let _ = write!(msg, " (JSON decode error: {json_err})");
            }
            let report = Report::new(primary_err).wrap_err(msg);
            Err(report.into())
        }
    }
}

/// Try decoding a query response using a more permissive Parameters layout.
fn decode_query_response_fallback(body: &[u8]) -> Option<QueryResponse> {
    let mut cursor = body;
    match compat::CompatQueryResponse::decode_all(&mut cursor) {
        Ok(decoded) => Some(decoded.into()),
        Err(err) => {
            eprintln!("Compat query decode failed: {err:?}");
            None
        }
    }
}

fn send_with_retry<F>(mut make_request: F) -> Result<http::Response<Vec<u8>>, QueryError>
where
    F: FnMut() -> Result<DefaultRequestBuilder, QueryError>,
{
    const MAX_RETRIES: usize = 1;
    const RETRY_DELAY: Duration = Duration::from_millis(200);
    const RETRY_DEADLINE: Duration = Duration::from_secs(1);

    let mut last_err: Option<QueryError> = None;
    let start = std::time::Instant::now();
    for attempt in 0..=MAX_RETRIES {
        let result = make_request().and_then(|builder| {
            builder
                .build()
                .map_err(QueryError::from)
                .and_then(|req| req.send().map_err(QueryError::from))
        });

        match result {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                let retryable = is_retryable_query_error(&err);
                if attempt == MAX_RETRIES || !retryable || start.elapsed() >= RETRY_DEADLINE {
                    return Err(err);
                }
                last_err = Some(err);
                thread::sleep(RETRY_DELAY);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| QueryError::Other(eyre!("exhausted query retries"))))
}

fn is_retryable_query_error(err: &QueryError) -> bool {
    match err {
        QueryError::Validation(_) => false,
        QueryError::Other(report) => report.chain().any(|cause| {
            cause.downcast_ref::<ReqwestError>().map_or_else(
                || {
                    cause
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|io_err| {
                            matches!(
                                io_err.kind(),
                                std::io::ErrorKind::ConnectionReset
                                    | std::io::ErrorKind::ConnectionAborted
                                    | std::io::ErrorKind::TimedOut
                                    | std::io::ErrorKind::BrokenPipe
                                    | std::io::ErrorKind::WouldBlock
                            )
                        })
                },
                |req_err| req_err.is_timeout() || req_err.is_connect() || req_err.is_request(),
            )
        }),
    }
}

fn retry_decode_with_send<F, D, T>(make_request: F, decode: D) -> Result<T, QueryError>
where
    F: FnMut() -> Result<DefaultRequestBuilder, QueryError> + Clone,
    D: Fn(&http::Response<Vec<u8>>) -> QueryResult<T>,
{
    const MAX_DECODE_RETRIES: usize = 1;
    const DECODE_RETRY_DELAY: Duration = Duration::from_millis(100);

    let mut attempt = 0;
    loop {
        let make_req = make_request.clone();
        let response = send_with_retry(make_req)?;
        match decode(&response) {
            Ok(value) => return Ok(value),
            Err(err) => {
                if is_decode_error(&err) && attempt < MAX_DECODE_RETRIES {
                    attempt += 1;
                    thread::sleep(DECODE_RETRY_DELAY);
                    continue;
                }
                return Err(err);
            }
        }
    }
}

fn is_decode_error(err: &QueryError) -> bool {
    match err {
        QueryError::Validation(_) => false,
        QueryError::Other(report) => report.chain().any(|cause| {
            cause.is::<NoritoDecodeError>()
                || cause.is::<norito::json::Error>()
                || cause.is::<std::str::Utf8Error>()
        }),
    }
}

/// Permissive decoder used to handle parameter payloads that may violate
/// non-zero invariants by falling back to sensible defaults.
mod compat {
    use std::collections::BTreeMap;

    use iroha_data_model::{
        Decode, Encode,
        parameter::{CustomParameter, CustomParameterId},
        query::{QueryOutput, SingularQueryOutputBox},
    };

    use super::*;

    #[derive(Debug, Encode, Decode)]
    pub(super) enum CompatQueryResponse {
        Singular(Box<CompatSingularOutput>),
        Iterable(QueryOutput),
    }

    #[derive(Debug, Encode, Decode)]
    pub(super) enum CompatSingularOutput {
        ExecutorDataModel(ExecutorDataModel),
        Parameters(CompatParameters),
        ProofRecord(ProofRecord),
        ContractManifest(ContractManifest),
        ActiveAbiVersions(crate::data_model::query::runtime::ActiveAbiVersions),
        Asset(crate::data_model::asset::value::Asset),
    }

    #[derive(Debug, Encode, Decode)]
    pub(super) struct CompatParameters {
        sumeragi: CompatSumeragiParameters,
        block: CompatBlockParameters,
        transaction: CompatTransactionParameters,
        executor: CompatSmartContractParameters,
        smart_contract: CompatSmartContractParameters,
        custom: BTreeMap<CustomParameterId, CustomParameter>,
    }

    #[derive(Debug, Encode, Decode)]
    struct CompatSumeragiParameters {
        block_time_ms: u64,
        commit_time_ms: u64,
        max_clock_drift_ms: u64,
        collectors_k: u16,
        collectors_redundant_send_r: u8,
        da_enabled: bool,
        next_mode: Option<u8>,
        mode_activation_height: Option<u64>,
    }

    #[derive(Debug, Encode, Decode)]
    struct CompatBlockParameters {
        max_transactions: u64,
    }

    #[derive(Debug, Encode, Decode)]
    struct CompatTransactionParameters {
        max_signatures: u64,
        max_instructions: u64,
        ivm_bytecode_size: u64,
        max_tx_bytes: u64,
        max_decompressed_bytes: u64,
        max_metadata_depth: u16,
        require_height_ttl: bool,
        require_sequence: bool,
    }

    #[derive(Debug, Encode, Decode)]
    struct CompatSmartContractParameters {
        fuel: u64,
        memory: u64,
        execution_depth: u8,
    }

    /// Produce a `CompatQueryResponse` carrying zeroed parameters for testing fallback decode paths.
    #[cfg(test)]
    pub(super) fn encode_zeroed_parameters_response() -> Vec<u8> {
        let params = CompatParameters {
            sumeragi: CompatSumeragiParameters {
                block_time_ms: 0,
                commit_time_ms: 0,
                max_clock_drift_ms: 0,
                collectors_k: 0,
                collectors_redundant_send_r: 0,
                da_enabled: false,
                next_mode: None,
                mode_activation_height: None,
            },
            block: CompatBlockParameters {
                max_transactions: 0,
            },
            transaction: CompatTransactionParameters {
                max_signatures: 0,
                max_instructions: 0,
                ivm_bytecode_size: 0,
                max_tx_bytes: 0,
                max_decompressed_bytes: 0,
                max_metadata_depth: 0,
                require_height_ttl: false,
                require_sequence: false,
            },
            executor: CompatSmartContractParameters {
                fuel: 0,
                memory: 0,
                execution_depth: 0,
            },
            smart_contract: CompatSmartContractParameters {
                fuel: 0,
                memory: 0,
                execution_depth: 0,
            },
            custom: BTreeMap::new(),
        };
        CompatQueryResponse::Singular(Box::new(CompatSingularOutput::Parameters(params))).encode()
    }

    fn to_nonzero_u64(value: u64, default: NonZeroU64) -> NonZeroU64 {
        NonZeroU64::new(value).unwrap_or(default)
    }

    fn to_nonzero_u16(value: u16, default: NonZeroU16) -> NonZeroU16 {
        NonZeroU16::new(value).unwrap_or(default)
    }

    fn sumeragi_params_from_compat(
        value: &CompatSumeragiParameters,
        defaults: &Parameters,
    ) -> SumeragiParameters {
        let defaults = defaults.sumeragi();
        SumeragiParameters {
            block_time_ms: if value.block_time_ms == 0 {
                defaults.block_time_ms
            } else {
                value.block_time_ms
            },
            commit_time_ms: if value.commit_time_ms == 0 {
                defaults.commit_time_ms
            } else {
                value.commit_time_ms
            },
            max_clock_drift_ms: if value.max_clock_drift_ms == 0 {
                defaults.max_clock_drift_ms
            } else {
                value.max_clock_drift_ms
            },
            collectors_k: if value.collectors_k == 0 {
                defaults.collectors_k
            } else {
                value.collectors_k
            },
            collectors_redundant_send_r: if value.collectors_redundant_send_r == 0 {
                defaults.collectors_redundant_send_r
            } else {
                value.collectors_redundant_send_r
            },
            da_enabled: value.da_enabled,
            next_mode: value.next_mode.map(|m| match m {
                1 => SumeragiConsensusMode::Npos,
                _ => SumeragiConsensusMode::Permissioned,
            }),
            mode_activation_height: value.mode_activation_height,
            key_activation_lead_blocks: defaults.key_activation_lead_blocks,
            key_overlap_grace_blocks: defaults.key_overlap_grace_blocks,
            key_expiry_grace_blocks: defaults.key_expiry_grace_blocks,
            key_require_hsm: defaults.key_require_hsm,
            key_allowed_algorithms: defaults.key_allowed_algorithms.clone(),
            key_allowed_hsm_providers: defaults.key_allowed_hsm_providers.clone(),
        }
    }

    fn block_params_from_compat(
        value: &CompatBlockParameters,
        defaults: &Parameters,
    ) -> iroha_data_model::parameter::BlockParameters {
        iroha_data_model::parameter::BlockParameters {
            max_transactions: to_nonzero_u64(
                value.max_transactions,
                defaults.block().max_transactions(),
            ),
        }
    }

    fn transaction_params_from_compat(
        value: &CompatTransactionParameters,
        defaults: &Parameters,
    ) -> iroha_data_model::parameter::TransactionParameters {
        iroha_data_model::parameter::TransactionParameters::with_max_signatures(
            to_nonzero_u64(
                value.max_signatures,
                defaults.transaction().max_signatures(),
            ),
            to_nonzero_u64(
                value.max_instructions,
                defaults.transaction().max_instructions(),
            ),
            to_nonzero_u64(
                value.ivm_bytecode_size,
                defaults.transaction().ivm_bytecode_size(),
            ),
            to_nonzero_u64(value.max_tx_bytes, defaults.transaction().max_tx_bytes()),
            to_nonzero_u64(
                value.max_decompressed_bytes,
                defaults.transaction().max_decompressed_bytes(),
            ),
            to_nonzero_u16(
                value.max_metadata_depth,
                defaults.transaction().max_metadata_depth(),
            ),
        )
        .with_ingress_enforcement(value.require_height_ttl, value.require_sequence)
    }

    fn smart_contract_params_from_compat(
        value: &CompatSmartContractParameters,
        defaults: &SmartContractParameters,
    ) -> SmartContractParameters {
        SmartContractParameters {
            fuel: to_nonzero_u64(value.fuel, defaults.fuel()),
            memory: to_nonzero_u64(value.memory, defaults.memory()),
            execution_depth: value.execution_depth,
        }
    }

    impl From<CompatQueryResponse> for QueryResponse {
        fn from(value: CompatQueryResponse) -> Self {
            match value {
                CompatQueryResponse::Iterable(iter) => QueryResponse::Iterable(iter),
                CompatQueryResponse::Singular(singular) => {
                    QueryResponse::Singular((*singular).into())
                }
            }
        }
    }

    impl From<CompatSingularOutput> for SingularQueryOutputBox {
        fn from(value: CompatSingularOutput) -> Self {
            match value {
                CompatSingularOutput::ExecutorDataModel(v) => {
                    SingularQueryOutputBox::ExecutorDataModel(v)
                }
                CompatSingularOutput::Parameters(v) => SingularQueryOutputBox::Parameters(v.into()),
                CompatSingularOutput::ProofRecord(v) => SingularQueryOutputBox::ProofRecord(v),
                CompatSingularOutput::ContractManifest(v) => {
                    SingularQueryOutputBox::ContractManifest(v)
                }
                CompatSingularOutput::ActiveAbiVersions(v) => {
                    SingularQueryOutputBox::ActiveAbiVersions(v)
                }
                CompatSingularOutput::Asset(v) => SingularQueryOutputBox::Asset(v),
            }
        }
    }

    impl From<CompatParameters> for Parameters {
        fn from(value: CompatParameters) -> Self {
            let defaults = Parameters::default();
            let sumeragi = sumeragi_params_from_compat(&value.sumeragi, &defaults);
            let block = block_params_from_compat(&value.block, &defaults);
            let transaction = transaction_params_from_compat(&value.transaction, &defaults);
            let executor_defaults = defaults.executor();
            let smart_contract_defaults = defaults.smart_contract();
            let executor = smart_contract_params_from_compat(&value.executor, &executor_defaults);
            let smart_contract =
                smart_contract_params_from_compat(&value.smart_contract, &smart_contract_defaults);

            Parameters {
                sumeragi,
                block,
                transaction,
                executor,
                smart_contract,
                custom: value.custom,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeMap;

        use super::*;

        fn zeroed_compat_sumeragi() -> CompatSumeragiParameters {
            CompatSumeragiParameters {
                block_time_ms: 0,
                commit_time_ms: 0,
                max_clock_drift_ms: 0,
                collectors_k: 0,
                collectors_redundant_send_r: 0,
                da_enabled: false,
                next_mode: None,
                mode_activation_height: None,
            }
        }

        #[test]
        fn compat_parameters_zeroed_defaults_apply() {
            let compat = CompatParameters {
                sumeragi: zeroed_compat_sumeragi(),
                block: CompatBlockParameters {
                    max_transactions: 0,
                },
                transaction: CompatTransactionParameters {
                    max_signatures: 0,
                    max_instructions: 0,
                    ivm_bytecode_size: 0,
                    max_tx_bytes: 0,
                    max_decompressed_bytes: 0,
                    max_metadata_depth: 0,
                    require_height_ttl: true,
                    require_sequence: true,
                },
                executor: CompatSmartContractParameters {
                    fuel: 0,
                    memory: 0,
                    execution_depth: 0,
                },
                smart_contract: CompatSmartContractParameters {
                    fuel: 0,
                    memory: 0,
                    execution_depth: 0,
                },
                custom: BTreeMap::new(),
            };

            let defaults = Parameters::default();
            let params = Parameters::from(compat);

            assert_eq!(
                params.sumeragi().block_time_ms,
                defaults.sumeragi().block_time_ms
            );
            assert_eq!(
                params.sumeragi().commit_time_ms,
                defaults.sumeragi().commit_time_ms
            );
            assert_eq!(
                params.sumeragi().max_clock_drift_ms,
                defaults.sumeragi().max_clock_drift_ms
            );
            assert_eq!(
                params.sumeragi().collectors_k,
                defaults.sumeragi().collectors_k
            );
            assert_eq!(
                params.sumeragi().collectors_redundant_send_r,
                defaults.sumeragi().collectors_redundant_send_r
            );
            assert_eq!(
                params.block().max_transactions(),
                defaults.block().max_transactions()
            );
            assert_eq!(
                params.transaction().max_signatures(),
                defaults.transaction().max_signatures()
            );
            assert_eq!(
                params.transaction().max_instructions(),
                defaults.transaction().max_instructions()
            );
            assert_eq!(
                params.transaction().ivm_bytecode_size(),
                defaults.transaction().ivm_bytecode_size()
            );
            assert_eq!(
                params.transaction().max_tx_bytes(),
                defaults.transaction().max_tx_bytes()
            );
            assert_eq!(
                params.transaction().max_decompressed_bytes(),
                defaults.transaction().max_decompressed_bytes()
            );
            assert_eq!(
                params.transaction().max_metadata_depth(),
                defaults.transaction().max_metadata_depth()
            );
            assert!(params.transaction().require_height_ttl());
            assert!(params.transaction().require_sequence());
            assert_eq!(params.executor().fuel(), defaults.executor().fuel());
            assert_eq!(params.executor().memory(), defaults.executor().memory());
            assert_eq!(
                params.smart_contract().execution_depth(),
                defaults.smart_contract().execution_depth()
            );
        }

        #[test]
        fn compat_sumeragi_next_mode_mapping() {
            let defaults = Parameters::default();
            let mut compat = zeroed_compat_sumeragi();
            compat.next_mode = Some(1);
            let params = sumeragi_params_from_compat(&compat, &defaults);
            assert_eq!(params.next_mode, Some(SumeragiConsensusMode::Npos));

            compat.next_mode = Some(99);
            let params = sumeragi_params_from_compat(&compat, &defaults);
            assert_eq!(params.next_mode, Some(SumeragiConsensusMode::Permissioned));
        }
    }
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

/// An iterable query cursor for use in the client
#[derive(Debug)]
pub struct QueryCursor {
    // instead of storing iroha client itself, we store the base URL and headers required to make a request
    //   along with the account id and key pair to sign the request.
    // this removes the need to either keep a reference or use an Arc, but breaks abstraction a little
    request_head: ClientQueryRequestHead,
    cursor: ForwardCursor,
}

/// Different errors as a result of query response handling
#[derive(Debug, thiserror::Error, displaydoc::Display)]
pub enum QueryError {
    /// Query validation error
    Validation(#[from] ValidationFail),
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
        let is_parameters_query = matches!(query, SingularQueryBox::FindParameters(_));
        let request_head = self.get_query_request_head();

        let request = QueryRequest::Singular(query);
        let body = request_head.sign_and_encode(request);
        let make_request = || {
            if is_parameters_query {
                Ok(request_head.assemble_body_with_accept(body.clone(), "application/json"))
            } else {
                Ok(request_head.assemble_body(body.clone()))
            }
        };
        retry_decode_with_send(make_request, decode_singular_query_response)
    }

    fn start_query(
        &self,
        query: QueryWithParams,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
        let requested_fetch_size = query
            .params
            .fetch_size
            .fetch_size
            .unwrap_or(DEFAULT_FETCH_SIZE);
        validate_fetch_size(requested_fetch_size)?;

        let request_head = self.get_query_request_head();

        let request = QueryRequest::Start(query);
        let body = request_head.sign_and_encode(request);
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        let response = retry_decode_with_send(make_request, decode_iterable_query_response)?;

        let (batch, remaining_items, cursor) = response.into_parts();

        let cursor = cursor.map(|cursor| QueryCursor {
            request_head,
            cursor,
        });

        Ok((batch, remaining_items, cursor))
    }

    fn continue_query(
        cursor: Self::Cursor,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
        let QueryCursor {
            request_head,
            cursor,
        } = cursor;

        let request = QueryRequest::Continue(cursor);
        let body = request_head.sign_and_encode(request);
        let make_request = || Ok(request_head.assemble_body(body.clone()));
        let response = retry_decode_with_send(make_request, decode_iterable_query_response)?;

        let (batch, remaining_items, cursor) = response.into_parts();

        let cursor = cursor.map(|cursor| QueryCursor {
            request_head,
            cursor,
        });

        Ok((batch, remaining_items, cursor))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_data_model::query::executor::prelude::FindExecutorDataModel;

    use super::*;

    #[test]
    fn assemble_sets_norito_accept_header() {
        let head = ClientQueryRequestHead {
            torii_url: Url::parse("http://127.0.0.1:8080").expect("url"),
            headers: HashMap::new(),
            account_id: iroha_test_samples::ALICE_ID.clone(),
            key_pair: KeyPair::random(),
        };
        let req = head
            .assemble(QueryRequest::Singular(
                SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            ))
            .build()
            .expect("request build");

        crate::http_default::set_send_hook(Some(Arc::new(|snapshot| {
            let accept = snapshot
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
                .map(|(_, value)| value.as_str())
                .expect("accept header");
            assert_eq!(accept, APPLICATION_NORITO);
            Ok(http::Response::new(Vec::new()))
        })));

        let _ = req.send();
        crate::http_default::set_send_hook(None);
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
}
impl Client {
    /// Execute an arbitrary `SignedQuery` (already signed and Norito-encoded) against the `/query` endpoint.
    /// Returns a typed `QueryResponse` which may be singular or iterable.
    /// # Errors
    /// Returns an error if the HTTP request fails or the server returns a non-OK response.
    pub fn execute_signed_query_raw(
        &self,
        body: &[u8],
    ) -> Result<iroha_data_model::query::QueryResponse, QueryError> {
        let make_request = || {
            Ok(DefaultRequestBuilder::new(
                HttpMethod::POST,
                join_torii_url(&self.torii_url, torii_uri::QUERY),
            )
            .headers(self.headers.clone())
            .header("Content-Type", APPLICATION_NORITO)
            .header("Accept", APPLICATION_NORITO)
            .body(body.to_owned()))
        };
        retry_decode_with_send(make_request, decode_query_response)
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
            account_id: self.account.clone(),
            key_pair: self.key_pair.clone(),
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
        let request_head = self.get_query_request_head();

        let request = QueryRequest::Continue(cursor);
        let body = request_head.sign_and_encode(request);
        let make_request = || Ok(request_head.assemble_body(body.clone()));

        let response = retry_decode_with_send(make_request, decode_query_response)?;

        Ok(response)
    }
}

#[cfg(test)]
mod query_errors_handling {
    use std::{
        collections::HashMap,
        num::NonZeroU64,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Mutex, OnceLock,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use http::Response;
    use iroha_config::parameters::actual::SorafsRolloutPhase;
    use iroha_data_model::{
        ChainId,
        query::{QueryOutput, QueryOutputBatchBoxTuple, QueryResponse},
    };
    use iroha_test_samples::gen_account_in;
    use norito::codec::Encode;
    use sorafs_manifest::alias_cache::AliasCachePolicy;
    use sorafs_orchestrator::AnonymityPolicy;
    use url::Url;

    use super::*;
    use crate::{
        client::APPLICATION_NORITO,
        data_model::ValidationFail,
        http::StatusCode as HttpStatusCode,
        http_default::{RequestSnapshot, set_send_hook},
        query::compat,
    };

    #[test]
    fn certain_errors() -> Result<()> {
        let responses = vec![(StatusCode::UNPROCESSABLE_ENTITY, ValidationFail::TooComplex)];
        for (status_code, err) in responses {
            let resp = Response::builder().status(status_code).body(err.encode())?;

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
    fn norito_body_decodes_when_content_type_is_json() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple { tuple: Vec::new() },
            remaining_items: 0,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "application/json")
            .body(expected.encode())?;

        let decoded = decode_query_response(&response)?;
        assert_eq!(decoded, expected);

        Ok(())
    }

    #[test]
    fn json_body_decodes_iterable_response() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple { tuple: Vec::new() },
            remaining_items: 0,
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
    fn json_body_reports_decode_errors_with_json_context() -> Result<()> {
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", "application/json")
            .body(vec![0_u8, 1, 2, 3])?;

        match decode_query_response(&response) {
            Err(QueryError::Other(inner)) => {
                let msg = inner.to_string();
                assert!(
                    msg.contains("JSON decode error"),
                    "error message should mention JSON decode failure: {msg}"
                );
            }
            other => panic!("decode must fail with QueryError::Other, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn missing_content_type_defaults_to_norito_decode() -> Result<()> {
        let expected = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple { tuple: Vec::new() },
            remaining_items: 0,
            continue_cursor: None,
        });
        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .body(expected.encode())?;

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
        let body = ValidationFail::TooComplex.encode();
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
    fn compat_only_payload_falls_back_successfully() -> Result<()> {
        // Construct a payload with zero values in places where strict decoding requires non-zero.
        let compat_body = compat::encode_zeroed_parameters_response();

        let response = Response::builder()
            .status(HttpStatusCode::OK)
            .header("content-type", APPLICATION_NORITO)
            .body(compat_body)?;

        let decoded = decode_query_response(&response)?;
        // Must not panic and must produce a response (shape not asserted further).
        if let QueryResponse::Singular(_) = decoded {
            Ok(())
        } else {
            Err(eyre!(
                "expected singular response from compat decoder, got {decoded:?}"
            ))
        }
    }

    #[test]
    fn query_request_head_sets_accept_header() {
        let (account_id, key_pair) = gen_account_in("wonderland");
        let head = ClientQueryRequestHead {
            torii_url: Url::parse("http://localhost:8080").expect("torii url"),
            headers: HashMap::new(),
            account_id,
            key_pair,
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
                assert_accept_header(&snapshot);
                Ok(ok_empty_response())
            },
            move || {
                head.assemble(query_request)
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
            torii_url: Url::parse("http://localhost:8081").expect("torii url"),
            key_pair: key_pair.clone(),
            transaction_ttl: Some(Duration::from_secs(5)),
            transaction_status_timeout: Duration::from_secs(5),
            account: account_id,
            headers: HashMap::new(),
            add_transaction_nonce: false,
            alias_cache_policy: sample_alias_policy(),
            default_anonymity_policy: AnonymityPolicy::GuardPq,
            rollout_phase: SorafsRolloutPhase::Default,
        };

        let encoded_response = QueryResponse::Iterable(QueryOutput {
            batch: QueryOutputBatchBoxTuple { tuple: Vec::new() },
            remaining_items: 0,
            continue_cursor: None,
        })
        .encode();

        let observed = Arc::new(AtomicBool::new(false));
        let observed_clone = Arc::clone(&observed);
        with_mock_http(
            move |snapshot| {
                observed_clone.store(true, Ordering::Relaxed);
                assert_accept_header(&snapshot);
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

    fn with_mock_http<R>(
        responder: impl Fn(RequestSnapshot) -> Result<Response<Vec<u8>>> + Send + Sync + 'static,
        f: impl FnOnce() -> R,
    ) -> R {
        static HOOK_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = HOOK_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("hook guard");
        set_send_hook(Some(Arc::new(responder)));
        let outcome = catch_unwind(AssertUnwindSafe(f));
        set_send_hook(None);
        drop(guard);
        match outcome {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn ok_empty_response() -> Response<Vec<u8>> {
        Response::builder()
            .status(HttpStatusCode::OK)
            .body(Vec::new())
            .expect("response")
    }

    fn assert_accept_header(snapshot: &RequestSnapshot) {
        let header = snapshot
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
            .map(|(_, value)| value.as_str());
        assert_eq!(
            header,
            Some("application/json"),
            "request must declare Accept: application/json; got {:?}",
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
