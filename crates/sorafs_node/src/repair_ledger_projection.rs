//! Complete finalized-ledger repair projections used by destructive local work.
//!
//! A projection can be constructed only by consuming every page anchored to one finalized cursor
//! and matching the chain-authoritative status count. Callers cannot obtain a partially collected
//! projection, so local storage deletion never relies on a stale or truncated repair-task cache.
use crate::repair_transaction_forwarder::{decode_repair_report, decode_slash_proposal};
use iroha_data_model::sorafs::moderation_ledger::{
    REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1, REPAIR_LEDGER_MAX_RECEIPTS_V1,
    REPAIR_LEDGER_TASK_VERSION_V1, REPAIR_QUERY_MAX_ITEMS_V1, REPAIR_QUERY_MAX_TASK_PAGE_BYTES_V1,
    RepairFinalizedCursorV1, RepairFinalizedStatusV1, RepairLedgerCompletedV1,
    RepairLedgerEscalatedV1, RepairLedgerFailedV1, RepairLedgerStatusV1, RepairLedgerTaskPageV1,
    RepairLedgerTaskV1, RepairLedgerTerminalKindV1, RepairLedgerTerminalOutcomeV1,
    sorafs_repair_appeal_id_v1, sorafs_repair_task_id_v1,
};
use std::collections::BTreeSet;
use thiserror::Error;
/// Maximum finalized task pages accepted for one GC projection.
///
/// The page bound is independent of the number of tasks retained by consensus. A larger ledger
/// therefore fails closed instead of authorizing deletion from a prefix.
pub const REPAIR_GC_PROJECTION_MAX_PAGES_V1: usize = 1_024;
/// Maximum finalized repair tasks accepted for one GC projection.
pub const REPAIR_GC_PROJECTION_MAX_TASKS_V1: usize = 65_536;
/// Maximum aggregate canonical page bytes accepted for one GC projection.
pub const REPAIR_GC_PROJECTION_MAX_ENCODED_BYTES_V1: usize = 64 * 1024 * 1024;
#[derive(Debug, Clone, Copy)]
struct RepairLedgerProjectionLimitsV1 {
    pages: usize,
    tasks: usize,
    encoded_bytes: usize,
}
impl Default for RepairLedgerProjectionLimitsV1 {
    fn default() -> Self {
        Self {
            pages: REPAIR_GC_PROJECTION_MAX_PAGES_V1,
            tasks: REPAIR_GC_PROJECTION_MAX_TASKS_V1,
            encoded_bytes: REPAIR_GC_PROJECTION_MAX_ENCODED_BYTES_V1,
        }
    }
}
/// Failure to prove one complete, bounded finalized repair-task projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RepairLedgerProjectionErrorV1 {
    /// The finalized cursor is empty or otherwise unusable.
    #[error("repair projection finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Chain-authoritative counters are internally inconsistent.
    #[error("repair projection status counters are inconsistent")]
    InvalidStatus,
    /// The authoritative task count exceeds the destructive-work ceiling.
    #[error("repair projection task ceiling exceeded")]
    TaskLimitExceeded,
    /// Collection required more pages than the destructive-work ceiling.
    #[error("repair projection page ceiling exceeded")]
    PageLimitExceeded,
    /// Aggregate canonical page bytes exceed the destructive-work ceiling.
    #[error("repair projection encoded-byte ceiling exceeded")]
    EncodedByteLimitExceeded,
    /// A page was not bound to the initial finalized cursor.
    #[error("repair projection page finalized cursor mismatch")]
    PageAnchorMismatch,
    /// A page violates the exclusive continuation-cursor contract.
    #[error("repair projection page continuation cursor is invalid")]
    InvalidPageCursor,
    /// A page arrived after the terminal page.
    #[error("repair projection received a page after completion")]
    PageAfterCompletion,
    /// Task identifiers are not strictly increasing across the whole projection.
    #[error("repair projection task order is invalid")]
    InvalidTaskOrder,
    /// A finalized task is malformed or conflicts with its canonical report.
    #[error("repair projection contains an invalid task")]
    InvalidTask,
    /// No terminal page was observed or its task count differs from status.
    #[error("repair projection is incomplete")]
    Incomplete,
}
/// Builder that withholds the projection until every bounded page is proven.
#[derive(Debug)]
pub struct RepairLedgerTaskProjectionBuilderV1 {
    finalized_cursor: RepairFinalizedCursorV1,
    expected_task_count: usize,
    limits: RepairLedgerProjectionLimitsV1,
    pages_seen: usize,
    encoded_bytes: usize,
    terminal_page_seen: bool,
    last_task_id: Option<[u8; 32]>,
    ticket_ids: BTreeSet<String>,
    tasks: Vec<RepairLedgerTaskV1>,
}
impl RepairLedgerTaskProjectionBuilderV1 {
    /// Start collecting pages for one chain-authoritative finalized status.
    ///
    /// # Errors
    ///
    /// Fails before allocation when the anchor, counters, or global task count
    /// cannot satisfy the destructive-work ceilings.
    pub fn new(status: RepairFinalizedStatusV1) -> Result<Self, RepairLedgerProjectionErrorV1> {
        Self::new_with_limits(status, RepairLedgerProjectionLimitsV1::default())
    }
    fn new_with_limits(
        status: RepairFinalizedStatusV1,
        limits: RepairLedgerProjectionLimitsV1,
    ) -> Result<Self, RepairLedgerProjectionErrorV1> {
        validate_finalized_cursor(status.finalized_cursor)?;
        validate_status(status.status)?;
        let expected_task_count = usize::try_from(status.status.tasks)
            .map_err(|_| RepairLedgerProjectionErrorV1::TaskLimitExceeded)?;
        if limits.pages == 0
            || limits.tasks == 0
            || limits.encoded_bytes == 0
            || expected_task_count > limits.tasks
        {
            return Err(RepairLedgerProjectionErrorV1::TaskLimitExceeded);
        }
        Ok(Self {
            finalized_cursor: status.finalized_cursor,
            expected_task_count,
            limits,
            pages_seen: 0,
            encoded_bytes: 0,
            terminal_page_seen: false,
            last_task_id: None,
            ticket_ids: BTreeSet::new(),
            tasks: Vec::new(),
        })
    }
    /// Consume the next exclusive-cursor page at the initial finalized anchor.
    ///
    /// # Errors
    ///
    /// Fails closed on anchor drift, malformed continuation metadata, invalid
    /// tasks, non-monotonic ordering, or any resource ceiling.
    pub fn push_page(
        &mut self,
        page: RepairLedgerTaskPageV1,
    ) -> Result<(), RepairLedgerProjectionErrorV1> {
        if self.terminal_page_seen {
            return Err(RepairLedgerProjectionErrorV1::PageAfterCompletion);
        }
        if page.finalized_cursor != self.finalized_cursor {
            return Err(RepairLedgerProjectionErrorV1::PageAnchorMismatch);
        }
        let next_pages = self
            .pages_seen
            .checked_add(1)
            .ok_or(RepairLedgerProjectionErrorV1::PageLimitExceeded)?;
        if next_pages > self.limits.pages {
            return Err(RepairLedgerProjectionErrorV1::PageLimitExceeded);
        }
        if page.tasks.len()
            > usize::try_from(REPAIR_QUERY_MAX_ITEMS_V1)
                .map_err(|_| RepairLedgerProjectionErrorV1::TaskLimitExceeded)?
        {
            return Err(RepairLedgerProjectionErrorV1::TaskLimitExceeded);
        }
        let encoded_page =
            norito::to_bytes(&page).map_err(|_| RepairLedgerProjectionErrorV1::InvalidTask)?;
        if encoded_page.len() > REPAIR_QUERY_MAX_TASK_PAGE_BYTES_V1 {
            return Err(RepairLedgerProjectionErrorV1::EncodedByteLimitExceeded);
        }
        let next_encoded_bytes = self
            .encoded_bytes
            .checked_add(encoded_page.len())
            .ok_or(RepairLedgerProjectionErrorV1::EncodedByteLimitExceeded)?;
        if next_encoded_bytes > self.limits.encoded_bytes {
            return Err(RepairLedgerProjectionErrorV1::EncodedByteLimitExceeded);
        }
        let page_last_task_id = page.tasks.last().map(|task| task.task_id);
        match (page.has_more, page.next_after_task_id, page_last_task_id) {
            (true, Some(next), Some(last)) if next == last => {}
            (false, None, _) => {}
            _ => return Err(RepairLedgerProjectionErrorV1::InvalidPageCursor),
        }
        let next_task_count = self
            .tasks
            .len()
            .checked_add(page.tasks.len())
            .ok_or(RepairLedgerProjectionErrorV1::TaskLimitExceeded)?;
        if next_task_count > self.limits.tasks || next_task_count > self.expected_task_count {
            return Err(RepairLedgerProjectionErrorV1::TaskLimitExceeded);
        }
        for task in &page.tasks {
            validate_task(task)?;
            if self
                .last_task_id
                .is_some_and(|previous| previous >= task.task_id)
                || !self.ticket_ids.insert(task.ticket_id.clone())
            {
                return Err(RepairLedgerProjectionErrorV1::InvalidTaskOrder);
            }
            self.last_task_id = Some(task.task_id);
        }
        self.pages_seen = next_pages;
        self.encoded_bytes = next_encoded_bytes;
        self.terminal_page_seen = !page.has_more;
        self.tasks.extend(page.tasks);
        Ok(())
    }
    /// Finish collection only after the terminal page and exact global count.
    ///
    /// # Errors
    ///
    /// A truncated, over-limit, or count-mismatched collection is never
    /// converted into a projection.
    pub fn finish(self) -> Result<RepairLedgerTaskProjectionV1, RepairLedgerProjectionErrorV1> {
        if self.pages_seen == 0
            || !self.terminal_page_seen
            || self.tasks.len() != self.expected_task_count
        {
            return Err(RepairLedgerProjectionErrorV1::Incomplete);
        }
        Ok(RepairLedgerTaskProjectionV1 {
            finalized_cursor: self.finalized_cursor,
            encoded_bytes: self.encoded_bytes,
            tasks: self.tasks,
        })
    }
}
/// Complete bounded repair-task state from one immutable finalized view.
#[derive(Debug, Clone)]
pub struct RepairLedgerTaskProjectionV1 {
    finalized_cursor: RepairFinalizedCursorV1,
    encoded_bytes: usize,
    tasks: Vec<RepairLedgerTaskV1>,
}
impl RepairLedgerTaskProjectionV1 {
    /// Finalized cursor shared by the status and every consumed page.
    #[must_use]
    pub const fn finalized_cursor(&self) -> RepairFinalizedCursorV1 {
        self.finalized_cursor
    }
    /// Exact globally proven task count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
    /// Whether the globally proven projection contains no tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
    /// Aggregate canonical bytes consumed while proving completeness.
    #[must_use]
    pub const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    /// Every finalized repair task in strict immutable task-id order.
    #[must_use]
    pub fn tasks(&self) -> &[RepairLedgerTaskV1] {
        &self.tasks
    }
    /// Non-terminal tasks for one provider.
    ///
    /// Filtering is deliberately available only on a finished projection, so provider-local GC
    /// cannot treat a globally truncated page set as proof that no active repair exists.
    pub fn active_tasks_for_provider(
        &self,
        provider_id: [u8; 32],
    ) -> impl Iterator<Item = &RepairLedgerTaskV1> {
        self.tasks
            .iter()
            .filter(move |task| task.provider_id == provider_id && task.terminal_outcome.is_none())
    }
}
fn validate_finalized_cursor(
    cursor: RepairFinalizedCursorV1,
) -> Result<(), RepairLedgerProjectionErrorV1> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] {
        return Err(RepairLedgerProjectionErrorV1::InvalidFinalizedCursor);
    }
    Ok(())
}
fn validate_status(status: RepairLedgerStatusV1) -> Result<(), RepairLedgerProjectionErrorV1> {
    if status == RepairLedgerStatusV1::default() {
        return Ok(());
    }
    let terminal_sum = status
        .completed
        .checked_add(status.failed)
        .and_then(|value| value.checked_add(status.escalated));
    let open_tasks = status.tasks.checked_sub(status.terminal_outcomes);
    if status.updated_at_unix_ms == 0
        || terminal_sum != Some(status.terminal_outcomes)
        || open_tasks.is_none()
        || open_tasks.is_some_and(|open| status.leased_tasks > open)
        || status.slash_proposals != status.escalated
        || status.appeals > status.slash_proposals
    {
        return Err(RepairLedgerProjectionErrorV1::InvalidStatus);
    }
    Ok(())
}
#[allow(clippy::too_many_lines)]
/// Validate one finalized native repair task and all embedded provenance.
///
/// This is shared by destructive storage execution and Torii finality reconciliation so malformed
/// task/receipt/slash/appeal records are rejected before either consumer performs a side effect.
pub fn validate_task(task: &RepairLedgerTaskV1) -> Result<(), RepairLedgerProjectionErrorV1> {
    let report = decode_repair_report(&task.canonical_report)
        .map_err(|_| RepairLedgerProjectionErrorV1::InvalidTask)?;
    let expected_revision = u64::try_from(task.action_receipts.len())
        .ok()
        .and_then(|count| count.checked_add(1));
    if task.version != REPAIR_LEDGER_TASK_VERSION_V1
        || task.source_identity == [0; 32]
        || task.task_id != sorafs_repair_task_id_v1(task.source_identity)
        || task.ticket_id != report.ticket_id.0
        || report.evidence.manifest_digest != task.manifest_digest
        || report.evidence.provider_id != task.provider_id
        || report.auditor_account != task.submitted_by.to_string()
        || task.submitted_at_unix_ms == 0
        || task.updated_at_unix_ms < task.submitted_at_unix_ms
        || task.revision == 0
        || expected_revision != Some(task.revision)
        || task.action_receipts.len() > REPAIR_LEDGER_MAX_RECEIPTS_V1
    {
        return Err(RepairLedgerProjectionErrorV1::InvalidTask);
    }
    let mut receipt_keys = BTreeSet::new();
    let mut last_revision = 1_u64;
    for receipt in &task.action_receipts {
        let expected = last_revision
            .checked_add(1)
            .ok_or(RepairLedgerProjectionErrorV1::InvalidTask)?;
        if receipt.idempotency_digest == [0; 32]
            || receipt.action_digest == [0; 32]
            || !receipt_keys.insert(receipt.idempotency_digest)
            || receipt.resulting_revision != expected
        {
            return Err(RepairLedgerProjectionErrorV1::InvalidTask);
        }
        last_revision = expected;
    }
    if let Some(lease) = &task.lease
        && (lease.generation == 0
            || lease.acquired_at_unix_ms == 0
            || lease.renewed_at_unix_ms < lease.acquired_at_unix_ms
            || lease.expires_at_unix_ms <= lease.renewed_at_unix_ms
            || task.terminal_outcome.is_some())
    {
        return Err(RepairLedgerProjectionErrorV1::InvalidTask);
    }
    match (&task.terminal_outcome, &task.slash, &task.appeal) {
        (None, None, None) => {}
        (
            Some(RepairLedgerTerminalOutcomeV1 {
                kind:
                    RepairLedgerTerminalKindV1::Completed(RepairLedgerCompletedV1 { evidence_digest }),
                ..
            }),
            None,
            None,
        ) if *evidence_digest != [0; 32] => {}
        (
            Some(RepairLedgerTerminalOutcomeV1 {
                kind: RepairLedgerTerminalKindV1::Failed(RepairLedgerFailedV1 { failure_digest }),
                ..
            }),
            None,
            None,
        ) if *failure_digest != [0; 32] => {}
        (
            Some(RepairLedgerTerminalOutcomeV1 {
                kind:
                    RepairLedgerTerminalKindV1::Escalated(RepairLedgerEscalatedV1 {
                        slash_proposal_digest,
                    }),
                finalized_by,
                ..
            }),
            Some(slash),
            appeal,
        ) if *slash_proposal_digest != [0; 32]
            && *slash_proposal_digest == slash.proposal_digest
            && slash.proposal_digest == *blake3::hash(&slash.canonical_proposal).as_bytes()
            && &slash.submitted_by == finalized_by
            && slash.submitted_at_unix_ms != 0
            && appeal.as_ref().is_none_or(|appeal| {
                appeal.slash_proposal_digest == slash.proposal_digest
                    && appeal.evidence_digest != [0; 32]
                    && !appeal.reason.is_empty()
                    && appeal.reason == appeal.reason.trim()
                    && appeal.reason.len() <= REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1
                    && !appeal.reason.chars().any(char::is_control)
                    && appeal.submitted_at_unix_ms >= slash.submitted_at_unix_ms
                    && appeal.appeal_id
                        == sorafs_repair_appeal_id_v1(
                            task.task_id,
                            slash.proposal_digest,
                            &appeal.appellant,
                            appeal.evidence_digest,
                            &appeal.reason,
                        )
            }) => {}
        _ => return Err(RepairLedgerProjectionErrorV1::InvalidTask),
    }
    if let Some(slash) = &task.slash {
        let proposal = decode_slash_proposal(&slash.canonical_proposal)
            .map_err(|_| RepairLedgerProjectionErrorV1::InvalidTask)?;
        if proposal.ticket_id.0 != task.ticket_id
            || proposal.provider_id != task.provider_id
            || proposal.manifest_digest != task.manifest_digest
            || proposal.auditor_account != task.submitted_by.to_string()
            || proposal.approval.is_some()
            || proposal.submitted_at_unix < report.submitted_at_unix
            || proposal.submitted_at_unix > slash.submitted_at_unix_ms / 1_000
        {
            return Err(RepairLedgerProjectionErrorV1::InvalidTask);
        }
    }
    if let Some(terminal) = &task.terminal_outcome
        && (terminal.lease_generation == 0
            || terminal.finalized_at_unix_ms < task.submitted_at_unix_ms
            || terminal.finalized_at_unix_ms > task.updated_at_unix_ms
            || task.lease.is_some())
    {
        return Err(RepairLedgerProjectionErrorV1::InvalidTask);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::account::AccountId;
    use sorafs_manifest::repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairManualCauseV1, RepairReportV1, RepairTicketId,
    };
    fn cursor(byte: u8) -> RepairFinalizedCursorV1 {
        RepairFinalizedCursorV1 {
            height: u64::from(byte),
            block_hash: [byte; 32],
        }
    }
    fn account(byte: u8) -> AccountId {
        let key = KeyPair::try_from_seed(vec![byte; 32], Algorithm::Ed25519)
            .expect("deterministic account key");
        AccountId::new(key.public_key().clone())
    }
    fn task(ticket: &str, source_identity: [u8; 32]) -> RepairLedgerTaskV1 {
        let submitted_by = account(source_identity[0]);
        let manifest_digest = [source_identity[0].wrapping_add(1); 32];
        let provider_id = [source_identity[0].wrapping_add(2); 32];
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket.to_owned()),
            auditor_account: submitted_by.to_string(),
            submitted_at_unix: 1,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "projection test".to_owned(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        RepairLedgerTaskV1 {
            version: REPAIR_LEDGER_TASK_VERSION_V1,
            task_id: sorafs_repair_task_id_v1(source_identity),
            source_identity,
            ticket_id: ticket.to_owned(),
            canonical_report: norito::to_bytes(&report).expect("encode repair report"),
            manifest_digest,
            provider_id,
            submitted_by,
            submitted_at_unix_ms: 1_000,
            revision: 1,
            lease: None,
            terminal_outcome: None,
            slash: None,
            appeal: None,
            action_receipts: Vec::new(),
            updated_at_unix_ms: 1_000,
        }
    }
    fn status(finalized_cursor: RepairFinalizedCursorV1, tasks: u64) -> RepairFinalizedStatusV1 {
        RepairFinalizedStatusV1 {
            finalized_cursor,
            status: RepairLedgerStatusV1 {
                tasks,
                updated_at_unix_ms: 1_000,
                ..RepairLedgerStatusV1::default()
            },
        }
    }
    #[test]
    fn complete_projection_is_exposed_only_after_exact_terminal_count() {
        let anchor = cursor(7);
        let task = task("REP-PROJECTION-1", [0x11; 32]);
        let provider_id = task.provider_id;
        let mut builder =
            RepairLedgerTaskProjectionBuilderV1::new(status(anchor, 1)).expect("builder");
        builder
            .push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![task],
                has_more: false,
                next_after_task_id: None,
            })
            .expect("terminal page");
        let projection = builder.finish().expect("complete projection");
        assert_eq!(projection.finalized_cursor(), anchor);
        assert_eq!(projection.len(), 1);
        assert!(!projection.is_empty());
        assert!(projection.encoded_bytes() > 0);
        assert_eq!(projection.active_tasks_for_provider(provider_id).count(), 1);
        assert_eq!(projection.active_tasks_for_provider([0xFF; 32]).count(), 0);
    }
    #[test]
    fn exact_all_zero_status_accepts_one_empty_terminal_page() {
        let anchor = cursor(6);
        let mut builder = RepairLedgerTaskProjectionBuilderV1::new(RepairFinalizedStatusV1 {
            finalized_cursor: anchor,
            status: RepairLedgerStatusV1::default(),
        })
        .expect("empty projection builder");
        builder
            .push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: Vec::new(),
                has_more: false,
                next_after_task_id: None,
            })
            .expect("empty terminal page");
        let projection = builder.finish().expect("complete empty projection");
        assert!(projection.is_empty());
        assert_eq!(projection.finalized_cursor(), anchor);
    }
    #[test]
    fn terminal_page_must_match_authoritative_global_count() {
        let anchor = cursor(8);
        let mut builder =
            RepairLedgerTaskProjectionBuilderV1::new(status(anchor, 2)).expect("builder");
        builder
            .push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![task("REP-PROJECTION-2", [0x21; 32])],
                has_more: false,
                next_after_task_id: None,
            })
            .expect("well-formed truncated page");
        assert!(matches!(
            builder.finish(),
            Err(RepairLedgerProjectionErrorV1::Incomplete)
        ));
    }
    #[test]
    fn page_anchor_and_exclusive_cursor_are_fail_closed() {
        let anchor = cursor(9);
        let first = task("REP-PROJECTION-3", [0x31; 32]);
        let mut builder =
            RepairLedgerTaskProjectionBuilderV1::new(status(anchor, 1)).expect("builder");
        assert_eq!(
            builder.push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: cursor(10),
                tasks: vec![first.clone()],
                has_more: false,
                next_after_task_id: None,
            }),
            Err(RepairLedgerProjectionErrorV1::PageAnchorMismatch)
        );
        assert_eq!(
            builder.push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![first],
                has_more: true,
                next_after_task_id: None,
            }),
            Err(RepairLedgerProjectionErrorV1::InvalidPageCursor)
        );
    }
    #[test]
    fn independent_projection_resource_ceilings_reject_prefixes() {
        let anchor = cursor(11);
        let one_page_only = RepairLedgerProjectionLimitsV1 {
            pages: 1,
            tasks: 2,
            encoded_bytes: REPAIR_GC_PROJECTION_MAX_ENCODED_BYTES_V1,
        };
        let first = task("REP-PROJECTION-4", [0x41; 32]);
        let first_id = first.task_id;
        let mut builder =
            RepairLedgerTaskProjectionBuilderV1::new_with_limits(status(anchor, 2), one_page_only)
                .expect("builder");
        builder
            .push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![first],
                has_more: true,
                next_after_task_id: Some(first_id),
            })
            .expect("first page");
        assert_eq!(
            builder.push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![task("REP-PROJECTION-5", [0x42; 32])],
                has_more: false,
                next_after_task_id: None,
            }),
            Err(RepairLedgerProjectionErrorV1::PageLimitExceeded)
        );
        assert!(matches!(
            RepairLedgerTaskProjectionBuilderV1::new_with_limits(
                status(anchor, 2),
                RepairLedgerProjectionLimitsV1 {
                    pages: 2,
                    tasks: 1,
                    encoded_bytes: REPAIR_GC_PROJECTION_MAX_ENCODED_BYTES_V1,
                },
            ),
            Err(RepairLedgerProjectionErrorV1::TaskLimitExceeded)
        ));
        let mut byte_limited = RepairLedgerTaskProjectionBuilderV1::new_with_limits(
            status(anchor, 1),
            RepairLedgerProjectionLimitsV1 {
                pages: 1,
                tasks: 1,
                encoded_bytes: 1,
            },
        )
        .expect("byte-limited builder");
        assert_eq!(
            byte_limited.push_page(RepairLedgerTaskPageV1 {
                finalized_cursor: anchor,
                tasks: vec![task("REP-PROJECTION-6", [0x43; 32])],
                has_more: false,
                next_after_task_id: None,
            }),
            Err(RepairLedgerProjectionErrorV1::EncodedByteLimitExceeded)
        );
    }
}
