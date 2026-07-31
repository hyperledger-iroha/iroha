//! Deterministic terminal-handoff identity and finalized-event binding.

use iroha_data_model::{
    ChainId,
    events::data::sorafs::SorafsModerationLedgerEventKind,
    sorafs::moderation_ledger::{
        ModerationFinalizedCursorV1, ModerationFinalizedEventV1,
        ModerationFinalizedLedgerSnapshotV1, ModerationOutcomeRecordV1,
        is_canonical_moderation_identifier_v1,
    },
};

use super::{
    ACTION_DIGEST_DOMAIN_V1, HANDOFF_ID_DOMAIN_V1, MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1,
    ModerationOrchestratorCheckpointV1, ModerationOrchestratorError,
    ModerationTerminalHandoffKindV1, ModerationTerminalHandoffV1, domain_hash,
    external_work_cursor_is_valid, refresh_panel_notification_outbox_digest,
};

impl ModerationOrchestratorCheckpointV1 {
    pub(super) fn new(chain_id: &ChainId) -> Self {
        let mut state = Self {
            version: MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1,
            chain_id: chain_id.as_str().to_owned(),
            generation: 0,
            panel_notification_clock_unix_ms: 0,
            panel_notification_scanned_cursor: None,
            panel_notification_outbox_digest: [0; 32],
            finalized_snapshot: None,
            finalized_snapshot_digest: None,
            operations: Vec::new(),
            outbox: Vec::new(),
            dead_letters: Vec::new(),
            pending_handoffs: Vec::new(),
            completed_handoffs: Vec::new(),
            panel_notifications: Vec::new(),
        };
        refresh_panel_notification_outbox_digest(&mut state);
        state
    }
}

pub(super) fn terminal_handoff_id(
    chain_id: &ChainId,
    kind: ModerationTerminalHandoffKindV1,
    case_id: &str,
    round_id: &str,
    outcome_digest: [u8; 32],
) -> [u8; 32] {
    let kind = [match kind {
        ModerationTerminalHandoffKindV1::Settlement => 0,
        ModerationTerminalHandoffKindV1::Publication => 1,
    }];
    domain_hash(
        HANDOFF_ID_DOMAIN_V1,
        &[
            chain_id.as_str().as_bytes(),
            &kind,
            case_id.as_bytes(),
            round_id.as_bytes(),
            &outcome_digest,
        ],
    )
}

pub(super) fn retained_terminal_finalization_event<'a>(
    snapshot: &'a ModerationFinalizedLedgerSnapshotV1,
    case_id: &str,
    round_id: &str,
) -> Result<Option<&'a ModerationFinalizedEventV1>, &'static str> {
    let mut matches = snapshot.events.iter().filter(|event| {
        *event.event.kind() == SorafsModerationLedgerEventKind::CaseFinalized
            && event.event.case_id().as_deref() == Some(case_id)
            && event.event.round_id().as_deref() == Some(round_id)
    });
    let event = matches.next();
    if matches.next().is_some() {
        return Err("multiple retained finalization events exist for one case and round");
    }
    Ok(event)
}

pub(super) fn terminal_finalization_event_matches_outcome(
    event: &ModerationFinalizedEventV1,
    outcome: &ModerationOutcomeRecordV1,
) -> bool {
    event.event.authority() == &outcome.finalized_by
        && *event.event.occurred_at_unix_ms() == outcome.finalized_at_unix_ms
}

pub(super) fn retained_terminal_finalization_cursor(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    outcome: &ModerationOutcomeRecordV1,
) -> Result<ModerationFinalizedCursorV1, &'static str> {
    let Some(event) =
        retained_terminal_finalization_event(snapshot, &outcome.case_id, &outcome.round_id)?
    else {
        // V1 requires the exact finalization event in the committed projection
        // and fails closed once that provenance is unavailable.
        return Err("terminal outcome has no retained exact finalization event");
    };
    if !terminal_finalization_event_matches_outcome(event, outcome) {
        return Err("terminal finalization event provenance differs from the outcome");
    }
    Ok(ModerationFinalizedCursorV1 {
        height: event.block_height,
        block_hash: event.block_hash,
    })
}

pub(super) fn validate_retained_terminal_handoff(
    handoff: &ModerationTerminalHandoffV1,
    snapshot: Option<&ModerationFinalizedLedgerSnapshotV1>,
    chain_id: &ChainId,
) -> Result<(), ModerationOrchestratorError> {
    if !is_canonical_moderation_identifier_v1(&handoff.case_id)
        || !is_canonical_moderation_identifier_v1(&handoff.round_id)
        || handoff.outcome_digest == [0; 32]
        || !external_work_cursor_is_valid(
            handoff.finalized_cursor.height,
            handoff.finalized_cursor.block_hash,
            snapshot,
        )
        || handoff.handoff_id
            != terminal_handoff_id(
                chain_id,
                handoff.kind,
                &handoff.case_id,
                &handoff.round_id,
                handoff.outcome_digest,
            )
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff identity, scope, or finalized cursor is invalid".to_owned(),
        ));
    }
    let snapshot = snapshot.ok_or_else(|| {
        ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff has no retained finalized projection".to_owned(),
        )
    })?;
    let outcome = snapshot
        .case(&handoff.case_id, &handoff.round_id)
        .and_then(|case| case.outcome.as_ref())
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "terminal handoff has no authoritative outcome".to_owned(),
            )
        })?;
    let outcome_bytes = norito::to_bytes(outcome).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode retained terminal outcome: {error}"
        ))
    })?;
    if handoff.outcome_digest != domain_hash(ACTION_DIGEST_DOMAIN_V1, &[&outcome_bytes]) {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff differs from its authoritative outcome".to_owned(),
        ));
    }
    if let Some(event) =
        retained_terminal_finalization_event(snapshot, &handoff.case_id, &handoff.round_id)
            .map_err(|message| ModerationOrchestratorError::CheckpointCorrupt(message.to_owned()))?
    {
        let exact_cursor = ModerationFinalizedCursorV1 {
            height: event.block_height,
            block_hash: event.block_hash,
        };
        if !terminal_finalization_event_matches_outcome(event, outcome)
            || handoff.finalized_cursor != exact_cursor
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "terminal handoff differs from its retained exact finalization event".to_owned(),
            ));
        }
    }
    Ok(())
}
