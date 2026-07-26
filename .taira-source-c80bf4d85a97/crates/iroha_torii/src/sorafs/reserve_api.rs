//! Chain-authoritative SoraFS reserve/rent HTTP boundary.
//!
//! V1 deliberately has no process-local reserve model. Mutation routes accept
//! one exact caller-signed native instruction and forward that transaction to
//! strict durable ingress. Read routes execute typed queries against one
//! immutable finalized view and expose the resulting cursor with every page or
//! singular record.

#![cfg(feature = "app_api")]

use std::{collections::VecDeque, convert::Infallible, future::Future, time::Duration};

use axum::{
    extract::{
        Extension, Path, State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, Sse},
    },
};
use futures::{SinkExt, StreamExt, stream};
use iroha_core::{smartcontracts::ValidSingularQuery, state::StateReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::{
        Instruction,
        sorafs::{
            DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
            RepaySorafsReserveCredit, RequestSorafsReserveMovement, SubmitSorafsReserveAppeal,
        },
    },
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsReserveAppealById, FindSorafsReserveAppeals, FindSorafsReserveEvents,
            FindSorafsReserveMovementById, FindSorafsReserveMovements, FindSorafsReservePolicy,
            FindSorafsReserveProviderById, FindSorafsReserveProviders,
        },
    },
    sorafs::{
        capacity::ProviderId,
        reserve::{
            RESERVE_QUERY_MAX_ITEMS_V1, ReserveAuthorityPolicyRecordV1, ReserveFinalizedCursorV1,
            ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1, ReserveFinalizedEventV1,
            ReserveMovementKindV1, ReserveMovementStatusV1, ReserveProviderAccountV1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use iroha_logger::{debug, warn};
use norito::json;
use sorafs_node::reserve_transaction_forwarder::RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1;

use crate::{
    JsonBody, SharedAppState,
    routing::MaybeTelemetry,
    utils::extractors::{ExtractAccept, JsonOrNoritoVersioned},
};

/// Exact TTL used by every caller- and worker-signed reserve V1 transaction.
const RESERVE_TRANSACTION_TTL_V1: Duration = Duration::from_secs(300);
const RESERVE_DEFAULT_PAGE_LIMIT_V1: u32 = 100;
const RESERVE_EVENT_POLL_INTERVAL_V1: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReserveCommandRouteV1 {
    RequestMovement(ReserveMovementKindV1),
    DecideMovement([u8; 32]),
    DrawCredit,
    RepayCredit,
    SubmitAppeal,
    DecideAppeal([u8; 32]),
}

async fn observe_reserve_api_response<F>(
    telemetry: MaybeTelemetry,
    route: &'static str,
    response: F,
) -> Response
where
    F: Future<Output = Response>,
{
    let response = response.await;
    let result = match response.status() {
        StatusCode::ACCEPTED => "accepted",
        status if status.is_success() || status == StatusCode::SWITCHING_PROTOCOLS => "ok",
        StatusCode::BAD_REQUEST => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "too_many_requests",
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => "unavailable",
        _ => "error",
    };
    telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_reserve_service_request(route, result);
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            metrics.inc_sorafs_reserve_service_rate_limit(route, "ingress");
        }
    });
    response
}

#[derive(Debug, Default)]
struct ReservePageQueryV1 {
    expected_finalized_height: Option<u64>,
    expected_finalized_block_hash: Option<[u8; 32]>,
    after_id: Option<[u8; 32]>,
    limit: Option<u32>,
}

#[derive(Debug, Default)]
struct ReserveAnchorQueryV1 {
    expected_finalized_height: Option<u64>,
    expected_finalized_block_hash: Option<[u8; 32]>,
}

#[derive(Debug, Default)]
struct ReserveEventQueryV1 {
    expected_finalized_height: Option<u64>,
    expected_finalized_block_hash: Option<[u8; 32]>,
    after_sequence: Option<u64>,
    after_block_height: Option<u64>,
    after_block_hash: Option<[u8; 32]>,
    after_event_index: Option<u32>,
    limit: Option<u32>,
}

impl ReserveAnchorQueryV1 {
    fn parse(raw: Option<&str>) -> Result<Self, Response> {
        let mut query = Self::default();
        walk_query(raw, |key, value| match key {
            "expected_finalized_height" => {
                parse_unique_u64(&mut query.expected_finalized_height, key, value)
            }
            "expected_finalized_block_hash_hex" => {
                parse_unique_hex(&mut query.expected_finalized_block_hash, key, value, true)
            }
            _ => Err(format!(
                "unknown finalized SoraFS reserve anchor parameter `{key}`"
            )),
        })?;
        validate_finalized_cursor_pair(
            query.expected_finalized_height,
            query.expected_finalized_block_hash,
        )?;
        Ok(query)
    }

    fn expected_finalized_cursor(&self) -> Option<ReserveFinalizedCursorV1> {
        self.expected_finalized_height
            .zip(self.expected_finalized_block_hash)
            .map(|(height, block_hash)| ReserveFinalizedCursorV1 { height, block_hash })
    }
}

impl ReservePageQueryV1 {
    fn parse(raw: Option<&str>, after_parameter: &str) -> Result<Self, Response> {
        let mut query = Self::default();
        walk_query(raw, |key, value| match key {
            "limit" => parse_unique_u32(&mut query.limit, key, value),
            "expected_finalized_height" => {
                parse_unique_u64(&mut query.expected_finalized_height, key, value)
            }
            "expected_finalized_block_hash_hex" => {
                parse_unique_hex(&mut query.expected_finalized_block_hash, key, value, true)
            }
            key if key == after_parameter => {
                parse_unique_hex(&mut query.after_id, key, value, true)
            }
            _ => Err(format!(
                "unknown finalized SoraFS reserve query parameter `{key}`"
            )),
        })?;
        validate_finalized_cursor_pair(
            query.expected_finalized_height,
            query.expected_finalized_block_hash,
        )?;
        if query
            .limit
            .is_some_and(|limit| !(1..=RESERVE_QUERY_MAX_ITEMS_V1).contains(&limit))
        {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "SoraFS reserve query limit must be within 1..={RESERVE_QUERY_MAX_ITEMS_V1}"
                ),
            ));
        }
        Ok(query)
    }

    fn expected_finalized_cursor(&self) -> Option<ReserveFinalizedCursorV1> {
        self.expected_finalized_height
            .zip(self.expected_finalized_block_hash)
            .map(|(height, block_hash)| ReserveFinalizedCursorV1 { height, block_hash })
    }

    fn limit(&self) -> u32 {
        self.limit
            .unwrap_or(RESERVE_DEFAULT_PAGE_LIMIT_V1)
            .clamp(1, RESERVE_QUERY_MAX_ITEMS_V1)
    }
}

impl ReserveEventQueryV1 {
    fn parse(raw: Option<&str>) -> Result<Self, Response> {
        let mut query = Self::default();
        walk_query(raw, |key, value| match key {
            "limit" => parse_unique_u32(&mut query.limit, key, value),
            "expected_finalized_height" => {
                parse_unique_u64(&mut query.expected_finalized_height, key, value)
            }
            "expected_finalized_block_hash_hex" => {
                parse_unique_hex(&mut query.expected_finalized_block_hash, key, value, true)
            }
            "after_sequence" => parse_unique_u64(&mut query.after_sequence, key, value),
            "after_block_height" => parse_unique_u64(&mut query.after_block_height, key, value),
            "after_block_hash_hex" => {
                parse_unique_hex(&mut query.after_block_hash, key, value, true)
            }
            "after_event_index" => parse_unique_u32(&mut query.after_event_index, key, value),
            _ => Err(format!(
                "unknown finalized SoraFS reserve event query parameter `{key}`"
            )),
        })?;
        validate_finalized_cursor_pair(
            query.expected_finalized_height,
            query.expected_finalized_block_hash,
        )?;
        if query
            .limit
            .is_some_and(|limit| !(1..=RESERVE_QUERY_MAX_ITEMS_V1).contains(&limit))
        {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "SoraFS reserve event query limit must be within 1..={RESERVE_QUERY_MAX_ITEMS_V1}"
                ),
            ));
        }
        let cursor_parts = [
            query.after_sequence.is_some(),
            query.after_block_height.is_some(),
            query.after_block_hash.is_some(),
            query.after_event_index.is_some(),
        ];
        if cursor_parts.iter().any(|present| *present)
            && !cursor_parts.iter().all(|present| *present)
        {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "complete finalized SoraFS reserve event cursor is required",
            ));
        }
        if query.after_sequence == Some(0) || query.after_block_height == Some(0) {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "SoraFS reserve event cursor sequence and block height must be non-zero",
            ));
        }
        Ok(query)
    }

    fn expected_finalized_cursor(&self) -> Option<ReserveFinalizedCursorV1> {
        self.expected_finalized_height
            .zip(self.expected_finalized_block_hash)
            .map(|(height, block_hash)| ReserveFinalizedCursorV1 { height, block_hash })
    }

    fn after(&self) -> Option<ReserveFinalizedEventCursorV1> {
        Some(ReserveFinalizedEventCursorV1 {
            sequence: self.after_sequence?,
            block_height: self.after_block_height?,
            block_hash: self.after_block_hash?,
            event_index: self.after_event_index?,
        })
    }

    fn limit(&self) -> u32 {
        self.limit
            .unwrap_or(RESERVE_DEFAULT_PAGE_LIMIT_V1)
            .clamp(1, RESERVE_QUERY_MAX_ITEMS_V1)
    }
}

pub(crate) async fn handle_post_sorafs_reserve_top_up(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "top_up", async move {
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp),
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_withdrawal(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "withdrawal", async move {
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::Withdrawal),
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_movement_decision(
    State(state): State<SharedAppState>,
    Path(movement_id_hex): Path<String>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "movement_decision", async move {
        let movement_id = match parse_nonzero_hex(&movement_id_hex, "movement_id_hex") {
            Ok(movement_id) => movement_id,
            Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
        };
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::DecideMovement(movement_id),
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_credit_draw(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "credit_draw", async move {
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::DrawCredit,
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_credit_repay(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "credit_repay", async move {
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::RepayCredit,
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_appeal(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "appeal", async move {
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::SubmitAppeal,
        )
        .await
    })
    .await
}

pub(crate) async fn handle_post_sorafs_reserve_appeal_decision(
    State(state): State<SharedAppState>,
    Path(appeal_id_hex): Path<String>,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    JsonOrNoritoVersioned(transaction): JsonOrNoritoVersioned<SignedTransaction>,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "appeal_decision", async move {
        let appeal_id = match parse_nonzero_hex(&appeal_id_hex, "appeal_id_hex") {
            Ok(appeal_id) => appeal_id,
            Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
        };
        submit_reserve_signed_transaction(
            state,
            headers,
            accept,
            transaction,
            ReserveCommandRouteV1::DecideAppeal(appeal_id),
        )
        .await
    })
    .await
}

async fn submit_reserve_signed_transaction(
    state: SharedAppState,
    headers: HeaderMap,
    accept: Option<ExtractAccept>,
    transaction: SignedTransaction,
    route: ReserveCommandRouteV1,
) -> Response {
    if !state.sorafs_node.is_enabled() {
        return feature_disabled();
    }
    if let Err(response) = validate_reserve_signed_transaction(&state, &transaction, route) {
        return response;
    }
    match crate::submit_signed_transaction_for_ingress_strict_durable(
        state,
        headers,
        accept,
        transaction,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

fn validate_reserve_signed_transaction(
    state: &SharedAppState,
    transaction: &SignedTransaction,
    route: ReserveCommandRouteV1,
) -> Result<(), Response> {
    validate_reserve_signed_envelope_and_route(state.chain_id.as_ref(), transaction, route)?;
    let view = state.state.query_view();
    let policy = FindSorafsReservePolicy::new()
        .execute(&view)
        .map_err(reserve_query_error_response)?;
    let authority = transaction.authority();

    match route {
        ReserveCommandRouteV1::RequestMovement(expected_kind) => {
            let request = one_instruction::<RequestSorafsReserveMovement>(transaction)?;
            if request.kind != expected_kind {
                return Err(route_mismatch(route));
            }
            let account = FindSorafsReserveProviderById::new(request.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_provider_authority(authority, &account)?;
            require_current_provider_binding(
                request.expected_provider_revision,
                request.policy_digest,
                &account,
                &policy,
            )
        }
        ReserveCommandRouteV1::DecideMovement(path_movement_id) => {
            let decision = one_instruction::<DecideSorafsReserveMovement>(transaction)?;
            if decision.movement_id != path_movement_id {
                return Err(json_error(
                    StatusCode::BAD_REQUEST,
                    "SoraFS reserve movement decision does not match the route identifier",
                ));
            }
            require_subject(authority, &policy.policy.decision_authority, "decision")?;
            let movement = FindSorafsReserveMovementById::new(decision.movement_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            if movement.status != ReserveMovementStatusV1::Pending {
                return Err(json_error(
                    StatusCode::CONFLICT,
                    "SoraFS reserve movement is already terminal",
                ));
            }
            let account = FindSorafsReserveProviderById::new(movement.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_current_provider_binding(
                decision.expected_provider_revision,
                decision.policy_digest,
                &account,
                &policy,
            )
        }
        ReserveCommandRouteV1::DrawCredit => {
            let draw = one_instruction::<DrawSorafsReserveCredit>(transaction)?;
            require_subject(authority, &policy.policy.operations_authority, "operations")?;
            let account = FindSorafsReserveProviderById::new(draw.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_current_provider_binding(
                draw.expected_provider_revision,
                draw.policy_digest,
                &account,
                &policy,
            )
        }
        ReserveCommandRouteV1::RepayCredit => {
            let repayment = one_instruction::<RepaySorafsReserveCredit>(transaction)?;
            let account = FindSorafsReserveProviderById::new(repayment.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_provider_authority(authority, &account)?;
            require_current_provider_binding(
                repayment.expected_provider_revision,
                repayment.policy_digest,
                &account,
                &policy,
            )
        }
        ReserveCommandRouteV1::SubmitAppeal => {
            let appeal = one_instruction::<SubmitSorafsReserveAppeal>(transaction)?;
            let account = FindSorafsReserveProviderById::new(appeal.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_provider_authority(authority, &account)?;
            require_current_provider_binding(
                appeal.expected_provider_revision,
                appeal.policy_digest,
                &account,
                &policy,
            )
        }
        ReserveCommandRouteV1::DecideAppeal(path_appeal_id) => {
            let decision = one_instruction::<DecideSorafsReserveAppeal>(transaction)?;
            if decision.appeal_id != path_appeal_id {
                return Err(json_error(
                    StatusCode::BAD_REQUEST,
                    "SoraFS reserve appeal decision does not match the route identifier",
                ));
            }
            require_subject(authority, &policy.policy.decision_authority, "decision")?;
            let appeal = FindSorafsReserveAppealById::new(decision.appeal_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            if appeal.status != iroha_data_model::sorafs::reserve::ReserveAppealStatusV1::Pending {
                return Err(json_error(
                    StatusCode::CONFLICT,
                    "SoraFS reserve appeal is already terminal",
                ));
            }
            let account = FindSorafsReserveProviderById::new(appeal.provider_id)
                .execute(&view)
                .map_err(reserve_query_error_response)?;
            require_current_provider_binding(
                decision.expected_provider_revision,
                decision.policy_digest,
                &account,
                &policy,
            )
        }
    }
}

fn validate_reserve_signed_envelope_and_route(
    expected_chain: &iroha_data_model::ChainId,
    transaction: &SignedTransaction,
    route: ReserveCommandRouteV1,
) -> Result<(), Response> {
    if transaction.chain() != expected_chain {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "SoraFS reserve transaction chain does not match this peer",
        ));
    }
    transaction.verify_signature().map_err(|_| {
        json_error(
            StatusCode::FORBIDDEN,
            "SoraFS reserve transaction signature or authority binding is invalid",
        )
    })?;
    if transaction.creation_time().is_zero()
        || transaction.time_to_live() != Some(RESERVE_TRANSACTION_TTL_V1)
        || transaction.nonce().is_some()
        || !transaction.metadata().is_empty()
        || transaction.fee_payment_intent().validate().is_err()
        || transaction.attachments().is_some()
    {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "SoraFS reserve transaction violates the exact V1 signed-envelope policy",
        ));
    }
    let canonical = norito::to_bytes(transaction).map_err(|_| {
        json_error(
            StatusCode::BAD_REQUEST,
            "failed to encode canonical SoraFS reserve transaction",
        )
    })?;
    if canonical.len() > RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return Err(json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "SoraFS reserve transaction exceeds the canonical V1 byte bound",
        ));
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(route_mismatch(route));
    };
    if instructions.len() != 1 {
        return Err(route_mismatch(route));
    }
    let matches_route = match route {
        ReserveCommandRouteV1::RequestMovement(expected_kind) => instructions[0]
            .as_any()
            .downcast_ref::<RequestSorafsReserveMovement>()
            .is_some_and(|request| request.kind == expected_kind),
        ReserveCommandRouteV1::DecideMovement(path_id) => instructions[0]
            .as_any()
            .downcast_ref::<DecideSorafsReserveMovement>()
            .is_some_and(|decision| decision.movement_id == path_id),
        ReserveCommandRouteV1::DrawCredit => instructions[0]
            .as_any()
            .downcast_ref::<DrawSorafsReserveCredit>()
            .is_some(),
        ReserveCommandRouteV1::RepayCredit => instructions[0]
            .as_any()
            .downcast_ref::<RepaySorafsReserveCredit>()
            .is_some(),
        ReserveCommandRouteV1::SubmitAppeal => instructions[0]
            .as_any()
            .downcast_ref::<SubmitSorafsReserveAppeal>()
            .is_some(),
        ReserveCommandRouteV1::DecideAppeal(path_id) => instructions[0]
            .as_any()
            .downcast_ref::<DecideSorafsReserveAppeal>()
            .is_some_and(|decision| decision.appeal_id == path_id),
    };
    if matches_route {
        Ok(())
    } else {
        Err(route_mismatch(route))
    }
}

fn one_instruction<T: 'static>(transaction: &SignedTransaction) -> Result<&T, Response> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "SoraFS reserve transaction must contain one native instruction",
        ));
    };
    instructions
        .first()
        .and_then(|instruction| instruction.as_any().downcast_ref::<T>())
        .ok_or_else(|| {
            json_error(
                StatusCode::BAD_REQUEST,
                "SoraFS reserve transaction instruction type changed after validation",
            )
        })
}

fn require_current_provider_binding(
    expected_revision: u64,
    policy_digest: [u8; 32],
    account: &ReserveProviderAccountV1,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), Response> {
    if expected_revision != account.revision
        || policy_digest != policy.policy_digest
        || account.policy_digest != policy.policy_digest
    {
        return Err(json_error(
            StatusCode::CONFLICT,
            "SoraFS reserve transaction is stale against finalized provider or policy state",
        ));
    }
    Ok(())
}

fn require_provider_authority(
    authority: &AccountId,
    account: &ReserveProviderAccountV1,
) -> Result<(), Response> {
    require_subject(authority, &account.terms.provider_account, "provider")
}

fn require_subject(
    actual: &AccountId,
    expected: &AccountId,
    authority_kind: &str,
) -> Result<(), Response> {
    if actual.subject_id() == expected.subject_id() {
        Ok(())
    } else {
        Err(json_error(
            StatusCode::FORBIDDEN,
            format!("SoraFS reserve transaction authority is not the governed {authority_kind}"),
        ))
    }
}

fn route_mismatch(route: ReserveCommandRouteV1) -> Response {
    let expected = match route {
        ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp) => {
            "RequestSorafsReserveMovement::TopUp"
        }
        ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::Withdrawal) => {
            "RequestSorafsReserveMovement::Withdrawal"
        }
        ReserveCommandRouteV1::DecideMovement(_) => "DecideSorafsReserveMovement",
        ReserveCommandRouteV1::DrawCredit => "DrawSorafsReserveCredit",
        ReserveCommandRouteV1::RepayCredit => "RepaySorafsReserveCredit",
        ReserveCommandRouteV1::SubmitAppeal => "SubmitSorafsReserveAppeal",
        ReserveCommandRouteV1::DecideAppeal(_) => "DecideSorafsReserveAppeal",
    };
    json_error(
        StatusCode::BAD_REQUEST,
        format!("SoraFS reserve route requires exactly one `{expected}` native instruction"),
    )
}

pub(crate) async fn handle_get_sorafs_reserve_policy(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "policy", async move {
        let account = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReserveAnchorQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let cursor = match require_expected_cursor(&view, query.expected_finalized_cursor()) {
            Ok(cursor) => cursor,
            Err(response) => return response,
        };
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&account, &policy) {
            return response;
        }
        anchored_record_response(cursor, "policy", &policy)
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_providers(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "providers", async move {
        let account = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReservePageQueryV1::parse(raw_query.as_deref(), "after_provider_id_hex") {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&account, &policy) {
            return response;
        }
        match FindSorafsReserveProviders::new(
            query.expected_finalized_cursor(),
            query.after_id.map(ProviderId::new),
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => crate::JsonBody(page).into_response(),
            Err(error) => reserve_query_error_response(error),
        }
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_provider(
    State(state): State<SharedAppState>,
    Path(provider_id_hex): Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "provider", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let provider_id = match parse_nonzero_hex(&provider_id_hex, "provider_id_hex") {
            Ok(provider_id) => ProviderId::new(provider_id),
            Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
        };
        let query = match ReserveAnchorQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let cursor = match require_expected_cursor(&view, query.expected_finalized_cursor()) {
            Ok(cursor) => cursor,
            Err(response) => return response,
        };
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        let provider = match FindSorafsReserveProviderById::new(provider_id).execute(&view) {
            Ok(provider) => provider,
            Err(error) => return reserve_query_error_response(error),
        };
        if caller.subject_id() != provider.terms.provider_account.subject_id()
            && require_reserve_operator(&caller, &policy).is_err()
        {
            return json_error(
                StatusCode::FORBIDDEN,
                "SoraFS reserve provider state is visible only to its provider or governed services",
            );
        }
        anchored_record_response(cursor, "provider", &provider)
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_movements(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "movements", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReservePageQueryV1::parse(raw_query.as_deref(), "after_movement_id_hex") {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&caller, &policy) {
            return response;
        }
        match FindSorafsReserveMovements::new(
            query.expected_finalized_cursor(),
            query.after_id,
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => crate::JsonBody(page).into_response(),
            Err(error) => reserve_query_error_response(error),
        }
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_movement(
    State(state): State<SharedAppState>,
    Path(movement_id_hex): Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "movement", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let movement_id = match parse_nonzero_hex(&movement_id_hex, "movement_id_hex") {
            Ok(movement_id) => movement_id,
            Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
        };
        let query = match ReserveAnchorQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let cursor = match require_expected_cursor(&view, query.expected_finalized_cursor()) {
            Ok(cursor) => cursor,
            Err(response) => return response,
        };
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        let movement = match FindSorafsReserveMovementById::new(movement_id).execute(&view) {
            Ok(movement) => movement,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) =
            require_provider_or_operator_for_id(&view, &caller, movement.provider_id, &policy)
        {
            return response;
        }
        anchored_record_response(cursor, "movement", &movement)
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_appeals(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "appeals", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReservePageQueryV1::parse(raw_query.as_deref(), "after_appeal_id_hex") {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&caller, &policy) {
            return response;
        }
        match FindSorafsReserveAppeals::new(
            query.expected_finalized_cursor(),
            query.after_id,
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => crate::JsonBody(page).into_response(),
            Err(error) => reserve_query_error_response(error),
        }
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_appeal(
    State(state): State<SharedAppState>,
    Path(appeal_id_hex): Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "appeal_detail", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let appeal_id = match parse_nonzero_hex(&appeal_id_hex, "appeal_id_hex") {
            Ok(appeal_id) => appeal_id,
            Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
        };
        let query = match ReserveAnchorQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let cursor = match require_expected_cursor(&view, query.expected_finalized_cursor()) {
            Ok(cursor) => cursor,
            Err(response) => return response,
        };
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        let appeal = match FindSorafsReserveAppealById::new(appeal_id).execute(&view) {
            Ok(appeal) => appeal,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) =
            require_provider_or_operator_for_id(&view, &caller, appeal.provider_id, &policy)
        {
            return response;
        }
        anchored_record_response(cursor, "appeal", &appeal)
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_events(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "events", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReserveEventQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&caller, &policy) {
            return response;
        }
        match FindSorafsReserveEvents::new(
            query.expected_finalized_cursor(),
            query.after(),
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => crate::JsonBody(page).into_response(),
            Err(error) => reserve_query_error_response(error),
        }
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_events_stream(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "events_stream", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReserveEventQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&caller, &policy) {
            return response;
        }
        let initial = match FindSorafsReserveEvents::new(
            query.expected_finalized_cursor(),
            query.after(),
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => page,
            Err(error) => return reserve_query_error_response(error),
        };
        drop(view);
        Sse::new(reserve_event_stream(
            state,
            initial,
            query.after(),
            query.limit(),
        ))
        .into_response()
    })
    .await
}

pub(crate) async fn handle_get_sorafs_reserve_events_ws(
    State(state): State<SharedAppState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    preauth_guard: Option<Extension<crate::PreAuthGuardHandoff>>,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    ws: WebSocketUpgrade,
) -> Response {
    let telemetry = state.telemetry.clone();
    observe_reserve_api_response(telemetry, "events_ws", async move {
        let caller = match authenticate_reserve_read(&state, &headers, &method, &uri) {
            Ok(account) => account,
            Err(response) => return response,
        };
        let query = match ReserveEventQueryV1::parse(raw_query.as_deref()) {
            Ok(query) => query,
            Err(response) => return response,
        };
        let view = state.state.query_view();
        let policy = match FindSorafsReservePolicy::new().execute(&view) {
            Ok(policy) => policy,
            Err(error) => return reserve_query_error_response(error),
        };
        if let Err(response) = require_reserve_operator(&caller, &policy) {
            return response;
        }
        let initial = match FindSorafsReserveEvents::new(
            query.expected_finalized_cursor(),
            query.after(),
            query.limit(),
        )
        .execute(&view)
        {
            Ok(page) => page,
            Err(error) => return reserve_query_error_response(error),
        };
        drop(view);
        let preauth_guard = crate::take_preauth_upgrade_guard(preauth_guard);
        ws.on_upgrade(move |socket| async move {
            let _preauth_guard = preauth_guard;
            if let Err(error) =
                reserve_event_websocket(socket, state, initial, query.after(), query.limit()).await
            {
                debug!(%error, "SoraFS reserve finalized-event WebSocket closed");
            }
        })
        .into_response()
    })
    .await
}

fn query_reserve_events(
    state: &SharedAppState,
    expected: Option<ReserveFinalizedCursorV1>,
    after: Option<ReserveFinalizedEventCursorV1>,
    limit: u32,
) -> Result<ReserveFinalizedEventPageV1, QueryExecutionFail> {
    let view = state.state.query_view();
    FindSorafsReserveEvents::new(expected, after, limit).execute(&view)
}

fn reserve_event_stream(
    state: SharedAppState,
    initial: ReserveFinalizedEventPageV1,
    initial_after: Option<ReserveFinalizedEventCursorV1>,
    limit: u32,
) -> impl futures::Stream<Item = Result<SseEvent, Infallible>> {
    struct PollState {
        state: SharedAppState,
        pending: VecDeque<ReserveFinalizedEventV1>,
        after: Option<ReserveFinalizedEventCursorV1>,
        limit: u32,
        terminal: bool,
    }

    let after = reserve_event_resume_cursor(&initial.events, initial_after);
    stream::unfold(
        PollState {
            state,
            pending: initial.events.into_iter().collect(),
            after,
            limit,
            terminal: false,
        },
        |mut poll| async move {
            loop {
                if let Some(event) = poll.pending.pop_front() {
                    let frame = reserve_sse_event(&event);
                    return Some((Ok(frame), poll));
                }
                if poll.terminal {
                    return None;
                }
                tokio::time::sleep(RESERVE_EVENT_POLL_INTERVAL_V1).await;
                match query_reserve_events(&poll.state, None, poll.after, poll.limit) {
                    Ok(page) => {
                        if let Some(after) = page.events.last().map(|event| event.cursor()) {
                            poll.after = Some(after);
                        }
                        poll.pending.extend(page.events);
                    }
                    Err(error) => {
                        poll.terminal = true;
                        return Some((
                            Ok(SseEvent::default()
                                .event("error")
                                .data(reserve_query_public_message(&error))),
                            poll,
                        ));
                    }
                }
            }
        },
    )
}

async fn reserve_event_websocket(
    socket: WebSocket,
    state: SharedAppState,
    initial: ReserveFinalizedEventPageV1,
    initial_after: Option<ReserveFinalizedEventCursorV1>,
    limit: u32,
) -> Result<(), String> {
    let (mut sender, mut receiver) = socket.split();
    let mut after = reserve_event_resume_cursor(&initial.events, initial_after);
    for event in initial.events {
        sender
            .send(WsMessage::Text(reserve_event_json(&event)?.into()))
            .await
            .map_err(|error| error.to_string())?;
    }

    loop {
        tokio::select! {
            inbound = receiver.next() => {
                match inbound {
                    Some(Ok(WsMessage::Close(_))) | None => return Ok(()),
                    Some(Err(error)) => return Err(error.to_string()),
                    Some(Ok(_)) => {}
                }
            }
            () = tokio::time::sleep(RESERVE_EVENT_POLL_INTERVAL_V1) => {
                let page = query_reserve_events(&state, None, after, limit)
                    .map_err(|error| reserve_query_public_message(&error))?;
                if let Some(next) = page.events.last().map(|event| event.cursor()) {
                    after = Some(next);
                }
                for event in page.events {
                    sender
                        .send(WsMessage::Text(reserve_event_json(&event)?.into()))
                        .await
                        .map_err(|error| error.to_string())?;
                }
            }
        }
    }
}

fn reserve_sse_event(event: &ReserveFinalizedEventV1) -> SseEvent {
    match reserve_event_json(event) {
        Ok(json) => SseEvent::default()
            .event("reserve_finalized")
            .id(event.sequence.to_string())
            .data(json),
        Err(error) => SseEvent::default().event("error").data(error),
    }
}

fn reserve_event_json(event: &ReserveFinalizedEventV1) -> Result<String, String> {
    json::to_json(event)
        .map(|json| json.to_string())
        .map_err(|error| format!("failed to encode finalized SoraFS reserve event: {error}"))
}

fn reserve_event_resume_cursor(
    events: &[ReserveFinalizedEventV1],
    initial_after: Option<ReserveFinalizedEventCursorV1>,
) -> Option<ReserveFinalizedEventCursorV1> {
    events
        .last()
        .map(ReserveFinalizedEventV1::cursor)
        .or(initial_after)
}

fn authenticate_reserve_read(
    state: &SharedAppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
) -> Result<AccountId, Response> {
    if !state.sorafs_node.is_enabled() {
        return Err(feature_disabled());
    }
    match crate::app_auth::verify_canonical_request(&state.state, headers, method, uri, &[], None) {
        Ok(Some(verified)) => Ok(verified.account),
        Ok(None) => Err(json_error(
            StatusCode::UNAUTHORIZED,
            "SoraFS reserve reads require X-Iroha canonical request authentication",
        )),
        Err(error) => {
            warn!(
                ?error,
                "SoraFS reserve canonical read authentication rejected"
            );
            Err(json_error(
                StatusCode::UNAUTHORIZED,
                "invalid SoraFS reserve read authentication",
            ))
        }
    }
}

fn require_reserve_operator(
    caller: &AccountId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), Response> {
    if caller.subject_id() == policy.policy.operations_authority.subject_id()
        || caller.subject_id() == policy.policy.decision_authority.subject_id()
    {
        Ok(())
    } else {
        Err(json_error(
            StatusCode::FORBIDDEN,
            "SoraFS reserve collection reads require a governed reserve service authority",
        ))
    }
}

fn require_provider_or_operator_for_id(
    view: &impl StateReadOnly,
    caller: &AccountId,
    provider_id: ProviderId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<(), Response> {
    if require_reserve_operator(caller, policy).is_ok() {
        return Ok(());
    }
    let provider = FindSorafsReserveProviderById::new(provider_id)
        .execute(view)
        .map_err(reserve_query_error_response)?;
    if caller.subject_id() == provider.terms.provider_account.subject_id() {
        Ok(())
    } else {
        Err(json_error(
            StatusCode::FORBIDDEN,
            "SoraFS reserve record is visible only to its provider or governed services",
        ))
    }
}

fn require_expected_cursor(
    view: &impl StateReadOnly,
    expected: Option<ReserveFinalizedCursorV1>,
) -> Result<ReserveFinalizedCursorV1, Response> {
    let actual = reserve_finalized_cursor(view).ok_or_else(|| {
        json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "authoritative finalized SoraFS reserve state is unavailable",
        )
    })?;
    if expected.is_some_and(|expected| expected != actual) {
        return Err(json_error(
            StatusCode::CONFLICT,
            "SoraFS reserve finalized cursor is stale; restart from the latest committed anchor",
        ));
    }
    Ok(actual)
}

fn reserve_finalized_cursor(view: &impl StateReadOnly) -> Option<ReserveFinalizedCursorV1> {
    u64::try_from(view.block_hashes().len())
        .ok()
        .zip(view.block_hashes().last())
        .map(|(height, hash)| ReserveFinalizedCursorV1 {
            height,
            block_hash: *hash.as_ref(),
        })
        .filter(|cursor| cursor.height != 0 && cursor.block_hash != [0; 32])
}

fn anchored_record_response<T: norito::json::JsonSerialize>(
    cursor: ReserveFinalizedCursorV1,
    field: &str,
    record: &T,
) -> Response {
    let cursor = match json::to_value(&cursor) {
        Ok(cursor) => cursor,
        Err(error) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to encode finalized SoraFS reserve cursor: {error}"),
            );
        }
    };
    let record = match json::to_value(record) {
        Ok(record) => record,
        Err(error) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to encode finalized SoraFS reserve record: {error}"),
            );
        }
    };
    JsonBody(crate::json_object(vec![
        crate::json_entry("schema", "sorafs.reserve.finalized_record.v1"),
        crate::json_entry("finalized_cursor", cursor),
        crate::json_entry(field, record),
    ]))
    .into_response()
}

fn reserve_query_error_response(error: QueryExecutionFail) -> Response {
    let status = match error {
        QueryExecutionFail::Find(
            FindError::SorafsReservePolicy
            | FindError::SorafsReserveProvider(_)
            | FindError::SorafsReserveMovement(_)
            | FindError::SorafsReserveAppeal(_),
        ) => StatusCode::NOT_FOUND,
        QueryExecutionFail::Expired | QueryExecutionFail::CursorMismatch => StatusCode::CONFLICT,
        QueryExecutionFail::Conversion(_)
        | QueryExecutionFail::FetchSizeTooBig
        | QueryExecutionFail::InvalidSingularParameters => StatusCode::BAD_REQUEST,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    if status.is_server_error() {
        warn!(
            ?error,
            "authoritative finalized SoraFS reserve query failed"
        );
    }
    json_error(status, reserve_query_public_message(&error))
}

fn reserve_query_public_message(error: &QueryExecutionFail) -> String {
    match error {
        QueryExecutionFail::Find(
            FindError::SorafsReservePolicy
            | FindError::SorafsReserveProvider(_)
            | FindError::SorafsReserveMovement(_)
            | FindError::SorafsReserveAppeal(_),
        ) => "authoritative SoraFS reserve record was not found".to_owned(),
        QueryExecutionFail::Expired | QueryExecutionFail::CursorMismatch => {
            "SoraFS reserve finalized cursor is stale; restart from the latest committed anchor"
                .to_owned()
        }
        QueryExecutionFail::Conversion(_)
        | QueryExecutionFail::FetchSizeTooBig
        | QueryExecutionFail::InvalidSingularParameters => {
            "invalid finalized SoraFS reserve query".to_owned()
        }
        _ => "authoritative finalized SoraFS reserve state is unavailable".to_owned(),
    }
}

fn walk_query(
    raw: Option<&str>,
    mut visit: impl FnMut(&str, &str) -> Result<(), String>,
) -> Result<(), Response> {
    let Some(raw) = raw.filter(|raw| !raw.is_empty()) else {
        return Ok(());
    };
    for pair in raw.split('&') {
        if pair.is_empty() {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "empty SoraFS reserve query component",
            ));
        }
        let (raw_key, raw_value) = pair.split_once('=').ok_or_else(|| {
            json_error(
                StatusCode::BAD_REQUEST,
                "SoraFS reserve query component must contain `=`",
            )
        })?;
        let key = urlencoding::decode(raw_key)
            .map_err(|_| json_error(StatusCode::BAD_REQUEST, "invalid reserve query key"))?;
        let value = urlencoding::decode(raw_value)
            .map_err(|_| json_error(StatusCode::BAD_REQUEST, "invalid reserve query value"))?;
        visit(key.as_ref(), value.as_ref())
            .map_err(|error| json_error(StatusCode::BAD_REQUEST, error))?;
    }
    Ok(())
}

fn parse_unique_u64(target: &mut Option<u64>, name: &str, raw: &str) -> Result<(), String> {
    if target.is_some() || raw.is_empty() {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must appear once with a value"
        ));
    }
    let value = raw.parse::<u64>().map_err(|_| {
        format!("SoraFS reserve query parameter `{name}` must be an unsigned integer")
    })?;
    if value.to_string() != raw {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must use canonical decimal encoding"
        ));
    }
    *target = Some(value);
    Ok(())
}

fn parse_unique_u32(target: &mut Option<u32>, name: &str, raw: &str) -> Result<(), String> {
    if target.is_some() || raw.is_empty() {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must appear once with a value"
        ));
    }
    let value = raw.parse::<u32>().map_err(|_| {
        format!("SoraFS reserve query parameter `{name}` must be an unsigned integer")
    })?;
    if value.to_string() != raw {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must use canonical decimal encoding"
        ));
    }
    *target = Some(value);
    Ok(())
}

fn parse_unique_hex(
    target: &mut Option<[u8; 32]>,
    name: &str,
    raw: &str,
    nonzero: bool,
) -> Result<(), String> {
    if target.is_some() || raw.is_empty() {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must appear once with a value"
        ));
    }
    let value = parse_hex(raw, name)?;
    if nonzero && value == [0; 32] {
        return Err(format!(
            "SoraFS reserve query parameter `{name}` must not be zero"
        ));
    }
    *target = Some(value);
    Ok(())
}

fn validate_finalized_cursor_pair(
    height: Option<u64>,
    block_hash: Option<[u8; 32]>,
) -> Result<(), Response> {
    if height.is_some() != block_hash.is_some() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "complete finalized SoraFS reserve cursor is required",
        ));
    }
    if height == Some(0) {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "finalized SoraFS reserve cursor height must be non-zero",
        ));
    }
    Ok(())
}

fn parse_nonzero_hex(raw: &str, name: &str) -> Result<[u8; 32], String> {
    let value = parse_hex(raw, name)?;
    if value == [0; 32] {
        return Err(format!("SoraFS reserve `{name}` must not be zero"));
    }
    Ok(value)
}

fn parse_hex(raw: &str, name: &str) -> Result<[u8; 32], String> {
    if raw.len() != 64
        || raw
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "SoraFS reserve `{name}` must be 64 lowercase hexadecimal characters"
        ));
    }
    let bytes = hex::decode(raw)
        .map_err(|_| format!("SoraFS reserve `{name}` is not valid hexadecimal"))?;
    bytes
        .try_into()
        .map_err(|_| format!("SoraFS reserve `{name}` must decode to 32 bytes"))
}

fn feature_disabled() -> Response {
    json_error(
        StatusCode::NOT_FOUND,
        "SoraFS reserve API is not enabled on this node",
    )
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        JsonBody(crate::json_object(vec![
            crate::json_entry("error", status.canonical_reason().unwrap_or("error")),
            crate::json_entry("message", message.into()),
        ])),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        isi::{InstructionBox, sorafs::RequestSorafsReserveMovement},
        metadata::Metadata,
        sorafs::{capacity::ProviderId, reserve::ReserveMovementKindV1},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::deal::XorQuantity;

    use super::*;

    fn signed_transaction(
        chain: &ChainId,
        instruction: InstructionBox,
        mutate: impl FnOnce(TransactionBuilder) -> TransactionBuilder,
    ) -> SignedTransaction {
        signed_transactions(chain, vec![instruction], mutate)
    }

    fn signed_transactions(
        chain: &ChainId,
        instructions: Vec<InstructionBox>,
        mutate: impl FnOnce(TransactionBuilder) -> TransactionBuilder,
    ) -> SignedTransaction {
        let key_pair =
            KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519).expect("test key");
        let authority = AccountId::new(key_pair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_ttl(RESERVE_TRANSACTION_TTL_V1);
        mutate(builder)
            .with_instructions(instructions)
            .sign(key_pair.private_key())
    }

    fn movement(kind: ReserveMovementKindV1) -> InstructionBox {
        RequestSorafsReserveMovement::new(
            [0x11; 32],
            ProviderId::new([0x22; 32]),
            kind,
            "1".parse::<XorQuantity>().expect("quantity"),
            7,
            [0x33; 32],
        )
        .into()
    }

    #[test]
    fn signed_boundary_rejects_wrong_route_chain_and_noncanonical_envelope_fields() {
        let chain = ChainId::from("reserve-boundary");
        let withdrawal = signed_transaction(
            &chain,
            movement(ReserveMovementKindV1::Withdrawal),
            |builder| builder,
        );
        assert_eq!(
            validate_reserve_signed_envelope_and_route(
                &chain,
                &withdrawal,
                ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp),
            )
            .expect_err("wrong route")
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            validate_reserve_signed_envelope_and_route(
                &ChainId::from("other-chain"),
                &withdrawal,
                ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::Withdrawal),
            )
            .expect_err("wrong chain")
            .status(),
            StatusCode::BAD_REQUEST
        );

        let nonce = signed_transaction(
            &chain,
            movement(ReserveMovementKindV1::TopUp),
            |mut builder| {
                builder.set_nonce(NonZeroU32::new(1).expect("nonce"));
                builder
            },
        );
        assert_eq!(
            validate_reserve_signed_envelope_and_route(
                &chain,
                &nonce,
                ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp),
            )
            .expect_err("nonce")
            .status(),
            StatusCode::BAD_REQUEST
        );

        let mut forbidden_metadata = Metadata::default();
        forbidden_metadata.insert("forbidden".parse().expect("metadata key"), true);
        let metadata =
            signed_transaction(&chain, movement(ReserveMovementKindV1::TopUp), |builder| {
                builder.with_metadata(forbidden_metadata)
            });
        assert_eq!(
            validate_reserve_signed_envelope_and_route(
                &chain,
                &metadata,
                ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp),
            )
            .expect_err("metadata")
            .status(),
            StatusCode::BAD_REQUEST
        );

        let multiple = signed_transactions(
            &chain,
            vec![
                movement(ReserveMovementKindV1::TopUp),
                movement(ReserveMovementKindV1::TopUp),
            ],
            |builder| builder,
        );
        assert_eq!(
            validate_reserve_signed_envelope_and_route(
                &chain,
                &multiple,
                ReserveCommandRouteV1::RequestMovement(ReserveMovementKindV1::TopUp),
            )
            .expect_err("multiple instructions")
            .status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn finalized_query_parser_rejects_duplicates_partial_cursors_and_noncanonical_hex() {
        assert!(ReserveAnchorQueryV1::parse(Some("limit=1")).is_err());
        assert!(
            ReservePageQueryV1::parse(Some("limit=1&limit=2"), "after_provider_id_hex").is_err()
        );
        assert!(
            ReservePageQueryV1::parse(Some("expected_finalized_height=1"), "after_provider_id_hex")
                .is_err()
        );
        assert!(
            ReservePageQueryV1::parse(
                Some(
                    "expected_finalized_height=1&expected_finalized_block_hash_hex=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                ),
                "after_provider_id_hex"
            )
            .is_err()
        );
        assert!(ReserveEventQueryV1::parse(Some("after_sequence=1")).is_err());
        assert!(
            ReserveEventQueryV1::parse(Some(
                "after_sequence=1&after_block_height=1&after_block_hash_hex=0000000000000000000000000000000000000000000000000000000000000000&after_event_index=0"
            ))
            .is_err()
        );
    }

    #[test]
    fn empty_initial_event_page_preserves_exclusive_resume_cursor() {
        let after = ReserveFinalizedEventCursorV1 {
            sequence: 9,
            block_height: 7,
            block_hash: [0xA9; 32],
            event_index: 2,
        };
        assert_eq!(reserve_event_resume_cursor(&[], Some(after)), Some(after));
    }
}
