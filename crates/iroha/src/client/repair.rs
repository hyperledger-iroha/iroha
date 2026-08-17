//! Route-to-instruction validation for native `SoraFS` repair submissions.
use super::{
    APPLICATION_JSON, Response, SorafsRepairCommandRoute, SorafsRepairEventsFilter,
    SorafsRepairFinalizedAnchor, SorafsRepairTasksFilter, StatusCode,
};
use eyre::{Result, eyre};
use iroha_data_model::{
    isi::sorafs::{
        ApplySorafsRepairTaskAction, SorafsRepairTaskActionV1, SubmitSorafsRepairAppeal,
        SubmitSorafsRepairTask,
    },
    sorafs::moderation_ledger::{
        REPAIR_QUERY_MAX_ITEMS_V1, RepairFinalizedCursorV1, RepairFinalizedEventCursorV1,
        RepairFinalizedEventPageV1, RepairFinalizedEventV1, RepairFinalizedStatusV1,
        RepairFinalizedTaskV1, RepairLedgerTaskPageV1,
    },
    transaction::{Executable, SignedTransaction},
};
use norito::json::Value;
const REPAIR_DEFAULT_PAGE_LIMIT_V1: u32 = 50;
fn response_error(kind: &str, detail: &str) -> eyre::Report {
    eyre!("invalid finalized SoraFS repair {kind} response: {detail}")
}
fn exact_json_payload(
    response: &Response<Vec<u8>>,
    kind: &'static str,
    payload_key: &'static str,
) -> Result<Value> {
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
    let wrapper = value
        .as_object()
        .ok_or_else(|| response_error(kind, "wrapper is not an object"))?;
    if wrapper.len() != 2 || !wrapper.contains_key("source") || !wrapper.contains_key(payload_key) {
        return Err(response_error(
            kind,
            "wrapper must contain exactly source and payload",
        ));
    }
    if wrapper.get("source").and_then(Value::as_str) != Some("finalized_chain") {
        return Err(response_error(kind, "source is not finalized_chain"));
    }
    wrapper
        .get(payload_key)
        .cloned()
        .ok_or_else(|| response_error(kind, "wrapper payload is missing"))
}
fn parse_request_hash(value: &str, kind: &str) -> Result<[u8; 32]> {
    let mut decoded = [0u8; 32];
    if value.len() != 64
        || hex::decode_to_slice(value, &mut decoded).is_err()
        || hex::encode(decoded) != value
        || decoded == [0; 32]
    {
        return Err(response_error(
            kind,
            "request contains a noncanonical cursor hash",
        ));
    }
    Ok(decoded)
}
fn validate_finalized_cursor(
    cursor: RepairFinalizedCursorV1,
    expected: &SorafsRepairFinalizedAnchor<'_>,
    kind: &str,
) -> Result<()> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] {
        return Err(response_error(kind, "finalized cursor is zero"));
    }
    if expected.expected_finalized_height.is_some()
        != expected.expected_finalized_block_hash_hex.is_some()
    {
        return Err(response_error(
            kind,
            "request finalized cursor is incomplete",
        ));
    }
    if expected
        .expected_finalized_height
        .is_some_and(|height| height != cursor.height)
    {
        return Err(response_error(
            kind,
            "finalized height does not match the request",
        ));
    }
    if let Some(block_hash) = expected.expected_finalized_block_hash_hex
        && parse_request_hash(block_hash, kind)? != cursor.block_hash
    {
        return Err(response_error(
            kind,
            "finalized block hash does not match the request",
        ));
    }
    Ok(())
}
fn response_limit(limit: Option<u32>, kind: &str) -> Result<usize> {
    let limit = limit.unwrap_or(REPAIR_DEFAULT_PAGE_LIMIT_V1);
    if !(1..=REPAIR_QUERY_MAX_ITEMS_V1).contains(&limit) {
        return Err(response_error(
            kind,
            "request limit is outside the protocol bound",
        ));
    }
    Ok(usize::try_from(limit).expect("bounded repair query limit fits usize"))
}
fn requested_event_cursor(
    filter: &SorafsRepairEventsFilter<'_>,
    kind: &str,
) -> Result<Option<RepairFinalizedEventCursorV1>> {
    match (
        filter.after_sequence,
        filter.after_block_height,
        filter.after_block_hash_hex,
        filter.after_event_index,
    ) {
        (None, None, None, None) => Ok(None),
        (Some(sequence), Some(block_height), Some(block_hash), Some(event_index)) => {
            if sequence == 0 || block_height == 0 {
                return Err(response_error(kind, "request event cursor is zero"));
            }
            Ok(Some(RepairFinalizedEventCursorV1 {
                sequence,
                block_height,
                block_hash: parse_request_hash(block_hash, kind)?,
                event_index,
            }))
        }
        _ => Err(response_error(kind, "request event cursor is incomplete")),
    }
}
macro_rules! define_finalized_event_validators {
    ($cursor:ty, $finalized:ty, $error:path) => {
        fn validate_event_cursor(cursor: $cursor, finalized: $finalized, kind: &str) -> Result<()> {
            if cursor.sequence == 0 || cursor.block_height == 0 || cursor.block_hash == [0; 32] {
                return Err($error(kind, "event cursor is zero"));
            }
            if cursor.block_height > finalized.height
                || (cursor.block_height == finalized.height
                    && cursor.block_hash != finalized.block_hash)
            {
                return Err($error(kind, "event cursor is outside the finalized view"));
            }
            Ok(())
        }
        fn validate_event_successor(
            previous: Option<$cursor>,
            current: $cursor,
            kind: &str,
        ) -> Result<()> {
            let Some(previous) = previous else {
                if current.sequence != 1 || current.event_index != 0 {
                    return Err($error(
                        kind,
                        "initial event is not sequence one at block index zero",
                    ));
                }
                return Ok(());
            };
            if previous.sequence.checked_add(1) != Some(current.sequence) {
                return Err($error(kind, "event sequence is not contiguous"));
            }
            match previous.block_height.cmp(&current.block_height) {
                core::cmp::Ordering::Less if current.event_index == 0 => Ok(()),
                core::cmp::Ordering::Equal
                    if previous.block_hash == current.block_hash
                        && previous.event_index.checked_add(1) == Some(current.event_index) =>
                {
                    Ok(())
                }
                _ => Err($error(
                    kind,
                    "event block height and index are not contiguous",
                )),
            }
        }
    };
}
pub(super) use define_finalized_event_validators;
define_finalized_event_validators!(
    RepairFinalizedEventCursorV1,
    RepairFinalizedCursorV1,
    response_error
);
/// Validate a successful finalized repair-status response without rewriting it.
pub(super) fn validate_status_response(
    response: Response<Vec<u8>>,
    expected: &SorafsRepairFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let status: RepairFinalizedStatusV1 =
        norito::json::from_value(exact_json_payload(&response, "status", "status")?)
            .map_err(|_| response_error("status", "payload is not the typed status DTO"))?;
    validate_finalized_cursor(status.finalized_cursor, expected, "status")?;
    Ok(response)
}
/// Validate a successful finalized repair-task page without rewriting it.
pub(super) fn validate_tasks_response(
    response: Response<Vec<u8>>,
    filter: &SorafsRepairTasksFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let page: RepairLedgerTaskPageV1 =
        norito::json::from_value(exact_json_payload(&response, "task page", "tasks")?)
            .map_err(|_| response_error("task page", "payload is not the typed task-page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "task page")?;
    if page.tasks.len() > response_limit(filter.limit, "task page")? {
        return Err(response_error(
            "task page",
            "payload exceeds the requested limit",
        ));
    }
    let previous = filter
        .after_task_id_hex
        .map(|value| parse_request_hash(value, "task page"))
        .transpose()?;
    super::reserve::validate_id_page(
        &page.tasks,
        previous,
        |task| task.task_id,
        page.has_more,
        page.next_after_task_id,
        "task page",
        "task identifiers are not strictly ordered",
        response_error,
    )?;
    Ok(response)
}
/// Validate a successful finalized repair-task response without rewriting it.
pub(super) fn validate_task_response(
    response: Response<Vec<u8>>,
    ticket_id: &str,
    expected: &SorafsRepairFinalizedAnchor<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let task: RepairFinalizedTaskV1 =
        norito::json::from_value(exact_json_payload(&response, "task", "task")?)
            .map_err(|_| response_error("task", "payload is not the typed task DTO"))?;
    validate_finalized_cursor(task.finalized_cursor, expected, "task")?;
    if task.task.ticket_id != ticket_id {
        return Err(response_error("task", "ticket does not match the request"));
    }
    Ok(response)
}
/// Validate a successful finalized repair-event page without rewriting it.
pub(super) fn validate_events_response(
    response: Response<Vec<u8>>,
    filter: &SorafsRepairEventsFilter<'_>,
) -> Result<Response<Vec<u8>>> {
    if response.status() != StatusCode::OK {
        return Ok(response);
    }
    let page: RepairFinalizedEventPageV1 =
        norito::json::from_value(exact_json_payload(&response, "event page", "events")?)
            .map_err(|_| response_error("event page", "payload is not the typed event-page DTO"))?;
    validate_finalized_cursor(page.finalized_cursor, &filter.finalized, "event page")?;
    if page.events.len() > response_limit(filter.limit, "event page")? {
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
            page.events.last().map(RepairFinalizedEventV1::cursor) != Some(next)
        })
    {
        return Err(response_error(
            "event page",
            "continuation cursor is inconsistent",
        ));
    }
    Ok(response)
}
impl SorafsRepairCommandRoute {
    pub(super) const fn expected_instruction_label(self) -> &'static str {
        match self {
            Self::Report => "SubmitSorafsRepairTask",
            Self::Slash => "ApplySorafsRepairTaskAction::Escalate",
            Self::Claim => "ApplySorafsRepairTaskAction::Claim",
            Self::Heartbeat => "ApplySorafsRepairTaskAction::Renew",
            Self::Complete => "ApplySorafsRepairTaskAction::Complete",
            Self::Fail => "ApplySorafsRepairTaskAction::Fail",
            Self::Appeal => "SubmitSorafsRepairAppeal",
        }
    }
}
fn route_mismatch(route: SorafsRepairCommandRoute) -> eyre::Report {
    eyre!(
        "SoraFS repair route requires exactly one `{}` native instruction",
        route.expected_instruction_label()
    )
}
macro_rules! instruction_is {
    ($instruction:expr, $type:ty) => {
        $instruction.as_any().is::<$type>()
    };
    ($instruction:expr, $route:expr, { $($variant:path => $type:ty),+ $(,)? }) => {
        match $route {
            $($variant => $instruction.as_any().is::<$type>(),)+
        }
    };
}
pub(super) use instruction_is;
/// Reject a transaction unless it contains the one native instruction selected by `route`.
pub(super) fn validate_transaction_route(
    route: SorafsRepairCommandRoute,
    transaction: &SignedTransaction,
) -> Result<()> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(route_mismatch(route));
    };
    let [instruction] = instructions.as_ref() else {
        return Err(route_mismatch(route));
    };
    let matches_route = match route {
        SorafsRepairCommandRoute::Report => instruction_is!(instruction, SubmitSorafsRepairTask),
        SorafsRepairCommandRoute::Appeal => instruction_is!(instruction, SubmitSorafsRepairAppeal),
        SorafsRepairCommandRoute::Slash
        | SorafsRepairCommandRoute::Claim
        | SorafsRepairCommandRoute::Heartbeat
        | SorafsRepairCommandRoute::Complete
        | SorafsRepairCommandRoute::Fail => instruction
            .as_any()
            .downcast_ref::<ApplySorafsRepairTaskAction>()
            .is_some_and(|apply| {
                matches!(
                    (route, &apply.action),
                    (
                        SorafsRepairCommandRoute::Slash,
                        SorafsRepairTaskActionV1::Escalate(_)
                    ) | (
                        SorafsRepairCommandRoute::Claim,
                        SorafsRepairTaskActionV1::Claim(_)
                    ) | (
                        SorafsRepairCommandRoute::Heartbeat,
                        SorafsRepairTaskActionV1::Renew(_)
                    ) | (
                        SorafsRepairCommandRoute::Complete,
                        SorafsRepairTaskActionV1::Complete(_)
                    ) | (
                        SorafsRepairCommandRoute::Fail,
                        SorafsRepairTaskActionV1::Fail(_)
                    )
                )
            }),
    };
    if !matches_route {
        return Err(route_mismatch(route));
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod route_test_support {
    use super::super::evidence_http_tests::*;
    use crate::http::StatusCode;
    use std::sync::Arc;
    pub(in crate::client) fn assert_rejected_before_http<T: std::fmt::Debug>(
        expected_error: String,
        request: impl FnOnce() -> eyre::Result<T>,
    ) {
        let snapshots: SnapshotStore = Arc::default();
        let error = with_mock_http(
            respond_with(&snapshots, empty_response(StatusCode::ACCEPTED)),
            || request().expect_err("invalid route transaction must fail locally"),
        );
        assert_eq!(error.to_string(), expected_error);
        assert!(
            snapshots.lock().expect("snapshot lock").is_empty(),
            "route validation must run before capability lookup or command HTTP"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::evidence_http_tests::{
            SnapshotStore, base_url, client_with_base_url, json_response, respond_with,
            with_mock_http,
        },
        http::StatusCode,
    };
    use iroha_data_model::{
        Level,
        events::data::sorafs::{SorafsRepairLedgerEvent, SorafsRepairLedgerEventKind},
        isi::{
            InstructionBox, Log,
            sorafs::{
                ApplySorafsRepairTaskAction, SorafsRepairClaimV1, SorafsRepairCompleteV1,
                SorafsRepairEscalateV1, SorafsRepairFailV1, SorafsRepairRenewV1,
                SorafsRepairTaskActionV1, SubmitSorafsRepairAppeal, SubmitSorafsRepairTask,
            },
        },
        metadata::Metadata,
        sorafs::{
            capacity::ProviderId,
            moderation_ledger::{
                REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedEventV1, RepairLedgerStatusV1,
                RepairLedgerTaskV1,
            },
            pin_registry::ManifestDigest,
        },
        transaction::{Executable, FeePaymentIntent, IvmBytecode, SignedTransaction},
    };
    use std::{num::NonZeroU64, sync::Arc};
    fn sign_executable(client: &super::super::Client, executable: Executable) -> SignedTransaction {
        let gas_limit = executable
            .requires_transaction_gas_limit()
            .then(|| NonZeroU64::new(1).expect("non-zero gas limit"));
        client.build_transaction(
            executable,
            FeePaymentIntent::authority(Vec::new(), gas_limit),
            Metadata::default(),
        )
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
    fn action_instruction(action: SorafsRepairTaskActionV1) -> ApplySorafsRepairTaskAction {
        ApplySorafsRepairTaskAction::new("REP-1".to_owned(), 1, action)
    }
    fn finalized_cursor() -> RepairFinalizedCursorV1 {
        RepairFinalizedCursorV1 {
            height: 7,
            block_hash: [0x71; 32],
        }
    }
    fn repair_task(
        client: &super::super::Client,
        ticket_id: &str,
        task_id: [u8; 32],
    ) -> RepairLedgerTaskV1 {
        RepairLedgerTaskV1 {
            version: REPAIR_LEDGER_TASK_VERSION_V1,
            task_id,
            source_identity: [0x41; 32],
            ticket_id: ticket_id.to_owned(),
            canonical_report: vec![0x42],
            manifest_digest: [0x43; 32],
            provider_id: [0x44; 32],
            submitted_by: client.account.clone(),
            submitted_at_unix_ms: 1,
            revision: 1,
            lease: None,
            terminal_outcome: None,
            slash: None,
            appeal: None,
            action_receipts: Vec::new(),
            updated_at_unix_ms: 1,
        }
    }
    fn repair_event(
        client: &super::super::Client,
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
    ) -> RepairFinalizedEventV1 {
        RepairFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            event: SorafsRepairLedgerEvent {
                kind: SorafsRepairLedgerEventKind::TaskSubmitted,
                ticket_id: "REP-1".to_owned(),
                task_id: [0x45; 32],
                provider_id: ProviderId::new([0x46; 32]),
                manifest_digest: ManifestDigest::new([0x47; 32]),
                revision: sequence,
                authority: client.account.clone(),
                occurred_at_unix_ms: sequence,
            },
        }
    }
    fn wrapped_response<T: norito::json::JsonSerialize>(
        source: &str,
        key: &str,
        payload: &T,
        content_type: &str,
    ) -> Response<Vec<u8>> {
        let mut wrapper = norito::json::Map::new();
        wrapper.insert("source".to_owned(), Value::from(source));
        wrapper.insert(
            key.to_owned(),
            norito::json::to_value(payload).expect("encode typed repair response"),
        );
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", content_type)
            .body(
                norito::json::to_vec(&Value::Object(wrapper))
                    .expect("encode repair response wrapper"),
            )
            .expect("build repair response")
    }
    fn exact_response<T: norito::json::JsonSerialize>(key: &str, payload: &T) -> Response<Vec<u8>> {
        wrapped_response("finalized_chain", key, payload, APPLICATION_JSON)
    }
    fn non_ok_response() -> Response<Vec<u8>> {
        Response::builder()
            .status(StatusCode::CONFLICT)
            .header("content-type", "application/problem+json")
            .header("x-repair-proof", "opaque")
            .body(vec![0x00, 0xFF, 0x51, 0x00])
            .expect("build non-OK response")
    }
    fn assert_non_ok_preserved(response: Response<Vec<u8>>) {
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
        assert_eq!(response.headers()["x-repair-proof"], "opaque");
        assert_eq!(response.body(), &[0x00, 0xFF, 0x51, 0x00]);
    }
    fn assert_rejected<T>(result: Result<T>) {
        assert!(result.is_err());
    }
    fn assert_tasks_rejected(page: &RepairLedgerTaskPageV1, filter: &SorafsRepairTasksFilter<'_>) {
        assert_rejected(validate_tasks_response(
            exact_response("tasks", page),
            filter,
        ));
    }
    fn assert_events_rejected(
        page: &RepairFinalizedEventPageV1,
        filter: &SorafsRepairEventsFilter<'_>,
    ) {
        assert_rejected(validate_events_response(
            exact_response("events", page),
            filter,
        ));
    }
    fn exact_route_instructions() -> [(SorafsRepairCommandRoute, InstructionBox); 7] {
        [
            (
                SorafsRepairCommandRoute::Report,
                SubmitSorafsRepairTask::new([0x51; 32], vec![0x01]).into(),
            ),
            (
                SorafsRepairCommandRoute::Slash,
                action_instruction(SorafsRepairTaskActionV1::Escalate(SorafsRepairEscalateV1 {
                    lease_generation: 1,
                    slash_proposal_payload: vec![0x02],
                    idempotency_key: "escalate-1".to_owned(),
                }))
                .into(),
            ),
            (
                SorafsRepairCommandRoute::Claim,
                action_instruction(SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                    lease_duration_ms: 1,
                    idempotency_key: "claim-1".to_owned(),
                }))
                .into(),
            ),
            (
                SorafsRepairCommandRoute::Heartbeat,
                action_instruction(SorafsRepairTaskActionV1::Renew(SorafsRepairRenewV1 {
                    lease_generation: 1,
                    lease_duration_ms: 1,
                    idempotency_key: "renew-1".to_owned(),
                }))
                .into(),
            ),
            (
                SorafsRepairCommandRoute::Complete,
                action_instruction(SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                    lease_generation: 1,
                    evidence_digest: [0x52; 32],
                    idempotency_key: "complete-1".to_owned(),
                }))
                .into(),
            ),
            (
                SorafsRepairCommandRoute::Fail,
                action_instruction(SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
                    lease_generation: 1,
                    failure_digest: [0x53; 32],
                    idempotency_key: "fail-1".to_owned(),
                }))
                .into(),
            ),
            (
                SorafsRepairCommandRoute::Appeal,
                SubmitSorafsRepairAppeal::new(
                    "REP-1".to_owned(),
                    1,
                    [0x54; 32],
                    "appeal".to_owned(),
                    "appeal-1".to_owned(),
                )
                .into(),
            ),
        ]
    }
    fn assert_rejected_before_http(
        client: &super::super::Client,
        route: SorafsRepairCommandRoute,
        transaction: &SignedTransaction,
    ) {
        super::route_test_support::assert_rejected_before_http(
            format!(
                "SoraFS repair route requires exactly one `{}` native instruction",
                route.expected_instruction_label()
            ),
            || client.post_sorafs_repair_transaction(route, transaction),
        );
    }
    fn assert_repair_route_contract() {
        let client = client_with_base_url(base_url());
        for (route, instruction) in exact_route_instructions() {
            let transaction = sign_instruction(&client, instruction);
            validate_transaction_route(route, &transaction).expect("matching repair route");
        }

        let report = sign_instruction(&client, SubmitSorafsRepairTask::new([0x51; 32], vec![0x01]));
        assert_rejected_before_http(&client, SorafsRepairCommandRoute::Appeal, &report);
        let claim = sign_instruction(
            &client,
            action_instruction(SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                lease_duration_ms: 1,
                idempotency_key: "claim-1".to_owned(),
            })),
        );
        assert_rejected_before_http(&client, SorafsRepairCommandRoute::Heartbeat, &claim);

        let report: InstructionBox = SubmitSorafsRepairTask::new([0x51; 32], vec![0x01]).into();
        for transaction in [
            sign_instruction(
                &client,
                Log::new(Level::INFO, "not a repair instruction".into()),
            ),
            sign_executable(
                &client,
                Executable::Instructions(vec![report.clone(), report].into()),
            ),
            sign_executable(
                &client,
                Executable::Ivm(IvmBytecode::from_compiled(vec![0x00])),
            ),
        ] {
            assert_rejected_before_http(&client, SorafsRepairCommandRoute::Report, &transaction);
        }
    }

    fn assert_repair_read_success_contract() {
        let client = client_with_base_url(base_url());
        let cursor = finalized_cursor();
        let hash = hex::encode(cursor.block_hash);
        let finalized = SorafsRepairFinalizedAnchor {
            expected_finalized_height: Some(cursor.height),
            expected_finalized_block_hash_hex: Some(&hash),
        };
        let status = RepairFinalizedStatusV1 {
            finalized_cursor: cursor,
            status: RepairLedgerStatusV1::default(),
        };
        validate_status_response(exact_response("status", &status), &finalized)
            .expect("exact status wrapper");

        let task = repair_task(&client, "REP-1", [0x20; 32]);
        validate_task_response(
            exact_response(
                "task",
                &RepairFinalizedTaskV1 {
                    finalized_cursor: cursor,
                    task: task.clone(),
                },
            ),
            "REP-1",
            &finalized,
        )
        .expect("exact task wrapper");
        validate_tasks_response(
            exact_response(
                "tasks",
                &RepairLedgerTaskPageV1 {
                    finalized_cursor: cursor,
                    tasks: vec![task, repair_task(&client, "REP-2", [0x30; 32])],
                    has_more: false,
                    next_after_task_id: None,
                },
            ),
            &SorafsRepairTasksFilter {
                finalized,
                limit: Some(2),
                after_task_id_hex: None,
            },
        )
        .expect("exact task page");

        let events = vec![
            repair_event(&client, 1, 5, [0x51; 32], 0),
            repair_event(&client, 2, 5, [0x51; 32], 1),
        ];
        validate_events_response(
            exact_response(
                "events",
                &RepairFinalizedEventPageV1 {
                    finalized_cursor: cursor,
                    events,
                    has_more: false,
                    next_after: None,
                },
            ),
            &SorafsRepairEventsFilter {
                finalized,
                limit: Some(2),
                ..SorafsRepairEventsFilter::default()
            },
        )
        .expect("exact event page");
    }

    fn assert_repair_rejection_contract() {
        let client = client_with_base_url(base_url());
        let cursor = finalized_cursor();
        let status = RepairFinalizedStatusV1 {
            finalized_cursor: cursor,
            status: RepairLedgerStatusV1::default(),
        };
        for response in [
            wrapped_response(
                "finalized_chain",
                "status",
                &status,
                "application/json; charset=utf-8",
            ),
            json_response(
                StatusCode::OK,
                r#"{"source":"finalized_chain","status":{},"extra":true}"#,
            ),
            wrapped_response("local_scheduler", "status", &status, APPLICATION_JSON),
        ] {
            assert_rejected(validate_status_response(
                response,
                &SorafsRepairFinalizedAnchor::default(),
            ));
        }

        let hash = hex::encode(cursor.block_hash);
        let wrong_finality = SorafsRepairFinalizedAnchor {
            expected_finalized_height: Some(cursor.height + 1),
            expected_finalized_block_hash_hex: Some(&hash),
        };
        assert_rejected(validate_status_response(
            exact_response("status", &status),
            &wrong_finality,
        ));
        assert_rejected(validate_task_response(
            exact_response(
                "task",
                &RepairFinalizedTaskV1 {
                    finalized_cursor: cursor,
                    task: repair_task(&client, "REP-OTHER", [0x20; 32]),
                },
            ),
            "REP-REQUESTED",
            &SorafsRepairFinalizedAnchor::default(),
        ));

        let first = repair_task(&client, "REP-1", [0x20; 32]);
        let second = repair_task(&client, "REP-2", [0x30; 32]);
        let task_filter = SorafsRepairTasksFilter {
            limit: Some(2),
            ..SorafsRepairTasksFilter::default()
        };
        for page in [
            RepairLedgerTaskPageV1 {
                finalized_cursor: cursor,
                tasks: vec![second, first.clone()],
                has_more: false,
                next_after_task_id: None,
            },
            RepairLedgerTaskPageV1 {
                finalized_cursor: cursor,
                tasks: vec![first],
                has_more: true,
                next_after_task_id: Some([0x21; 32]),
            },
        ] {
            assert_tasks_rejected(&page, &task_filter);
        }

        let event_filter = SorafsRepairEventsFilter {
            limit: Some(2),
            ..SorafsRepairEventsFilter::default()
        };
        let first = repair_event(&client, 1, 5, [0x51; 32], 0);
        for events in [
            vec![first.clone(), repair_event(&client, 3, 5, [0x51; 32], 1)],
            vec![first, repair_event(&client, 2, 5, [0x51; 32], 2)],
            vec![repair_event(&client, 1, 0, [0; 32], 0)],
        ] {
            assert_events_rejected(
                &RepairFinalizedEventPageV1 {
                    finalized_cursor: cursor,
                    events,
                    has_more: false,
                    next_after: None,
                },
                &event_filter,
            );
        }
        let mut continuation = RepairFinalizedEventPageV1 {
            finalized_cursor: cursor,
            events: vec![repair_event(
                &client,
                2,
                cursor.height,
                cursor.block_hash,
                0,
            )],
            has_more: true,
            next_after: Some(RepairFinalizedEventCursorV1 {
                sequence: 2,
                block_height: cursor.height,
                block_hash: cursor.block_hash,
                event_index: 1,
            }),
        };
        let after_hash = hex::encode([0x51; 32]);
        assert_events_rejected(
            &continuation,
            &SorafsRepairEventsFilter {
                limit: Some(2),
                after_sequence: Some(1),
                after_block_height: Some(5),
                after_block_hash_hex: Some(&after_hash),
                after_event_index: Some(0),
                ..SorafsRepairEventsFilter::default()
            },
        );
        continuation.events.clear();
    }

    fn assert_repair_transport_contract() {
        let finalized = SorafsRepairFinalizedAnchor::default();
        let tasks = SorafsRepairTasksFilter::default();
        let events = SorafsRepairEventsFilter::default();
        for response in [
            validate_status_response(non_ok_response(), &finalized),
            validate_tasks_response(non_ok_response(), &tasks),
            validate_task_response(non_ok_response(), "REP-1", &finalized),
            validate_events_response(non_ok_response(), &events),
        ] {
            assert_non_ok_preserved(response.expect("non-OK repair response"));
        }

        let client = client_with_base_url(base_url());
        let snapshots: SnapshotStore = Arc::default();
        with_mock_http(
            respond_with(&snapshots, json_response(StatusCode::OK, "{}")),
            || {
                for response in [
                    client.get_sorafs_repair_status(&finalized),
                    client.get_sorafs_repair_tasks(&tasks),
                    client.get_sorafs_repair_task("REP-1", &finalized),
                    client.get_sorafs_repair_events(&events),
                ] {
                    assert_rejected(response);
                }
            },
        );
        assert_eq!(snapshots.lock().expect("snapshot lock").len(), 4);
    }

    macro_rules! repair_contract_tests {
        ($matrix:ident => [$($name:ident),+ $(,)?]) => {
            $(
                #[test]
                fn $name() {
                    $matrix();
                }
            )+
        };
    }

    repair_contract_tests!(assert_repair_route_contract => [
        repair_route_validation_accepts_every_exact_instruction,
        repair_route_validation_rejects_mismatch_and_wrong_action_before_http,
        repair_route_validation_rejects_non_native_and_non_singleton_before_http,
    ]);
    repair_contract_tests!(assert_repair_read_success_contract => [
        repair_read_response_binding_accepts_exact_typed_wrappers,
    ]);
    repair_contract_tests!(assert_repair_rejection_contract => [
        repair_read_response_binding_rejects_wrapper_finality_and_ticket_mismatches,
        repair_task_page_response_binding_rejects_bounds_order_and_bad_continuations,
        repair_task_page_response_binding_rejects_omitted_limit_over_torii_default,
        repair_event_page_response_binding_rejects_bounds_order_and_bad_continuations,
        repair_event_page_response_binding_rejects_omitted_limit_over_torii_default,
        repair_event_page_response_binding_rejects_noncanonical_block_index_successors,
    ]);
    repair_contract_tests!(assert_repair_transport_contract => [
        repair_read_response_binding_preserves_every_non_ok_response,
        repair_read_methods_validate_every_successful_response_after_send,
    ]);
}
