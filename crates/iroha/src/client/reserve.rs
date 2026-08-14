//! Strict route and finalized-read validation for native `SoraFS` reserve APIs.
use super::{
    APPLICATION_JSON, DefaultRequestBuilder, RequestBuilder, Response,
    SorafsReserveAppealReadbackFilter, SorafsReserveCommandRoute,
    SorafsReserveEventsReadbackFilter, SorafsReserveFinalizedAnchor,
    SorafsReserveMovementReadbackFilter, SorafsReserveProvidersReadbackFilter, StatusCode,
};
use eyre::{Result, eyre};
use iroha_data_model::{
    events::data::sorafs::SorafsReserveLedgerEventKind,
    isi::sorafs::{
        DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
        RepaySorafsReserveCredit, RequestSorafsReserveMovement, SubmitSorafsReserveAppeal,
    },
    sorafs::{
        capacity::ProviderId,
        reserve::{
            RESERVE_MAX_OPEN_APPEALS_V1, RESERVE_MAX_PENDING_MOVEMENTS_V1,
            RESERVE_MAX_REASON_BYTES_V1, RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            RESERVE_QUERY_MAX_ITEMS_V1, ReserveAppealPageV1, ReserveAppealRecordV1,
            ReserveAppealStatusV1, ReserveAuthorityPolicyRecordV1, ReserveFinalizedCursorV1,
            ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1, ReserveFinalizedEventV1,
            ReserveMovementKindV1, ReserveMovementPageV1, ReserveMovementRecordV1,
            ReserveMovementStatusV1, ReserveProviderAccountPageV1, ReserveProviderAccountV1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use norito::json::Value;
const FINALIZED_RECORD_SCHEMA_V1: &str = "sorafs.reserve.finalized_record.v1";
const RESERVE_DEFAULT_PAGE_LIMIT_V1: u32 = 100;
const RESERVE_JSON_RESPONSE_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
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
    let limit = limit.unwrap_or(RESERVE_DEFAULT_PAGE_LIMIT_V1);
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
super::repair::define_finalized_event_validators!(
    ReserveFinalizedEventCursorV1,
    ReserveFinalizedCursorV1,
    response_error
);
fn exact_json_object(response: &Response<Vec<u8>>, kind: &str, keys: &[&str]) -> Result<Value> {
    if response.body().len() > RESERVE_JSON_RESPONSE_MAX_BYTES_V1 {
        return Err(response_error(
            kind,
            "body exceeds the JSON transport bound",
        ));
    }
    let mut content_types = response.headers().get_all("content-type").iter();
    if content_types.next().and_then(|value| value.to_str().ok()) != Some(APPLICATION_JSON)
        || content_types.next().is_some()
    {
        return Err(response_error(
            kind,
            "expected exactly one application/json content type",
        ));
    }
    let mut content_encodings = response.headers().get_all("content-encoding").iter();
    match content_encodings.next() {
        None => {}
        Some(value)
            if value.to_str().ok() == Some("identity") && content_encodings.next().is_none() => {}
        Some(_) => {
            return Err(response_error(
                kind,
                "content encoding is not absent or exactly identity",
            ));
        }
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
fn exact_typed_payload<T>(value: Value, kind: &str, detail: &str) -> Result<T>
where
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize,
{
    let decoded: T =
        norito::json::from_value(value.clone()).map_err(|_| response_error(kind, detail))?;
    let canonical = norito::json::to_value(&decoded)
        .map_err(|_| response_error(kind, "typed payload cannot be re-encoded"))?;
    if canonical != value {
        return Err(response_error(
            kind,
            "typed payload contains noncanonical or unknown fields",
        ));
    }
    Ok(decoded)
}
fn validate_encoded_page<T: norito::core::NoritoSerialize>(page: &T, kind: &str) -> Result<()> {
    let encoded_len = norito::to_bytes(page)
        .map_err(|_| response_error(kind, "typed page cannot be encoded as canonical Norito"))?
        .len();
    if encoded_len > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1 {
        return Err(response_error(
            kind,
            "canonical Norito page exceeds the V1 bound",
        ));
    }
    Ok(())
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
    let cursor = exact_typed_payload(
        object
            .get("finalized_cursor")
            .cloned()
            .expect("exact record wrapper has finalized_cursor"),
        kind,
        "finalized cursor is not the typed DTO",
    )?;
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
fn validate_policy_record(record: &ReserveAuthorityPolicyRecordV1, kind: &str) -> Result<()> {
    record
        .policy
        .validate()
        .map_err(|_| response_error(kind, "policy body violates the V1 invariants"))?;
    let digest = record
        .policy
        .digest()
        .map_err(|_| response_error(kind, "policy digest cannot be recomputed"))?;
    if digest != record.policy_digest || record.activated_at_unix == 0 {
        return Err(response_error(
            kind,
            "policy digest or activation timestamp is inconsistent",
        ));
    }
    Ok(())
}
fn validate_provider_record(record: &ReserveProviderAccountV1, kind: &str) -> Result<()> {
    if record.terms.provider_id.0 == [0; 32]
        || record.terms.capacity_gib == 0
        || record.policy_digest == [0; 32]
        || record.revision == 0
        || record.debt_principal > record.credit_cap
        || record.pending_movements > RESERVE_MAX_PENDING_MOVEMENTS_V1
        || record.open_appeals > RESERVE_MAX_OPEN_APPEALS_V1
        || record.rent_charged_through_unix == 0
        || record.interest_accrued_at_unix == 0
        || record.updated_at_unix == 0
        || record.rent_charged_through_unix > record.updated_at_unix
        || record.interest_accrued_at_unix > record.updated_at_unix
    {
        return Err(response_error(kind, "provider record is inconsistent"));
    }
    Ok(())
}
fn validate_movement_record(record: &ReserveMovementRecordV1, kind: &str) -> Result<()> {
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.movement_id == [0; 32]
        || record.amount.is_zero()
        || record.expected_provider_revision == 0
        || record.policy_digest == [0; 32]
        || record.requested_at_unix == 0
        || match record.status {
            ReserveMovementStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveMovementStatusV1::Approved | ReserveMovementStatusV1::Rejected => {
                !terminal_fields
            }
        }
    {
        return Err(response_error(kind, "movement record is inconsistent"));
    }
    Ok(())
}
fn validate_appeal_record(record: &ReserveAppealRecordV1, kind: &str) -> Result<()> {
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.appeal_id == [0; 32]
        || record.reason.is_empty()
        || record.reason.len() > RESERVE_MAX_REASON_BYTES_V1
        || record.expected_provider_revision == 0
        || record.submitted_at_unix == 0
        || match record.status {
            ReserveAppealStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveAppealStatusV1::Accepted | ReserveAppealStatusV1::Rejected => !terminal_fields,
        }
    {
        return Err(response_error(kind, "appeal record is inconsistent"));
    }
    Ok(())
}
fn validate_event_record(record: &ReserveFinalizedEventV1, kind: &str) -> Result<()> {
    let event = &record.event;
    if event.occurred_at_unix_ms == 0
        || event.policy_digest == [0; 32]
        || event
            .provider_id
            .is_some_and(|provider_id| provider_id.0 == [0; 32])
        || event.operation_id == Some([0; 32])
    {
        return Err(response_error(
            kind,
            "event payload metadata is inconsistent",
        ));
    }
    let shape_is_valid = match event.kind {
        SorafsReserveLedgerEventKind::PolicyActivated => {
            event.provider_id.is_none()
                && event.operation_id.is_none()
                && event.provider_revision == 0
                && event.resulting_lifecycle_stage.is_none()
        }
        SorafsReserveLedgerEventKind::ProviderRegistered => {
            event.provider_id.is_some()
                && event.operation_id.is_none()
                && event.provider_revision == 1
                && event.resulting_lifecycle_stage.is_some()
        }
        SorafsReserveLedgerEventKind::MovementRequested
        | SorafsReserveLedgerEventKind::MovementApproved
        | SorafsReserveLedgerEventKind::MovementRejected
        | SorafsReserveLedgerEventKind::AppealSubmitted
        | SorafsReserveLedgerEventKind::AppealAccepted
        | SorafsReserveLedgerEventKind::AppealRejected => {
            event.provider_id.is_some()
                && event.operation_id.is_some()
                && event.provider_revision > 0
                && event.resulting_lifecycle_stage.is_some()
        }
        SorafsReserveLedgerEventKind::RentCharged
        | SorafsReserveLedgerEventKind::LifecycleAdvanced
        | SorafsReserveLedgerEventKind::CreditDrawn
        | SorafsReserveLedgerEventKind::CreditRepaid => {
            event.provider_id.is_some()
                && event.operation_id.is_none()
                && event.provider_revision > 0
                && event.resulting_lifecycle_stage.is_some()
        }
    };
    if !shape_is_valid {
        return Err(response_error(kind, "event payload shape is inconsistent"));
    }
    Ok(())
}
pub(super) fn validate_id_page<T>(
    records: &[T],
    mut previous: Option<[u8; 32]>,
    id: impl Fn(&T) -> [u8; 32],
    has_more: bool,
    next_after: Option<[u8; 32]>,
    kind: &str,
    order_detail: &str,
    error: fn(&str, &str) -> eyre::Report,
) -> Result<()> {
    for record in records {
        let current = id(record);
        if current == [0; 32] || previous.is_some_and(|previous| current <= previous) {
            return Err(error(kind, order_detail));
        }
        previous = Some(current);
    }
    if has_more != next_after.is_some()
        || next_after.is_some_and(|next| records.last().map(&id) != Some(next))
    {
        return Err(error(kind, "continuation cursor is inconsistent"));
    }
    Ok(())
}
/// Require exact JSON and an untransformed response body for finalized reads.
pub(super) fn finalized_json_request(builder: DefaultRequestBuilder) -> DefaultRequestBuilder {
    builder
        .header("Accept", APPLICATION_JSON)
        .header("Accept-Encoding", "identity")
        .max_response_bytes(RESERVE_JSON_RESPONSE_MAX_BYTES_V1)
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
    let policy: ReserveAuthorityPolicyRecordV1 =
        exact_typed_payload(payload, "policy", "payload is not the typed policy DTO")?;
    validate_policy_record(&policy, "policy")?;
    validate_finalized_cursor(cursor, expected, "policy")?;
    Ok(response)
}
macro_rules! define_id_page_response_validator {
    (
        $(#[$meta:meta])*
        $name:ident, $filter:ty, $page:ty, $records:ident, $after:ident, $kind:literal,
        $record_validator:path, $id:expr, $next:expr
    ) => {
        $(#[$meta])*
        pub(super) fn $name(
            response: Response<Vec<u8>>,
            filter: &$filter,
        ) -> Result<Response<Vec<u8>>> {
            if response.status() != StatusCode::OK {
                return Ok(response);
            }
            let value = exact_json_object(
                &response,
                $kind,
                &["finalized_cursor", stringify!($records), "has_more", "next_after"],
            )?;
            let page: $page = exact_typed_payload(
                value,
                $kind,
                "body is not the typed page DTO",
            )?;
            validate_encoded_page(&page, $kind)?;
            validate_finalized_cursor(page.finalized_cursor, &filter.finalized, $kind)?;
            if page.$records.len() > validate_limit(filter.limit, $kind)? {
                return Err(response_error($kind, "payload exceeds the requested limit"));
            }
            for record in &page.$records {
                $record_validator(record, $kind)?;
            }
            let after = filter.$after.map(|value| parse_request_hash(value, $kind)).transpose()?;
            validate_id_page(
                &page.$records,
                after,
                $id,
                page.has_more,
                ($next)(&page),
                $kind,
                "identifiers are not strictly ordered",
                response_error,
            )?;
            Ok(response)
        }
    };
}
define_id_page_response_validator!(
    /// Validate a successful finalized reserve-provider page without rewriting it.
    validate_providers_response,
    SorafsReserveProvidersReadbackFilter<'_>,
    ReserveProviderAccountPageV1,
    accounts,
    after_provider_id_hex,
    "provider page",
    validate_provider_record,
    |account: &ReserveProviderAccountV1| account.terms.provider_id.0,
    |page: &ReserveProviderAccountPageV1| page.next_after.map(|provider| provider.0)
);
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
    let provider: ReserveProviderAccountV1 =
        exact_typed_payload(payload, "provider", "payload is not the typed provider DTO")?;
    validate_provider_record(&provider, "provider")?;
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
define_id_page_response_validator!(
    /// Validate a successful finalized reserve-movement page without rewriting it.
    validate_movements_response,
    SorafsReserveMovementReadbackFilter<'_>,
    ReserveMovementPageV1,
    movements,
    after_movement_id_hex,
    "movement page",
    validate_movement_record,
    |movement: &ReserveMovementRecordV1| movement.movement_id,
    |page: &ReserveMovementPageV1| page.next_after
);
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
    let movement: ReserveMovementRecordV1 =
        exact_typed_payload(payload, "movement", "payload is not the typed movement DTO")?;
    validate_movement_record(&movement, "movement")?;
    validate_finalized_cursor(cursor, expected, "movement")?;
    if movement.movement_id != parse_request_hash(movement_id_hex, "movement")? {
        return Err(response_error(
            "movement",
            "identifier does not match the request",
        ));
    }
    Ok(response)
}
define_id_page_response_validator!(
    /// Validate a successful finalized reserve-appeal page without rewriting it.
    validate_appeals_response,
    SorafsReserveAppealReadbackFilter<'_>,
    ReserveAppealPageV1,
    appeals,
    after_appeal_id_hex,
    "appeal page",
    validate_appeal_record,
    |appeal: &ReserveAppealRecordV1| appeal.appeal_id,
    |page: &ReserveAppealPageV1| page.next_after
);
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
    let appeal: ReserveAppealRecordV1 =
        exact_typed_payload(payload, "appeal", "payload is not the typed appeal DTO")?;
    validate_appeal_record(&appeal, "appeal")?;
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
    let page: ReserveFinalizedEventPageV1 =
        exact_typed_payload(value, "event page", "body is not the typed page DTO")?;
    validate_encoded_page(&page, "event page")?;
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
    if after.is_none() {
        match page.events.first() {
            Some(event) if event.event.kind == SorafsReserveLedgerEventKind::PolicyActivated => {}
            Some(_) => {
                return Err(response_error(
                    "event page",
                    "initial event is not the policy activation",
                ));
            }
            None => {
                return Err(response_error("event page", "initial event page is empty"));
            }
        }
    }
    for event in &page.events {
        validate_event_record(event, "event page")?;
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
        SorafsReserveCommandRoute::CreditDraw => {
            super::repair::instruction_is!(instruction, DrawSorafsReserveCredit)
        }
        SorafsReserveCommandRoute::CreditRepay => {
            super::repair::instruction_is!(instruction, RepaySorafsReserveCredit)
        }
        SorafsReserveCommandRoute::Appeal => {
            super::repair::instruction_is!(instruction, SubmitSorafsReserveAppeal)
        }
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
    use super::*;
    use crate::{
        client::evidence_http_tests::{
            SnapshotStore, assert_single_accept_header, base_url, client_with_base_url,
            empty_response, respond_with, with_mock_http,
        },
        http::StatusCode,
    };
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
    use std::{num::NonZeroU64, sync::Arc, time::Duration};
    const EXACT_RESERVE_TTL: Duration = Duration::from_secs(300);
    const MOVEMENT_ID: [u8; 32] = [0x61; 32];
    const APPEAL_ID: [u8; 32] = [0x62; 32];
    fn finalized_cursor() -> ReserveFinalizedCursorV1 {
        ReserveFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        }
    }
    fn ordered_id(index: u32) -> [u8; 32] {
        let mut id = [0; 32];
        id[28..].copy_from_slice(&index.to_be_bytes());
        id
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
            custody_account: iroha_data_model::account::AccountId::new(
                iroha_crypto::derive_non_signing_ed25519_public_key(
                    b"iroha:sorafs:client-reserve-custody-test:v1",
                    &[],
                ),
            ),
            treasury_account: client.account.clone(),
            operations_authority: client.account.clone(),
            decision_authority: client.account.clone(),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: "1".parse().expect("debt cap"),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        };
        policy.validate().expect("valid reserve policy fixture");
        let policy_digest = policy.digest().expect("digest reserve policy fixture");
        ReserveAuthorityPolicyRecordV1 {
            policy,
            policy_digest,
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
        let is_policy_activation = sequence == 1;
        ReserveFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            event: SorafsReserveLedgerEvent {
                kind: if is_policy_activation {
                    SorafsReserveLedgerEventKind::PolicyActivated
                } else {
                    SorafsReserveLedgerEventKind::LifecycleAdvanced
                },
                provider_id: (!is_policy_activation).then(|| ProviderId::new([0x63; 32])),
                operation_id: None,
                policy_digest: [0x64; 32],
                provider_revision: if is_policy_activation { 0 } else { sequence },
                resulting_lifecycle_stage: (!is_policy_activation)
                    .then_some(ReserveLifecycleStage::Active),
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
            norito::json::to_value(&finalized_cursor()).expect("encode finalized cursor"),
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
    fn assert_rejected<T>(result: Result<T>) {
        assert!(result.is_err());
    }
    fn event_page(events: Vec<ReserveFinalizedEventV1>) -> ReserveFinalizedEventPageV1 {
        ReserveFinalizedEventPageV1 {
            finalized_cursor: finalized_cursor(),
            events,
            has_more: false,
            next_after: None,
        }
    }
    fn assert_movements_rejected(
        page: &ReserveMovementPageV1,
        filter: &SorafsReserveMovementReadbackFilter<'_>,
    ) {
        assert_rejected(validate_movements_response(page_response(page), filter));
    }
    fn assert_events_rejected(
        page: &ReserveFinalizedEventPageV1,
        filter: &SorafsReserveEventsReadbackFilter<'_>,
    ) {
        assert_rejected(validate_events_response(page_response(page), filter));
    }
    fn assert_exact_read_request(snapshot: &crate::http_default::RequestSnapshot, path: &str) {
        assert_eq!(snapshot.method, crate::http::Method::GET);
        assert_eq!(snapshot.url.path(), path);
        assert_single_accept_header(snapshot, APPLICATION_JSON);
        let encodings: Vec<_> = snapshot
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("accept-encoding"))
            .collect();
        assert_eq!(encodings.len(), 1);
        assert_eq!(encodings[0].1, "identity");
        assert_eq!(
            snapshot.max_response_bytes,
            RESERVE_JSON_RESPONSE_MAX_BYTES_V1
        );
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
        super::super::repair::route_test_support::assert_rejected_before_http(
            format!(
                "SoraFS reserve route requires exactly one `{}` native instruction",
                route.expected_instruction_label()
            ),
            || client.post_sorafs_reserve_transaction(route, transaction),
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
        let initial_events = vec![
            reserve_event(&client, 1, 5, [0x51; 32], 0),
            reserve_event(&client, 2, 5, [0x51; 32], 1),
        ];
        validate_events_response(
            page_response(&ReserveFinalizedEventPageV1 {
                finalized_cursor: finalized_cursor(),
                next_after: initial_events.last().map(ReserveFinalizedEventV1::cursor),
                events: initial_events,
                has_more: true,
            }),
            &SorafsReserveEventsReadbackFilter {
                limit: Some(2),
                ..SorafsReserveEventsReadbackFilter::default()
            },
        )
        .expect("exact initial policy-activation page");
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
    fn reserve_page_response_binding_separates_json_transport_and_norito_bounds() {
        let client = client_with_base_url(base_url());
        let escaped_reason = "\u{0001}".repeat(RESERVE_MAX_REASON_BYTES_V1);
        let escaped_page = ReserveAppealPageV1 {
            finalized_cursor: finalized_cursor(),
            appeals: (1..=RESERVE_DEFAULT_PAGE_LIMIT_V1)
                .map(|index| {
                    let mut appeal = appeal_record(&client, ordered_id(index));
                    appeal.reason = escaped_reason.clone();
                    appeal
                })
                .collect(),
            has_more: false,
            next_after: None,
        };
        let response = page_response(&escaped_page);
        assert!(response.body().len() > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1);
        assert!(response.body().len() <= RESERVE_JSON_RESPONSE_MAX_BYTES_V1);
        assert!(
            norito::to_bytes(&escaped_page)
                .expect("encode escaped appeal page as Norito")
                .len()
                <= RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1
        );
        validate_appeals_response(response, &SorafsReserveAppealReadbackFilter::default())
            .expect("large escaped JSON with bounded canonical Norito is valid");
        let movements = (1..=RESERVE_DEFAULT_PAGE_LIMIT_V1)
            .map(|index| {
                let mut movement = movement_record(&client, ordered_id(index));
                movement.status = ReserveMovementStatusV1::Approved;
                movement.decided_by = Some(client.account.clone());
                movement.decided_at_unix = Some(2);
                movement.rationale = Some("r".repeat(12 * 1024));
                movement
            })
            .collect();
        let oversized_page = ReserveMovementPageV1 {
            finalized_cursor: finalized_cursor(),
            movements,
            has_more: false,
            next_after: None,
        };
        let response = page_response(&oversized_page);
        assert!(response.body().len() <= RESERVE_JSON_RESPONSE_MAX_BYTES_V1);
        assert!(
            norito::to_bytes(&oversized_page)
                .expect("encode oversized movement page as Norito")
                .len()
                > RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1
        );
        let error =
            validate_movements_response(response, &SorafsReserveMovementReadbackFilter::default())
                .expect_err("oversized canonical Norito page must be rejected");
        assert!(error.to_string().contains("canonical Norito page exceeds"));
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
        let mut duplicate_media = record_response("policy", &policy);
        duplicate_media.headers_mut().append(
            "content-type",
            APPLICATION_JSON.parse().expect("media type"),
        );
        let mut identity_encoded = record_response("policy", &policy);
        identity_encoded.headers_mut().insert(
            "content-encoding",
            "identity".parse().expect("identity encoding"),
        );
        validate_policy_response(identity_encoded, &SorafsReserveFinalizedAnchor::default())
            .expect("exact identity encoding is allowed");
        let mut compressed = record_response("policy", &policy);
        compressed
            .headers_mut()
            .insert("content-encoding", "gzip".parse().expect("gzip encoding"));
        for response in [wrong_media, duplicate_media, compressed] {
            assert_rejected(validate_policy_response(
                response,
                &SorafsReserveFinalizedAnchor::default(),
            ));
        }
        let mut wrapper = record_response("policy", &policy);
        let mut value: Value =
            norito::json::from_slice(wrapper.body()).expect("decode record wrapper");
        value
            .as_object_mut()
            .expect("record wrapper object")
            .insert("extra".to_owned(), Value::from(true));
        *wrapper.body_mut() = norito::json::to_vec(&value).expect("encode invalid wrapper");
        let mut nested = record_response("policy", &policy);
        let mut value: Value =
            norito::json::from_slice(nested.body()).expect("decode record wrapper");
        value
            .as_object_mut()
            .expect("record wrapper object")
            .get_mut("policy")
            .expect("policy payload")
            .as_object_mut()
            .expect("policy record object")
            .insert("unexpected".to_owned(), Value::from(true));
        *nested.body_mut() = norito::json::to_vec(&value).expect("encode nested mutant");
        for response in [wrapper, nested] {
            assert_rejected(validate_policy_response(
                response,
                &SorafsReserveFinalizedAnchor::default(),
            ));
        }
        let oversized = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", APPLICATION_JSON)
            .body(vec![b' '; RESERVE_JSON_RESPONSE_MAX_BYTES_V1 + 1])
            .expect("build oversized response");
        let error = validate_policy_response(oversized, &SorafsReserveFinalizedAnchor::default())
            .expect_err("oversized response must fail before JSON decoding");
        assert!(
            error
                .to_string()
                .contains("body exceeds the JSON transport bound")
        );
        let wrong_hash = hex::encode([0x72; 32]);
        assert_rejected(validate_policy_response(
            record_response("policy", &policy),
            &finalized_anchor(&wrong_hash),
        ));
        for response in [
            validate_provider_response(
                record_response("provider", &provider_account(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            ),
            validate_movement_response(
                record_response("movement", &movement_record(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            ),
            validate_appeal_response(
                record_response("appeal", &appeal_record(&client, [0x20; 32])),
                &hex::encode([0x21; 32]),
                &SorafsReserveFinalizedAnchor::default(),
            ),
        ] {
            assert_rejected(response);
        }
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn reserve_read_response_binding_rejects_typed_semantic_mutants() {
        let client = client_with_base_url(base_url());
        let finalized = SorafsReserveFinalizedAnchor::default();
        for mutate in [
            |record: &mut ReserveAuthorityPolicyRecordV1| record.policy_digest = [0x99; 32],
            |record: &mut ReserveAuthorityPolicyRecordV1| record.activated_at_unix = 0,
            |record: &mut ReserveAuthorityPolicyRecordV1| {
                record.policy.custody_account = record.policy.treasury_account.clone();
            },
        ] {
            let mut policy = policy_record(&client);
            mutate(&mut policy);
            assert_rejected(validate_policy_response(
                record_response("policy", &policy),
                &finalized,
            ));
        }
        let mut provider = provider_account(&client, [0x20; 32]);
        provider.terms.capacity_gib = 0;
        assert_rejected(validate_provider_response(
            record_response("provider", &provider),
            &hex::encode(provider.terms.provider_id.0),
            &finalized,
        ));
        for mutate in [
            |record: &mut ReserveProviderAccountV1| {
                record.debt_principal = "2".parse().expect("debt principal");
            },
            |record: &mut ReserveProviderAccountV1| {
                record.rent_charged_through_unix = record.updated_at_unix + 1;
            },
            |record: &mut ReserveProviderAccountV1| record.revision = 0,
            |record: &mut ReserveProviderAccountV1| record.policy_digest = [0; 32],
            |record: &mut ReserveProviderAccountV1| {
                record.pending_movements = RESERVE_MAX_PENDING_MOVEMENTS_V1 + 1;
            },
            |record: &mut ReserveProviderAccountV1| {
                record.open_appeals = RESERVE_MAX_OPEN_APPEALS_V1 + 1;
            },
            |record: &mut ReserveProviderAccountV1| record.updated_at_unix = 0,
            |record: &mut ReserveProviderAccountV1| {
                record.interest_accrued_at_unix = record.updated_at_unix + 1;
            },
        ] {
            let mut provider = provider_account(&client, [0x20; 32]);
            mutate(&mut provider);
            assert_rejected(validate_provider_record(&provider, "provider"));
        }
        let mut movement = movement_record(&client, [0x20; 32]);
        movement.amount = "0".parse().expect("zero movement amount");
        assert_rejected(validate_movement_response(
            record_response("movement", &movement),
            &hex::encode(movement.movement_id),
            &finalized,
        ));
        let mut movement = movement_record(&client, [0x20; 32]);
        movement.decided_by = Some(client.account.clone());
        assert_rejected(validate_movement_record(&movement, "movement"));
        let mut movement = movement_record(&client, [0x20; 32]);
        movement.status = ReserveMovementStatusV1::Approved;
        assert_rejected(validate_movement_record(&movement, "movement"));
        for mutate in [
            |record: &mut ReserveMovementRecordV1| record.expected_provider_revision = 0,
            |record: &mut ReserveMovementRecordV1| record.policy_digest = [0; 32],
            |record: &mut ReserveMovementRecordV1| record.requested_at_unix = 0,
        ] {
            let mut movement = movement_record(&client, [0x20; 32]);
            mutate(&mut movement);
            assert_rejected(validate_movement_record(&movement, "movement"));
        }
        let mut appeal = appeal_record(&client, [0x20; 32]);
        appeal.reason.clear();
        assert_rejected(validate_appeal_response(
            record_response("appeal", &appeal),
            &hex::encode(appeal.appeal_id),
            &finalized,
        ));
        let mut appeal = appeal_record(&client, [0x20; 32]);
        appeal.reason = "x".repeat(RESERVE_MAX_REASON_BYTES_V1 + 1);
        assert_rejected(validate_appeal_record(&appeal, "appeal"));
        let mut appeal = appeal_record(&client, [0x20; 32]);
        appeal.status = ReserveAppealStatusV1::Accepted;
        assert_rejected(validate_appeal_record(&appeal, "appeal"));
        for mutate in [
            |record: &mut ReserveAppealRecordV1| record.expected_provider_revision = 0,
            |record: &mut ReserveAppealRecordV1| record.submitted_at_unix = 0,
        ] {
            let mut appeal = appeal_record(&client, [0x20; 32]);
            mutate(&mut appeal);
            assert_rejected(validate_appeal_record(&appeal, "appeal"));
        }
        let mut event = reserve_event(&client, 1, 5, [0x51; 32], 0);
        event.event.occurred_at_unix_ms = 0;
        assert_rejected(validate_events_response(
            page_response(&event_page(vec![event])),
            &SorafsReserveEventsReadbackFilter::default(),
        ));
        for mutate in [
            |event: &mut ReserveFinalizedEventV1| event.event.operation_id = Some([0x91; 32]),
            |event: &mut ReserveFinalizedEventV1| {
                event.event.provider_id = Some(ProviderId::new([0; 32]));
            },
            |event: &mut ReserveFinalizedEventV1| event.event.policy_digest = [0; 32],
        ] {
            let mut event = reserve_event(&client, 1, 5, [0x51; 32], 0);
            mutate(&mut event);
            assert_rejected(validate_event_record(&event, "event page"));
        }
        let mut invalid_kind = reserve_event(&client, 2, 5, [0x51; 32], 1);
        invalid_kind.event.kind = SorafsReserveLedgerEventKind::MovementRequested;
        assert_rejected(validate_event_record(&invalid_kind, "event page"));
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
        assert_movements_rejected(&page, &limit_one);
        page.movements = vec![second, first.clone()];
        let limit_two = SorafsReserveMovementReadbackFilter {
            limit: Some(2),
            ..SorafsReserveMovementReadbackFilter::default()
        };
        assert_movements_rejected(&page, &limit_two);
        page.movements = vec![first];
        let after = hex::encode([0x20; 32]);
        let not_exclusive = SorafsReserveMovementReadbackFilter {
            limit: Some(2),
            after_movement_id_hex: Some(&after),
            ..SorafsReserveMovementReadbackFilter::default()
        };
        assert_movements_rejected(&page, &not_exclusive);
        page.has_more = true;
        page.next_after = Some([0x21; 32]);
        assert_movements_rejected(&page, &limit_two);
        let page = ReserveMovementPageV1 {
            finalized_cursor: finalized_cursor(),
            movements: (1..=RESERVE_DEFAULT_PAGE_LIMIT_V1 + 1)
                .map(|index| movement_record(&client, ordered_id(index)))
                .collect(),
            has_more: false,
            next_after: None,
        };
        let error = validate_movements_response(
            page_response(&page),
            &SorafsReserveMovementReadbackFilter::default(),
        )
        .expect_err("omitted movement limit must use Torii's 100-record default");
        assert!(
            error
                .to_string()
                .contains("payload exceeds the requested limit")
        );
    }
    #[test]
    fn reserve_event_response_binding_rejects_gaps_finality_and_bad_continuations() {
        let client = client_with_base_url(base_url());
        let filter = SorafsReserveEventsReadbackFilter {
            limit: Some(2),
            ..SorafsReserveEventsReadbackFilter::default()
        };
        let empty = event_page(Vec::new());
        assert_events_rejected(&empty, &filter);
        let after_hash = hex::encode([0x51; 32]);
        validate_events_response(
            page_response(&empty),
            &SorafsReserveEventsReadbackFilter {
                limit: Some(2),
                after_sequence: Some(1),
                after_block_height: Some(5),
                after_block_hash_hex: Some(&after_hash),
                after_event_index: Some(0),
                ..SorafsReserveEventsReadbackFilter::default()
            },
        )
        .expect("empty terminal continuation page is valid");
        let mut wrong_initial_kind = reserve_event(&client, 1, 5, [0x51; 32], 0);
        wrong_initial_kind.event.kind = SorafsReserveLedgerEventKind::LifecycleAdvanced;
        wrong_initial_kind.event.provider_id = Some(ProviderId::new([0x63; 32]));
        wrong_initial_kind.event.provider_revision = 1;
        wrong_initial_kind.event.resulting_lifecycle_stage = Some(ReserveLifecycleStage::Active);
        for events in [
            vec![wrong_initial_kind],
            vec![reserve_event(&client, 2, 5, [0x51; 32], 0)],
            vec![
                reserve_event(&client, 1, 5, [0x51; 32], 0),
                reserve_event(&client, 3, 5, [0x51; 32], 1),
            ],
            vec![
                reserve_event(&client, 1, 5, [0x51; 32], 0),
                reserve_event(&client, 2, 5, [0x52; 32], 1),
            ],
            vec![reserve_event(&client, 1, 8, [0x81; 32], 0)],
            vec![reserve_event(
                &client,
                1,
                finalized_cursor().height,
                [0x72; 32],
                0,
            )],
        ] {
            assert_events_rejected(&event_page(events), &filter);
        }
        let mut page = event_page(vec![reserve_event(&client, 1, 5, [0x51; 32], 0)]);
        page.has_more = true;
        page.next_after = Some(reserve_event(&client, 2, 5, [0x51; 32], 1).cursor());
        assert_events_rejected(&page, &filter);
        let page = ReserveFinalizedEventPageV1 {
            finalized_cursor: finalized_cursor(),
            events: (1_u64..=u64::from(RESERVE_DEFAULT_PAGE_LIMIT_V1 + 1))
                .map(|sequence| {
                    reserve_event(
                        &client,
                        sequence,
                        5,
                        [0x51; 32],
                        u32::try_from(sequence - 1).expect("bounded event index"),
                    )
                })
                .collect(),
            has_more: false,
            next_after: None,
        };
        let error = validate_events_response(
            page_response(&page),
            &SorafsReserveEventsReadbackFilter::default(),
        )
        .expect_err("omitted event limit must use Torii's 100-record default");
        assert!(
            error
                .to_string()
                .contains("payload exceeds the requested limit")
        );
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
    #[test]
    fn reserve_read_requests_pin_identity_json_and_transport_bound() {
        let client = client_with_base_url(base_url());
        let provider_id = hex::encode([0x21; 32]);
        let movement_id = hex::encode([0x22; 32]);
        let appeal_id = hex::encode([0x23; 32]);
        let snapshots: SnapshotStore = Arc::default();
        with_mock_http(
            respond_with(&snapshots, empty_response(StatusCode::NOT_FOUND)),
            || {
                for response in [
                    client.get_sorafs_reserve_policy(SorafsReserveFinalizedAnchor::default()),
                    client.get_sorafs_reserve_providers(
                        SorafsReserveProvidersReadbackFilter::default(),
                    ),
                    client.get_sorafs_reserve_provider(
                        &provider_id,
                        SorafsReserveFinalizedAnchor::default(),
                    ),
                    client.get_sorafs_reserve_movements(
                        SorafsReserveMovementReadbackFilter::default(),
                    ),
                    client.get_sorafs_reserve_movement(
                        &movement_id,
                        SorafsReserveFinalizedAnchor::default(),
                    ),
                    client.get_sorafs_reserve_appeals(SorafsReserveAppealReadbackFilter::default()),
                    client.get_sorafs_reserve_appeal(
                        &appeal_id,
                        SorafsReserveFinalizedAnchor::default(),
                    ),
                    client.get_sorafs_reserve_events(SorafsReserveEventsReadbackFilter::default()),
                ] {
                    response.expect("non-OK reserve read is preserved");
                }
            },
        );
        let expected_paths = [
            "/v1/sorafs/reserve/policy".to_owned(),
            "/v1/sorafs/reserve/providers".to_owned(),
            format!("/v1/sorafs/reserve/providers/{provider_id}"),
            "/v1/sorafs/reserve/movements".to_owned(),
            format!("/v1/sorafs/reserve/movements/{movement_id}"),
            "/v1/sorafs/reserve/appeals".to_owned(),
            format!("/v1/sorafs/reserve/appeals/{appeal_id}"),
            "/v1/sorafs/reserve/events".to_owned(),
        ];
        let snapshots = snapshots.lock().expect("snapshot lock");
        assert_eq!(snapshots.len(), expected_paths.len());
        for (snapshot, path) in snapshots.iter().zip(expected_paths) {
            assert_exact_read_request(snapshot, &path);
        }
    }
}
