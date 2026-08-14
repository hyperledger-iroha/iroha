//! Deterministic terminal-handoff identity and finalized-event binding.
use iroha_data_model::{
    NetworkId,
    events::data::sorafs::SorafsModerationLedgerEventKind,
    sorafs::moderation_ledger::{
        ModerationFinalizedEventV1, ModerationFinalizedLedgerSnapshotV1, ModerationOutcomeRecordV1,
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
    pub(super) fn new(network_id: &NetworkId) -> Self {
        let mut state = Self {
            version: MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1,
            network_id: *network_id,
            generation: 0,
            panel_notification_clock_unix_ms: 0,
            panel_notification_scanned_cursor: None,
            terminal_handoff_scanned_cursor: None,
            panel_notification_outbox_digest: [0; 32],
            panel_notification_archived_dead_letter_count: 0,
            terminal_handoff_archived_cursor: None,
            panel_notification_archive_compaction_reservation: None,
            panel_notification_archive_signer_epochs: Vec::new(),
            panel_notification_archive_head: None,
            panel_notification_archive_pending_publication: None,
            panel_notification_archive_published_head: None,
            panel_notification_archive_audit_cursor: None,
            finalized_snapshot: None,
            finalized_snapshot_digest: None,
            operations: Vec::new(),
            outbox: Vec::new(),
            dead_letters: Vec::new(),
            dead_letter_incident_sequence: 0,
            pending_handoffs: Vec::new(),
            completed_handoffs: Vec::new(),
            panel_notifications: Vec::new(),
            panel_notification_dead_letter_resolutions: Vec::new(),
        };
        refresh_panel_notification_outbox_digest(&mut state);
        state
    }
}
pub(super) fn terminal_handoff_id(
    network_id: &NetworkId,
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
            network_id.as_bytes(),
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
pub(super) fn validate_retained_terminal_handoff(
    handoff: &ModerationTerminalHandoffV1,
    snapshot: Option<&ModerationFinalizedLedgerSnapshotV1>,
    network_id: &NetworkId,
) -> Result<(), ModerationOrchestratorError> {
    if !is_canonical_moderation_identifier_v1(&handoff.case_id)
        || !is_canonical_moderation_identifier_v1(&handoff.round_id)
        || handoff.outcome_digest == [0; 32]
        || handoff.outcome_finalized_at_unix_ms == 0
        || !external_work_cursor_is_valid(
            handoff.finalized_cursor.block_height,
            handoff.finalized_cursor.block_hash,
            snapshot,
        )
        || !handoff.is_bound_to_network(network_id)
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff identity, scope, or finalized cursor is invalid".to_owned(),
        ));
    }
    let witness = &handoff.source_event_witness;
    if witness.cursor() != handoff.finalized_cursor
        || *witness.event.kind() != SorafsModerationLedgerEventKind::CaseFinalized
        || witness.event.case_id().as_deref() != Some(&handoff.case_id)
        || witness.event.round_id().as_deref() != Some(&handoff.round_id)
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff sealed event witness is malformed or substituted".to_owned(),
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
    if handoff.outcome_finalized_at_unix_ms != outcome.finalized_at_unix_ms {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff timestamp differs from its authoritative outcome".to_owned(),
        ));
    }
    if !terminal_finalization_event_matches_outcome(witness, outcome) {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff witness provenance differs from its outcome".to_owned(),
        ));
    }
    let retained_event =
        retained_terminal_finalization_event(snapshot, &handoff.case_id, &handoff.round_id)
            .map_err(|message| {
                ModerationOrchestratorError::CheckpointCorrupt(message.to_owned())
            })?;
    if retained_event.is_some_and(|event| event != witness) {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "terminal handoff differs from its retained exact finalization event".to_owned(),
        ));
    }
    Ok(())
}
