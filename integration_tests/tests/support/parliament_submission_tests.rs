//! Controlled HTTP exercises the real Parliament submission and observation fence.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::Result;
use iroha::{
    blocking::Client,
    client::PreparedTransactionPayload,
    crypto::{Hash, HashOf},
    data_model::{
        NetworkId,
        isi::Log,
        prelude::{Level, SignedTransaction},
    },
    http::{HttpTransport, Method, Response, TransportFuture, TransportRequest},
};
use iroha_version::codec::DecodeVersioned as _;

use super::{
    OPERATION_TIMEOUT, prepare_parliament_transaction, submit_parliament_instructions,
    submit_prepared_parliament_transaction_until,
};

const PIPELINE: &str = "/v1/pipeline/transactions/status";
const SUBMISSION: &str = "/v1/pipeline/transactions";
const HEIGHT: u64 = 143;

#[derive(Debug)]
struct Snapshot {
    path: String,
    timeout: Duration,
    scope: Option<String>,
    hash: Option<String>,
}

#[derive(Debug)]
enum Reply {
    Pipeline {
        kind: &'static str,
        source: &'static str,
        scope: &'static str,
        height: Option<u64>,
        substitute_hash: bool,
    },
    Height(u64),
    Refused(iroha::StatusFailureReason),
    Malformed,
    TransportFailure,
    Pending,
    LateHeight(u64),
}

#[derive(Debug)]
struct Step {
    path: &'static str,
    scope: Option<&'static str>,
    reply: Reply,
}

fn pipeline(scope: &'static str, kind: &'static str, source: &'static str) -> Step {
    Step {
        path: PIPELINE,
        scope: Some(scope),
        reply: Reply::Pipeline {
            kind,
            source,
            scope,
            height: Some(HEIGHT),
            substitute_hash: false,
        },
    }
}

fn status(reply: Reply) -> Step {
    Step {
        path: "/status",
        scope: None,
        reply,
    }
}

#[derive(Debug)]
struct OriginalTransportFailure;

impl std::fmt::Display for OriginalTransportFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("original Parliament observation transport failure")
    }
}

impl std::error::Error for OriginalTransportFailure {}

struct PendingRead(Arc<AtomicBool>);

impl Drop for PendingRead {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct SubmissionTransport {
    steps: Mutex<VecDeque<Step>>,
    requests: Mutex<Vec<Snapshot>>,
    submitted: Mutex<Vec<Vec<u8>>>,
    pending_dropped: Arc<AtomicBool>,
}

impl SubmissionTransport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(steps.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            submitted: Mutex::new(Vec::new()),
            pending_dropped: Arc::new(AtomicBool::new(false)),
        })
    }

    fn signed_transaction(&self) -> SignedTransaction {
        let submitted = self.submitted.lock().unwrap();
        assert_eq!(
            submitted.len(),
            1,
            "the signed write must never be replayed"
        );
        SignedTransaction::decode_all_versioned(&submitted[0]).expect("actual signed wire")
    }

    fn assert_identity_and_budget(&self, budget: Duration) {
        let transaction = self.signed_transaction();
        assert_eq!(
            self.submitted.lock().unwrap()[0],
            PreparedTransactionPayload::from_transaction(&transaction).as_bytes(),
            "the submitted wire must retain its exact canonical identity"
        );
        let requests = self.requests.lock().unwrap();
        let completion = requests
            .iter()
            .filter(|request| request.path != "/v1/fees/quote")
            .collect::<Vec<_>>();
        assert!(completion.iter().all(|request| request.timeout <= budget));
        assert!(
            completion
                .windows(2)
                .all(|pair| pair[1].timeout <= pair[0].timeout),
            "global, local and status requests must consume one original deadline"
        );
        for request in completion {
            if request.path == PIPELINE {
                assert_eq!(
                    request.hash.as_deref(),
                    Some(transaction.hash().to_string().as_str())
                );
            }
        }
    }

    fn record(&self, request: &TransportRequest) {
        assert_eq!(
            request.url.host_str(),
            Some("parliament-completion.invalid")
        );
        let parameter = |name| {
            request
                .url
                .query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        self.requests.lock().unwrap().push(Snapshot {
            path: request.url.path().to_owned(),
            timeout: request.timeout.expect("every request is deadline-bound"),
            scope: parameter("scope"),
            hash: parameter("hash"),
        });
    }

    fn observation(&self, request: &TransportRequest) -> Reply {
        assert_eq!(request.method, Method::GET);
        assert!(request.body.is_empty());
        let step = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("extra observation");
        assert_eq!(request.url.path(), step.path, "observation order matters");
        let requests = self.requests.lock().unwrap();
        let observed = requests.last().unwrap();
        assert_eq!(observed.scope.as_deref(), step.scope);
        if step.path == PIPELINE {
            assert_eq!(request.url.query_pairs().count(), 2);
            assert_eq!(
                observed.hash.as_deref(),
                Some(self.signed_transaction().hash().to_string().as_str())
            );
        } else {
            assert!(request.url.query().is_none());
        }
        step.reply
    }
}

fn json_response(status: u16, body: Vec<u8>) -> Result<Response<Vec<u8>>> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body)?)
}

fn height_response(blocks: u64) -> Result<Response<Vec<u8>>> {
    json_response(
        200,
        norito::json::to_vec(&iroha_torii_shared::status::Status {
            blocks,
            ..Default::default()
        })?,
    )
}

fn fee_quote(request: &TransportRequest) -> Result<Response<Vec<u8>>> {
    assert_eq!(request.method, Method::POST);
    let quote: iroha_torii_shared::FeeQuoteRequest = norito::json::from_slice(&request.body)?;
    let response = iroha_torii_shared::FeeQuoteResponse {
        intent: quote.payload.fee_payment_intent().clone(),
        observation: iroha_torii_shared::FeeQuoteObservation {
            ledger_time_ms: 1,
            next_block_height: HEIGHT,
            route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        },
        components: Vec::new(),
        capacities: Vec::new(),
        decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(
                quote.payload.authority().clone(),
            ),
            program_revision: None,
        },
    };
    json_response(200, norito::json::to_vec(&response)?)
}

impl HttpTransport for SubmissionTransport {
    fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
        panic!("Parliament completion must use the asynchronous SDK");
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            self.record(&request);
            match request.url.path() {
                "/v1/fees/quote" => return fee_quote(&request),
                "/v1/node/capabilities" => {
                    assert_eq!(request.method, Method::GET);
                    return json_response(
                        200,
                        norito::json::to_vec(&norito::json!({
                            "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
                            "signed_transaction_schema_hash_hex": (hex::encode(
                                norito::schema::identity::frame_hash::<SignedTransaction>()
                            )),
                        }))?,
                    );
                }
                SUBMISSION => {
                    assert_eq!(request.method, Method::POST);
                    self.submitted.lock().unwrap().push(request.body);
                    self.signed_transaction();
                    return json_response(202, Vec::new());
                }
                _ => {}
            }
            match self.observation(&request) {
                Reply::Pipeline {
                    kind,
                    source,
                    scope,
                    height,
                    substitute_hash,
                } => {
                    let hash = if substitute_hash {
                        Hash::prehashed([0x79; Hash::LENGTH]).to_string()
                    } else {
                        self.signed_transaction().hash().to_string()
                    };
                    json_response(
                        200,
                        norito::json::to_vec(&norito::json!({
                            "hash": hash,
                            "status": { "kind": kind, "block_height": height },
                            "scope": scope,
                            "resolved_from": source,
                        }))?,
                    )
                }
                Reply::Height(blocks) => height_response(blocks),
                Reply::LateHeight(blocks) => {
                    // Deliberately return Ready after consuming the exact remaining budget:
                    // timeout_at alone cannot reject an inner future that never yielded.
                    std::thread::sleep(request.timeout.unwrap() + Duration::from_millis(1));
                    height_response(blocks)
                }
                Reply::Refused(reason) => Ok(Response::builder()
                    .status(503)
                    .header("x-iroha-reject-code", reason.code())
                    .body(Vec::new())?),
                Reply::Malformed => json_response(200, b"malformed status".to_vec()),
                Reply::TransportFailure => Err(OriginalTransportFailure.into()),
                Reply::Pending => {
                    let _retained = PendingRead(Arc::clone(&self.pending_dropped));
                    std::future::pending().await
                }
            }
        })
    }
}

fn client(transport: Arc<SubmissionTransport>) -> Client {
    let config = iroha::config::Config {
        chain: "parliament-completion-test".into(),
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0xA7; Hash::LENGTH],
        ))),
        key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
        account: iroha_test_samples::ALICE_ID.clone(),
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        http_transport_policy: iroha::config::HttpTransportPolicy::Ordinary,
        torii_api_url: "http://parliament-completion.invalid/".parse().unwrap(),
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        basic_auth: None,
        api_token: None,
        transaction_add_nonce: false,
        transaction_ttl: Duration::from_secs(600),
        transaction_status_timeout: Duration::from_secs(600),
        sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
        sorafs_anonymity_policy: iroha_service_model::soranet::AnonymityPolicy::default(),
        sorafs_rollout_phase: iroha_service_model::soranet::RolloutPhase::default(),
    };
    Client::from_client(
        iroha::client::Client::builder(config)
            .http_transport(transport)
            .build()
            .unwrap(),
    )
    .unwrap()
}

fn runtime() -> tokio::runtime::Runtime {
    iroha_test_network::init_instruction_registry();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn transaction(runtime: &tokio::runtime::Runtime, client: &Client) -> SignedTransaction {
    runtime
        .block_on(prepare_parliament_transaction(
            client,
            [Log::new(
                Level::INFO,
                "exact Parliament completion".to_owned(),
            )],
        ))
        .unwrap()
}

#[test]
fn parliament_submission_fences_local_state_before_exact_status() -> Result<()> {
    let runtime = runtime();
    let transport = SubmissionTransport::new([
        pipeline("global", "Applied", "state"),
        pipeline("local", "Queued", "queue"),
        pipeline("local", "Applied", "cache"),
        pipeline("local", "Applied", "state"),
        status(Reply::Refused(iroha::StatusFailureReason::StateBusy)),
        status(Reply::Refused(iroha::StatusFailureReason::DeadlineElapsed)),
        status(Reply::Height(HEIGHT - 1)),
        status(Reply::Height(HEIGHT)),
    ]);
    let client = client(transport.clone());
    runtime.block_on(submit_parliament_instructions(
        &client,
        [Log::new(
            Level::INFO,
            "public Parliament completion".to_owned(),
        )],
    ))?;
    transport.assert_identity_and_budget(OPERATION_TIMEOUT);
    assert!(transport.steps.lock().unwrap().is_empty());
    assert_eq!(
        transport
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.path == "/v1/fees/quote")
            .count(),
        1
    );
    assert_eq!(
        client.client().to_builder().transaction_status_timeout,
        Duration::from_secs(600)
    );
    Ok(())
}

#[test]
fn parliament_submission_refuses_unbound_or_heightless_local_applied() {
    let runtime = runtime();
    for (reply, expected) in [
        (
            Reply::Pipeline {
                kind: "Applied",
                source: "state",
                scope: "local",
                height: Some(HEIGHT),
                substitute_hash: true,
            },
            "hash",
        ),
        (
            Reply::Pipeline {
                kind: "Applied",
                source: "state",
                scope: "global",
                height: Some(HEIGHT),
                substitute_hash: false,
            },
            "scope",
        ),
        (
            Reply::Pipeline {
                kind: "Applied",
                source: "state",
                scope: "local",
                height: None,
                substitute_hash: false,
            },
            "height",
        ),
    ] {
        let transport = SubmissionTransport::new([
            pipeline("global", "Applied", "state"),
            Step {
                path: PIPELINE,
                scope: Some("local"),
                reply,
            },
        ]);
        let client = client(transport.clone());
        let signed = transaction(&runtime, &client);
        let error = runtime
            .block_on(submit_prepared_parliament_transaction_until(
                &client,
                &signed,
                Instant::now() + Duration::from_secs(2),
            ))
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
        transport.assert_identity_and_budget(Duration::from_secs(2));
        assert!(transport.steps.lock().unwrap().is_empty());
        assert!(
            !transport
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|request| request.path == "/status")
        );
    }
}

#[test]
fn parliament_submission_refuses_status_overshoot_and_preserves_errors() {
    let runtime = runtime();
    for reply in [
        Reply::Height(HEIGHT + 1),
        Reply::Refused(iroha::StatusFailureReason::MissingBlock),
        Reply::Malformed,
        Reply::TransportFailure,
    ] {
        let original_transport_error = matches!(reply, Reply::TransportFailure);
        let typed_refusal = matches!(reply, Reply::Refused(_));
        let overshoot = matches!(reply, Reply::Height(_));
        let transport = SubmissionTransport::new([
            pipeline("global", "Applied", "state"),
            pipeline("local", "Applied", "state"),
            status(reply),
        ]);
        let client = client(transport.clone());
        let signed = transaction(&runtime, &client);
        let error = runtime
            .block_on(submit_prepared_parliament_transaction_until(
                &client,
                &signed,
                Instant::now() + Duration::from_secs(2),
            ))
            .unwrap_err();
        if original_transport_error {
            assert!(
                matches!(error.downcast_ref::<iroha::Error>(), Some(iroha::Error::Transport { details, .. }) if details.contains("original Parliament observation transport failure"))
            );
        }
        if typed_refusal {
            assert!(matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::StatusUnavailable {
                    reason: Some(iroha::StatusFailureReason::MissingBlock),
                    ..
                })
            ));
        }
        if overshoot {
            assert!(format!("{error:#}").contains("expected exact height 143, observed 144"));
        }
        transport.assert_identity_and_budget(Duration::from_secs(2));
        assert!(transport.steps.lock().unwrap().is_empty());
    }
}

#[test]
fn parliament_submission_keeps_one_deadline_and_retires_each_pending_phase() {
    let runtime = runtime();
    // The final case inherits a deadline earlier than the fixture's 300-second
    // budget. Rebinding the fixture must retain that shorter caller deadline.
    for (pending_phase, inherit_earlier_deadline) in [(0, false), (1, false), (2, false), (2, true)]
    {
        let mut steps = Vec::new();
        if pending_phase > 0 {
            steps.push(pipeline("global", "Applied", "state"));
        }
        if pending_phase > 1 {
            steps.push(pipeline("local", "Applied", "state"));
        }
        steps.push(Step {
            path: if pending_phase == 2 {
                "/status"
            } else {
                PIPELINE
            },
            scope: match pending_phase {
                0 => Some("global"),
                1 => Some("local"),
                _ => None,
            },
            reply: Reply::Pending,
        });
        let transport = SubmissionTransport::new(steps);
        let client = client(transport.clone());
        let signed = transaction(&runtime, &client);
        let budget = Duration::from_secs(1);
        let deadline = Instant::now() + budget;
        let bounded = if inherit_earlier_deadline {
            client.with_request_deadline(deadline).unwrap()
        } else {
            client.clone()
        };
        let completion_deadline = if inherit_earlier_deadline {
            Instant::now() + OPERATION_TIMEOUT
        } else {
            deadline
        };
        let error = runtime
            .block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(5),
                    submit_prepared_parliament_transaction_until(
                        &bounded,
                        &signed,
                        completion_deadline,
                    ),
                )
                .await
                .expect("the original deadline cannot be renewed")
            })
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(&signed.hash().to_string()),
            "the unresolved identity must survive: {error:#}"
        );
        assert!(transport.pending_dropped.load(Ordering::SeqCst));
        transport.assert_identity_and_budget(budget);
        assert!(transport.steps.lock().unwrap().is_empty());
    }
}

#[test]
fn parliament_submission_expired_completion_never_dispatches() {
    let runtime = runtime();
    let transport = SubmissionTransport::new([]);
    let client = client(transport.clone());
    let signed = transaction(&runtime, &client);
    transport.requests.lock().unwrap().clear();
    let error = runtime
        .block_on(submit_prepared_parliament_transaction_until(
            &client,
            &signed,
            Instant::now(),
        ))
        .expect_err("an expired completion cannot submit");
    assert!(format!("{error:#}").contains(&signed.hash().to_string()));
    assert!(transport.requests.lock().unwrap().is_empty());
    assert!(transport.submitted.lock().unwrap().is_empty());
}

#[test]
fn parliament_submission_refuses_ready_status_after_original_deadline() {
    let runtime = runtime();
    let transport = SubmissionTransport::new([
        pipeline("global", "Applied", "state"),
        pipeline("local", "Applied", "state"),
        status(Reply::LateHeight(HEIGHT)),
    ]);
    let client = client(transport.clone());
    let signed = transaction(&runtime, &client);
    let budget = Duration::from_secs(1);
    let error = runtime
        .block_on(submit_prepared_parliament_transaction_until(
            &client,
            &signed,
            Instant::now() + budget,
        ))
        .expect_err("an exact height returned after the deadline cannot certify completion");
    assert!(format!("{error:#}").contains(&signed.hash().to_string()));
    transport.assert_identity_and_budget(budget);
    assert!(transport.steps.lock().unwrap().is_empty());
}
