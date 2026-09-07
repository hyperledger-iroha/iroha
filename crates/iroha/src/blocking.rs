//! Explicit blocking facade for the asynchronous Iroha SDK transport.

// TODO: Move the remaining synchronous read/query and WebSocket operations out
// of `client::Client`, then expose their canonical forms only through this facade.

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use eyre::{Result, WrapErr, eyre};
use iroha_crypto::{HashOf, KeyPair, PrivateKey};
use iroha_data_model::{
    Identifiable,
    account::AccountId,
    alias_setup::{AliasLifecycleTransactionPlanV1, AliasTransactionPlanV1},
    isi::{InstructionBox, SetParameter, register::RegisterBox},
    metadata::Metadata,
    nexus::{LaneLifecycleParameterV1, LaneLifecyclePlan},
    parameter::Parameter,
    smart_contract::{ContractAddress, ContractAlias},
    transaction::{FeePaymentIntent, SignedTransaction},
};
use iroha_torii_shared::{
    FeeQuoteResponse, validation_fee_api::ValidationFeeProposalDraftRequestV1,
};
use thiserror::Error;

use crate::{
    client::{
        AccountClient as AsyncAccountClient, Client as AsyncClient, ContractCallDraftIntent,
        FeeQuoteRequest, OperatorClient as AsyncOperatorClient, PreparedTransactionPayload,
        SorafsPinRegisterArgs, TransactionWaitOptions, TransactionWaitOutcome,
        validate_validation_fee_draft_response,
    },
    config::Config,
    http::HttpTransport,
};

/// Tokio runtime kind in which a blocking SDK call was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncRuntimeFlavor {
    /// A Tokio current-thread runtime.
    CurrentThread,
    /// A Tokio multi-thread runtime.
    MultiThread,
    /// A future Tokio runtime kind unknown to this SDK release.
    Unknown,
}

impl std::fmt::Display for AsyncRuntimeFlavor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CurrentThread => "current-thread",
            Self::MultiThread => "multi-thread",
            Self::Unknown => "unknown",
        })
    }
}

/// Structured failure returned when a blocking SDK call cannot run safely.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum BlockingCallError {
    /// The caller is already executing inside a Tokio runtime.
    #[error("blocking Iroha SDK call rejected inside a Tokio {flavor} runtime")]
    AsyncRuntime {
        /// Runtime kind observed by the SDK.
        flavor: AsyncRuntimeFlavor,
    },
    /// The facade runtime was poisoned by an earlier panic or already shut down.
    #[error("blocking Iroha SDK runtime is unavailable")]
    RuntimeUnavailable,
}

impl BlockingCallError {
    /// Return the runtime kind for an async-context rejection.
    #[must_use]
    pub const fn async_runtime_flavor(self) -> Option<AsyncRuntimeFlavor> {
        match self {
            Self::AsyncRuntime { flavor } => Some(flavor),
            Self::RuntimeUnavailable => None,
        }
    }
}

pub(crate) fn reject_inside_async_runtime() -> std::result::Result<(), BlockingCallError> {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return Ok(());
    };
    let flavor = match handle.runtime_flavor() {
        tokio::runtime::RuntimeFlavor::CurrentThread => AsyncRuntimeFlavor::CurrentThread,
        tokio::runtime::RuntimeFlavor::MultiThread => AsyncRuntimeFlavor::MultiThread,
        _ => AsyncRuntimeFlavor::Unknown,
    };
    Err(BlockingCallError::AsyncRuntime { flavor })
}

#[derive(Debug)]
struct RuntimeOwner {
    runtime: Mutex<Option<tokio::runtime::Runtime>>,
    #[cfg(test)]
    runs: std::sync::atomic::AtomicUsize,
}

impl RuntimeOwner {
    fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .wrap_err("failed to build the blocking Iroha SDK runtime")?;
        Ok(Self {
            runtime: Mutex::new(Some(runtime)),
            #[cfg(test)]
            runs: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn block_on<F: Future>(&self, future: F) -> std::result::Result<F::Output, BlockingCallError> {
        reject_inside_async_runtime()?;
        let guard = self
            .runtime
            .lock()
            .map_err(|_| BlockingCallError::RuntimeUnavailable)?;
        let runtime = guard
            .as_ref()
            .ok_or(BlockingCallError::RuntimeUnavailable)?;
        #[cfg(test)]
        self.runs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(runtime.block_on(future))
    }
}

impl Drop for RuntimeOwner {
    fn drop(&mut self) {
        let runtime = self
            .runtime
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(runtime) = runtime {
            // This never blocks and is safe even if the final facade handle is
            // dropped while another Tokio runtime is entered.
            runtime.shutdown_background();
        }
    }
}

/// Blocking client context backed by one reusable owned Tokio runtime.
#[derive(Clone, Debug)]
pub struct Client {
    inner: AsyncClient,
    account: AsyncAccountClient,
    runtime: Arc<RuntimeOwner>,
}

impl Client {
    /// Construct an isolated blocking client context.
    ///
    /// # Errors
    /// Returns an error if account authority binding fails or the owned runtime
    /// cannot be created.
    pub fn new(configuration: Config) -> Result<Self> {
        Self::from_client(AsyncClient::new(configuration))
    }

    /// Construct an isolated blocking context with a custom HTTP transport.
    ///
    /// # Errors
    /// Returns an error if account authority binding fails or the owned runtime
    /// cannot be created.
    pub fn with_transport(
        configuration: Config,
        transport: Arc<dyn HttpTransport>,
    ) -> Result<Self> {
        Self::from_client(AsyncClient::with_transport(configuration, transport))
    }

    /// Wrap an asynchronous client context in the explicit blocking facade.
    ///
    /// # Errors
    /// Returns an error if account authority binding fails or the owned runtime
    /// cannot be created.
    pub fn from_client(client: AsyncClient) -> Result<Self> {
        let account = client
            .account_client()
            .wrap_err("failed to bind blocking account authority")?;
        Ok(Self {
            inner: client,
            account,
            runtime: Arc::new(RuntimeOwner::new()?),
        })
    }

    /// Borrow the underlying asynchronous client for local transaction builders
    /// and context inspection.
    #[must_use]
    pub const fn client(&self) -> &AsyncClient {
        &self.inner
    }

    /// Borrow the immutable asynchronous account context.
    #[must_use]
    pub const fn account_client(&self) -> &AsyncAccountClient {
        &self.account
    }

    /// Bind an explicit operator authority to this blocking context.
    ///
    /// # Errors
    /// Returns an error when the endpoint, network, or authority configuration
    /// is invalid.
    pub fn operator_client(&self, key_pair: KeyPair) -> Result<OperatorClient> {
        Ok(OperatorClient {
            inner: self.inner.operator_client(key_pair)?,
            runtime: Arc::clone(&self.runtime),
        })
    }

    /// Submit one signed transaction and return after Torii accepts it.
    ///
    /// # Errors
    /// Returns compatibility, signing, transport, rejection, ambiguity, or
    /// [`BlockingCallError`] failures.
    pub fn submit_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>> {
        self.runtime
            .block_on(self.account.submit_transaction(transaction))?
    }

    /// Submit one signed transaction and wait for state-resolved `Applied` finality.
    ///
    /// Queue-plan ambiguity is retained while finality is reconciled and attached
    /// to an unresolved confirmation error without replaying the transaction.
    ///
    /// # Errors
    /// Returns compatibility, transport, rejection, expiry, timeout, ambiguity,
    /// or [`BlockingCallError`] failures.
    pub fn submit_transaction_and_wait(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>> {
        self.runtime
            .block_on(self.account.submit_transaction_and_wait(transaction))?
    }

    /// Register one `SoraFS` pin manifest through the account-bound workflow.
    ///
    /// # Errors
    /// Returns local validation, fee quote, signing, transport, Torii rejection,
    /// or [`BlockingCallError`] failures.
    pub fn post_sorafs_pin_register(
        &self,
        params: SorafsPinRegisterArgs<'_>,
    ) -> Result<norito::json::Value> {
        self.runtime
            .block_on(self.account.post_sorafs_pin_register(params))?
    }

    /// Quote the exact fee intent for one unsigned transaction payload.
    ///
    /// # Errors
    /// Returns authorization, binding, transport, response, or
    /// [`BlockingCallError`] failures.
    pub fn quote_fees(&self, request: FeeQuoteRequest<'_>) -> Result<FeeQuoteResponse> {
        self.runtime.block_on(self.account.quote_fees(request))?
    }

    /// Build, submit, and wait for one instruction to reach `Applied` finality.
    ///
    /// # Errors
    /// Returns building, fee, signing, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit<I>(
        &self,
        instruction: I,
        fee_payment: FeePaymentIntent,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all(core::iter::once(instruction), fee_payment)
    }

    /// Build, submit, and wait for instructions to reach `Applied` finality.
    ///
    /// # Errors
    /// Returns building, fee, signing, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_all<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
        fee_payment: FeePaymentIntent,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_with_metadata(instructions, fee_payment, Metadata::default())
    }

    /// Build, submit, and wait for one instruction with transaction metadata.
    ///
    /// # Errors
    /// Returns building, fee, signing, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_with_metadata<I>(
        &self,
        instruction: I,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        self.submit_all_with_metadata(core::iter::once(instruction), fee_payment, metadata)
    }

    /// Build, submit, and wait for instructions with transaction metadata.
    ///
    /// # Errors
    /// Returns building, fee, signing, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_all_with_metadata<I>(
        &self,
        instructions: impl IntoIterator<Item = I>,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>>
    where
        I: Into<InstructionBox>,
    {
        const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";
        reject_inside_async_runtime()?;
        let instructions: Vec<InstructionBox> = instructions.into_iter().map(Into::into).collect();
        for instruction in &instructions {
            if let Some(RegisterBox::Role(register_role)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
                && {
                    let name = register_role.object().id().name().as_ref();
                    name == MULTISIG_SIGNATORY
                        || name
                            .strip_prefix(MULTISIG_SIGNATORY)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                }
            {
                return Err(eyre!(
                    "reserved multisig role names may not be registered by clients"
                ));
            }
        }
        let mut payload =
            self.account
                .prepare_transaction(crate::client::AccountTransactionDraft::new(
                    instructions,
                    fee_payment,
                    metadata,
                ))?;
        let quote = self.runtime.block_on(
            self.account
                .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }),
        )??;
        crate::client::apply_fee_quote_intent(&mut payload, quote.intent)
            .wrap_err("apply exact fee quote to transaction payload")?;
        let transaction = self.account.sign_transaction(payload)?;
        self.submit_transaction_and_wait(&transaction)
    }

    /// Submit a lane lifecycle update and wait for `Applied` finality.
    ///
    /// # Errors
    /// Returns status validation, building, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_lane_lifecycle(
        &self,
        plan: LaneLifecyclePlan,
    ) -> Result<HashOf<SignedTransaction>> {
        reject_inside_async_runtime()?;
        let status = self.inner.get_lane_lifecycle_status()?;
        let catalog = status
            .validate()
            .wrap_err("invalid Nexus lane lifecycle status")?;
        let parameter = LaneLifecycleParameterV1::new(&catalog, &status.incarnations, plan)
            .wrap_err("failed to bind Nexus lane incarnation commitments")?
            .into_custom_parameter();
        self.submit(
            SetParameter::new(Parameter::Custom(parameter)),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    }

    /// Verify and submit one alias setup plan, then wait for `Applied` finality.
    ///
    /// # Errors
    /// Returns plan verification, building, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_alias_setup_plan(
        &self,
        plan: &AliasTransactionPlanV1,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<HashOf<SignedTransaction>> {
        let instructions = self.inner.verify_alias_setup_plan(plan)?;
        self.submit_all_with_metadata(instructions, fee_payment, metadata)
            .wrap_err("submit and confirm verified alias setup plan")
    }

    /// Verify and submit one alias lifecycle plan, then wait for `Applied` finality.
    ///
    /// A verified no-op returns `None` without creating a transaction.
    ///
    /// # Errors
    /// Returns plan verification, building, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_alias_lifecycle_plan(
        &self,
        plan: &AliasLifecycleTransactionPlanV1,
        fee_payment: FeePaymentIntent,
        metadata: Metadata,
    ) -> Result<Option<HashOf<SignedTransaction>>> {
        let Some(instruction) = self.inner.verify_alias_lifecycle_plan(plan)? else {
            return Ok(None);
        };
        self.submit_all_with_metadata([instruction], fee_payment, metadata)
            .map(Some)
            .wrap_err("submit and confirm verified alias lifecycle plan")
    }

    /// Draft, verify, submit, and confirm one native validation-fee proposal.
    ///
    /// # Errors
    /// Returns draft validation, building, submission, finality, or
    /// [`BlockingCallError`] failures.
    pub fn submit_validation_fee_proposal(
        &self,
        request: &ValidationFeeProposalDraftRequestV1,
        fee_payment: FeePaymentIntent,
    ) -> Result<HashOf<SignedTransaction>> {
        let response = self.inner.post_validation_fee_proposal_draft(request)?;
        let instruction = validate_validation_fee_draft_response(&response, request)?;
        self.submit(instruction, fee_payment)
    }

    /// Submit one prepared transaction payload and return after acceptance.
    ///
    /// # Errors
    /// Returns compatibility, transport, rejection, ambiguity, or
    /// [`BlockingCallError`] failures.
    pub fn submit_prepared_transaction_payload(
        &self,
        payload: &PreparedTransactionPayload,
    ) -> Result<HashOf<SignedTransaction>> {
        self.runtime
            .block_on(self.account.submit_prepared_transaction_payload(payload))?
    }

    /// Submit prepared transaction payloads as one batch.
    ///
    /// # Errors
    /// Returns compatibility, transport, rejection, acknowledgement, or
    /// [`BlockingCallError`] failures.
    pub fn submit_prepared_transaction_payload_batch(
        &self,
        payloads: &[PreparedTransactionPayload],
    ) -> Result<Vec<HashOf<SignedTransaction>>> {
        self.runtime.block_on(
            self.account
                .submit_prepared_transaction_payload_batch(payloads),
        )?
    }

    /// Prepare, verify, and optionally sign and submit one contract call.
    ///
    /// # Errors
    /// Returns request, draft-validation, signing, submission, or
    /// [`BlockingCallError`] failures.
    #[expect(
        clippy::too_many_arguments,
        reason = "the facade preserves the exact contract-call request binding"
    )]
    pub fn post_contract_call_json(
        &self,
        authority: &AccountId,
        private_key: Option<&PrivateKey>,
        contract_address: Option<&ContractAddress>,
        contract_alias: Option<&ContractAlias>,
        entrypoint: &str,
        payload: Option<&norito::json::Value>,
        caller_metadata: Option<&Metadata>,
        creation_time_ms: Option<u64>,
        transaction_ttl_ms: Option<u64>,
        fee_payment: &FeePaymentIntent,
        draft_intent: &ContractCallDraftIntent,
    ) -> Result<norito::json::Value> {
        self.runtime.block_on(self.account.post_contract_call_json(
            authority,
            private_key,
            contract_address,
            contract_alias,
            entrypoint,
            payload,
            caller_metadata,
            creation_time_ms,
            transaction_ttl_ms,
            fee_payment,
            draft_intent,
        ))?
    }

    /// Wait for a transaction to reach state-resolved `Applied` finality.
    ///
    /// # Errors
    /// Returns status, rejection, expiry, timeout, transport, or
    /// [`BlockingCallError`] failures.
    pub fn wait_for_transaction_applied(
        &self,
        hash: HashOf<SignedTransaction>,
        options: TransactionWaitOptions,
    ) -> Result<TransactionWaitOutcome> {
        self.runtime
            .block_on(self.inner.wait_until_transaction_applied(hash, options))?
    }

    /// Refresh the context-local node compatibility decision.
    ///
    /// # Errors
    /// Returns a typed compatibility, capability-probe, or
    /// [`BlockingCallError`] failure.
    pub fn refresh_capabilities(&self) -> Result<()> {
        self.runtime.block_on(self.inner.refresh_capabilities())?
    }
}

/// Explicit blocking facade for an immutable operator authority context.
#[derive(Clone, Debug)]
pub struct OperatorClient {
    inner: AsyncOperatorClient,
    runtime: Arc<RuntimeOwner>,
}

impl OperatorClient {
    /// Inspect node-local proof retention configuration and live counters.
    ///
    /// # Errors
    /// Returns signing, transport, response-validation, or
    /// [`BlockingCallError`] failures.
    pub fn get_proof_retention_status(&self) -> Result<iroha_torii_shared::ProofRetentionStatus> {
        self.runtime
            .block_on(self.inner.get_proof_retention_status())?
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use iroha_data_model::{ChainId, metadata::Metadata, transaction::FeePaymentIntent};
    use iroha_service_model::soranet::{AnonymityPolicy, RolloutPhase};
    use iroha_test_samples::gen_account_in;
    use iroha_torii_shared::{PipelineTransactionStatus, PipelineTransactionStatusResponse};

    use super::*;
    use crate::{
        client::test_network_id,
        http::{Response, TransportFuture, TransportRequest},
    };

    fn config_factory() -> Config {
        let (account, key_pair) = gen_account_in("wonderland");
        Config {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            network_id: test_network_id(),
            account,
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            key_pair,
            basic_auth: None,
            torii_api_url: "http://127.0.0.1:8080".parse().expect("test URL"),
            torii_request_timeout: crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            transaction_ttl: std::time::Duration::from_secs(5),
            transaction_status_timeout: std::time::Duration::from_secs(10),
            transaction_add_nonce: false,
            sorafs_alias_cache: crate::client::default_alias_policy(),
            sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: RolloutPhase::Canary,
        }
    }

    #[derive(Debug)]
    struct AsyncAcceptTransport {
        async_sends: Arc<AtomicUsize>,
        runtime_threads: Arc<Mutex<Vec<std::thread::ThreadId>>>,
    }

    impl HttpTransport for AsyncAcceptTransport {
        fn send_blocking(&self, _request: TransportRequest) -> Result<Response<Vec<u8>>> {
            panic!("blocking facade must use only the asynchronous transport")
        }

        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            self.async_sends.fetch_add(1, Ordering::Relaxed);
            self.runtime_threads
                .lock()
                .expect("runtime thread log")
                .push(std::thread::current().id());
            let response = match request.url.path() {
                "/v1/node/capabilities" => Response::builder()
                    .status(http::StatusCode::OK)
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(crate::client::compatible_capabilities_body().into_bytes())
                    .expect("capability response"),
                "/v1/pipeline/transactions/status" => {
                    let hash = request
                        .url
                        .query_pairs()
                        .find_map(|(key, value)| (key == "hash").then(|| value.into_owned()))
                        .expect("status request hash");
                    let payload = PipelineTransactionStatusResponse::new(
                        hash,
                        PipelineTransactionStatus {
                            kind: "Applied".to_owned(),
                            block_height: Some(7),
                        },
                        "global".to_owned(),
                        "state".to_owned(),
                    );
                    let body = norito::json::to_string(
                        &norito::json::to_value(&payload).expect("status response value"),
                    )
                    .expect("status response body");
                    Response::builder()
                        .status(http::StatusCode::OK)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .body(body.into_bytes())
                        .expect("status response")
                }
                "/v1/fees/quote" => {
                    let request: iroha_torii_shared::FeeQuoteRequest =
                        norito::json::from_slice(&request.body).expect("fee quote request body");
                    let payload = request.payload;
                    let quote = FeeQuoteResponse {
                        intent: payload.fee_payment_intent().clone(),
                        observation: iroha_torii_shared::FeeQuoteObservation {
                            ledger_time_ms: 1,
                            next_block_height: 1,
                            route_dataspace_id: iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                        },
                        components: Vec::new(),
                        capacities: Vec::new(),
                        decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
                            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(
                                payload.authority().clone(),
                            ),
                            program_revision: None,
                        },
                    };
                    Response::builder()
                        .status(http::StatusCode::OK)
                        .header(http::header::CONTENT_TYPE, "application/json")
                        .body(norito::json::to_vec(&quote).expect("fee quote response body"))
                        .expect("fee quote response")
                }
                _ => Response::new(Vec::new()),
            };
            Box::pin(async move { Ok(response) })
        }
    }

    fn accepting_client() -> (
        Client,
        Arc<AtomicUsize>,
        Arc<Mutex<Vec<std::thread::ThreadId>>>,
    ) {
        let async_sends = Arc::new(AtomicUsize::new(0));
        let runtime_threads = Arc::new(Mutex::new(Vec::new()));
        let async_client = AsyncClient::with_transport(
            config_factory(),
            Arc::new(AsyncAcceptTransport {
                async_sends: Arc::clone(&async_sends),
                runtime_threads: Arc::clone(&runtime_threads),
            }),
        );
        (
            Client::from_client(async_client).expect("blocking client"),
            async_sends,
            runtime_threads,
        )
    }

    #[test]
    fn one_owned_runtime_is_reused_across_calls_and_clones() {
        let (client, async_sends, runtime_threads) = accepting_client();
        let clone = client.clone();
        assert!(Arc::ptr_eq(&client.runtime, &clone.runtime));
        let transaction = {
            let account = client.account_client();
            account
                .prepare_transaction(crate::client::AccountTransactionDraft::new(
                    Vec::<iroha_data_model::isi::InstructionBox>::new(),
                    FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))
                .and_then(|payload| account.sign_transaction(payload))
        }
        .expect("build transaction");

        client
            .refresh_capabilities()
            .expect("blocking capability refresh");
        client
            .submit_transaction(&transaction)
            .expect("first blocking submission");
        clone
            .submit_transaction(&transaction)
            .expect("second blocking submission");

        assert_eq!(
            async_sends.load(Ordering::Relaxed),
            3,
            "one compatibility probe and two transaction submissions expected"
        );
        assert_eq!(
            client.runtime.runs.load(Ordering::Relaxed),
            3,
            "refresh and both submissions must enter the shared runtime"
        );
        let threads = runtime_threads.lock().expect("runtime thread log");
        assert_eq!(threads.len(), 3);
        assert!(threads.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn fee_quote_uses_the_blocking_facades_owned_async_runtime() {
        let (client, async_sends, _) = accepting_client();
        let payload = client
            .account_client()
            .prepare_transaction(crate::client::AccountTransactionDraft::new(
                Vec::<iroha_data_model::isi::InstructionBox>::new(),
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
            .expect("build exact fee quote payload");

        let quote = client
            .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
            .expect("blocking facade fee quote");

        assert_eq!(quote.intent, payload.fee_payment_intent().clone());
        assert_eq!(async_sends.load(Ordering::Relaxed), 1);
        assert_eq!(client.runtime.runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn submit_transaction_and_wait_returns_the_exact_applied_hash() {
        let (client, async_sends, _) = accepting_client();
        let transaction = {
            let account = client.account_client();
            account
                .prepare_transaction(crate::client::AccountTransactionDraft::new(
                    Vec::<iroha_data_model::isi::InstructionBox>::new(),
                    FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))
                .and_then(|payload| account.sign_transaction(payload))
        }
        .expect("build transaction");
        let expected = transaction.hash();

        let actual = client
            .submit_transaction_and_wait(&transaction)
            .expect("transaction reaches state-resolved Applied finality");

        assert_eq!(actual, expected);
        assert_eq!(
            async_sends.load(Ordering::Relaxed),
            3,
            "one capability probe, one submission, and one status poll expected"
        );
    }

    #[tokio::test]
    async fn blocking_facade_rejects_async_runtime_and_drops_safely() {
        let (client, async_sends, _) = accepting_client();
        let transaction = {
            let account = client.account_client();
            account
                .prepare_transaction(crate::client::AccountTransactionDraft::new(
                    Vec::<iroha_data_model::isi::InstructionBox>::new(),
                    FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))
                .and_then(|payload| account.sign_transaction(payload))
        }
        .expect("build transaction");
        let error = client
            .submit_transaction(&transaction)
            .expect_err("blocking call inside Tokio must reject");
        let typed = error
            .downcast_ref::<BlockingCallError>()
            .expect("typed blocking-call error");
        assert_eq!(
            typed.async_runtime_flavor(),
            Some(AsyncRuntimeFlavor::CurrentThread)
        );
        assert_eq!(async_sends.load(Ordering::Relaxed), 0);
        drop(client);
    }
}
