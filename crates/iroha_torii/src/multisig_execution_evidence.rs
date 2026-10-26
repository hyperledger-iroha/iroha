//! Bounded data-only publication of immutable multisig records from a certified pre-tail cut.
use super::*;
use iroha_core::{
    smartcontracts::isi::multisig::{
        multisig_approval_outcome_state_key, multisig_proposal_terminal_execution_state_key,
    },
    state::AllocationBudget,
    sumeragi::certified_chain::CommittedBlock,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{account::AccountId, isi::InstructionBox};
use iroha_executor_data_model::isi::multisig::MultisigProposalId;
use iroha_torii_shared::multisig_execution_evidence::{
    MULTISIG_EXECUTION_EVIDENCE_MAX_BYTES_V1, MultisigExecutionEvidenceRefV1,
};
use norito::json::{BoundedJsonError, JsonSerialize as _, JsonWriteSink};
use std::str::FromStr;
const ROUTE: &str =
    iroha_torii_shared::multisig_execution_evidence::MULTISIG_EXECUTION_EVIDENCE_PATH_V1;

pub async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::Path((account, entrypoint, proposal, instructions)): axum::extract::Path<(
        String,
        String,
        String,
        String,
    )>,
    Extension(finality_interval::RouteExecutionDeadline(deadline)): Extension<
        finality_interval::RouteExecutionDeadline,
    >,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let (account, entrypoint, proposal, instructions) =
        selectors(&account, &entrypoint, &proposal, &instructions)?;
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(&headers, Some(remote.ip()), ROUTE, principal);
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let mut admission = acquire_query_admission(app.as_ref(), true).await?;
    history_producer::HistoryProducerOwner::validate_finality_inheritance(&admission)?;
    let bytes = app.query_fanout_working_set_bytes;
    let memory = crate::acquire_query_pool_credit(&app, bytes)
        .await
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    // Tip certification is a finality read under that route's own bodyless admission,
    // taken without waiting after the response credit so neither waits holding the other.
    if admission.fanout_memory.is_none() {
        admission.fanout_memory = Some(match crate::try_acquire_bodyless_finality_memory(&app) {
            Ok(reservation) => reservation,
            Err(response) => return Ok(response),
        });
    }
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission)?;
    let budget = AllocationBudget::new(bytes);
    let max_response = app
        .torii_proxy_max_response_bytes
        .min(MULTISIG_EXECUTION_EVIDENCE_MAX_BYTES_V1);
    let state = app.state.clone();
    let response = finality_interval::run_with_stop(admission, owner, move |owner, cancelled| {
        let memory = memory;
        let retained_tip = certified_tip(&state, &owner, deadline, &cancelled)?;
        let tip = retained_tip.as_ref();
        let height = tip.height();
        let context_id = Hash::prehashed(*tip.id().0.as_ref());
        let keys = [
            multisig_approval_outcome_state_key(entrypoint, &account, &proposal),
            multisig_proposal_terminal_execution_state_key(entrypoint, &account, &proposal),
        ];
        check_request_stop(deadline, &cancelled)?;
        let mut body = state
            .with_native_multisig_execution_snapshot_v1(
                tip,
                &keys,
                &budget,
                |snapshot, outcome, terminal, target| {
                    // Snapshot capture is complete; do not encode after the request stops.
                    check_request_stop(deadline, &cancelled)
                        .map_err(|_| "multisig execution evidence request stopped".to_owned())?;
                    let payload = MultisigExecutionEvidenceRefV1::new(
                        &height,
                        &context_id,
                        &account,
                        &entrypoint,
                        &proposal,
                        &instructions,
                        snapshot,
                        outcome,
                        terminal,
                        target,
                    );
                    let body = encode(&payload, format, max_response, &budget)
                        .map_err(|_| "multisig execution evidence serialization refused".to_owned())?;
                    check_request_stop(deadline, &cancelled)
                        .map_err(|_| "multisig execution evidence request stopped".to_owned())?;
                    Ok(body)
                },
            )
            .map_err(|_| unavailable())?;
        check_request_stop(deadline, &cancelled)?;
        body.memory = Some(memory);
        let mut response = AxResponse::new(Body::from(Bytes::from_owner(body)));
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(match format {
                ResponseFormat::Norito => "application/x-norito",
                ResponseFormat::Json => "application/json",
            }),
        );
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
        check_request_stop(deadline, &cancelled)?;
        Ok(response)
    })
    .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}
/// The current tip's committed receipt, certified through the State's finality ladder.
///
/// The same native certificate checks as a walk from signed genesis, resumed from a
/// strictly lower retained checkpoint when available. Same-height requests may repeat
/// the final gap; optional retention failure can require a longer direct proof walk.
fn certified_tip(
    state: &CoreState,
    owner: &history_producer::HistoryProducerOwner,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<impl AsRef<CommittedBlock> + use<>, Error> {
    check_request_stop(deadline, cancelled)?;
    let height = {
        let view = state.try_view_once().map_err(|_| unavailable())?;
        check_request_stop(deadline, cancelled)?;
        u64::try_from(view.height())
            .ok()
            .and_then(NonZeroU64::new)
            .filter(|height| height.get() >= 2)
            .ok_or_else(unavailable)?
    };
    check_request_stop(deadline, cancelled)?;
    let proof_bytes = NonZeroUsize::new(owner.response_bytes()).ok_or_else(capacity)?;
    let tip = state
        .read_certified_ladder_receipt(
            &owner.finality_budget()?,
            height,
            proof_bytes,
            deadline,
            cancelled,
        )
        .map_err(|_| unavailable())?;
    check_request_stop(deadline, cancelled)?;
    Ok(tip)
}

// Preserve this route's existing unavailable response for original stop refusals.
// HTTP middleware still owns its timeout response; this only stops physical suffix work.
fn check_request_stop(deadline: Instant, cancelled: &AtomicBool) -> Result<(), Error> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Acquire) {
        Err(unavailable())
    } else {
        Ok(())
    }
}
type MultisigExecutionSelectors = (
    AccountId,
    [u8; 32],
    MultisigProposalId,
    HashOf<Vec<InstructionBox>>,
);
fn selectors(
    account: &str,
    entrypoint: &str,
    proposal: &str,
    instructions: &str,
) -> Result<MultisigExecutionSelectors, Error> {
    if account.is_empty()
        || account.len() > 1024
        || [entrypoint, proposal, instructions].iter().any(|s| {
            s.len() != 64
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return Err(conversion_error(
            "multisig execution selectors must be exact bounded native identities".into(),
        ));
    }
    let native = AccountId::parse_encoded(account)
        .map_err(|_| conversion_error("invalid canonical multisig account".into()))?;
    if native.to_string() != account {
        return Err(conversion_error(
            "multisig account must be canonical".into(),
        ));
    }
    let entrypoint = Hash::from_str(entrypoint)
        .map_err(|_| conversion_error("invalid native entrypoint hash".into()))?;
    let instructions = HashOf::<Vec<InstructionBox>>::from_str(instructions)
        .map_err(|_| conversion_error("invalid native instructions hash".into()))?;
    let proposal = proposal
        .parse()
        .map_err(|_| conversion_error("invalid proposal occurrence".into()))?;
    Ok((native, *entrypoint.as_ref(), proposal, instructions))
}
fn unavailable() -> Error {
    Error::AppServiceUnavailable {code: "multisig_execution_evidence_unavailable", message: "Both immutable execution records are not yet available in the current certified native cut.".into()}
}
fn capacity() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
    ))
}
struct EncodedBody {
    bytes: iroha_allocation::ChargedBuffer<u8>,
    memory: Option<QueryFanoutMemoryReservation>,
}
impl AsRef<[u8]> for EncodedBody {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
struct ChargedWriter(iroha_allocation::ChargedBuffer<u8>);
impl std::io::Write for ChargedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.0.capacity().saturating_sub(self.0.as_slice().len()) {
            return Err(std::io::Error::other("native response length changed"));
        }
        for byte in bytes {
            self.0.push_reserved(*byte);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl JsonWriteSink for ChargedWriter {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        std::io::Write::write_all(self, value.as_bytes())
            .map_err(|_| BoundedJsonError::LengthMismatch)
    }
}
struct CountJson {
    length: usize,
    limit: usize,
    depth: usize,
}
impl JsonWriteSink for CountJson {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        let next = self
            .length
            .checked_add(value.len())
            .ok_or(BoundedJsonError::BodyTooLarge)?;
        if next > self.limit {
            return Err(BoundedJsonError::BodyTooLarge);
        }
        self.length = next;
        Ok(())
    }
    fn begin_container(&mut self) -> Result<(), BoundedJsonError> {
        let next = self
            .depth
            .checked_add(1)
            .ok_or(BoundedJsonError::Unsupported)?;
        if next >= norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            return Err(BoundedJsonError::Unsupported);
        }
        self.depth = next;
        Ok(())
    }
    fn end_container(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
fn encode(
    payload: &MultisigExecutionEvidenceRefV1<'_>,
    format: ResponseFormat,
    limit: usize,
    budget: &AllocationBudget,
) -> Result<EncodedBody, Error> {
    let length = match format {
        ResponseFormat::Norito => {
            norito::canonical_frame_len(payload).map_err(|_| unavailable())?
        }
        ResponseFormat::Json => {
            let mut count = CountJson {
                length: 0,
                limit,
                depth: 0,
            };
            payload
                .json_serialize_to(&mut count)
                .map_err(|_| capacity())?;
            count.length
        }
    };
    if length > limit {
        return Err(capacity());
    }
    let bytes = iroha_allocation::ChargedBuffer::new(length, budget).map_err(|_| capacity())?;
    let mut writer = ChargedWriter(bytes);
    match format {
        ResponseFormat::Norito => norito::core::write_canonical_to_writer(payload, &mut writer)
            .map_err(|_| unavailable())?,
        ResponseFormat::Json => payload
            .json_serialize_to(&mut writer)
            .map_err(|_| unavailable())?,
    }
    if writer.0.as_slice().len() != length {
        return Err(unavailable());
    }
    Ok(EncodedBody {
        bytes: writer.0,
        memory: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_selectors_admit_exact_native_identities_and_refuse_alternate_spellings() {
        let account = iroha_test_samples::ALICE_ID.to_string();
        let entrypoint = Hash::new(b"actual selector encoding")
            .to_string()
            .to_ascii_lowercase();
        let instructions = HashOf::new(&Vec::<InstructionBox>::new())
            .to_string()
            .to_ascii_lowercase();
        let occurrence = "11".repeat(32);
        let (selected, hash, id, proposal) =
            selectors(&account, &entrypoint, &occurrence, &instructions).unwrap();
        assert_eq!(id.to_string(), occurrence);
        assert_eq!(selected.to_string(), account);
        assert_eq!(hash, *Hash::from_str(&entrypoint).unwrap().as_ref());
        assert_eq!(proposal.to_string().to_ascii_lowercase(), instructions);
        for wrong in [
            entrypoint.to_ascii_uppercase(),
            format!(" {entrypoint}"),
            format!("{entrypoint}00"),
        ] {
            assert!(selectors(&account, &wrong, &occurrence, &instructions).is_err());
        }
        assert!(
            selectors(
                &format!(" {account}"),
                &entrypoint,
                &occurrence,
                &instructions
            )
            .is_err()
        );
        assert!(selectors(&account, &entrypoint, &occurrence, "1").is_err());
    }
    #[test]
    fn bounded_native_body_never_allocates_a_replacement_pool_on_growth() {
        use std::io::Write as _;
        let budget = AllocationBudget::new(3);
        let mut writer = ChargedWriter(iroha_allocation::ChargedBuffer::new(3, &budget).unwrap());
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        let body = Bytes::from_owner(EncodedBody {
            bytes: writer.0,
            memory: None,
        });
        assert!(budget.reserved_bytes() > 0);
        drop(body);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn certified_tip_is_the_ladder_certified_receipt_of_the_current_height() {
        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        while chain.height() < 4 {
            chain.commit(Vec::new());
        }
        let working = 48_000_000;
        let pool = ByteWeightedMemoryPool::new(working).unwrap();
        // Each request prepays finality from its own bodyless admission, as the route does.
        let owner = || {
            let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
                pool.try_acquire_parts([working as u64]).unwrap(),
                QueryFanoutMemoryEnvelope::for_bodyless_finality(working).unwrap(),
                pool.generation(),
            )
            .unwrap();
            history_producer::HistoryProducerOwner::from_reservation(&reservation).unwrap()
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        let running = AtomicBool::new(false);
        let tip = certified_tip(chain.state(), &owner(), deadline, &running).unwrap();
        assert_eq!(tip.as_ref().height(), 4);
        assert_eq!(tip.as_ref().id(), chain.committed(4).id());
        drop(tip);
        // A later tip certifies the new height from the same State's ladder.
        chain.commit(Vec::new());
        let tip = certified_tip(chain.state(), &owner(), deadline, &running).unwrap();
        assert_eq!(tip.as_ref().id(), chain.committed(5).id());
        drop(tip);
        // A stopped request is unavailable rather than answered from an older cut.
        assert!(matches!(
            certified_tip(chain.state(), &owner(), deadline, &AtomicBool::new(true)),
            Err(Error::AppServiceUnavailable { .. })
        ));
        assert_eq!(pool.available_bytes(), 48_000_000);
    }

    #[test]
    fn expired_certified_tip_refuses_before_finality_grant_debit() {
        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let working = 48_000_000;
        let pool = ByteWeightedMemoryPool::new(working).unwrap();
        let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
            pool.try_acquire_parts([working as u64]).unwrap(),
            QueryFanoutMemoryEnvelope::for_bodyless_finality(working).unwrap(),
            pool.generation(),
        )
        .unwrap();
        let owner =
            history_producer::HistoryProducerOwner::from_reservation(&reservation).unwrap();
        let consumed = owner.allocation_context().consumed_allocated_bytes();
        let result = certified_tip(
            chain.state(),
            &owner,
            Instant::now(),
            &AtomicBool::new(false),
        );
        assert!(matches!(result, Err(Error::AppServiceUnavailable { .. })));
        assert_eq!(owner.allocation_context().consumed_allocated_bytes(), consumed);
        drop(owner);
        drop(reservation);
        assert_eq!(pool.available_bytes(), 48_000_000);
    }

    #[tokio::test]
    async fn mounted_expired_evidence_route_releases_real_admission_without_native_reads() {
        use iroha_core::{state::World, sumeragi::test_chain::{CertifiedTestChain, TestChainConfig}};
        use tower::ServiceExt as _;
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        while chain.height() < 4 { chain.commit(Vec::new()); }
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let fixture = Arc::get_mut(&mut app).unwrap();
        fixture.state = Arc::clone(chain.state());
        fixture.kura = Arc::clone(chain.kura());
        let query = Arc::clone(&app.query_inflight);
        let heavy = Arc::clone(&app.query_heavy_inflight);
        let pool = app.query_fanout_inflight.clone();
        let before = (query.available_permits(), heavy.available_permits(), pool.available_bytes());
        // The actual timeout middleware runs outside this fixture's already-expired
        // deadline. The inner extension models a request whose budget was consumed
        // before this handler, without waiting or constructing a replacement worker.
        let router = axum::Router::new()
            .route(ROUTE, axum::routing::get(handler))
            .layer(Extension(finality_interval::RouteExecutionDeadline(Instant::now())))
            .layer(axum::middleware::from_fn(enforce_route_timeout))
            .with_state(app);
        let account = iroha_test_samples::ALICE_ID.to_string();
        let entrypoint = Hash::new(b"expired evidence route").to_string().to_ascii_lowercase();
        let proposal = "11".repeat(32);
        let instructions = HashOf::new(&Vec::<InstructionBox>::new()).to_string().to_ascii_lowercase();
        let uri = format!("/v1/multisig/execution-evidence/{account}/{entrypoint}/{proposal}/{instructions}");
        for accept in ["application/x-norito", "application/json"] {
            chain.kura().reset_canonical_query_reads_for_test();
            let mut request = axum::http::Request::builder().uri(&uri)
                .header(axum::http::header::ACCEPT, accept).body(Body::empty()).unwrap();
            request.extensions_mut().insert(crate::loopback_connect_info());
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            drop(response);
            tokio::time::timeout(Duration::from_secs(2), async {
                while (query.available_permits(), heavy.available_permits(), pool.available_bytes()) != before {
                    tokio::task::yield_now().await;
                }
            }).await.unwrap();
            assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_evidence_worker_retains_original_admission_until_physical_retirement() {
        use iroha_core::{state::World, sumeragi::test_chain::{CertifiedTestChain, TestChainConfig}};
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let state = Arc::clone(chain.state());
        let working = 48_000_000;
        let pool = ByteWeightedMemoryPool::new(working).unwrap();
        let initial_available = pool.available_bytes();
        let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
            pool.try_acquire_parts([working as u64]).unwrap(),
            QueryFanoutMemoryEnvelope::for_bodyless_finality(working).unwrap(), pool.generation(),
        ).unwrap();
        let query = Arc::new(tokio::sync::Semaphore::new(1));
        let admission = QueryAdmissionPermit {
            _query: Arc::clone(&query).try_acquire_owned().unwrap(),
            heavy: None,
            body: None,
            fanout_memory: Some(reservation),
        };
        let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
        let frames = owner.cold_frames().clone();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let (finished, result) = tokio::sync::oneshot::channel();
        chain.kura().reset_canonical_query_reads_for_test();
        let request = tokio::spawn(finality_interval::run_with_stop(admission, owner, move |owner, cancelled| {
            let consumed = owner.allocation_context().consumed_allocated_bytes();
            started.send(()).unwrap();
            held.recv_timeout(Duration::from_secs(5)).unwrap();
            let stopped = matches!(certified_tip(&state, &owner, Instant::now() + Duration::from_secs(60), &cancelled),
                Err(Error::AppServiceUnavailable { .. }));
            let debit = owner.allocation_context().consumed_allocated_bytes() - consumed;
            finished.send((stopped, debit)).unwrap();
            Ok(())
        }));
        entered.await.unwrap();
        // Only this test's request future is cancelled; its native worker closes naturally.
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        assert_eq!(query.available_permits(), 0);
        assert!(pool.try_acquire_parts([working as u64]).is_none());
        release.send(()).unwrap();
        assert_eq!(result.await.unwrap(), (true, 0));
        tokio::time::timeout(Duration::from_secs(2), async {
            while query.available_permits() != 1 || pool.available_bytes() != initial_available {
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
        assert_eq!(frames.reserved_bytes(), 0);
    }
}
