//! Canonical Torii command surface for Offline Cash V1.
//!
//! The public request and response values come directly from the data model.
//! Torii validates one exact V1 request, binds its idempotency key to the
//! canonical request digest, and submits the corresponding V1 instruction.
//! No legacy request, status, finality, or compatibility decoding path is
//! reachable from these routes.

use crate::{AppState, Error, SharedAppState, app_auth, routing};
use axum::{http::HeaderMap, response::Response as AxResponse};
use iroha_config::parameters::actual;
use iroha_core::{
    smartcontracts::isi::offline::OfflineCashReserveOperationRecordV1,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{
        InstructionBox,
        offline_cash_v1::{RedeemOfflineCashV1, TopUpOfflineCashV1},
    },
    transaction::{SignedTransaction, TransactionBuilder},
};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_torii_shared::offline_api::{
    OFFLINE_CASH_CHAIN_VERSION_V1, OFFLINE_CASH_OPERATION_STATUS_ROUTE_PREFIX_V1,
    OfflineCashOperationKindV1, OfflineCashOperationRejectionCodeV1,
    OfflineCashOperationRejectionV1, OfflineCashOperationResultV1, OfflineCashOperationStateV1,
    OfflineCashOperationStatusV1, OfflineCashRedemptionRequestV1, OfflineCashRedemptionResultV1,
    OfflineCashTopUpRequestV1,
};
use mv::storage::StorageReadOnly;
use parking_lot::Mutex;
use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc};

const PATH_OFFLINE_TOP_UP: &str = iroha_torii_shared::uri::OFFLINE_TOP_UP;
const PATH_OFFLINE_REDEEM: &str = iroha_torii_shared::uri::OFFLINE_REDEEM;

// The configuration value predates the public V1 DTO cutover, but remains the
// operator-selected bound for the same in-memory idempotency registry.
const OPERATION_ACCOUNTED_BYTES: usize =
    iroha_config::parameters::defaults::torii::offline_cash_v1_commands::OPERATION_REGISTRY_ACCOUNTED_BYTES_PER_ENTRY;

#[derive(Debug, Clone)]
pub(crate) struct OfflineCommandRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    minimum_xor_balance: Quantity,
    max_tx_value: Quantity,
    registry: Arc<Mutex<OfflineOperationRegistry>>,
}

impl OfflineCommandRuntime {
    pub(crate) fn from_config(config: actual::ToriiOfflineCashV1Commands) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            minimum_xor_balance: config.minimum_xor_balance,
            max_tx_value: config.max_tx_value,
            registry: Arc::new(Mutex::new(OfflineOperationRegistry::new(
                config.operation_registry_max_entries,
                config.operation_registry_max_bytes,
            ))),
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

    fn claim(self: &Arc<Self>, binding: OfflineOperationBinding) -> Result<SubmissionClaim, Error> {
        let mut registry = self.registry.lock();
        match registry.entries.get(&binding.operation_id) {
            Some(OfflineOperationEntry::Reserved(existing)) => {
                ensure_same_binding(existing, &binding)?;
                return Err(Error::AppServiceUnavailable {
                    code: "offline_operation_in_flight",
                    message: "The same Offline Cash V1 operation is already being submitted; retry its canonical status resource."
                        .to_owned(),
                });
            }
            Some(OfflineOperationEntry::Admitted(existing)) => {
                ensure_same_binding(&existing.binding, &binding)?;
                return Ok(SubmissionClaim::Existing(existing.clone()));
            }
            None => {}
        }
        if !registry.has_capacity_for_new_operation() {
            return Err(Error::AppServiceUnavailable {
                code: "offline_operation_capacity_exhausted",
                message: "Offline Cash V1 operation admission capacity is exhausted.".to_owned(),
            });
        }
        registry.entries.insert(
            binding.operation_id,
            OfflineOperationEntry::Reserved(binding),
        );
        Ok(SubmissionClaim::Reserved(SubmissionReservation {
            runtime: Arc::clone(self),
            binding,
            active: true,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OfflineOperationBinding {
    operation_id: [u8; 32],
    kind: OfflineCashOperationKindV1,
    request_digest: [u8; 32],
}

#[derive(Debug, Clone)]
struct AdmittedOfflineOperation {
    binding: OfflineOperationBinding,
    transaction_hash: HashOf<SignedTransaction>,
}

#[derive(Debug, Clone)]
enum OfflineOperationEntry {
    Reserved(OfflineOperationBinding),
    Admitted(AdmittedOfflineOperation),
}

#[derive(Debug)]
struct OfflineOperationRegistry {
    entries: BTreeMap<[u8; 32], OfflineOperationEntry>,
    max_entries: NonZeroUsize,
    max_accounted_bytes: NonZeroUsize,
}

impl OfflineOperationRegistry {
    fn new(max_entries: NonZeroUsize, max_accounted_bytes: NonZeroUsize) -> Self {
        Self {
            entries: BTreeMap::new(),
            max_entries,
            max_accounted_bytes,
        }
    }

    fn has_capacity_for_new_operation(&self) -> bool {
        self.entries.len().saturating_add(1) <= self.max_entries.get()
            && self
                .entries
                .len()
                .saturating_add(1)
                .saturating_mul(OPERATION_ACCOUNTED_BYTES)
                <= self.max_accounted_bytes.get()
    }
}

enum SubmissionClaim {
    Existing(AdmittedOfflineOperation),
    Reserved(SubmissionReservation),
}

struct SubmissionReservation {
    runtime: Arc<OfflineCommandRuntime>,
    binding: OfflineOperationBinding,
    active: bool,
}

impl SubmissionReservation {
    fn accept(
        mut self,
        transaction_hash: HashOf<SignedTransaction>,
    ) -> Result<AdmittedOfflineOperation, Error> {
        let admitted = AdmittedOfflineOperation {
            binding: self.binding,
            transaction_hash,
        };
        let mut registry = self.runtime.registry.lock();
        match registry.entries.get(&self.binding.operation_id) {
            Some(OfflineOperationEntry::Reserved(existing)) if existing == &self.binding => {}
            Some(OfflineOperationEntry::Admitted(existing)) => {
                ensure_same_binding(&existing.binding, &self.binding)?;
                self.active = false;
                return Ok(existing.clone());
            }
            _ => {
                return Err(Error::AppServiceUnavailable {
                    code: "offline_operation_admission_inconsistent",
                    message:
                        "The accepted Offline Cash V1 operation lost its admission reservation."
                            .to_owned(),
                });
            }
        }
        registry.entries.insert(
            self.binding.operation_id,
            OfflineOperationEntry::Admitted(admitted.clone()),
        );
        self.active = false;
        Ok(admitted)
    }
}

impl Drop for SubmissionReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut registry = self.runtime.registry.lock();
        if registry
            .entries
            .get(&self.binding.operation_id)
            .is_some_and(|entry| {
                matches!(entry, OfflineOperationEntry::Reserved(existing) if existing == &self.binding)
            })
        {
            registry.entries.remove(&self.binding.operation_id);
        }
    }
}

pub(crate) async fn handle_top_up(
    app: SharedAppState,
    headers: &HeaderMap,
    request: OfflineCashTopUpRequestV1,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, request.operation_id)?;
    request.validate().map_err(|source| {
        validation_owned(
            "offline_top_up_invalid",
            format!("Offline Cash V1 top-up request is invalid: {source}"),
        )
    })?;
    validate_top_up_snapshot(&app, &request)?;
    let binding = OfflineOperationBinding {
        operation_id: request.operation_id,
        kind: OfflineCashOperationKindV1::TopUp,
        request_digest: request.canonical_digest().map_err(|source| {
            validation_owned(
                "offline_top_up_invalid",
                format!("Offline Cash V1 top-up digest is invalid: {source}"),
            )
        })?,
    };
    let issuer = require_configured_issuer(&app)?;
    ensure_amount_within_policy(request.amount, request.scale, &issuer.max_tx_value)?;
    if let Some(existing) = admitted_operation_from_consensus(&app, binding.operation_id)? {
        ensure_same_binding(&existing.binding, &binding)?;
        return operation_response_for_record(&app, &existing);
    }
    match issuer.claim(binding)? {
        SubmissionClaim::Existing(record) => return operation_response_for_record(&app, &record),
        SubmissionClaim::Reserved(reservation) => {
            ensure_offline_command_authority_ready(&app, &issuer)?;
            let instruction = TopUpOfflineCashV1::new(request).map_err(|source| {
                validation_owned(
                    "offline_top_up_invalid",
                    format!("Offline Cash V1 top-up instruction is invalid: {source}"),
                )
            })?;
            let transaction = TransactionBuilder::new(
                *app.state.network_id_ref(),
                issuer.authority.clone().into(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([InstructionBox::from(instruction)]);
            let signed = issuer.quote_and_sign_transaction(
                &app,
                transaction,
                "offline_cash_v1_top_up_transaction",
            )?;
            let transaction_hash = signed.hash();
            routing::handle_transaction_with_metrics(
                app.queue.clone(),
                app.state.clone(),
                signed,
                app.telemetry.clone(),
                PATH_OFFLINE_TOP_UP,
            )
            .await?;
            let record = reservation.accept(transaction_hash)?;
            Ok(respond_with_operation_status(
                pending_status(record.binding),
                true,
            ))
        }
    }
}

pub(crate) async fn handle_redeem(
    app: SharedAppState,
    headers: &HeaderMap,
    request: OfflineCashRedemptionRequestV1,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, request.operation_id)?;
    request.validate().map_err(|source| {
        validation_owned(
            "offline_redeem_invalid",
            format!("Offline Cash V1 redemption request is invalid: {source}"),
        )
    })?;
    validate_redemption_snapshot(&app, &request)?;
    let statement = &request.voucher.statement;
    let binding = OfflineOperationBinding {
        operation_id: request.operation_id,
        kind: OfflineCashOperationKindV1::Redemption,
        request_digest: request.canonical_digest().map_err(|source| {
            validation_owned(
                "offline_redeem_invalid",
                format!("Offline Cash V1 redemption digest is invalid: {source}"),
            )
        })?,
    };
    let issuer = require_configured_issuer(&app)?;
    ensure_amount_within_policy(statement.amount, statement.scale, &issuer.max_tx_value)?;
    if let Some(existing) = admitted_operation_from_consensus(&app, binding.operation_id)? {
        ensure_same_binding(&existing.binding, &binding)?;
        return operation_response_for_record(&app, &existing);
    }
    match issuer.claim(binding)? {
        SubmissionClaim::Existing(record) => return operation_response_for_record(&app, &record),
        SubmissionClaim::Reserved(reservation) => {
            ensure_offline_command_authority_ready(&app, &issuer)?;
            let instruction = RedeemOfflineCashV1::new(request).map_err(|source| {
                validation_owned(
                    "offline_redeem_invalid",
                    format!("Offline Cash V1 redemption instruction is invalid: {source}"),
                )
            })?;
            let transaction = TransactionBuilder::new(
                *app.state.network_id_ref(),
                issuer.authority.clone().into(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([InstructionBox::from(instruction)]);
            let signed = issuer.quote_and_sign_transaction(
                &app,
                transaction,
                "offline_cash_v1_redemption_transaction",
            )?;
            let transaction_hash = signed.hash();
            routing::handle_transaction_with_metrics(
                app.queue.clone(),
                app.state.clone(),
                signed,
                app.telemetry.clone(),
                PATH_OFFLINE_REDEEM,
            )
            .await?;
            let record = reservation.accept(transaction_hash)?;
            Ok(respond_with_operation_status(
                pending_status(record.binding),
                true,
            ))
        }
    }
}

pub(crate) fn handle_operation_status(
    app: &SharedAppState,
    operation_id: &str,
) -> Result<AxResponse, Error> {
    let operation_id = parse_operation_id(operation_id)?;
    let issuer = require_configured_issuer(app)?;
    let local_record = {
        let registry = issuer.registry.lock();
        match registry.entries.get(&operation_id) {
            Some(OfflineOperationEntry::Admitted(record)) => Some(record.clone()),
            Some(OfflineOperationEntry::Reserved(_)) => {
                return Err(Error::AppServiceUnavailable {
                    code: "offline_operation_in_flight",
                    message:
                        "The Offline Cash V1 operation is still entering the transaction queue."
                            .to_owned(),
                });
            }
            None => None,
        }
    };
    let record = match local_record {
        Some(record) => record,
        None => admitted_operation_from_consensus(app, operation_id)?.ok_or_else(|| {
            Error::AppNotFound {
                code: "offline_operation_not_found",
                message: "The Offline Cash V1 operation is unknown on this Torii node.".to_owned(),
            }
        })?,
    };
    operation_response_for_record(app, &record)
}

fn operation_response_for_record(
    app: &SharedAppState,
    record: &AdmittedOfflineOperation,
) -> Result<AxResponse, Error> {
    let status = match crate::pipeline_status_local_entry_checked(app, &record.transaction_hash)? {
        Some((entry, resolved_from)) => match entry.kind {
            crate::PipelineStatusKind::Applied if resolved_from != "state" => {
                pending_status(record.binding)
            }
            crate::PipelineStatusKind::Applied => {
                let height = entry.block_height.ok_or_else(|| {
                    offline_consensus_inconsistency(
                        "state-resolved Applied operation has no canonical block height",
                    )
                })?;
                match applied_status_from_consensus(app, record, height.get())? {
                    Some(status) => status,
                    None => pending_status(record.binding),
                }
            }
            crate::PipelineStatusKind::Rejected | crate::PipelineStatusKind::Expired => {
                rejected_status(
                    record.binding,
                    rejection_code(entry.kind, entry.rejection),
                    entry.rejection.unwrap_or(entry.kind.as_str()),
                )
            }
            crate::PipelineStatusKind::Queued
            | crate::PipelineStatusKind::Approved
            | crate::PipelineStatusKind::Committed => pending_status(record.binding),
        },
        None => pending_status(record.binding),
    };
    Ok(respond_with_operation_status(status, false))
}

fn admitted_operation_from_consensus(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<Option<AdmittedOfflineOperation>, Error> {
    let operation = {
        let state = app.state.view();
        state
            .world()
            .offline_cash_reserve_operations()
            .get(&operation_id)
            .cloned()
    };
    let Some(operation) = operation else {
        return Ok(None);
    };
    let (kind, request_digest) = match &operation {
        OfflineCashReserveOperationRecordV1::TopUp(record) => {
            record.issuance_intent.request.validate().map_err(|error| {
                offline_consensus_inconsistency(format!(
                    "persisted top-up request is invalid: {error}"
                ))
            })?;
            (
                OfflineCashOperationKindV1::TopUp,
                record.issuance_intent.request_digest,
            )
        }
        OfflineCashReserveOperationRecordV1::Redemption(record) => {
            record
                .redemption_request
                .validate_shape()
                .map_err(|error| {
                    offline_consensus_inconsistency(format!(
                        "persisted redemption request is invalid: {error}"
                    ))
                })?;
            (
                OfflineCashOperationKindV1::Redemption,
                record.request_digest,
            )
        }
    };
    let receipt = operation.reserve_receipt();
    receipt.validate().map_err(|error| {
        offline_consensus_inconsistency(format!("persisted reserve receipt is invalid: {error}"))
    })?;
    if operation.operation_id() != operation_id
        || receipt.operation_id != operation_id
        || receipt.kind != kind
        || receipt.request_digest != request_digest
    {
        return Err(offline_consensus_inconsistency(
            "persisted reserve operation identity is internally inconsistent",
        ));
    }
    Ok(Some(AdmittedOfflineOperation {
        binding: OfflineOperationBinding {
            operation_id,
            kind,
            request_digest,
        },
        transaction_hash: HashOf::from_untyped_unchecked(Hash::prehashed(receipt.transaction_hash)),
    }))
}

fn applied_status_from_consensus(
    app: &SharedAppState,
    admitted: &AdmittedOfflineOperation,
    height: u64,
) -> Result<Option<OfflineCashOperationStatusV1>, Error> {
    let operation = {
        let state = app.state.view();
        state
            .world()
            .offline_cash_reserve_operations()
            .get(&admitted.binding.operation_id)
            .cloned()
    }
    .ok_or_else(|| {
        offline_consensus_inconsistency(
            "state-resolved transaction has no Offline Cash V1 reserve operation",
        )
    })?;
    let canonical = admitted_operation_from_consensus(app, admitted.binding.operation_id)?
        .ok_or_else(|| {
            offline_consensus_inconsistency(
                "persisted Offline Cash V1 reserve operation disappeared during lookup",
            )
        })?;
    ensure_same_binding(&canonical.binding, &admitted.binding)?;
    if canonical.transaction_hash != admitted.transaction_hash {
        return Err(offline_consensus_inconsistency(
            "reserve receipt transaction hash differs from the admitted operation",
        ));
    }
    let finality = app
        .kura
        .offline_cash_operation_finality_v1(height, admitted.binding.operation_id)
        .map_err(|error| {
            offline_consensus_inconsistency(format!(
                "canonical reserve-receipt finality lookup failed: {error}"
            ))
        })?;
    let Some(finality) = finality else {
        let artifact_exists = app.kura.v2_finality_artifact(height).map_err(|error| {
            offline_consensus_inconsistency(format!(
                "canonical finality lookup failed while resolving reserve receipt: {error}"
            ))
        })?;
        if artifact_exists.is_some() {
            return Err(offline_consensus_inconsistency(
                "canonical finality exists without the applied reserve receipt witness",
            ));
        }
        return Ok(None);
    };
    if finality.reserve_receipt_witness.receipt != *operation.reserve_receipt()
        || finality.finality_artifact.height != height
    {
        return Err(offline_consensus_inconsistency(
            "Kura finality does not match the persisted reserve receipt or height",
        ));
    }
    let result = match operation {
        OfflineCashReserveOperationRecordV1::TopUp(record) => {
            if admitted.binding.kind != OfflineCashOperationKindV1::TopUp
                || admitted.binding.request_digest != record.issuance_intent.request_digest
            {
                return Err(offline_consensus_inconsistency(
                    "top-up operation kind or request digest differs from admission",
                ));
            }
            let Some(result) = app
                .kura
                .offline_cash_mint_outbox_entry_v1(admitted.binding.operation_id)
                .map_err(|error| {
                    offline_consensus_inconsistency(format!(
                        "canonical mint outbox lookup failed: {error}"
                    ))
                })?
            else {
                return Ok(None);
            };
            if result.request != record.issuance_intent.request || result.finality != finality {
                return Err(offline_consensus_inconsistency(
                    "mint outbox result differs from the consensus top-up record",
                ));
            }
            OfflineCashOperationResultV1::TopUp(result)
        }
        OfflineCashReserveOperationRecordV1::Redemption(record) => {
            if admitted.binding.kind != OfflineCashOperationKindV1::Redemption
                || admitted.binding.request_digest != record.request_digest
            {
                return Err(offline_consensus_inconsistency(
                    "redemption operation kind or request digest differs from admission",
                ));
            }
            OfflineCashOperationResultV1::Redemption(OfflineCashRedemptionResultV1 {
                version: OFFLINE_CASH_CHAIN_VERSION_V1,
                request: record.redemption_request,
                finality,
            })
        }
    };
    let status = OfflineCashOperationStatusV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: admitted.binding.operation_id,
        kind: admitted.binding.kind,
        state: OfflineCashOperationStateV1::Applied,
        result: Some(result),
        rejection: None,
    };
    let finality = match status.result.as_ref().expect("Applied status has a result") {
        OfflineCashOperationResultV1::TopUp(result) => &result.finality,
        OfflineCashOperationResultV1::Redemption(result) => &result.finality,
    };
    let anchor = iroha_torii_shared::offline_api::OfflineCashFinalityTrustAnchorV1 {
        network_id: finality.finality_artifact.height_context.network_id,
        block_height: finality.finality_artifact.height,
        height_context_id: finality.finality_artifact.context_id(),
    };
    status.validate_against(&anchor).map_err(|error| {
        offline_consensus_inconsistency(format!(
            "constructed Applied status failed canonical validation: {error}"
        ))
    })?;
    Ok(Some(status))
}

fn offline_consensus_inconsistency(detail: impl std::fmt::Display) -> Error {
    iroha_logger::error!(%detail, "Offline Cash V1 consensus projection is inconsistent");
    Error::AppServiceUnavailable {
        code: "offline_operation_consensus_inconsistent",
        message: "Canonical Offline Cash V1 operation evidence is unavailable or inconsistent."
            .to_owned(),
    }
}

fn pending_status(binding: OfflineOperationBinding) -> OfflineCashOperationStatusV1 {
    OfflineCashOperationStatusV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: binding.operation_id,
        kind: binding.kind,
        state: OfflineCashOperationStateV1::Pending,
        result: None,
        rejection: None,
    }
}

fn rejected_status(
    binding: OfflineOperationBinding,
    code: OfflineCashOperationRejectionCodeV1,
    detail: &str,
) -> OfflineCashOperationStatusV1 {
    let mut hasher = blake3::Hasher::new_derive_key("iroha.offline-cash.v1.rejection-detail");
    hasher.update(detail.as_bytes());
    OfflineCashOperationStatusV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: binding.operation_id,
        kind: binding.kind,
        state: OfflineCashOperationStateV1::Rejected,
        result: None,
        rejection: Some(OfflineCashOperationRejectionV1 {
            code,
            detail_digest: *hasher.finalize().as_bytes(),
        }),
    }
}

fn rejection_code(
    kind: crate::PipelineStatusKind,
    detail: Option<&'static str>,
) -> OfflineCashOperationRejectionCodeV1 {
    if kind == crate::PipelineStatusKind::Expired {
        return OfflineCashOperationRejectionCodeV1::InvalidRequest;
    }
    match detail {
        Some("Account does not exist.") => OfflineCashOperationRejectionCodeV1::Unauthorized,
        Some("Transaction limits were exceeded.") | Some("Transaction validation failed.") => {
            OfflineCashOperationRejectionCodeV1::InvalidRequest
        }
        Some("Instruction execution failed.")
        | Some("IVM execution failed.")
        | Some("Trigger execution failed.")
        | None
        | Some(_) => OfflineCashOperationRejectionCodeV1::InternalFailure,
    }
}

fn respond_with_operation_status(
    status: OfflineCashOperationStatusV1,
    accepted_submission: bool,
) -> AxResponse {
    let pending = status.state == OfflineCashOperationStateV1::Pending;
    let operation_id = status.operation_id;
    let status_code = if accepted_submission && pending {
        axum::http::StatusCode::ACCEPTED
    } else {
        axum::http::StatusCode::OK
    };
    let mut response = crate::utils::respond_with_status_and_format(
        status_code,
        status,
        crate::utils::current_response_format(),
    );
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
    if accepted_submission {
        let location = format!(
            "{OFFLINE_CASH_OPERATION_STATUS_ROUTE_PREFIX_V1}{}",
            hex::encode(operation_id)
        );
        if let Ok(value) = axum::http::HeaderValue::from_str(&location) {
            response
                .headers_mut()
                .insert(axum::http::header::LOCATION, value);
        }
    }
    response
}

fn ensure_same_binding(
    existing: &OfflineOperationBinding,
    requested: &OfflineOperationBinding,
) -> Result<(), Error> {
    if existing == requested {
        return Ok(());
    }
    Err(Error::AppConflict {
        code: "operation_id_conflict",
        message: "Offline Cash V1 operation id is already bound to a different canonical request."
            .to_owned(),
    })
}

fn validate_top_up_snapshot(
    app: &SharedAppState,
    request: &OfflineCashTopUpRequestV1,
) -> Result<(), Error> {
    if request.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "offline_wrong_network",
            "Offline Cash V1 top-up targets a different network.",
        ));
    }
    validate_live_asset_scale(app, &request.asset, request.scale)
}

fn validate_redemption_snapshot(
    app: &SharedAppState,
    request: &OfflineCashRedemptionRequestV1,
) -> Result<(), Error> {
    let statement = &request.voucher.statement;
    if statement.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "offline_wrong_network",
            "Offline Cash V1 redemption targets a different network.",
        ));
    }
    validate_live_asset_scale(app, &statement.asset, statement.scale)
}

fn validate_live_asset_scale(
    app: &SharedAppState,
    asset: &AssetDefinitionId,
    requested_scale: u32,
) -> Result<(), Error> {
    let state = app.state.view();
    let definition = state.world().asset_definition(asset).map_err(|_| {
        validation(
            "offline_asset_not_found",
            "Offline Cash V1 asset definition is not registered.",
        )
    })?;
    let live_scale = definition.spec().scale().ok_or_else(|| {
        validation(
            "offline_asset_scale_invalid",
            "Offline Cash V1 requires a fixed live asset scale.",
        )
    })?;
    if requested_scale != live_scale {
        return Err(validation(
            "offline_asset_scale_mismatch",
            "Offline Cash V1 request scale differs from the live asset scale.",
        ));
    }
    Ok(())
}

fn ensure_amount_within_policy(
    atomic_units: u128,
    scale: u32,
    maximum: &Quantity,
) -> Result<(), Error> {
    let numeric = Numeric::try_new(atomic_units, scale).map_err(|source| {
        validation_owned(
            "offline_amount_invalid",
            format!("Offline Cash V1 amount is not canonical: {source}"),
        )
    })?;
    let amount = Quantity::try_from_numeric(numeric).map_err(|source| {
        validation_owned(
            "offline_amount_invalid",
            format!("Offline Cash V1 amount is not a quantity: {source}"),
        )
    })?;
    if &amount > maximum {
        return Err(validation(
            "offline_amount_exceeds_limit",
            "Offline Cash V1 amount exceeds issuer policy.",
        ));
    }
    Ok(())
}

fn require_configured_issuer(app: &AppState) -> Result<Arc<OfflineCommandRuntime>, Error> {
    app.offline_commands
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_service_unavailable",
            message: "Offline Cash V1 operation signing is not configured on this Torii node."
                .to_owned(),
        })
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
        snapshot_time_ms(&state),
    )
}

pub(super) fn ensure_offline_command_authority_ready_in_world(
    world: &impl WorldReadOnly,
    issuer: &OfflineCommandRuntime,
    fee_asset_selector: &str,
    snapshot_time_ms: u64,
) -> Result<(), Error> {
    if world.account(&issuer.authority).is_err()
        || !iroha_core::smartcontracts::isi::offline::isi::world_has_offline_reserve_manager_permission(
            world,
            &issuer.authority,
        )
    {
        return Err(Error::AppServiceUnavailable {
            code: "offline_command_authority_not_ready",
            message: "Offline Cash V1 command authority is not registered with CanManageOfflineReserve."
                .to_owned(),
        });
    }
    let fee_asset_definition =
        routing::resolve_asset_definition_selector(world, fee_asset_selector, snapshot_time_ms)
            .map_err(|error| {
                iroha_logger::error!(
                    ?error,
                    %fee_asset_selector,
                    "Offline Cash V1 command fee asset could not be resolved"
                );
                Error::AppServiceUnavailable {
                    code: "offline_command_fee_asset_not_ready",
                    message: "Offline Cash V1 command fee asset is not available.".to_owned(),
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
            message: "Offline Cash V1 command authority does not meet its configured minimum fee balance."
                .to_owned(),
        });
    }
    Ok(())
}

fn snapshot_time_ms(state: &impl StateReadOnly) -> u64 {
    state.latest_block().map_or(0, |block| {
        u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    })
}

fn require_idempotency_key(headers: &HeaderMap, operation_id: [u8; 32]) -> Result<(), Error> {
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "Offline Cash V1 operation id must be non-zero.".to_owned(),
        });
    }
    let expected = hex::encode(operation_id);
    if validated_idempotency_key(headers)? != expected {
        return Err(Error::AppConflict {
            code: "idempotency_key_conflict",
            message: "Idempotency-Key does not match the Offline Cash V1 operation id.".to_owned(),
        });
    }
    Ok(())
}

fn validated_idempotency_key(headers: &HeaderMap) -> Result<&str, Error> {
    let mut values = headers.get_all("idempotency-key").iter();
    let Some(raw) = values.next() else {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_missing",
            message: "Offline Cash V1 commands require one Idempotency-Key header.".to_owned(),
        });
    };
    if values.next().is_some() {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_invalid",
            message: "Offline Cash V1 commands require exactly one Idempotency-Key header."
                .to_owned(),
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

fn parse_operation_id(raw: &str) -> Result<[u8; 32], Error> {
    if raw.len() != 64
        || raw.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || raw.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message:
                "Offline Cash V1 operation id must be exactly 64 lowercase hexadecimal characters."
                    .to_owned(),
        });
    }
    let mut operation_id = [0_u8; 32];
    hex::decode_to_slice(raw, &mut operation_id).map_err(|_| Error::AppQueryValidation {
        code: "operation_id_invalid",
        message: "Offline Cash V1 operation id is not valid hexadecimal.".to_owned(),
    })?;
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "Offline Cash V1 operation id must be non-zero.".to_owned(),
        });
    }
    Ok(operation_id)
}

fn offline_transaction_signing_error(
    context: &'static str,
    source: impl std::fmt::Display,
) -> Error {
    iroha_logger::error!(%context, error = %source, "Offline Cash V1 signer failed");
    Error::Query(ValidationFail::InternalError(
        "Offline Cash V1 signer failed to sign the transaction.".to_owned(),
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
                message: "Offline Cash V1 commands are submitted by the configured Torii authority; X-Iroha account-auth headers are not accepted."
                    .to_owned(),
            });
        }
    }
    Ok(())
}

/// Validate body-independent Offline Cash V1 headers before payload decoding.
pub(crate) fn validate_command_headers_before_body(headers: &HeaderMap) -> Result<(), Error> {
    reject_x_iroha_auth_headers(headers)?;
    validated_idempotency_key(headers).map(|_| ())
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

    #[test]
    fn operation_id_accepts_every_nonzero_32_byte_value() {
        let operation_id = [0x22; 32];
        assert_eq!(
            parse_operation_id(&hex::encode(operation_id)).expect("parse operation id"),
            operation_id
        );
        assert!(parse_operation_id(&"00".repeat(32)).is_err());
    }

    #[test]
    fn idempotency_key_must_match_exact_operation_id() {
        let operation_id = [0x31; 32];
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            axum::http::HeaderValue::from_str(&hex::encode(operation_id))
                .expect("valid idempotency key"),
        );
        require_idempotency_key(&headers, operation_id).expect("matching idempotency key");
        assert!(require_idempotency_key(&headers, [0x32; 32]).is_err());
    }

    #[test]
    fn pending_and_rejected_statuses_validate_without_a_trust_anchor() {
        let binding = OfflineOperationBinding {
            operation_id: [0x41; 32],
            kind: OfflineCashOperationKindV1::TopUp,
            request_digest: [0x42; 32],
        };
        pending_status(binding).validate().expect("pending status");
        rejected_status(
            binding,
            OfflineCashOperationRejectionCodeV1::InvalidRequest,
            "canonical test rejection",
        )
        .validate()
        .expect("rejected status");
    }

    #[test]
    fn one_operation_id_cannot_bind_two_requests() {
        let existing = OfflineOperationBinding {
            operation_id: [0x51; 32],
            kind: OfflineCashOperationKindV1::TopUp,
            request_digest: [0x52; 32],
        };
        let conflicting = OfflineOperationBinding {
            request_digest: [0x53; 32],
            ..existing
        };
        ensure_same_binding(&existing, &existing).expect("identical binding");
        assert!(ensure_same_binding(&existing, &conflicting).is_err());
    }
}
