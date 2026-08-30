//! Authenticated submission and terminal-state resolution for native Exact12 actions.
//!
//! HTTP acceptance and public pipeline labels are deliberately insufficient here. Submission
//! authenticates the exact signed transaction and a fresh committed capability manifest. A
//! successful terminal view additionally requires both authenticated committed transaction
//! details and the finalized native execution receipt written with the ledger effect.

use std::{error::Error as _, fmt};

use crate::http::RequestBuilder as _;
use eyre::eyre;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{error::InstructionExecutionError, privacy::SubmitPrivacyProofV1},
    privacy::{
        IrohaZkAmsProofV1, PrivacyActionExecutionReceiptViewV1, PrivacyActionLocalStateV1,
        PrivacyActionOperationViewV1, PrivacyActionTerminalChainStateV1,
        PrivacyCompiledProfileResultV1, PrivacyConsensusLimitsV1, PrivacyEngineIdV1,
        PrivacyEngineManifestDigestV1, PrivacyExact12ActionOperationV1,
        PrivacyExact12ActionRequestV1, PrivacyExact12CapabilityManifestDigestV1,
        PrivacyExact12CapabilityManifestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
        PrivacyProofSystemIdV1, PrivacyProofV1, PrivacyStatementDigestV1,
        PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    },
    query::{error::QueryExecutionFail, privacy::prelude::FindPrivacyActionExecutionReceiptV1},
    transaction::{
        Executable, SignedTransaction, TransactionEntrypoint, error::TransactionRejectionReason,
    },
};
use iroha_torii_shared::{PipelineTransactionStatusResponse, uri as torii_uri};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use thiserror::Error;
use url::Url;

use super::{
    APPLICATION_JSON, APPLICATION_NORITO, Client, HttpMethod,
    PIPELINE_TRANSACTION_STATUS_RESPONSE_MAX_BYTES, QueryError, StatusCode,
    TRANSACTION_SUBMISSION_RESPONSE_MAX_BYTES, TransactionResponseHandler, join_torii_url,
};

const PRIVACY_ACTION_INDEX_V1: u32 = 0;

/// Fail-closed error returned by the authenticated Rust Exact12 controller.
#[derive(Debug, Error)]
pub enum PrivacyActionClientErrorV1 {
    /// A request, capability, pipeline result, or finalized binding was invalid.
    #[error("{reason}")]
    Invalid {
        /// Stable non-secret diagnostic. Untrusted rejection text is never reflected here.
        reason: &'static str,
    },
    /// An authenticated Torii transport or query failed.
    #[error("{context}: {source}")]
    Transport {
        /// Operation whose transport failed.
        context: &'static str,
        /// Underlying transport or query failure.
        #[source]
        source: eyre::Report,
    },
}

impl PrivacyActionClientErrorV1 {
    fn invalid(reason: &'static str) -> Self {
        Self::Invalid { reason }
    }

    fn transport(context: &'static str, source: impl Into<eyre::Report>) -> Self {
        Self::Transport {
            context,
            source: source.into(),
        }
    }
}

/// Opaque provenance for one authenticated Exact12 submission.
///
/// There is intentionally no public constructor or decoder. A detached
/// [`PrivacyActionOperationViewV1`] is a useful projection but is not authority to perform a
/// status lookup. The controller also re-inspects the original signed request on every refresh.
#[derive(Clone)]
pub struct AuthenticatedPrivacyActionHandleV1 {
    request: PrivacyExact12ActionRequestV1,
    inspection: InspectedActionBindingV1,
    network_id: iroha_data_model::NetworkId,
    authority: AccountId,
    torii_url: Url,
    view: PrivacyActionOperationViewV1,
    typed_rejection_reason: Option<TransactionRejectionReason>,
}

impl fmt::Debug for AuthenticatedPrivacyActionHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedPrivacyActionHandleV1")
            .field("network_id", &self.network_id)
            .field("authority", &self.authority)
            .field("torii_url", &self.torii_url)
            .field("view", &self.view)
            .field(
                "has_typed_rejection_reason",
                &self.typed_rejection_reason.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl AuthenticatedPrivacyActionHandleV1 {
    /// Borrow the validated public projection associated with this authenticated handle.
    #[must_use]
    pub const fn view(&self) -> &PrivacyActionOperationViewV1 {
        &self.view
    }

    /// Borrow the committed typed rejection reason when the terminal result is rejected.
    ///
    /// The public view retains its canonical bounded textual projection for cross-SDK wire
    /// compatibility; this accessor preserves the native typed reason for Rust callers.
    #[must_use]
    pub const fn typed_rejection_reason(&self) -> Option<&TransactionRejectionReason> {
        self.typed_rejection_reason.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EnvelopeProfileBindingV1 {
    proof_system_id: PrivacyProofSystemIdV1,
    engine_id: PrivacyEngineIdV1,
    parameter_id: PrivacyParameterIdV1,
    parameter_digest: PrivacyParameterDigestV1,
    verifier_digest: PrivacyVerifierDigestV1,
    statement_schema_digest: PrivacyStatementSchemaDigestV1,
    engine_manifest_digest: PrivacyEngineManifestDigestV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InspectedActionBindingV1 {
    operation: PrivacyExact12ActionOperationV1,
    transaction_hash: [u8; Hash::LENGTH],
    transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    statement_digest: PrivacyStatementDigestV1,
    proof_envelope_hash: [u8; Hash::LENGTH],
    profile: EnvelopeProfileBindingV1,
}

struct InspectedActionV1 {
    signed_transaction: SignedTransaction,
    binding: InspectedActionBindingV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AuthenticatedCommittedActionResultV1 {
    Applied,
    Rejected(TransactionRejectionReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AuthenticatedCommittedActionV1 {
    block_height: u64,
    result: AuthenticatedCommittedActionResultV1,
}

trait PrivacyActionTransportV1 {
    fn fetch_capability_manifest(
        &self,
    ) -> Result<PrivacyExact12CapabilityManifestV1, PrivacyActionClientErrorV1>;

    fn submit_signed_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>, PrivacyActionClientErrorV1>;

    fn fetch_global_pipeline_status(
        &self,
        transaction_hash: HashOf<SignedTransaction>,
    ) -> Result<Option<PipelineTransactionStatusResponse>, PrivacyActionClientErrorV1>;

    fn fetch_committed_action(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<Option<AuthenticatedCommittedActionV1>, PrivacyActionClientErrorV1>;

    fn fetch_execution_receipt(
        &self,
        binding: &InspectedActionBindingV1,
    ) -> Result<Option<PrivacyActionExecutionReceiptViewV1>, PrivacyActionClientErrorV1>;
}

struct ClientPrivacyActionTransportV1<'client>(&'client Client);

impl PrivacyActionTransportV1 for ClientPrivacyActionTransportV1<'_> {
    fn fetch_capability_manifest(
        &self,
    ) -> Result<PrivacyExact12CapabilityManifestV1, PrivacyActionClientErrorV1> {
        self.0.get_privacy_capabilities().map_err(|error| {
            PrivacyActionClientErrorV1::transport(
                "failed to fetch authenticated Exact12 capability manifest",
                error,
            )
        })
    }

    fn submit_signed_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<HashOf<SignedTransaction>, PrivacyActionClientErrorV1> {
        self.0
            .ensure_transaction_submit_compatibility()
            .map_err(|error| {
                PrivacyActionClientErrorV1::transport(
                    "Exact12 transaction compatibility admission failed",
                    error,
                )
            })?;
        let payload = Client::prepare_transaction_payload(transaction);
        let hash = payload.hash();
        let url = join_torii_url(&self.0.torii_url, torii_uri::TRANSACTION);
        let request = self
            .0
            .account_signed_request(HttpMethod::POST, url, payload.as_bytes().to_vec())
            .map_err(|error| {
                PrivacyActionClientErrorV1::transport(
                    "failed to authenticate Exact12 transaction request",
                    error,
                )
            })?
            .header("Content-Type", APPLICATION_NORITO)
            .header("Accept", APPLICATION_NORITO)
            .max_response_bytes(TRANSACTION_SUBMISSION_RESPONSE_MAX_BYTES);
        let response = self.0.send_builder(request).map_err(|error| {
            PrivacyActionClientErrorV1::transport(
                "failed to dispatch authenticated Exact12 transaction",
                error,
            )
        })?;
        TransactionResponseHandler::handle(&response).map_err(|error| {
            PrivacyActionClientErrorV1::transport(
                "Torii rejected authenticated Exact12 transaction dispatch",
                error,
            )
        })?;
        Ok(hash)
    }

    fn fetch_global_pipeline_status(
        &self,
        transaction_hash: HashOf<SignedTransaction>,
    ) -> Result<Option<PipelineTransactionStatusResponse>, PrivacyActionClientErrorV1> {
        let hash = hex::encode(transaction_hash.as_ref());
        let mut url = join_torii_url(&self.0.torii_url, "v1/pipeline/transactions/status");
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("hash", &hash);
            query.append_pair("scope", "global");
        }
        let request = self
            .0
            .account_signed_request(HttpMethod::GET, url, Vec::new())
            .map_err(|error| {
                PrivacyActionClientErrorV1::transport(
                    "failed to authenticate Exact12 global-status request",
                    error,
                )
            })?
            .header("Accept", APPLICATION_JSON)
            .max_response_bytes(PIPELINE_TRANSACTION_STATUS_RESPONSE_MAX_BYTES);
        let response = self.0.send_builder(request).map_err(|error| {
            PrivacyActionClientErrorV1::transport(
                "failed to fetch authenticated Exact12 global status",
                error,
            )
        })?;
        match response.status() {
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Ok(None),
            StatusCode::OK | StatusCode::ACCEPTED => {
                if response.body().is_empty() {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "authenticated Exact12 global status omitted its body",
                    ));
                }
                if Client::response_content_type(&response) != APPLICATION_JSON {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "authenticated Exact12 global status did not use exact application/json",
                    ));
                }
                let status = norito::json::from_slice(response.body()).map_err(|error| {
                    PrivacyActionClientErrorV1::transport(
                        "failed to decode authenticated Exact12 global status",
                        eyre!(error),
                    )
                })?;
                Ok(Some(status))
            }
            _ => Err(PrivacyActionClientErrorV1::transport(
                "Torii rejected authenticated Exact12 global-status request",
                eyre!(
                    "unexpected HTTP status {}; body omitted from diagnostic",
                    response.status()
                ),
            )),
        }
    }

    fn fetch_committed_action(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<Option<AuthenticatedCommittedActionV1>, PrivacyActionClientErrorV1> {
        let details = match self
            .0
            .get_transaction_details(transaction.hash_as_entrypoint())
        {
            Ok(details) => details,
            Err(error) if query_error_is_not_found(&error) => return Ok(None),
            Err(error) => {
                return Err(PrivacyActionClientErrorV1::transport(
                    "failed to fetch authenticated Exact12 committed details",
                    eyre!(error),
                ));
            }
        };
        if details.block_height == 0 {
            return Err(PrivacyActionClientErrorV1::invalid(
                "authenticated Exact12 committed details used height zero",
            ));
        }
        let TransactionEntrypoint::External(committed_transaction) =
            details.transaction.entrypoint()
        else {
            return Err(PrivacyActionClientErrorV1::invalid(
                "authenticated Exact12 committed details changed the entrypoint kind",
            ));
        };
        if committed_transaction != transaction {
            return Err(PrivacyActionClientErrorV1::invalid(
                "authenticated Exact12 committed details changed the exact signed transaction",
            ));
        }
        let result = match details.transaction.result().as_ref() {
            Ok(_) => AuthenticatedCommittedActionResultV1::Applied,
            Err(reason) => AuthenticatedCommittedActionResultV1::Rejected(reason.clone()),
        };
        Ok(Some(AuthenticatedCommittedActionV1 {
            block_height: details.block_height,
            result,
        }))
    }

    fn fetch_execution_receipt(
        &self,
        binding: &InspectedActionBindingV1,
    ) -> Result<Option<PrivacyActionExecutionReceiptViewV1>, PrivacyActionClientErrorV1> {
        let query = FindPrivacyActionExecutionReceiptV1::new(
            binding.operation.protocol_id(),
            binding.transaction_hash,
            PRIVACY_ACTION_INDEX_V1,
        );
        match self.0.query_single(query) {
            Ok(receipt) => Ok(Some(receipt)),
            Err(error) if query_error_is_not_found(&error) => Ok(None),
            Err(error) => Err(PrivacyActionClientErrorV1::transport(
                "failed to fetch finalized Exact12 execution receipt",
                eyre!(error),
            )),
        }
    }
}

fn query_error_is_not_found(error: &QueryError) -> bool {
    matches!(
        error,
        QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
    )
}

impl Client {
    /// Authenticate, fresh-gate, and submit one already-signed Exact12 action.
    ///
    /// The returned opaque handle is the only input accepted by
    /// [`Self::get_privacy_action_status_v1`]. HTTP acceptance leaves its public view in
    /// `Submitted`; it does not establish proof acceptance or a ledger effect.
    ///
    /// # Errors
    ///
    /// Fails closed for a noncanonical or invalid signature, another authority or network, any
    /// executable other than one direct privacy instruction, operation/profile drift, a stale or
    /// unavailable fresh capability tuple, canonical request authentication failure, or Torii
    /// submission rejection.
    pub fn submit_signed_privacy_action_v1(
        &self,
        request: PrivacyExact12ActionRequestV1,
    ) -> Result<AuthenticatedPrivacyActionHandleV1, PrivacyActionClientErrorV1> {
        let transport = ClientPrivacyActionTransportV1(self);
        submit_with_transport_v1(
            PrivacyActionClientContextV1::from_client(self),
            request,
            &transport,
        )
    }

    /// Refresh the authenticated state of one Exact12 action handle.
    ///
    /// `Applied` requires an exact successful committed transaction plus the matching finalized
    /// ID105 receipt. `Rejected` requires the exact committed typed rejection and forbids a
    /// receipt. A terminal handle never regresses or changes its authenticated result.
    ///
    /// # Errors
    ///
    /// Fails closed if the handle belongs to another client/network/endpoint, re-inspection
    /// changes, a pipeline field is noncanonical, committed details or the receipt disagree, a
    /// rejection reason is not canonical and bounded, or terminal state regresses.
    pub fn get_privacy_action_status_v1(
        &self,
        handle: &mut AuthenticatedPrivacyActionHandleV1,
    ) -> Result<PrivacyActionOperationViewV1, PrivacyActionClientErrorV1> {
        let transport = ClientPrivacyActionTransportV1(self);
        refresh_with_transport_v1(
            PrivacyActionClientContextV1::from_client(self),
            handle,
            &transport,
        )
    }
}

#[derive(Clone, Copy)]
struct PrivacyActionClientContextV1<'context> {
    network_id: &'context iroha_data_model::NetworkId,
    authority: &'context AccountId,
    torii_url: &'context Url,
}

impl<'context> PrivacyActionClientContextV1<'context> {
    fn from_client(client: &'context Client) -> Self {
        Self {
            network_id: &client.network_id,
            authority: &client.account,
            torii_url: &client.torii_url,
        }
    }
}

fn submit_with_transport_v1<T: PrivacyActionTransportV1>(
    context: PrivacyActionClientContextV1<'_>,
    request: PrivacyExact12ActionRequestV1,
    transport: &T,
) -> Result<AuthenticatedPrivacyActionHandleV1, PrivacyActionClientErrorV1> {
    let inspected = inspect_signed_action_v1(&request, context.network_id, context.authority)?;
    let manifest = transport.fetch_capability_manifest()?;
    require_fresh_capability_admission_v1(&request, &inspected.binding, &manifest)?;
    let submitted_hash = transport.submit_signed_transaction(&inspected.signed_transaction)?;
    if submitted_hash.as_ref() != &inspected.binding.transaction_hash {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Torii submission returned another Exact12 transaction hash",
        ));
    }
    let view = make_view_v1(
        inspected.binding,
        manifest.manifest_digest,
        manifest.committed_height,
        ResolvedActionStateV1::Submitted,
    )?;
    Ok(AuthenticatedPrivacyActionHandleV1 {
        request,
        inspection: inspected.binding,
        network_id: *context.network_id,
        authority: context.authority.clone(),
        torii_url: context.torii_url.clone(),
        view,
        typed_rejection_reason: None,
    })
}

fn refresh_with_transport_v1<T: PrivacyActionTransportV1>(
    context: PrivacyActionClientContextV1<'_>,
    handle: &mut AuthenticatedPrivacyActionHandleV1,
    transport: &T,
) -> Result<PrivacyActionOperationViewV1, PrivacyActionClientErrorV1> {
    if handle.network_id != *context.network_id
        || &handle.authority != context.authority
        || &handle.torii_url != context.torii_url
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 status handle belongs to another client context",
        ));
    }
    let inspected =
        inspect_signed_action_v1(&handle.request, context.network_id, context.authority)?;
    if inspected.binding != handle.inspection {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 status handle changed after authenticated submission",
        ));
    }
    let status = transport.fetch_global_pipeline_status(inspected.signed_transaction.hash())?;
    let Some(status) = status else {
        if handle.view.local_state() == PrivacyActionLocalStateV1::Terminal {
            return Err(PrivacyActionClientErrorV1::invalid(
                "terminal Exact12 action disappeared from global pipeline status",
            ));
        }
        return Ok(handle.view.clone());
    };
    validate_pipeline_status_v1(&status, inspected.binding.transaction_hash)?;
    let resolved = resolve_status_v1(
        &handle.view,
        inspected.binding,
        &inspected.signed_transaction,
        &status,
        transport,
    )?;
    if handle.view.local_state() == PrivacyActionLocalStateV1::Terminal {
        require_stable_terminal_v1(handle, &resolved)?;
        return Ok(handle.view.clone());
    }
    handle.view = resolved.view;
    handle.typed_rejection_reason = resolved.typed_rejection_reason;
    Ok(handle.view.clone())
}

fn inspect_signed_action_v1(
    request: &PrivacyExact12ActionRequestV1,
    expected_network_id: &iroha_data_model::NetworkId,
    expected_authority: &AccountId,
) -> Result<InspectedActionV1, PrivacyActionClientErrorV1> {
    request.validate().map_err(|_| {
        PrivacyActionClientErrorV1::invalid("invalid Exact12 signed-action request")
    })?;
    let signed = SignedTransaction::decode_all_versioned(request.signed_transaction_versioned())
        .map_err(|_| {
            PrivacyActionClientErrorV1::invalid(
                "Exact12 transaction is not current versioned Norito",
            )
        })?;
    if signed.encode_versioned() != request.signed_transaction_versioned() {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction is not its exact canonical wire",
        ));
    }
    signed.verify_signature().map_err(|_| {
        PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction has an invalid authority signature",
        )
    })?;
    if signed.authority() != expected_authority {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction authority differs from the client account",
        ));
    }
    if signed.network_id() != Some(expected_network_id) {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction belongs to another NetworkId",
        ));
    }
    let Executable::Instructions(instructions) = signed.instructions() else {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction is not one direct instruction list",
        ));
    };
    if instructions.len() != 1
        || instructions[0]
            .as_any()
            .downcast_ref::<SubmitPrivacyProofV1>()
            .is_none()
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction must contain exactly one direct privacy instruction",
        ));
    }
    let (transaction_intent_digest, submission) = signed
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| {
            PrivacyActionClientErrorV1::invalid(
                "Exact12 transaction has an invalid privacy intent binding",
            )
        })?
        .ok_or_else(|| {
            PrivacyActionClientErrorV1::invalid(
                "Exact12 transaction contains no direct privacy action",
            )
        })?;
    let operation = request.operation();
    let envelope = &submission.envelope;
    if envelope.protocol_id != operation.protocol_id()
        || envelope.statement.protocol_id() != operation.protocol_id()
        || envelope.proof.protocol_id() != operation.protocol_id()
        || envelope.statement.operation_schema() != operation
        || !proof_matches_operation_v1(operation, &envelope.proof)
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 transaction does not match the requested protocol operation",
        ));
    }
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| {
            PrivacyActionClientErrorV1::invalid(
                "Exact12 transaction carries an invalid proof envelope",
            )
        })?;
    let context = envelope.statement.context();
    if context.network_id != *expected_network_id
        || context.action_index != PRIVACY_ACTION_INDEX_V1
        || context.transaction_intent_digest != transaction_intent_digest
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 statement context differs from the signed transaction binding",
        ));
    }
    let envelope_bytes = norito::to_bytes(envelope).map_err(|error| {
        PrivacyActionClientErrorV1::transport(
            "failed to encode canonical Exact12 proof envelope",
            eyre!(error),
        )
    })?;
    let binding = InspectedActionBindingV1 {
        operation,
        transaction_hash: *signed.hash().as_ref(),
        transaction_intent_digest,
        statement_digest: envelope.statement_digest,
        proof_envelope_hash: *Hash::new(&envelope_bytes).as_ref(),
        profile: EnvelopeProfileBindingV1 {
            proof_system_id: envelope.proof_system_id,
            engine_id: envelope.engine_id,
            parameter_id: envelope.parameter_id,
            parameter_digest: envelope.parameter_digest,
            verifier_digest: envelope.verifier_digest,
            statement_schema_digest: envelope.statement_schema_digest,
            engine_manifest_digest: envelope.engine_manifest_digest,
        },
    };
    Ok(InspectedActionV1 {
        signed_transaction: signed,
        binding,
    })
}

fn proof_matches_operation_v1(
    operation: PrivacyExact12ActionOperationV1,
    proof: &PrivacyProofV1,
) -> bool {
    match operation {
        PrivacyExact12ActionOperationV1::ZkAmsBatchAdmissionActionV1 => matches!(
            proof,
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(_))
        ),
        PrivacyExact12ActionOperationV1::ZkAmsProvisionAccountActionV1 => matches!(
            proof,
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(_))
        ),
        _ => proof.protocol_id() == operation.protocol_id(),
    }
}

fn require_fresh_capability_admission_v1(
    request: &PrivacyExact12ActionRequestV1,
    binding: &InspectedActionBindingV1,
    manifest: &PrivacyExact12CapabilityManifestV1,
) -> Result<(), PrivacyActionClientErrorV1> {
    manifest.validate().map_err(|_| {
        PrivacyActionClientErrorV1::invalid(
            "fresh Exact12 capability manifest failed native validation",
        )
    })?;
    if request
        .expected_manifest_digest()
        .is_some_and(|expected| expected != manifest.manifest_digest)
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "fresh Exact12 capability manifest differs from the expected digest",
        ));
    }
    let protocol_id = binding.operation.protocol_id();
    let row = manifest
        .protocols
        .iter()
        .find(|row| row.protocol_id == protocol_id)
        .ok_or_else(|| {
            PrivacyActionClientErrorV1::invalid(
                "fresh Exact12 capability manifest omitted the requested protocol",
            )
        })?;
    if !row.is_network_available()
        || !row.operation_schemas.contains(binding.operation)
        || row.activation.is_none()
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "requested Exact12 capability tuple is not active and network-available",
        ));
    }
    let PrivacyCompiledProfileResultV1::Available(profile) = row.compiled_profile else {
        return Err(PrivacyActionClientErrorV1::invalid(
            "requested Exact12 capability tuple has no compiled native profile",
        ));
    };
    if profile.protocol_id != protocol_id
        || profile.proof_system_id != binding.profile.proof_system_id
        || profile.engine_id != binding.profile.engine_id
        || profile.parameter_id != binding.profile.parameter_id
        || profile.parameter_digest != binding.profile.parameter_digest
        || profile.verifier_digest != binding.profile.verifier_digest
        || profile.statement_schema_digest != binding.profile.statement_schema_digest
        || profile.engine_manifest_digest != binding.profile.engine_manifest_digest
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "signed Exact12 envelope differs from the fresh committed capability tuple",
        ));
    }
    Ok(())
}

fn validate_pipeline_status_v1(
    status: &PipelineTransactionStatusResponse,
    transaction_hash: [u8; 32],
) -> Result<(), PrivacyActionClientErrorV1> {
    if status.hash != hex::encode(transaction_hash) || status.scope != "global" {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 global pipeline status changed its requested hash or scope",
        ));
    }
    if !matches!(status.resolved_from.as_str(), "cache" | "queue" | "state") {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 global pipeline status used an unknown evidence source",
        ));
    }
    if status.status.block_height == Some(0) {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 global pipeline status used block height zero",
        ));
    }
    if !matches!(
        status.status.kind.as_str(),
        "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired"
    ) {
        return Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 global pipeline status used an unknown state",
        ));
    }
    Ok(())
}

struct ResolvedActionV1 {
    view: PrivacyActionOperationViewV1,
    typed_rejection_reason: Option<TransactionRejectionReason>,
}

enum ResolvedActionStateV1 {
    Submitted,
    Expired,
    Applied {
        committed_height: u64,
        receipt: PrivacyActionExecutionReceiptViewV1,
    },
    Rejected {
        committed_height: u64,
        reason: String,
    },
}

fn resolve_status_v1<T: PrivacyActionTransportV1>(
    previous: &PrivacyActionOperationViewV1,
    binding: InspectedActionBindingV1,
    transaction: &SignedTransaction,
    status: &PipelineTransactionStatusResponse,
    transport: &T,
) -> Result<ResolvedActionV1, PrivacyActionClientErrorV1> {
    match status.status.kind.as_str() {
        "Queued" | "Approved" | "Committed" => {
            if previous.local_state() == PrivacyActionLocalStateV1::Terminal {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "terminal Exact12 action regressed to a nonterminal pipeline state",
                ));
            }
            Ok(ResolvedActionV1 {
                view: previous.clone(),
                typed_rejection_reason: None,
            })
        }
        "Expired" => {
            if status.resolved_from == "cache" {
                if previous.local_state() == PrivacyActionLocalStateV1::Terminal {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "terminal Exact12 action regressed to local cache expiry",
                    ));
                }
                return Ok(ResolvedActionV1 {
                    view: previous.clone(),
                    typed_rejection_reason: None,
                });
            }
            if status.resolved_from != "state" || status.status.block_height.is_some() {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "expired Exact12 action lacks exact state-resolved evidence",
                ));
            }
            Ok(ResolvedActionV1 {
                view: make_view_v1(
                    binding,
                    previous.capability_manifest_digest(),
                    previous.capability_committed_height(),
                    ResolvedActionStateV1::Expired,
                )?,
                typed_rejection_reason: None,
            })
        }
        "Applied" | "Rejected" => {
            if !matches!(status.resolved_from.as_str(), "state" | "cache") {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "terminal Exact12 action was not resolved from committed state or cache",
                ));
            }
            let details = transport.fetch_committed_action(transaction)?;
            let receipt = transport.fetch_execution_receipt(&binding)?;
            if let (Some(public_height), Some(details)) =
                (status.status.block_height, details.as_ref())
                && public_height != details.block_height
            {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "Exact12 pipeline height differs from authenticated committed details",
                ));
            }
            if let (Some(public_height), Some(receipt)) =
                (status.status.block_height, receipt.as_ref())
                && public_height != receipt.admitted_at_height
            {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "Exact12 pipeline height differs from the finalized execution receipt",
                ));
            }
            if status.status.kind == "Rejected" {
                if receipt.is_some() {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "rejected Exact12 action has a finalized execution receipt",
                    ));
                }
                let Some(details) = details else {
                    return Ok(ResolvedActionV1 {
                        view: previous.clone(),
                        typed_rejection_reason: None,
                    });
                };
                let AuthenticatedCommittedActionResultV1::Rejected(reason) = details.result else {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "rejected Exact12 pipeline state resolved to a successful transaction",
                    ));
                };
                let Some(reason_text) = validated_rejection_message_v1(&reason) else {
                    return Err(PrivacyActionClientErrorV1::invalid(
                        "authenticated Exact12 rejection reason is not canonical and bounded",
                    ));
                };
                return Ok(ResolvedActionV1 {
                    view: make_view_v1(
                        binding,
                        previous.capability_manifest_digest(),
                        previous.capability_committed_height(),
                        ResolvedActionStateV1::Rejected {
                            committed_height: details.block_height,
                            reason: reason_text,
                        },
                    )?,
                    typed_rejection_reason: Some(reason),
                });
            }
            if details.as_ref().is_some_and(|details| {
                matches!(
                    &details.result,
                    AuthenticatedCommittedActionResultV1::Rejected(_)
                )
            }) {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "applied Exact12 pipeline state resolved to a rejected transaction",
                ));
            }
            let (Some(details), Some(receipt)) = (details, receipt) else {
                return Ok(ResolvedActionV1 {
                    view: previous.clone(),
                    typed_rejection_reason: None,
                });
            };
            validate_execution_receipt_v1(&receipt, &binding, transaction)?;
            if details.block_height != receipt.admitted_at_height {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "Exact12 receipt admission height differs from committed details",
                ));
            }
            Ok(ResolvedActionV1 {
                view: make_view_v1(
                    binding,
                    previous.capability_manifest_digest(),
                    previous.capability_committed_height(),
                    ResolvedActionStateV1::Applied {
                        committed_height: details.block_height,
                        receipt,
                    },
                )?,
                typed_rejection_reason: None,
            })
        }
        _ => Err(PrivacyActionClientErrorV1::invalid(
            "Exact12 global pipeline status used an unknown state",
        )),
    }
}

fn validate_execution_receipt_v1(
    receipt: &PrivacyActionExecutionReceiptViewV1,
    binding: &InspectedActionBindingV1,
    transaction: &SignedTransaction,
) -> Result<(), PrivacyActionClientErrorV1> {
    receipt.validate().map_err(|_| {
        PrivacyActionClientErrorV1::invalid(
            "finalized Exact12 execution receipt failed native validation",
        )
    })?;
    let transaction_network_id = transaction.network_id().ok_or_else(|| {
        PrivacyActionClientErrorV1::invalid(
            "authenticated Exact12 transaction lost its NetworkId during receipt resolution",
        )
    })?;
    if receipt.network_id != *transaction_network_id
        || receipt.protocol_id != binding.operation.protocol_id()
        || receipt.operation_schema != binding.operation
        || receipt.ledger_effect_kind != binding.operation.ledger_effect_kind()
        || receipt.transaction_hash != binding.transaction_hash
        || receipt.action_index != PRIVACY_ACTION_INDEX_V1
        || receipt.transaction_intent_digest != binding.transaction_intent_digest
        || receipt.statement_digest != binding.statement_digest
        || receipt.proof_envelope_hash != binding.proof_envelope_hash
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "finalized Exact12 execution receipt changed an authenticated action binding",
        ));
    }
    Ok(())
}

fn make_view_v1(
    binding: InspectedActionBindingV1,
    capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1,
    capability_committed_height: u64,
    state: ResolvedActionStateV1,
) -> Result<PrivacyActionOperationViewV1, PrivacyActionClientErrorV1> {
    let (
        local_state,
        terminal_chain_state,
        committed_height,
        rejection_reason,
        execution_capability_manifest_digest,
        execution_capability_committed_height,
        execution_receipt_finalized_height,
        execution_receipt_finalized_block_hash,
    ) = match state {
        ResolvedActionStateV1::Submitted => (
            PrivacyActionLocalStateV1::Submitted,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        ResolvedActionStateV1::Expired => (
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Expired),
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        ResolvedActionStateV1::Rejected {
            committed_height,
            reason,
        } => (
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Rejected),
            Some(committed_height),
            Some(reason),
            None,
            None,
            None,
            None,
        ),
        ResolvedActionStateV1::Applied {
            committed_height,
            receipt,
        } => (
            PrivacyActionLocalStateV1::Terminal,
            Some(PrivacyActionTerminalChainStateV1::Applied),
            Some(committed_height),
            None,
            Some(receipt.capability_manifest_digest),
            Some(receipt.capability_committed_height),
            Some(receipt.finalized_height),
            Some(receipt.finalized_block_hash),
        ),
    };
    PrivacyActionOperationViewV1::try_new(
        binding.operation.protocol_id(),
        binding.operation,
        binding.transaction_hash,
        binding.transaction_intent_digest,
        binding.statement_digest,
        binding.proof_envelope_hash,
        local_state,
        terminal_chain_state,
        committed_height,
        rejection_reason,
        binding.operation.ledger_effect_kind(),
        capability_manifest_digest,
        capability_committed_height,
        execution_capability_manifest_digest,
        execution_capability_committed_height,
        execution_receipt_finalized_height,
        execution_receipt_finalized_block_hash,
    )
    .map_err(|_| {
        PrivacyActionClientErrorV1::invalid(
            "authenticated Exact12 state does not form a canonical operation view",
        )
    })
}

fn canonical_rejection_reason_v1(reason: &str) -> bool {
    !reason.is_empty()
        && reason.len() <= iroha_data_model::privacy::PRIVACY_ACTION_REJECTION_REASON_MAX_BYTES_V1
        && reason.trim() == reason
        && !reason.chars().any(char::is_control)
}

fn validated_rejection_message_v1(reason: &TransactionRejectionReason) -> Option<String> {
    let message = match reason {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::OfflineDeviceEligibility(rejection),
        )) => rejection.detail.clone(),
        _ => {
            let mut message = reason.to_string();
            let mut source = reason.source();
            while let Some(current) = source {
                message = current.to_string();
                source = current.source();
            }
            message
        }
    };
    canonical_rejection_reason_v1(&message).then_some(message)
}

fn require_stable_terminal_v1(
    handle: &AuthenticatedPrivacyActionHandleV1,
    refreshed: &ResolvedActionV1,
) -> Result<(), PrivacyActionClientErrorV1> {
    let previous = &handle.view;
    let next = &refreshed.view;
    if previous.terminal_chain_state() != next.terminal_chain_state()
        || previous.committed_height() != next.committed_height()
        || previous.rejection_reason() != next.rejection_reason()
        || previous.execution_capability_manifest_digest()
            != next.execution_capability_manifest_digest()
        || previous.execution_capability_committed_height()
            != next.execution_capability_committed_height()
        || handle.typed_rejection_reason.as_ref() != refreshed.typed_rejection_reason.as_ref()
    {
        return Err(PrivacyActionClientErrorV1::invalid(
            "terminal Exact12 action changed its authenticated result",
        ));
    }
    match (
        previous.execution_receipt_finalized_height(),
        next.execution_receipt_finalized_height(),
    ) {
        (None, None) => {}
        (Some(old), Some(new)) if new >= old => {
            if new == old
                && previous.execution_receipt_finalized_block_hash()
                    != next.execution_receipt_finalized_block_hash()
            {
                return Err(PrivacyActionClientErrorV1::invalid(
                    "terminal Exact12 receipt changed its finalized block at the same height",
                ));
            }
        }
        _ => {
            return Err(PrivacyActionClientErrorV1::invalid(
                "terminal Exact12 receipt finality regressed",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        privacy::{
            PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1,
            PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1, PrivacyActionExecutionReceiptViewV1,
            PrivacyActionLocalStateV1, PrivacyActionTerminalChainStateV1, PrivacyActiveLifecycleV1,
            PrivacyAssuranceV1, PrivacyCapabilityRowV1, PrivacyCapabilitySnapshotV1,
            PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
            PrivacyCompiledProfileUnavailableReasonV1, PrivacyConsensusPolicyV1,
            PrivacyExact12ActionRequestV1, PrivacyExact12CapabilityManifestV1,
            PrivacyProtocolActivationLimitsV1, PrivacyProtocolActivationRecordV1,
            PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, privacy_exact12_fixture_bundle_v1,
        },
        transaction::{SignedTransaction, error::TransactionLimitError},
    };
    use iroha_torii_shared::{PipelineTransactionStatus, PipelineTransactionStatusResponse};
    use iroha_version::codec::DecodeVersioned as _;
    use url::Url;

    use super::*;

    struct Exact12Fixture {
        network_id: NetworkId,
        authority: AccountId,
        torii_url: Url,
        request: PrivacyExact12ActionRequestV1,
        signed_transaction: SignedTransaction,
        manifest: PrivacyExact12CapabilityManifestV1,
    }

    fn active_manifest_v1(transaction: &SignedTransaction) -> PrivacyExact12CapabilityManifestV1 {
        let (_, submission) = transaction
            .privacy_transaction_intent_binding_if_present_v1()
            .expect("valid fixture privacy binding")
            .expect("fixture carries privacy action");
        let envelope = &submission.envelope;
        assert_eq!(
            envelope.protocol_id,
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            "the first deterministic row remains ZK-ACE",
        );
        let protocol_limits = PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0;
        let activation = PrivacyProtocolActivationRecordV1 {
            protocol_id: envelope.protocol_id,
            proof_system_id: envelope.proof_system_id,
            engine_id: envelope.engine_id,
            parameter_id: envelope.parameter_id,
            parameter_digest: envelope.parameter_digest,
            verifier_digest: envelope.verifier_digest,
            statement_schema_digest: envelope.statement_schema_digest,
            engine_manifest_digest: envelope.engine_manifest_digest,
            lifecycle: PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            }),
            protocol_limits,
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        };
        let profile = PrivacyCompiledProfileSnapshotV1 {
            protocol_id: activation.protocol_id,
            proof_system_id: activation.proof_system_id,
            engine_id: activation.engine_id,
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
            protocol_limits,
        };
        PrivacyCapabilitySnapshotV1 {
            version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
            committed_height: 42,
            consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
            protocols: PrivacyProtocolIdV1::ALL
                .into_iter()
                .map(|protocol_id| {
                    if protocol_id == activation.protocol_id {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Available(profile),
                            activation: Some(activation),
                        }
                    } else {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Unavailable(
                                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                            ),
                            activation: None,
                        }
                    }
                })
                .collect(),
        }
        .exact12_capability_manifest_v1()
        .expect("valid active fixture manifest")
    }

    fn exact12_fixture() -> Exact12Fixture {
        let bundle = privacy_exact12_fixture_bundle_v1().expect("deterministic Exact12 bundle");
        let row = bundle.rows.first().expect("ZK-ACE fixture row");
        let signed_transaction =
            SignedTransaction::decode_all_versioned(&row.signed_transaction_versioned_norito)
                .expect("decode exact fixture transaction");
        let network_id = *signed_transaction
            .network_id()
            .expect("ordinary fixture NetworkId");
        let authority = signed_transaction.authority().clone();
        let manifest = active_manifest_v1(&signed_transaction);
        let request = PrivacyExact12ActionRequestV1::try_new(
            PrivacyExact12ActionOperationV1::ZkAceAuthorizationActionV1,
            row.signed_transaction_versioned_norito.clone(),
            Some(manifest.manifest_digest),
        )
        .expect("valid Exact12 request");
        Exact12Fixture {
            network_id,
            authority,
            torii_url: Url::parse("https://taira.invalid/").expect("test URL"),
            request,
            signed_transaction,
            manifest,
        }
    }

    #[derive(Default)]
    struct MockState {
        submit_count: usize,
        status: Option<PipelineTransactionStatusResponse>,
        committed: Option<AuthenticatedCommittedActionV1>,
        receipt: Option<PrivacyActionExecutionReceiptViewV1>,
    }

    struct MockTransport {
        manifest: PrivacyExact12CapabilityManifestV1,
        expected_transaction: SignedTransaction,
        state: Mutex<MockState>,
    }

    impl MockTransport {
        fn new(fixture: &Exact12Fixture) -> Self {
            Self {
                manifest: fixture.manifest.clone(),
                expected_transaction: fixture.signed_transaction.clone(),
                state: Mutex::new(MockState::default()),
            }
        }

        fn set_terminal(
            &self,
            status: PipelineTransactionStatusResponse,
            committed: Option<AuthenticatedCommittedActionV1>,
            receipt: Option<PrivacyActionExecutionReceiptViewV1>,
        ) {
            let mut state = self.state.lock().expect("mock state lock");
            state.status = Some(status);
            state.committed = committed;
            state.receipt = receipt;
        }
    }

    impl PrivacyActionTransportV1 for MockTransport {
        fn fetch_capability_manifest(
            &self,
        ) -> Result<PrivacyExact12CapabilityManifestV1, PrivacyActionClientErrorV1> {
            Ok(self.manifest.clone())
        }

        fn submit_signed_transaction(
            &self,
            transaction: &SignedTransaction,
        ) -> Result<HashOf<SignedTransaction>, PrivacyActionClientErrorV1> {
            assert_eq!(transaction, &self.expected_transaction);
            self.state.lock().expect("mock state lock").submit_count += 1;
            Ok(transaction.hash())
        }

        fn fetch_global_pipeline_status(
            &self,
            _transaction_hash: HashOf<SignedTransaction>,
        ) -> Result<Option<PipelineTransactionStatusResponse>, PrivacyActionClientErrorV1> {
            Ok(self.state.lock().expect("mock state lock").status.clone())
        }

        fn fetch_committed_action(
            &self,
            transaction: &SignedTransaction,
        ) -> Result<Option<AuthenticatedCommittedActionV1>, PrivacyActionClientErrorV1> {
            assert_eq!(transaction, &self.expected_transaction);
            Ok(self
                .state
                .lock()
                .expect("mock state lock")
                .committed
                .clone())
        }

        fn fetch_execution_receipt(
            &self,
            _binding: &InspectedActionBindingV1,
        ) -> Result<Option<PrivacyActionExecutionReceiptViewV1>, PrivacyActionClientErrorV1>
        {
            Ok(self.state.lock().expect("mock state lock").receipt)
        }
    }

    fn context(fixture: &Exact12Fixture) -> PrivacyActionClientContextV1<'_> {
        PrivacyActionClientContextV1 {
            network_id: &fixture.network_id,
            authority: &fixture.authority,
            torii_url: &fixture.torii_url,
        }
    }

    fn pipeline_status(
        hash: [u8; 32],
        kind: &str,
        block_height: Option<u64>,
        resolved_from: &str,
    ) -> PipelineTransactionStatusResponse {
        PipelineTransactionStatusResponse::new(
            hex::encode(hash),
            PipelineTransactionStatus {
                kind: kind.to_owned(),
                block_height,
            },
            "global".to_owned(),
            resolved_from.to_owned(),
        )
    }

    fn execution_receipt(
        fixture: &Exact12Fixture,
        binding: InspectedActionBindingV1,
        finalized_height: u64,
        finalized_block_seed: u8,
    ) -> PrivacyActionExecutionReceiptViewV1 {
        PrivacyActionExecutionReceiptViewV1 {
            version: PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1,
            network_id: fixture.network_id,
            protocol_id: binding.operation.protocol_id(),
            operation_schema: binding.operation,
            ledger_effect_kind: binding.operation.ledger_effect_kind(),
            transaction_hash: binding.transaction_hash,
            action_index: PRIVACY_ACTION_INDEX_V1,
            transaction_intent_digest: binding.transaction_intent_digest,
            statement_digest: binding.statement_digest,
            proof_envelope_hash: binding.proof_envelope_hash,
            capability_manifest_digest: fixture.manifest.manifest_digest,
            capability_committed_height: fixture.manifest.committed_height,
            admitted_at_height: 43,
            finalized_height,
            finalized_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [finalized_block_seed; 32],
            )),
        }
    }

    #[test]
    fn mocked_flow_requires_committed_success_and_exact_finalized_receipt() {
        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        assert_eq!(
            handle.view().local_state(),
            PrivacyActionLocalStateV1::Submitted
        );
        assert_eq!(
            transport
                .state
                .lock()
                .expect("mock state lock")
                .submit_count,
            1
        );

        let receipt = execution_receipt(&fixture, handle.inspection, 44, 0x72);
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Applied",
                Some(43),
                "state",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Applied,
            }),
            Some(receipt),
        );
        let applied = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("resolve applied action");
        assert_eq!(applied.local_state(), PrivacyActionLocalStateV1::Terminal);
        assert_eq!(
            applied.terminal_chain_state(),
            Some(PrivacyActionTerminalChainStateV1::Applied)
        );
        assert_eq!(applied.committed_height(), Some(43));
        assert_eq!(applied.execution_receipt_finalized_height(), Some(44));

        let later_receipt = execution_receipt(&fixture, handle.inspection, 45, 0x73);
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Applied",
                Some(43),
                "cache",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Applied,
            }),
            Some(later_receipt),
        );
        let stable = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("preserve first terminal view across later finality");
        assert_eq!(stable, applied);
    }

    #[test]
    fn mocked_rejection_preserves_typed_reason_and_rejects_a_receipt() {
        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        let reason = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "testnet fixture rejection".to_owned(),
        });
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Rejected",
                Some(43),
                "state",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Rejected(reason.clone()),
            }),
            None,
        );
        let rejected = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("resolve authenticated rejection");
        assert_eq!(
            rejected.terminal_chain_state(),
            Some(PrivacyActionTerminalChainStateV1::Rejected)
        );
        assert_eq!(
            rejected.rejection_reason(),
            Some("testnet fixture rejection")
        );
        assert_eq!(handle.typed_rejection_reason(), Some(&reason));

        let receipt = execution_receipt(&fixture, handle.inspection, 44, 0x72);
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Rejected",
                Some(43),
                "state",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Rejected(reason),
            }),
            Some(receipt),
        );
        let error = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect_err("a rejected action must never carry an execution receipt");
        assert!(
            error
                .to_string()
                .contains("has a finalized execution receipt")
        );
        assert_eq!(handle.view(), &rejected);
    }

    #[test]
    fn fresh_manifest_must_match_the_signed_envelope_tuple() {
        let fixture = exact12_fixture();
        let mut substituted = fixture.manifest.clone();
        let row = substituted
            .protocols
            .iter_mut()
            .find(|row| row.protocol_id == PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("ZK-ACE capability row");
        let PrivacyCompiledProfileResultV1::Available(mut profile) = row.compiled_profile else {
            panic!("active fixture profile")
        };
        let mut changed = *profile.parameter_digest.as_bytes();
        changed[0] ^= 0x80;
        profile.parameter_digest = PrivacyParameterDigestV1::new(changed);
        row.compiled_profile = PrivacyCompiledProfileResultV1::Available(profile);
        let mut activation = row.activation.expect("active fixture activation");
        activation.parameter_digest = profile.parameter_digest;
        row.activation = Some(activation);
        substituted.manifest_digest = substituted
            .computed_manifest_digest()
            .expect("redigest substituted manifest");
        substituted
            .validate()
            .expect("structurally valid substitution");

        let request = PrivacyExact12ActionRequestV1::try_new(
            fixture.request.operation(),
            fixture.request.signed_transaction_versioned().to_vec(),
            None,
        )
        .expect("request without observation pin");
        let transport = MockTransport {
            manifest: substituted,
            expected_transaction: fixture.signed_transaction.clone(),
            state: Mutex::new(MockState::default()),
        };
        let error = submit_with_transport_v1(context(&fixture), request, &transport)
            .expect_err("substituted committed profile must fail before dispatch");
        assert!(
            error
                .to_string()
                .contains("differs from the fresh committed capability tuple")
        );
        assert_eq!(
            transport
                .state
                .lock()
                .expect("mock state lock")
                .submit_count,
            0
        );
    }

    #[test]
    fn malformed_rejection_text_and_terminal_regression_fail_closed() {
        assert!(!canonical_rejection_reason_v1(" rejected"));
        assert!(!canonical_rejection_reason_v1("rejected\nreason"));
        assert!(!canonical_rejection_reason_v1(&"x".repeat(
            iroha_data_model::privacy::PRIVACY_ACTION_REJECTION_REASON_MAX_BYTES_V1 + 1
        )));

        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        transport.set_terminal(
            pipeline_status(handle.inspection.transaction_hash, "Expired", None, "state"),
            None,
            None,
        );
        refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("resolve state expiry");
        transport.set_terminal(
            pipeline_status(handle.inspection.transaction_hash, "Queued", None, "queue"),
            None,
            None,
        );
        let error = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect_err("terminal result must not regress");
        assert!(error.to_string().contains("regressed"));
        assert_eq!(
            handle.view().terminal_chain_state(),
            Some(PrivacyActionTerminalChainStateV1::Expired)
        );
    }

    #[test]
    fn mocked_rejection_rejects_noncanonical_deepest_typed_source_text() {
        for invalid_reason in [
            "rejected\nreason".to_owned(),
            "x".repeat(iroha_data_model::privacy::PRIVACY_ACTION_REJECTION_REASON_MAX_BYTES_V1 + 1),
        ] {
            let fixture = exact12_fixture();
            let transport = MockTransport::new(&fixture);
            let mut handle =
                submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                    .expect("authenticate and dispatch fixture");
            let reason = TransactionRejectionReason::LimitCheck(TransactionLimitError {
                reason: invalid_reason,
            });
            transport.set_terminal(
                pipeline_status(
                    handle.inspection.transaction_hash,
                    "Rejected",
                    Some(43),
                    "state",
                ),
                Some(AuthenticatedCommittedActionV1 {
                    block_height: 43,
                    result: AuthenticatedCommittedActionResultV1::Rejected(reason),
                }),
                None,
            );
            let error = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
                .expect_err("noncanonical deepest typed rejection source must fail closed");
            assert!(error.to_string().contains("not canonical and bounded"));
            assert_eq!(
                handle.view().local_state(),
                PrivacyActionLocalStateV1::Submitted
            );
            assert!(handle.typed_rejection_reason().is_none());
        }
    }

    #[test]
    fn mocked_applied_status_waits_for_both_evidence_sources_and_rejects_contradictions() {
        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Applied",
                Some(43),
                "state",
            ),
            None,
            None,
        );
        let pending = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("missing terminal evidence remains retryable");
        assert_eq!(pending.local_state(), PrivacyActionLocalStateV1::Submitted);

        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Applied",
                Some(43),
                "state",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Applied,
            }),
            None,
        );
        let still_pending = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect("successful details without receipt remain retryable");
        assert_eq!(
            still_pending.local_state(),
            PrivacyActionLocalStateV1::Submitted
        );

        let rejection = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "contradictory rejection".to_owned(),
        });
        transport.set_terminal(
            pipeline_status(
                handle.inspection.transaction_hash,
                "Applied",
                Some(43),
                "state",
            ),
            Some(AuthenticatedCommittedActionV1 {
                block_height: 43,
                result: AuthenticatedCommittedActionResultV1::Rejected(rejection),
            }),
            None,
        );
        let error = refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
            .expect_err("Applied must reject committed rejection details");
        assert!(error.to_string().contains("rejected transaction"));
        assert_eq!(
            handle.view().local_state(),
            PrivacyActionLocalStateV1::Submitted
        );
    }

    #[test]
    fn mocked_noncanonical_pipeline_fields_fail_before_state_resolution() {
        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        let valid = pipeline_status(handle.inspection.transaction_hash, "Queued", None, "queue");
        let mut cases = Vec::new();
        let mut wrong_hash = valid.clone();
        wrong_hash.hash = "11".repeat(32);
        cases.push(wrong_hash);
        let mut wrong_scope = valid.clone();
        wrong_scope.scope = "local".to_owned();
        cases.push(wrong_scope);
        let mut wrong_source = valid.clone();
        wrong_source.resolved_from = "replica".to_owned();
        cases.push(wrong_source);
        let mut zero_height = valid.clone();
        zero_height.status.block_height = Some(0);
        cases.push(zero_height);
        let mut unknown_kind = valid;
        unknown_kind.status.kind = "Accepted".to_owned();
        cases.push(unknown_kind);

        for status in cases {
            transport.set_terminal(status, None, None);
            refresh_with_transport_v1(context(&fixture), &mut handle, &transport)
                .expect_err("noncanonical pipeline field must fail closed");
            assert_eq!(
                handle.view().local_state(),
                PrivacyActionLocalStateV1::Submitted
            );
        }
    }

    #[test]
    fn status_handle_is_bound_to_the_exact_client_context() {
        let fixture = exact12_fixture();
        let transport = MockTransport::new(&fixture);
        let mut handle =
            submit_with_transport_v1(context(&fixture), fixture.request.clone(), &transport)
                .expect("authenticate and dispatch fixture");
        let another_url = Url::parse("https://another-validator.invalid/").expect("test URL");
        let another_context = PrivacyActionClientContextV1 {
            network_id: &fixture.network_id,
            authority: &fixture.authority,
            torii_url: &another_url,
        };
        let error = refresh_with_transport_v1(another_context, &mut handle, &transport)
            .expect_err("another Torii endpoint cannot adopt the opaque handle");
        assert!(error.to_string().contains("another client context"));
    }

    #[test]
    fn pipeline_status_json_rejects_duplicate_declared_fields() {
        let hash = "11".repeat(32);
        let duplicate = format!(
            r#"{{"hash":"{hash}","hash":"{hash}","status":{{"kind":"Queued"}},"scope":"global","resolved_from":"queue"}}"#
        );
        let error = norito::json::from_str::<PipelineTransactionStatusResponse>(&duplicate)
            .expect_err("duplicate status fields must not be last-write-wins");
        assert!(
            error.to_string().contains("duplicate field"),
            "unexpected duplicate-field error: {error}",
        );
    }
}
