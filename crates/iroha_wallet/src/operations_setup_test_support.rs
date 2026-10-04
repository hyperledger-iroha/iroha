//! Test-only canonical transport for closed setup journals; replies confer no native authority.

use super::*;
use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
use iroha_crypto::{Algorithm, KeyPair};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
pub(super) struct Transport {
    pub(super) requests: AtomicUsize,
    pub(super) quotes: AtomicUsize,
    pub(super) submissions: AtomicUsize,
    pub(super) journal: Mutex<Option<std::path::PathBuf>>,
}
impl HttpTransport for Transport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        let (status, body) = match request.url.path() {
            "/v1/node/capabilities" => (
                200,
                norito::json::to_vec(&norito::json!({
                    "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
                    "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()))
                }))?,
            ),
            "/v1/fees/quote" => {
                self.quotes.fetch_add(1, Ordering::SeqCst);
                let request: iroha_torii_shared::FeeQuoteRequest =
                    norito::json::from_slice(&request.body)?;
                (
                    200,
                    norito::json::to_vec(&empty_quote(
                        request.payload.authority(),
                        request.payload.fee_payment_intent(),
                    ))?,
                )
            }
            "/v1/pipeline/transactions/status" => {
                assert_eq!(request.method, iroha::http::Method::GET);
                let hash = request
                    .url
                    .query_pairs()
                    .find(|(key, _)| key == "hash")
                    .unwrap()
                    .1
                    .parse::<iroha_crypto::HashOf<SignedTransaction>>()?;
                let absence = iroha_torii_shared::ErrorEnvelope::new(
                    iroha_torii_shared::PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE,
                    "Missing status.",
                )
                .with_details(iroha_torii_shared::ErrorDetails {
                    pipeline_transaction_status_not_found: Some(
                        iroha_torii_shared::PipelineTransactionStatusNotFoundV1::new(
                            &hash, "global",
                        ),
                    ),
                    ..iroha_torii_shared::ErrorDetails::default()
                });
                (404, norito::json::to_vec(&absence)?)
            }
            path if path == iroha_torii_shared::route_catalog::pipeline::TRANSACTION.path() => {
                self.submissions.fetch_add(1, Ordering::SeqCst);
                let journal = self.journal.lock().unwrap();
                let path = journal
                    .as_ref()
                    .expect("submission requires a prepared journal");
                assert!(path.join("operation.json").is_file());
                assert!(path.join("submission.json").is_file());
                (503, b"unavailable".to_vec())
            }
            path => panic!("unexpected setup HTTP request {path}"),
        };
        Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(body)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

pub(super) fn empty_quote(authority: &AccountId, intent: &FeePaymentIntent) -> FeeQuoteResponse {
    FeeQuoteResponse {
        intent: intent.clone(),
        observation: iroha_torii_shared::FeeQuoteObservation {
            ledger_time_ms: 1,
            next_block_height: 1,
            route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        },
        components: Vec::new(),
        capacities: Vec::new(),
        decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(authority.clone()),
            program_revision: None,
        },
    }
}
pub(super) fn service() -> (AccountService, Arc<Transport>) {
    let config = super::tests::fixture_config();
    let transport = Arc::new(Transport::default());
    let client = Client::with_http_transport(config.clone(), transport.clone()).unwrap();
    (
        AccountService {
            config,
            client,
            deadline: None,
            cancellation: None,
        },
        transport,
    )
}
pub(super) fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
}
