use std::{
    collections::BTreeMap,
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, Mutex, RwLock},
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
        error::TransactionRejectionReason, signed::TransactionResult,
    },
};
use iroha_primitives::numeric::Numeric;
use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationResult, OfflineOperationState,
    OfflineOperationStatus, OfflineRedeemRequest, OfflineRedeemResult, OfflineTopUpRequest,
    OfflineTopUpResult,
};
use mv::storage::StorageReadOnly;
use tokio::sync::watch;

use crate::{AppState, Error, SharedAppState, app_auth, routing};

const PATH_OFFLINE_TOP_UP: &str = iroha_torii_shared::uri::OFFLINE_TOP_UP;
const PATH_OFFLINE_REDEEM: &str = iroha_torii_shared::uri::OFFLINE_REDEEM;
const OFFLINE_OPERATION_RETENTION_AFTER_EXPIRY_MS: u64 = 24 * 60 * 60 * 1_000;
#[derive(Debug, Clone)]
pub(crate) struct OfflineV2IssuerRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    max_tx_value: Numeric,
    operations: Arc<RwLock<BTreeMap<[u8; 32], OfflineOperationRecord>>>,
    in_flight: Arc<Mutex<BTreeMap<[u8; 32], InFlightSubmission>>>,
}

impl OfflineV2IssuerRuntime {
    pub(crate) fn from_config(config: actual::ToriiOfflineIssuer) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            max_tx_value: config.max_tx_value,
            operations: Arc::new(RwLock::new(BTreeMap::new())),
            in_flight: Arc::new(Mutex::new(BTreeMap::new())),
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

#[derive(Debug)]
struct InFlightSubmission {
    request: OfflineOperationRequestOwned,
    token: Arc<()>,
    updates: watch::Sender<SubmissionOutcome>,
}

#[derive(Debug, Clone)]
enum SubmissionOutcome {
    Pending,
    Accepted(OfflineOperationRecord),
    Retry,
}

enum SubmissionClaim {
    Accepted(OfflineOperationRecord),
    Leader(SubmissionLeader),
    Follower(watch::Receiver<SubmissionOutcome>),
}

struct SubmissionLeader {
    issuer: Arc<OfflineV2IssuerRuntime>,
    operation_id: [u8; 32],
    token: Arc<()>,
    request: OfflineOperationRequestOwned,
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
    let (issuer, submission) = loop {
        if let Some(response) =
            find_existing_offline_operation(&app, app.offline_v2_issuer.as_deref(), requested)?
        {
            return Ok(response);
        }
        let issuer = require_issuer(&app)?;
        match issuer.claim_submission(requested)? {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_record(&record);
            }
            SubmissionClaim::Leader(submission) => break (issuer, submission),
            SubmissionClaim::Follower(receiver) => {
                match wait_for_submission_outcome(receiver).await {
                    SubmissionOutcome::Accepted(record) => {
                        return offline_operation_reference_for_record(&record);
                    }
                    SubmissionOutcome::Retry | SubmissionOutcome::Pending => continue,
                }
            }
        }
    };
    validate_kagemusha_v2_topup_snapshot(&app, &topup_request)?;
    if topup_request.amount.public_numeric() > issuer.max_tx_value.clone() {
        return Err(validation(
            "offline_amount_exceeds_limit",
            "Offline top-up amount exceeds issuer policy.",
        ));
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
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_TOP_UP,
    )
    .await?;
    let record = submission.accept(tx_hash);
    offline_operation_reference_for_record(&record)
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
    let (issuer, submission) = loop {
        if let Some(response) =
            find_existing_offline_operation(&app, app.offline_v2_issuer.as_deref(), requested)?
        {
            return Ok(response);
        }
        let issuer = require_issuer(&app)?;
        match issuer.claim_submission(requested)? {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_record(&record);
            }
            SubmissionClaim::Leader(submission) => break (issuer, submission),
            SubmissionClaim::Follower(receiver) => {
                match wait_for_submission_outcome(receiver).await {
                    SubmissionOutcome::Accepted(record) => {
                        return offline_operation_reference_for_record(&record);
                    }
                    SubmissionOutcome::Retry | SubmissionOutcome::Pending => continue,
                }
            }
        }
    };
    validate_kagemusha_v2_redeem_snapshot(&app, &redeem_request)?;
    if redeem_request.amount.public_numeric() > issuer.max_tx_value.clone() {
        return Err(validation(
            "offline_amount_exceeds_limit",
            "Offline redemption amount exceeds issuer policy.",
        ));
    }
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
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_REDEEM,
    )
    .await?;
    let record = submission.accept(tx_hash);
    offline_operation_reference_for_record(&record)
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

impl OfflineV2IssuerRuntime {
    fn claim_submission(
        self: &Arc<Self>,
        request: OfflineOperationRequestRef<'_>,
    ) -> Result<SubmissionClaim, Error> {
        let operation_id = request.authorization().operation_id;

        // The admitted-operation registry and in-flight table are checked in
        // one fixed lock order. This closes the gap between a caller's chain
        // lookup and its in-flight claim: a leader that finishes in that gap is
        // observed here as either still in flight or already admitted, never as
        // permission to submit the same operation again.
        let mut operations = self.operations.write().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline operation registry lock is poisoned".to_owned(),
            ))
        })?;
        let now_ms = now_ms();
        operations.retain(|_, stored| {
            offline_operation_is_retained(stored.request.authorization().expires_at_ms, now_ms)
        });
        if let Some(existing) = operations.get(&operation_id) {
            ensure_same_offline_request(&existing.request.as_ref(), &request)?;
            return Ok(SubmissionClaim::Accepted(existing.clone()));
        }

        let mut in_flight = self.in_flight.lock().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline submission coordinator lock is poisoned".to_owned(),
            ))
        })?;
        if let Some(existing) = in_flight.get(&operation_id) {
            ensure_same_offline_request(&existing.request.as_ref(), &request)?;
            return Ok(SubmissionClaim::Follower(existing.updates.subscribe()));
        }

        let token = Arc::new(());
        let (updates, _) = watch::channel(SubmissionOutcome::Pending);
        let request = request.into_owned();
        in_flight.insert(
            operation_id,
            InFlightSubmission {
                request: request.clone(),
                token: Arc::clone(&token),
                updates: updates.clone(),
            },
        );
        Ok(SubmissionClaim::Leader(SubmissionLeader {
            issuer: Arc::clone(self),
            operation_id,
            token,
            request,
            updates,
            active: true,
        }))
    }

    fn record_admitted_operation(
        &self,
        record: OfflineOperationRecord,
    ) -> Result<OfflineOperationRecord, Error> {
        let request = record.request.as_ref();
        let operation_id = request.authorization().operation_id;
        let mut operations = self.operations.write().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline operation registry lock is poisoned".to_owned(),
            ))
        })?;
        let now_ms = now_ms();
        operations.retain(|_, stored| {
            offline_operation_is_retained(stored.request.authorization().expires_at_ms, now_ms)
        });
        if let Some(existing) = operations.get(&operation_id) {
            ensure_same_offline_request(&existing.request.as_ref(), &request)?;
            return Ok(existing.clone());
        }
        operations.insert(operation_id, record.clone());
        Ok(record)
    }
}

impl SubmissionLeader {
    fn accept(mut self, transaction_hash: HashOf<SignedTransaction>) -> OfflineOperationRecord {
        let record = OfflineOperationRecord {
            submitted_at_ms: self.request.authorization().issued_at_ms,
            request: self.request.clone(),
            transaction_hash,
        };
        let admitted = match self.issuer.record_admitted_operation(record.clone()) {
            Ok(admitted) => admitted,
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    operation_id = %hex::encode(self.operation_id),
                    "accepted offline operation could not be cached"
                );
                record
            }
        };
        self.finish(SubmissionOutcome::Accepted(admitted.clone()));
        admitted
    }

    fn finish(&mut self, outcome: SubmissionOutcome) {
        if !self.active {
            return;
        }
        if let Ok(mut in_flight) = self.issuer.in_flight.lock()
            && in_flight
                .get(&self.operation_id)
                .is_some_and(|entry| Arc::ptr_eq(&entry.token, &self.token))
        {
            in_flight.remove(&self.operation_id);
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
    issuer: &OfflineV2IssuerRuntime,
    request: OfflineOperationRequestRef<'_>,
) -> Result<Option<OfflineOperationRecord>, Error> {
    let mut operations = issuer.operations.write().map_err(|_| {
        Error::Query(ValidationFail::InternalError(
            "offline operation registry lock is poisoned".to_owned(),
        ))
    })?;
    let now_ms = now_ms();
    operations.retain(|_, record| {
        offline_operation_is_retained(record.request.authorization().expires_at_ms, now_ms)
    });
    let operation_id = request.authorization().operation_id;
    let Some(existing) = operations.get(&operation_id) else {
        return Ok(None);
    };
    ensure_same_offline_request(&existing.request.as_ref(), &request)?;
    Ok(Some(existing.clone()))
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

fn offline_operation_reference_for_record(
    record: &OfflineOperationRecord,
) -> Result<AxResponse, Error> {
    offline_operation_reference_response(
        record.request.authorization().operation_id,
        record.request.kind().into(),
        record.transaction_hash.to_string(),
        record.submitted_at_ms,
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

fn finalized_time_ms(app: &SharedAppState, height: u64) -> u64 {
    usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .and_then(|height| app.kura.get_block(height))
        .map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        })
}

fn offline_operation_record_in_transaction(
    transaction: &SignedTransaction,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    if operation_id == [0; 32] {
        return None;
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return None;
    };
    for instruction in instructions.iter() {
        let any = instruction.as_any();
        let candidate = if let Some(top_up) = any.downcast_ref::<TopUpKagemushaRecursiveV2>() {
            Some(OfflineOperationRequest::TopUp(&top_up.request))
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
    None
}

fn signed_transaction_for_entrypoint(
    entrypoint: &TransactionEntrypoint,
) -> Option<&SignedTransaction> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => Some(transaction),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
        TransactionEntrypoint::SealedCommitment(_)
        | TransactionEntrypoint::PrivateKaigi(_)
        | TransactionEntrypoint::Time(_) => None,
    }
}

fn terminal_offline_operation_in_transaction(
    transaction: &SignedTransaction,
    result: &TransactionResult,
    operation_id: [u8; 32],
    finalized_block_height: u64,
    server_time_ms: u64,
) -> Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)> {
    let record = offline_operation_record_in_transaction(transaction, operation_id)?;
    let transaction_hash = record.transaction_hash.to_string();
    Some((
        record,
        kagemusha_v2_committed_finality(
            operation_id,
            transaction_hash,
            finalized_block_height,
            server_time_ms,
            result
                .0
                .as_ref()
                .err()
                .map(|reason| kagemusha_v2_rejection_detail(Some(reason))),
        ),
    ))
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
        if let Some(record) = offline_operation_record_in_transaction(transaction, operation_id) {
            return Some(record);
        }
    }
    None
}

fn find_existing_offline_operation(
    app: &SharedAppState,
    issuer: Option<&OfflineV2IssuerRuntime>,
    requested: OfflineOperationRequestRef<'_>,
) -> Result<Option<AxResponse>, Error> {
    if let Some(issuer) = issuer {
        if let Some(existing) = find_admitted_offline_operation(issuer, requested)? {
            return offline_operation_reference_for_record(&existing).map(Some);
        }
    }

    let authorization = requested.authorization();
    if let Some(existing) = find_pending_offline_operation_by_id(app, authorization.operation_id) {
        ensure_same_offline_request(&existing.request.as_ref(), &requested)?;
        return offline_operation_reference_response(
            authorization.operation_id,
            existing.request.kind().into(),
            existing.transaction_hash.to_string(),
            existing.submitted_at_ms,
        )
        .map(Some);
    }

    let Some(finality) = find_committed_kagemusha_v2_operation(app, requested)? else {
        return Ok(None);
    };
    offline_operation_reference_response(
        authorization.operation_id,
        requested.kind().into(),
        finality.transaction_hash,
        authorization.issued_at_ms,
    )
    .map(Some)
}

fn find_terminal_offline_operation_by_id(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)>, Error> {
    let indexed_height = app
        .kura
        .get_earliest_block_height_by_offline_operation_id(operation_id)
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_operation_index_unavailable",
            message: "The offline operation index is still being reconstructed.".to_owned(),
        })?;
    let Some(height) = indexed_height else {
        return Ok(None);
    };
    let block = app
        .kura
        .get_block(height)
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_operation_history_unavailable",
            message: "The indexed offline operation block body is not locally available."
                .to_owned(),
        })?;
    let block_ref = block.as_ref();
    let finalized_block_height = u64::try_from(height.get()).unwrap_or(u64::MAX);
    let server_time_ms =
        u64::try_from(block_ref.header().creation_time().as_millis()).unwrap_or(u64::MAX);
    for (_, entrypoint, result) in block_ref.entrypoint_results() {
        let Some(transaction) = signed_transaction_for_entrypoint(&entrypoint) else {
            continue;
        };
        if let Some(terminal) = terminal_offline_operation_in_transaction(
            transaction,
            result,
            operation_id,
            finalized_block_height,
            server_time_ms,
        ) {
            return Ok(Some(terminal));
        }
    }

    let merge_entry = app
        .kura
        .get_merge_entry_by_carrier_height(height)
        .map_err(|error| {
            iroha_logger::warn!(
                ?error,
                operation_id = %hex::encode(operation_id),
                indexed_height = height.get(),
                "failed to resolve indexed offline operation merge carrier"
            );
            Error::AppServiceUnavailable {
                code: "offline_operation_history_unavailable",
                message: "The indexed offline operation merge entry is not locally available."
                    .to_owned(),
            }
        })?;
    if let Some(batch) = merge_entry.and_then(|entry| entry.execution_batch) {
        for execution in batch.lanes {
            if execution.entrypoints.len() != execution.results.len() {
                return Err(Error::AppServiceUnavailable {
                    code: "offline_operation_index_inconsistent",
                    message: "The indexed offline merge execution has misaligned results."
                        .to_owned(),
                });
            }
            for (entrypoint, result) in execution.entrypoints.iter().zip(&execution.results) {
                let Some(transaction) = signed_transaction_for_entrypoint(entrypoint) else {
                    continue;
                };
                if let Some(terminal) = terminal_offline_operation_in_transaction(
                    transaction,
                    result,
                    operation_id,
                    finalized_block_height,
                    server_time_ms,
                ) {
                    return Ok(Some(terminal));
                }
            }
        }
    }
    Err(Error::AppServiceUnavailable {
        code: "offline_operation_index_inconsistent",
        message: "The offline operation index does not match its canonical block body.".to_owned(),
    })
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
    } else if let Some((committed_record, finality)) =
        find_terminal_offline_operation_by_id(app, operation_id)?
    {
        ensure_same_offline_request(&committed_record.request.as_ref(), &record.request.as_ref())?;
        return offline_operation_status_response(app, &committed_record, Some(&finality));
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
    if let Some((record, finality)) = find_terminal_offline_operation_by_id(app, operation_id)? {
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
    let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, authorization.operation_id)?
    else {
        return Ok(None);
    };
    ensure_same_offline_request(&record.request.as_ref(), &requested)?;
    Ok(Some(finality))
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
    use std::{sync::Barrier, time::Duration};

    use iroha_crypto::{Algorithm, Hash, Signature};
    use iroha_data_model::{
        ChainId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        offline::{
            KagemushaRequestAuthorizationV2, KagemushaScaledAmountV2,
            KagemushaSpendableNoteDescriptorV2, KagemushaVerifiedFoldBundle,
            KagemushaVerifiedFoldRecordBundle,
        },
        trigger::DataTriggerSequence,
    };

    use super::*;

    fn submission_test_issuer() -> Arc<OfflineV2IssuerRuntime> {
        let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .expect("derive offline submission coordinator fixture key");
        Arc::new(OfflineV2IssuerRuntime {
            authority: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            max_tx_value: Numeric::new(1_000, 0),
            operations: Arc::new(RwLock::new(BTreeMap::new())),
            in_flight: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    fn submission_test_request(operation_seed: u8) -> OfflineTopUpRequest {
        let key_pair = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .expect("derive offline submission request fixture key");
        let authority = AccountId::new(key_pair.public_key().clone());
        let chain_id: ChainId = "offline-submission-coordinator"
            .parse()
            .expect("fixture chain id");
        let domain_id = DomainId::try_new("offline", "universal").expect("fixture domain id");
        let definition = AssetDefinitionId::new(
            domain_id,
            "coordinator".parse().expect("fixture asset name"),
        );
        let amount = KagemushaScaledAmountV2 {
            atomic_units: 7,
            scale: 0,
        };
        let operation_id = [operation_seed; 32];
        OfflineTopUpRequest {
            asset: AssetId::new(definition.clone(), authority.clone()),
            amount,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id: chain_id.clone(),
                asset: definition.clone(),
                note_commitment: [0x61; 32],
                spend_nullifier: [0x62; 32],
                amount,
            },
            record_bundle: KagemushaVerifiedFoldRecordBundle {
                bundle: KagemushaVerifiedFoldBundle {
                    chain_id,
                    asset: definition,
                    steps: Vec::new(),
                },
                verifier_records: Vec::new(),
            },
            pallas_open_envelopes_archive: Vec::new(),
            artifact_generation: "submission-coordinator-fixture".to_owned(),
            operation_id,
            authorization: KagemushaRequestAuthorizationV2 {
                authority,
                device_id: "submission-coordinator-device".to_owned(),
                operation_id,
                issued_at_ms: 1,
                expires_at_ms: u64::MAX,
                nonce: [0x63; 32],
                payload_digest: [0x64; 32],
                app_attest_evidence_sha256: None,
                app_attest_evidence: None,
                signature: Signature::new(key_pair.private_key(), b"coordinator fixture"),
            },
        }
    }

    fn claim_test_leader(
        issuer: &Arc<OfflineV2IssuerRuntime>,
        request: &OfflineTopUpRequest,
    ) -> SubmissionLeader {
        match issuer
            .claim_submission(OfflineOperationRequest::TopUp(request))
            .expect("claim fixture submission")
        {
            SubmissionClaim::Leader(leader) => leader,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Follower(_) => {
                panic!("fresh fixture request must elect one leader")
            }
        }
    }

    fn submission_test_hash(seed: u8) -> HashOf<SignedTransaction> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32]))
    }

    fn submission_test_transaction(requests: Vec<OfflineTopUpRequest>) -> SignedTransaction {
        let issuer = submission_test_issuer();
        let instructions = requests
            .into_iter()
            .map(TopUpKagemushaRecursiveV2::new)
            .map(InstructionBox::from)
            .collect::<Vec<_>>();
        TransactionBuilder::new(
            ChainId::from("offline-submission-coordinator"),
            issuer.authority.clone().into(),
        )
        .with_instructions(instructions)
        .sign(issuer.key_pair.private_key())
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

    #[test]
    fn transaction_recovery_uses_the_authorized_nonzero_id_and_exact_matching_instruction() {
        let first = submission_test_request(0x15);
        let second = submission_test_request(0x16);
        let transaction = submission_test_transaction(vec![first.clone(), second.clone()]);

        let recovered = offline_operation_record_in_transaction(
            &transaction,
            second.authorization.operation_id,
        )
        .expect("matching second instruction must be recovered");
        assert_eq!(
            recovered.request,
            OfflineOperationRequest::TopUp(&second).into_owned()
        );
        assert_eq!(recovered.transaction_hash, transaction.hash());
        assert_eq!(recovered.submitted_at_ms, second.authorization.issued_at_ms);
        assert!(
            offline_operation_record_in_transaction(&transaction, [0x17; 32]).is_none(),
            "an attacker-controlled miss must not recover an unrelated instruction"
        );
        assert!(
            offline_operation_record_in_transaction(&transaction, [0; 32]).is_none(),
            "zero is never a valid operation identity"
        );

        let mut mismatched = submission_test_request(0x18);
        let authorized_id = mismatched.authorization.operation_id;
        mismatched.operation_id = [0x19; 32];
        let malformed_transaction = submission_test_transaction(vec![mismatched.clone()]);
        let recovered =
            offline_operation_record_in_transaction(&malformed_transaction, authorized_id)
                .expect("authorization remains the canonical retry identity");
        assert_eq!(
            recovered.request,
            OfflineOperationRequest::TopUp(&mismatched).into_owned()
        );
        assert!(
            offline_operation_record_in_transaction(
                &malformed_transaction,
                mismatched.operation_id,
            )
            .is_none(),
            "a forged duplicate top-level id must not create another lookup identity"
        );

        let issuer = submission_test_issuer();
        let unrelated = TransactionBuilder::new(
            ChainId::from("offline-submission-coordinator"),
            issuer.authority.clone().into(),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "unrelated".to_owned(),
        )])
        .sign(issuer.key_pair.private_key());
        assert!(
            offline_operation_record_in_transaction(&unrelated, authorized_id).is_none(),
            "ordinary transactions must never enter offline recovery"
        );
    }

    #[test]
    fn terminal_recovery_binds_the_exact_operation_and_preserves_both_outcomes() {
        let request = submission_test_request(0x1A);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let applied_result = TransactionResult(Ok(DataTriggerSequence::default()));
        let (applied_record, applied) = terminal_offline_operation_in_transaction(
            &transaction,
            &applied_result,
            operation_id,
            17,
            23,
        )
        .expect("matching applied operation must be reconstructed");
        assert_eq!(applied_record.transaction_hash, transaction.hash());
        assert_eq!(applied.operation_id, operation_id);
        assert_eq!(applied.transaction_hash, transaction.hash().to_string());
        assert_eq!(applied.finalized_block_height, 17);
        assert_eq!(applied.server_time_ms, 23);
        assert_eq!(applied.outcome, KagemushaV2TerminalOutcome::Applied);
        assert!(
            terminal_offline_operation_in_transaction(
                &transaction,
                &applied_result,
                [0x1B; 32],
                17,
                23,
            )
            .is_none(),
            "a transaction containing another operation must not satisfy the lookup"
        );

        let rejected_result = TransactionResult(Err(TransactionRejectionReason::Validation(
            ValidationFail::TooComplex,
        )));
        let expected_rejection = rejected_result
            .0
            .as_ref()
            .expect_err("fixture is rejected")
            .to_string();
        let (_, rejected) = terminal_offline_operation_in_transaction(
            &transaction,
            &rejected_result,
            operation_id,
            19,
            29,
        )
        .expect("matching rejected operation must be reconstructed");
        assert_eq!(
            rejected.outcome,
            KagemushaV2TerminalOutcome::Rejected(expected_rejection)
        );
        assert_eq!(rejected.finalized_block_height, 19);
        assert_eq!(rejected.server_time_ms, 29);
    }

    #[tokio::test]
    async fn submission_claim_deduplicates_and_binds_the_complete_typed_request() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x11);
        let leader = claim_test_leader(&issuer, &request);

        let follower = match issuer
            .claim_submission(OfflineOperationRequest::TopUp(&request))
            .expect("identical concurrent request must join the leader")
        {
            SubmissionClaim::Follower(receiver) => receiver,
            SubmissionClaim::Accepted(_) | SubmissionClaim::Leader(_) => {
                panic!("identical in-flight request must be a follower")
            }
        };

        let mut conflicting = request.clone();
        conflicting.artifact_generation.push_str("-forged");
        let error = match issuer.claim_submission(OfflineOperationRequest::TopUp(&conflicting)) {
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
        let admitted = leader.accept(transaction_hash);
        let observed = tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_submission_outcome(follower),
        )
        .await
        .expect("accepted submission must release every follower");
        let SubmissionOutcome::Accepted(observed) = observed else {
            panic!("accepted leader must publish the admitted operation")
        };
        assert_eq!(observed.request, admitted.request);
        assert_eq!(observed.transaction_hash, transaction_hash);

        match issuer
            .claim_submission(OfflineOperationRequest::TopUp(&request))
            .expect("admitted replay must be returned without resubmission")
        {
            SubmissionClaim::Accepted(replayed) => {
                assert_eq!(replayed.transaction_hash, transaction_hash);
                assert_eq!(replayed.request, admitted.request);
            }
            SubmissionClaim::Leader(_) | SubmissionClaim::Follower(_) => {
                panic!("admitted replay must never create or join another submission")
            }
        }
        let error = match issuer.claim_submission(OfflineOperationRequest::TopUp(&conflicting)) {
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
        assert_eq!(issuer.operations.read().expect("operations lock").len(), 1);
        assert!(issuer.in_flight.lock().expect("in-flight lock").is_empty());
    }

    #[tokio::test]
    async fn cancelled_submission_leader_releases_followers_for_retry() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x12);
        let leader = claim_test_leader(&issuer, &request);
        let follower = match issuer
            .claim_submission(OfflineOperationRequest::TopUp(&request))
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
        assert!(issuer.in_flight.lock().expect("in-flight lock").is_empty());
        assert!(
            issuer
                .operations
                .read()
                .expect("operations lock")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn panicking_submission_leader_releases_followers_without_poisoning_coordinator() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x13);
        let leader = claim_test_leader(&issuer, &request);
        let follower = match issuer
            .claim_submission(OfflineOperationRequest::TopUp(&request))
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
        assert!(issuer.in_flight.lock().expect("in-flight lock").is_empty());
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
            let mut in_flight = issuer.in_flight.lock().expect("in-flight lock");
            in_flight.insert(
                operation_id,
                InFlightSubmission {
                    request: OfflineOperationRequest::TopUp(&request).into_owned(),
                    token: Arc::clone(&replacement_token),
                    updates: replacement_updates.clone(),
                },
            );
        }

        drop(stale_leader);

        let in_flight = issuer.in_flight.lock().expect("in-flight lock");
        let replacement = in_flight
            .get(&operation_id)
            .expect("newer generation must survive stale leader drop");
        assert!(Arc::ptr_eq(&replacement.token, &replacement_token));
        drop(in_flight);
        assert!(matches!(
            &*replacement_receiver.borrow(),
            SubmissionOutcome::Pending
        ));

        issuer
            .in_flight
            .lock()
            .expect("in-flight lock")
            .remove(&operation_id);
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
                claim_issuer.claim_submission(OfflineOperationRequest::TopUp(&claim_request))
            });
            barrier.wait();
            let admitted = leader.accept(submission_test_hash(seed));
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
