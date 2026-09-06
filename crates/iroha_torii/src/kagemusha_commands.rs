//! Canonical Torii command surface for KAGEMUSHA V1.
//!
//! Top-up accepts one exact payer-signed transaction, while redemption accepts
//! its exact V1 request for issuer submission. Both bind the idempotency key to
//! the canonical request digest; top-up also binds the caller-signed
//! payload/entrypoint hash. No legacy request, status, finality, or compatibility
//! decoding path is reachable from these routes.

use crate::{AppState, Error, SharedAppState, app_auth, routing};
use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::Response as AxResponse,
};
use iroha_config::parameters::actual;
use iroha_core::{
    smartcontracts::isi::kagemusha::KagemushaReserveOperationRecordV1,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, kagemusha_v1::RedeemKagemushaV1},
    transaction::{SignedTransaction, TransactionAdmissionIntent, TransactionBuilder},
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::kagemusha_api::{
    KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_OPERATION_STATUS_ROUTE_PREFIX_V1, KagemushaApiErrorV1,
    KagemushaOperationKindV1, KagemushaOperationRejectionCodeV1, KagemushaOperationRejectionV1,
    KagemushaOperationResultV1, KagemushaOperationStateV1, KagemushaOperationStatusV1,
    KagemushaRedemptionRequestV1, KagemushaRedemptionResultV1, KagemushaTopUpRequestV1,
    validate_kagemusha_top_up_signed_transaction_v1,
};
use mv::storage::StorageReadOnly;
use parking_lot::Mutex;
use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc};

// The operator-selected bound accounts for every in-memory idempotency reservation.
const OPERATION_ACCOUNTED_BYTES: usize =
    iroha_config::parameters::defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_ACCOUNTED_BYTES_PER_ENTRY;

#[derive(Debug, Clone)]
pub(crate) struct KagemushaCommandRuntime {
    redemption_issuer: Option<KagemushaRedemptionIssuer>,
    registry: Arc<Mutex<KagemushaOperationRegistry>>,
}

#[derive(Debug, Clone)]
struct KagemushaRedemptionIssuer {
    authority: AccountId,
    key_pair: KeyPair,
    minimum_xor_balance: Quantity,
}

impl KagemushaCommandRuntime {
    pub(crate) fn from_config(config: actual::ToriiKagemushaV1Commands) -> Self {
        Self {
            redemption_issuer: config
                .redemption_issuer
                .map(|issuer| KagemushaRedemptionIssuer {
                    authority: issuer.authority,
                    key_pair: issuer.key_pair,
                    minimum_xor_balance: issuer.minimum_xor_balance,
                }),
            registry: Arc::new(Mutex::new(KagemushaOperationRegistry::new(
                config.operation_registry_max_entries,
                config.operation_registry_max_bytes,
            ))),
        }
    }

    fn claim(
        self: &Arc<Self>,
        binding: KagemushaOperationBinding,
    ) -> Result<SubmissionClaim, Error> {
        let mut registry = self.registry.lock();
        match registry.entries.get(&binding.operation_id) {
            Some(KagemushaOperationEntry::Reserved(existing)) => {
                ensure_same_binding(existing, &binding)?;
                return Ok(SubmissionClaim::InFlight(*existing));
            }
            Some(KagemushaOperationEntry::Admitted(existing)) => {
                ensure_same_binding(&existing.binding, &binding)?;
                return Ok(SubmissionClaim::Existing(existing.clone()));
            }
            None => {}
        }
        if !registry.has_capacity_for_new_operation() {
            return Err(Error::AppServiceUnavailable {
                code: "kagemusha_operation_capacity_exhausted",
                message: "KAGEMUSHA V1 operation admission capacity is exhausted.".to_owned(),
            });
        }
        registry.entries.insert(
            binding.operation_id,
            KagemushaOperationEntry::Reserved(binding),
        );
        Ok(SubmissionClaim::Reserved(SubmissionReservation {
            runtime: Arc::clone(self),
            binding,
            active: true,
        }))
    }
}

impl KagemushaRedemptionIssuer {
    fn quote_and_sign_transaction(
        &self,
        app: &AppState,
        transaction: TransactionBuilder,
        context: &'static str,
    ) -> Result<SignedTransaction, Error> {
        let mut payload = transaction
            .into_payload()
            .map_err(|source| kagemusha_transaction_signing_error(context, source))?;
        payload.fee_payment = crate::quote_internal_fee_payment(app, &payload)?;
        TransactionBuilder::from_payload(payload)
            .map_err(|source| kagemusha_transaction_signing_error(context, source))?
            .try_sign(self.key_pair.private_key())
            .map_err(|source| kagemusha_transaction_signing_error(context, source))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KagemushaOperationBinding {
    operation_id: [u8; 32],
    kind: KagemushaOperationKindV1,
    request_digest: [u8; 32],
    top_up_transaction_hash: Option<HashOf<SignedTransaction>>,
}

#[derive(Debug, Clone)]
struct AdmittedKagemushaOperation {
    binding: KagemushaOperationBinding,
    transaction_hash: HashOf<SignedTransaction>,
}

#[derive(Debug, Clone)]
enum KagemushaOperationEntry {
    Reserved(KagemushaOperationBinding),
    Admitted(AdmittedKagemushaOperation),
}

#[derive(Debug)]
struct KagemushaOperationRegistry {
    entries: BTreeMap<[u8; 32], KagemushaOperationEntry>,
    max_entries: NonZeroUsize,
    max_accounted_bytes: NonZeroUsize,
}

impl KagemushaOperationRegistry {
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

    fn remove_exact_consensus_admissions(
        &mut self,
        reconstructible: &[AdmittedKagemushaOperation],
    ) {
        for candidate in reconstructible {
            let remove = self
                .entries
                .get(&candidate.binding.operation_id)
                .is_some_and(|entry| {
                    matches!(entry, KagemushaOperationEntry::Admitted(existing) if existing.binding == candidate.binding && existing.transaction_hash == candidate.transaction_hash)
                });
            if remove {
                self.entries.remove(&candidate.binding.operation_id);
            }
        }
    }
}

enum SubmissionClaim {
    InFlight(KagemushaOperationBinding),
    Existing(AdmittedKagemushaOperation),
    Reserved(SubmissionReservation),
}

struct SubmissionReservation {
    runtime: Arc<KagemushaCommandRuntime>,
    binding: KagemushaOperationBinding,
    active: bool,
}

impl SubmissionReservation {
    fn accept(
        mut self,
        transaction_hash: HashOf<SignedTransaction>,
    ) -> Result<AdmittedKagemushaOperation, Error> {
        let admitted = AdmittedKagemushaOperation {
            binding: self.binding,
            transaction_hash,
        };
        let mut registry = self.runtime.registry.lock();
        match registry.entries.get(&self.binding.operation_id) {
            Some(KagemushaOperationEntry::Reserved(existing)) if existing == &self.binding => {}
            Some(KagemushaOperationEntry::Admitted(existing)) => {
                ensure_same_binding(&existing.binding, &self.binding)?;
                self.active = false;
                return Ok(existing.clone());
            }
            _ => {
                return Err(Error::AppServiceUnavailable {
                    code: "kagemusha_operation_admission_inconsistent",
                    message: "The accepted KAGEMUSHA V1 operation lost its admission reservation."
                        .to_owned(),
                });
            }
        }
        registry.entries.insert(
            self.binding.operation_id,
            KagemushaOperationEntry::Admitted(admitted.clone()),
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
                matches!(entry, KagemushaOperationEntry::Reserved(existing) if existing == &self.binding)
            })
        {
            registry.entries.remove(&self.binding.operation_id);
        }
    }
}

pub(crate) async fn handle_top_up(
    app: SharedAppState,
    headers: HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    body: Bytes,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(&headers)?;
    let transaction = decode_top_up_signed_transaction_v1(&body)?;
    let request =
        validate_top_up_signed_transaction(app.state.network_id_ref(), &transaction)?.clone();
    require_idempotency_key(&headers, request.operation_id)?;
    let runtime = require_command_runtime(&app)?;
    let transaction_hash = transaction.hash();
    let binding = KagemushaOperationBinding {
        operation_id: request.operation_id,
        kind: KagemushaOperationKindV1::TopUp,
        request_digest: request.canonical_digest().map_err(|source| {
            validation_owned(
                "kagemusha_top_up_invalid",
                format!("KAGEMUSHA V1 top-up digest is invalid: {source}"),
            )
        })?,
        top_up_transaction_hash: Some(transaction_hash),
    };
    if let Some(existing) = admitted_operation_from_consensus(&app, binding.operation_id)? {
        ensure_same_binding(&existing.binding, &binding)?;
        return operation_submission_response_for_record(&app, &existing);
    }
    compact_consensus_reconstructible_operations_if_needed(&app, &runtime)?;
    match runtime.claim(binding)? {
        SubmissionClaim::InFlight(binding) => {
            return Ok(respond_with_operation_status(pending_status(binding), true));
        }
        SubmissionClaim::Existing(record) => {
            return operation_submission_response_for_record(&app, &record);
        }
        SubmissionClaim::Reserved(reservation) => {
            validate_top_up_snapshot(&app, &request)?;
            let response = crate::submit_signed_transaction_for_ingress_strict_durable(
                app,
                headers,
                accept,
                transaction,
            )
            .await?;
            if response.status() != StatusCode::ACCEPTED {
                return Ok(response);
            }
            let record = reservation.accept(transaction_hash)?;
            Ok(respond_with_operation_status(
                pending_status(record.binding),
                true,
            ))
        }
    }
}

fn decode_top_up_signed_transaction_v1(body: &[u8]) -> Result<SignedTransaction, Error> {
    let decode_limits = norito::canonical_decode_limits(body.len());
    let transaction = norito::with_decode_limits_scope(decode_limits, || {
        <SignedTransaction as iroha_version::codec::DecodeVersioned>::decode_all_versioned(body)
    })
    .map_err(|source| {
        validation_owned(
            "kagemusha_top_up_invalid",
            format!("KAGEMUSHA V1 top-up signed transaction is invalid: {source}"),
        )
    })?;
    let canonical = <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(
        &transaction,
    );
    if canonical.as_slice() != body {
        return Err(validation(
            "kagemusha_top_up_invalid",
            "KAGEMUSHA V1 top-up requires the canonical versioned signed-transaction encoding.",
        ));
    }
    Ok(transaction)
}

fn validate_top_up_signed_transaction<'a>(
    expected_network: &iroha_data_model::NetworkId,
    transaction: &'a SignedTransaction,
) -> Result<&'a KagemushaTopUpRequestV1, Error> {
    validate_kagemusha_top_up_signed_transaction_v1(expected_network, transaction).map_err(
        |source| match source {
            KagemushaApiErrorV1::TopUpTransactionWrongNetwork => validation(
                "kagemusha_wrong_network",
                "KAGEMUSHA V1 top-up signed transaction targets a different network.",
            ),
            source @ (KagemushaApiErrorV1::TopUpTransactionSignatureInvalid
            | KagemushaApiErrorV1::TopUpTransactionAuthorityMismatch) => validation_owned(
                "kagemusha_authorization_invalid",
                format!("KAGEMUSHA V1 top-up debit authorization is invalid: {source}"),
            ),
            source => validation_owned(
                "kagemusha_top_up_invalid",
                format!("KAGEMUSHA V1 top-up signed transaction is invalid: {source}"),
            ),
        },
    )
}

pub(crate) async fn handle_redeem(
    app: SharedAppState,
    headers: HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    request: KagemushaRedemptionRequestV1,
) -> Result<AxResponse, Error> {
    reject_x_iroha_auth_headers(&headers)?;
    require_idempotency_key(&headers, request.operation_id)?;
    request.validate_shape().map_err(|source| {
        validation_owned(
            "kagemusha_redeem_invalid",
            format!("KAGEMUSHA V1 redemption request is invalid: {source}"),
        )
    })?;
    let binding = KagemushaOperationBinding {
        operation_id: request.operation_id,
        kind: KagemushaOperationKindV1::Redemption,
        request_digest: request.canonical_digest().map_err(|source| {
            validation_owned(
                "kagemusha_redeem_invalid",
                format!("KAGEMUSHA V1 redemption digest is invalid: {source}"),
            )
        })?,
        top_up_transaction_hash: None,
    };
    let runtime = require_command_runtime(&app)?;
    if let Some(existing) = admitted_operation_from_consensus(&app, binding.operation_id)? {
        ensure_same_binding(&existing.binding, &binding)?;
        return operation_submission_response_for_record(&app, &existing);
    }
    compact_consensus_reconstructible_operations_if_needed(&app, &runtime)?;
    match runtime.claim(binding)? {
        SubmissionClaim::InFlight(binding) => {
            return Ok(respond_with_operation_status(pending_status(binding), true));
        }
        SubmissionClaim::Existing(record) => {
            return operation_submission_response_for_record(&app, &record);
        }
        SubmissionClaim::Reserved(reservation) => {
            validate_redemption_snapshot(&app, &request)?;
            let issuer =
                runtime
                    .redemption_issuer
                    .as_ref()
                    .ok_or_else(|| Error::AppServiceUnavailable {
                        code: "kagemusha_redemption_issuer_unavailable",
                        message:
                            "KAGEMUSHA V1 redemption signing is not configured on this Torii node."
                                .to_owned(),
                    })?;
            ensure_kagemusha_command_authority_ready(&app, issuer)?;
            let instruction = RedeemKagemushaV1::new(request).map_err(|source| {
                validation_owned(
                    "kagemusha_redeem_invalid",
                    format!("KAGEMUSHA V1 redemption instruction is invalid: {source}"),
                )
            })?;
            let transaction = TransactionBuilder::new(
                *app.state.network_id_ref(),
                issuer.authority.clone().into(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([InstructionBox::from(instruction)])
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced);
            let signed = issuer.quote_and_sign_transaction(
                &app,
                transaction,
                "kagemusha_v1_redemption_transaction",
            )?;
            let transaction_hash = signed.hash();
            let response = crate::submit_signed_transaction_for_ingress_strict_durable(
                app, headers, accept, signed,
            )
            .await?;
            if response.status() != StatusCode::ACCEPTED {
                return Ok(response);
            }
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
    let runtime = require_command_runtime(app)?;
    let local_record = {
        let registry = runtime.registry.lock();
        match registry.entries.get(&operation_id) {
            Some(KagemushaOperationEntry::Admitted(record)) => Some(record.clone()),
            Some(KagemushaOperationEntry::Reserved(binding)) => {
                return Ok(respond_with_operation_status(
                    pending_status(*binding),
                    false,
                ));
            }
            None => None,
        }
    };
    let record = match local_record {
        Some(record) => record,
        None => admitted_operation_from_consensus(app, operation_id)?.ok_or_else(|| {
            Error::AppNotFound {
                code: "kagemusha_operation_not_found",
                message: "The KAGEMUSHA V1 operation is unknown on this Torii node.".to_owned(),
            }
        })?,
    };
    operation_response_for_record(app, &record)
}

fn operation_response_for_record(
    app: &SharedAppState,
    record: &AdmittedKagemushaOperation,
) -> Result<AxResponse, Error> {
    Ok(respond_with_operation_status(
        operation_status_for_record(app, record)?,
        false,
    ))
}

fn operation_submission_response_for_record(
    app: &SharedAppState,
    record: &AdmittedKagemushaOperation,
) -> Result<AxResponse, Error> {
    Ok(respond_with_operation_status(
        operation_status_for_record(app, record)?,
        true,
    ))
}

fn operation_status_for_record(
    app: &SharedAppState,
    record: &AdmittedKagemushaOperation,
) -> Result<KagemushaOperationStatusV1, Error> {
    let status = match crate::pipeline_status_local_entry_checked(app, &record.transaction_hash)? {
        Some((entry, resolved_from)) => match entry.kind {
            crate::PipelineStatusKind::Applied if resolved_from != "state" => {
                pending_status(record.binding)
            }
            crate::PipelineStatusKind::Applied => {
                let height = entry.block_height.ok_or_else(|| {
                    kagemusha_consensus_inconsistency(
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
    Ok(status)
}

fn admitted_operation_from_consensus(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<Option<AdmittedKagemushaOperation>, Error> {
    let operation = {
        let state = app.state.view();
        state
            .world()
            .kagemusha_reserve_operations()
            .get(&operation_id)
            .cloned()
    };
    let Some(operation) = operation else {
        return Ok(None);
    };
    let (kind, request_digest) = match &operation {
        KagemushaReserveOperationRecordV1::TopUp(record) => {
            record
                .issuance_intent
                .request
                .validate_shape()
                .map_err(|error| {
                    kagemusha_consensus_inconsistency(format!(
                        "persisted top-up request is invalid: {error}"
                    ))
                })?;
            (
                KagemushaOperationKindV1::TopUp,
                record.issuance_intent.request_digest,
            )
        }
        KagemushaReserveOperationRecordV1::Redemption(record) => {
            record
                .redemption_request
                .validate_shape()
                .map_err(|error| {
                    kagemusha_consensus_inconsistency(format!(
                        "persisted redemption request is invalid: {error}"
                    ))
                })?;
            (KagemushaOperationKindV1::Redemption, record.request_digest)
        }
    };
    let receipt = operation.reserve_receipt();
    receipt.validate().map_err(|error| {
        kagemusha_consensus_inconsistency(format!("persisted reserve receipt is invalid: {error}"))
    })?;
    if operation.operation_id() != operation_id
        || receipt.operation_id != operation_id
        || receipt.kind != kind
        || receipt.request_digest != request_digest
    {
        return Err(kagemusha_consensus_inconsistency(
            "persisted reserve operation identity is internally inconsistent",
        ));
    }
    let transaction_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed(receipt.transaction_hash));
    Ok(Some(AdmittedKagemushaOperation {
        binding: KagemushaOperationBinding {
            operation_id,
            kind,
            request_digest,
            top_up_transaction_hash: (kind == KagemushaOperationKindV1::TopUp)
                .then_some(transaction_hash),
        },
        transaction_hash,
    }))
}

fn compact_consensus_reconstructible_operations_if_needed(
    app: &SharedAppState,
    runtime: &Arc<KagemushaCommandRuntime>,
) -> Result<(), Error> {
    let candidates = {
        let registry = runtime.registry.lock();
        if registry.has_capacity_for_new_operation() {
            return Ok(());
        }
        registry
            .entries
            .values()
            .filter_map(|entry| match entry {
                KagemushaOperationEntry::Admitted(record) => Some(record.clone()),
                KagemushaOperationEntry::Reserved(_) => None,
            })
            .collect::<Vec<_>>()
    };
    let mut reconstructible = Vec::new();
    for local in candidates {
        let Some(canonical) = admitted_operation_from_consensus(app, local.binding.operation_id)?
        else {
            continue;
        };
        if canonical.binding != local.binding
            || canonical.transaction_hash != local.transaction_hash
        {
            return Err(kagemusha_consensus_inconsistency(
                "local admitted operation differs from its durable consensus record",
            ));
        }
        reconstructible.push(canonical);
    }
    runtime
        .registry
        .lock()
        .remove_exact_consensus_admissions(&reconstructible);
    Ok(())
}

fn applied_status_from_consensus(
    app: &SharedAppState,
    admitted: &AdmittedKagemushaOperation,
    height: u64,
) -> Result<Option<KagemushaOperationStatusV1>, Error> {
    let operation = {
        let state = app.state.view();
        state
            .world()
            .kagemusha_reserve_operations()
            .get(&admitted.binding.operation_id)
            .cloned()
    }
    .ok_or_else(|| {
        kagemusha_consensus_inconsistency(
            "state-resolved transaction has no KAGEMUSHA V1 reserve operation",
        )
    })?;
    let canonical = admitted_operation_from_consensus(app, admitted.binding.operation_id)?
        .ok_or_else(|| {
            kagemusha_consensus_inconsistency(
                "persisted KAGEMUSHA V1 reserve operation disappeared during lookup",
            )
        })?;
    ensure_same_binding(&canonical.binding, &admitted.binding)?;
    if canonical.transaction_hash != admitted.transaction_hash {
        return Err(kagemusha_consensus_inconsistency(
            "reserve receipt transaction hash differs from the admitted operation",
        ));
    }
    let finality = app
        .kura
        .kagemusha_operation_finality_v1(height, admitted.binding.operation_id)
        .map_err(|error| {
            kagemusha_consensus_inconsistency(format!(
                "canonical reserve-receipt finality lookup failed: {error}"
            ))
        })?;
    let Some(finality) = finality else {
        let artifact_exists = app.kura.v2_finality_artifact(height).map_err(|error| {
            kagemusha_consensus_inconsistency(format!(
                "canonical finality lookup failed while resolving reserve receipt: {error}"
            ))
        })?;
        if artifact_exists.is_some() {
            return Err(kagemusha_consensus_inconsistency(
                "canonical finality exists without the applied reserve receipt witness",
            ));
        }
        return Ok(None);
    };
    if finality.reserve_receipt_witness.receipt != *operation.reserve_receipt()
        || finality.finality_artifact.height != height
    {
        return Err(kagemusha_consensus_inconsistency(
            "Kura finality does not match the persisted reserve receipt or height",
        ));
    }
    let result = match operation {
        KagemushaReserveOperationRecordV1::TopUp(record) => {
            if admitted.binding.kind != KagemushaOperationKindV1::TopUp
                || admitted.binding.request_digest != record.issuance_intent.request_digest
            {
                return Err(kagemusha_consensus_inconsistency(
                    "top-up operation kind or request digest differs from admission",
                ));
            }
            let Some(result) = app
                .kura
                .kagemusha_mint_outbox_entry_v1(admitted.binding.operation_id)
                .map_err(|error| {
                    kagemusha_consensus_inconsistency(format!(
                        "canonical mint outbox lookup failed: {error}"
                    ))
                })?
            else {
                return Ok(None);
            };
            if result.request != record.issuance_intent.request || result.finality != finality {
                return Err(kagemusha_consensus_inconsistency(
                    "mint outbox result differs from the consensus top-up record",
                ));
            }
            KagemushaOperationResultV1::TopUp(result)
        }
        KagemushaReserveOperationRecordV1::Redemption(record) => {
            if admitted.binding.kind != KagemushaOperationKindV1::Redemption
                || admitted.binding.request_digest != record.request_digest
            {
                return Err(kagemusha_consensus_inconsistency(
                    "redemption operation kind or request digest differs from admission",
                ));
            }
            KagemushaOperationResultV1::Redemption(KagemushaRedemptionResultV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                request: record.redemption_request,
                finality,
            })
        }
    };
    let status = KagemushaOperationStatusV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: admitted.binding.operation_id,
        kind: admitted.binding.kind,
        state: KagemushaOperationStateV1::Applied,
        result: Some(result),
        rejection: None,
    };
    let finality = match status.result.as_ref().expect("Applied status has a result") {
        KagemushaOperationResultV1::TopUp(result) => &result.finality,
        KagemushaOperationResultV1::Redemption(result) => &result.finality,
    };
    let anchor = iroha_torii_shared::kagemusha_api::KagemushaFinalityTrustAnchorV1 {
        network_id: finality.finality_artifact.height_context.network_id,
        block_height: finality.finality_artifact.height,
        height_context_id: finality.finality_artifact.context_id(),
    };
    status.validate_against(&anchor).map_err(|error| {
        kagemusha_consensus_inconsistency(format!(
            "constructed Applied status failed canonical validation: {error}"
        ))
    })?;
    Ok(Some(status))
}

fn kagemusha_consensus_inconsistency(detail: impl std::fmt::Display) -> Error {
    iroha_logger::error!(%detail, "KAGEMUSHA V1 consensus projection is inconsistent");
    Error::AppServiceUnavailable {
        code: "kagemusha_operation_consensus_inconsistent",
        message: "Canonical KAGEMUSHA V1 operation evidence is unavailable or inconsistent."
            .to_owned(),
    }
}

fn pending_status(binding: KagemushaOperationBinding) -> KagemushaOperationStatusV1 {
    KagemushaOperationStatusV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: binding.operation_id,
        kind: binding.kind,
        state: KagemushaOperationStateV1::Pending,
        result: None,
        rejection: None,
    }
}

fn rejected_status(
    binding: KagemushaOperationBinding,
    code: KagemushaOperationRejectionCodeV1,
    detail: &str,
) -> KagemushaOperationStatusV1 {
    let mut hasher = blake3::Hasher::new_derive_key("iroha.kagemusha.v1.rejection-detail");
    hasher.update(detail.as_bytes());
    KagemushaOperationStatusV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: binding.operation_id,
        kind: binding.kind,
        state: KagemushaOperationStateV1::Rejected,
        result: None,
        rejection: Some(KagemushaOperationRejectionV1 {
            code,
            detail_digest: *hasher.finalize().as_bytes(),
        }),
    }
}

fn rejection_code(
    kind: crate::PipelineStatusKind,
    detail: Option<&'static str>,
) -> KagemushaOperationRejectionCodeV1 {
    if kind == crate::PipelineStatusKind::Expired {
        return KagemushaOperationRejectionCodeV1::InvalidRequest;
    }
    match detail {
        Some("Account does not exist.") => KagemushaOperationRejectionCodeV1::Unauthorized,
        Some("Transaction limits were exceeded.") | Some("Transaction validation failed.") => {
            KagemushaOperationRejectionCodeV1::InvalidRequest
        }
        Some("Instruction execution failed.")
        | Some("IVM execution failed.")
        | Some("Trigger execution failed.")
        | None
        | Some(_) => KagemushaOperationRejectionCodeV1::InternalFailure,
    }
}

fn respond_with_operation_status(
    status: KagemushaOperationStatusV1,
    accepted_submission: bool,
) -> AxResponse {
    let pending = status.state == KagemushaOperationStateV1::Pending;
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
            "{KAGEMUSHA_OPERATION_STATUS_ROUTE_PREFIX_V1}{}",
            hex::encode(operation_id)
        );
        let value = axum::http::HeaderValue::from_str(&location)
            .expect("canonical KAGEMUSHA operation path is a valid header value");
        response
            .headers_mut()
            .insert(axum::http::header::LOCATION, value);
    }
    response
}

fn ensure_same_binding(
    existing: &KagemushaOperationBinding,
    requested: &KagemushaOperationBinding,
) -> Result<(), Error> {
    if existing == requested {
        return Ok(());
    }
    Err(Error::AppConflict {
        code: "operation_id_conflict",
        message: "KAGEMUSHA V1 operation id is already bound to a different canonical request or top-up transaction payload."
            .to_owned(),
    })
}

fn validate_top_up_snapshot(
    app: &SharedAppState,
    request: &KagemushaTopUpRequestV1,
) -> Result<(), Error> {
    if request.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "kagemusha_wrong_network",
            "KAGEMUSHA V1 top-up targets a different network.",
        ));
    }
    validate_live_asset_scale(app, &request.asset, request.scale)
}

fn validate_redemption_snapshot(
    app: &SharedAppState,
    request: &KagemushaRedemptionRequestV1,
) -> Result<(), Error> {
    let statement = &request.voucher.statement;
    if statement.lifecycle.network_id != *app.state.network_id_ref() {
        return Err(validation(
            "kagemusha_wrong_network",
            "KAGEMUSHA V1 redemption targets a different network.",
        ));
    }
    validate_live_asset_scale(app, &statement.lifecycle.asset, statement.lifecycle.scale)
}

fn validate_live_asset_scale(
    app: &SharedAppState,
    asset: &AssetDefinitionId,
    requested_scale: u32,
) -> Result<(), Error> {
    let state = app.state.view();
    let definition = state.world().asset_definition(asset).map_err(|_| {
        validation(
            "kagemusha_asset_not_found",
            "KAGEMUSHA V1 asset definition is not registered.",
        )
    })?;
    let live_scale = definition.spec().scale().ok_or_else(|| {
        validation(
            "kagemusha_asset_scale_invalid",
            "KAGEMUSHA V1 requires a fixed live asset scale.",
        )
    })?;
    if requested_scale != live_scale {
        return Err(validation(
            "kagemusha_asset_scale_mismatch",
            "KAGEMUSHA V1 request scale differs from the live asset scale.",
        ));
    }
    Ok(())
}

fn require_command_runtime(app: &AppState) -> Result<Arc<KagemushaCommandRuntime>, Error> {
    app.kagemusha_commands
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "kagemusha_service_unavailable",
            message: "KAGEMUSHA V1 operation admission is not configured on this Torii node."
                .to_owned(),
        })
}

fn ensure_kagemusha_command_authority_ready(
    app: &AppState,
    issuer: &KagemushaRedemptionIssuer,
) -> Result<(), Error> {
    let state = app.state.view();
    let fee_asset_selector = app.state.nexus_snapshot().fees.fee_asset_id;
    ensure_kagemusha_command_authority_ready_in_world(
        state.world(),
        issuer,
        &fee_asset_selector,
        snapshot_time_ms(&state),
    )
}

fn ensure_kagemusha_command_authority_ready_in_world(
    world: &impl WorldReadOnly,
    issuer: &KagemushaRedemptionIssuer,
    fee_asset_selector: &str,
    snapshot_time_ms: u64,
) -> Result<(), Error> {
    if world.account(&issuer.authority).is_err()
        || !iroha_core::smartcontracts::isi::kagemusha::isi::world_has_kagemusha_reserve_manager_permission(
            world,
            &issuer.authority,
        )
    {
        return Err(Error::AppServiceUnavailable {
            code: "kagemusha_command_authority_not_ready",
            message: "KAGEMUSHA V1 command authority is not registered with CanManageKagemushaReserve."
                .to_owned(),
        });
    }
    let fee_asset_definition =
        routing::resolve_asset_definition_selector(world, fee_asset_selector, snapshot_time_ms)
            .map_err(|error| {
                iroha_logger::error!(
                    ?error,
                    %fee_asset_selector,
                    "KAGEMUSHA V1 command fee asset could not be resolved"
                );
                Error::AppServiceUnavailable {
                    code: "kagemusha_command_fee_asset_not_ready",
                    message: "KAGEMUSHA V1 command fee asset is not available.".to_owned(),
                }
            })?;
    let fee_asset = AssetId::new(fee_asset_definition, issuer.authority.clone());
    let balance = world
        .asset(&fee_asset)
        .map(|entry| entry.value().as_ref().clone())
        .unwrap_or_else(|_| Quantity::zero());
    if balance < issuer.minimum_xor_balance {
        return Err(Error::AppServiceUnavailable {
            code: "kagemusha_command_authority_unfunded",
            message:
                "KAGEMUSHA V1 command authority does not meet its configured minimum fee balance."
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
            message: "KAGEMUSHA V1 operation id must be non-zero.".to_owned(),
        });
    }
    let expected = hex::encode(operation_id);
    if validated_idempotency_key(headers)? != expected {
        return Err(Error::AppConflict {
            code: "idempotency_key_conflict",
            message: "Idempotency-Key does not match the KAGEMUSHA V1 operation id.".to_owned(),
        });
    }
    Ok(())
}

fn validated_idempotency_key(headers: &HeaderMap) -> Result<&str, Error> {
    let mut values = headers.get_all("idempotency-key").iter();
    let Some(raw) = values.next() else {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_missing",
            message: "KAGEMUSHA V1 commands require one Idempotency-Key header.".to_owned(),
        });
    };
    if values.next().is_some() {
        return Err(Error::AppQueryValidation {
            code: "idempotency_key_invalid",
            message: "KAGEMUSHA V1 commands require exactly one Idempotency-Key header.".to_owned(),
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
                "KAGEMUSHA V1 operation id must be exactly 64 lowercase hexadecimal characters."
                    .to_owned(),
        });
    }
    let mut operation_id = [0_u8; 32];
    hex::decode_to_slice(raw, &mut operation_id).map_err(|_| Error::AppQueryValidation {
        code: "operation_id_invalid",
        message: "KAGEMUSHA V1 operation id is not valid hexadecimal.".to_owned(),
    })?;
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "KAGEMUSHA V1 operation id must be non-zero.".to_owned(),
        });
    }
    Ok(operation_id)
}

fn kagemusha_transaction_signing_error(
    context: &'static str,
    source: impl std::fmt::Display,
) -> Error {
    iroha_logger::error!(%context, error = %source, "KAGEMUSHA V1 signer failed");
    Error::Query(ValidationFail::InternalError(
        "KAGEMUSHA V1 signer failed to sign the transaction.".to_owned(),
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
                code: "kagemusha_auth_header_unsupported",
                message: "KAGEMUSHA V1 command authorization is carried by its canonical protocol body; X-Iroha account-auth headers are not accepted."
                    .to_owned(),
            });
        }
    }
    Ok(())
}

/// Validate body-independent KAGEMUSHA V1 headers before payload decoding.
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
    use iroha_data_model::{Level, block::BlockHeader, isi::Log, transaction::FeePaymentIntent};
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

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
    fn signed_top_up_decoder_preserves_the_canonical_transaction_identity() {
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"top-up-signed-envelope")),
        );
        let transaction = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "envelope fixture".to_owned())])
        .try_sign(ALICE_KEYPAIR.private_key())
        .expect("sign transaction fixture");
        let bytes = <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(
            &transaction,
        );
        let decoded =
            decode_top_up_signed_transaction_v1(&bytes).expect("decode canonical transaction");
        assert_eq!(decoded.hash(), transaction.hash());
        assert_eq!(
            <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(
                &decoded
            ),
            bytes
        );

        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_top_up_signed_transaction_v1(&trailing).is_err());
    }

    #[test]
    fn finalized_operations_do_not_permanently_exhaust_a_small_registry() {
        let mut registry = KagemushaOperationRegistry::new(
            NonZeroUsize::new(1).expect("positive entry capacity"),
            NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES).expect("positive byte capacity"),
        );
        for sequence in 1_u16..=1_024 {
            assert!(registry.has_capacity_for_new_operation());
            let mut operation_id = [0_u8; 32];
            operation_id[..2].copy_from_slice(&sequence.to_be_bytes());
            let record = AdmittedKagemushaOperation {
                binding: KagemushaOperationBinding {
                    operation_id,
                    kind: KagemushaOperationKindV1::TopUp,
                    request_digest: [u8::try_from(sequence % 251).expect("bounded tag"); 32],
                    top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                        operation_id,
                    ))),
                },
                transaction_hash: HashOf::from_untyped_unchecked(Hash::new(operation_id)),
            };
            registry.entries.insert(
                operation_id,
                KagemushaOperationEntry::Admitted(record.clone()),
            );
            assert!(!registry.has_capacity_for_new_operation());
            registry.remove_exact_consensus_admissions(std::slice::from_ref(&record));
            assert!(registry.entries.is_empty());
        }
    }

    #[test]
    fn compaction_never_removes_reserved_or_nonidentical_admissions() {
        let mut registry = KagemushaOperationRegistry::new(
            NonZeroUsize::new(2).expect("positive entry capacity"),
            NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES * 2).expect("positive byte capacity"),
        );
        let binding = KagemushaOperationBinding {
            operation_id: [0x61; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x62; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x65]))),
        };
        let canonical = AdmittedKagemushaOperation {
            binding,
            transaction_hash: HashOf::from_untyped_unchecked(Hash::new([0x63])),
        };
        registry.entries.insert(
            binding.operation_id,
            KagemushaOperationEntry::Reserved(binding),
        );
        registry.remove_exact_consensus_admissions(std::slice::from_ref(&canonical));
        assert!(matches!(
            registry.entries.get(&binding.operation_id),
            Some(KagemushaOperationEntry::Reserved(existing)) if existing == &binding
        ));

        let local = AdmittedKagemushaOperation {
            transaction_hash: HashOf::from_untyped_unchecked(Hash::new([0x64])),
            ..canonical.clone()
        };
        registry.entries.insert(
            binding.operation_id,
            KagemushaOperationEntry::Admitted(local.clone()),
        );
        registry.remove_exact_consensus_admissions(std::slice::from_ref(&canonical));
        assert!(matches!(
            registry.entries.get(&binding.operation_id),
            Some(KagemushaOperationEntry::Admitted(existing)) if existing.transaction_hash == local.transaction_hash
        ));
    }

    #[test]
    fn failed_ingress_drops_the_in_flight_reservation() {
        let binding = KagemushaOperationBinding {
            operation_id: [0x71; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x72; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x73]))),
        };
        let runtime = Arc::new(KagemushaCommandRuntime {
            redemption_issuer: None,
            registry: Arc::new(Mutex::new(KagemushaOperationRegistry::new(
                NonZeroUsize::new(1).expect("positive entry capacity"),
                NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES).expect("positive byte capacity"),
            ))),
        });
        let SubmissionClaim::Reserved(reservation) = runtime
            .claim(binding)
            .expect("reserve operation for ingress")
        else {
            panic!("new operation must reserve capacity");
        };
        assert!(
            runtime
                .registry
                .lock()
                .entries
                .contains_key(&binding.operation_id)
        );
        drop(reservation);
        assert!(runtime.registry.lock().entries.is_empty());
    }

    #[test]
    fn payer_signed_top_up_admission_does_not_require_a_redemption_signer() {
        let runtime = Arc::new(KagemushaCommandRuntime {
            redemption_issuer: None,
            registry: Arc::new(Mutex::new(KagemushaOperationRegistry::new(
                NonZeroUsize::new(1).expect("positive entry capacity"),
                NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES).expect("positive byte capacity"),
            ))),
        });
        let binding = KagemushaOperationBinding {
            operation_id: [0x74; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x75; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x76]))),
        };
        assert!(runtime.redemption_issuer.is_none());
        assert!(matches!(
            runtime.claim(binding),
            Ok(SubmissionClaim::Reserved(_))
        ));
    }

    #[test]
    fn identical_concurrent_claim_returns_the_canonical_pending_binding() {
        let runtime = Arc::new(KagemushaCommandRuntime {
            redemption_issuer: None,
            registry: Arc::new(Mutex::new(KagemushaOperationRegistry::new(
                NonZeroUsize::new(1).expect("positive entry capacity"),
                NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES).expect("positive byte capacity"),
            ))),
        });
        let binding = KagemushaOperationBinding {
            operation_id: [0x77; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x78; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x79]))),
        };
        let SubmissionClaim::Reserved(reservation) = runtime
            .claim(binding)
            .expect("first claim reserves the operation")
        else {
            panic!("first claim must reserve capacity");
        };

        assert!(matches!(
            runtime.claim(binding),
            Ok(SubmissionClaim::InFlight(existing)) if existing == binding
        ));
        assert!(matches!(
            runtime
                .registry
                .lock()
                .entries
                .get(&binding.operation_id),
            Some(KagemushaOperationEntry::Reserved(existing)) if existing == &binding
        ));

        drop(reservation);
    }

    #[test]
    fn pending_and_rejected_statuses_validate_without_a_trust_anchor() {
        let binding = KagemushaOperationBinding {
            operation_id: [0x41; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x42; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x43]))),
        };
        pending_status(binding).validate().expect("pending status");
        rejected_status(
            binding,
            KagemushaOperationRejectionCodeV1::InvalidRequest,
            "canonical test rejection",
        )
        .validate()
        .expect("rejected status");
    }

    #[test]
    fn submission_response_headers_follow_pending_and_terminal_state() {
        let binding = KagemushaOperationBinding {
            operation_id: [0x45; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x46; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x47]))),
        };
        let expected_location = format!(
            "{KAGEMUSHA_OPERATION_STATUS_ROUTE_PREFIX_V1}{}",
            hex::encode(binding.operation_id)
        );

        let pending = respond_with_operation_status(pending_status(binding), true);
        assert_eq!(pending.status(), StatusCode::ACCEPTED);
        assert_eq!(
            pending
                .headers()
                .get(axum::http::header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some(expected_location.as_str())
        );
        assert_eq!(
            pending
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );

        let pending_status_resource = respond_with_operation_status(pending_status(binding), false);
        assert_eq!(pending_status_resource.status(), StatusCode::OK);
        assert!(
            !pending_status_resource
                .headers()
                .contains_key(axum::http::header::LOCATION)
        );
        assert_eq!(
            pending_status_resource
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );

        let terminal = respond_with_operation_status(
            rejected_status(
                binding,
                KagemushaOperationRejectionCodeV1::InvalidRequest,
                "terminal replay",
            ),
            true,
        );
        assert_eq!(terminal.status(), StatusCode::OK);
        assert_eq!(
            terminal
                .headers()
                .get(axum::http::header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some(expected_location.as_str())
        );
        assert!(
            !terminal
                .headers()
                .contains_key(axum::http::header::RETRY_AFTER)
        );
    }

    #[test]
    fn one_operation_id_cannot_bind_two_requests() {
        let existing = KagemushaOperationBinding {
            operation_id: [0x51; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x52; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x53]))),
        };
        let conflicting = KagemushaOperationBinding {
            request_digest: [0x53; 32],
            ..existing
        };
        ensure_same_binding(&existing, &existing).expect("identical binding");
        assert!(ensure_same_binding(&existing, &conflicting).is_err());
    }

    #[test]
    fn one_top_up_operation_id_cannot_bind_two_signed_transaction_payloads() {
        let existing = KagemushaOperationBinding {
            operation_id: [0x81; 32],
            kind: KagemushaOperationKindV1::TopUp,
            request_digest: [0x82; 32],
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x83]))),
        };
        let conflicting = KagemushaOperationBinding {
            top_up_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::new([0x84]))),
            ..existing
        };
        let runtime = Arc::new(KagemushaCommandRuntime {
            redemption_issuer: None,
            registry: Arc::new(Mutex::new(KagemushaOperationRegistry::new(
                NonZeroUsize::new(1).expect("positive entry capacity"),
                NonZeroUsize::new(OPERATION_ACCOUNTED_BYTES).expect("positive byte capacity"),
            ))),
        });
        let SubmissionClaim::Reserved(reservation) = runtime
            .claim(existing)
            .expect("reserve first signed transaction payload")
        else {
            panic!("new operation must reserve capacity");
        };

        assert!(runtime.claim(conflicting).is_err());
        reservation
            .accept(existing.top_up_transaction_hash.expect("top-up hash"))
            .expect("admit first signed transaction payload");
        assert!(runtime.claim(conflicting).is_err());
    }
}
