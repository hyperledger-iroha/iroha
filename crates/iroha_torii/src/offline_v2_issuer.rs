use std::{
    collections::BTreeMap,
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{http::HeaderMap, response::Response as AxResponse};
use iroha_config::parameters::actual;
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{
        InstructionBox,
        offline::{RedeemKagemushaRecursiveV2, TopUpKagemushaRecursiveV2},
    },
    name::Name,
    offline::KagemushaRecursiveSpendTopUpAnchorV2,
    transaction::{
        Executable, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        error::TransactionRejectionReason,
    },
};
use iroha_primitives::numeric::Numeric;
use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationResult, OfflineOperationState,
    OfflineOperationStatus, OfflineRedeemRequest, OfflineRedeemResult, OfflineTopUpRequest,
    OfflineTopUpResult,
};
use mv::storage::StorageReadOnly;

use crate::{AppState, Error, SharedAppState, app_auth, routing};

const PATH_KAGEMUSHA_TOPUP: &str = iroha_torii_shared::uri::OFFLINE_TOP_UP;
const PATH_NOTES_REDEEM: &str = iroha_torii_shared::uri::OFFLINE_REDEEM;
const OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS: u64 = 24 * 60 * 60 * 1_000;
#[derive(Debug, Clone)]
pub(crate) struct OfflineV2IssuerRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    max_tx_value: Numeric,
    operations: Arc<RwLock<BTreeMap<[u8; 32], OfflineOperationRecord>>>,
}

impl OfflineV2IssuerRuntime {
    pub(crate) fn from_config(config: actual::ToriiOfflineIssuer) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            max_tx_value: config.max_tx_value,
            operations: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    fn sign_transaction(
        &self,
        transaction: TransactionBuilder,
        context: &'static str,
    ) -> Result<SignedTransaction, Error> {
        transaction
            .try_sign(self.key_pair.private_key())
            .map_err(|source| offline_v2_transaction_signing_error(context, source))
    }
}

pub(crate) async fn handle_kagemusha_topup(
    app: SharedAppState,
    headers: &HeaderMap,
    topup_request: OfflineTopUpRequest,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, topup_request.authorization.operation_id)?;
    topup_request.validate_public_binding().map_err(|source| {
        validation_owned(
            "offline_top_up_invalid",
            format!("Offline top-up request is invalid: {source}"),
        )
    })?;
    validate_kagemusha_v2_topup_snapshot(&app, &topup_request)?;
    if topup_request.amount.public_numeric() > issuer.max_tx_value.clone() {
        return Err(validation(
            "offline_amount_exceeds_limit",
            "Offline top-up amount exceeds issuer policy.",
        ));
    }
    if let Some(finality) =
        find_committed_kagemusha_v2_operation(&app, OfflineOperationRequest::TopUp(&topup_request))?
    {
        return offline_operation_reference_response(
            topup_request.authorization.operation_id,
            OfflineOperationKind::TopUp,
            finality.transaction_hash,
            topup_request.authorization.issued_at_ms,
        );
    }

    let instruction = TopUpKagemushaRecursiveV2::new(topup_request.clone());
    let mut transaction =
        TransactionBuilder::new((*app.chain_id).clone(), issuer.authority.clone().into())
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
    let tx = issuer.sign_transaction(transaction, "offline_top_up_transaction")?;
    let tx_hash = tx.hash();
    let submitted_at_ms = topup_request.authorization.issued_at_ms;
    if let Some(existing) = reserve_offline_operation(
        &issuer,
        OfflineOperationRequest::TopUp(&topup_request),
        tx_hash.clone(),
        submitted_at_ms,
    )? {
        return offline_operation_reference_response(
            existing.request.authorization().operation_id,
            existing.request.kind().into(),
            existing.transaction_hash.to_string(),
            existing.submitted_at_ms,
        );
    }
    if let Err(error) = routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_KAGEMUSHA_TOPUP,
    )
    .await
    {
        remove_reserved_offline_operation(
            &issuer,
            topup_request.authorization.operation_id,
            &tx_hash,
        );
        return Err(error);
    }
    offline_operation_reference_response(
        topup_request.authorization.operation_id,
        OfflineOperationKind::TopUp,
        tx_hash.to_string(),
        submitted_at_ms,
    )
}

pub(crate) async fn handle_notes_redeem(
    app: SharedAppState,
    headers: &HeaderMap,
    redeem_request: OfflineRedeemRequest,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    reject_x_iroha_auth_headers(headers)?;
    require_idempotency_key(headers, redeem_request.authorization.operation_id)?;
    redeem_request.validate_public_binding().map_err(|source| {
        validation_owned(
            "offline_redeem_invalid",
            format!("Offline redemption request is invalid: {source}"),
        )
    })?;
    validate_kagemusha_v2_redeem_snapshot(&app, &redeem_request)?;
    if redeem_request.amount.public_numeric() > issuer.max_tx_value.clone() {
        return Err(validation(
            "offline_amount_exceeds_limit",
            "Offline redemption amount exceeds issuer policy.",
        ));
    }
    if let Some(finality) = find_committed_kagemusha_v2_operation(
        &app,
        OfflineOperationRequest::Redeem(&redeem_request),
    )? {
        return offline_operation_reference_response(
            redeem_request.authorization.operation_id,
            OfflineOperationKind::Redeem,
            finality.transaction_hash,
            redeem_request.authorization.issued_at_ms,
        );
    }
    let operation_id = redeem_request.authorization.operation_id;
    let authorization = redeem_request.authorization.clone();
    let instruction = RedeemKagemushaRecursiveV2::new(redeem_request.clone());
    let mut transaction =
        TransactionBuilder::new((*app.chain_id).clone(), issuer.authority.clone().into())
            .with_instructions([InstructionBox::from(instruction)]);
    transaction.set_creation_time(Duration::from_millis(authorization.issued_at_ms));
    transaction.set_ttl(Duration::from_millis(
        authorization
            .expires_at_ms
            .saturating_sub(authorization.issued_at_ms),
    ));
    let tx = issuer.sign_transaction(transaction, "offline_redeem_transaction")?;
    let tx_hash = tx.hash();
    let submitted_at_ms = authorization.issued_at_ms;
    if let Some(existing) = reserve_offline_operation(
        &issuer,
        OfflineOperationRequest::Redeem(&redeem_request),
        tx_hash.clone(),
        submitted_at_ms,
    )? {
        return offline_operation_reference_response(
            existing.request.authorization().operation_id,
            existing.request.kind().into(),
            existing.transaction_hash.to_string(),
            existing.submitted_at_ms,
        );
    }
    if let Err(error) = routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_NOTES_REDEEM,
    )
    .await
    {
        remove_reserved_offline_operation(&issuer, operation_id, &tx_hash);
        return Err(error);
    }
    offline_operation_reference_response(
        operation_id,
        OfflineOperationKind::Redeem,
        tx_hash.to_string(),
        submitted_at_ms,
    )
}

fn kagemusha_v2_snapshot_time_ms(app: &SharedAppState) -> u64 {
    app.state.view().latest_block().map_or(0, |block| {
        u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
    })
}

fn validate_kagemusha_v2_topup_snapshot(
    app: &SharedAppState,
    request: &OfflineTopUpRequest,
) -> Result<(), Error> {
    ensure_kagemusha_v2_backend_available()?;
    if request.current_note.chain_id != *app.chain_id {
        return Err(validation(
            "offline_wrong_chain",
            "Offline top-up request targets a different chain.",
        ));
    }
    let world = app.state.world_view();
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
    request
        .validate_authorization_at(kagemusha_v2_snapshot_time_ms(app))
        .map_err(|err| {
            validation_owned(
                "offline_authorization_invalid",
                format!("Offline top-up authorization is not live at chain time: {err}"),
            )
        })
}

fn validate_kagemusha_v2_redeem_snapshot(
    app: &SharedAppState,
    request: &OfflineRedeemRequest,
) -> Result<(), Error> {
    ensure_kagemusha_v2_backend_available()?;
    if request.bundle.statement.chain_id != *app.chain_id {
        return Err(validation(
            "offline_wrong_chain",
            "Offline redemption request targets a different chain.",
        ));
    }
    let world = app.state.world_view();
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
    request
        .validate_authorization_at(kagemusha_v2_snapshot_time_ms(app))
        .map_err(|err| {
            validation_owned(
                "offline_authorization_invalid",
                format!("Offline redemption authorization is not live at chain time: {err}"),
            )
        })
}

fn ensure_kagemusha_v2_backend_available() -> Result<(), Error> {
    if iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE {
        return Ok(());
    }
    Err(Error::AppServiceUnavailable {
        code: "offline_not_ready",
        message: "Offline proof generation and verification are not ready.".to_owned(),
    })
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

fn ensure_same_offline_request<TopUp: PartialEq, Redeem: PartialEq>(
    existing: &OfflineOperationRequest<TopUp, Redeem>,
    requested: &OfflineOperationRequest<TopUp, Redeem>,
) -> Result<(), Error> {
    if existing == requested {
        return Ok(());
    }
    Err(Error::AppConflict {
        code: "operation_id_conflict",
        message: "Offline operation id is already bound to a different request.".to_owned(),
    })
}

#[derive(Debug, Clone)]
struct OfflineOperationRecord {
    request: OfflineOperationRequestOwned,
    transaction_hash: HashOf<SignedTransaction>,
    submitted_at_ms: u64,
}

fn require_idempotency_key(headers: &HeaderMap, operation_id: [u8; 32]) -> Result<(), Error> {
    if operation_id == [0; 32] {
        return Err(Error::AppQueryValidation {
            code: "operation_id_invalid",
            message: "The signed offline operation id must be non-zero.".to_owned(),
        });
    }
    let expected = hex::encode(operation_id);
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
    if actual != expected {
        return Err(Error::AppConflict {
            code: "idempotency_key_conflict",
            message: "Idempotency-Key does not match the signed operation id.".to_owned(),
        });
    }
    Ok(())
}

fn reserve_offline_operation(
    issuer: &OfflineV2IssuerRuntime,
    request: OfflineOperationRequestRef<'_>,
    transaction_hash: HashOf<SignedTransaction>,
    submitted_at_ms: u64,
) -> Result<Option<OfflineOperationRecord>, Error> {
    let mut operations = issuer.operations.write().map_err(|_| {
        Error::Query(ValidationFail::InternalError(
            "offline operation registry lock is poisoned".to_owned(),
        ))
    })?;
    let now_ms = now_ms();
    operations.retain(|_, record| {
        record
            .request
            .authorization()
            .expires_at_ms
            .saturating_add(OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS)
            >= now_ms
    });
    let authorization = request.authorization();
    if let Some(existing) = operations.get(&authorization.operation_id) {
        // The stored transaction hash remains authoritative for an equivalent
        // replay; signing the same request is not assumed to be deterministic.
        ensure_same_offline_request(&existing.request.as_ref(), &request)?;
        return Ok(Some(existing.clone()));
    }
    operations.insert(
        authorization.operation_id,
        OfflineOperationRecord {
            request: request.into_owned(),
            transaction_hash,
            submitted_at_ms,
        },
    );
    Ok(None)
}

fn remove_reserved_offline_operation(
    issuer: &OfflineV2IssuerRuntime,
    operation_id: [u8; 32],
    transaction_hash: &HashOf<SignedTransaction>,
) {
    let Ok(mut operations) = issuer.operations.write() else {
        return;
    };
    if operations
        .get(&operation_id)
        .is_some_and(|record| record.transaction_hash == *transaction_hash)
    {
        operations.remove(&operation_id);
    }
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

fn finalized_time_ms(app: &SharedAppState, height: u64) -> u64 {
    usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .and_then(|height| app.kura.get_block(height))
        .map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        })
}

fn find_pending_offline_operation_by_id(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    let state = app.state.view();
    for accepted in app.queue.all_transactions(&state) {
        let Some(transaction) = accepted.external() else {
            continue;
        };
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions.iter() {
            let any = instruction.as_any();
            let candidate = if let Some(topup) = any.downcast_ref::<TopUpKagemushaRecursiveV2>() {
                Some(OfflineOperationRequest::TopUp(&topup.request))
            } else if let Some(redeem) = any.downcast_ref::<RedeemKagemushaRecursiveV2>() {
                Some(OfflineOperationRequest::Redeem(&redeem.request))
            } else {
                None
            };
            let Some(request) = candidate else {
                continue;
            };
            let authorization = request.authorization();
            if authorization.operation_id == operation_id {
                return Some(OfflineOperationRecord {
                    request: request.into_owned(),
                    transaction_hash: transaction.hash(),
                    submitted_at_ms: authorization.issued_at_ms,
                });
            }
        }
    }
    None
}

fn find_terminal_offline_operation_by_id(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)> {
    let mut height = u64::try_from(app.state.committed_height()).unwrap_or(u64::MAX);
    while height > 0 {
        let height_nz = usize::try_from(height).ok().and_then(NonZeroUsize::new)?;
        let Some(block) = app.kura.get_block(height_nz) else {
            height = height.saturating_sub(1);
            continue;
        };
        let block_ref = block.as_ref();
        for (index, entrypoint, result) in block_ref.entrypoint_results() {
            if index >= block_ref.external_entrypoint_count() {
                continue;
            }
            let tx = match entrypoint {
                TransactionEntrypoint::External(tx) => tx,
                TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => continue,
            };
            let Executable::Instructions(instructions) = tx.instructions() else {
                continue;
            };
            for instruction in instructions.iter() {
                let any = instruction.as_any();
                let candidate = if let Some(topup) = any.downcast_ref::<TopUpKagemushaRecursiveV2>()
                {
                    Some(OfflineOperationRequest::TopUp(&topup.request))
                } else if let Some(redeem) = any.downcast_ref::<RedeemKagemushaRecursiveV2>() {
                    Some(OfflineOperationRequest::Redeem(&redeem.request))
                } else {
                    None
                };
                let Some(request) = candidate else {
                    continue;
                };
                let authorization = request.authorization();
                if authorization.operation_id != operation_id {
                    continue;
                }
                let transaction_hash = tx.hash();
                let submitted_at_ms = authorization.issued_at_ms;
                return Some((
                    OfflineOperationRecord {
                        request: request.into_owned(),
                        transaction_hash: transaction_hash.clone(),
                        submitted_at_ms,
                    },
                    kagemusha_v2_committed_finality(
                        operation_id,
                        transaction_hash.to_string(),
                        height,
                        u64::try_from(block_ref.header().creation_time().as_millis())
                            .unwrap_or(u64::MAX),
                        result
                            .0
                            .as_ref()
                            .err()
                            .map(|reason| kagemusha_v2_rejection_detail(Some(reason))),
                    ),
                ));
            }
        }
        height = height.saturating_sub(1);
    }
    None
}

fn offline_operation_status_response(
    app: &SharedAppState,
    record: &OfflineOperationRecord,
    committed: Option<&KagemushaV2CommittedFinality>,
) -> Result<AxResponse, Error> {
    let operation_id = record.request.authorization().operation_id;
    let kind = record.request.kind();
    let operation_id_hex = hex::encode(operation_id);
    let applied = |finalized_block_height: u64, server_time_ms: u64| {
        let result = match kind {
            KagemushaV2OperationKind::TopUp => {
                let anchor = load_finalized_kagemusha_v2_anchor(app, operation_id)?;
                OfflineOperationResult::TopUp(OfflineTopUpResult {
                    transaction_hash: record.transaction_hash.to_string(),
                    finalized_block_height,
                    server_time_ms,
                    anchor,
                })
            }
            KagemushaV2OperationKind::Redeem => {
                OfflineOperationResult::Redeem(OfflineRedeemResult {
                    transaction_hash: record.transaction_hash.to_string(),
                    finalized_block_height,
                    server_time_ms,
                })
            }
        };
        Ok::<_, Error>(OfflineOperationStatus::Applied {
            operation_id: operation_id_hex.clone(),
            result,
        })
    };
    let rejected = |message: String| OfflineOperationStatus::Rejected {
        operation_id: operation_id_hex.clone(),
        kind: kind.into(),
        transaction_hash: record.transaction_hash.to_string(),
        error: iroha_torii_shared::ErrorEnvelope::new("offline_operation_rejected", message),
    };
    let status = if let Some(finality) = committed {
        match &finality.outcome {
            KagemushaV2TerminalOutcome::Applied => {
                applied(finality.finalized_block_height, finality.server_time_ms)?
            }
            KagemushaV2TerminalOutcome::Rejected(message) => rejected(message.clone()),
        }
    } else if let Some((entry, _)) =
        crate::pipeline_status_local_entry(app, &record.transaction_hash)
    {
        match entry.kind {
            crate::PipelineStatusKind::Applied => {
                let finalized_block_height = entry.block_height.map_or(0, NonZeroU64::get);
                let server_time_ms = finalized_time_ms(app, finalized_block_height);
                applied(finalized_block_height, server_time_ms)?
            }
            crate::PipelineStatusKind::Rejected | crate::PipelineStatusKind::Expired => {
                rejected(kagemusha_v2_rejection_detail(entry.rejection.as_ref()))
            }
            _ => OfflineOperationStatus::Pending {
                operation_id: operation_id_hex.clone(),
                kind: kind.into(),
                transaction_hash: record.transaction_hash.to_string(),
                submitted_at_ms: record.submitted_at_ms,
            },
        }
    } else if let Some(finality) =
        find_committed_kagemusha_v2_operation(app, record.request.as_ref())?
    {
        match finality.outcome {
            KagemushaV2TerminalOutcome::Applied => {
                applied(finality.finalized_block_height, finality.server_time_ms)?
            }
            KagemushaV2TerminalOutcome::Rejected(message) => rejected(message),
        }
    } else {
        OfflineOperationStatus::Pending {
            operation_id: operation_id_hex.clone(),
            kind: kind.into(),
            transaction_hash: record.transaction_hash.to_string(),
            submitted_at_ms: record.submitted_at_ms,
        }
    };
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
    Ok(response)
}

pub(crate) fn handle_operation_status(
    app: &SharedAppState,
    operation_id: &str,
) -> Result<AxResponse, Error> {
    let operation_id = parse_operation_id(operation_id)?;
    let record = app
        .offline_v2_issuer
        .as_ref()
        .map(|issuer| {
            issuer
                .operations
                .read()
                .map_err(|_| {
                    Error::Query(ValidationFail::InternalError(
                        "offline operation registry lock is poisoned".to_owned(),
                    ))
                })
                .map(|operations| operations.get(&operation_id).cloned())
        })
        .transpose()?
        .flatten();
    if let Some(record) = record {
        return offline_operation_status_response(app, &record, None);
    }
    if let Some(record) = find_pending_offline_operation_by_id(app, operation_id) {
        return offline_operation_status_response(app, &record, None);
    }
    if let Some((record, finality)) = find_terminal_offline_operation_by_id(app, operation_id) {
        return offline_operation_status_response(app, &record, Some(&finality));
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
    server_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KagemushaV2TerminalOutcome {
    Applied,
    Rejected(String),
}

fn kagemusha_v2_applied_finality(
    operation_id: [u8; 32],
    transaction_hash: String,
    finalized_block_height: u64,
    server_time_ms: u64,
) -> KagemushaV2CommittedFinality {
    kagemusha_v2_committed_finality(
        operation_id,
        transaction_hash,
        finalized_block_height,
        server_time_ms,
        None,
    )
}

fn kagemusha_v2_committed_finality(
    operation_id: [u8; 32],
    transaction_hash: String,
    finalized_block_height: u64,
    server_time_ms: u64,
    rejection: Option<String>,
) -> KagemushaV2CommittedFinality {
    KagemushaV2CommittedFinality {
        operation_id,
        transaction_hash,
        finalized_block_height,
        outcome: rejection.map_or(
            KagemushaV2TerminalOutcome::Applied,
            KagemushaV2TerminalOutcome::Rejected,
        ),
        server_time_ms,
    }
}

fn kagemusha_v2_rejection_detail(rejection: Option<&TransactionRejectionReason>) -> String {
    rejection.map_or_else(|| "no rejection reason".to_owned(), ToString::to_string)
}

fn find_committed_kagemusha_v2_operation(
    app: &SharedAppState,
    requested: OfflineOperationRequestRef<'_>,
) -> Result<Option<KagemushaV2CommittedFinality>, Error> {
    let authorization = requested.authorization();
    let mut height = u64::try_from(app.state.committed_height()).unwrap_or(u64::MAX);
    while height > 0 {
        let Some(height_nz) = usize::try_from(height).ok().and_then(NonZeroUsize::new) else {
            break;
        };
        let Some(block) = app.kura.get_block(height_nz) else {
            height -= 1;
            continue;
        };
        let block_ref = block.as_ref();
        for (index, entrypoint, result) in block_ref.entrypoint_results() {
            if index >= block_ref.external_entrypoint_count() {
                continue;
            }
            let tx = match entrypoint {
                TransactionEntrypoint::External(tx) => tx,
                TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => continue,
            };
            let Executable::Instructions(instructions) = tx.instructions() else {
                continue;
            };
            for instruction in instructions.iter() {
                let any = instruction.as_any();
                let candidate = if let Some(topup) = any.downcast_ref::<TopUpKagemushaRecursiveV2>()
                {
                    Some(OfflineOperationRequest::TopUp(&topup.request))
                } else if let Some(redeem) = any.downcast_ref::<RedeemKagemushaRecursiveV2>() {
                    Some(OfflineOperationRequest::Redeem(&redeem.request))
                } else {
                    None
                };
                let Some(committed) = candidate else {
                    continue;
                };
                let committed_auth = committed.authorization();
                if committed_auth.operation_id != authorization.operation_id {
                    continue;
                }
                ensure_same_offline_request(&committed, &requested)?;
                return Ok(Some(kagemusha_v2_committed_finality(
                    authorization.operation_id,
                    tx.hash().to_string(),
                    height,
                    u64::try_from(block_ref.header().creation_time().as_millis())
                        .unwrap_or(u64::MAX),
                    result
                        .0
                        .as_ref()
                        .err()
                        .map(|reason| kagemusha_v2_rejection_detail(Some(reason))),
                )));
            }
        }
        height -= 1;
    }
    Ok(None)
}

fn kagemusha_v2_anchor_state_key(operation_id: [u8; 32]) -> Result<Name, Error> {
    format!("kagemusha_v2_topup_anchor_{}", hex::encode(operation_id))
        .parse()
        .map_err(|err| {
            validation_owned(
                "offline_top_up_result_invalid",
                format!("Failed to derive the finalized top-up anchor key: {err}"),
            )
        })
}

fn load_finalized_kagemusha_v2_anchor(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<KagemushaRecursiveSpendTopUpAnchorV2, Error> {
    let key = kagemusha_v2_anchor_state_key(operation_id)?;
    let world = app.state.world_view();
    let archive = world.smart_contract_state().get(&key).ok_or_else(|| {
        validation(
            "offline_top_up_result_missing",
            "The finalized top-up anchor is missing from chain state.",
        )
    })?;
    let anchor: KagemushaRecursiveSpendTopUpAnchorV2 =
        norito::decode_from_bytes(archive).map_err(|err| {
            validation_owned(
                "offline_top_up_result_invalid",
                format!("The finalized top-up anchor is invalid: {err}"),
            )
        })?;
    anchor.validate_public_binding().map_err(|err| {
        validation_owned(
            "offline_top_up_result_invalid",
            format!("The finalized top-up anchor failed validation: {err}"),
        )
    })?;
    Ok(anchor)
}

fn require_issuer(app: &AppState) -> Result<Arc<OfflineV2IssuerRuntime>, Error> {
    app.offline_v2_issuer
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_service_unavailable",
            message: "Offline operation signing is not configured on this Torii node.".to_owned(),
        })
}

fn offline_v2_transaction_signing_error(
    context: &'static str,
    source: impl std::fmt::Display,
) -> Error {
    Error::Query(ValidationFail::InternalError(format!(
        "Offline operation signer failed to sign {context}: {source}"
    )))
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

    #[test]
    fn unavailable_v2_backend_fails_closed_with_stable_service_error() {
        let error = ensure_kagemusha_v2_backend_available()
            .expect_err("the unreleased V2 proof backend must fail closed");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "offline_not_ready",
                ..
            }
        ));
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
    fn applied_kagemusha_v2_finality_preserves_requested_operation_id() {
        let operation_id = [0x5A; 32];
        let finality =
            kagemusha_v2_applied_finality(operation_id, "transaction-hash".to_owned(), 7, 11);

        assert_eq!(finality.operation_id, operation_id);
        assert_eq!(finality.transaction_hash, "transaction-hash");
        assert_eq!(finality.finalized_block_height, 7);
        assert_eq!(finality.outcome, KagemushaV2TerminalOutcome::Applied);
        assert_eq!(finality.server_time_ms, 11);
    }

    #[test]
    fn kagemusha_v2_rejection_detail_formats_borrowed_reason() {
        assert_eq!(kagemusha_v2_rejection_detail(None), "no rejection reason");

        let rejection = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "fixture rejection".to_owned(),
        ));
        assert_eq!(
            kagemusha_v2_rejection_detail(Some(&rejection)),
            rejection.to_string()
        );
    }
}
