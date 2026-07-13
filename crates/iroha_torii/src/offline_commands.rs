use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{http::HeaderMap, response::Response as AxResponse};
use iroha_config::parameters::actual;
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{Hash, HashOf, KeyPair};
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
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationResult, OfflineOperationState,
    OfflineOperationStatus, OfflineRedeemRequest, OfflineRedeemResult, OfflineTopUpFinalityProof,
    OfflineTopUpRequest, OfflineTopUpResult,
};
use mv::storage::StorageReadOnly;
use tokio::sync::watch;

use crate::{AppState, Error, SharedAppState, app_auth, routing};

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
    max_tx_value: Quantity,
    admission: Arc<Mutex<OfflineOperationRegistry>>,
}

impl OfflineCommandRuntime {
    pub(crate) fn from_config(config: actual::ToriiKagemushaCommands) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            max_tx_value: config.max_tx_value,
            admission: Arc::new(Mutex::new(OfflineOperationRegistry::new(
                config.operation_registry_max_entries,
                config.operation_registry_max_bytes,
            ))),
        }
    }

    fn sign_transaction(
        &self,
        transaction: TransactionBuilder,
        context: &'static str,
    ) -> Result<SignedTransaction, Error> {
        transaction
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
    let issuer = require_issuer(&app)?;
    let submission = loop {
        if let Some(response) =
            find_existing_offline_operation(&app, &issuer, requested, &requested_binding)?
        {
            return Ok(response);
        }
        match issuer.claim_submission(requested_binding)? {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_admitted_record(&record);
            }
            SubmissionClaim::Leader(submission) => break submission,
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
    // Once the claim is ours, repeat the authoritative queue/Kura lookup before
    // signing; any earlier leader is either still represented in-flight or
    // recoverable from one of those durable transaction sources.
    if let Some(response) =
        find_existing_offline_operation(&app, &issuer, requested, &requested_binding)?
    {
        drop(submission);
        return Ok(response);
    }
    validate_kagemusha_v2_topup_snapshot(&app, &topup_request)?;
    if topup_request.amount.public_quantity() > issuer.max_tx_value.clone() {
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
    let admission = routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_TOP_UP,
    )
    .await;
    if let Err(error) = admission {
        return reconcile_duplicate_queue_admission(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    let record = submission.accept(tx_hash)?;
    offline_operation_reference_for_admitted_record(&record)
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
    let issuer = require_issuer(&app)?;
    let submission = loop {
        if let Some(response) =
            find_existing_offline_operation(&app, &issuer, requested, &requested_binding)?
        {
            return Ok(response);
        }
        match issuer.claim_submission(requested_binding)? {
            SubmissionClaim::Accepted(record) => {
                return offline_operation_reference_for_admitted_record(&record);
            }
            SubmissionClaim::Leader(submission) => break submission,
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
    if let Some(response) =
        find_existing_offline_operation(&app, &issuer, requested, &requested_binding)?
    {
        drop(submission);
        return Ok(response);
    }
    validate_kagemusha_v2_redeem_snapshot(&app, &redeem_request)?;
    if redeem_request.amount.public_quantity() > issuer.max_tx_value.clone() {
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
    let admission = routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_OFFLINE_REDEEM,
    )
    .await;
    if let Err(error) = admission {
        return reconcile_duplicate_queue_admission(
            &app,
            &issuer,
            requested,
            &requested_binding,
            error,
        );
    }
    let record = submission.accept(tx_hash)?;
    offline_operation_reference_for_admitted_record(&record)
}

fn kagemusha_v2_snapshot_time_ms(state: &impl StateReadOnly) -> u64 {
    state.latest_block().map_or(0, |block| {
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
    let zk_state = world
        .zk_assets()
        .get(request.asset.definition())
        .ok_or_else(|| {
            validation(
                "offline_confidential_state_unavailable",
                "Offline top-up asset has no confidential tree state.",
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
    let authoritative_initial_root =
        iroha_core::zk::confidential_v2::compute_confidential_root_v2(&zk_state.commitments)
            .map_err(|error| {
                validation_owned(
                    "offline_confidential_state_invalid",
                    format!("Offline confidential tree is invalid: {error}"),
                )
            })?;
    let authoritative_leaf_index = u32::try_from(zk_state.commitments.len()).map_err(|_| {
        validation(
            "offline_topup_tree_full",
            "Offline confidential tree position exceeds the protocol index.",
        )
    })?;
    if authoritative_leaf_index
        >= iroha_data_model::offline::KAGEMUSHA_TOPUP_SHIELD_TREE_CAPACITY_V2
        || zk_state
            .commitments
            .contains(&request.current_note.note_commitment)
        || zk_state
            .nullifiers
            .contains(&request.current_note.spend_nullifier)
    {
        return Err(validation(
            "offline_topup_state_conflict",
            "Offline top-up note conflicts with existing confidential state.",
        ));
    }
    let mut commitments_after = zk_state.commitments.clone();
    commitments_after.push(request.current_note.note_commitment);
    let authoritative_finalized_root =
        iroha_core::zk::confidential_v2::compute_confidential_root_v2(&commitments_after).map_err(
            |error| {
                validation_owned(
                    "offline_confidential_state_invalid",
                    format!("Offline confidential tree is invalid after append: {error}"),
                )
            },
        )?;
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
        .validate_authorization_at(kagemusha_v2_snapshot_time_ms(&state))
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
    request
        .validate_authorization_at(kagemusha_v2_snapshot_time_ms(&state))
        .map_err(|err| {
            validation_owned(
                "offline_authorization_invalid",
                format!("Offline redemption authorization is not live at chain time: {err}"),
            )
        })
}

fn ensure_kagemusha_v2_backend_available() -> Result<(), Error> {
    if iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE {
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
            canonical_request_digest: canonical_offline_request_digest(request)?,
            submitted_at_ms: authorization.issued_at_ms,
            expires_at_ms: authorization.expires_at_ms,
        })
    }
}

fn canonical_offline_request_digest(
    request: OfflineOperationRequestRef<'_>,
) -> Result<[u8; 32], Error> {
    let canonical = match request {
        OfflineOperationRequest::TopUp(request) => norito::to_bytes(request),
        OfflineOperationRequest::Redeem(request) => norito::to_bytes(request),
    }
    .map_err(|source| Error::SerializationFailure {
        context: "offline_request_binding",
        source: Box::new(source),
    })?;
    Ok(Hash::new(canonical).into())
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
    transaction_hash: HashOf<SignedTransaction>,
    submitted_at_ms: u64,
}

impl OfflineOperationRecord {
    fn binding(&self) -> Result<OfflineOperationRequestBinding, Error> {
        OfflineOperationRequestBinding::from_request(self.request.as_ref())
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
        let mut admission = self.admission.lock().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline operation admission registry lock is poisoned".to_owned(),
            ))
        })?;
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
        let mut admission = self.admission.lock().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline operation admission registry lock is poisoned".to_owned(),
            ))
        })?;
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
        if let Ok(mut admission) = self.issuer.admission.lock()
            && admission
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
) -> Result<Option<AdmittedOfflineOperationRecord>, Error> {
    let mut admission = issuer.admission.lock().map_err(|_| {
        Error::Query(ValidationFail::InternalError(
            "offline operation admission registry lock is poisoned".to_owned(),
        ))
    })?;
    let now_ms = now_ms();
    admission.prune_expired(now_ms);
    let operation_id = requested_binding.operation_id;
    let Some(existing) = admission.get(&operation_id) else {
        return Ok(None);
    };
    ensure_same_offline_request_binding(&existing.binding, requested_binding)?;
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
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
    finalized_block_height: u64,
    server_time_ms: u64,
) -> Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)> {
    let record =
        offline_operation_record_in_transaction(transaction, issuer_authority, operation_id)?;
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
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Option<OfflineOperationRecord> {
    let state = app.state.view();
    for accepted in app.queue.all_transactions(&state) {
        let Some(transaction) = accepted.external() else {
            continue;
        };
        if let Some(record) =
            offline_operation_record_in_transaction(transaction, issuer_authority, operation_id)
        {
            return Some(record);
        }
    }
    None
}

fn find_existing_offline_operation(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    requested: OfflineOperationRequestRef<'_>,
    requested_binding: &OfflineOperationRequestBinding,
) -> Result<Option<AxResponse>, Error> {
    if let Some(existing) = find_admitted_offline_operation(issuer, requested_binding)? {
        return offline_operation_reference_for_admitted_record(&existing).map(Some);
    }

    let authorization = requested.authorization();
    if let Some(existing) =
        find_pending_offline_operation_by_id(app, &issuer.authority, authorization.operation_id)
    {
        ensure_same_offline_request(&existing.request.as_ref(), &requested)?;
        return offline_operation_reference_response(
            authorization.operation_id,
            existing.request.kind().into(),
            existing.transaction_hash.to_string(),
            existing.submitted_at_ms,
        )
        .map(Some);
    }

    let Some(finality) = find_committed_kagemusha_v2_operation(app, issuer, requested)? else {
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

fn is_duplicate_queue_admission_error(error: &Error) -> bool {
    matches!(
        error,
        Error::PushIntoQueue { source, .. }
            if matches!(
                source.as_ref(),
                iroha_core::queue::Error::InBlockchain | iroha_core::queue::Error::IsInQueue
            )
    )
}

fn reconcile_duplicate_queue_admission(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    requested: OfflineOperationRequestRef<'_>,
    requested_binding: &OfflineOperationRequestBinding,
    admission_error: Error,
) -> Result<AxResponse, Error> {
    if !is_duplicate_queue_admission_error(&admission_error) {
        return Err(admission_error);
    }

    match find_existing_offline_operation(app, issuer, requested, requested_binding)? {
        Some(response) => Ok(response),
        None => Err(admission_error),
    }
}

fn find_terminal_offline_operation_by_id(
    app: &SharedAppState,
    issuer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<(OfflineOperationRecord, KagemushaV2CommittedFinality)>, Error> {
    let indexed_height = app
        .kura
        .get_earliest_block_height_by_offline_operation_id(issuer_authority, operation_id)
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
            issuer_authority,
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
                    issuer_authority,
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

fn ensure_admitted_operation_matches_recovered_record(
    admitted: &AdmittedOfflineOperationRecord,
    recovered: &OfflineOperationRecord,
) -> Result<(), Error> {
    let recovered_binding = recovered.binding()?;
    if admitted.binding != recovered_binding
        || admitted.transaction_hash != recovered.transaction_hash
    {
        return Err(offline_operation_index_inconsistent(
            "The authoritative offline operation differs from its admitted request binding.",
        ));
    }
    Ok(())
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
            kagemusha_v2_rejection_detail(entry.rejection.as_ref()),
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
    let kind = record.request.kind();
    let operation_id_hex = hex::encode(operation_id);
    let applied = |finalized_block_height: u64, server_time_ms: u64| {
        if finalized_block_height == 0 || server_time_ms == 0 {
            return Err(offline_operation_index_inconsistent(
                "An applied offline operation requires a committed height and block timestamp.",
            ));
        }
        let result = match kind {
            KagemushaV2OperationKind::TopUp => {
                let anchor = load_finalized_kagemusha_v2_anchor(app, operation_id)?;
                let OfflineOperationRequest::TopUp(request) = &record.request else {
                    unreachable!("the operation kind was derived from the same typed request")
                };
                ensure_kagemusha_v2_topup_anchor_matches_request(&anchor, request)?;
                ensure_kagemusha_v2_anchor_finality_binding(
                    anchor.topup_operation_id,
                    anchor.finalized_tx_hash,
                    anchor.finalized_height,
                    operation_id,
                    &record.transaction_hash,
                    finalized_block_height,
                )?;
                let finality_proof = load_finalized_kagemusha_v2_topup_proof(
                    app,
                    finalized_block_height,
                    operation_id,
                    &anchor,
                )?;
                OfflineOperationResult::TopUp(OfflineTopUpResult {
                    transaction_hash: record.transaction_hash.to_string(),
                    finalized_block_height,
                    server_time_ms,
                    anchor,
                    finality_proof,
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
    let rejected = |message: String| {
        rejected_offline_operation_status(operation_id, kind, &record.transaction_hash, message)
    };
    let status = if let Some(finality) = committed {
        ensure_kagemusha_v2_terminal_finality_matches_record(record, finality)?;
        match &finality.outcome {
            KagemushaV2TerminalOutcome::Applied => {
                applied(finality.finalized_block_height, finality.server_time_ms)?
            }
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
                    let Some((committed_record, finality)) = find_terminal_offline_operation_by_id(
                        app,
                        &issuer.authority,
                        operation_id,
                    )?
                    else {
                        return Err(offline_operation_index_inconsistent(
                            "The applied offline operation is absent from the canonical operation index.",
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
    } else if known_pending_in_queue {
        // The queue scan is authoritative pending provenance for this poll.
        // Do not consult a temporarily incomplete Kura operation index and
        // turn a known pending operation into a spurious 503 during rebuild.
        pending_offline_operation_status(
            operation_id,
            kind,
            &record.transaction_hash,
            record.submitted_at_ms,
        )
    } else if let Some((committed_record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        ensure_same_offline_request(&committed_record.request.as_ref(), &record.request.as_ref())?;
        return offline_operation_status_response(
            app,
            issuer,
            &committed_record,
            Some(&finality),
            false,
        );
    } else {
        let state = app.state.view();
        ensure_unproven_pending_window_is_live(
            kagemusha_v2_snapshot_time_ms(&state),
            record.request.authorization().expires_at_ms,
        )?;
        pending_offline_operation_status(
            operation_id,
            kind,
            &record.transaction_hash,
            record.submitted_at_ms,
        )
    };
    Ok(respond_with_offline_operation_status(status))
}

fn admitted_offline_operation_status_response(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    admitted: &AdmittedOfflineOperationRecord,
) -> Result<AxResponse, Error> {
    let operation_id = admitted.binding.operation_id;
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
                let Some((record, finality)) =
                    find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
                else {
                    return Err(offline_operation_index_inconsistent(
                        "The applied offline operation is absent from the canonical operation index.",
                    ));
                };
                ensure_admitted_operation_matches_recovered_record(admitted, &record)?;
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

    if let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        ensure_admitted_operation_matches_recovered_record(admitted, &record)?;
        return offline_operation_status_response(app, issuer, &record, Some(&finality), false);
    }

    let state = app.state.view();
    ensure_unproven_pending_window_is_live(
        kagemusha_v2_snapshot_time_ms(&state),
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
    let issuer = require_issuer(app)?;
    let admitted = {
        let mut admission = issuer.admission.lock().map_err(|_| {
            Error::Query(ValidationFail::InternalError(
                "offline operation admission registry lock is poisoned".to_owned(),
            ))
        })?;
        admission.prune_expired(now_ms());
        admission.get(&operation_id).cloned()
    };
    if let Some(admitted) = admitted {
        if let Some(pending) =
            find_pending_offline_operation_by_id(app, &issuer.authority, operation_id)
        {
            ensure_admitted_operation_matches_recovered_record(&admitted, &pending)?;
            return offline_operation_status_response(app, &issuer, &pending, None, true);
        }
        return admitted_offline_operation_status_response(app, &issuer, &admitted);
    }
    if let Some(record) = find_pending_offline_operation_by_id(app, &issuer.authority, operation_id)
    {
        return offline_operation_status_response(app, &issuer, &record, None, true);
    }
    if let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, operation_id)?
    {
        return offline_operation_status_response(app, &issuer, &record, Some(&finality), false);
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
        outcome: rejection.map_or(KagemushaV2TerminalOutcome::Applied, |message| {
            KagemushaV2TerminalOutcome::Rejected(canonical_offline_rejection_message(message))
        }),
        server_time_ms,
    }
}

fn kagemusha_v2_rejection_detail(rejection: Option<&TransactionRejectionReason>) -> String {
    canonical_offline_rejection_message(
        rejection.map_or_else(|| "no rejection reason".to_owned(), ToString::to_string),
    )
}

fn canonical_offline_rejection_message(message: String) -> String {
    if crate::utils::is_valid_error_message(&message) {
        message
    } else {
        "The offline operation was rejected.".to_owned()
    }
}

fn offline_operation_index_inconsistent(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "offline_operation_index_inconsistent",
        message: message.into(),
    }
}

fn ensure_unproven_pending_window_is_live(
    snapshot_time_ms: u64,
    expires_at_ms: u64,
) -> Result<(), Error> {
    if snapshot_time_ms <= expires_at_ms {
        return Ok(());
    }
    Err(offline_operation_index_inconsistent(
        "The accepted offline operation expired without queue, pipeline, or canonical terminal provenance.",
    ))
}

fn ensure_kagemusha_v2_terminal_finality_matches_record(
    record: &OfflineOperationRecord,
    finality: &KagemushaV2CommittedFinality,
) -> Result<(), Error> {
    if finality.operation_id == [0; 32]
        || finality.operation_id != record.request.authorization().operation_id
        || finality.transaction_hash != record.transaction_hash.to_string()
        || finality.finalized_block_height == 0
        || finality.server_time_ms == 0
    {
        return Err(offline_operation_index_inconsistent(
            "The terminal offline operation identity, transaction, height, or timestamp is incomplete.",
        ));
    }
    Ok(())
}

fn find_committed_kagemusha_v2_operation(
    app: &SharedAppState,
    issuer: &OfflineCommandRuntime,
    requested: OfflineOperationRequestRef<'_>,
) -> Result<Option<KagemushaV2CommittedFinality>, Error> {
    let authorization = requested.authorization();
    let Some((record, finality)) =
        find_terminal_offline_operation_by_id(app, &issuer.authority, authorization.operation_id)?
    else {
        return Ok(None);
    };
    ensure_same_offline_request(&record.request.as_ref(), &requested)?;
    Ok(Some(finality))
}

fn kagemusha_v2_anchor_state_key(operation_id: [u8; 32]) -> Result<Name, Error> {
    if operation_id == [0; 32] {
        return Err(offline_operation_index_inconsistent(
            "A finalized top-up anchor requires a non-zero operation id.",
        ));
    }
    format!("kagemusha_v2_topup_anchor_{}", hex::encode(operation_id))
        .parse()
        .map_err(|err| {
            offline_operation_index_inconsistent(format!(
                "Failed to derive the finalized top-up anchor key: {err}"
            ))
        })
}

fn ensure_kagemusha_v2_topup_anchor_matches_request(
    anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
    request: &OfflineTopUpRequest,
) -> Result<(), Error> {
    if anchor.chain_id != request.current_note.chain_id
        || anchor.payer != request.authorization.authority
        || anchor.asset != request.asset
        || anchor.asset_scale != request.amount.scale
        || anchor.amount != request.amount
        || anchor.current_note != request.current_note
        || anchor.topup_operation_id != request.authorization.operation_id
        || anchor.topup_operation_id != request.operation_id
        || anchor.artifact_binding != request.artifact_binding
    {
        return Err(offline_operation_index_inconsistent(
            "The finalized top-up anchor does not match the admitted signed request.",
        ));
    }
    Ok(())
}

fn ensure_kagemusha_v2_anchor_finality_binding(
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
        return Err(offline_operation_index_inconsistent(
            "The top-up anchor operation, transaction, or height differs from terminal finality.",
        ));
    }
    Ok(())
}

fn load_finalized_kagemusha_v2_anchor(
    app: &SharedAppState,
    operation_id: [u8; 32],
) -> Result<KagemushaRecursiveSpendTopUpAnchorV2, Error> {
    let key = kagemusha_v2_anchor_state_key(operation_id)?;
    let world = app.state.world_view();
    let archive = world.smart_contract_state().get(&key).ok_or_else(|| {
        offline_operation_index_inconsistent(
            "The finalized top-up anchor is missing from canonical chain state.",
        )
    })?;
    let anchor: KagemushaRecursiveSpendTopUpAnchorV2 =
        norito::decode_from_bytes(archive).map_err(|err| {
            offline_operation_index_inconsistent(format!(
                "The finalized top-up anchor is invalid: {err}"
            ))
        })?;
    anchor.validate_public_binding().map_err(|err| {
        offline_operation_index_inconsistent(format!(
            "The finalized top-up anchor failed validation: {err}"
        ))
    })?;
    let canonical = norito::to_bytes(&anchor).map_err(|err| {
        offline_operation_index_inconsistent(format!(
            "The finalized top-up anchor could not be canonically re-encoded: {err}"
        ))
    })?;
    if anchor.topup_operation_id != operation_id || canonical.as_slice() != archive.as_slice() {
        return Err(offline_operation_index_inconsistent(
            "The finalized top-up anchor has a mismatched operation id or non-canonical encoding.",
        ));
    }
    Ok(anchor)
}

fn load_finalized_kagemusha_v2_topup_proof(
    app: &SharedAppState,
    finalized_block_height: u64,
    operation_id: [u8; 32],
    anchor: &KagemushaRecursiveSpendTopUpAnchorV2,
) -> Result<OfflineTopUpFinalityProof, Error> {
    let proof = app
        .kura
        .kagemusha_topup_finality_proof_v2(finalized_block_height, operation_id)
        .map_err(|_| offline_topup_finality_proof_unavailable())?
        .ok_or_else(offline_topup_finality_proof_unavailable)?;
    let anchor_ref = anchor
        .compact_ref()
        .map_err(|_| offline_topup_finality_proof_unavailable())?;
    if proof.anchor != anchor_ref || proof.validate_structure().is_err() {
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

fn require_issuer(app: &AppState) -> Result<Arc<OfflineCommandRuntime>, Error> {
    app.offline_commands
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "offline_service_unavailable",
            message: "Offline operation signing is not configured on this Torii node.".to_owned(),
        })
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
    use std::{
        num::{NonZeroU64, NonZeroUsize},
        sync::Barrier,
        time::Duration,
    };

    use axum::response::IntoResponse as _;
    use iroha_config::{
        base::WithOrigin,
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
            defaults::kura,
        },
    };
    use iroha_core::kura::Kura;
    use iroha_crypto::{Algorithm, Hash, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId,
        asset::{AssetDefinitionId, AssetId},
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
            KagemushaRecursiveSpendArtifactBindingV3, KagemushaRequestAuthorizationV2,
            KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
            KagemushaTopUpShieldEvidenceV2,
        },
        peer::PeerId,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::signed::TransactionResultInner,
        trigger::DataTriggerSequence,
    };
    use tempfile::TempDir;

    use super::*;

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
            max_tx_value: Quantity::from(1_000_u32),
            admission: Arc::new(Mutex::new(OfflineOperationRegistry::new(
                NonZeroUsize::new(max_entries).expect("positive registry count"),
                NonZeroUsize::new(max_accounted_bytes).expect("positive registry byte budget"),
            ))),
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
        let issued_at_ms = now_ms().max(1);
        let mut request = OfflineTopUpRequest {
            asset: AssetId::new(definition.clone(), authority.clone()),
            amount,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id: chain_id.clone(),
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
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV3 {
                generation: "submission-coordinator-fixture".to_owned(),
                manifest_sha256: [0x69; 32],
            },
            operation_id,
            authorization: KagemushaRequestAuthorizationV2 {
                authority,
                device_id: "submission-coordinator-device".to_owned(),
                operation_id,
                issued_at_ms,
                expires_at_ms: issued_at_ms
                    .saturating_add(KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2),
                nonce: [0x63; 32],
                payload_digest: [0x64; 32],
                app_attest_evidence_sha256: None,
                app_attest_evidence: None,
                signature: Signature::new(key_pair.private_key(), b"placeholder"),
            },
        };
        let signing_bytes = request
            .authorization
            .signing_bytes()
            .expect("encode exact offline authorization signing bytes");
        request.authorization.signature = Signature::new(key_pair.private_key(), &signing_bytes);
        request
            .authorization
            .signature
            .verify(request.authorization.authority.signatory(), &signing_bytes)
            .expect("offline authorization fixture signature must bind the exact typed fields");
        request
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

    fn submission_test_transaction(requests: Vec<OfflineTopUpRequest>) -> SignedTransaction {
        let issuer = submission_test_issuer();
        let instructions = requests
            .into_iter()
            .map(TopUpKagemushaRecursiveV2::new)
            .map(InstructionBox::from)
            .collect::<Vec<_>>();
        let transaction = TransactionBuilder::new(
            ChainId::from("offline-submission-coordinator"),
            issuer.authority.clone().into(),
        )
        .with_instructions(instructions)
        .sign(issuer.key_pair.private_key());
        transaction
            .verify_signature()
            .expect("offline history fixture transaction must carry an exact valid signature");
        transaction
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
        let entrypoint_hashes = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect::<Vec<_>>();
        assert_eq!(
            entrypoint_hashes.len(),
            results.len(),
            "every ordinary history entrypoint needs one deterministic result"
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
            .set_transaction_results(Vec::new(), &entrypoint_hashes, results)
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
            init_mode: InitMode::Strict,
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
            block_sync_roster_retention: kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: kura::ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas: kura::EVICTION_REQUIRED_REPLICAS,
        }
    }

    fn app_with_offline_history(
        kura: Arc<Kura>,
        issuer: Arc<OfflineCommandRuntime>,
    ) -> SharedAppState {
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let inner = Arc::get_mut(&mut app).expect("fresh test AppState must be uniquely owned");
        inner.kura = kura;
        inner.offline_commands = Some(issuer);
        app
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

    fn merge_history_settlement(lane_incarnation: Hash) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::SINGLE,
            lane_incarnation,
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
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
        let execution = MergeLaneExecution {
            source_bundle: vec![0xA5],
            source_bundle_hash: Hash::new(b"offline-status-merge-source"),
            proposal: proposal.clone(),
            origin_proposal: proposal,
            prepare_qc,
            commit_qc,
            signer_proofs: Vec::new(),
            autonomous_chain_id_hash: Hash::new(b"offline-status-merge-chain"),
            autonomous_epoch: 1,
            autonomous_payload_hash: Hash::new(b"offline-status-merge-payload"),
            entrypoint_hashes,
            entrypoints: vec![entrypoint],
            reservation_keys: vec![vec![0x01]],
            routing_plans: vec![vec![0x02]],
            native_amx_receipts: vec![None],
            result_hashes,
            results: vec![result],
            settlement_commitment: settlement_commitment.clone(),
            settlement_hash,
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
                Hash::new(b"offline-status-merge-chain"),
                VALIDATOR_SET_HASH_VERSION_V1,
                HashOf::new(&validator_set),
                validator_set.clone(),
                Vec::new(),
                Vec::new(),
                vec![0xAA],
                Hash::new(b"offline-status-merge-qc-message"),
            ),
            execution_batch: Some(execution_batch),
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
    fn transaction_recovery_uses_the_authorized_nonzero_id_and_exact_matching_instruction() {
        let issuer = submission_test_issuer();
        let first = submission_test_request(0x15);
        let second = submission_test_request(0x16);
        let transaction = submission_test_transaction(vec![first.clone(), second.clone()]);

        let recovered = offline_operation_record_in_transaction(
            &transaction,
            &issuer.authority,
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
        let recovered = offline_operation_record_in_transaction(
            &malformed_transaction,
            &issuer.authority,
            authorized_id,
        )
        .expect("authorization remains the canonical retry identity");
        assert_eq!(
            recovered.request,
            OfflineOperationRequest::TopUp(&mismatched).into_owned()
        );
        assert!(
            offline_operation_record_in_transaction(
                &malformed_transaction,
                &issuer.authority,
                mismatched.operation_id,
            )
            .is_none(),
            "a forged duplicate top-level id must not create another lookup identity"
        );

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
            offline_operation_record_in_transaction(&unrelated, &issuer.authority, authorized_id,)
                .is_none(),
            "ordinary transactions must never enter offline recovery"
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
            ChainId::from("offline-submission-coordinator"),
            AccountId::new(front_runner.public_key().clone()),
        )
        .with_instructions([InstructionBox::from(TopUpKagemushaRecursiveV2::new(
            request.clone(),
        ))])
        .sign(front_runner.private_key());
        let rejected_result = TransactionResult(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(
                "outer authority is not the configured Kagemusha submission authority".to_owned(),
            ),
        )));

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
            terminal_offline_operation_in_transaction(
                &front_runner_transaction,
                &rejected_result,
                &issuer.authority,
                operation_id,
                1,
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
        let applied_result = TransactionResult(Ok(DataTriggerSequence::default()));
        let (applied_record, applied) = terminal_offline_operation_in_transaction(
            &transaction,
            &applied_result,
            &issuer.authority,
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
                &issuer.authority,
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
            &issuer.authority,
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
    async fn ordinary_canonical_history_survives_restart_with_an_empty_operation_registry() {
        let request = submission_test_request(0x81);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let transaction_hash = transaction.hash();
        let creation_time_ms = request.authorization.issued_at_ms;
        let block = signed_history_block(
            1,
            None,
            creation_time_ms,
            vec![transaction],
            vec![TransactionResultInner::Err(
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    "canonical offline history rejection".to_owned(),
                )),
            )],
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(block)
            .expect("store canonical offline history block");

        let restarted_issuer = submission_test_issuer();
        {
            let admission = restarted_issuer
                .admission
                .lock()
                .expect("operation admission lock");
            assert!(
                admission.records.is_empty() && admission.in_flight.is_empty(),
                "restart fixture must not rely on the process-local admission registry"
            );
        }
        let app = app_with_offline_history(Arc::clone(&kura), Arc::clone(&restarted_issuer));
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
        assert_eq!(finality.server_time_ms, creation_time_ms);
        assert!(matches!(
            finality.outcome,
            KagemushaV2TerminalOutcome::Rejected(_)
        ));

        let response = handle_operation_status(&app, &hex::encode(operation_id))
            .expect("restart status must reconstruct the operation from canonical history");
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

    #[tokio::test]
    async fn committed_merge_execution_history_is_resolved_from_its_carrier_sidecar() {
        let request = submission_test_request(0x82);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request.clone()]);
        let transaction_hash = transaction.hash();
        let kura = Kura::blank_kura_for_testing();
        let parent = signed_history_block(1, None, 101, Vec::new(), Vec::new());
        let parent_hash = parent.hash();
        kura.store_block(parent)
            .expect("store merge carrier parent block");
        let carrier = signed_history_block(2, Some(parent_hash), 202, Vec::new(), Vec::new());
        let entry = committed_merge_history_entry(
            transaction,
            TransactionResult(Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("certified merge rejection".to_owned()),
            ))),
            &carrier.header(),
        );
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
        let app = app_with_offline_history(Arc::clone(&kura), Arc::clone(&restarted_issuer));
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
        assert_eq!(finality.server_time_ms, 202);
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
    fn partially_reconstructed_kura_index_fails_closed_instead_of_reporting_not_found() {
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
            .expect("store indexed history prefix before snapshot recovery");
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
        assert_eq!(
            kura.get_earliest_block_height_by_offline_operation_id(
                &issuer.authority,
                operation_id,
            ),
            None,
            "a verified snapshot suffix must expose reconstruction as unknown, not as a miss"
        );
        let app = app_with_offline_history(kura, issuer);
        let error = handle_operation_status(&app, &hex::encode(operation_id))
            .expect_err("partial canonical index must make status temporarily unavailable");
        assert_offline_history_error(error, "offline_operation_index_unavailable");
    }

    #[test]
    fn indexed_operation_with_missing_block_body_fails_closed() {
        let request = submission_test_request(0x84);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let directory = TempDir::new().expect("create evicted Kura fixture directory");
        let config = persistent_kura_config(&directory);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("create evicted offline history Kura");
        let block1 = signed_history_block(1, None, 401, Vec::new(), Vec::new());
        let block1_hash = block1.hash();
        kura.store_block(block1).expect("store eviction block 1");
        let block2 = signed_history_block(
            2,
            Some(block1_hash),
            402,
            vec![transaction],
            vec![TransactionResultInner::Err(
                TransactionRejectionReason::Validation(ValidationFail::TooComplex),
            )],
        );
        let block2_hash = block2.hash();
        kura.store_block(block2)
            .expect("store indexed offline history block");
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
        let indexed_height = NonZeroUsize::new(2).expect("history height is non-zero");
        assert_eq!(
            kura.get_earliest_block_height_by_offline_operation_id(
                &issuer.authority,
                operation_id,
            ),
            Some(Some(indexed_height)),
        );
        let data_path = RuntimeLaneConfig::default()
            .primary()
            .blocks_dir(directory.path())
            .join("blocks.data");
        assert!(
            data_path.exists(),
            "the canonical body data file must exist before adversarial loss"
        );
        std::fs::remove_file(&data_path)
            .expect("simulate adversarial loss of the indexed local block body");
        assert!(
            kura.get_block(indexed_height).is_none(),
            "the index remains authoritative while the local body is unavailable"
        );

        let app = app_with_offline_history(kura, issuer);
        let error = handle_operation_status(&app, &hex::encode(operation_id))
            .expect_err("an indexed operation cannot be answered without its canonical body");
        assert_offline_history_error(error, "offline_operation_history_unavailable");
    }

    #[test]
    fn misaligned_committed_merge_execution_is_never_zipped_or_partially_trusted() {
        let request = submission_test_request(0x85);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let kura = Kura::blank_kura_for_testing();
        let parent = signed_history_block(1, None, 501, Vec::new(), Vec::new());
        let parent_hash = parent.hash();
        kura.store_block(parent)
            .expect("store malformed merge carrier parent");
        let carrier = signed_history_block(2, Some(parent_hash), 502, Vec::new(), Vec::new());
        let mut entry = committed_merge_history_entry(
            transaction,
            TransactionResult(Err(TransactionRejectionReason::Validation(
                ValidationFail::TooComplex,
            ))),
            &carrier.header(),
        );
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
        assert_eq!(
            kura.get_earliest_block_height_by_offline_operation_id(
                &issuer.authority,
                operation_id,
            ),
            Some(NonZeroUsize::new(2)),
            "the index deliberately points at the malformed carrier under test"
        );
        let app = app_with_offline_history(kura, Arc::clone(&issuer));
        let error = find_terminal_offline_operation_by_id(&app, &issuer.authority, operation_id)
            .expect_err("misaligned entrypoint/result history must fail closed");
        assert_offline_history_error(error, "offline_operation_index_inconsistent");
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
            .expect("operation admission lock")
            .insert_reserved(AdmittedOfflineOperationRecord {
                binding: submission_test_binding(&request),
                transaction_hash,
            });
        let kura = Kura::blank_kura_for_testing();
        assert_eq!(
            kura.get_earliest_block_height_by_offline_operation_id(
                &issuer.authority,
                operation_id,
            ),
            Some(None),
            "the canonical operation index deliberately has no matching history"
        );
        let app = app_with_offline_history(kura, issuer);
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
        assert_offline_history_error(error, "offline_operation_index_inconsistent");
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
        assert_eq!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .records
                .len(),
            1
        );
        assert!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .in_flight
                .is_empty()
        );
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
        assert!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .in_flight
                .is_empty()
        );
        assert!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .records
                .is_empty()
        );
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
        assert!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .in_flight
                .is_empty()
        );
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
            let mut admission = issuer.admission.lock().expect("admission lock");
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

        let admission = issuer.admission.lock().expect("admission lock");
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

        issuer
            .admission
            .lock()
            .expect("admission lock")
            .in_flight
            .remove(&operation_id);
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
            let mut admission = issuer.admission.lock().expect("admission lock");
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
            let admission = issuer.admission.lock().expect("admission lock");
            assert!(admission.records.get(&operation_id).is_none());
            let replacement = admission
                .in_flight
                .get(&operation_id)
                .expect("newer reservation survives stale acceptance");
            assert!(Arc::ptr_eq(&replacement.token, &replacement_token));
        }
        issuer
            .admission
            .lock()
            .expect("admission lock")
            .in_flight
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
    fn only_duplicate_queue_outcomes_enter_idempotent_recovery() {
        for source in [
            iroha_core::queue::Error::InBlockchain,
            iroha_core::queue::Error::IsInQueue,
        ] {
            let error = Error::PushIntoQueue {
                source: Box::new(source),
                backpressure: iroha_core::queue::BackpressureState::default(),
            };
            assert!(is_duplicate_queue_admission_error(&error));
        }

        for source in [
            iroha_core::queue::Error::Full,
            iroha_core::queue::Error::LatencySaturated,
            iroha_core::queue::Error::MaximumTransactionsPerUser,
            iroha_core::queue::Error::Expired,
        ] {
            let error = Error::PushIntoQueue {
                source: Box::new(source),
                backpressure: iroha_core::queue::BackpressureState::default(),
            };
            assert!(
                !is_duplicate_queue_admission_error(&error),
                "non-duplicate queue failure must retain its original semantics: {error}"
            );
        }
        assert!(!is_duplicate_queue_admission_error(&operation_id_conflict()));
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
        request.shield_evidence.proof.proof.bytes = vec![0xA5; 2 * 1024 * 1024];
        let original =
            OfflineOperationRequestBinding::from_request(OfflineOperationRequest::TopUp(&request))
                .expect("canonical large request binding");
        request.shield_evidence.proof.proof.bytes[1_048_576] ^= 0xFF;
        let changed =
            OfflineOperationRequestBinding::from_request(OfflineOperationRequest::TopUp(&request))
                .expect("canonical changed request binding");

        assert_eq!(original.operation_id, changed.operation_id);
        assert_ne!(
            original.canonical_request_digest, changed.canonical_request_digest,
            "a changed byte in a multi-MiB request must change the retained binding"
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
        let admission = issuer.admission.lock().expect("admission lock");
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
        assert_eq!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .tracked_entries(),
            1
        );
        drop(replacement);
        assert_eq!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .tracked_entries(),
            0
        );
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
            let admission = issuer.admission.lock().expect("admission lock");
            assert_eq!(admission.tracked_entries(), CAPACITY);
            assert_eq!(admission.in_flight.len(), CAPACITY);
            assert_eq!(
                admission.accounted_bytes(),
                CAPACITY * ADMITTED_OPERATION_ACCOUNTED_BYTES
            );
        }
        drop(leaders);
        assert_eq!(
            issuer
                .admission
                .lock()
                .expect("admission lock")
                .tracked_entries(),
            0
        );
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
            binding: recovered.binding().expect("canonical recovered binding"),
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
    fn admitted_binding_recovery_rejects_digest_or_transaction_mismatch() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x37);
        let transaction = submission_test_transaction(vec![request]);
        let recovered =
            offline_operation_record_in_transaction(&transaction, &issuer.authority, [0x37; 32])
                .expect("recover admitted fixture");
        let matching = AdmittedOfflineOperationRecord {
            binding: recovered.binding().expect("canonical binding"),
            transaction_hash: recovered.transaction_hash,
        };
        ensure_admitted_operation_matches_recovered_record(&matching, &recovered)
            .expect("matching authoritative record");

        let mut wrong_digest = matching.clone();
        wrong_digest.binding.canonical_request_digest[0] ^= 1;
        let mut wrong_hash = matching;
        wrong_hash.transaction_hash = submission_test_hash(0x38);
        for adversarial in [wrong_digest, wrong_hash] {
            let error =
                ensure_admitted_operation_matches_recovered_record(&adversarial, &recovered)
                    .expect_err("mismatched metadata must fail closed");
            assert!(matches!(
                error,
                Error::AppServiceUnavailable {
                    code: "offline_operation_index_inconsistent",
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
                        server_time_ms: 29,
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
    fn terminal_finality_requires_nonzero_exact_identity_hash_height_and_time() {
        let issuer = submission_test_issuer();
        let request = submission_test_request(0x2A);
        let operation_id = request.authorization.operation_id;
        let transaction = submission_test_transaction(vec![request]);
        let record =
            offline_operation_record_in_transaction(&transaction, &issuer.authority, operation_id)
                .expect("fixture operation must be recoverable");
        let matching = kagemusha_v2_applied_finality(
            operation_id,
            record.transaction_hash.to_string(),
            41,
            43,
        );
        ensure_kagemusha_v2_terminal_finality_matches_record(&record, &matching)
            .expect("matching canonical finality");
        let rejected = kagemusha_v2_committed_finality(
            operation_id,
            record.transaction_hash.to_string(),
            41,
            43,
            Some("canonical rejection".to_owned()),
        );
        ensure_kagemusha_v2_terminal_finality_matches_record(&record, &rejected)
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
            (
                "block timestamp",
                KagemushaV2CommittedFinality {
                    server_time_ms: 0,
                    ..matching.clone()
                },
            ),
        ] {
            let error = ensure_kagemusha_v2_terminal_finality_matches_record(&record, &finality)
                .expect_err("terminal mismatch must fail closed");
            match &error {
                Error::AppServiceUnavailable { code, .. } => {
                    assert_eq!(*code, "offline_operation_index_inconsistent", "{label}");
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
        ensure_unproven_pending_window_is_live(10_000, 10_000)
            .expect("the signed expiry boundary is inclusive");

        let error = ensure_unproven_pending_window_is_live(10_001, 10_000)
            .expect_err("an expired operation without provenance must not remain pending forever");
        assert!(matches!(
            &error,
            Error::AppServiceUnavailable {
                code: "offline_operation_index_inconsistent",
                ..
            }
        ));
        assert_eq!(
            error.into_response().status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn kagemusha_v2_anchor_finality_binding_rejects_identity_hash_or_height_mismatch() {
        let operation_id = [0x31; 32];
        let transaction_hash = submission_test_hash(0x73);
        let anchor_transaction_hash = *transaction_hash.as_ref();
        ensure_kagemusha_v2_anchor_finality_binding(
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
            let result = ensure_kagemusha_v2_anchor_finality_binding(
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
                    assert_eq!(*code, "offline_operation_index_inconsistent");
                }
                other => panic!("anchor mismatch returned the wrong error class: {other:?}"),
            }
            assert_eq!(
                error.into_response().status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
        }

        let zero_transaction_hash = submission_test_hash(0);
        let error = ensure_kagemusha_v2_anchor_finality_binding(
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
                code: "offline_operation_index_inconsistent",
                ..
            }
        ));
    }

    #[test]
    fn authorization_refresh_cannot_alias_a_second_anchor_or_bypass_exact_replay() {
        let original = submission_test_request(0x33);
        let mut refreshed = original.clone();
        refreshed.authorization.issued_at_ms = 2;
        refreshed.authorization.expires_at_ms = u64::MAX - 1;

        assert_eq!(
            kagemusha_v2_anchor_state_key(original.authorization.operation_id)
                .expect("original anchor key"),
            kagemusha_v2_anchor_state_key(refreshed.authorization.operation_id)
                .expect("refreshed anchor key"),
            "one operation id has exactly one direct canonical anchor key"
        );
        assert!(
            kagemusha_v2_anchor_state_key([0; 32]).is_err(),
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
    fn kagemusha_v2_rejection_detail_formats_borrowed_reason() {
        assert_eq!(kagemusha_v2_rejection_detail(None), "no rejection reason");

        let rejection = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "fixture rejection".to_owned(),
        ));
        assert_eq!(
            kagemusha_v2_rejection_detail(Some(&rejection)),
            rejection.to_string()
        );

        let adversarial = TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
            "attacker-controlled\nmessage".to_owned(),
        ));
        let message = kagemusha_v2_rejection_detail(Some(&adversarial));
        assert_eq!(message, adversarial.to_string());
        assert_eq!(message, "Validation failed");
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

        let finality = kagemusha_v2_committed_finality(
            [0x2D; 32],
            submission_test_hash(0x2E).to_string(),
            47,
            53,
            Some("attacker-controlled\nterminal rejection".to_owned()),
        );
        let KagemushaV2TerminalOutcome::Rejected(message) = finality.outcome else {
            panic!("a rejected finality fixture must remain rejected")
        };
        assert_eq!(message, FALLBACK);
        assert!(crate::utils::is_valid_error_message(&message));
    }
}
