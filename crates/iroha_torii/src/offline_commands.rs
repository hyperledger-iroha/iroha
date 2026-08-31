use crate::{AppState, Error, SharedAppState, app_auth, routing};
use axum::{http::HeaderMap, response::Response as AxResponse};
use iroha_config::parameters::actual;
use iroha_core::kagemusha_operation::{
    KagemushaOperationExecutionPhaseV4, KagemushaOperationOutcomeRecordV4,
    KagemushaOperationOutcomeStateV4, kagemusha_operation_authority_digest_v4,
    kagemusha_operation_finality_v4,
    kagemusha_operation_outcome_state_key_from_authority_digest_v4, kagemusha_operation_outcome_v4,
    signed_transaction_wire_hash_v4,
};
use iroha_core::queue::{PendingKagemushaOperation, PendingKagemushaOperationLookupError};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetId,
    isi::{
        InstructionBox,
        offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
    },
    offline::{
        KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2, KagemushaOperationCarrierV4,
        KagemushaOperationRequestV4, KagemushaRecursiveSpendTopUpAnchorV4,
        classify_kagemusha_operation_entrypoint_v4, classify_kagemusha_operation_transaction_v4,
    },
    state_path::StatePath,
    transaction::{
        SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        error::TransactionRejectionReason, signed::TransactionResult,
    },
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationResult, OfflineOperationState,
    OfflineOperationStatus, OfflineRedeemRequest, OfflineRedeemResult, OfflineTopUpFinalityProof,
    OfflineTopUpRequest, OfflineTopUpResult,
};
use mv::storage::StorageReadOnly;
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;
const PATH_OFFLINE_TOP_UP: &str = iroha_torii_shared::uri::OFFLINE_TOP_UP;
const PATH_OFFLINE_REDEEM: &str = iroha_torii_shared::uri::OFFLINE_REDEEM;
const OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS: u64 = 24 * 60 * 60 * 1_000;
// Canonical logical accounting is intentionally independent of allocator and
// architecture details: operation id + kind + request digest + transaction
// hash + submission/expiry timestamps. The count budget separately bounds the
// map-node/key duplication and in-flight coordination objects.
const ADMITTED_OPERATION_ACCOUNTED_BYTES: usize =
    iroha_config::parameters::defaults::torii::kagemusha_commands::OPERATION_REGISTRY_ACCOUNTED_BYTES_PER_ENTRY;
#[derive(Debug, Clone)]
pub(crate) struct OfflineCommandRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    minimum_xor_balance: Quantity,
    max_tx_value: Quantity,
    admission: Arc<Mutex<OfflineOperationRegistry>>,
}
impl OfflineCommandRuntime {
    pub(crate) fn from_config(config: actual::ToriiKagemushaCommands) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            minimum_xor_balance: config.minimum_xor_balance,
            max_tx_value: config.max_tx_value,
            admission: Arc::new(Mutex::new(OfflineOperationRegistry::new(
                config.operation_registry_max_entries,
                config.operation_registry_max_bytes,
            ))),
        }
    }
    pub(super) fn startup_config(&self) -> actual::ToriiKagemushaCommands {
        let admission = self.admission.lock();
        actual::ToriiKagemushaCommands {
            authority: self.authority.clone(),
            key_pair: self.key_pair.clone(),
            minimum_xor_balance: self.minimum_xor_balance.clone(),
            max_tx_value: self.max_tx_value.clone(),
            operation_registry_max_entries: admission.max_entries,
            operation_registry_max_bytes: admission.max_accounted_bytes,
        }
    }
    fn quote_and_sign_transaction(
        &self,
        app: &AppState,
        transaction: TransactionBuilder,
        context: &'static str,
    ) -> Result<SignedTransaction, Error> {
        let mut payload = transaction
            .into_payload()
            .map_err(|source| offline_transaction_signing_error(context, source))?;
        payload.fee_payment = crate::quote_internal_fee_payment(app, &payload)?;
        TransactionBuilder::from_payload(payload)
            .map_err(|source| offline_transaction_signing_error(context, source))?
            .try_sign(self.key_pair.private_key())
            .map_err(|source| offline_transaction_signing_error(context, source))
    }
}
#[derive(Debug)]
struct InFlightSubmission {
    binding: OfflineOperationRequestBinding,
    token: Arc<()>,
    updates: watch::Sender<SubmissionOutcome>,
}
#[derive(Debug, Clone)]
enum SubmissionOutcome {
    Pending,
    Accepted(AdmittedOfflineOperationRecord),
    Retry,
}
enum SubmissionClaim {
    Accepted(AdmittedOfflineOperationRecord),
    Leader(SubmissionLeader),
    Follower(watch::Receiver<SubmissionOutcome>),
}
struct SubmissionLeader {
    issuer: Arc<OfflineCommandRuntime>,
    operation_id: [u8; 32],
    token: Arc<()>,
    binding: OfflineOperationRequestBinding,
    updates: watch::Sender<SubmissionOutcome>,
    active: bool,
}
pub(crate) async fn handle_top_up(
    app: SharedAppState,
    headers: &HeaderMap,
    topup_request: OfflineTopUpRequest,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, topup_request.authorization.operation_id)?;
    topup_request.validate_public_binding().map_err(|source| {
        validation_owned(
            "offline_top_up_invalid",
            format!("Offline top-up request is invalid: {source}"),
        )
    })?;
    let requested = OfflineOperationRequest::TopUp(&topup_request);
    let requested_binding = OfflineOperationRequestBinding::from_request(requested)?;
    let issuer = require_configured_issuer(&app)?;
    let submission = loop {
        match find_existing_offline_operation(&app, &issuer, requested, &requested_binding)? {
            OfflineSubmissionRecovery::Existing(response) => return Ok(response),
            OfflineSubmissionRecovery::RetryRejected { .. } | OfflineSubmissionRecovery::Absent => {
            }
        }
        let claim = match issuer.claim_submission(requested_binding) {
            Ok(claim) => claim,
            Err(error) => {
                return reconcile_offline_submission_failure(
                    &app,
                    &issuer,
                    requested,
                    &requested_binding,
                    error,
                );
            }
        };
        match claim {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_admitted_record(&record);
            }
            SubmissionClaim::Leader(submission) => {
                if let Err(error) = ensure_offline_command_authority_ready(&app, &issuer) {
                    drop(submission);
                    return reconcile_offline_submission_failure(
                        &app,
                        &issuer,
                        requested,
                        &requested_binding,
                        error,
                    );
                }
                break submission;
            }
            SubmissionClaim::Follower(receiver) => {
                match wait_for_submission_outcome(receiver).await {
                    SubmissionOutcome::Accepted(record) => {
                        return offline_operation_reference_for_admitted_record(&record);
                    }
                    SubmissionOutcome::Retry | SubmissionOutcome::Pending => continue,
                }
            }
        }
    };
    // Retention pruning can remove an expired admitted binding after this
    // caller's first recovery pass but before it acquires the in-flight claim.
    // Once the claim is ours, repeat the consensus-outcome and typed Queue lookup before
    // signing; any earlier leader is either still represented in-flight or
    // recoverable from one of those durable transaction sources.
    let retry_nonce =
        match find_existing_offline_operation(&app, &issuer, requested, &requested_binding)? {
            OfflineSubmissionRecovery::Existing(response) => {
                drop(submission);
                return Ok(response);
            }
            OfflineSubmissionRecovery::RetryRejected { next_nonce } => Some(next_nonce),
            OfflineSubmissionRecovery::Absent => None,
        };
    if let Err(error) = preflight_kagemusha_v2_hardware_authorization(
        &app,
        &topup_request.authorization,
        topup_request.asset.definition(),
    ) {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    if let Err(error) = validate_kagemusha_v4_topup_snapshot(&app, &topup_request) {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    let public_amount = match topup_request.amount.public_quantity() {
        Ok(amount) => amount,
        Err(source) => {
            return reconcile_offline_submission_failure(
                &app,
                &issuer,
                requested,
                &requested_binding,
                validation_owned(
                    "offline_top_up_invalid",
                    format!("Offline top-up amount cannot be represented exactly: {source}"),
                ),
            );
        }
    };
    if public_amount > issuer.max_tx_value.clone() {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            validation(
                "offline_amount_exceeds_limit",
                "Offline top-up amount exceeds issuer policy.",
            ),
        );
    }
    let instruction = TopUpKagemushaRecursiveV4::new(topup_request.clone());
    let mut transaction = TransactionBuilder::new(
        *app.state.network_id_ref(),
        issuer.authority.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(instruction)]);
    transaction.set_creation_time(Duration::from_millis(
        topup_request.authorization.issued_at_ms,
    ));
    transaction.set_ttl(Duration::from_millis(
        topup_request
            .authorization
            .expires_at_ms
            .saturating_sub(topup_request.authorization.issued_at_ms),
    ));
    if let Some(nonce) = retry_nonce {
        transaction.set_nonce(nonce);
    }
    let tx =
        match issuer.quote_and_sign_transaction(&app, transaction, "offline_top_up_transaction") {
            Ok(transaction) => transaction,
            Err(error) => {
                return reconcile_offline_submission_failure(
                    &app,
                    &issuer,
                    requested,
                    &requested_binding,
                    error,
                );
            }
        };
    let tx_hash = tx.hash();
    let admission = routing::handle_transaction_with_metrics(
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_TOP_UP,
    )
    .await;
    if let Err(error) = admission {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    match submission.accept(tx_hash) {
        Ok(record) => offline_operation_reference_for_admitted_record(&record),
        Err(error) => reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        ),
    }
}
pub(crate) async fn handle_redeem(
    app: SharedAppState,
    headers: &HeaderMap,
    redeem_request: OfflineRedeemRequest,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, redeem_request.authorization.operation_id)?;
    redeem_request.validate_public_binding().map_err(|source| {
        validation_owned(
            "offline_redeem_invalid",
            format!("Offline redemption request is invalid: {source}"),
        )
    })?;
    let requested = OfflineOperationRequest::Redeem(&redeem_request);
    let requested_binding = OfflineOperationRequestBinding::from_request(requested)?;
    let issuer = require_configured_issuer(&app)?;
    let submission = loop {
        match find_existing_offline_operation(&app, &issuer, requested, &requested_binding)? {
            OfflineSubmissionRecovery::Existing(response) => return Ok(response),
            OfflineSubmissionRecovery::RetryRejected { .. } | OfflineSubmissionRecovery::Absent => {
            }
        }
        let claim = match issuer.claim_submission(requested_binding) {
            Ok(claim) => claim,
            Err(error) => {
                return reconcile_offline_submission_failure(
                    &app,
                    &issuer,
                    requested,
                    &requested_binding,
                    error,
                );
            }
        };
        match claim {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_admitted_record(&record);
            }
            SubmissionClaim::Leader(submission) => {
                if let Err(error) = ensure_offline_command_authority_ready(&app, &issuer) {
                    drop(submission);
                    return reconcile_offline_submission_failure(
                        &app,
                        &issuer,
                        requested,
                        &requested_binding,
                        error,
                    );
                }
                break submission;
            }
            SubmissionClaim::Follower(receiver) => {
                match wait_for_submission_outcome(receiver).await {
                    SubmissionOutcome::Accepted(record) => {
                        return offline_operation_reference_for_admitted_record(&record);
                    }
                    SubmissionOutcome::Retry | SubmissionOutcome::Pending => continue,
                }
            }
        }
    };
    // See the top-up path: this second authoritative read closes the retention
    // pruning window before a replacement transaction can be signed.
    let retry_nonce =
        match find_existing_offline_operation(&app, &issuer, requested, &requested_binding)? {
            OfflineSubmissionRecovery::Existing(response) => {
                drop(submission);
                return Ok(response);
            }
            OfflineSubmissionRecovery::RetryRejected { next_nonce } => Some(next_nonce),
            OfflineSubmissionRecovery::Absent => None,
        };
    if let Err(error) = preflight_kagemusha_v2_hardware_authorization(
        &app,
        &redeem_request.authorization,
        &redeem_request.bundle.statement.asset,
    ) {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    if let Err(error) = validate_kagemusha_v4_redeem_snapshot(&app, &redeem_request) {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    let public_amount = match redeem_request.amount.public_quantity() {
        Ok(amount) => amount,
        Err(source) => {
            return reconcile_offline_submission_failure(
                &app,
                &issuer,
                requested,
                &requested_binding,
                validation_owned(
                    "offline_redeem_invalid",
                    format!("Offline redemption amount cannot be represented exactly: {source}"),
                ),
            );
        }
    };
    if public_amount > issuer.max_tx_value.clone() {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            validation(
                "offline_amount_exceeds_limit",
                "Offline redemption amount exceeds issuer policy.",
            ),
        );
    }
    let authorization = redeem_request.authorization.clone();
    let instruction = RedeemKagemushaRecursiveV4::new(redeem_request.clone());
    let mut transaction = TransactionBuilder::new(
        *app.state.network_id_ref(),
        issuer.authority.clone().into(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(instruction)]);
    transaction.set_creation_time(Duration::from_millis(authorization.issued_at_ms));
    transaction.set_ttl(Duration::from_millis(
        authorization
            .expires_at_ms
            .saturating_sub(authorization.issued_at_ms),
    ));
    if let Some(nonce) = retry_nonce {
        transaction.set_nonce(nonce);
    }
    let tx =
        match issuer.quote_and_sign_transaction(&app, transaction, "offline_redeem_transaction") {
            Ok(transaction) => transaction,
            Err(error) => {
                return reconcile_offline_submission_failure(
                    &app,
                    &issuer,
                    requested,
                    &requested_binding,
                    error,
                );
            }
        };
    let tx_hash = tx.hash();
    let admission = routing::handle_transaction_with_metrics(
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_REDEEM,
    )
    .await;
    if let Err(error) = admission {
        return reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    match submission.accept(tx_hash) {
        Ok(record) => offline_operation_reference_for_admitted_record(&record),
        Err(error) => reconcile_offline_submission_failure(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        ),
    }
}
fn kagemusha_v4_snapshot_time_ms(state: &impl StateReadOnly) -> u64 {
    state.latest_block().map_or(0, |block| {
        u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    })
}
fn preflight_kagemusha_v2_hardware_authorization(
    app: &SharedAppState,
    authorization: &iroha_data_model::offline::KagemushaRequestAuthorizationV2,
    asset: &iroha_data_model::asset::AssetDefinitionId,
) -> Result<(), Error> {
    let state = app.state.view();
    iroha_core::smartcontracts::isi::offline::isi::preflight_registered_kagemusha_v2_hardware_authorization(
        state.world(),
        authorization,
        asset,
        kagemusha_v4_snapshot_time_ms(&state),
    )
    .map_err(|source| {
        validation_owned(
            "offline_hardware_authorization_invalid",
            format!(
                "Offline hardware authorization does not authenticate against protected registration state: {source}"
            ),
        )
    })
}
fn validate_kagemusha_v4_topup_snapshot(
    app: &SharedAppState,
    request: &OfflineTopUpRequest,
) -> Result<(), Error> {
    if request.current_note.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "offline_wrong_network",
            "Offline top-up request targets a different exact network.",
        ));
    }
    let state = app.state.view();
    let world = state.world();
    let definition = world
        .asset_definition(request.asset.definition())
        .map_err(|_| {
            validation(
                "offline_asset_not_found",
                "Offline top-up asset definition is not registered.",
            )
        })?;
    let live_scale = definition.spec().scale().ok_or_else(|| {
        validation(
            "offline_asset_scale_invalid",
            "Offline payments require a fixed live asset scale.",
        )
    })?;
    if request.amount.scale != live_scale {
        return Err(validation(
            "offline_asset_scale_mismatch",
            "Offline top-up amount scale differs from the live asset scale.",
        ));
    }
    let block_height = u64::try_from(state.height()).unwrap_or(u64::MAX);
    ensure_kagemusha_v4_transaction_release(
        iroha_core::smartcontracts::isi::offline::resolve_kagemusha_recursive_transaction_release_v4(
            world,
            &state.kagemusha_release_catalog,
            &request.artifact_binding,
            block_height,
            block_height,
            app.state.network_id_ref(),
            request.asset.definition(),
            live_scale,
        ),
        true,
    )?;
    let zk_state = world
        .zk_assets()
        .get(request.asset.definition())
        .ok_or_else(|| {
            validation(
                "offline_confidential_state_unavailable",
                "Offline top-up asset has no confidential tree state.",
            )
        })?;
    zk_state.validate_tree_metadata().map_err(|error| {
        validation_owned(
            "offline_confidential_state_invalid",
            format!(
                "Offline confidential tree is inconsistent with its persisted profile: {error}"
            ),
        )
    })?;
    let shield_binding = zk_state.vk_shield.as_ref().ok_or_else(|| {
        validation(
            "offline_topup_shield_verifier_unavailable",
            "Offline top-up asset has no bound shield verifier.",
        )
    })?;
    if request.shield_evidence.proof.vk_ref != shield_binding.id
        || request.shield_evidence.proof.vk_commitment != Some(shield_binding.commitment)
    {
        return Err(validation(
            "offline_topup_shield_verifier_mismatch",
            "Offline top-up proof does not use the asset-bound shield verifier.",
        ));
    }
    let authoritative_initial_root = zk_state.persisted_root;
    let authoritative_leaf_index = u32::try_from(zk_state.commitments.len()).map_err(|_| {
        validation(
            "offline_topup_tree_full",
            "Offline confidential tree position exceeds the protocol index.",
        )
    })?;
    if authoritative_leaf_index >= KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2 {
        return Err(validation(
            "offline_topup_tree_full",
            "Offline confidential tree has no top-up position with a complete recursive lifecycle.",
        ));
    }
    if zk_state
        .commitments
        .contains(&request.current_note.note_commitment)
        || zk_state
            .nullifiers
            .contains(&request.current_note.note_commitment)
        || zk_state
            .nullifiers
            .contains(&request.current_note.spend_nullifier)
        || zk_state
            .commitments
            .contains(&request.current_note.spend_nullifier)
    {
        return Err(validation(
            "offline_topup_state_conflict",
            "Offline top-up note conflicts with existing confidential state.",
        ));
    }
    let authoritative_finalized_root = zk_state
        .preview_commitment_root(request.current_note.note_commitment)
        .map_err(|error| {
            validation_owned(
                "offline_confidential_state_invalid",
                format!("Offline confidential tree is invalid after append: {error}"),
            )
        })?;
    if request.shield_evidence.initial_root != authoritative_initial_root
        || request.shield_evidence.finalized_root != authoritative_finalized_root
        || request.shield_evidence.leaf_index != authoritative_leaf_index
    {
        return Err(validation(
            "offline_topup_snapshot_stale",
            "Offline top-up root or leaf index is stale at the evaluated snapshot.",
        ));
    }
    request
        .validate_authorization_at(kagemusha_v4_snapshot_time_ms(&state))
        .map_err(|err| {
            validation_owned(
                "offline_authorization_invalid",
                format!("Offline top-up authorization is not live at chain time: {err}"),
            )
        })
}
fn validate_kagemusha_v4_redeem_snapshot(
    app: &SharedAppState,
    request: &OfflineRedeemRequest,
) -> Result<(), Error> {
    if request.bundle.statement.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "offline_wrong_network",
            "Offline redemption request targets a different exact network.",
        ));
    }
    let state = app.state.view();
    let world = state.world();
    let definition = world
        .asset_definition(&request.bundle.statement.asset)
        .map_err(|_| {
            validation(
                "offline_asset_not_found",
                "Offline redemption asset definition is not registered.",
            )
        })?;
    let live_scale = definition.spec().scale().ok_or_else(|| {
        validation(
            "offline_asset_scale_invalid",
            "Offline payments require a fixed live asset scale.",
        )
    })?;
    if request.amount.scale != live_scale || request.bundle.statement.asset_scale != live_scale {
        return Err(validation(
            "offline_asset_scale_mismatch",
            "Offline redemption scale differs from the live asset scale.",
        ));
    }
    let block_height = u64::try_from(state.height()).unwrap_or(u64::MAX);
    ensure_kagemusha_v4_transaction_release(
        iroha_core::smartcontracts::isi::offline::resolve_kagemusha_recursive_transaction_release_v4(
            world,
            &state.kagemusha_release_catalog,
            &request.bundle.statement.artifact_binding,
            request.block_height,
            block_height,
            app.state.network_id_ref(),
            &request.bundle.statement.asset,
            live_scale,
        ),
        false,
    )?;
    if let Some(change) = request.offline_change.as_ref() {
        ensure_kagemusha_v4_transaction_release(
            iroha_core::smartcontracts::isi::offline::resolve_kagemusha_recursive_transaction_release_v4(
                world,
                &state.kagemusha_release_catalog,
                &change.bundle.statement.artifact_binding,
                request.block_height,
                block_height,
                &change.bundle.statement.network_id,
                &change.bundle.statement.asset,
                change.bundle.statement.asset_scale,
            ),
            true,
        )?;
    }
    request
        .validate_authorization_at(kagemusha_v4_snapshot_time_ms(&state))
        .map_err(|err| {
            validation_owned(
                "offline_authorization_invalid",
                format!("Offline redemption authorization is not live at chain time: {err}"),
            )
        })
}
fn ensure_kagemusha_v4_transaction_release(
    resolution: Result<
        iroha_core::smartcontracts::isi::offline::KagemushaRecursiveTransactionReleaseV4,
        String,
    >,
    issuance_required: bool,
) -> Result<(), Error> {
    let resolved = resolution.map_err(|error| Error::AppServiceUnavailable {
        code: "offline_recursive_release_invalid",
        message: format!("The authenticated ABI-21 V4 release could not be resolved: {error}"),
    })?;
    ensure_kagemusha_v4_issuance_window(resolved.issuance_active, issuance_required)
}
fn ensure_kagemusha_v4_issuance_window(
    issuance_active: bool,
    issuance_required: bool,
) -> Result<(), Error> {
    if issuance_required && !issuance_active {
        return Err(Error::AppServiceUnavailable {
            code: "offline_recursive_release_outside_issuance_window",
            message: "The selected authenticated ABI-21 V4 release is outside its issuance window."
                .to_owned(),
        });
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KagemushaV2OperationKind {
    TopUp,
    Redeem,
}
impl From<KagemushaV2OperationKind> for OfflineOperationKind {
    fn from(value: KagemushaV2OperationKind) -> Self {
        match value {
            KagemushaV2OperationKind::TopUp => Self::TopUp,
            KagemushaV2OperationKind::Redeem => Self::Redeem,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfflineOperationRequest<TopUp, Redeem> {
    TopUp(TopUp),
    Redeem(Redeem),
}
type OfflineOperationRequestOwned =
    OfflineOperationRequest<Box<OfflineTopUpRequest>, Box<OfflineRedeemRequest>>;
type OfflineOperationRequestRef<'a> =
    OfflineOperationRequest<&'a OfflineTopUpRequest, &'a OfflineRedeemRequest>;
impl<TopUp, Redeem> OfflineOperationRequest<TopUp, Redeem> {
    const fn kind(&self) -> KagemushaV2OperationKind {
        match self {
            Self::TopUp(_) => KagemushaV2OperationKind::TopUp,
            Self::Redeem(_) => KagemushaV2OperationKind::Redeem,
        }
    }
}
impl<'a> OfflineOperationRequestRef<'a> {
    fn authorization(self) -> &'a iroha_data_model::offline::KagemushaRequestAuthorizationV2 {
        match self {
            Self::TopUp(request) => &request.authorization,
            Self::Redeem(request) => &request.authorization,
        }
    }
    fn into_owned(self) -> OfflineOperationRequestOwned {
        match self {
            Self::TopUp(request) => OfflineOperationRequest::TopUp(Box::new(request.clone())),
            Self::Redeem(request) => OfflineOperationRequest::Redeem(Box::new(request.clone())),
        }
    }
    fn as_kagemusha_request(self) -> KagemushaOperationRequestV4<'a> {
        match self {
            Self::TopUp(request) => KagemushaOperationRequestV4::TopUp(request),
            Self::Redeem(request) => KagemushaOperationRequestV4::Redeem(request),
        }
    }
}
impl OfflineOperationRequestOwned {
    fn as_ref(&self) -> OfflineOperationRequestRef<'_> {
        match self {
            Self::TopUp(request) => OfflineOperationRequest::TopUp(request.as_ref()),
            Self::Redeem(request) => OfflineOperationRequest::Redeem(request.as_ref()),
        }
    }
    fn authorization(&self) -> &iroha_data_model::offline::KagemushaRequestAuthorizationV2 {
        match self {
            Self::TopUp(request) => &request.authorization,
            Self::Redeem(request) => &request.authorization,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OfflineOperationRequestBinding {
    operation_id: [u8; 32],
    kind: KagemushaV2OperationKind,
    canonical_request_digest: [u8; 32],
    submitted_at_ms: u64,
    expires_at_ms: u64,
}
impl OfflineOperationRequestBinding {
    fn from_request(request: OfflineOperationRequestRef<'_>) -> Result<Self, Error> {
        let authorization = request.authorization();
        Ok(Self {
            operation_id: authorization.operation_id,
            kind: request.kind(),
            canonical_request_digest: request
                .as_kagemusha_request()
                .canonical_request_digest()
                .map_err(|source| Error::SerializationFailure {
                    context: "offline_request_binding",
                    source: Box::new(source),
                })?,
            submitted_at_ms: authorization.issued_at_ms,
            expires_at_ms: authorization.expires_at_ms,
        })
    }
}
fn ensure_same_offline_request_binding(
    existing: &OfflineOperationRequestBinding,
    requested: &OfflineOperationRequestBinding,
) -> Result<(), Error> {
    if existing == requested {
        return Ok(());
    }
    Err(operation_id_conflict())
}
fn ensure_same_offline_request<TopUp: PartialEq, Redeem: PartialEq>(
    existing: &OfflineOperationRequest<TopUp, Redeem>,
    requested: &OfflineOperationRequest<TopUp, Redeem>,
) -> Result<(), Error> {
    if existing == requested {
        return Ok(());
    }
    Err(operation_id_conflict())
}
fn operation_id_conflict() -> Error {
    Error::AppConflict {
        code: "operation_id_conflict",
        message: "Offline operation id is already bound to a different request.".to_owned(),
    }
}
#[derive(Debug, Clone)]
struct OfflineOperationRecord {
    request: OfflineOperationRequestOwned,
    canonical_request_digest: [u8; 32],
    transaction_hash: HashOf<SignedTransaction>,
    transaction_nonce: Option<NonZeroU32>,
    submitted_at_ms: u64,
}
impl OfflineOperationRecord {
    fn binding(&self) -> OfflineOperationRequestBinding {
        let authorization = self.request.authorization();
        OfflineOperationRequestBinding {
            operation_id: authorization.operation_id,
            kind: self.request.kind(),
            canonical_request_digest: self.canonical_request_digest,
            submitted_at_ms: authorization.issued_at_ms,
            expires_at_ms: authorization.expires_at_ms,
        }
    }
}
#[derive(Debug, Clone)]
struct AdmittedOfflineOperationRecord {
    binding: OfflineOperationRequestBinding,
    transaction_hash: HashOf<SignedTransaction>,
}
#[derive(Debug)]
struct OfflineOperationRegistry {
    records: BTreeMap<[u8; 32], AdmittedOfflineOperationRecord>,
    in_flight: BTreeMap<[u8; 32], InFlightSubmission>,
    max_entries: NonZeroUsize,
    max_accounted_bytes: NonZeroUsize,
}
impl OfflineOperationRegistry {
    fn new(max_entries: NonZeroUsize, max_accounted_bytes: NonZeroUsize) -> Self {
        Self {
            records: BTreeMap::new(),
            in_flight: BTreeMap::new(),
            max_entries,
            max_accounted_bytes,
        }
    }
    fn accounted_bytes(&self) -> usize {
        self.tracked_entries()
            .saturating_mul(ADMITTED_OPERATION_ACCOUNTED_BYTES)
    }
    fn tracked_entries(&self) -> usize {
        self.records.len().saturating_add(self.in_flight.len())
    }
    fn has_capacity_for_new_operation(&self) -> bool {
        self.tracked_entries().saturating_add(1) <= self.max_entries.get()
            && self
                .accounted_bytes()
                .saturating_add(ADMITTED_OPERATION_ACCOUNTED_BYTES)
                <= self.max_accounted_bytes.get()
    }
    fn prune_expired(&mut self, now_ms: u64) {
        self.records.retain(|_, record| {
            offline_operation_is_retained(record.binding.expires_at_ms, now_ms)
        });
    }
    fn get(&self, operation_id: &[u8; 32]) -> Option<&AdmittedOfflineOperationRecord> {
        self.records.get(operation_id)
    }
    fn insert_reserved(&mut self, record: AdmittedOfflineOperationRecord) {
        self.records.insert(record.binding.operation_id, record);
        debug_assert!(self.tracked_entries() <= self.max_entries.get());
        debug_assert!(self.accounted_bytes() <= self.max_accounted_bytes.get());
    }
}
fn require_idempotency_key(headers: &HeaderMap, operation_id: [u8; 32]) -> Result<(), Error> {
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "The signed offline operation id must be non-zero.".to_owned(),
        });
    }
    let expected = hex::encode(operation_id);
    let actual = validated_idempotency_key(headers)?;
    if actual != expected {
        return Err(Error::AppConflict {
            code: "idempotency_key_conflict",
            message: "Idempotency-Key does not match the signed operation id.".to_owned(),
        });
    }
    Ok(())
}
fn validated_idempotency_key(headers: &HeaderMap) -> Result<&str, Error> {
    let mut values = headers.get_all("idempotency-key").iter();
    let Some(raw) = values.next() else {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_missing",
            message: "Offline commands require Idempotency-Key equal to the signed operation id."
                .to_owned(),
        });
    };
    if values.next().is_some() {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_invalid",
            message: "Offline commands require exactly one Idempotency-Key header.".to_owned(),
        });
    }
    let actual = raw.to_str().map_err(|_| Error::AppQueryValidation {
        code: "idempotency_key_invalid",
        message: "Idempotency-Key must be lowercase hexadecimal ASCII.".to_owned(),
    })?;
    if actual.len() != 64
        || actual.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || actual.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_invalid",
            message: "Idempotency-Key must be exactly 64 lowercase hexadecimal characters."
                .to_owned(),
        });
    }
    Ok(actual)
}
impl OfflineCommandRuntime {
    fn claim_submission(
        self: &Arc<Self>,
        binding: OfflineOperationRequestBinding,
    ) -> Result<SubmissionClaim, Error> {
        let operation_id = binding.operation_id;
        // Accepted bindings and in-flight reservations share one capacity and
        // one mutex. The transition from reservation to admitted binding is
        // therefore atomic: no request can observe an absent operation between
        // the two states or overbook memory while submissions are stalled.
        let mut admission = self.admission.lock();
        let now_ms = now_ms();
        admission.prune_expired(now_ms);
        if let Some(existing) = admission.get(&operation_id) {
            ensure_same_offline_request_binding(&existing.binding, &binding)?;
            return Ok(SubmissionClaim::Accepted(existing.clone()));
        }
        if let Some(existing) = admission.in_flight.get(&operation_id) {
            ensure_same_offline_request_binding(&existing.binding, &binding)?;
            return Ok(SubmissionClaim::Follower(existing.updates.subscribe()));
        }
        if !admission.has_capacity_for_new_operation() {
            return Err(Error::AppServiceUnavailable {
                code: "offline_operation_capacity_exhausted",
                message: "Offline operation admission capacity is exhausted; retry after an in-flight submission completes or an admitted binding expires.".to_owned(),
            });
        }
        let token = Arc::new(());
        let (updates, _) = watch::channel(SubmissionOutcome::Pending);
        admission.in_flight.insert(
            operation_id,
            InFlightSubmission {
                binding,
                token: Arc::clone(&token),
                updates: updates.clone(),
            },
        );
        Ok(SubmissionClaim::Leader(SubmissionLeader {
            issuer: Arc::clone(self),
            operation_id,
            token,
            binding,
            updates,
            active: true,
        }))
    }
    fn record_admitted_operation(
        &self,
        record: AdmittedOfflineOperationRecord,
        token: &Arc<()>,
    ) -> Result<AdmittedOfflineOperationRecord, Error> {
        let operation_id = record.binding.operation_id;
        let mut admission = self.admission.lock();
        admission.prune_expired(now_ms());
        if let Some(existing) = admission.get(&operation_id).cloned() {
            ensure_same_offline_request_binding(&existing.binding, &record.binding)?;
            if admission
                .in_flight
                .get(&operation_id)
                .is_some_and(|entry| Arc::ptr_eq(&entry.token, token))
            {
                admission.in_flight.remove(&operation_id);
            }
            return Ok(existing);
        }
        let owns_reservation = admission
            .in_flight
            .get(&operation_id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.token, token));
        if !owns_reservation {
            return Err(Error::AppServiceUnavailable {
                code: "offline_operation_admission_inconsistent",
                message: "The accepted offline operation lost its admission reservation."
                    .to_owned(),
            });
        }
        admission.in_flight.remove(&operation_id);
        admission.insert_reserved(record.clone());
        Ok(record)
    }
}
impl SubmissionLeader {
    fn accept(
        mut self,
        transaction_hash: HashOf<SignedTransaction>,
    ) -> Result<AdmittedOfflineOperationRecord, Error> {
        let record = AdmittedOfflineOperationRecord {
            binding: self.binding,
            transaction_hash,
        };
        match self
            .issuer
            .record_admitted_operation(record.clone(), &self.token)
        {
            Ok(admitted) => {
                self.finish(SubmissionOutcome::Accepted(admitted.clone()));
                Ok(admitted)
            }
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    operation_id = %hex::encode(self.operation_id),
                    "accepted offline operation could not transition its admission reservation"
                );
                self.finish(SubmissionOutcome::Retry);
                Err(error)
            }
        }
    }
    fn finish(&mut self, outcome: SubmissionOutcome) {
        if !self.active {
            return;
        }
        let mut admission = self.issuer.admission.lock();
        if admission
            .in_flight
            .get(&self.operation_id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.token, &self.token))
        {
            admission.in_flight.remove(&self.operation_id);
        }
        let _ = self.updates.send_replace(outcome);
        self.active = false;
    }
}
impl Drop for SubmissionLeader {
    fn drop(&mut self) {
        self.finish(SubmissionOutcome::Retry);
    }
}
async fn wait_for_submission_outcome(
    mut receiver: watch::Receiver<SubmissionOutcome>,
) -> SubmissionOutcome {
    loop {
        let outcome = receiver.borrow().clone();
        if !matches!(outcome, SubmissionOutcome::Pending) {
            return outcome;
        }
        if receiver.changed().await.is_err() {
            return SubmissionOutcome::Retry;
        }
    }
}
fn offline_operation_is_retained(expires_at_ms: u64, now_ms: u64) -> bool {
    expires_at_ms.saturating_add(OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS) >= now_ms
}
fn find_admitted_offline_operation(
    issuer: &OfflineCommandRuntime,
    requested_binding: &OfflineOperationRequestBinding,
    rejected_transaction_hash: Option<&HashOf<SignedTransaction>>,
) -> Result<Option<AdmittedOfflineOperationRecord>, Error> {
    let mut admission = issuer.admission.lock();
    let now_ms = now_ms();
    admission.prune_expired(now_ms);
    let operation_id = requested_binding.operation_id;
    let Some(existing) = admission.get(&operation_id).cloned() else {
        return Ok(None);
    };
    ensure_same_offline_request_binding(&existing.binding, requested_binding)?;
    if rejected_transaction_hash.is_some_and(|hash| hash == &existing.transaction_hash) {
        admission.records.remove(&operation_id);
        return Ok(None);
    }
    Ok(Some(existing))
}
fn offline_operation_status_uri(operation_id: [u8; 32]) -> String {
    format!("/v1/offline/operations/{}", hex::encode(operation_id))
}
fn offline_operation_reference_response(
    operation_id: [u8; 32],
    kind: OfflineOperationKind,
    transaction_hash: String,
    submitted_at_ms: u64,
) -> Result<AxResponse, Error> {
    let status_uri = offline_operation_status_uri(operation_id);
    let payload = OfflineOperationReference {
        operation_id: hex::encode(operation_id),
        kind,
        state: OfflineOperationState::Pending,
        transaction_hash,
        status_uri: status_uri.clone(),
        submitted_at_ms,
    };
    let mut response = crate::utils::respond_with_status_and_format(
        axum::http::StatusCode::ACCEPTED,
        payload,
        crate::utils::current_response_format(),
    );
    if let Ok(location) = axum::http::HeaderValue::from_str(&status_uri) {
        response
            .headers_mut()
            .insert(axum::http::header::LOCATION, location);
    }
    response.headers_mut().insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("1"),
    );
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
fn offline_operation_reference_for_admitted_record(
    record: &AdmittedOfflineOperationRecord,
) -> Result<AxResponse, Error> {
    offline_operation_reference_response(
        record.binding.operation_id,
        record.binding.kind.into(),
        record.transaction_hash.to_string(),
        record.binding.submitted_at_ms,
    )
}
fn parse_operation_id(raw: &str) -> Result<[u8; 32], Error> {
    if raw.len() != 64
        || raw.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || raw.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "Offline operation id must be exactly 64 lowercase hexadecimal characters."
                .to_owned(),
        });
    }
    let bytes = hex::decode(raw).map_err(|_| Error::AppQueryValidation {
        code: "operation_id_invalid",
        message: "Offline operation id is not valid hexadecimal.".to_owned(),
    })?;
    let operation_id: [u8; 32] = bytes.try_into().map_err(|_| Error::AppQueryValidation {
        code: "operation_id_invalid",
        message: "Offline operation id must decode to 32 bytes.".to_owned(),
    })?;
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "Offline operation id must be non-zero.".to_owned(),
        });
    }
    Ok(operation_id)
}
fn offline_operation_record_in_transaction(
    transaction: &SignedTransaction,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    if operation_id == [0; 32] || transaction.authority() != issuer_authority {
        return None;
    }
    let carrier = classify_kagemusha_operation_transaction_v4(transaction)
        .ok()
        .flatten()?;
    offline_operation_record_from_carrier(transaction, carrier, operation_id)
}
fn offline_operation_record_in_entrypoint(
    entrypoint: &TransactionEntrypoint,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    if operation_id == [0; 32] {
        return None;
    }
    let TransactionEntrypoint::External(transaction) = entrypoint else {
        return None;
    };
    if transaction.authority() != issuer_authority {
        return None;
    }
    let carrier = classify_kagemusha_operation_entrypoint_v4(entrypoint)
        .ok()
        .flatten()?;
    offline_operation_record_from_carrier(transaction, carrier, operation_id)
}
fn offline_operation_record_from_carrier(
    transaction: &SignedTransaction,
    carrier: KagemushaOperationCarrierV4<'_>,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    if carrier.operation_id() != operation_id {
        return None;
    }
    let request = match carrier.request() {
        KagemushaOperationRequestV4::TopUp(request) => OfflineOperationRequest::TopUp(request),
        KagemushaOperationRequestV4::Redeem(request) => OfflineOperationRequest::Redeem(request),
    };
    Some(OfflineOperationRecord {
        request: request.into_owned(),
        canonical_request_digest: carrier.canonical_request_digest(),
        transaction_hash: transaction.hash(),
        transaction_nonce: transaction.nonce(),
        submitted_at_ms: request.authorization().issued_at_ms,
    })
}
fn terminal_offline_operation_in_entrypoint(
    entrypoint: &TransactionEntrypoint,
    result: &TransactionResult,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
    finalized_block_height: u64,
) -> Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)> {
    let record =
        offline_operation_record_in_entrypoint(entrypoint, issuer_authority, operation_id)?;
    let transaction_hash = record.transaction_hash.to_string();
    Some((
        record,
        kagemusha_v4_committed_finality(
            operation_id,
            transaction_hash,
            finalized_block_height,
            result
                .0
                .as_ref()
                .err()
                .map(|reason| kagemusha_v4_rejection_detail(Some(reason))),
        ),
    ))
}
fn terminal_offline_operation_from_outcome_evidence(
    outcome: &KagemushaOperationOutcomeRecordV4,
    entrypoint: &TransactionEntrypoint,
    result: &TransactionResult,
    configured_issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<(OfflineOperationRecord, KagemushaV2CommittedFinality), Error> {
    let TransactionEntrypoint::External(transaction) = entrypoint else {
        return Err(offline_operation_evidence_inconsistent(
            "The terminal offline operation locator does not reference a direct external carrier.",
        ));
    };
    let carrier = classify_kagemusha_operation_entrypoint_v4(entrypoint)
        .map_err(|error| {
            offline_operation_evidence_inconsistent(format!(
                "The terminal offline operation carrier is invalid: {error}"
            ))
        })?
        .ok_or_else(|| {
            offline_operation_evidence_inconsistent(
                "The terminal offline operation locator does not reference a Kagemusha carrier.",
            )
        })?;
    let result_matches_outcome = matches!(
        (outcome.outcome, result.is_ok()),
        (KagemushaOperationOutcomeStateV4::Applied, true)
            | (KagemushaOperationOutcomeStateV4::Rejected, false)
    );
    let signed_transaction_wire_hash =
        signed_transaction_wire_hash_v4(transaction).map_err(|error| {
            offline_operation_evidence_inconsistent(format!(
                "The terminal offline operation signed transaction is not canonical: {error}"
            ))
        })?;
    let configured_issuer_authority_digest =
        kagemusha_operation_authority_digest_v4(configured_issuer_authority).map_err(|error| {
            offline_operation_evidence_inconsistent(format!(
                "The configured offline operation authority is not canonical: {error}"
            ))
        })?;
    let outer_authority_digest = kagemusha_operation_authority_digest_v4(transaction.authority())
        .map_err(|error| {
        offline_operation_evidence_inconsistent(format!(
            "The terminal offline operation outer authority is not canonical: {error}"
        ))
    })?;
    let request_authority_digest =
        kagemusha_operation_authority_digest_v4(&carrier.request().authorization().authority)
            .map_err(|error| {
                offline_operation_evidence_inconsistent(format!(
                    "The terminal offline operation request authority is not canonical: {error}"
                ))
            })?;
    let foreign_rejected_attempt = outcome.outcome == KagemushaOperationOutcomeStateV4::Rejected
        && outcome.outer_authority_digest != configured_issuer_authority_digest;
    if outcome.operation_id != operation_id
        || outcome.outer_authority_digest != outer_authority_digest
        || foreign_rejected_attempt
        || outcome.request_authority_digest != request_authority_digest
        || outcome.kind != carrier.kind()
        || outcome.operation_id != carrier.operation_id()
        || outcome.canonical_request_digest != carrier.canonical_request_digest()
        || outcome.signed_transaction_wire_hash != signed_transaction_wire_hash
        || outcome.entrypoint_hash != entrypoint.hash()
        || outcome.result_hash != Some(result.hash())
        || !result_matches_outcome
    {
        return Err(offline_operation_evidence_inconsistent(
            "The terminal offline operation outcome does not match its exact carrier and result evidence.",
        ));
    }
    terminal_offline_operation_in_entrypoint(
        entrypoint,
        result,
        transaction.authority(),
        operation_id,
        outcome.carrier_height,
    )
    .ok_or_else(|| {
        offline_operation_evidence_inconsistent(
            "The terminal offline operation evidence could not reconstruct its canonical request.",
        )
    })
}
fn offline_operation_record_from_pending(
    pending: &PendingKagemushaOperation,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<OfflineOperationRecord, Error> {
    let transaction = pending.signed_transaction();
    let carrier = classify_kagemusha_operation_transaction_v4(transaction)
        .map_err(|error| {
            offline_operation_evidence_inconsistent(format!(
                "The pending offline operation carrier is invalid: {error}"
            ))
        })?
        .ok_or_else(|| {
            offline_operation_evidence_inconsistent(
                "The pending offline operation owner is not a Kagemusha carrier.",
            )
        })?;
    let authorization = carrier.request().authorization();
    let signed_transaction_wire_hash =
        signed_transaction_wire_hash_v4(transaction).map_err(|error| {
            offline_operation_evidence_inconsistent(format!(
                "The pending offline operation signed transaction is not canonical: {error}"
            ))
        })?;
    if pending.authority() != issuer_authority
        || transaction.authority() != issuer_authority
        || pending.operation_id() != operation_id
        || carrier.operation_id() != operation_id
        || pending.kind() != carrier.kind()
        || pending.canonical_request_digest() != carrier.canonical_request_digest()
        || pending.submitted_at_ms() != authorization.issued_at_ms
        || pending.expires_at_ms() != authorization.expires_at_ms
        || pending.signed_transaction_wire_hash() != signed_transaction_wire_hash
        || pending.entrypoint_hash() != transaction.hash_as_entrypoint()
    {
        return Err(offline_operation_evidence_inconsistent(
            "The pending offline operation owner does not match its exact carrier evidence.",
        ));
    }
    offline_operation_record_from_carrier(transaction, carrier, operation_id).ok_or_else(|| {
        offline_operation_evidence_inconsistent(
            "The pending offline operation evidence could not reconstruct its canonical request.",
        )
    })
}
fn pending_offline_operation_lookup_error(error: PendingKagemushaOperationLookupError) -> Error {
    match error {
        PendingKagemushaOperationLookupError::Unavailable { .. }
        | PendingKagemushaOperationLookupError::DurabilityTransition { .. } => {
            offline_operation_pending_unavailable(
                "Pending offline operation ownership is temporarily unavailable; retry after Queue recovery completes.",
            )
        }
        PendingKagemushaOperationLookupError::Inconsistent { .. } => {
            offline_operation_evidence_inconsistent(
                "Pending offline operation ownership is internally inconsistent.",
            )
        }
    }
}
enum PendingOfflineOperationResolution {
    Pending(OfflineOperationRecord),
    Terminal(OfflineOperationRecord, KagemushaV2CommittedFinality),
}
fn resolve_pending_offline_operation_by_id(
    app: &SharedAppState,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<PendingOfflineOperationResolution>, Error> {
    let pending = {
        let state = app.state.view();
        app.queue
            .pending_kagemusha_operation(&state, issuer_authority, operation_id)
    };
    match pending {
        Ok(Some(pending)) => {
            offline_operation_record_from_pending(&pending, issuer_authority, operation_id)
                .map(PendingOfflineOperationResolution::Pending)
                .map(Some)
        }
        Ok(None) => find_terminal_offline_operation_by_id(app, issuer_authority, operation_id).map(
            |terminal| {
                terminal.map(|(record, finality)| {
                    PendingOfflineOperationResolution::Terminal(record, finality)
                })
            },
        ),
        Err(error @ PendingKagemushaOperationLookupError::Unavailable { .. })
        | Err(error @ PendingKagemushaOperationLookupError::DurabilityTransition { .. }) => {
            match find_terminal_offline_operation_by_id(app, issuer_authority, operation_id)? {
                Some((
                    record,
                    finality @ KagemushaV2CommittedFinality {
                        outcome: KagemushaV2TerminalOutcome::Applied,
                        ..
                    },
                )) => Ok(Some(PendingOfflineOperationResolution::Terminal(
                    record, finality,
                ))),
                Some((
                    _,
                    KagemushaV2CommittedFinality {
                        outcome: KagemushaV2TerminalOutcome::Rejected(_),
                        ..
                    },
                ))
                | None => Err(pending_offline_operation_lookup_error(error)),
            }
        }
        Err(error @ PendingKagemushaOperationLookupError::Inconsistent { .. }) => {
            Err(pending_offline_operation_lookup_error(error))
        }
    }
}
enum OfflineSubmissionRecovery {
    Existing(AxResponse),
    RetryRejected { next_nonce: NonZeroU32 },
    Absent,
}
fn next_rejected_attempt_nonce(rejected: &OfflineOperationRecord) -> Result<NonZeroU32, Error> {
    let Some(nonce) = rejected.transaction_nonce else {
        return Ok(NonZeroU32::MIN);
    };
    nonce
        .get()
        .checked_add(1)
        .and_then(NonZeroU32::new)
        .ok_or_else(|| Error::AppConflict {
            code: "offline_operation_retry_exhausted",
            message: "The rejected offline operation exhausted its deterministic transaction nonce space; submit a newly authorized operation id."
                .to_owned(),
        })
}
fn find_existing_offline_operation(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    requested: OfflineOperationRequestRef<'_>,
    requested_binding: &OfflineOperationRequestBinding,
) -> Result<OfflineSubmissionRecovery, Error> {
    let authorization = requested.authorization();
    let mut rejected_attempt = None;
    if let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, authorization.operation_id)?
    {
        ensure_same_offline_request(&record.request.as_ref(), &requested)?;
        match finality.outcome {
            KagemushaV2TerminalOutcome::Applied => {
                return offline_operation_reference_response(
                    authorization.operation_id,
                    requested.kind().into(),
                    finality.transaction_hash,
                    authorization.issued_at_ms,
                )
                .map(OfflineSubmissionRecovery::Existing);
            }
            KagemushaV2TerminalOutcome::Rejected(_) => rejected_attempt = Some(record),
        }
    }
    let rejected_transaction_hash = rejected_attempt
        .as_ref()
        .map(|record| &record.transaction_hash);
    if let Some(existing) =
        find_admitted_offline_operation(issuer, requested_binding, rejected_transaction_hash)?
    {
        return offline_operation_reference_for_admitted_record(&existing)
            .map(OfflineSubmissionRecovery::Existing);
    }
    if let Some(resolution) =
        resolve_pending_offline_operation_by_id(app, &issuer.authority, authorization.operation_id)?
    {
        match resolution {
            PendingOfflineOperationResolution::Pending(existing) => {
                ensure_same_offline_request(&existing.request.as_ref(), &requested)?;
                if rejected_attempt
                    .as_ref()
                    .is_some_and(|rejected| rejected.transaction_hash == existing.transaction_hash)
                {
                    return Err(offline_operation_evidence_inconsistent(
                        "A rejected offline operation is simultaneously reported as pending.",
                    ));
                }
                return offline_operation_reference_response(
                    authorization.operation_id,
                    existing.request.kind().into(),
                    existing.transaction_hash.to_string(),
                    existing.submitted_at_ms,
                )
                .map(OfflineSubmissionRecovery::Existing);
            }
            PendingOfflineOperationResolution::Terminal(existing, finality) => {
                ensure_same_offline_request(&existing.request.as_ref(), &requested)?;
                match finality.outcome {
                    KagemushaV2TerminalOutcome::Applied => {
                        return offline_operation_reference_response(
                            authorization.operation_id,
                            existing.request.kind().into(),
                            existing.transaction_hash.to_string(),
                            existing.submitted_at_ms,
                        )
                        .map(OfflineSubmissionRecovery::Existing);
                    }
                    KagemushaV2TerminalOutcome::Rejected(_) => {
                        rejected_attempt = Some(existing);
                    }
                }
            }
        }
    }
    match rejected_attempt {
        Some(rejected) => Ok(OfflineSubmissionRecovery::RetryRejected {
            next_nonce: next_rejected_attempt_nonce(&rejected)?,
        }),
        None => Ok(OfflineSubmissionRecovery::Absent),
    }
}
// Every fallible boundary after the first authoritative lookup may race a
// direct manager that commits the same exact request. Re-read global finality,
// local admission, and typed Queue provenance before exposing that stale error.
fn reconcile_offline_submission_failure(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    requested: OfflineOperationRequestRef<'_>,
    requested_binding: &OfflineOperationRequestBinding,
    original_error: Error,
) -> Result<AxResponse, Error> {
    match find_existing_offline_operation(app, issuer, requested, requested_binding)? {
        OfflineSubmissionRecovery::Existing(response) => Ok(response),
        OfflineSubmissionRecovery::RetryRejected { .. } | OfflineSubmissionRecovery::Absent => {
            Err(original_error)
        }
    }
}
fn find_terminal_offline_operation_by_id(
    app: &SharedAppState,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)>, Error> {
    let outcome = {
        let world = app.state.world_view();
        kagemusha_operation_outcome_v4(&world, issuer_authority, operation_id).map_err(|error| {
            iroha_logger::warn!(
                ?error,
                operation_id = %hex::encode(operation_id),
                "failed to load the canonical Kagemusha operation outcome"
            );
            offline_operation_evidence_inconsistent(
                "The canonical offline operation outcome record is malformed.",
            )
        })?
    };
    reconstruct_terminal_offline_operation(app, issuer_authority, operation_id, outcome)
}
fn find_global_applied_offline_operation_by_id(
    app: &SharedAppState,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)>, Error> {
    let outcome = {
        let world = app.state.world_view();
        kagemusha_operation_finality_v4(&world, operation_id).map_err(|error| {
            iroha_logger::warn!(
                ?error,
                operation_id = %hex::encode(operation_id),
                "failed to load the global Kagemusha operation finality"
            );
            offline_operation_evidence_inconsistent(
                "The global offline operation finality record is malformed.",
            )
        })?
    };
    reconstruct_terminal_offline_operation(app, issuer_authority, operation_id, outcome)
}
fn reconstruct_terminal_offline_operation(
    app: &SharedAppState,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
    outcome: Option<KagemushaOperationOutcomeRecordV4>,
) -> Result<Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)>, Error> {
    let Some(outcome) = outcome else {
        return Ok(None);
    };
    let height = usize::try_from(outcome.carrier_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| {
            offline_operation_evidence_inconsistent(
                "The terminal offline operation has an invalid carrier height.",
            )
        })?;
    let phase_index = usize::try_from(outcome.phase_index).map_err(|_| {
        offline_operation_evidence_inconsistent(
            "The terminal offline operation has an invalid execution-phase index.",
        )
    })?;
    match outcome.execution_phase {
        KagemushaOperationExecutionPhaseV4::Ordinary => {
            let block = app
                .kura
                .get_block(height)
                .ok_or_else(|| Error::AppServiceUnavailable {
                    code: "offline_operation_history_unavailable",
                    message: "The terminal offline operation block body is not locally available."
                        .to_owned(),
                })?;
            if !block.has_results()
                || block.header().height().get() != outcome.carrier_height
                || block.entrypoints_cloned().len() != block.results().len()
            {
                return Err(offline_operation_evidence_inconsistent(
                    "The terminal offline operation block has inconsistent entrypoint evidence.",
                ));
            }
            let entrypoint = block
                .external_entrypoints_slice()
                .get(phase_index)
                .ok_or_else(|| {
                    offline_operation_evidence_inconsistent(
                        "The terminal offline operation ordinary index is out of bounds.",
                    )
                })?;
            let result = block.results().nth(phase_index).ok_or_else(|| {
                offline_operation_evidence_inconsistent(
                    "The terminal offline operation ordinary result is missing.",
                )
            })?;
            terminal_offline_operation_from_outcome_evidence(
                &outcome,
                entrypoint,
                result,
                issuer_authority,
                operation_id,
            )
            .map(Some)
        }
        KagemushaOperationExecutionPhaseV4::Merge => {
            let merge_entry = app
                .kura
                .get_merge_entry_by_carrier_height(height)
                .map_err(|error| {
                    iroha_logger::warn!(
                        ?error,
                        operation_id = %hex::encode(operation_id),
                        carrier_height = outcome.carrier_height,
                        "failed to resolve terminal offline operation merge evidence"
                    );
                    Error::AppServiceUnavailable {
                        code: "offline_operation_history_unavailable",
                        message:
                            "The terminal offline operation merge entry is not locally available."
                                .to_owned(),
                    }
                })?
                .ok_or_else(|| Error::AppServiceUnavailable {
                    code: "offline_operation_history_unavailable",
                    message: "The terminal offline operation merge entry is not locally available."
                        .to_owned(),
                })?;
            let batch = merge_entry.execution_batch.as_ref().ok_or_else(|| {
                Error::AppServiceUnavailable {
                    code: "offline_operation_history_unavailable",
                    message:
                        "The terminal offline operation merge execution batch is not available."
                            .to_owned(),
                }
            })?;
            if batch.application_block_header.height().get() != outcome.carrier_height {
                return Err(offline_operation_evidence_inconsistent(
                    "The terminal offline operation merge batch has the wrong carrier height.",
                ));
            }
            let mut remaining = phase_index;
            let mut evidence = None;
            let mut entrypoint_count = 0_u64;
            for lane in &batch.lanes {
                if lane.entrypoints.len() != lane.results.len() {
                    return Err(offline_operation_evidence_inconsistent(
                        "The terminal offline operation merge execution has misaligned results.",
                    ));
                }
                entrypoint_count = entrypoint_count
                    .checked_add(u64::try_from(lane.entrypoints.len()).map_err(|_| {
                        offline_operation_evidence_inconsistent(
                            "The terminal offline operation merge entrypoint count overflowed.",
                        )
                    })?)
                    .ok_or_else(|| {
                        offline_operation_evidence_inconsistent(
                            "The terminal offline operation merge entrypoint count overflowed.",
                        )
                    })?;
                if evidence.is_none() {
                    if remaining < lane.entrypoints.len() {
                        evidence = Some((&lane.entrypoints[remaining], &lane.results[remaining]));
                    } else {
                        remaining -= lane.entrypoints.len();
                    }
                }
            }
            if entrypoint_count != batch.entrypoint_count {
                return Err(offline_operation_evidence_inconsistent(
                    "The terminal offline operation merge batch has an inconsistent entrypoint count.",
                ));
            }
            let (entrypoint, result) = evidence.ok_or_else(|| {
                offline_operation_evidence_inconsistent(
                    "The terminal offline operation merge index is out of bounds.",
                )
            })?;
            terminal_offline_operation_from_outcome_evidence(
                &outcome,
                entrypoint,
                result,
                issuer_authority,
                operation_id,
            )
            .map(Some)
        }
    }
}
fn ensure_admitted_operation_binding_matches_recovered_record(
    admitted: &AdmittedOfflineOperationRecord,
    recovered: &OfflineOperationRecord,
) -> Result<(), Error> {
    let recovered_binding = recovered.binding();
    if admitted.binding != recovered_binding {
        return Err(offline_operation_evidence_inconsistent(
            "The authoritative offline operation differs from its admitted request binding.",
        ));
    }
    Ok(())
}
fn terminal_operation_supersedes_admitted_record(
    admitted: &AdmittedOfflineOperationRecord,
    recovered: &OfflineOperationRecord,
    finality: &KagemushaV2CommittedFinality,
) -> Result<bool, Error> {
    let recovered_binding = recovered.binding();
    if admitted.binding != recovered_binding {
        return Err(offline_operation_evidence_inconsistent(
            "The authoritative offline operation differs from its admitted request binding.",
        ));
    }
    Ok(
        matches!(&finality.outcome, KagemushaV2TerminalOutcome::Applied)
            || admitted.transaction_hash == recovered.transaction_hash,
    )
}
fn terminal_operation_supersedes_pending_record(
    pending: &OfflineOperationRecord,
    recovered: &OfflineOperationRecord,
    finality: &KagemushaV2CommittedFinality,
) -> Result<bool, Error> {
    ensure_same_offline_request(&recovered.request.as_ref(), &pending.request.as_ref())?;
    Ok(
        matches!(&finality.outcome, KagemushaV2TerminalOutcome::Applied)
            || pending.transaction_hash == recovered.transaction_hash,
    )
}
fn pending_offline_operation_status(
    operation_id: [u8; 32],
    kind: KagemushaV2OperationKind,
    transaction_hash: &HashOf<SignedTransaction>,
    submitted_at_ms: u64,
) -> OfflineOperationStatus {
    OfflineOperationStatus::Pending {
        operation_id: hex::encode(operation_id),
        kind: kind.into(),
        transaction_hash: transaction_hash.to_string(),
        submitted_at_ms,
    }
}
fn rejected_offline_operation_status(
    operation_id: [u8; 32],
    kind: KagemushaV2OperationKind,
    transaction_hash: &HashOf<SignedTransaction>,
    message: String,
) -> OfflineOperationStatus {
    OfflineOperationStatus::Rejected {
        operation_id: hex::encode(operation_id),
        kind: kind.into(),
        transaction_hash: transaction_hash.to_string(),
        error: iroha_torii_shared::ErrorEnvelope::new(
            "offline_operation_rejected",
            canonical_offline_rejection_message(message),
        ),
    }
}
fn terminal_rejected_or_expired_offline_operation_status(
    entry: &crate::PipelineStatusEntry,
    operation_id: [u8; 32],
    kind: KagemushaV2OperationKind,
    transaction_hash: &HashOf<SignedTransaction>,
) -> Option<OfflineOperationStatus> {
    matches!(
        entry.kind,
        crate::PipelineStatusKind::Rejected | crate::PipelineStatusKind::Expired
    )
    .then(|| {
        rejected_offline_operation_status(
            operation_id,
            kind,
            transaction_hash,
            kagemusha_v4_cached_rejection_detail(entry.rejection.as_ref()),
        )
    })
}
fn respond_with_offline_operation_status(status: OfflineOperationStatus) -> AxResponse {
    let pending = matches!(status, OfflineOperationStatus::Pending { .. });
    let mut response =
        crate::utils::respond_with_format(status, crate::utils::current_response_format());
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    if pending {
        response.headers_mut().insert(
            axum::http::header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("1"),
        );
    }
    response
}
fn offline_operation_status_response(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    record: &OfflineOperationRecord,
    committed: Option<&KagemushaV2CommittedFinality>,
    known_pending_in_queue: bool,
) -> Result<AxResponse, Error> {
    let operation_id = record.request.authorization().operation_id;
    if committed.is_none() {
        if let Some((committed_record, finality)) =
            find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
        {
            if terminal_operation_supersedes_pending_record(record, &committed_record, &finality)? {
                return offline_operation_status_response(
                    app,
                    issuer,
                    &committed_record,
                    Some(&finality),
                    false,
                );
            }
        }
    }
    let kind = record.request.kind();
    let operation_id_hex = hex::encode(operation_id);
    let applied = |finalized_block_height: u64| {
        if finalized_block_height == 0 {
            return Err(offline_operation_evidence_inconsistent(
                "An applied offline operation requires a committed height.",
            ));
        }
        let result = match kind {
            KagemushaV2OperationKind::TopUp => {
                let anchor = load_finalized_kagemusha_v4_anchor(app, operation_id)?;
                let OfflineOperationRequest::TopUp(request) = &record.request else {
                    unreachable!("the operation kind was derived from the same typed request")
                };
                ensure_kagemusha_v4_topup_anchor_matches_request(&anchor, request)?;
                ensure_kagemusha_v4_anchor_finality_binding(
                    anchor.topup_operation_id,
                    anchor.finalized_tx_hash,
                    anchor.finalized_height,
                    operation_id,
                    &record.transaction_hash,
                    finalized_block_height,
                )?;
                let finality_proof = load_finalized_kagemusha_v4_topup_proof(
                    app,
                    finalized_block_height,
                    operation_id,
                    &anchor,
                )?;
                OfflineOperationResult::TopUp(OfflineTopUpResult {
                    transaction_hash: record.transaction_hash.to_string(),
                    finalized_block_height,
                    anchor,
                    finality_proof,
                })
            }
            KagemushaV2OperationKind::Redeem => {
                OfflineOperationResult::Redeem(OfflineRedeemResult {
                    transaction_hash: record.transaction_hash.to_string(),
                    finalized_block_height,
                })
            }
        };
        Ok::<_, Error>(OfflineOperationStatus::Applied {
            operation_id: operation_id_hex.clone(),
            result,
        })
    };
    let rejected = |message: String| {
        rejected_offline_operation_status(operation_id, kind, &record.transaction_hash, message)
    };
    let pending = || -> Result<OfflineOperationStatus, Error> {
        if !known_pending_in_queue {
            let state = app.state.view();
            ensure_unproven_pending_window_is_live(
                kagemusha_v4_snapshot_time_ms(&state),
                record.request.authorization().expires_at_ms,
            )?;
        }
        Ok(pending_offline_operation_status(
            operation_id,
            kind,
            &record.transaction_hash,
            record.submitted_at_ms,
        ))
    };
    let status = if let Some(finality) = committed {
        ensure_kagemusha_v4_terminal_finality_matches_record(record, finality)?;
        match &finality.outcome {
            KagemushaV2TerminalOutcome::Applied => applied(finality.finalized_block_height)?,
            KagemushaV2TerminalOutcome::Rejected(message) => rejected(message.clone()),
        }
    } else if let Some((entry, _)) =
        crate::pipeline_status_local_entry(app, &record.transaction_hash)
    {
        if let Some(status) = terminal_rejected_or_expired_offline_operation_status(
            &entry,
            operation_id,
            kind,
            &record.transaction_hash,
        ) {
            status
        } else {
            match entry.kind {
                crate::PipelineStatusKind::Applied => {
                    let Some((committed_record, finality)) =
                        find_global_applied_offline_operation_by_id(
                            app,
                            &issuer.authority,
                            operation_id,
                        )?
                    else {
                        return Err(offline_operation_evidence_inconsistent(
                            "The applied offline operation is absent from global finality state.",
                        ));
                    };
                    ensure_same_offline_request(
                        &committed_record.request.as_ref(),
                        &record.request.as_ref(),
                    )?;
                    return offline_operation_status_response(
                        app,
                        issuer,
                        &committed_record,
                        Some(&finality),
                        false,
                    );
                }
                _ => pending_offline_operation_status(
                    operation_id,
                    kind,
                    &record.transaction_hash,
                    record.submitted_at_ms,
                ),
            }
        }
    } else if let Some((committed_record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        if terminal_operation_supersedes_pending_record(record, &committed_record, &finality)? {
            return offline_operation_status_response(
                app,
                issuer,
                &committed_record,
                Some(&finality),
                false,
            );
        }
        pending()?
    } else {
        // The typed Queue owner is authoritative pending provenance while an
        // older rejected attempt remains only a retry fallback.
        pending()?
    };
    Ok(respond_with_offline_operation_status(status))
}
fn admitted_offline_operation_status_response(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    admitted: &AdmittedOfflineOperationRecord,
) -> Result<AxResponse, Error> {
    let operation_id = admitted.binding.operation_id;
    if let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        if terminal_operation_supersedes_admitted_record(admitted, &record, &finality)? {
            return offline_operation_status_response(app, issuer, &record, Some(&finality), false);
        }
    }
    if let Some((entry, _)) = crate::pipeline_status_local_entry(app, &admitted.transaction_hash) {
        if let Some(status) = terminal_rejected_or_expired_offline_operation_status(
            &entry,
            operation_id,
            admitted.binding.kind,
            &admitted.transaction_hash,
        ) {
            return Ok(respond_with_offline_operation_status(status));
        }
        match entry.kind {
            crate::PipelineStatusKind::Applied => {
                let Some((record, finality)) = find_global_applied_offline_operation_by_id(
                    app,
                    &issuer.authority,
                    operation_id,
                )?
                else {
                    return Err(offline_operation_evidence_inconsistent(
                        "The applied offline operation is absent from global finality state.",
                    ));
                };
                if !terminal_operation_supersedes_admitted_record(admitted, &record, &finality)? {
                    return Err(offline_operation_evidence_inconsistent(
                        "The applied offline operation does not supersede its admitted request.",
                    ));
                }
                return offline_operation_status_response(
                    app,
                    issuer,
                    &record,
                    Some(&finality),
                    false,
                );
            }
            _ => {
                let status = pending_offline_operation_status(
                    operation_id,
                    admitted.binding.kind,
                    &admitted.transaction_hash,
                    admitted.binding.submitted_at_ms,
                );
                return Ok(respond_with_offline_operation_status(status));
            }
        }
    }
    let state = app.state.view();
    ensure_unproven_pending_window_is_live(
        kagemusha_v4_snapshot_time_ms(&state),
        admitted.binding.expires_at_ms,
    )?;
    let status = pending_offline_operation_status(
        operation_id,
        admitted.binding.kind,
        &admitted.transaction_hash,
        admitted.binding.submitted_at_ms,
    );
    Ok(respond_with_offline_operation_status(status))
}
pub(crate) fn handle_operation_status(
    app: &SharedAppState,
    operation_id: &str,
) -> Result<AxResponse, Error> {
    let operation_id = parse_operation_id(operation_id)?;
    // Polling an already-submitted operation must not depend on the authority
    // still being funded or retaining permission to sign a new command.  The
    // configured identity is sufficient to recover queue and committed outcome provenance.
    let issuer = require_configured_issuer(app)?;
    if let Some((record, finality)) =
        find_global_applied_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        return offline_operation_status_response(app, &issuer, &record, Some(&finality), false);
    }
    let admitted = {
        let mut admission = issuer.admission.lock();
        admission.prune_expired(now_ms());
        admission.get(&operation_id).cloned()
    };
    if let Some(admitted) = admitted {
        if let Some(resolution) =
            resolve_pending_offline_operation_by_id(app, &issuer.authority, operation_id)?
        {
            match resolution {
                PendingOfflineOperationResolution::Pending(pending) => {
                    ensure_admitted_operation_binding_matches_recovered_record(
                        &admitted, &pending,
                    )?;
                    return offline_operation_status_response(app, &issuer, &pending, None, true);
                }
                PendingOfflineOperationResolution::Terminal(record, finality) => {
                    if terminal_operation_supersedes_admitted_record(&admitted, &record, &finality)?
                    {
                        return offline_operation_status_response(
                            app,
                            &issuer,
                            &record,
                            Some(&finality),
                            false,
                        );
                    }
                }
            }
        }
        return admitted_offline_operation_status_response(app, &issuer, &admitted);
    }
    if let Some(resolution) =
        resolve_pending_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        return match resolution {
            PendingOfflineOperationResolution::Pending(record) => {
                offline_operation_status_response(app, &issuer, &record, None, true)
            }
            PendingOfflineOperationResolution::Terminal(record, finality) => {
                offline_operation_status_response(app, &issuer, &record, Some(&finality), false)
            }
        };
    }
    Err(Error::AppNotFound {
        code: "offline_operation_not_found",
        message: "Offline operation is unknown on this Torii node.".to_owned(),
    })
}
#[derive(Debug, Clone)]
struct KagemushaV2CommittedFinality {
    operation_id: [u8; 32],
    transaction_hash: String,
    finalized_block_height: u64,
    outcome: KagemushaV2TerminalOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum KagemushaV2TerminalOutcome {
    Applied,
    Rejected(String),
}
fn kagemusha_v4_applied_finality(
    operation_id: [u8; 32],
    transaction_hash: String,
    finalized_block_height: u64,
) -> KagemushaV2CommittedFinality {
    kagemusha_v4_committed_finality(operation_id, transaction_hash, finalized_block_height, None)
}
fn kagemusha_v4_committed_finality(
    operation_id: [u8; 32],
    transaction_hash: String,
    finalized_block_height: u64,
    rejection: Option<String>,
) -> KagemushaV2CommittedFinality {
    KagemushaV2CommittedFinality {
        operation_id,
        transaction_hash,
        finalized_block_height,
        outcome: rejection.map_or(KagemushaV2TerminalOutcome::Applied, |message| {
            KagemushaV2TerminalOutcome::Rejected(canonical_offline_rejection_message(message))
        }),
    }
}
fn kagemusha_v4_rejection_detail(rejection: Option<&TransactionRejectionReason>) -> String {
    canonical_offline_rejection_message(rejection.map_or_else(
        || "no rejection reason".to_owned(),
        |reason| crate::pipeline_rejection_summary(reason).to_owned(),
    ))
}
fn kagemusha_v4_cached_rejection_detail(rejection: Option<&&'static str>) -> String {
    canonical_offline_rejection_message(rejection.map_or_else(
        || "no rejection reason".to_owned(),
        |message| (*message).to_owned(),
    ))
}
fn canonical_offline_rejection_message(message: String) -> String {
    if crate::utils::is_valid_error_message(&message) {
        message
    } else {
        "The offline operation was rejected.".to_owned()
    }
}
fn offline_operation_evidence_inconsistent(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "offline_operation_evidence_inconsistent",
        message: message.into(),
    }
}
fn offline_operation_pending_unavailable(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "offline_operation_pending_unavailable",
        message: message.into(),
    }
}
fn ensure_unproven_pending_window_is_live(
    snapshot_time_ms: u64,
    expires_at_ms: u64,
) -> Result<(), Error> {
    if snapshot_time_ms < expires_at_ms {
        return Ok(());
    }
    Err(offline_operation_evidence_inconsistent(
        "The accepted offline operation expired without queue, pipeline, or canonical terminal provenance.",
    ))
}
fn ensure_kagemusha_v4_terminal_finality_matches_record(
    record: &OfflineOperationRecord,
    finality: &KagemushaV2CommittedFinality,
) -> Result<(), Error> {
    if finality.operation_id == [0; 32]
        || finality.operation_id != record.request.authorization().operation_id
        || finality.transaction_hash != record.transaction_hash.to_string()
        || finality.finalized_block_height == 0
    {
        return Err(offline_operation_evidence_inconsistent(
            "The terminal offline operation identity, transaction, or height is incomplete.",
        ));
    }
    Ok(())
}
fn kagemusha_v4_anchor_state_key(operation_id: [u8; 32]) -> Result<StatePath, Error> {
    if operation_id == [0; 32] {
        return Err(offline_operation_evidence_inconsistent(
            "A finalized top-up anchor requires a non-zero operation id.",
        ));
    }
    format!("kagemusha_v4_topup_anchor_{}", hex::encode(operation_id))
        .parse()
        .map_err(|err| {
            offline_operation_evidence_inconsistent(format!(
                "Failed to derive the finalized top-up anchor key: {err}"
            ))
        })
}
fn ensure_kagemusha_v4_topup_anchor_matches_request(
    anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
    request: &OfflineTopUpRequest,
) -> Result<(), Error> {
    anchor
        .validate_against_topup_request(request)
        .map_err(|err| {
            offline_operation_evidence_inconsistent(format!(
                "The finalized top-up anchor does not match the admitted signed request: {err}"
            ))
        })
}
fn ensure_kagemusha_v4_anchor_finality_binding(
    anchor_operation_id: [u8; 32],
    anchor_transaction_hash: [u8; 32],
    anchor_height: u64,
    operation_id: [u8; 32],
    transaction_hash: &HashOf<SignedTransaction>,
    finalized_block_height: u64,
) -> Result<(), Error> {
    if anchor_operation_id == [0; 32]
        || operation_id == [0; 32]
        || anchor_operation_id != operation_id
        || anchor_transaction_hash == [0; 32]
        || anchor_transaction_hash.as_slice() != transaction_hash.as_ref()
        || anchor_height != finalized_block_height
        || finalized_block_height == 0
    {
        return Err(offline_operation_evidence_inconsistent(
            "The top-up anchor operation, transaction, or height differs from terminal finality.",
        ));
    }
    Ok(())
}
fn load_finalized_kagemusha_v4_anchor(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<KagemushaRecursiveSpendTopUpAnchorV4, Error> {
    let key = kagemusha_v4_anchor_state_key(operation_id)?;
    let world = app.state.world_view();
    let archive = world.smart_contract_state().get(&key).ok_or_else(|| {
        offline_operation_evidence_inconsistent(
            "The finalized top-up anchor is missing from canonical chain state.",
        )
    })?;
    let anchor: KagemushaRecursiveSpendTopUpAnchorV4 =
        norito::decode_from_bytes(archive).map_err(|err| {
            offline_operation_evidence_inconsistent(format!(
                "The finalized top-up anchor is invalid: {err}"
            ))
        })?;
    anchor.validate_public_binding().map_err(|err| {
        offline_operation_evidence_inconsistent(format!(
            "The finalized top-up anchor failed validation: {err}"
        ))
    })?;
    let canonical = norito::to_bytes(&anchor).map_err(|err| {
        offline_operation_evidence_inconsistent(format!(
            "The finalized top-up anchor could not be canonically re-encoded: {err}"
        ))
    })?;
    if anchor.topup_operation_id != operation_id || canonical.as_slice() != archive.as_slice() {
        return Err(offline_operation_evidence_inconsistent(
            "The finalized top-up anchor has a mismatched operation id or non-canonical encoding.",
        ));
    }
    Ok(anchor)
}
fn load_finalized_kagemusha_v4_topup_proof(
    app: &SharedAppState,
    finalized_block_height: u64,
    operation_id: [u8; 32],
    anchor: &KagemushaRecursiveSpendTopUpAnchorV4,
) -> Result<OfflineTopUpFinalityProof, Error> {
    let proof = app
        .kura
        .kagemusha_topup_finality_proof_v2(finalized_block_height, operation_id)
        .map_err(|_| offline_topup_finality_proof_unavailable())?
        .ok_or_else(offline_topup_finality_proof_unavailable)?;
    let anchor_ref = anchor
        .compact_ref()
        .map_err(|_| offline_topup_finality_proof_unavailable())?;
    if proof.anchor != anchor_ref || proof.validate_anchor_inclusion().is_err() {
        return Err(offline_topup_finality_proof_unavailable());
    }
    Ok(proof)
}
fn offline_topup_finality_proof_unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "offline_topup_finality_proof_unavailable",
        message: "The finalized top-up proof is not available yet.".to_owned(),
    }
}
fn require_configured_issuer(app: &AppState) -> Result<Arc<OfflineCommandRuntime>, Error> {
    let issuer = app
        .offline_commands
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_service_unavailable",
            message: "Offline operation signing is not configured on this Torii node.".to_owned(),
        })?;
    Ok(issuer)
}

pub(crate) fn ensure_offline_command_authority_ready(
    app: &AppState,
    issuer: &OfflineCommandRuntime,
) -> Result<(), Error> {
    let state = app.state.view();
    let fee_asset_selector = app.state.nexus_snapshot().fees.fee_asset_id;
    ensure_offline_command_authority_ready_in_world(
        state.world(),
        issuer,
        &fee_asset_selector,
        kagemusha_v4_snapshot_time_ms(&state),
    )
}
pub(super) fn ensure_offline_command_authority_ready_in_world(
    world: &impl WorldReadOnly,
    issuer: &OfflineCommandRuntime,
    fee_asset_selector: &str,
    snapshot_time_ms: u64,
) -> Result<(), Error> {
    if world.account(&issuer.authority).is_err()
        || !iroha_core::smartcontracts::isi::offline::isi::world_has_offline_escrow_manager_permission(
            world,
            &issuer.authority,
        )
    {
        return Err(Error::AppServiceUnavailable {
            code: "offline_command_authority_not_ready",
            message: "Offline command authority is not registered with the exact CanManageOfflineEscrow permission."
                .to_owned(),
        });
    }
    let fee_asset_definition =
        routing::resolve_asset_definition_selector(world, fee_asset_selector, snapshot_time_ms)
            .map_err(|error| {
                iroha_logger::error!(
                    ?error,
                    %fee_asset_selector,
                    "offline command authority XOR fee asset could not be resolved"
                );
                Error::AppServiceUnavailable {
                    code: "offline_command_fee_asset_not_ready",
                    message: "Offline command authority XOR fee asset is not available.".to_owned(),
                }
            })?;
    let fee_asset = AssetId::new(fee_asset_definition, issuer.authority.clone());
    let balance = world
        .asset(&fee_asset)
        .map(|entry| entry.value().as_ref().clone())
        .unwrap_or_else(|_| Quantity::zero());
    if balance < issuer.minimum_xor_balance {
        return Err(Error::AppServiceUnavailable {
            code: "offline_command_authority_unfunded",
            message: "Offline command authority does not meet its configured minimum XOR balance."
                .to_owned(),
        });
    }
    Ok(())
}
fn offline_transaction_signing_error(
    context: &'static str,
    source: impl std::fmt::Display,
) -> Error {
    iroha_logger::error!(%context, error = %source, "offline operation signer failed");
    Error::Query(ValidationFail::InternalError(
        "Offline operation signer failed to sign the transaction.".to_owned(),
    ))
}
fn reject_x_iroha_auth_headers(headers: &HeaderMap) -> Result<(), Error> {
    for name in [
        app_auth::HEADER_ACCOUNT,
        app_auth::HEADER_SIGNATURE,
        app_auth::HEADER_TIMESTAMP_MS,
        app_auth::HEADER_NONCE,
        app_auth::HEADER_WITNESS,
    ] {
        if headers.contains_key(name) {
            return Err(Error::AppForbidden {
                code: "offline_auth_header_unsupported",
                message: "Offline commands authenticate through their signed request body; X-Iroha canonical auth headers are not accepted.".to_owned(),
            });
        }
    }
    Ok(())
}
/// Validate body-independent Offline command headers before payload decoding.
pub(crate) fn validate_command_headers_before_body(headers: &HeaderMap) -> Result<(), Error> {
    reject_x_iroha_auth_headers(headers)?;
    validated_idempotency_key(headers).map(|_| ())
}
fn now_ms() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn validation(code: &'static str, message: &'static str) -> Error {
    validation_owned(code, message.to_owned())
}
fn validation_owned(code: &'static str, message: String) -> Error {
    Error::AppQueryValidation { code, message }
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse as _;
    use iroha_config::{
        base::WithOrigin,
        kura::FsyncMode,
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura,
        },
    };
    use iroha_core::state::World;
    use iroha_core::{
        kagemusha_operation::{
            KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4,
            kagemusha_operation_finality_state_key_v4, kagemusha_operation_outcome_state_key_v4,
        },
        kura::Kura,
        tx::AcceptedTransaction,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, NetworkId, Registrable as _,
        account::Account,
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature,
            CertifiedMergeLedgerReference, SignedBlock,
            consensus::{
                CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
                LaneBlockQcV1,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        domain::DomainId,
        merge::{
            MergeExecutionBatch, MergeLaneBinding, MergeLaneExecution, MergeLaneSnapshot,
            MergeLedgerEntry, MergeQuorumCertificate,
        },
        nexus::{DataSpaceId, LaneId},
        offline::{
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2,
            KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2, KagemushaRecursiveSpendArtifactBindingV4,
            KagemushaRequestAuthorizationV2, KagemushaScaledAmountV2,
            KagemushaSpendableNoteDescriptorV2, KagemushaTopUpShieldEvidenceV2,
        },
        peer::PeerId,
        permission::{Permission, Permissions},
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::{ExecutionStep, signed::TransactionResultInner},
        trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
    };
    use iroha_primitives::const_vec::ConstVec;
    use std::{
        borrow::Cow,
        num::{NonZeroU64, NonZeroUsize},
        sync::Barrier,
        time::Duration,
    };
    use tempfile::TempDir;
    fn submission_test_issuer() -> Arc<OfflineCommandRuntime> {
        submission_test_issuer_with_limits(64, 64 * ADMITTED_OPERATION_ACCOUNTED_BYTES)
    }
    fn submission_test_issuer_with_limits(
        max_entries: usize,
        max_accounted_bytes: usize,
    ) -> Arc<OfflineCommandRuntime> {
        let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .expect("derive offline submission coordinator fixture key");
        Arc::new(OfflineCommandRuntime {
            authority: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            minimum_xor_balance: Quantity::from(25_u32),
            max_tx_value: Quantity::from(1_000_u32),
            admission: Arc::new(Mutex::new(OfflineOperationRegistry::new(
                NonZeroUsize::new(max_entries).expect("positive registry count"),
                NonZeroUsize::new(max_accounted_bytes).expect("positive registry byte budget"),
            ))),
        })
    }
    fn command_authority_readiness_world(
        issuer: &OfflineCommandRuntime,
        permission: Permission,
        xor_balance: Quantity,
    ) -> (World, String) {
        let fee_asset_definition_id: AssetDefinitionId =
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
                .parse()
                .expect("canonical XOR asset definition id");
        let account = Account::new(issuer.authority.clone()).build(&issuer.authority);
        let definition = AssetDefinition::numeric(
            fee_asset_definition_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&issuer.authority);
        let asset = Asset::new(
            AssetId::new(fee_asset_definition_id.clone(), issuer.authority.clone()),
            xor_balance,
        );
        let mut world = World::with_assets([], [account], [definition], [asset], []);
        let mut permissions = Permissions::new();
        permissions.insert(permission);
        world
            .account_permissions_mut_for_testing()
            .insert(issuer.authority.clone(), permissions);
        (world, fee_asset_definition_id.to_string())
    }
    fn assert_offline_readiness_code(error: Error, expected: &'static str) {
        match error {
            Error::AppServiceUnavailable { code, .. } => assert_eq!(code, expected),
            other => panic!("unexpected offline readiness error: {other:?}"),
        }
    }
    #[test]
    fn command_authority_readiness_requires_exact_permission_and_xor_floor() {
        let issuer = submission_test_issuer();
        let wrong_permission = Permission::new(
            "CanManageOfflineEscrow".to_owned(),
            iroha_primitives::json::Json::new("wildcard"),
        );
        let (mut world, fee_asset_selector) = command_authority_readiness_world(
            &issuer,
            wrong_permission,
            issuer.minimum_xor_balance.clone(),
        );
        let error = ensure_offline_command_authority_ready_in_world(
            &world.view(),
            &issuer,
            &fee_asset_selector,
            0,
        )
        .expect_err("same-name wildcard payload must not authorize offline commands");
        assert_offline_readiness_code(error, "offline_command_authority_not_ready");
        world.account_permissions_mut_for_testing().insert(
            issuer.authority.clone(),
            [iroha_core::smartcontracts::isi::offline::isi::offline_escrow_manager_permission()]
                .into_iter()
                .collect(),
        );
        ensure_offline_command_authority_ready_in_world(
            &world.view(),
            &issuer,
            &fee_asset_selector,
            0,
        )
        .expect("exact manager permission and configured XOR floor must be ready");
        let (underfunded_world, underfunded_fee_asset_selector) = command_authority_readiness_world(
            &issuer,
            iroha_core::smartcontracts::isi::offline::isi::offline_escrow_manager_permission(),
            Quantity::from(24_u32),
        );
        let error = ensure_offline_command_authority_ready_in_world(
            &underfunded_world.view(),
            &issuer,
            &underfunded_fee_asset_selector,
            0,
        )
        .expect_err("balance below the configured XOR floor must stay unavailable");
        assert_offline_readiness_code(error, "offline_command_authority_unfunded");
    }

    #[test]
    fn configured_issuer_lookup_remains_available_without_write_readiness() {
        let issuer = submission_test_issuer();
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("fresh test AppState must be uniquely owned")
            .offline_commands = Some(Arc::clone(&issuer));

        let configured = require_configured_issuer(&app)
            .expect("status lookup only requires the configured issuer identity");
        assert!(Arc::ptr_eq(&configured, &issuer));
        let readiness_error = ensure_offline_command_authority_ready(&app, &configured)
            .expect_err("fresh world has no command-authority account");
        assert_offline_readiness_code(readiness_error, "offline_command_authority_not_ready");
    }

    fn submission_test_request(operation_seed: u8) -> OfflineTopUpRequest {
        let key_pair = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .expect("derive offline submission request fixture key");
        let authority = AccountId::new(key_pair.public_key().clone());
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"offline-submission-coordinator-network",
            )));
        let domain_id = DomainId::try_new("offline", "universal").expect("fixture domain id");
        let definition = AssetDefinitionId::derive_from_components(
            domain_id,
            "coordinator".parse().expect("fixture asset name"),
        );
        let amount = KagemushaScaledAmountV2 {
            atomic_units: 7,
            scale: 0,
        };
        let operation_id = [operation_seed; 32];
        let issued_at_ms = now_ms().max(1);
        let mut request = OfflineTopUpRequest {
            version: 4,
            asset: AssetId::new(definition.clone(), authority.clone()),
            amount,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                network_id,
                asset: definition.clone(),
                note_commitment: [0x61; 32],
                spend_nullifier: [0x62; 32],
                amount,
            },
            shield_evidence: KagemushaTopUpShieldEvidenceV2 {
                initial_root: [0x65; 32],
                finalized_root: [0x66; 32],
                leaf_index: 0,
                proof: {
                    let backend = "halo2/ipa";
                    let mut attachment = ProofAttachment::new_ref(
                        backend.into(),
                        ProofBox::new(backend.to_owned(), vec![0x67]),
                        VerifyingKeyId::new(backend, "kagemusha-topup-shield-v2"),
                    );
                    attachment.vk_commitment = Some([0x68; 32]);
                    attachment
                },
            },
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                version: 4,
                generation: "submission-coordinator-fixture".to_owned(),
                manifest_sha256: [0x69; 32],
            },
            operation_id,
            authorization: KagemushaRequestAuthorizationV2 {
                authority,
                device_id: "submission-coordinator-device".to_owned(),
                asset_definition_id: definition,
                operation_id,
                issued_at_ms,
                expires_at_ms: issued_at_ms
                    .saturating_add(KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2),
                nonce: [0x63; 32],
                payload_digest: [0x64; 32],
                registration_hash: Hash::new([0x6A; 32]).into(),
                hardware_assertion:
                    iroha_data_model::offline::KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                        iroha_data_model::offline::KagemushaAndroidKeyMintHardwareAssertionV1 {
                            signature: iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                                &[1_u8; 64],
                            )
                            .expect("fixture hardware signature"),
                        },
                    ),
            },
        };
        let payload_digest = request
            .unsigned_payload_digest()
            .expect("derive exact offline top-up payload digest");
        request.authorization.payload_digest = payload_digest;
        let signing_bytes = request
            .authorization
            .signing_bytes()
            .expect("encode exact offline authorization signing bytes");
        use p256::{ecdsa::signature::Signer as _, elliptic_curve::sec1::ToEncodedPoint as _};
        let hardware_key =
            p256::ecdsa::SigningKey::from_slice(&[1_u8; 32]).expect("fixed P-256 fixture key");
        let hardware_signature: p256::ecdsa::Signature = hardware_key.sign(&signing_bytes);
        let hardware_signature = hardware_signature
            .normalize_s()
            .unwrap_or(hardware_signature);
        request.authorization.set_hardware_signature(
            iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                hardware_signature.to_bytes().as_slice(),
            )
            .expect("canonical hardware fixture signature"),
        );
        request
            .authorization
            .verify_hardware_signature(
                hardware_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes(),
            )
            .expect("offline hardware fixture signature must bind the exact typed fields");
        request
    }

    fn finalized_topup_anchor_for_request(
        request: &OfflineTopUpRequest,
    ) -> KagemushaRecursiveSpendTopUpAnchorV4 {
        KagemushaRecursiveSpendTopUpAnchorV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            network_id: request.current_note.network_id,
            payer: request.authorization.authority.clone(),
            asset: request.asset.clone(),
            asset_scale: request.amount.scale,
            amount: request.amount,
            initial_root: request.shield_evidence.initial_root,
            finalized_root: request.shield_evidence.finalized_root,
            shield_leaf_index: request.shield_evidence.leaf_index,
            current_note: request.current_note.clone(),
            topup_operation_id: request.operation_id,
            shield_verifier_id: request.shield_evidence.proof.vk_ref.clone(),
            shield_verifier_commitment: request
                .shield_evidence
                .proof
                .vk_commitment
                .expect("fixture shield verifier commitment"),
            artifact_binding: request.artifact_binding.clone(),
            finalized_height: 42,
            finalized_tx_hash: Hash::new(b"offline-topup-anchor-transaction").into(),
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("finalize top-up anchor fixture")
    }
    #[test]
    fn topup_snapshot_uses_checked_incremental_tree_admission() {
        let source = include_str!("offline_commands.rs");
        let start = source
            .find("fn validate_kagemusha_v4_topup_snapshot(")
            .expect("top-up snapshot validator");
        let end = source[start..]
            .find("fn validate_kagemusha_v4_redeem_snapshot(")
            .map(|offset| start + offset)
            .expect("redemption snapshot validator");
        let topup = &source[start..end];
        assert!(topup.contains(".preview_commitment_root("));
        assert!(topup.contains("KAGEMUSHA_TOPUP_SHIELD_INSERTION_CAPACITY_V2"));
        assert!(!topup.contains("tree_profile.capacity()"));
        assert!(!topup.contains("commitments.clone()"));
        assert!(!topup.contains(".compute_root("));
        for namespace_check in [
            ".nullifiers\n            .contains(&request.current_note.note_commitment)",
            ".commitments\n            .contains(&request.current_note.spend_nullifier)",
        ] {
            assert!(
                topup.contains(namespace_check),
                "top-up snapshot must reject commitment/nullifier cross-namespace collisions"
            );
        }
    }
    #[test]
    fn topup_admission_rejects_same_label_foreign_genesis_before_state_work() {
        let first_display_label = ChainId::from("shared-offline-display-label");
        let second_display_label = ChainId::from("shared-offline-display-label");
        assert_eq!(first_display_label, second_display_label);
        let app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let mut request = submission_test_request(0x7B);
        let foreign_network =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"same-label-foreign-torii-offline-genesis",
            )));
        assert_ne!(foreign_network, *app.state.network_id_ref());
        request.current_note.network_id = foreign_network;
        let error = validate_kagemusha_v4_topup_snapshot(&app, &request)
            .expect_err("same-label foreign genesis must be rejected before asset lookup");
        assert!(matches!(
            error,
            Error::AppQueryValidation {
                code: "offline_wrong_network",
                ..
            }
        ));
    }
    fn claim_test_leader(
        issuer: &Arc<OfflineCommandRuntime>,
        request: &OfflineTopUpRequest,
    ) -> SubmissionLeader {
        match issuer
            .claim_submission(submission_test_binding(request))
            .expect("claim fixture submission")
        {
            SubmissionClaim::Leader(leader) => leader,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Follower(_) => {
                panic!("fresh fixture request must elect one leader")
            }
        }
    }
    fn submission_test_binding(request: &OfflineTopUpRequest) -> OfflineOperationRequestBinding {
        OfflineOperationRequestBinding::from_request(OfflineOperationRequest::TopUp(request))
            .expect("canonical submission fixture binding")
    }
    fn refresh_submission_test_authorization(request: &mut OfflineTopUpRequest) {
        let payload_digest = request
            .unsigned_payload_digest()
            .expect("derive refreshed offline top-up payload digest");
        request.authorization.payload_digest = payload_digest;
        let signing_bytes = request
            .authorization
            .signing_bytes()
            .expect("encode refreshed offline authorization signing bytes");
        use p256::{ecdsa::signature::Signer as _, elliptic_curve::sec1::ToEncodedPoint as _};
        let hardware_key =
            p256::ecdsa::SigningKey::from_slice(&[1_u8; 32]).expect("fixed P-256 fixture key");
        let hardware_signature: p256::ecdsa::Signature = hardware_key.sign(&signing_bytes);
        let hardware_signature = hardware_signature
            .normalize_s()
            .unwrap_or(hardware_signature);
        request.authorization.set_hardware_signature(
            iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                hardware_signature.to_bytes().as_slice(),
            )
            .expect("canonical refreshed hardware fixture signature"),
        );
        request
            .authorization
            .verify_hardware_signature(
                hardware_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes(),
            )
            .expect("refreshed fixture signature must bind the exact typed fields");
        request
            .validate_public_binding()
            .expect("refreshed fixture request must remain valid");
    }
    fn submission_test_hash(seed: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32]))
    }
    fn admitted_record_fixture(
        operation_seed: u8,
        submitted_at_ms: u64,
        expires_at_ms: u64,
    ) -> AdmittedOfflineOperationRecord {
        AdmittedOfflineOperationRecord {
            binding: OfflineOperationRequestBinding {
                operation_id: [operation_seed; 32],
                kind: KagemushaV2OperationKind::TopUp,
                canonical_request_digest: [operation_seed.wrapping_add(1); 32],
                submitted_at_ms,
                expires_at_ms,
            },
            transaction_hash: submission_test_hash(operation_seed),
        }
    }
    fn submission_test_transaction_signed_by_with_nonce(
        requests: Vec<OfflineTopUpRequest>,
        signer: &KeyPair,
        nonce: Option<NonZeroU32>,
    ) -> SignedTransaction {
        let instructions = requests
            .into_iter()
            .map(TopUpKagemushaRecursiveV4::new)
            .map(InstructionBox::from)
            .collect::<Vec<_>>();
        let mut transaction = TransactionBuilder::new(
            crate::signed_query_test_network_id(),
            AccountId::new(signer.public_key().clone()).into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        if let Some(nonce) = nonce {
            transaction.set_nonce(nonce);
        }
        let transaction = transaction.sign(signer.private_key());
        transaction
            .verify_signature()
            .expect("offline history fixture transaction must carry an exact valid signature");
        transaction
    }
    fn submission_test_transaction_signed_by(
        requests: Vec<OfflineTopUpRequest>,
        signer: &KeyPair,
    ) -> SignedTransaction {
        submission_test_transaction_signed_by_with_nonce(requests, signer, None)
    }
    fn submission_test_transaction(requests: Vec<OfflineTopUpRequest>) -> SignedTransaction {
        let issuer = submission_test_issuer();
        submission_test_transaction_signed_by(requests, &issuer.key_pair)
    }
    fn history_block_signer() -> KeyPair {
        KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519)
            .expect("derive offline history block fixture key")
    }
    fn signed_history_block(
        height: u64,
        prev_block_hash: Option<HashOf<BlockHeader>>,
        creation_time_ms: u64,
        transactions: Vec<SignedTransaction>,
        results: Vec<TransactionResultInner>,
    ) -> SignedBlock {
        signed_history_block_with_time_triggers(
            height,
            prev_block_hash,
            creation_time_ms,
            transactions,
            Vec::new(),
            results,
        )
    }
    fn signed_history_block_with_time_triggers(
        height: u64,
        prev_block_hash: Option<HashOf<BlockHeader>>,
        creation_time_ms: u64,
        transactions: Vec<SignedTransaction>,
        time_triggers: Vec<TimeTriggerEntrypoint>,
        results: Vec<TransactionResultInner>,
    ) -> SignedBlock {
        let entrypoint_hashes = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .chain(
                time_triggers
                    .iter()
                    .map(TimeTriggerEntrypoint::hash_as_entrypoint),
            )
            .collect::<Vec<_>>();
        assert_eq!(
            entrypoint_hashes.len(),
            results.len(),
            "every history entrypoint needs one deterministic result"
        );
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("offline history block height is non-zero"),
            prev_block_hash,
            None,
            None,
            creation_time_ms,
            0,
        );
        let signer = history_block_signer();
        let signature = SignatureOf::try_from_hash(signer.private_key(), header.hash())
            .expect("sign offline history block header");
        let mut block =
            SignedBlock::presigned(BlockSignature::new(0, signature), header, transactions);
        block
            .set_transaction_results(time_triggers, &entrypoint_hashes, results)
            .expect("offline history block results must align with its signed entrypoints");
        let final_signature =
            SignatureOf::try_from_hash(signer.private_key(), block.header().hash())
                .expect("sign finalized offline history block header");
        block
            .replace_signatures(
                [BlockSignature::new(0, final_signature)]
                    .into_iter()
                    .collect(),
            )
            .expect("replace provisional history block signature with finalized signature");
        block
    }
    fn persistent_kura_config(directory: &TempDir) -> KuraConfig {
        KuraConfig {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: WithOrigin::inline(
                directory
                    .path()
                    .to_str()
                    .expect("temporary Kura path is UTF-8")
                    .into(),
            ),
            max_disk_usage_bytes: kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("one retained block is non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: kura::FSYNC_INTERVAL,
            lane_history_retention: kura::LANE_HISTORY_RETENTION,
            replica_advert: kura::REPLICA_ADVERT_POLICY,
        }
    }
    fn terminal_outcome_fixture(
        transaction: &SignedTransaction,
        result: &TransactionResult,
        carrier_height: u64,
        execution_phase: KagemushaOperationExecutionPhaseV4,
        phase_index: u64,
    ) -> KagemushaOperationOutcomeRecordV4 {
        let carrier = classify_kagemusha_operation_transaction_v4(transaction)
            .expect("classify terminal outcome fixture")
            .expect("terminal outcome fixture is a Kagemusha carrier");
        KagemushaOperationOutcomeRecordV4 {
            version: KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4,
            operation_id: carrier.operation_id(),
            kind: carrier.kind(),
            request_authority_digest: kagemusha_operation_authority_digest_v4(
                &carrier.request().authorization().authority,
            )
            .expect("hash terminal fixture request authority"),
            outer_authority_digest: kagemusha_operation_authority_digest_v4(
                transaction.authority(),
            )
            .expect("hash terminal fixture outer authority"),
            canonical_request_digest: carrier.canonical_request_digest(),
            signed_transaction_wire_hash: signed_transaction_wire_hash_v4(transaction)
                .expect("hash terminal fixture signed wire"),
            entrypoint_hash: transaction.hash_as_entrypoint(),
            carrier_height,
            execution_phase,
            phase_index,
            result_hash: Some(result.hash()),
            outcome: if result.is_ok() {
                KagemushaOperationOutcomeStateV4::Applied
            } else {
                KagemushaOperationOutcomeStateV4::Rejected
            },
        }
    }
    fn app_with_offline_histories(
        kura: Arc<Kura>,
        issuer: Arc<OfflineCommandRuntime>,
        outcomes: impl IntoIterator<Item = &KagemushaOperationOutcomeRecordV4>,
    ) -> SharedAppState {
        let mut world = World::default();
        for outcome in outcomes {
            let key = match outcome.outcome {
                KagemushaOperationOutcomeStateV4::Applied => {
                    kagemusha_operation_finality_state_key_v4(outcome.operation_id)
                        .expect("derive global finality fixture state key")
                }
                KagemushaOperationOutcomeStateV4::Rejected => {
                    kagemusha_operation_outcome_state_key_from_authority_digest_v4(
                        outcome.outer_authority_digest,
                        outcome.operation_id,
                    )
                    .expect("derive rejected-attempt fixture state key")
                }
                KagemushaOperationOutcomeStateV4::Pending => {
                    panic!("a pending outcome must not survive block finalization")
                }
            };
            let payload = norito::encode_canonical(outcome)
                .expect("encode canonical terminal outcome fixture");
            world
                .smart_contract_state_mut_for_testing()
                .insert(key, payload);
        }
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(world);
        let inner = Arc::get_mut(&mut app).expect("fresh test AppState must be uniquely owned");
        inner.kura = kura;
        inner.offline_commands = Some(issuer);
        app
    }
    fn app_with_offline_history(
        kura: Arc<Kura>,
        issuer: Arc<OfflineCommandRuntime>,
        outcome: Option<&KagemushaOperationOutcomeRecordV4>,
    ) -> SharedAppState {
        app_with_offline_histories(kura, issuer, outcome)
    }
    fn assert_offline_history_error(error: Error, expected_code: &'static str) {
        match &error {
            Error::AppServiceUnavailable { code, .. } => assert_eq!(*code, expected_code),
            other => panic!("offline history returned the wrong error class: {other:?}"),
        }
        assert_eq!(
            error.into_response().status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }
    async fn decode_offline_operation_status(response: AxResponse) -> OfflineOperationStatus {
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect offline status response body");
        norito::decode_from_bytes(&bytes).expect("decode typed Norito offline status response")
    }
    async fn decode_offline_operation_reference(response: AxResponse) -> OfflineOperationReference {
        assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect offline operation reference body");
        norito::decode_from_bytes(&bytes).expect("decode typed Norito offline operation reference")
    }
    fn merge_history_settlement(lane_incarnation: Hash) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::SINGLE,
            lane_incarnation,
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 1,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        }
    }
    fn committed_merge_history_entry(
        transaction: SignedTransaction,
        result: TransactionResult,
        carrier_header: &BlockHeader,
    ) -> MergeLedgerEntry {
        let entrypoint = TransactionEntrypoint::External(transaction);
        let entrypoint_hashes = vec![Hash::from(entrypoint.hash())];
        let result_hashes = vec![Hash::from(result.hash())];
        let validator_set = Vec::<PeerId>::new();
        let lane_incarnation = Hash::new(b"offline-status-merge-lane-incarnation");
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation,
            proposal_height: carrier_header.height().get(),
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"offline-status-merge-subject"),
            payload_ownership_hash: Hash::new(b"offline-status-merge-ownership"),
            rbc_instance_hash: Hash::new(b"offline-status-merge-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: entrypoint_hashes.clone(),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 0,
            min_quorum: 0,
            qc_mode_tag: "offline-status-merge-fixture".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let lane_qc = |phase| LaneBlockQcV1 {
            body: proposal.vote_body(phase),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            signers_bitmap: Vec::new(),
            bls_aggregate_signature: Vec::new(),
            payload_availability_qc: None,
        };
        let prepare_qc = lane_qc(CertPhase::Prepare);
        let commit_qc = lane_qc(CertPhase::Commit);
        let settlement_commitment = merge_history_settlement(lane_incarnation);
        let settlement_hash =
            iroha_data_model::nexus::compute_settlement_hash(&settlement_commitment)
                .expect("hash offline merge settlement fixture");
        let autonomous_network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"offline-status-merge-genesis",
            )));
        let execution = MergeLaneExecution {
            source_bundle: vec![0xA5],
            source_bundle_hash: Hash::new(b"offline-status-merge-source"),
            proposal: proposal.clone(),
            origin_proposal: proposal,
            prepare_qc,
            commit_qc,
            signer_proofs: Vec::new(),
            autonomous_network_id,
            autonomous_epoch: 1,
            autonomous_payload_hash: Hash::new(b"offline-status-merge-payload"),
            entrypoint_hashes,
            authenticated_signed_replay_aliases: vec![None],
            entrypoints: vec![entrypoint],
            reservation_keys: vec![vec![0x01]],
            routing_plans: vec![vec![0x02]],
            native_amx_receipts: vec![None],
            result_hashes,
            results: vec![result],
            settlement_commitment: settlement_commitment.clone(),
            settlement_hash,
            fastpq_transcripts: Vec::new().into(),
        };
        let lanes = vec![execution];
        let entrypoint_merkle_root =
            iroha_core::merge::merge_execution_entrypoint_merkle_root(&lanes)
                .expect("offline merge fixture has one entrypoint");
        let result_merkle_root = iroha_core::merge::merge_execution_result_merkle_root(&lanes)
            .expect("offline merge fixture has one result");
        let base_state_hash = carrier_header
            .prev_block_hash()
            .expect("merge carrier has a canonical parent");
        let write_set_root = Hash::new(b"offline-status-merge-write-set");
        let mut execution_batch = MergeExecutionBatch {
            version: 1,
            base_state_height: carrier_header.height().get().saturating_sub(1),
            base_state_hash,
            application_block_header: carrier_header.clone(),
            execution_root: iroha_core::merge::merge_execution_root(&lanes),
            lanes,
            entrypoint_count: 1,
            entrypoint_merkle_root,
            result_merkle_root,
            application_write_set_root: Hash::new(b"offline-status-merge-application-write-set"),
            write_set_root,
            expected_post_state_hash: iroha_core::merge::merge_expected_post_state_hash(
                carrier_header.height().get().saturating_sub(1),
                base_state_hash,
                write_set_root,
            ),
            batch_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        execution_batch.batch_hash =
            iroha_core::merge::merge_execution_batch_hash(&execution_batch);
        let merge_hint_root = Hash::new(b"offline-status-merge-hint");
        let lane_snapshot = MergeLaneSnapshot {
            lane_id: LaneId::SINGLE,
            lane_incarnation,
            incarnation_activation_height: 1,
            proposal_height: carrier_header.height().get(),
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_block_height: 1,
            tip_hash: HashOf::from_untyped_unchecked(Hash::new(b"offline-status-merge-tip")),
            merge_hint_root,
            settlement_commitment,
            settlement_hash,
            relay_envelope: None,
        };
        MergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: 1,
            lane_catalog_hash: Hash::new(b"offline-status-merge-catalog"),
            active_lanes: vec![MergeLaneBinding {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
                lane_config_hash: Hash::new(b"offline-status-merge-lane-config"),
                incarnation: lane_incarnation,
                activation_height: 1,
            }],
            incarnation_root: Hash::new(b"offline-status-merge-incarnation-root"),
            activation_root: Hash::new(b"offline-status-merge-activation-root"),
            lane_snapshots: vec![lane_snapshot],
            global_state_root: iroha_core::merge::reduce_merge_hint_roots(&[merge_hint_root]),
            merge_qc: MergeQuorumCertificate::new(
                carrier_header.view_change_index(),
                1,
                carrier_header.height().get(),
                base_state_hash,
                autonomous_network_id,
                VALIDATOR_SET_HASH_VERSION_V1,
                HashOf::new(&validator_set),
                validator_set.clone(),
                Vec::new(),
                Vec::new(),
                vec![0xAA],
                Hash::new(b"offline-status-merge-qc-message"),
            ),
            execution_batch: Some(execution_batch),
            lane_drain_certificates: Vec::new(),
        }
    }
    fn attach_committed_merge_reference(
        mut carrier: SignedBlock,
        entry: &MergeLedgerEntry,
    ) -> SignedBlock {
        let context = BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(entry));
        carrier.set_execution_context(Some(context));
        carrier
    }
    async fn retry_outcome(receiver: watch::Receiver<SubmissionOutcome>) {
        let outcome = tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_submission_outcome(receiver),
        )
        .await
        .expect("submission follower must be released promptly");
        assert!(matches!(outcome, SubmissionOutcome::Retry));
    }
    async fn response_json(response: AxResponse) -> norito::json::Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect offline response body");
        norito::json::from_slice(&body).expect("decode offline JSON response")
    }
    #[test]
    fn transaction_recovery_requires_one_valid_direct_external_operation() {
        let issuer = submission_test_issuer();
        let first = submission_test_request(0x15);
        let second = submission_test_request(0x16);
        let mixed_transaction = submission_test_transaction(vec![first, second.clone()]);
        assert!(
            offline_operation_record_in_transaction(
                &mixed_transaction,
                &issuer.authority,
                second.authorization.operation_id,
            )
            .is_none(),
            "multiple native instructions are not a canonical operation carrier"
        );
        let transaction = submission_test_transaction(vec![second.clone()]);
        let recovered = offline_operation_record_in_transaction(
            &transaction,
            &issuer.authority,
            second.authorization.operation_id,
        )
        .expect("the exact singleton operation must be recovered");
        assert_eq!(
            recovered.request,
            OfflineOperationRequest::TopUp(&second).into_owned()
        );
        assert_eq!(recovered.transaction_hash, transaction.hash());
        assert_eq!(recovered.submitted_at_ms, second.authorization.issued_at_ms);
        let batch_transaction = TransactionBuilder::new(
            crate::signed_query_test_network_id(),
            issuer.authority.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(iroha_data_model::transaction::Executable::Batch(
            vec![
                iroha_data_model::transaction::ExecutableBatchItem::Instruction(
                    InstructionBox::from(TopUpKagemushaRecursiveV4::new(second.clone())),
                ),
            ]
            .into(),
        ))
        .sign(issuer.key_pair.private_key());
        assert!(
            offline_operation_record_in_transaction(
                &batch_transaction,
                &issuer.authority,
                second.authorization.operation_id,
            )
            .is_none(),
            "a singleton operation wrapped in an executable batch is not canonical"
        );
        assert!(
            offline_operation_record_in_transaction(&transaction, &issuer.authority, [0x17; 32],)
                .is_none(),
            "an attacker-controlled miss must not recover an unrelated instruction"
        );
        assert!(
            offline_operation_record_in_transaction(&transaction, &issuer.authority, [0; 32])
                .is_none(),
            "zero is never a valid operation identity"
        );
        let mut mismatched = submission_test_request(0x18);
        let authorized_id = mismatched.authorization.operation_id;
        mismatched.operation_id = [0x19; 32];
        let malformed_transaction = submission_test_transaction(vec![mismatched.clone()]);
        assert!(
            offline_operation_record_in_transaction(
                &malformed_transaction,
                &issuer.authority,
                authorized_id,
            )
            .is_none(),
            "a request whose duplicated operation ids disagree is not a carrier"
        );
        assert!(
            offline_operation_record_in_transaction(
                &malformed_transaction,
                &issuer.authority,
                mismatched.operation_id,
            )
            .is_none(),
            "a malformed top-level id must not create another lookup identity"
        );
        let sealed = TransactionEntrypoint::SealedReveal(
            iroha_data_model::transaction::signed::SealedTransactionReveal::new(
                Hash::new(b"sealed-offline-operation-test"),
                transaction.clone(),
                [0xA5; 32],
            ),
        );
        assert!(
            offline_operation_record_in_entrypoint(
                &sealed,
                &issuer.authority,
                second.authorization.operation_id,
            )
            .is_none(),
            "sealed reveal is not an alternate operation carrier"
        );
        let unrelated = TransactionBuilder::new(
            crate::signed_query_test_network_id(),
            issuer.authority.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "unrelated".to_owned(),
        )])
        .sign(issuer.key_pair.private_key());
        assert!(
            offline_operation_record_in_transaction(&unrelated, &issuer.authority, authorized_id,)
                .is_none(),
            "ordinary transactions must never enter offline recovery"
        );
    }
    #[test]
    fn pending_recovery_uses_the_exact_typed_queue_owner() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x74);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let account = Account::new(issuer.authority.clone()).build(&issuer.authority);
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
            World::with([], [account], []),
        );
        Arc::get_mut(&mut app)
            .expect("fresh pending-operation AppState must be uniquely owned")
            .offline_commands = Some(Arc::clone(&issuer));
        app.queue
            .push(
                AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone())),
                app.state.view(),
            )
            .expect("canonical singleton Kagemusha carrier must enter the Queue");

        let recovered =
            match resolve_pending_offline_operation_by_id(&app, &issuer.authority, operation_id)
                .expect("typed Queue ownership must remain coherent")
                .expect("the exact pending operation must be indexed")
            {
                PendingOfflineOperationResolution::Pending(record) => record,
                PendingOfflineOperationResolution::Terminal(_, _) => {
                    panic!("a fresh Queue owner cannot already be terminal")
                }
            };
        assert_eq!(
            recovered.request,
            OfflineOperationRequest::TopUp(&request).into_owned()
        );
        assert_eq!(
            recovered.canonical_request_digest,
            KagemushaOperationRequestV4::TopUp(&request)
                .canonical_request_digest()
                .expect("derive canonical pending request digest")
        );
        assert_eq!(recovered.transaction_hash, transaction.hash());
        assert!(
            resolve_pending_offline_operation_by_id(&app, &issuer.authority, [0x75; 32])
                .expect("an absent exact Queue key remains a coherent lookup")
                .is_none(),
            "a different operation id must not be found by scanning unrelated transactions"
        );
    }
    #[tokio::test]
    async fn newer_queue_attempt_supersedes_stale_admitted_transaction_hash() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x75);
        let operation_id = request.authorization.operation_id;
        let stale = submission_test_transaction_signed_by_with_nonce(
            vec![request.clone()],
            &issuer.key_pair,
            None,
        );
        let retry_nonce = NonZeroU32::new(1).expect("one is non-zero");
        let retry = submission_test_transaction_signed_by_with_nonce(
            vec![request.clone()],
            &issuer.key_pair,
            Some(retry_nonce),
        );
        assert_ne!(stale.hash(), retry.hash());

        let account = Account::new(issuer.authority.clone()).build(&issuer.authority);
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(
            World::with([], [account], []),
        );
        Arc::get_mut(&mut app)
            .expect("fresh pending-operation AppState must be uniquely owned")
            .offline_commands = Some(Arc::clone(&issuer));
        issuer
            .admission
            .lock()
            .insert_reserved(AdmittedOfflineOperationRecord {
                binding: submission_test_binding(&request),
                transaction_hash: stale.hash(),
            });
        app.queue
            .push(
                AcceptedTransaction::new_unchecked(Cow::Owned(retry.clone())),
                app.state.view(),
            )
            .expect("the exact retry carrier must enter the Queue");

        let status = handle_operation_status(&app, &hex::encode(operation_id))
            .expect("authoritative Queue retry must supersede stale process-local admission");
        assert!(matches!(
            decode_offline_operation_status(status).await,
            OfflineOperationStatus::Pending {
                transaction_hash,
                submitted_at_ms,
                ..
            } if transaction_hash == retry.hash().to_string()
                && submitted_at_ms == request.authorization.issued_at_ms
        ));
    }
    #[test]
    fn pending_queue_lookup_errors_preserve_unavailable_and_inconsistent_classes() {
        let entrypoint_hash =
            submission_test_transaction(vec![submission_test_request(0x76)]).hash_as_entrypoint();
        for error in [
            PendingKagemushaOperationLookupError::Unavailable {
                reason: "startup recovery".to_owned(),
            },
            PendingKagemushaOperationLookupError::DurabilityTransition { entrypoint_hash },
        ] {
            assert!(matches!(
                pending_offline_operation_lookup_error(error),
                Error::AppServiceUnavailable {
                    code: "offline_operation_pending_unavailable",
                    ..
                }
            ));
        }
        assert!(matches!(
            pending_offline_operation_lookup_error(
                PendingKagemushaOperationLookupError::Inconsistent {
                    reason: "lost reverse owner".to_owned(),
                }
            ),
            Error::AppServiceUnavailable {
                code: "offline_operation_evidence_inconsistent",
                ..
            }
        ));
    }
    #[test]
    fn proved_overlay_kagemusha_operation_is_rejected_as_noncanonical() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x1E);
        let operation_id = request.authorization.operation_id;
        let transaction = TransactionBuilder::new(
            crate::signed_query_test_network_id(),
            issuer.authority.clone().into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(iroha_data_model::transaction::Executable::IvmProved(
            iroha_data_model::transaction::IvmProved {
                bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![0x01]),
                overlay: vec![InstructionBox::from(TopUpKagemushaRecursiveV4::new(
                    request,
                ))]
                .into(),
                events_commitment: Hash::new(b"Kagemusha overlay events"),
                gas_policy_commitment: Hash::new(b"Kagemusha overlay gas policy"),
            },
        ))
        .sign(issuer.key_pair.private_key());
        assert!(matches!(
            classify_kagemusha_operation_transaction_v4(&transaction),
            Err(
                iroha_data_model::offline::KagemushaOperationCarrierErrorV4::NonCanonicalExecutable
            )
        ));
        assert!(
            offline_operation_record_in_transaction(&transaction, &issuer.authority, operation_id,)
                .is_none(),
            "a proved overlay must not create Torii recovery provenance"
        );
    }
    #[test]
    fn unauthorized_outer_authority_cannot_poison_offline_recovery() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x1C);
        let operation_id = request.authorization.operation_id;
        let front_runner = KeyPair::try_from_seed(vec![0x1D; 32], Algorithm::Ed25519)
            .expect("derive unauthorized offline front-run fixture key");
        let front_runner_transaction = TransactionBuilder::new(
            crate::signed_query_test_network_id(),
            AccountId::new(front_runner.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(TopUpKagemushaRecursiveV4::new(
            request.clone(),
        ))])
        .sign(front_runner.private_key());
        let rejected_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "outer authority is not the configured Kagemusha submission authority".to_owned(),
            ),
        )));
        let front_runner_entrypoint =
            TransactionEntrypoint::External(front_runner_transaction.clone());
        assert!(
            offline_operation_record_in_transaction(
                &front_runner_transaction,
                &issuer.authority,
                operation_id,
            )
            .is_none(),
            "an observed signed request wrapped by another outer authority is not an admitted operation"
        );
        assert!(
            terminal_offline_operation_in_entrypoint(
                &front_runner_entrypoint,
                &rejected_result,
                &issuer.authority,
                operation_id,
                1,
            )
            .is_none(),
            "a rejected front-run must not become a terminal idempotency record"
        );
        let issuer_transaction = submission_test_transaction(vec![request]);
        assert!(
            offline_operation_record_in_transaction(
                &issuer_transaction,
                &issuer.authority,
                operation_id,
            )
            .is_some(),
            "the same signed request remains recoverable under the configured issuer authority"
        );
    }
    #[test]
    fn terminal_recovery_binds_the_exact_operation_and_preserves_both_outcomes() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x1A);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        let applied_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let (applied_record, applied) = terminal_offline_operation_in_entrypoint(
            &entrypoint,
            &applied_result,
            &issuer.authority,
            operation_id,
            17,
        )
        .expect("matching applied operation must be reconstructed");
        assert_eq!(applied_record.transaction_hash, transaction.hash());
        assert_eq!(applied.operation_id, operation_id);
        assert_eq!(applied.transaction_hash, transaction.hash().to_string());
        assert_eq!(applied.finalized_block_height, 17);
        assert_eq!(applied.outcome, KagemushaV2TerminalOutcome::Applied);
        assert!(
            terminal_offline_operation_in_entrypoint(
                &entrypoint,
                &applied_result,
                &issuer.authority,
                [0x1B; 32],
                17,
            )
            .is_none(),
            "a transaction containing another operation must not satisfy the lookup"
        );
        let rejected_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let expected_rejection = rejected_result
            .0
            .as_ref()
            .expect_err("fixture is rejected")
            .to_string();
        let (_, rejected) = terminal_offline_operation_in_entrypoint(
            &entrypoint,
            &rejected_result,
            &issuer.authority,
            operation_id,
            19,
        )
        .expect("matching rejected operation must be reconstructed");
        assert_eq!(
            rejected.outcome,
            KagemushaV2TerminalOutcome::Rejected(expected_rejection)
        );
        assert_eq!(rejected.finalized_block_height, 19);
    }
    #[test]
    fn terminal_outcome_evidence_binds_every_persisted_identity_and_hash() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x7F);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let entrypoint = TransactionEntrypoint::External(transaction.clone());
        let result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &result,
            9,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        terminal_offline_operation_from_outcome_evidence(
            &outcome,
            &entrypoint,
            &result,
            &issuer.authority,
            operation_id,
        )
        .expect("exact terminal outcome evidence must be accepted");

        let foreign_key = KeyPair::try_from_seed(vec![0x7E; 32], Algorithm::Ed25519)
            .expect("derive foreign outcome authority");
        let foreign_authority = AccountId::new(foreign_key.public_key().clone());
        let mut mutations = Vec::new();
        let mut candidate = outcome.clone();
        candidate.outer_authority_digest =
            kagemusha_operation_authority_digest_v4(&foreign_authority)
                .expect("hash foreign outer authority");
        mutations.push(("outer authority", candidate));
        let mut candidate = outcome.clone();
        candidate.request_authority_digest =
            kagemusha_operation_authority_digest_v4(&foreign_authority)
                .expect("hash foreign request authority");
        mutations.push(("request authority", candidate));
        let mut candidate = outcome.clone();
        candidate.operation_id = [0x7D; 32];
        mutations.push(("operation id", candidate));
        let mut candidate = outcome.clone();
        candidate.kind = match outcome.kind {
            iroha_data_model::offline::KagemushaOperationKindV4::TopUp => {
                iroha_data_model::offline::KagemushaOperationKindV4::Redeem
            }
            iroha_data_model::offline::KagemushaOperationKindV4::Redeem => {
                iroha_data_model::offline::KagemushaOperationKindV4::TopUp
            }
        };
        mutations.push(("operation kind", candidate));
        let mut candidate = outcome.clone();
        candidate.canonical_request_digest = [0x7C; 32];
        mutations.push(("canonical request digest", candidate));
        let mut candidate = outcome.clone();
        candidate.signed_transaction_wire_hash = [0x7B; 32];
        mutations.push(("signed transaction wire hash", candidate));
        let mut candidate = outcome.clone();
        candidate.entrypoint_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign terminal entrypoint"));
        mutations.push(("entrypoint hash", candidate));
        let mut candidate = outcome.clone();
        candidate.result_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign terminal result",
        )));
        mutations.push(("result hash", candidate));
        let mut candidate = outcome;
        candidate.outcome = KagemushaOperationOutcomeStateV4::Applied;
        mutations.push(("terminal outcome", candidate));

        for (field, candidate) in mutations {
            assert!(
                terminal_offline_operation_from_outcome_evidence(
                    &candidate,
                    &entrypoint,
                    &result,
                    &issuer.authority,
                    operation_id,
                )
                .is_err(),
                "mismatched {field} must fail closed"
            );
        }
    }
    #[tokio::test]
    async fn submission_failure_reconciliation_returns_exact_committed_reference() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x8C);
        let operation_id = request.authorization.operation_id;
        let foreign_signer = KeyPair::try_from_seed(vec![0x8D; 32], Algorithm::Ed25519)
            .expect("derive authorized foreign manager fixture key");
        let transaction =
            submission_test_transaction_signed_by(vec![request.clone()], &foreign_signer);
        let transaction_hash = transaction.hash();
        let applied_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &applied_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(signed_history_block(
            1,
            None,
            request.authorization.issued_at_ms,
            vec![transaction],
            vec![applied_result.0],
        ))
        .expect("store globally applied reconciliation fixture");
        let app = app_with_offline_history(kura, Arc::clone(&issuer), Some(&outcome));
        let requested_binding = submission_test_binding(&request);
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            axum::http::HeaderValue::from_str(&hex::encode(operation_id))
                .expect("canonical operation id header"),
        );
        let replay = handle_top_up(Arc::clone(&app), &headers, request.clone())
            .await
            .expect("an exact committed retry must not require current issuer write readiness");
        assert_eq!(
            decode_offline_operation_reference(replay)
                .await
                .transaction_hash,
            transaction_hash.to_string()
        );

        for stale_error in [
            validation(
                "offline_hardware_authorization_invalid",
                "stale preflight failure",
            ),
            validation("offline_asset_not_found", "stale snapshot failure"),
        ] {
            let response = reconcile_offline_submission_failure(
                &app,
                &issuer,
                OfflineOperationRequest::TopUp(&request),
                &requested_binding,
                stale_error,
            )
            .expect("canonical finality must supersede a stale local validation failure");
            let reference = decode_offline_operation_reference(response).await;
            assert_eq!(reference.operation_id, hex::encode(operation_id));
            assert_eq!(reference.kind, OfflineOperationKind::TopUp);
            assert_eq!(reference.transaction_hash, transaction_hash.to_string());
        }

        let lost_leader = claim_test_leader(&issuer, &request);
        issuer.admission.lock().in_flight.remove(&operation_id);
        let acceptance_error = lost_leader
            .accept(submission_test_hash(0x8C))
            .expect_err("a leader without its reservation must fail local acceptance");
        let response = reconcile_offline_submission_failure(
            &app,
            &issuer,
            OfflineOperationRequest::TopUp(&request),
            &requested_binding,
            acceptance_error,
        )
        .expect("canonical finality must supersede a local acceptance bookkeeping failure");
        assert_eq!(
            decode_offline_operation_reference(response)
                .await
                .transaction_hash,
            transaction_hash.to_string()
        );
    }
    #[tokio::test]
    async fn foreign_global_applied_outcome_wins_and_accepts_transaction_hash_change() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x8E);
        let operation_id = request.authorization.operation_id;
        let configured_transaction = submission_test_transaction(vec![request.clone()]);
        let configured_transaction_hash = configured_transaction.hash();
        let configured_result = TransactionResult::new(Err(
            TransactionRejectionReason::Validation(ValidationFail::TooComplex),
        ));
        let configured_outcome = terminal_outcome_fixture(
            &configured_transaction,
            &configured_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let foreign_signer = KeyPair::try_from_seed(vec![0x8F; 32], Algorithm::Ed25519)
            .expect("derive second authorized manager fixture key");
        let foreign_transaction =
            submission_test_transaction_signed_by(vec![request.clone()], &foreign_signer);
        let foreign_transaction_hash = foreign_transaction.hash();
        let foreign_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let foreign_outcome = terminal_outcome_fixture(
            &foreign_transaction,
            &foreign_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            1,
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(signed_history_block(
            1,
            None,
            request.authorization.issued_at_ms,
            vec![configured_transaction, foreign_transaction],
            vec![configured_result.0, foreign_result.0],
        ))
        .expect("store competing manager outcome fixture");
        let app = app_with_offline_histories(
            kura,
            Arc::clone(&issuer),
            [&configured_outcome, &foreign_outcome],
        );

        let (record, finality) =
            find_terminal_offline_operation_by_id(&app, &issuer.authority, operation_id)
                .expect("global Applied lookup must remain coherent")
                .expect("global Applied must win over the configured manager's rejection");
        assert_eq!(record.transaction_hash, foreign_transaction_hash);
        assert_eq!(finality.outcome, KagemushaV2TerminalOutcome::Applied);
        let admitted = AdmittedOfflineOperationRecord {
            binding: submission_test_binding(&request),
            transaction_hash: configured_transaction_hash,
        };
        assert!(
            terminal_operation_supersedes_admitted_record(&admitted, &record, &finality)
                .expect("global Applied may carry another authorized manager's transaction hash")
        );
        issuer.admission.lock().insert_reserved(admitted);
        let response = find_existing_offline_operation(
            &app,
            &issuer,
            OfflineOperationRequest::TopUp(&request),
            &submission_test_binding(&request),
        )
        .expect("global finality lookup must succeed");
        let OfflineSubmissionRecovery::Existing(response) = response else {
            panic!("global finality must supersede process-local admission provenance")
        };
        let reference = decode_offline_operation_reference(response).await;
        assert_eq!(
            reference.transaction_hash,
            foreign_transaction_hash.to_string()
        );
        assert!(
            issuer.admission.lock().get(&operation_id).is_some(),
            "global Applied must return before any attempt-local admission eviction"
        );
    }
    #[test]
    fn foreign_rejected_attempt_neither_shadows_nor_replaces_configured_attempt() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x90);
        let operation_id = request.authorization.operation_id;
        let foreign_signer = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
            .expect("derive rejected foreign manager fixture key");
        let foreign_transaction =
            submission_test_transaction_signed_by(vec![request.clone()], &foreign_signer);
        let foreign_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("foreign attempt rejected".to_owned()),
        )));
        let foreign_outcome = terminal_outcome_fixture(
            &foreign_transaction,
            &foreign_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let configured_transaction = submission_test_transaction(vec![request.clone()]);
        let configured_transaction_hash = configured_transaction.hash();
        let configured_result = TransactionResult::new(Err(
            TransactionRejectionReason::Validation(ValidationFail::TooComplex),
        ));
        let configured_outcome = terminal_outcome_fixture(
            &configured_transaction,
            &configured_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            1,
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(signed_history_block(
            1,
            None,
            request.authorization.issued_at_ms,
            vec![foreign_transaction, configured_transaction],
            vec![foreign_result.0, configured_result.0],
        ))
        .expect("store per-manager rejection fixture");

        let foreign_only =
            app_with_offline_histories(Arc::clone(&kura), Arc::clone(&issuer), [&foreign_outcome]);
        assert!(
            find_terminal_offline_operation_by_id(&foreign_only, &issuer.authority, operation_id,)
                .expect("foreign rejection lookup must remain coherent")
                .is_none(),
            "another manager's rejected attempt must remain private to that manager"
        );
        assert!(matches!(
            find_existing_offline_operation(
                &foreign_only,
                &issuer,
                OfflineOperationRequest::TopUp(&request),
                &submission_test_binding(&request),
            )
            .expect("foreign rejection must not corrupt POST recovery"),
            OfflineSubmissionRecovery::Absent
        ));

        let both = app_with_offline_histories(
            kura,
            Arc::clone(&issuer),
            [&foreign_outcome, &configured_outcome],
        );
        let (record, finality) =
            find_terminal_offline_operation_by_id(&both, &issuer.authority, operation_id)
                .expect("configured attempt lookup must remain coherent")
                .expect("configured manager's rejected attempt must remain visible");
        assert_eq!(record.transaction_hash, configured_transaction_hash);
        assert!(matches!(
            finality.outcome,
            KagemushaV2TerminalOutcome::Rejected(_)
        ));
    }
    #[tokio::test]
    async fn exact_rejected_attempt_retries_with_next_nonce_and_newer_status_wins() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x92);
        let operation_id = request.authorization.operation_id;
        let rejected_transaction = submission_test_transaction(vec![request.clone()]);
        assert_eq!(rejected_transaction.nonce(), None);
        let rejected_transaction_hash = rejected_transaction.hash();
        let rejected_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let rejected_outcome = terminal_outcome_fixture(
            &rejected_transaction,
            &rejected_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(signed_history_block(
            1,
            None,
            request.authorization.issued_at_ms,
            vec![rejected_transaction],
            vec![rejected_result.0],
        ))
        .expect("store retryable rejected attempt fixture");
        issuer
            .admission
            .lock()
            .insert_reserved(AdmittedOfflineOperationRecord {
                binding: submission_test_binding(&request),
                transaction_hash: rejected_transaction_hash,
            });
        let app = app_with_offline_history(kura, Arc::clone(&issuer), Some(&rejected_outcome));

        let mut conflicting = request.clone();
        conflicting
            .artifact_binding
            .generation
            .push_str("-conflict");
        refresh_submission_test_authorization(&mut conflicting);
        assert!(matches!(
            find_existing_offline_operation(
                &app,
                &issuer,
                OfflineOperationRequest::TopUp(&conflicting),
                &submission_test_binding(&conflicting),
            ),
            Err(Error::AppConflict {
                code: "operation_id_conflict",
                ..
            })
        ));
        assert!(
            issuer.admission.lock().get(&operation_id).is_some(),
            "a different request must not evict the rejected request's admitted binding"
        );

        let recovery = find_existing_offline_operation(
            &app,
            &issuer,
            OfflineOperationRequest::TopUp(&request),
            &submission_test_binding(&request),
        )
        .expect("exact rejected attempt must be recoverable for retry");
        let OfflineSubmissionRecovery::RetryRejected { next_nonce } = recovery else {
            panic!("an exact configured-authority rejection must request a replacement carrier")
        };
        assert_eq!(next_nonce, NonZeroU32::new(1).expect("one is non-zero"));
        assert!(
            issuer.admission.lock().get(&operation_id).is_none(),
            "only the exact stale admitted transaction may be evicted"
        );
        assert!(matches!(
            reconcile_offline_submission_failure(
                &app,
                &issuer,
                OfflineOperationRequest::TopUp(&request),
                &submission_test_binding(&request),
                validation("retry_probe", "retry must not return the old rejection"),
            ),
            Err(Error::AppQueryValidation {
                code: "retry_probe",
                ..
            })
        ));

        let replacement_transaction = submission_test_transaction_signed_by_with_nonce(
            vec![request.clone()],
            &issuer.key_pair,
            Some(next_nonce),
        );
        let replacement_hash = replacement_transaction.hash();
        assert_ne!(replacement_hash, rejected_transaction_hash);
        assert_eq!(replacement_transaction.nonce(), Some(next_nonce));
        let replacement_record = offline_operation_record_in_transaction(
            &replacement_transaction,
            &issuer.authority,
            operation_id,
        )
        .expect("replacement carrier must preserve the exact logical request");
        assert_eq!(
            next_rejected_attempt_nonce(&replacement_record)
                .expect("a second rejection must advance its exact carrier nonce"),
            NonZeroU32::new(2).expect("two is non-zero")
        );
        issuer
            .admission
            .lock()
            .insert_reserved(AdmittedOfflineOperationRecord {
                binding: submission_test_binding(&request),
                transaction_hash: replacement_hash,
            });
        app.pipeline_status_cache.record_entry(
            replacement_hash,
            crate::PipelineStatusEntry::fresh(crate::PipelineStatusKind::Queued, None, None),
        );

        let recovery = find_existing_offline_operation(
            &app,
            &issuer,
            OfflineOperationRequest::TopUp(&request),
            &submission_test_binding(&request),
        )
        .expect("newer admitted attempt must remain recoverable");
        let OfflineSubmissionRecovery::Existing(response) = recovery else {
            panic!("newer admitted attempt must supersede the older rejection")
        };
        assert_eq!(
            decode_offline_operation_reference(response)
                .await
                .transaction_hash,
            replacement_hash.to_string()
        );
        let status = handle_operation_status(&app, &hex::encode(operation_id))
            .expect("newer pipeline attempt must supersede the older rejection");
        assert!(matches!(
            decode_offline_operation_status(status).await,
            OfflineOperationStatus::Pending {
                transaction_hash,
                ..
            } if transaction_hash == replacement_hash.to_string()
        ));

        let exhausted_issuer = submission_test_issuer();
        let exhausted_transaction = submission_test_transaction_signed_by_with_nonce(
            vec![request.clone()],
            &exhausted_issuer.key_pair,
            NonZeroU32::new(u32::MAX),
        );
        let exhausted_record = offline_operation_record_in_transaction(
            &exhausted_transaction,
            &exhausted_issuer.authority,
            operation_id,
        )
        .expect("exhausted nonce fixture remains an exact carrier");
        assert!(matches!(
            next_rejected_attempt_nonce(&exhausted_record),
            Err(Error::AppConflict {
                code: "offline_operation_retry_exhausted",
                ..
            })
        ));

        let exhausted_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let exhausted_outcome = terminal_outcome_fixture(
            &exhausted_transaction,
            &exhausted_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let exhausted_kura = Kura::blank_kura_for_testing();
        exhausted_kura
            .store_block(signed_history_block(
                1,
                None,
                request.authorization.issued_at_ms,
                vec![exhausted_transaction],
                vec![exhausted_result.0],
            ))
            .expect("store exhausted rejected attempt fixture");
        let exhausted_app = app_with_offline_history(
            exhausted_kura,
            Arc::clone(&exhausted_issuer),
            Some(&exhausted_outcome),
        );
        assert!(matches!(
            find_existing_offline_operation(
                &exhausted_app,
                &exhausted_issuer,
                OfflineOperationRequest::TopUp(&request),
                &submission_test_binding(&request),
            ),
            Err(Error::AppConflict {
                code: "offline_operation_retry_exhausted",
                ..
            })
        ));
    }
    #[test]
    fn malformed_terminal_outcome_state_is_unavailable() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x7B);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let mut outcome = terminal_outcome_fixture(
            &transaction,
            &result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        outcome.version = KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4.saturating_sub(1);
        let app = app_with_offline_history(
            Kura::blank_kura_for_testing(),
            Arc::clone(&issuer),
            Some(&outcome),
        );
        let error = find_terminal_offline_operation_by_id(&app, &issuer.authority, operation_id)
            .expect_err("a malformed consensus outcome must fail closed");
        assert_offline_history_error(error, "offline_operation_evidence_inconsistent");
    }
    #[tokio::test]
    async fn ordinary_canonical_history_survives_restart_with_an_empty_operation_registry() {
        let request = submission_test_request(0x81);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let transaction_hash = transaction.hash();
        let creation_time_ms = request.authorization.issued_at_ms;
        let terminal_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("canonical offline history rejection".to_owned()),
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &terminal_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            1,
        );
        let decoy = submission_test_transaction(vec![submission_test_request(0x80)]);
        let block = signed_history_block(
            1,
            None,
            creation_time_ms,
            vec![decoy, transaction],
            vec![
                TransactionResultInner::Ok(DataTriggerSequence::default()),
                terminal_result.0.clone(),
            ],
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(block)
            .expect("store canonical offline history block");
        let restarted_issuer = submission_test_issuer();
        {
            let admission = restarted_issuer.admission.lock();
            assert!(
                admission.records.is_empty() && admission.in_flight.is_empty(),
                "restart fixture must not rely on the process-local admission registry"
            );
        }
        let app = app_with_offline_history(
            Arc::clone(&kura),
            Arc::clone(&restarted_issuer),
            Some(&outcome),
        );
        let (record, finality) =
            find_terminal_offline_operation_by_id(&app, &restarted_issuer.authority, operation_id)
                .expect("canonical history lookup must remain available")
                .expect("canonical history must contain the signed operation");
        assert_eq!(
            record.request,
            OfflineOperationRequest::TopUp(&request).into_owned()
        );
        assert_eq!(record.transaction_hash, transaction_hash);
        assert_eq!(finality.finalized_block_height, 1);
        assert!(matches!(
            finality.outcome,
            KagemushaV2TerminalOutcome::Rejected(_)
        ));
        let readiness_error = ensure_offline_command_authority_ready(&app, &restarted_issuer)
            .expect_err("history fixture deliberately has no live command-authority account");
        assert_offline_readiness_code(readiness_error, "offline_command_authority_not_ready");
        let response = handle_operation_status(&app, &hex::encode(operation_id))
            .expect("historical status must not require authority readiness to reconstruct");
        match decode_offline_operation_status(response).await {
            OfflineOperationStatus::Rejected {
                operation_id: actual_operation_id,
                kind,
                transaction_hash: actual_transaction_hash,
                ..
            } => {
                assert_eq!(actual_operation_id, hex::encode(operation_id));
                assert_eq!(kind, OfflineOperationKind::TopUp);
                assert_eq!(actual_transaction_hash, transaction_hash.to_string());
            }
            other => panic!("canonical rejection returned the wrong status: {other:?}"),
        }
    }
    #[test]
    fn ordinary_terminal_lookup_uses_the_external_result_prefix_before_time_triggers() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x8B);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let terminal_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("canonical offline result before timer".to_owned()),
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &terminal_result,
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let time_trigger = TimeTriggerEntrypoint {
            id: "offline-history-timer".parse().expect("fixture trigger id"),
            instructions: ExecutionStep(ConstVec::new_empty()),
            authority: issuer.authority.clone(),
        };
        let block = signed_history_block_with_time_triggers(
            1,
            None,
            601,
            vec![transaction],
            vec![time_trigger],
            vec![
                terminal_result.0.clone(),
                TransactionResultInner::Ok(DataTriggerSequence::default()),
            ],
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(block)
            .expect("store ordinary history with a time-trigger suffix");
        let app = app_with_offline_history(kura, Arc::clone(&issuer), Some(&outcome));

        let (_, finality) =
            find_terminal_offline_operation_by_id(&app, &issuer.authority, operation_id)
                .expect("time-trigger suffix must not corrupt ordinary operation evidence")
                .expect("terminal operation must be found at the external prefix");
        assert!(matches!(
            finality.outcome,
            KagemushaV2TerminalOutcome::Rejected(_)
        ));
    }
    #[tokio::test]
    async fn committed_merge_execution_history_is_resolved_from_its_carrier_sidecar() {
        let request = submission_test_request(0x82);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let transaction_hash = transaction.hash();
        let terminal_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("certified merge rejection".to_owned()),
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &terminal_result,
            2,
            KagemushaOperationExecutionPhaseV4::Merge,
            0,
        );
        let kura = Kura::blank_kura_for_testing();
        let parent = signed_history_block(1, None, 101, Vec::new(), Vec::new());
        let parent_hash = parent.hash();
        kura.store_block(parent)
            .expect("store merge carrier parent block");
        let carrier = signed_history_block(2, Some(parent_hash), 202, Vec::new(), Vec::new());
        let entry = committed_merge_history_entry(transaction, terminal_result, &carrier.header());
        let carrier = attach_committed_merge_reference(carrier, &entry);
        kura.store_block_with_merge_entry(carrier, &entry)
            .expect("commit merge execution carrier and exact sidecar");
        assert!(
            kura.get_merge_entry_by_carrier_height(
                NonZeroUsize::new(2).expect("merge carrier height is non-zero")
            )
            .expect("read committed merge carrier")
            .is_some(),
            "the operation must be recovered from a durable carrier association"
        );
        let restarted_issuer = submission_test_issuer();
        let app = app_with_offline_history(
            Arc::clone(&kura),
            Arc::clone(&restarted_issuer),
            Some(&outcome),
        );
        let (record, finality) =
            find_terminal_offline_operation_by_id(&app, &restarted_issuer.authority, operation_id)
                .expect("merge history lookup must remain available")
                .expect("merge sidecar must contain the signed operation");
        assert_eq!(
            record.request,
            OfflineOperationRequest::TopUp(&request).into_owned()
        );
        assert_eq!(record.transaction_hash, transaction_hash);
        assert_eq!(finality.finalized_block_height, 2);
        assert!(matches!(
            finality.outcome,
            KagemushaV2TerminalOutcome::Rejected(_)
        ));
        let response = handle_operation_status(&app, &hex::encode(operation_id))
            .expect("status must reconstruct a committed merge-side operation");
        assert!(matches!(
            decode_offline_operation_status(response).await,
            OfflineOperationStatus::Rejected { .. }
        ));
    }
    #[test]
    fn absent_terminal_outcome_does_not_scan_partially_reconstructed_kura_history() {
        let request = submission_test_request(0x83);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let kura = Kura::blank_kura_for_testing();
        let block1 = signed_history_block(
            1,
            None,
            301,
            vec![transaction],
            vec![TransactionResultInner::Err(
                TransactionRejectionReason::Validation(ValidationFail::TooComplex),
            )],
        );
        let block1_hash = block1.hash();
        kura.store_block(block1)
            .expect("store history prefix before snapshot recovery");
        let snapshot_tail_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"offline-status-verified-snapshot-tail"));
        assert_eq!(
            kura.extend_hash_only_suffix_from_verified_snapshot(
                &[block1_hash, snapshot_tail_hash,]
            )
            .expect("publish verified snapshot hash-only suffix"),
            1,
        );
        assert_eq!(kura.blocks_count(), 2);
        assert!(
            kura.get_block(NonZeroUsize::new(2).expect("snapshot tail height is non-zero"))
                .is_none(),
            "verified snapshot recovery deliberately leaves one body pending reconstruction"
        );
        let issuer = submission_test_issuer();
        let app = app_with_offline_history(kura, issuer, None);
        let error = handle_operation_status(&app, &hex::encode(operation_id))
            .expect_err("missing authoritative outcome must remain unknown");
        assert!(matches!(
            error,
            Error::AppNotFound {
                code: "offline_operation_not_found",
                ..
            }
        ));
    }
    #[test]
    fn terminal_outcome_with_missing_block_body_fails_closed() {
        let request = submission_test_request(0x84);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let terminal_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &terminal_result,
            2,
            KagemushaOperationExecutionPhaseV4::Ordinary,
            0,
        );
        let directory = TempDir::new().expect("create evicted Kura fixture directory");
        let config = persistent_kura_config(&directory);
        let (kura, _) = Kura::new_fresh_single_lane(&config, &RuntimeLaneConfig::default())
            .expect("create evicted offline history Kura");
        let block1 = signed_history_block(1, None, 401, Vec::new(), Vec::new());
        let block1_hash = block1.hash();
        kura.store_block(block1).expect("store eviction block 1");
        let block2 = signed_history_block(
            2,
            Some(block1_hash),
            402,
            vec![transaction],
            vec![terminal_result.0.clone()],
        );
        let block2_hash = block2.hash();
        kura.store_block(block2)
            .expect("store terminal offline history block");
        let block3 = signed_history_block(3, Some(block2_hash), 403, Vec::new(), Vec::new());
        let block3_hash = block3.hash();
        kura.store_block(block3).expect("store eviction block 3");
        kura.store_block(signed_history_block(
            4,
            Some(block3_hash),
            404,
            Vec::new(),
            Vec::new(),
        ))
        .expect("store eviction block 4");
        let issuer = submission_test_issuer();
        let carrier_height = NonZeroUsize::new(2).expect("history height is non-zero");
        let data_path = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(directory.path())
            .join("blocks.data");
        assert!(
            data_path.exists(),
            "the canonical body data file must exist before adversarial loss"
        );
        std::fs::remove_file(&data_path)
            .expect("simulate adversarial loss of the terminal evidence block body");
        assert!(
            kura.get_block(carrier_height).is_none(),
            "the terminal locator remains while the local body is unavailable"
        );
        let app = app_with_offline_history(kura, issuer, Some(&outcome));
        let error = handle_operation_status(&app, &hex::encode(operation_id))
            .expect_err("a terminal outcome cannot be answered without its canonical body");
        assert_offline_history_error(error, "offline_operation_history_unavailable");
    }
    #[test]
    fn misaligned_committed_merge_execution_is_never_zipped_or_partially_trusted() {
        let request = submission_test_request(0x85);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let terminal_result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let outcome = terminal_outcome_fixture(
            &transaction,
            &terminal_result,
            2,
            KagemushaOperationExecutionPhaseV4::Merge,
            0,
        );
        let kura = Kura::blank_kura_for_testing();
        let parent = signed_history_block(1, None, 501, Vec::new(), Vec::new());
        let parent_hash = parent.hash();
        kura.store_block(parent)
            .expect("store malformed merge carrier parent");
        let carrier = signed_history_block(2, Some(parent_hash), 502, Vec::new(), Vec::new());
        let mut entry =
            committed_merge_history_entry(transaction, terminal_result, &carrier.header());
        entry
            .execution_batch
            .as_mut()
            .expect("merge fixture has an execution batch")
            .lanes[0]
            .results
            .clear();
        let carrier = attach_committed_merge_reference(carrier, &entry);
        kura.store_block_with_merge_entry(carrier, &entry)
            .expect("persist adversarial misaligned merge history fixture");
        let issuer = submission_test_issuer();
        let app = app_with_offline_history(kura, Arc::clone(&issuer), Some(&outcome));
        let error = find_terminal_offline_operation_by_id(&app, &issuer.authority, operation_id)
            .expect_err("misaligned entrypoint/result history must fail closed");
        assert_offline_history_error(error, "offline_operation_evidence_inconsistent");
    }
    #[test]
    fn pipeline_applied_ram_hint_cannot_manufacture_canonical_offline_finality() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x86);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let transaction_hash = transaction.hash();
        issuer
            .admission
            .lock()
            .insert_reserved(AdmittedOfflineOperationRecord {
                binding: submission_test_binding(&request),
                transaction_hash,
            });
        let kura = Kura::blank_kura_for_testing();
        let app = app_with_offline_history(kura, issuer, None);
        app.pipeline_status_cache.record_entry(
            transaction_hash,
            crate::PipelineStatusEntry::fresh(
                crate::PipelineStatusKind::Applied,
                NonZeroU64::new(1),
                None,
            ),
        );
        let error = handle_operation_status(&app, &hex::encode(operation_id))
            .expect_err("RAM-only Applied must not become a durable applied operation response");
        assert_offline_history_error(error, "offline_operation_evidence_inconsistent");
    }
    #[tokio::test]
    async fn submission_claim_deduplicates_and_binds_the_complete_typed_request() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x11);
        let leader = claim_test_leader(&issuer, &request);
        let follower = match issuer
            .claim_submission(submission_test_binding(&request))
            .expect("identical concurrent request must join the leader")
        {
            SubmissionClaim::Follower(receiver) => receiver,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Leader(_) => {
                panic!("identical in-flight request must be a follower")
            }
        };
        let mut conflicting = request.clone();
        conflicting.artifact_binding.generation.push_str("-forged");
        refresh_submission_test_authorization(&mut conflicting);
        let error = match issuer.claim_submission(submission_test_binding(&conflicting)) {
            Err(error) => error,
            Ok(_) => panic!("same operation id with changed fields must conflict"),
        };
        assert!(matches!(
            error,
            Error::AppConflict {
                code: "operation_id_conflict",
                ..
            }
        ));
        let transaction_hash = submission_test_hash(0x71);
        let admitted = leader
            .accept(transaction_hash)
            .expect("reserved fixture transition must succeed");
        let observed = tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_submission_outcome(follower),
        )
        .await
        .expect("accepted submission must release every follower");
        let SubmissionOutcome::Accepted(observed) = observed else {
            panic!("accepted leader must publish the admitted operation")
        };
        assert_eq!(observed.binding, admitted.binding);
        assert_eq!(observed.transaction_hash, transaction_hash);
        match issuer
            .claim_submission(submission_test_binding(&request))
            .expect("admitted replay must be returned without resubmission")
        {
            SubmissionClaim::Accepted(replayed) => {
                assert_eq!(replayed.transaction_hash, transaction_hash);
                assert_eq!(replayed.binding, admitted.binding);
            }
            SubmissionClaim::Leader(_) | SubmissionClaim::Follower(_) => {
                panic!("admitted replay must never create or join another submission")
            }
        }
        let error = match issuer.claim_submission(submission_test_binding(&conflicting)) {
            Err(error) => error,
            Ok(_) => panic!("admitted operation id must stay bound to its original request"),
        };
        assert!(matches!(
            error,
            Error::AppConflict {
                code: "operation_id_conflict",
                ..
            }
        ));
        assert_eq!(issuer.admission.lock().records.len(), 1);
        assert!(issuer.admission.lock().in_flight.is_empty());
    }
    #[tokio::test]
    async fn cancelled_submission_leader_releases_followers_for_retry() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x12);
        let leader = claim_test_leader(&issuer, &request);
        let follower = match issuer
            .claim_submission(submission_test_binding(&request))
            .expect("claim cancellation follower")
        {
            SubmissionClaim::Follower(receiver) => receiver,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Leader(_) => {
                panic!("concurrent request must follow the elected leader")
            }
        };
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _leader = leader;
            let _ = ready_tx.send(());
            std::future::pending::<()>().await;
        });
        ready_rx.await.expect("leader task entered pending state");
        task.abort();
        assert!(
            task.await
                .expect_err("leader task must be cancelled")
                .is_cancelled()
        );
        retry_outcome(follower).await;
        let replacement = claim_test_leader(&issuer, &request);
        drop(replacement);
        assert!(issuer.admission.lock().in_flight.is_empty());
        assert!(issuer.admission.lock().records.is_empty());
    }
    #[tokio::test]
    async fn panicking_submission_leader_releases_followers_without_poisoning_coordinator() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x13);
        let leader = claim_test_leader(&issuer, &request);
        let follower = match issuer
            .claim_submission(submission_test_binding(&request))
            .expect("claim panic follower")
        {
            SubmissionClaim::Follower(receiver) => receiver,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Leader(_) => {
                panic!("concurrent request must follow the elected leader")
            }
        };
        let task = tokio::spawn(async move {
            let _leader = leader;
            panic!("adversarial leader panic");
        });
        assert!(task.await.expect_err("leader task must panic").is_panic());
        retry_outcome(follower).await;
        let replacement = claim_test_leader(&issuer, &request);
        drop(replacement);
        assert!(issuer.admission.lock().in_flight.is_empty());
    }
    #[tokio::test]
    async fn stale_submission_leader_cannot_remove_a_newer_generation() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x14);
        let stale_leader = claim_test_leader(&issuer, &request);
        let operation_id = request.authorization.operation_id;
        let replacement_token = Arc::new(());
        let (replacement_updates, replacement_receiver) =
            watch::channel(SubmissionOutcome::Pending);
        {
            let mut admission = issuer.admission.lock();
            admission.in_flight.insert(
                operation_id,
                InFlightSubmission {
                    binding: OfflineOperationRequestBinding::from_request(
                        OfflineOperationRequest::TopUp(&request),
                    )
                    .expect("canonical replacement binding"),
                    token: Arc::clone(&replacement_token),
                    updates: replacement_updates.clone(),
                },
            );
        }
        drop(stale_leader);
        let admission = issuer.admission.lock();
        let replacement = admission
            .in_flight
            .get(&operation_id)
            .expect("newer generation must survive stale leader drop");
        assert!(Arc::ptr_eq(&replacement.token, &replacement_token));
        drop(admission);
        assert!(matches!(
            &*replacement_receiver.borrow(),
            SubmissionOutcome::Pending
        ));
        issuer.admission.lock().in_flight.remove(&operation_id);
        let _ = replacement_updates.send_replace(SubmissionOutcome::Retry);
        retry_outcome(replacement_receiver).await;
    }
    #[tokio::test]
    async fn accepted_submission_with_lost_reservation_fails_closed_and_releases_followers() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x1F);
        let stale_leader = claim_test_leader(&issuer, &request);
        let stale_follower = stale_leader.updates.subscribe();
        let operation_id = request.authorization.operation_id;
        let replacement_token = Arc::new(());
        let (replacement_updates, replacement_receiver) =
            watch::channel(SubmissionOutcome::Pending);
        {
            let mut admission = issuer.admission.lock();
            admission.in_flight.insert(
                operation_id,
                InFlightSubmission {
                    binding: submission_test_binding(&request),
                    token: Arc::clone(&replacement_token),
                    updates: replacement_updates.clone(),
                },
            );
        }
        let error = stale_leader
            .accept(submission_test_hash(0x7F))
            .expect_err("a stale leader must not publish a false accepted cache transition");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "offline_operation_admission_inconsistent",
                ..
            }
        ));
        retry_outcome(stale_follower).await;
        {
            let admission = issuer.admission.lock();
            assert!(admission.records.get(&operation_id).is_none());
            let replacement = admission
                .in_flight
                .get(&operation_id)
                .expect("newer reservation survives stale acceptance");
            assert!(Arc::ptr_eq(&replacement.token, &replacement_token));
        }
        issuer.admission.lock().in_flight.remove(&operation_id);
        let _ = replacement_updates.send_replace(SubmissionOutcome::Retry);
        retry_outcome(replacement_receiver).await;
    }
    #[tokio::test]
    async fn closed_submission_channel_fails_safe_to_retry() {
        let (updates, receiver) = watch::channel(SubmissionOutcome::Pending);
        drop(updates);
        retry_outcome(receiver).await;
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn admission_and_duplicate_claim_race_never_elects_a_second_leader() {
        for seed in 0x20..0x30 {
            let issuer = submission_test_issuer();
            let request = submission_test_request(seed);
            let leader = claim_test_leader(&issuer, &request);
            let barrier = Arc::new(Barrier::new(2));
            let claim_issuer = Arc::clone(&issuer);
            let claim_request = request.clone();
            let claim_barrier = Arc::clone(&barrier);
            let claim = tokio::task::spawn_blocking(move || {
                claim_barrier.wait();
                claim_issuer.claim_submission(submission_test_binding(&claim_request))
            });
            barrier.wait();
            let admitted = leader
                .accept(submission_test_hash(seed))
                .expect("reserved race winner transition must succeed");
            match claim
                .await
                .expect("duplicate claim task")
                .expect("duplicate claim must not fail")
            {
                SubmissionClaim::Accepted(record) => {
                    assert_eq!(record.transaction_hash, admitted.transaction_hash);
                }
                SubmissionClaim::Follower(receiver) => {
                    let outcome = wait_for_submission_outcome(receiver).await;
                    assert!(matches!(
                        outcome,
                        SubmissionOutcome::Accepted(ref record)
                            if record.transaction_hash == admitted.transaction_hash
                    ));
                }
                SubmissionClaim::Leader(_) => {
                    panic!("admission race elected a duplicate submission leader")
                }
            }
        }
    }
    #[tokio::test]
    async fn duplicate_submission_waiter_observes_only_terminal_coordinator_outcomes() {
        let (updates, receiver) = watch::channel(SubmissionOutcome::Pending);
        let waiter = tokio::spawn(wait_for_submission_outcome(receiver));
        tokio::task::yield_now().await;
        assert!(
            !waiter.is_finished(),
            "a duplicate caller must not treat an in-flight reservation as accepted"
        );
        let _ = updates.send_replace(SubmissionOutcome::Retry);
        assert!(matches!(
            waiter.await.expect("waiter task"),
            SubmissionOutcome::Retry
        ));
    }
    #[test]
    fn v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag() {
        let runtime_source = include_str!("offline_commands.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("offline command runtime source");
        assert!(
            !runtime_source.contains("KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE"),
            "Torii V4 admission must authenticate a concrete release instead of treating compile capability as runtime readiness",
        );
        assert!(
            runtime_source.contains("resolve_kagemusha_recursive_transaction_release_v4")
                && runtime_source.contains("ensure_kagemusha_v4_transaction_release"),
            "Torii V4 admission must authenticate the exact activated release",
        );
        assert_eq!(
            runtime_source
                .matches("resolve_kagemusha_recursive_transaction_release_v4")
                .count(),
            3,
            "top-up, redemption parent, and optional redemption change each resolve an exact binding",
        );
        assert!(
            runtime_source.contains("&request.bundle.statement.artifact_binding")
                && runtime_source.contains("&change.bundle.statement.artifact_binding"),
            "redemption must authenticate parent and change bindings independently",
        );
    }
    #[test]
    fn v4_issuance_window_distinguishes_historic_redemption_from_new_notes() {
        ensure_kagemusha_v4_issuance_window(false, false)
            .expect("full redemption remains valid after parent issuance withdrawal");
        for operation in ["top-up", "redemption change"] {
            let error = ensure_kagemusha_v4_issuance_window(false, true)
                .expect_err("new note issuance must reject a withdrawn release");
            assert!(
                matches!(
                    &error,
                    Error::AppServiceUnavailable {
                        code: "offline_recursive_release_outside_issuance_window",
                        ..
                    }
                ),
                "unexpected {operation} error: {error:?}",
            );
        }
    }
    #[test]
    fn v4_rotated_change_accepts_withdrawn_parent_and_active_successor() {
        ensure_kagemusha_v4_issuance_window(false, false)
            .expect("the exact withdrawn parent remains valid for redemption");
        ensure_kagemusha_v4_issuance_window(true, true)
            .expect("an independently selected active successor may issue change");
    }
    #[test]
    fn operation_ids_use_one_canonical_path_spelling() {
        let operation_id = [0xAB; 32];
        let encoded = "ab".repeat(32);
        assert_eq!(
            parse_operation_id(&encoded).expect("canonical id"),
            operation_id
        );
        assert_eq!(
            offline_operation_status_uri(operation_id),
            format!("/v1/offline/operations/{encoded}")
        );
        let uppercase = "AB".repeat(32);
        let non_hex = "gg".repeat(32);
        let zero = "00".repeat(32);
        for invalid in ["ab", uppercase.as_str(), non_hex.as_str(), zero.as_str()] {
            assert!(
                parse_operation_id(invalid).is_err(),
                "invalid id: {invalid}"
            );
        }
    }
    #[test]
    fn idempotency_key_must_equal_the_signed_operation_id() {
        let operation_id = [0x11; 32];
        let mut headers = HeaderMap::new();
        let zero_error = require_idempotency_key(&headers, [0; 32])
            .expect_err("zero signed operation id must fail");
        assert!(matches!(
            zero_error,
            Error::AppQueryValidation {
                code: "operation_id_invalid",
                ..
            }
        ));
        let error = require_idempotency_key(&headers, operation_id)
            .expect_err("missing idempotency key must fail");
        assert!(matches!(
            error,
            Error::AppQueryValidation {
                code: "idempotency_key_missing",
                ..
            }
        ));
        headers.insert(
            "idempotency-key",
            axum::http::HeaderValue::from_static(
                "1111111111111111111111111111111111111111111111111111111111111111",
            ),
        );
        require_idempotency_key(&headers, operation_id).expect("matching idempotency key");
        headers.append(
            "idempotency-key",
            axum::http::HeaderValue::from_static(
                "1111111111111111111111111111111111111111111111111111111111111111",
            ),
        );
        let error = require_idempotency_key(&headers, operation_id)
            .expect_err("duplicate idempotency keys must fail");
        assert!(matches!(
            error,
            Error::AppQueryValidation {
                code: "idempotency_key_invalid",
                ..
            }
        ));
        headers.remove("idempotency-key");
        for malformed in [
            "11",
            "111111111111111111111111111111111111111111111111111111111111111g",
            "111111111111111111111111111111111111111111111111111111111111111A",
        ] {
            headers.insert(
                "idempotency-key",
                axum::http::HeaderValue::from_str(malformed).expect("ASCII fixture header"),
            );
            let error = require_idempotency_key(&headers, operation_id)
                .expect_err("malformed idempotency keys must fail validation");
            assert!(matches!(
                error,
                Error::AppQueryValidation {
                    code: "idempotency_key_invalid",
                    ..
                }
            ));
        }
        headers.insert(
            "idempotency-key",
            axum::http::HeaderValue::from_static(
                "2222222222222222222222222222222222222222222222222222222222222222",
            ),
        );
        let error = require_idempotency_key(&headers, operation_id)
            .expect_err("mismatched idempotency key must fail");
        assert!(matches!(
            error,
            Error::AppConflict {
                code: "idempotency_key_conflict",
                ..
            }
        ));
    }
    #[test]
    fn post_handlers_reconcile_every_mutable_submission_boundary() {
        let source = include_str!("offline_commands.rs");
        let topup_start = source
            .find("pub(crate) async fn handle_top_up(")
            .expect("top-up handler source");
        let redeem_start = source
            .find("pub(crate) async fn handle_redeem(")
            .expect("redemption handler source");
        let redeem_end = source[redeem_start..]
            .find("fn kagemusha_v4_snapshot_time_ms(")
            .map(|offset| redeem_start + offset)
            .expect("redemption handler end");
        for (name, handler) in [
            ("top-up", &source[topup_start..redeem_start]),
            ("redemption", &source[redeem_start..redeem_end]),
        ] {
            assert!(handler.contains("let issuer = require_configured_issuer(&app)?;"));
            assert!(!handler.contains("require_issuer(&app)?"));
            let recovery = handler
                .find("find_existing_offline_operation(")
                .expect("initial authoritative recovery");
            let claim = handler
                .find("issuer.claim_submission(requested_binding)")
                .expect("process-local submission claim");
            assert!(
                recovery < claim,
                "{name} must recover committed or queued work before electing a writer"
            );
            let leader = handler
                .find("SubmissionClaim::Leader(submission)")
                .expect("leader claim arm");
            let follower = handler
                .find("SubmissionClaim::Follower(receiver)")
                .expect("follower claim arm");
            let readiness = handler
                .find("ensure_offline_command_authority_ready(&app, &issuer)")
                .expect("write-readiness check");
            assert!(
                leader < readiness && readiness < follower,
                "{name} must require write readiness only from the elected leader"
            );
            assert_eq!(
                handler
                    .matches("reconcile_offline_submission_failure(")
                    .count(),
                9,
                "{name} must reconcile claim/readiness, preflight, snapshot, amount, policy, signing, Queue admission, and local acceptance failures"
            );
            assert!(handler.contains(
                "OfflineSubmissionRecovery::RetryRejected { next_nonce } => Some(next_nonce)"
            ));
            assert!(handler.contains("transaction.set_nonce(nonce);"));
            assert!(handler.contains("match submission.accept(tx_hash)"));
        }
    }
    #[test]
    fn operation_binding_covers_the_full_typed_request_and_route() {
        #[derive(Clone, Copy, PartialEq, Eq)]
        struct RequestFixture {
            operation_id: [u8; 32],
            amount: u64,
        }
        let original = RequestFixture {
            operation_id: [0x11; 32],
            amount: 7,
        };
        let identical = original;
        let different_amount = RequestFixture {
            amount: 8,
            ..original
        };
        let top_up = OfflineOperationRequest::<&RequestFixture, &RequestFixture>::TopUp(&original);
        let identical_top_up =
            OfflineOperationRequest::<&RequestFixture, &RequestFixture>::TopUp(&identical);
        let changed_top_up =
            OfflineOperationRequest::<&RequestFixture, &RequestFixture>::TopUp(&different_amount);
        let different_route =
            OfflineOperationRequest::<&RequestFixture, &RequestFixture>::Redeem(&identical);
        ensure_same_offline_request(&top_up, &identical_top_up)
            .expect("identical typed request is an idempotent replay");
        for mismatch in [&changed_top_up, &different_route] {
            let error = ensure_same_offline_request(&top_up, mismatch)
                .expect_err("a changed field or route must conflict");
            assert!(matches!(
                error,
                Error::AppConflict {
                    code: "operation_id_conflict",
                    ..
                }
            ));
        }
    }
    #[test]
    fn admission_registry_retention_has_an_inclusive_saturating_boundary() {
        let expires_at_ms = 1_000_u64;
        let retained_until =
            expires_at_ms.saturating_add(OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS);
        assert!(offline_operation_is_retained(expires_at_ms, retained_until));
        assert!(!offline_operation_is_retained(
            expires_at_ms,
            retained_until + 1
        ));
        assert!(offline_operation_is_retained(u64::MAX, u64::MAX));
    }
    #[test]
    fn transaction_signing_failure_does_not_expose_the_signer_error() {
        let error = offline_transaction_signing_error(
            "offline_redeem_transaction",
            "sensitive signer backend detail",
        );
        let Error::Query(ValidationFail::InternalError(message)) = error else {
            panic!("signer failure must remain a typed internal error")
        };
        assert_eq!(
            message,
            "Offline operation signer failed to sign the transaction."
        );
        assert!(!message.contains("sensitive"));
    }
    #[test]
    fn admitted_bindings_are_fixed_size_and_cover_every_canonical_request_byte() {
        assert!(
            !std::mem::needs_drop::<OfflineOperationRequestBinding>(),
            "a binding must not own request buffers"
        );
        assert!(
            !std::mem::needs_drop::<AdmittedOfflineOperationRecord>(),
            "an admitted record must not own request buffers"
        );
        assert!(
            std::mem::size_of::<AdmittedOfflineOperationRecord>() <= 128,
            "the admitted record unexpectedly grew beyond its fixed-size metadata budget"
        );
        let mut request = submission_test_request(0x44);
        request.shield_evidence.proof.proof.bytes =
            vec![0xA5; KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2];
        refresh_submission_test_authorization(&mut request);
        let original =
            OfflineOperationRequestBinding::from_request(OfflineOperationRequest::TopUp(&request))
                .expect("canonical large request binding");
        request.shield_evidence.proof.proof.bytes[KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2 / 2] ^=
            0xFF;
        refresh_submission_test_authorization(&mut request);
        let changed =
            OfflineOperationRequestBinding::from_request(OfflineOperationRequest::TopUp(&request))
                .expect("canonical changed request binding");
        assert_eq!(original.operation_id, changed.operation_id);
        assert_ne!(
            original.canonical_request_digest, changed.canonical_request_digest,
            "a changed byte in a maximum-sized request must change the retained binding"
        );
        assert!(matches!(
            ensure_same_offline_request_binding(&original, &changed),
            Err(Error::AppConflict {
                code: "operation_id_conflict",
                ..
            })
        ));
    }
    #[test]
    fn count_capacity_covers_admitted_and_in_flight_without_evicting_replays() {
        let issuer = submission_test_issuer_with_limits(2, 2 * ADMITTED_OPERATION_ACCOUNTED_BYTES);
        let admitted_request = submission_test_request(0x01);
        let admitted_binding = submission_test_binding(&admitted_request);
        claim_test_leader(&issuer, &admitted_request)
            .accept(submission_test_hash(0x71))
            .expect("reserved admitted fixture transition");
        let pending_request = submission_test_request(0x02);
        let pending_binding = submission_test_binding(&pending_request);
        let pending_leader = claim_test_leader(&issuer, &pending_request);
        assert!(matches!(
            issuer
                .claim_submission(admitted_binding)
                .expect("an admitted replay bypasses capacity"),
            SubmissionClaim::Accepted(_)
        ));
        assert!(matches!(
            issuer
                .claim_submission(pending_binding)
                .expect("an in-flight replay bypasses capacity"),
            SubmissionClaim::Follower(_)
        ));
        let mut conflicting = pending_binding;
        conflicting.canonical_request_digest[0] ^= 1;
        assert!(matches!(
            issuer.claim_submission(conflicting),
            Err(Error::AppConflict {
                code: "operation_id_conflict",
                ..
            })
        ));
        let capacity_error = match issuer
            .claim_submission(submission_test_binding(&submission_test_request(0x03)))
        {
            Err(error) => error,
            Ok(_) => panic!("a third identity must fail closed at capacity"),
        };
        assert!(matches!(
            &capacity_error,
            Error::AppServiceUnavailable {
                code: "offline_operation_capacity_exhausted",
                ..
            }
        ));
        assert_eq!(
            capacity_error.into_response().status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        pending_leader
            .accept(submission_test_hash(0x72))
            .expect("reserved pending fixture transition");
        let admission = issuer.admission.lock();
        assert_eq!(admission.records.len(), 2);
        assert!(admission.in_flight.is_empty());
        assert_eq!(admission.tracked_entries(), 2);
        assert_eq!(
            admission.accounted_bytes(),
            2 * ADMITTED_OPERATION_ACCOUNTED_BYTES
        );
    }
    #[test]
    fn byte_capacity_bounds_stalled_reservations_and_releases_on_drop() {
        let issuer = submission_test_issuer_with_limits(32, ADMITTED_OPERATION_ACCOUNTED_BYTES);
        let first = submission_test_request(0x11);
        let first_leader = claim_test_leader(&issuer, &first);
        let error = match issuer
            .claim_submission(submission_test_binding(&submission_test_request(0x12)))
        {
            Err(error) => error,
            Ok(_) => panic!("the byte budget permits only one reservation"),
        };
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "offline_operation_capacity_exhausted",
                ..
            }
        ));
        drop(first_leader);
        let replacement = claim_test_leader(&issuer, &submission_test_request(0x12));
        assert_eq!(issuer.admission.lock().tracked_entries(), 1);
        drop(replacement);
        assert_eq!(issuer.admission.lock().tracked_entries(), 0);
    }
    #[test]
    fn concurrent_unique_claims_cannot_overbook_admission_capacity() {
        const CAPACITY: usize = 8;
        const ATTEMPTS: usize = 32;
        let issuer = submission_test_issuer_with_limits(
            CAPACITY,
            CAPACITY * ADMITTED_OPERATION_ACCOUNTED_BYTES,
        );
        let barrier = Arc::new(Barrier::new(ATTEMPTS));
        let mut handles = Vec::with_capacity(ATTEMPTS);
        for index in 0..ATTEMPTS {
            let claim_issuer = Arc::clone(&issuer);
            let claim_barrier = Arc::clone(&barrier);
            let request =
                submission_test_request(u8::try_from(index + 0x40).expect("fixture seed fits u8"));
            handles.push(std::thread::spawn(move || {
                let binding = submission_test_binding(&request);
                claim_barrier.wait();
                claim_issuer.claim_submission(binding)
            }));
        }
        let mut leaders = Vec::with_capacity(CAPACITY);
        let mut rejected = 0;
        for handle in handles {
            match handle.join().expect("claim thread must not panic") {
                Ok(SubmissionClaim::Leader(leader)) => leaders.push(leader),
                Err(Error::AppServiceUnavailable {
                    code: "offline_operation_capacity_exhausted",
                    ..
                }) => rejected += 1,
                Ok(SubmissionClaim::Accepted(_) | SubmissionClaim::Follower(_)) => {
                    panic!("unique operation ids cannot be replays")
                }
                Err(error) => panic!("unexpected claim error: {error:?}"),
            }
        }
        assert_eq!(leaders.len(), CAPACITY);
        assert_eq!(rejected, ATTEMPTS - CAPACITY);
        {
            let admission = issuer.admission.lock();
            assert_eq!(admission.tracked_entries(), CAPACITY);
            assert_eq!(admission.in_flight.len(), CAPACITY);
            assert_eq!(
                admission.accounted_bytes(),
                CAPACITY * ADMITTED_OPERATION_ACCOUNTED_BYTES
            );
        }
        drop(leaders);
        assert_eq!(issuer.admission.lock().tracked_entries(), 0);
    }
    #[test]
    fn admission_registry_prunes_only_past_retention_and_never_capacity_evicts() {
        let mut registry = OfflineOperationRegistry::new(
            NonZeroUsize::new(1).expect("positive count"),
            NonZeroUsize::new(ADMITTED_OPERATION_ACCOUNTED_BYTES).expect("positive bytes"),
        );
        let expires_at = 10;
        registry.insert_reserved(admitted_record_fixture(0x31, 1, expires_at));
        let retained_until = expires_at + OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS;
        registry.prune_expired(retained_until);
        assert!(registry.get(&[0x31; 32]).is_some());
        assert!(!registry.has_capacity_for_new_operation());
        registry.prune_expired(retained_until + 1);
        assert!(registry.records.is_empty());
        assert!(registry.has_capacity_for_new_operation());
    }
    #[test]
    fn retention_pruning_leaves_queue_and_kura_records_authoritative_for_conflicts() {
        let issuer = submission_test_issuer();
        let mut original = submission_test_request(0x35);
        original.authorization.expires_at_ms = 10;
        let operation_id = original.authorization.operation_id;
        let transaction = submission_test_transaction(vec![original.clone()]);
        let recovered =
            offline_operation_record_in_transaction(&transaction, &issuer.authority, operation_id)
                .expect("authoritative transaction retains the complete request");
        let mut registry = OfflineOperationRegistry::new(
            NonZeroUsize::new(1).expect("positive count"),
            NonZeroUsize::new(ADMITTED_OPERATION_ACCOUNTED_BYTES).expect("positive bytes"),
        );
        let admitted = AdmittedOfflineOperationRecord {
            binding: recovered.binding(),
            transaction_hash: recovered.transaction_hash,
        };
        registry.insert_reserved(admitted);
        registry.prune_expired(10 + OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS + 1);
        assert!(
            registry.get(&operation_id).is_none(),
            "fixture prunes only after the normative retention window"
        );
        ensure_same_offline_request(
            &recovered.request.as_ref(),
            &OfflineOperationRequest::TopUp(&original),
        )
        .expect("identical authoritative replay remains idempotent");
        let mut conflicting = original;
        conflicting
            .artifact_binding
            .generation
            .push_str("-conflict");
        assert!(matches!(
            ensure_same_offline_request(
                &recovered.request.as_ref(),
                &OfflineOperationRequest::TopUp(&conflicting),
            ),
            Err(Error::AppConflict {
                code: "operation_id_conflict",
                ..
            })
        ));
    }
    #[test]
    fn pending_queue_recovery_requires_binding_but_allows_newer_transaction() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x37);
        let transaction = submission_test_transaction(vec![request]);
        let recovered =
            offline_operation_record_in_transaction(&transaction, &issuer.authority, [0x37; 32])
                .expect("recover admitted fixture");
        let matching = AdmittedOfflineOperationRecord {
            binding: recovered.binding(),
            transaction_hash: recovered.transaction_hash,
        };
        ensure_admitted_operation_binding_matches_recovered_record(&matching, &recovered)
            .expect("matching authoritative record");
        let mut wrong_digest = matching.clone();
        wrong_digest.binding.canonical_request_digest[0] ^= 1;
        let mut wrong_hash = matching;
        wrong_hash.transaction_hash = submission_test_hash(0x38);
        let error =
            ensure_admitted_operation_binding_matches_recovered_record(&wrong_digest, &recovered)
                .expect_err("a mismatched request binding must fail closed");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "offline_operation_evidence_inconsistent",
                ..
            }
        ));
        ensure_admitted_operation_binding_matches_recovered_record(&wrong_hash, &recovered)
            .expect("the same request may advance to a newer authoritative Queue transaction");
    }
    #[test]
    fn applied_kagemusha_v4_finality_preserves_requested_operation_id() {
        let operation_id = [0x5A; 32];
        let finality =
            kagemusha_v4_applied_finality(operation_id, "transaction-hash".to_owned(), 7);
        assert_eq!(finality.operation_id, operation_id);
        assert_eq!(finality.transaction_hash, "transaction-hash");
        assert_eq!(finality.finalized_block_height, 7);
        assert_eq!(finality.outcome, KagemushaV2TerminalOutcome::Applied);
    }
    #[tokio::test]
    async fn canonical_operation_references_and_applied_redeem_status_preserve_operation_id() {
        let operation_id = [0x5B; 32];
        let transaction_hash = submission_test_hash(0x6B).to_string();
        let make_reference = || {
            crate::utils::with_current_response_format(crate::utils::ResponseFormat::Json, async {
                offline_operation_reference_response(
                    operation_id,
                    OfflineOperationKind::Redeem,
                    transaction_hash.clone(),
                    17,
                )
                .expect("build accepted operation reference")
            })
        };
        let first = make_reference().await;
        let replay = make_reference().await;
        assert_eq!(first.status(), axum::http::StatusCode::ACCEPTED);
        assert_eq!(replay.status(), axum::http::StatusCode::ACCEPTED);
        for response in [&first, &replay] {
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::LOCATION)
                    .and_then(|value| value.to_str().ok()),
                Some(offline_operation_status_uri(operation_id).as_str())
            );
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::CACHE_CONTROL)
                    .and_then(|value| value.to_str().ok()),
                Some("no-store")
            );
        }
        let first_json = response_json(first).await;
        let replay_json = response_json(replay).await;
        assert_eq!(
            first_json, replay_json,
            "an exact replay must return the same operation resource"
        );
        assert_eq!(
            first_json
                .get("operation_id")
                .and_then(norito::json::Value::as_str),
            Some(hex::encode(operation_id).as_str())
        );
        assert_eq!(
            first_json
                .get("status_uri")
                .and_then(norito::json::Value::as_str),
            Some(offline_operation_status_uri(operation_id).as_str())
        );
        let applied =
            crate::utils::with_current_response_format(crate::utils::ResponseFormat::Json, async {
                respond_with_offline_operation_status(OfflineOperationStatus::Applied {
                    operation_id: hex::encode(operation_id),
                    result: OfflineOperationResult::Redeem(OfflineRedeemResult {
                        transaction_hash: transaction_hash.clone(),
                        finalized_block_height: 23,
                    }),
                })
            })
            .await;
        assert!(
            applied
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .is_none(),
            "a terminal operation must never tell clients to keep polling"
        );
        let applied_json = response_json(applied).await;
        assert_eq!(
            applied_json
                .get("state")
                .and_then(norito::json::Value::as_str),
            Some("applied")
        );
        assert_eq!(
            applied_json
                .get("value")
                .and_then(norito::json::Value::as_object)
                .and_then(|value| value.get("operation_id"))
                .and_then(norito::json::Value::as_str),
            Some(hex::encode(operation_id).as_str())
        );
        let applied_json_text =
            norito::json::to_string(&applied_json).expect("response JSON must re-serialize");
        assert!(
            !applied_json_text.contains("topup_anchor"),
            "an applied redeem result must not regain an unused top-up field"
        );
    }
    #[test]
    fn terminal_finality_requires_nonzero_exact_identity_hash_and_height() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x2A);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let record =
            offline_operation_record_in_transaction(&transaction, &issuer.authority, operation_id)
                .expect("fixture operation must be recoverable");
        let matching =
            kagemusha_v4_applied_finality(operation_id, record.transaction_hash.to_string(), 41);
        ensure_kagemusha_v4_terminal_finality_matches_record(&record, &matching)
            .expect("matching canonical finality");
        let rejected = kagemusha_v4_committed_finality(
            operation_id,
            record.transaction_hash.to_string(),
            41,
            Some("canonical rejection".to_owned()),
        );
        ensure_kagemusha_v4_terminal_finality_matches_record(&record, &rejected)
            .expect("a canonical rejection is terminal finality, not incomplete finality");
        for (label, finality) in [
            (
                "zero operation identity",
                KagemushaV2CommittedFinality {
                    operation_id: [0; 32],
                    ..matching.clone()
                },
            ),
            (
                "operation identity",
                KagemushaV2CommittedFinality {
                    operation_id: [0x2B; 32],
                    ..matching.clone()
                },
            ),
            (
                "transaction hash",
                KagemushaV2CommittedFinality {
                    transaction_hash: submission_test_hash(0x2C).to_string(),
                    ..matching.clone()
                },
            ),
            (
                "block height",
                KagemushaV2CommittedFinality {
                    finalized_block_height: 0,
                    ..matching.clone()
                },
            ),
        ] {
            let error = ensure_kagemusha_v4_terminal_finality_matches_record(&record, &finality)
                .expect_err("terminal mismatch must fail closed");
            match &error {
                Error::AppServiceUnavailable { code, .. } => {
                    assert_eq!(*code, "offline_operation_evidence_inconsistent", "{label}");
                }
                other => panic!("{label} returned the wrong error class: {other:?}"),
            }
            assert_eq!(
                error.into_response().status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "{label}"
            );
        }
    }
    #[tokio::test]
    async fn rejected_and_expired_pipeline_entries_are_terminal_without_retry() {
        let operation_id = [0x2F; 32];
        let transaction_hash = submission_test_hash(0x3F);
        for kind in [
            crate::PipelineStatusKind::Rejected,
            crate::PipelineStatusKind::Expired,
        ] {
            let entry = crate::PipelineStatusEntry::fresh(kind, None, None);
            let status = terminal_rejected_or_expired_offline_operation_status(
                &entry,
                operation_id,
                KagemushaV2OperationKind::Redeem,
                &transaction_hash,
            )
            .expect("rejected and expired cache entries are terminal");
            let response = crate::utils::with_current_response_format(
                crate::utils::ResponseFormat::Json,
                async { respond_with_offline_operation_status(status) },
            )
            .await;
            assert_eq!(
                response
                    .headers()
                    .get(axum::http::header::CACHE_CONTROL)
                    .and_then(|value| value.to_str().ok()),
                Some("no-store")
            );
            assert!(
                response
                    .headers()
                    .get(axum::http::header::RETRY_AFTER)
                    .is_none(),
                "{kind:?} must resolve immediately instead of masquerading as pending"
            );
            let value = response_json(response).await;
            assert_eq!(
                value.get("state").and_then(norito::json::Value::as_str),
                Some("rejected"),
                "{kind:?}"
            );
            assert_eq!(
                value
                    .get("value")
                    .and_then(norito::json::Value::as_object)
                    .and_then(|value| value.get("operation_id"))
                    .and_then(norito::json::Value::as_str),
                Some(hex::encode(operation_id).as_str()),
                "{kind:?}"
            );
        }
        for kind in [
            crate::PipelineStatusKind::Queued,
            crate::PipelineStatusKind::Approved,
            crate::PipelineStatusKind::Committed,
            crate::PipelineStatusKind::Applied,
        ] {
            let entry = crate::PipelineStatusEntry::fresh(kind, None, None);
            assert!(
                terminal_rejected_or_expired_offline_operation_status(
                    &entry,
                    operation_id,
                    KagemushaV2OperationKind::Redeem,
                    &transaction_hash,
                )
                .is_none(),
                "{kind:?} must not be synthesized as a rejection"
            );
        }
    }
    #[test]
    fn unproven_pending_state_fails_closed_after_signed_expiry() {
        ensure_unproven_pending_window_is_live(9_999, 10_000)
            .expect("a pre-expiry operation may still acquire authoritative provenance");
        for snapshot_time_ms in [10_000, 10_001] {
            let error = ensure_unproven_pending_window_is_live(snapshot_time_ms, 10_000)
                .expect_err(
                    "an operation at or after its exclusive expiry must not remain pending",
                );
            assert!(matches!(
                &error,
                Error::AppServiceUnavailable {
                    code: "offline_operation_evidence_inconsistent",
                    ..
                }
            ));
            assert_eq!(
                error.into_response().status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
        }
    }
    #[test]
    fn kagemusha_v4_anchor_finality_binding_rejects_identity_hash_or_height_mismatch() {
        let operation_id = [0x31; 32];
        let transaction_hash = submission_test_hash(0x73);
        let anchor_transaction_hash = *transaction_hash.as_ref();
        ensure_kagemusha_v4_anchor_finality_binding(
            operation_id,
            anchor_transaction_hash,
            42,
            operation_id,
            &transaction_hash,
            42,
        )
        .expect("matching anchor and finality");
        for (case, anchor_operation_id, anchor_hash, anchor_height, finalized_height) in [
            (
                "operation id mismatch",
                [0x32; 32],
                anchor_transaction_hash,
                42,
                42,
            ),
            (
                "transaction hash mismatch",
                operation_id,
                [0x75; 32],
                42,
                42,
            ),
            (
                "height mismatch",
                operation_id,
                anchor_transaction_hash,
                43,
                42,
            ),
            (
                "zero finality height",
                operation_id,
                anchor_transaction_hash,
                0,
                0,
            ),
        ] {
            let result = ensure_kagemusha_v4_anchor_finality_binding(
                anchor_operation_id,
                anchor_hash,
                anchor_height,
                operation_id,
                &transaction_hash,
                finalized_height,
            );
            let error = match result {
                Err(error) => error,
                Ok(()) => panic!("{case} must fail closed"),
            };
            match &error {
                Error::AppServiceUnavailable { code, .. } => {
                    assert_eq!(*code, "offline_operation_evidence_inconsistent");
                }
                other => panic!("anchor mismatch returned the wrong error class: {other:?}"),
            }
            assert_eq!(
                error.into_response().status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
        }
        let zero_transaction_hash = submission_test_hash(0);
        let error = ensure_kagemusha_v4_anchor_finality_binding(
            [0; 32],
            [0; 32],
            42,
            [0; 32],
            &zero_transaction_hash,
            42,
        )
        .expect_err("matching all-zero identities must still fail closed");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "offline_operation_evidence_inconsistent",
                ..
            }
        ));
    }
    #[test]
    fn kagemusha_v4_anchor_request_binding_rejects_every_shield_field_drift() {
        let request = submission_test_request(0x34);
        request
            .validate_public_binding()
            .expect("fixture request must be independently valid");
        let anchor = finalized_topup_anchor_for_request(&request);
        ensure_kagemusha_v4_topup_anchor_matches_request(&anchor, &request)
            .expect("exact finalized anchor must bind its admitted request");

        let mut wrong_initial_root = anchor.clone();
        wrong_initial_root.initial_root = [0x71; 32];
        let mut wrong_finalized_root = anchor.clone();
        wrong_finalized_root.finalized_root = [0x72; 32];
        let mut wrong_leaf_index = anchor.clone();
        wrong_leaf_index.shield_leaf_index = 1;
        let mut wrong_verifier_id = anchor.clone();
        wrong_verifier_id.shield_verifier_id =
            VerifyingKeyId::new("halo2/ipa", "kagemusha-topup-shield-v2-alternate");
        let mut wrong_verifier_commitment = anchor;
        wrong_verifier_commitment.shield_verifier_commitment = [0x73; 32];

        for (case, mutated) in [
            ("initial root", wrong_initial_root),
            ("finalized root", wrong_finalized_root),
            ("shield leaf index", wrong_leaf_index),
            ("shield verifier id", wrong_verifier_id),
            ("shield verifier commitment", wrong_verifier_commitment),
        ] {
            let mutated = mutated
                .finalize_digest()
                .unwrap_or_else(|err| panic!("{case} mutation must remain self-valid: {err}"));
            let error = ensure_kagemusha_v4_topup_anchor_matches_request(&mutated, &request)
                .err()
                .unwrap_or_else(|| panic!("{case} drift must fail closed"));
            assert!(matches!(
                error,
                Error::AppServiceUnavailable {
                    code: "offline_operation_evidence_inconsistent",
                    ..
                }
            ));
        }
    }
    #[test]
    fn authorization_refresh_cannot_alias_a_second_anchor_or_bypass_exact_replay() {
        let original = submission_test_request(0x33);
        let mut refreshed = original.clone();
        refreshed.authorization.issued_at_ms = 2;
        refreshed.authorization.expires_at_ms =
            2_u64.saturating_add(KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2);
        refresh_submission_test_authorization(&mut refreshed);
        assert_eq!(
            kagemusha_v4_anchor_state_key(original.authorization.operation_id)
                .expect("original anchor key"),
            kagemusha_v4_anchor_state_key(refreshed.authorization.operation_id)
                .expect("refreshed anchor key"),
            "one operation id has exactly one direct canonical anchor key"
        );
        assert!(
            kagemusha_v4_anchor_state_key([0; 32]).is_err(),
            "the all-zero operation id must never address chain state"
        );
        let original_binding = submission_test_binding(&original);
        let refreshed_binding = submission_test_binding(&refreshed);
        assert_eq!(
            original_binding.operation_id,
            refreshed_binding.operation_id
        );
        assert_ne!(
            original_binding.canonical_request_digest, refreshed_binding.canonical_request_digest,
            "changing the signed authorization window changes the exact request binding"
        );
        assert!(matches!(
            ensure_same_offline_request_binding(&original_binding, &refreshed_binding),
            Err(Error::AppConflict {
                code: "operation_id_conflict",
                ..
            })
        ));
    }
    #[test]
    fn kagemusha_v4_rejection_detail_formats_borrowed_reason() {
        assert_eq!(kagemusha_v4_rejection_detail(None), "no rejection reason");
        let rejection = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "fixture rejection".to_owned(),
        ));
        assert_eq!(
            kagemusha_v4_rejection_detail(Some(&rejection)),
            "Transaction validation failed."
        );
        let adversarial = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "attacker-controlled\nmessage".to_owned(),
        ));
        let message = kagemusha_v4_rejection_detail(Some(&adversarial));
        assert_eq!(message, "Transaction validation failed.");
        assert!(!message.contains("attacker-controlled"));
        assert!(!message.contains('\n'));
        assert!(crate::utils::is_valid_error_message(&message));
    }
    #[test]
    fn terminal_rejection_messages_replace_control_characters_before_nesting() {
        const FALLBACK: &str = "The offline operation was rejected.";
        for adversarial in [
            "line\nbreak",
            "carriage\rreturn",
            "tab\tseparated",
            "nul\0byte",
            "next-line\u{85}control",
        ] {
            let message = canonical_offline_rejection_message(adversarial.to_owned());
            assert_eq!(message, FALLBACK, "input={adversarial:?}");
            assert!(crate::utils::is_valid_error_message(&message));
        }
        let finality = kagemusha_v4_committed_finality(
            [0x2D; 32],
            submission_test_hash(0x2E).to_string(),
            47,
            Some("attacker-controlled\nterminal rejection".to_owned()),
        );
        let KagemushaV2TerminalOutcome::Rejected(message) = finality.outcome else {
            panic!("a rejected finality fixture must remain rejected")
        };
        assert_eq!(message, FALLBACK);
        assert!(crate::utils::is_valid_error_message(&message));
    }
}
