//! Route-to-instruction validation for native `SoraFS` reserve submissions.

use eyre::{Result, eyre};
use iroha_data_model::{
    isi::sorafs::{
        DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
        RepaySorafsReserveCredit, RequestSorafsReserveMovement, SubmitSorafsReserveAppeal,
    },
    sorafs::{
        capacity::ProviderId,
        reserve::{
            RESERVE_QUERY_MAX_ITEMS_V1, ReserveAppealPageV1, ReserveAppealRecordV1,
            ReserveAuthorityPolicyRecordV1, ReserveFinalizedCursorV1,
            ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1, ReserveFinalizedEventV1,
            ReserveMovementKindV1, ReserveMovementPageV1, ReserveMovementRecordV1,
            ReserveProviderAccountPageV1, ReserveProviderAccountV1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use norito::json::Value;

use super::{
    APPLICATION_JSON, Response, SorafsReserveAppealReadbackFilter, SorafsReserveCommandRoute,
    SorafsReserveEventsReadbackFilter, SorafsReserveFinalizedAnchor,
    SorafsReserveMovementReadbackFilter, SorafsReserveProvidersReadbackFilter, StatusCode,
};

const FINALIZED_RECORD_SCHEMA_V1: &str = "sorafs.reserve.finalized_record.v1";

fn response_error(kind: &str, detail: &str) -> eyre::Report {
    eyre!("invalid finalized SoraFS reserve {kind} response: {detail}")
}

fn request_error(kind: &str, detail: &str) -> eyre::Report {
    eyre!("invalid finalized SoraFS reserve {kind} request: {detail}")
}

fn parse_request_hash(value: &str, kind: &str) -> Result<[u8; 32]> {
    let mut decoded = [0u8; 32];
    if value.len() != 64
        || hex::decode_to_slice(value, &mut decoded).is_err()
        || hex::encode(decoded) != value
        || decoded == [0; 32]
    {
        return Err(request_error(kind, "cursor is not canonical non-zero hex"));
    }
    Ok(decoded)
}

fn validate_anchor(
    expected: &SorafsReserveFinalizedAnchor<'_>,
    kind: &str,
) -> Result<Option<ReserveFinalizedCursorV1>> {
    match (
        expected.expected_finalized_height,
        expected.expected_finalized_block_hash_hex,
    ) {
        (None, None) => Ok(None),
        (Some(height), Some(block_hash)) if height != 0 => Ok(Some(ReserveFinalizedCursorV1 {
            height,
            block_hash: parse_request_hash(block_hash, kind)?,
        })),
        (Some(0), Some(_)) => Err(request_error(kind, "finalized height is zero")),
        _ => Err(request_error(kind, "finalized cursor is incomplete")),
    }
}

fn validate_limit(limit: Option<u32>, kind: &str) -> Result<usize> {
    let limit = limit.unwrap_or(RESERVE_QUERY_MAX_ITEMS_V1);
    if !(1..=RESERVE_QUERY_MAX_ITEMS_V1).contains(&limit) {
        return Err(request_error(kind, "limit is outside the protocol bound"));
    }
    Ok(usize::try_from(limit).expect("bounded reserve query limit fits usize"))
}

fn requested_event_cursor(
    filter: &SorafsReserveEventsReadbackFilter<'_>,
    kind: &str,
) -> Result<Option<ReserveFinalizedEventCursorV1>> {
    match (
        filter.after_sequence,
        filter.after_block_height,
        filter.after_block_hash_hex,
        filter.after_event_index,
    ) {
        (None, None, None, None) => Ok(None),
        (Some(sequence), Some(block_height), Some(block_hash), Some(event_index))
            if sequence != 0 && block_height != 0 =>
        {
            Ok(Some(ReserveFinalizedEventCursorV1 {
                sequence,
                block_height,
                block_hash: parse_request_hash(block_hash, kind)?,
                event_index,
            }))
        }
        (Some(0), Some(_), Some(_), Some(_)) | (Some(_), Some(0), Some(_), Some(_)) => {
            Err(request_error(kind, "event cursor is zero"))
        }
        _ => Err(request_error(kind, "event cursor is incomplete")),
    }
}

fn validate_event_cursor(
    cursor: ReserveFinalizedEventCursorV1,
    finalized: ReserveFinalizedCursorV1,
    kind: &str,
) -> Result<()> {
    if cursor.sequence == 0 || cursor.block_height == 0 || cursor.block_hash == [0; 32] {
        return Err(response_error(kind, "event cursor is zero"));
    }
    if cursor.block_height > finalized.height
        || (cursor.block_height == finalized.height && cursor.block_hash != finalized.block_hash)
    {
        return Err(response_error(
            kind,
            "event cursor is outside the finalized view",
        ));
    }
    Ok(())
}

fn validate_event_successor(
    previous: Option<ReserveFinalizedEventCursorV1>,
    current: ReserveFinalizedEventCursorV1,
    kind: &str,
) -> Result<()> {
    let Some(previous) = previous else {
        if current.sequence != 1 || current.event_index != 0 {
            return Err(response_error(
                kind,
                "initial event is not sequence one at block index zero",
            ));
        }
        return Ok(());
    };
    if previous.sequence.checked_add(1) != Some(current.sequence) {
        return Err(response_error(kind, "event sequence is not contiguous"));
    }
    match previous.block_height.cmp(&current.block_height) {
        core::cmp::Ordering::Less if current.event_index == 0 => Ok(()),
        core::cmp::Ordering::Equal
            if previous.block_hash == current.block_hash
                && previous.event_index.checked_add(1) == Some(current.event_index) =>
        {
            Ok(())
        }
        _ => Err(response_error(
            kind,
            "event block height and index are not contiguous",
        )),
    }
}

fn exact_json_object(response: &Response<Vec<u8>>, kind: &str, keys: &[&str]) -> Result<Value> {
    let mut content_types = response.headers().get_all("content-type").iter();
    if content_types.next().and_then(|value| value.to_str().ok()) != Some(APPLICATION_JSON)
        || content_types.next().is_some()
    {
        return Err(response_error(
            kind,
            "expected exactly one application/json content type",
        ));
    }
    let value: Value = norito::json::from_slice(response.body())
        .map_err(|_| response_error(kind, "body is not valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| response_error(kind, "body is not an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(response_error(
            kind,
            "object fields do not match the schema",
        ));
    }
    Ok(value)
}

fn exact_record_payload(
    response: &Response<Vec<u8>>,
    kind: &str,
    payload_key: &'static str,
) -> Result<(ReserveFinalizedCursorV1, Value)> {
    let value = exact_json_object(response, kind, &["schema", "finalized_cursor", payload_key])?;
    let object = value
        .as_object()
        .expect("exact JSON object validation succeeded");
    if object.get("schema").and_then(Value::as_str) != Some(FINALIZED_RECORD_SCHEMA_V1) {
        return Err(response_error(kind, "record schema is not V1"));
    }
    let cursor = norito::json::from_value(
        object
            .get("finalized_cursor")
            .cloned()
            .expect("exact record wrapper has finalized_cursor"),
    )
    .map_err(|_| response_error(kind, "finalized cursor is not the typed DTO"))?;
    let payload = object
        .get(payload_key)
        .cloned()
        .expect("exact record wrapper has its payload");
    Ok((cursor, payload))
}

fn validate_finalized_cursor(
    cursor: ReserveFinalizedCursorV1,
    expected: &SorafsReserveFinalizedAnchor<'_>,
    kind: &str,
) -> Result<()> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] {
        return Err(response_error(kind, "finalized cursor is zero"));
    }
    if validate_anchor(expected, kind)?.is_some_and(|expected| expected != cursor) {
        return Err(response_error(
            kind,
            "finalized cursor does not match the request",
        ));
    }
    Ok(())
}

fn validate_id_page<T>(
    records: &[T],
    mut previous: Option<[u8; 32]>,
    id: impl Fn(&T) -> [u8; 32],
    has_more: bool,
    next_after: Option<[u8; 32]>,
    kind: &str,
) -> Result<()> {
    for record in records {
        let current = id(record);
        if current == [0; 32] || previous.is_some_and(|previous| current <= previous) {
            return Err(response_error(kind, "identifiers are not strictly ordered"));
        }
        previous = Some(current);
    }
    if has_more != next_after.is_some()
        || next_after.is_some_and(|next| records.last().map(&id) != Some(next))
    {
        return Err(response_error(kind, "continuation cursor is inconsistent"));
    }
    Ok(())
}

/// Validate one finalized-anchor request before any HTTP call.
pub(super) fn validate_anchor_request(
    expected: &SorafsReserveFinalizedAnchor<'_>,
    kind: &str,
) -> Result<()> {
    validate_anchor(expected, kind).map(drop)
}

/// Validate one non-zero detail identifier and finalized anchor before HTTP.
pub(super) fn validate_detail_request(
    id_hex: &str,
    expected: &SorafsReserveFinalizedAnchor<'_>,
    kind: &str,
) -> Result<()> {
    parse_request_hash(id_hex, kind)?;
    validate_anchor_request(expected, kind)
}

/// Validate one provider-page request before any HTTP call.
pub(super) fn validate_providers_request(
    filter: &SorafsReserveProvidersReadbackFilter<'_>,
) -> Result<()> {
    validate_anchor(&filter.finalized, "provider page")?;
    validate_limit(filter.limit, "provider page")?;
    filter
        .after_provider_id_hex
        .map(|after| parse_request_hash(after, "provider page"))
        .transpose()?;
    Ok(())
}

/// Validate one movement-page request before any HTTP call.
pub(super) fn validate_movements_request(
    filter: &SorafsReserveMovementReadbackFilter<'_>,
) -> Result<()> {
    validate_anchor(&filter.finalized, "movement page")?;
    validate_limit(filter.limit, "movement page")?;
    filter
        .after_movement_id_hex
        .map(|after| parse_request_hash(after, "movement page"))
        .transpose()?;
    Ok(())
}

/// Validate one appeal-page request before any HTTP call.
pub(super) fn validate_appeals_request(
    filter: &SorafsReserveAppealReadbackFilter<'_>,
) -> Result<()> {
    validate_anchor(&filter.finalized, "appeal page")?;
    validate_limit(filter.limit, "appeal page")?;
    filter
        .after_appeal_id_hex
        .map(|after| parse_request_hash(after, "appeal page"))
        .transpose()?;
    Ok(())
}

/// Validate one event-page request before any HTTP call.
pub(super) fn validate_events_request(
    filter: &SorafsReserveEventsReadbackFilter<'_>,
) -> Result<()> {
    let finalized = validate_anchor(&filter.finalized, "event page")?;
    validate_limit(filter.limit, "event page")?;
    if let Some(after) = requested_event_cursor(filter, "event page")?
        && let Some(finalized) = finalized
    {
        validate_event_cursor(after, finalized, "event page")
            .map_err(|_| request_error("event page", "event cursor is outside the anchor"))?;
    }
    Ok(())
}

/// Validate a successful finalized reserve-policy response without rewriting it.
pub(super) fn validate_policy_response(
    response: Response<Vec<u8>>,
    expected: &SorafsReserveFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let (cursor, payload) = exact_record_payload(&response, "policy", "policy")?;
    let _: ReserveAuthorityPolicyRecordV1 = norito::json::from_value(payload)
        .map_err(|_| response_error("policy", "payload is not the typed policy DTO"))?;
    validate_finalized_cursor(cursor, expected, "policy")?;
    Ok(response)
}

/// Validate a successful finalized reserve-provider page without rewriting it.
pub(super) fn validate_providers_response(
    response: Response<Vec<u8>>,
    filter: &SorafsReserveProvidersReadbackFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let value = exact_json_object(
        &response,
        "provider page",
        &["finalized_cursor", "accounts", "has_more", "next_after"],
    )?;
    let page: ReserveProviderAccountPageV1 = norito::json::from_value(value)
        .map_err(|_| response_error("provider page", "body is not the typed page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "provider page")?;
    if page.accounts.len() > validate_limit(filter.limit, "provider page")? {
        return Err(response_error(
            "provider page",
            "payload exceeds the requested limit",
        ));
    }
    let after = filter
        .after_provider_id_hex
        .map(|value| parse_request_hash(value, "provider page"))
        .transpose()?;
    validate_id_page(
        &page.accounts,
        after,
        |account| account.terms.provider_id.0,
        page.has_more,
        page.next_after.map(|provider| provider.0),
        "provider page",
    )?;
    Ok(response)
}

/// Validate a successful finalized reserve-provider record without rewriting it.
pub(super) fn validate_provider_response(
    response: Response<Vec<u8>>,
    provider_id_hex: &str,
    expected: &SorafsReserveFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let (cursor, payload) = exact_record_payload(&response, "provider", "provider")?;
    let provider: ReserveProviderAccountV1 = norito::json::from_value(payload)
        .map_err(|_| response_error("provider", "payload is not the typed provider DTO"))?;
    validate_finalized_cursor(cursor, expected, "provider")?;
    if provider.terms.provider_id
        != ProviderId::new(parse_request_hash(provider_id_hex, "provider")?)
    {
        return Err(response_error(
            "provider",
            "identifier does not match the request",
        ));
    }
    Ok(response)
}

/// Validate a successful finalized reserve-movement page without rewriting it.
pub(super) fn validate_movements_response(
    response: Response<Vec<u8>>,
    filter: &SorafsReserveMovementReadbackFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let value = exact_json_object(
        &response,
        "movement page",
        &["finalized_cursor", "movements", "has_more", "next_after"],
    )?;
    let page: ReserveMovementPageV1 = norito::json::from_value(value)
        .map_err(|_| response_error("movement page", "body is not the typed page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "movement page")?;
    if page.movements.len() > validate_limit(filter.limit, "movement page")? {
        return Err(response_error(
            "movement page",
            "payload exceeds the requested limit",
        ));
    }
    let after = filter
        .after_movement_id_hex
        .map(|value| parse_request_hash(value, "movement page"))
        .transpose()?;
    validate_id_page(
        &page.movements,
        after,
        |movement| movement.movement_id,
        page.has_more,
        page.next_after,
        "movement page",
    )?;
    Ok(response)
}

/// Validate a successful finalized reserve-movement record without rewriting it.
pub(super) fn validate_movement_response(
    response: Response<Vec<u8>>,
    movement_id_hex: &str,
    expected: &SorafsReserveFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let (cursor, payload) = exact_record_payload(&response, "movement", "movement")?;
    let movement: ReserveMovementRecordV1 = norito::json::from_value(payload)
        .map_err(|_| response_error("movement", "payload is not the typed movement DTO"))?;
    validate_finalized_cursor(cursor, expected, "movement")?;
    if movement.movement_id != parse_request_hash(movement_id_hex, "movement")? {
        return Err(response_error(
            "movement",
            "identifier does not match the request",
        ));
    }
    Ok(response)
}

/// Validate a successful finalized reserve-appeal page without rewriting it.
pub(super) fn validate_appeals_response(
    response: Response<Vec<u8>>,
    filter: &SorafsReserveAppealReadbackFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let value = exact_json_object(
        &response,
        "appeal page",
        &["finalized_cursor", "appeals", "has_more", "next_after"],
    )?;
    let page: ReserveAppealPageV1 = norito::json::from_value(value)
        .map_err(|_| response_error("appeal page", "body is not the typed page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "appeal page")?;
    if page.appeals.len() > validate_limit(filter.limit, "appeal page")? {
        return Err(response_error(
            "appeal page",
            "payload exceeds the requested limit",
        ));
    }
    let after = filter
        .after_appeal_id_hex
        .map(|value| parse_request_hash(value, "appeal page"))
        .transpose()?;
    validate_id_page(
        &page.appeals,
        after,
        |appeal| appeal.appeal_id,
        page.has_more,
        page.next_after,
        "appeal page",
    )?;
    Ok(response)
}

/// Validate a successful finalized reserve-appeal record without rewriting it.
pub(super) fn validate_appeal_response(
    response: Response<Vec<u8>>,
    appeal_id_hex: &str,
    expected: &SorafsReserveFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let (cursor, payload) = exact_record_payload(&response, "appeal", "appeal")?;
    let appeal: ReserveAppealRecordV1 = norito::json::from_value(payload)
        .map_err(|_| response_error("appeal", "payload is not the typed appeal DTO"))?;
    validate_finalized_cursor(cursor, expected, "appeal")?;
    if appeal.appeal_id != parse_request_hash(appeal_id_hex, "appeal")? {
        return Err(response_error(
            "appeal",
            "identifier does not match the request",
        ));
    }
    Ok(response)
}

/// Validate a successful finalized reserve-event page without rewriting it.
pub(super) fn validate_events_response(
    response: Response<Vec<u8>>,
    filter: &SorafsReserveEventsReadbackFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let value = exact_json_object(
        &response,
        "event page",
        &["finalized_cursor", "events", "has_more", "next_after"],
    )?;
    let page: ReserveFinalizedEventPageV1 = norito::json::from_value(value)
        .map_err(|_| response_error("event page", "body is not the typed page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "event page")?;
    if page.events.len() > validate_limit(filter.limit, "event page")? {
        return Err(response_error(
            "event page",
            "payload exceeds the requested limit",
        ));
    }
    let after = requested_event_cursor(filter, "event page")?;
    if let Some(after) = after {
        validate_event_cursor(after, page.finalized_cursor, "event page")?;
    }
    let mut previous = after;
    for event in &page.events {
        let current = event.cursor();
        validate_event_cursor(current, page.finalized_cursor, "event page")?;
        validate_event_successor(previous, current, "event page")?;
        previous = Some(current);
    }
    if page.has_more != page.next_after.is_some()
        || page.next_after.is_some_and(|next| {
            page.events.last().map(ReserveFinalizedEventV1::cursor) != Some(next)
        })
    {
        return Err(response_error(
            "event page",
            "continuation cursor is inconsistent",
        ));
    }
    Ok(response)
}

impl SorafsReserveCommandRoute {
    pub(super) const fn expected_instruction_label(self) -> &'static str {
        match self {
            Self::TopUp => "RequestSorafsReserveMovement::TopUp",
            Self::Withdrawal => "RequestSorafsReserveMovement::Withdrawal",
            Self::MovementDecision(_) => "DecideSorafsReserveMovement",
            Self::CreditDraw => "DrawSorafsReserveCredit",
            Self::CreditRepay => "RepaySorafsReserveCredit",
            Self::Appeal => "SubmitSorafsReserveAppeal",
            Self::AppealDecision(_) => "DecideSorafsReserveAppeal",
        }
    }
}

fn route_mismatch(route: SorafsReserveCommandRoute) -> eyre::Report {
    eyre!(
        "SoraFS reserve route requires exactly one `{}` native instruction",
        route.expected_instruction_label()
    )
}

/// Reject a transaction unless it contains the one native instruction selected by `route`.
pub(super) fn validate_transaction_route(
    route: SorafsReserveCommandRoute,
    transaction: &SignedTransaction,
) -> Result<()> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(route_mismatch(route));
    };
    let [instruction] = instructions.as_ref() else {
        return Err(route_mismatch(route));
    };
    let matches_route = match route {
        SorafsReserveCommandRoute::TopUp => instruction
            .as_any()
            .downcast_ref::<RequestSorafsReserveMovement>()
            .is_some_and(|request| request.kind == ReserveMovementKindV1::TopUp),
        SorafsReserveCommandRoute::Withdrawal => instruction
            .as_any()
            .downcast_ref::<RequestSorafsReserveMovement>()
            .is_some_and(|request| request.kind == ReserveMovementKindV1::Withdrawal),
        SorafsReserveCommandRoute::MovementDecision(path_id) => {
            path_id != [0; 32]
                && instruction
                    .as_any()
                    .downcast_ref::<DecideSorafsReserveMovement>()
                    .is_some_and(|decision| decision.movement_id == path_id)
        }
        SorafsReserveCommandRoute::CreditDraw => instruction
            .as_any()
            .downcast_ref::<DrawSorafsReserveCredit>()
            .is_some(),
        SorafsReserveCommandRoute::CreditRepay => instruction
            .as_any()
            .downcast_ref::<RepaySorafsReserveCredit>()
            .is_some(),
        SorafsReserveCommandRoute::Appeal => instruction
            .as_any()
            .downcast_ref::<SubmitSorafsReserveAppeal>()
            .is_some(),
        SorafsReserveCommandRoute::AppealDecision(path_id) => {
            path_id != [0; 32]
                && instruction
                    .as_any()
                    .downcast_ref::<DecideSorafsReserveAppeal>()
                    .is_some_and(|decision| decision.appeal_id == path_id)
        }
    };
    if !matches_route {
        return Err(route_mismatch(route));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, sync::Arc, time::Duration};

    use iroha_data_model::{
        Level,
        asset::AssetDefinitionId,
        domain::DomainId,
        events::data::sorafs::{SorafsReserveLedgerEvent, SorafsReserveLedgerEventKind},
        isi::{
            InstructionBox, Log,
            sorafs::{
                DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
                RepaySorafsReserveCredit, RequestSorafsReserveMovement, SubmitSorafsReserveAppeal,
            },
        },
        metadata::Metadata,
        sorafs::{
            capacity::ProviderId,
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAppealPageV1, ReserveAppealRecordV1,
                ReserveAppealStatusV1, ReserveAuthorityPolicyRecordV1, ReserveAuthorityPolicyV1,
                ReserveDuration, ReserveFinalizedCursorV1, ReserveFinalizedEventPageV1,
                ReserveFinalizedEventV1, ReserveLifecycleStage, ReserveMovementKindV1,
                ReserveMovementPageV1, ReserveMovementRecordV1, ReserveMovementStatusV1,
                ReservePolicyV1, ReserveProviderAccountPageV1, ReserveProviderAccountV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
        transaction::{
            Executable, FeePaymentIntent, IvmBytecode, SignedTransaction, TransactionBuilder,
        },
    };

    use super::*;
    use crate::{
        client::evidence_http_tests::{
            SnapshotStore, base_url, client_with_base_url, empty_response, respond_with,
            with_mock_http,
        },
        http::StatusCode,
    };

    const EXACT_RESERVE_TTL: Duration = Duration::from_secs(300);
    const MOVEMENT_ID: [u8; 32] = [0x61; 32];
    const APPEAL_ID: [u8; 32] = [0x62; 32];

    fn finalized_cursor() -> ReserveFinalizedCursorV1 {
        ReserveFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        }
    }

    fn finalized_anchor<'a>(block_hash: &'a str) -> SorafsReserveFinalizedAnchor<'a> {
        SorafsReserveFinalizedAnchor {
            expected_finalized_height: Some(7),
            expected_finalized_block_hash_hex: Some(block_hash),
        }
    }

    fn policy_record(client: &super::super::Client) -> ReserveAuthorityPolicyRecordV1 {
        let policy = ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: ReservePolicyV1::default(),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("reserve", "universal").expect("reserve domain"),
                "xor".parse().expect("reserve asset"),
            ),
            custody_account: client.account.clone(),
            treasury_account: client.account.clone(),
            operations_authority: client.account.clone(),
            decision_authority: client.account.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: "1".parse().expect("debt cap"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        };
        ReserveAuthorityPolicyRecordV1 {
            policy,
            policy_digest: [0x64; 32],
            activated_by: client.account.clone(),
            activated_at_unix: 1,
        }
    }

    fn provider_account(
        client: &super::super::Client,
        provider_id: [u8; 32],
    ) -> ReserveProviderAccountV1 {
        ReserveProviderAccountV1 {
            terms: ReserveProviderTermsV1 {
                provider_id: ProviderId::new(provider_id),
                provider_account: client.account.clone(),
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 1,
            },
            policy_digest: [0x64; 32],
            revision: 1,
            reserve_balance: "1".parse().expect("reserve balance"),
            debt_principal: "0".parse().expect("debt principal"),
            accrued_interest: "0".parse().expect("interest"),
            credit_cap: "1".parse().expect("credit cap"),
            lifecycle_stage: ReserveLifecycleStage::Active,
            days_past_due: 0,
            pending_movements: 0,
            open_appeals: 0,
            rent_charged_through_unix: 1,
            interest_accrued_at_unix: 1,
            updated_at_unix: 1,
        }
    }

    fn movement_record(
        client: &super::super::Client,
        movement_id: [u8; 32],
    ) -> ReserveMovementRecordV1 {
        ReserveMovementRecordV1 {
            movement_id,
            provider_id: ProviderId::new([0x63; 32]),
            kind: ReserveMovementKindV1::TopUp,
            amount: "1".parse().expect("movement amount"),
            requested_by: client.account.clone(),
            expected_provider_revision: 1,
            policy_digest: [0x64; 32],
            status: ReserveMovementStatusV1::Pending,
            requested_at_unix: 1,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        }
    }

    fn appeal_record(client: &super::super::Client, appeal_id: [u8; 32]) -> ReserveAppealRecordV1 {
        ReserveAppealRecordV1 {
            appeal_id,
            provider_id: ProviderId::new([0x63; 32]),
            submitted_by: client.account.clone(),
            requested_stage: ReserveLifecycleStage::Active,
            reason: "appeal".to_owned(),
            evidence_digest: None,
            expected_provider_revision: 1,
            status: ReserveAppealStatusV1::Pending,
            submitted_at_unix: 1,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        }
    }

    fn reserve_event(
        client: &super::super::Client,
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
    ) -> ReserveFinalizedEventV1 {
        ReserveFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            event: SorafsReserveLedgerEvent {
                kind: SorafsReserveLedgerEventKind::ProviderRegistered,
                provider_id: Some(ProviderId::new([0x63; 32])),
                operation_id: None,
                policy_digest: [0x64; 32],
                provider_revision: sequence,
                resulting_lifecycle_stage: Some(ReserveLifecycleStage::Active),
                authority: client.account.clone(),
                occurred_at_unix_ms: sequence,
            },
        }
    }

    fn page_response<T: norito::json::JsonSerialize>(payload: &T) -> Response<Vec<u8>> {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .body(norito::json::to_vec(payload).expect("encode reserve page"))
            .expect("build reserve page response")
    }

    fn record_response<T: norito::json::JsonSerialize>(
        key: &str,
        payload: &T,
    ) -> Response<Vec<u8>> {
        let mut wrapper = norito::json::Map::new();
        wrapper.insert("schema".to_owned(), Value::from(FINALIZED_RECORD_SCHEMA_V1));
        wrapper.insert(
            "finalized_cursor".to_owned(),
            norito::json::to_value(finalized_cursor()).expect("encode finalized cursor"),
        );
        wrapper.insert(
            key.to_owned(),
            norito::json::to_value(payload).expect("encode reserve record"),
        );
        page_response(&Value::Object(wrapper))
    }

    fn non_ok_response() -> Response<Vec<u8>> {
        Response::builder()
            .status(StatusCode::CONFLICT)
            .header("content-type", "application/problem+json")
            .header("x-reserve-proof", "opaque")
            .body(vec![0x00, 0xFF, 0x51, 0x00])
            .expect("build non-OK response")
    }

    fn sign_executable(client: &super::super::Client, executable: Executable) -> SignedTransaction {
        let gas_limit = executable
            .requires_transaction_gas_limit()
            .then(|| NonZeroU64::new(1).expect("non-zero gas limit"));
        let mut builder = TransactionBuilder::new(
            client.network_id,
            client.account.clone(),
            FeePaymentIntent::authority(Vec::new(), gas_limit),
        )
        .with_executable(executable)
        .with_metadata(Metadata::default());
        builder.set_ttl(EXACT_RESERVE_TTL);
        client
            .try_sign_transaction(builder)
            .expect("sign reserve route validation fixture")
    }

    fn sign_instruction(
        client: &super::super::Client,
        instruction: impl Into<InstructionBox>,
    ) -> SignedTransaction {
        sign_executable(
            client,
            Executable::Instructions(vec![instruction.into()].into()),
        )
    }

    fn movement(kind: ReserveMovementKindV1) -> RequestSorafsReserveMovement {
        RequestSorafsReserveMovement::new(
            MOVEMENT_ID,
            ProviderId::new([0x63; 32]),
            kind,
            "1".parse().expect("reserve amount"),
            1,
            [0x64; 32],
        )
    }

    fn movement_decision(movement_id: [u8; 32]) -> DecideSorafsReserveMovement {
        DecideSorafsReserveMovement::new(movement_id, 1, [0x64; 32], true, "approved".to_owned())
    }

    fn appeal_decision(appeal_id: [u8; 32]) -> DecideSorafsReserveAppeal {
        DecideSorafsReserveAppeal::new(appeal_id, 1, [0x64; 32], true, "accepted".to_owned())
    }

    fn exact_route_instructions() -> [(SorafsReserveCommandRoute, InstructionBox); 7] {
        let provider_id = ProviderId::new([0x63; 32]);
        [
            (
                SorafsReserveCommandRoute::TopUp,
                movement(ReserveMovementKindV1::TopUp).into(),
            ),
            (
                SorafsReserveCommandRoute::Withdrawal,
                movement(ReserveMovementKindV1::Withdrawal).into(),
            ),
            (
                SorafsReserveCommandRoute::MovementDecision(MOVEMENT_ID),
                movement_decision(MOVEMENT_ID).into(),
            ),
            (
                SorafsReserveCommandRoute::CreditDraw,
                DrawSorafsReserveCredit::new(
                    provider_id,
                    1,
                    "1".parse().expect("credit amount"),
                    [0x64; 32],
                )
                .into(),
            ),
            (
                SorafsReserveCommandRoute::CreditRepay,
                RepaySorafsReserveCredit::new(
                    provider_id,
                    1,
                    "1".parse().expect("repayment amount"),
                    [0x64; 32],
                )
                .into(),
            ),
            (
                SorafsReserveCommandRoute::Appeal,
                SubmitSorafsReserveAppeal::new(
                    APPEAL_ID,
                    provider_id,
                    1,
                    ReserveLifecycleStage::Active,
                    "appeal".to_owned(),
                    None,
                    [0x64; 32],
                )
                .into(),
            ),
            (
                SorafsReserveCommandRoute::AppealDecision(APPEAL_ID),
                appeal_decision(APPEAL_ID).into(),
            ),
        ]
    }

    fn assert_rejected_before_http(
        client: &super::super::Client,
        route: SorafsReserveCommandRoute,
        transaction: &SignedTransaction,
    ) {
        let snapshots: SnapshotStore = Arc::default();
        let error = with_mock_http(
            respond_with(&snapshots, empty_response(StatusCode::ACCEPTED)),
            || {
                client
                    .post_sorafs_reserve_transaction(route, transaction)
                    .expect_err("invalid reserve route transaction must fail locally")
            },
        );
        assert_eq!(
            error.to_string(),
            format!(
                "SoraFS reserve route requires exactly one `{}` native instruction",
                route.expected_instruction_label()
            )
        );
        assert!(
            snapshots.lock().expect("snapshot lock").is_empty(),
            "route validation must run before capability lookup or command HTTP"
        );
    }

    #[test]
    fn reserve_route_validation_accepts_every_exact_instruction() {
        let client = client_with_base_url(base_url());
        for (route, instruction) in exact_route_instructions() {
            let transaction = sign_instruction(&client, instruction);
            assert_eq!(transaction.time_to_live(), Some(EXACT_RESERVE_TTL));
            validate_transaction_route(route, &transaction).expect("matching reserve route");
        }
    }

    #[test]
    fn reserve_route_validation_rejects_wrong_kind_and_identifiers_before_http() {
        let client = client_with_base_url(base_url());
        let withdrawal = sign_instruction(&client, movement(ReserveMovementKindV1::Withdrawal));
        assert_rejected_before_http(&client, SorafsReserveCommandRoute::TopUp, &withdrawal);

        let movement = sign_instruction(&client, movement_decision(MOVEMENT_ID));
        assert_rejected_before_http(
            &client,
            SorafsReserveCommandRoute::MovementDecision([0x71; 32]),
            &movement,
        );
        let appeal = sign_instruction(&client, appeal_decision(APPEAL_ID));
        assert_rejected_before_http(
            &client,
            SorafsReserveCommandRoute::AppealDecision([0x72; 32]),
            &appeal,
        );
        for (route, transaction) in [
            (
                SorafsReserveCommandRoute::MovementDecision([0; 32]),
                sign_instruction(&client, movement_decision([0; 32])),
            ),
            (
                SorafsReserveCommandRoute::AppealDecision([0; 32]),
                sign_instruction(&client, appeal_decision([0; 32])),
            ),
        ] {
            assert_rejected_before_http(&client, route, &transaction);
        }
    }

    #[test]
    fn reserve_route_validation_rejects_wrong_type_non_native_and_non_singleton_before_http() {
        let client = client_with_base_url(base_url());
        let draw = sign_instruction(
            &client,
            DrawSorafsReserveCredit::new(
                ProviderId::new([0x63; 32]),
                1,
                "1".parse().expect("credit amount"),
                [0x64; 32],
            ),
        );
        assert_rejected_before_http(&client, SorafsReserveCommandRoute::TopUp, &draw);

        let log = sign_instruction(
            &client,
            Log::new(Level::INFO, "not a reserve instruction".into()),
        );
        assert_rejected_before_http(&client, SorafsReserveCommandRoute::TopUp, &log);

        let top_up: InstructionBox = movement(ReserveMovementKindV1::TopUp).into();
        let multiple = sign_executable(
            &client,
            Executable::Instructions(vec![top_up.clone(), top_up].into()),
        );
        assert_rejected_before_http(&client, SorafsReserveCommandRoute::TopUp, &multiple);

        let ivm = sign_executable(
            &client,
            Executable::Ivm(IvmBytecode::from_compiled(vec![0x00])),
        );
        assert_rejected_before_http(&client, SorafsReserveCommandRoute::TopUp, &ivm);
    }

    #[test]
    fn reserve_read_response_binding_accepts_exact_typed_records_and_pages() {
        let client = client_with_base_url(base_url());
        let hash = hex::encode(finalized_cursor().block_hash);
        let finalized = finalized_anchor(&hash);

        validate_policy_response(
            record_response("policy", &policy_record(&client)),
            &finalized,
        )
        .expect("exact policy record");

        let provider = provider_account(&client, [0x20; 32]);
        validate_provider_response(
            record_response("provider", &provider),
            &hex::encode(provider.terms.provider_id.0),
            &finalized,
        )
        .expect("exact provider record");
        let provider_page = ReserveProviderAccountPageV1 {
            finalized_cursor: finalized_cursor(),
            accounts: vec![provider, provider_account(&client, [0x30; 32])],
            has_more: true,
            next_after: Some(ProviderId::new([0x30; 32])),
        };
        let provider_after = hex::encode([0x10; 32]);
        validate_providers_response(
            page_response(&provider_page),
            &SorafsReserveProvidersReadbackFilter {
                finalized,
                limit: Some(2),
                after_provider_id_hex: Some(&provider_after),
            },
        )
        .expect("exact provider page");

        let movement = movement_record(&client, [0x20; 32]);
        validate_movement_response(
            record_response("movement", &movement),
            &hex::encode(movement.movement_id),
            &finalized,
        )
        .expect("exact movement record");
        validate_movements_response(
            page_response(&ReserveMovementPageV1 {
                finalized_cursor: finalized_cursor(),
                movements: vec![movement, movement_record(&client, [0x30; 32])],
                has_more: true,
                next_after: Some([0x30; 32]),
            }),
            &SorafsReserveMovementReadbackFilter {
                finalized,
                limit: Some(2),
                after_movement_id_hex: Some(&provider_after),
            },
        )
        .expect("exact movement page");

        let appeal = appeal_record(&client, [0x20; 32]);
        validate_appeal_response(
            record_response("appeal", &appeal),
            &hex::encode(appeal.appeal_id),
            &finalized,
        )
        .expect("exact appeal record");
        validate_appeals_response(
            page_response(&ReserveAppealPageV1 {
                finalized_cursor: finalized_cursor(),
                appeals: vec![appeal, appeal_record(&client, [0x30; 32])],
                has_more: true,
                next_after: Some([0x30; 32]),
            }),
            &SorafsReserveAppealReadbackFilter {
                finalized,
                limit: Some(2),
                after_appeal_id_hex: Some(&provider_after),
            },
        )
        .expect("exact appeal page");
    }

    #[test]
    fn reserve_event_response_binding_accepts_exact_successors() {
        let client = client_with_base_url(base_url());
        let hash = hex::encode(finalized_cursor().block_hash);
        let after_hash = hex::encode([0x51; 32]);
        let events = vec![
            reserve_event(&client, 2, 5, [0x51; 32], 1),
            reserve_event(&client, 3, 7, finalized_cursor().block_hash, 0),
        ];
        let page = ReserveFinalizedEventPageV1 {
            finalized_cursor: finalized_cursor(),
            next_after: events.last().map(ReserveFinalizedEventV1::cursor),
            events,
            has_more: true,
        };
        validate_events_response(
            page_response(&page),
            &SorafsReserveEventsReadbackFilter {
                finalized: finalized_anchor(&hash),
                limit: Some(2),
                after_sequence: Some(1),
                after_block_height: Some(5),
                after_block_hash_hex: Some(&after_hash),
                after_event_index: Some(0),
            },
        )
        .expect("exact contiguous event page");
    }

    #[test]
    fn reserve_read_response_binding_rejects_media_wrapper_finality_and_detail_mismatch() {
        let client = client_with_base_url(base_url());
        let policy = policy_record(&client);
        let mut wrong_media = record_response("policy", &policy);
        wrong_media.headers_mut().insert(
            "content-type",
            "application/json; charset=utf-8"
                .parse()
                .expect("media type"),
        );
        assert!(
            validate_policy_response(wrong_media, &SorafsReserveFinalizedAnchor::default())
                .is_err()
        );
        let mut duplicate_media = record_response("policy", &policy);
        duplicate_media.headers_mut().append(
            "content-type",
            APPLICATION_JSON.parse().expect("media type"),
        );
        assert!(
            validate_policy_response(duplicate_media, &SorafsReserveFinalizedAnchor::default())
                .is_err()
        );

        let mut wrapper = record_response("policy", &policy);
        let mut value: Value =
            norito::json::from_slice(wrapper.body()).expect("decode record wrapper");
        value
            .as_object_mut()
            .expect("record wrapper object")
            .insert("extra".to_owned(), Value::from(true));
        *wrapper.body_mut() = norito::json::to_vec(&value).expect("encode invalid wrapper");
        assert!(
            validate_policy_response(wrapper, &SorafsReserveFinalizedAnchor::default()).is_err()
        );

        let wrong_hash = hex::encode([0x72; 32]);
        assert!(
            validate_policy_response(
                record_response("policy", &policy),
                &finalized_anchor(&wrong_hash),
            )
            .is_err()
        );
        assert!(
            validate_provider_response(
                record_response("provider", &provider_account(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            )
            .is_err()
        );
        assert!(
            validate_movement_response(
                record_response("movement", &movement_record(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            )
            .is_err()
        );
        assert!(
            validate_appeal_response(
                record_response("appeal", &appeal_record(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn reserve_page_response_binding_rejects_bounds_order_exclusivity_and_continuation() {
        let client = client_with_base_url(base_url());
        let first = movement_record(&client, [0x20; 32]);
        let second = movement_record(&client, [0x30; 32]);
        let mut page = ReserveMovementPageV1 {
            finalized_cursor: finalized_cursor(),
            movements: vec![first.clone(), second.clone()],
            has_more: false,
            next_after: None,
        };
        let limit_one = SorafsReserveMovementReadbackFilter {
            limit: Some(1),
            ..SorafsReserveMovementReadbackFilter::default()
        };
        assert!(validate_movements_response(page_response(&page), &limit_one).is_err());

        page.movements = vec![second, first.clone()];
        let limit_two = SorafsReserveMovementReadbackFilter {
            limit: Some(2),
            ..SorafsReserveMovementReadbackFilter::default()
        };
        assert!(validate_movements_response(page_response(&page), &limit_two).is_err());

        page.movements = vec![first];
        let after = hex::encode([0x20; 32]);
        let not_exclusive = SorafsReserveMovementReadbackFilter {
            limit: Some(2),
            after_movement_id_hex: Some(&after),
            ..SorafsReserveMovementReadbackFilter::default()
        };
        assert!(validate_movements_response(page_response(&page), &not_exclusive).is_err());

        page.has_more = true;
        page.next_after = Some([0x21; 32]);
        assert!(validate_movements_response(page_response(&page), &limit_two).is_err());
    }

    #[test]
    fn reserve_event_response_binding_rejects_gaps_finality_and_bad_continuations() {
        let client = client_with_base_url(base_url());
        let filter = SorafsReserveEventsReadbackFilter {
            limit: Some(2),
            ..SorafsReserveEventsReadbackFilter::default()
        };
        let mut page = ReserveFinalizedEventPageV1 {
            finalized_cursor: finalized_cursor(),
            events: vec![reserve_event(&client, 2, 5, [0x51; 32], 0)],
            has_more: false,
            next_after: None,
        };
        assert!(validate_events_response(page_response(&page), &filter).is_err());

        page.events = vec![reserve_event(&client, 1, 5, [0x51; 32], 0)];
        page.events
            .push(reserve_event(&client, 3, 5, [0x51; 32], 1));
        assert!(validate_events_response(page_response(&page), &filter).is_err());

        page.events = vec![
            reserve_event(&client, 1, 5, [0x51; 32], 0),
            reserve_event(&client, 2, 5, [0x52; 32], 1),
        ];
        assert!(validate_events_response(page_response(&page), &filter).is_err());

        page.events = vec![reserve_event(&client, 1, 8, [0x81; 32], 0)];
        assert!(validate_events_response(page_response(&page), &filter).is_err());

        page.events = vec![reserve_event(
            &client,
            1,
            finalized_cursor().height,
            [0x72; 32],
            0,
        )];
        assert!(validate_events_response(page_response(&page), &filter).is_err());

        page.events = vec![reserve_event(&client, 1, 5, [0x51; 32], 0)];
        page.has_more = true;
        page.next_after = Some(reserve_event(&client, 2, 5, [0x51; 32], 1).cursor());
        assert!(validate_events_response(page_response(&page), &filter).is_err());
    }

    #[test]
    fn reserve_malformed_filters_fail_before_http_and_non_ok_is_unchanged() {
        let client = client_with_base_url(base_url());
        let snapshots: SnapshotStore = Arc::default();
        let filter = SorafsReserveEventsReadbackFilter {
            after_sequence: Some(1),
            ..SorafsReserveEventsReadbackFilter::default()
        };
        let error = with_mock_http(
            respond_with(&snapshots, empty_response(StatusCode::OK)),
            || {
                client
                    .get_sorafs_reserve_events(filter)
                    .expect_err("incomplete event cursor must fail locally")
            },
        );
        assert!(error.to_string().contains("event cursor is incomplete"));
        assert!(snapshots.lock().expect("snapshot lock").is_empty());

        assert!(
            validate_providers_request(&SorafsReserveProvidersReadbackFilter {
                limit: Some(RESERVE_QUERY_MAX_ITEMS_V1 + 1),
                ..SorafsReserveProvidersReadbackFilter::default()
            })
            .is_err()
        );
        assert!(
            validate_appeals_request(&SorafsReserveAppealReadbackFilter {
                after_appeal_id_hex: Some("AA"),
                ..SorafsReserveAppealReadbackFilter::default()
            })
            .is_err()
        );
        let response =
            validate_policy_response(non_ok_response(), &SorafsReserveFinalizedAnchor::default())
                .expect("non-OK response is preserved");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        assert_eq!(response.headers()["x-reserve-proof"], "opaque");
        assert_eq!(response.body(), &[0x00, 0xFF, 0x51, 0x00]);
    }
}
