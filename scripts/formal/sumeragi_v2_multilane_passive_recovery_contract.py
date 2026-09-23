"""Static source contract for passive diagnostics and bounded recovery retry."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable, Optional

from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments


PASSIVE_RECOVERY_CONTRACT_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_passive_recovery_contract.py"
)
PASSIVE_RECOVERY_TEST_RELATIVE = Path(
    "pytests/scripts/sumeragi_v2_multilane_passive_recovery_contract_test.py"
)

NATIVE_MODULE = "SumeragiV2NativeApplicationEvidence"
AUTONOMOUS_MODULE = "SumeragiV2AutonomousReservationCarrier"

# Passive completed diagnostics retain proof data only; none of these owners
# reconstructs a Ready row or authorizes a new consensus output.
COMPLETED_EQUIVOCATION_BINDINGS = (
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'recover_finalized_lifecycle_equivocations',
        (
            'let view = state.view()',
            'configured_v2_evidence_horizon(view.world())',
            '.checked_add(1)',
            'Some(horizon) if horizon != 0 => proposal_height.saturating_sub(horizon).max(1)',
            'for height in first_height..=height',
            '.v2_finality_artifact(height)',
            'recover_context_lifecycle_equivocations(',
            '&finality.height_context',
            '&finality.validator_set_pops',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'recover_context_lifecycle_equivocations',
        (
            'if &context.network_id != state.network_id_ref()',
            'state\n        .kura()\n        .sumeragi_v2_storage_root()',
            '.join(hex::encode(context.id().0.as_ref()))',
            'LifecycleLedgerV1::read_completed_equivocations(',
            'for proof in proofs',
            'retain_sumeragi_v2_equivocation(state, context, proofs_of_possession, proof)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'retain_sumeragi_v2_equivocation',
        (
            'context: context.clone()',
            'proofs_of_possession: proofs_of_possession.to_vec()',
            'conflict,',
            'if v2_evidence_encoded_len(&payload) > MAX_V2_EVIDENCE_ADMISSION_BYTES',
            'validate_v2_equivocation(&payload)?',
            'if &payload.context.network_id != state.network_id_ref()',
            'canonicalize_v2_equivocation_evidence(&payload)',
            'Ok(retain_validated_local_evidence(state, canonical))',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'retain_validated_local_evidence',
        (
            'let snapshot = v2_committed_evidence_snapshot(view.world())',
            'snapshot.record_capacity_exceeded || snapshot.byte_capacity_exceeded',
            'if subject_height > next_height',
            'next_height.max(after_subject_height)',
            '!evidence_within_configured_horizon(earliest_admission_height, horizon, Some(subject_height))',
            'committed_key == &key',
            'Some(&offender_roster)',
            'pending.contains_key(&key)',
            'existing.offender_roster == offender_roster',
            'pending.len() >= MAX_V2_COMMITTED_EVIDENCE_RECORDS',
            'bytes > MAX_V2_LOCAL_EVIDENCE_BYTES',
            'pending.insert(',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs',
        'method',
        'LifecycleLedgerV1::read_completed_equivocations',
        (
            'let context = projection::lifecycle_context(height_context)',
            'LifecycleLedgerStoreV1::read_existing(root, context)?',
            'for record in ledger.records',
            'record.work_class() != Some(LifecycleWorkClass::EquivocationReport)',
            'record.terminal() != Some(Some(TerminalOutcome::Advanced))',
            'record.reconstruction_source() != record.owner().causal_root().digest()',
            'record.durable_payload() != Some(DurablePayloadReference::None)',
            'record.continuation() != Some(DurableContinuation::None)',
            'record.owner().first_admission_ordinal() != record.ordinal()',
            '.into_equivocation_proof()',
            'proofs.push(proof)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs',
        'method',
        'LifecycleReplayAuthorityV1::into_equivocation_proof',
        (
            'fn into_equivocation_proof(self)',
            'match self.source',
            'LifecycleReplaySourceV1::Equivocation(proof) => Some(proof)',
            '_ => None',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs',
        'method',
        'LifecycleLedgerStoreV1::read_existing',
        (
            'fs::symlink_metadata(root)',
            'error.kind() == std::io::ErrorKind::NotFound => return Ok(None)',
            'BoundLifecycleLedgerDirectory::bind(root, false)?',
            'let guard = directory.lock()?',
            '.read_bounded_locked(LEDGER_FILE, MAX_LEDGER_FRAME_BYTES)?',
            'decode_frame(&bytes, MAX_LEDGER_FRAME_BYTES)?',
            'if ledger.context() != context',
            'ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?',
            'Ok(Some(ledger))',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs',
        'method',
        'BoundLifecycleLedgerDirectory::bind',
        (
            'bind_lifecycle_directory_path(path, create)?',
            'validate_lifecycle_directory_metadata(&metadata, path)?',
            'identity: LifecycleStorageIdentity::from_metadata(&metadata)',
            'directory,',
            'operation_lock: std::sync::Mutex::new(())',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs',
        'method',
        'BoundLifecycleLedgerDirectory::read_bounded_locked',
        (
            'self.inspect_leaf(name, maximum)?',
            'self.open_leaf(name, leaf)?',
            '.take(maximum.saturating_add(1))',
            'observed != leaf.length || observed > maximum',
            'self.verify_open_leaf(&file, name, leaf)?',
            'self.verify_linked()?',
            'Ok(Some(bytes))',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/mod.rs',
        'method',
        'SumeragiStartArgs::start',
        (
            'evidence::recover_finalized_lifecycle_equivocations(state.as_ref())',
            'FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(',
            'SumeragiWorker',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs',
        'method',
        'ProductionV2Services::start_with_apply_service',
        (
            '!state.matches_kura_instance(&kura)',
            '!apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)',
            'Self::start_inner(',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs',
        'method',
        'ProductionV2Services::start_inner',
        (
            'Self::freeze_timeout_certificate_targets(',
            'super::evidence::recover_context_lifecycle_equivocations(',
            '            state.as_ref(),\n            &context,\n            &validator_set_pops,\n        )?;',
            'let io = V2IoHandle::spawn(',
        ),
    ),
)

PASSIVE_RECOVERY_MODEL_BINDINGS = (
    *COMPLETED_EQUIVOCATION_BINDINGS,
    # Exact route pointer -> authenticated record/current pair; no repair.
    ('SumeragiV2AutonomousReservationCarrier',
     'crates/iroha_core/src/kura.rs',
     'fn',
     'latest_autonomous_lane_block_artifacts_snapshot',
     ('read_autonomous_lane_route_latest_attempt_locked',
      'read_autonomous_lane_block_attempt_record_with_current_locked',
      'record.retirement.is_some',
      'recovered.sort_by_key',
      'recovered.truncate(limit)',
      'pointer.network_id != expected_network_id',
      'epoch_for_height(pointer.proposal_height)',
      '                    &entry,\n'
      '                    lane_id,\n'
      '                    pointer.lane_block_height,\n'
      '                    pointer.proposal_height,\n'
      '                    expected_network_id,\n'
      '                    expected_epoch,\n'
      '                    None,',
      'let Some((record, current)) = record? else',
      'recovered.push((artifact, current))')),
    ('SumeragiV2AutonomousReservationCarrier',
     'crates/iroha_core/src/kura.rs',
     'fn',
     'read_autonomous_lane_block_attempt_record_with_current_locked',
     ('Result<Option<(AutonomousLaneBlockDurableRecord, LaneBlockProposalV1)>>',
      'Self::decode_autonomous_lane_attempt_frame(',
      'if pointer.lane_id != lane_id\n'
      '                || pointer.lane_block_height != lane_block_height\n'
      '                || pointer.proposal_height != proposal_height',
      '.read_autonomous_lane_block_attempt_artifact_with_current_locked(\n'
      '                    entry,\n'
      '                    &pointer,\n'
      '                    expected_network_id,\n'
      '                    expected_epoch,',
      'pending_canonical_bytes.map_or(\n'
      '                        AutonomousLaneBlockViewStateReadMode::MainOnly,',
      'Some(DecodedAutonomousLaneAttemptRead { read, artifact })')),
    ('SumeragiV2AutonomousReservationCarrier',
     'crates/iroha_core/src/kura.rs',
     'fn',
     'read_autonomous_lane_block_attempt_artifact_with_current_locked',
     ('if pointer.network_id != expected_network_id || pointer.epoch != expected_epoch',
      'decoded.read.bytes == read.bytes',
      'Self::stable_sidecar_metadata_unchanged(',
      'if !pointer.matches_payload(&artifact.executable_payload)',
      'self.require_active_lane_artifact(entry, descriptor)?;',
      '.read_autonomous_lane_block_view_state_with_current_locked(\n'
      '                &artifact.executable_payload,\n'
      '                &view_state_path,\n'
      '                view_state_mode,',
      'Some((state, current))',
      '(state.retirement, current)',
      'Self::validate_autonomous_lane_block_artifact(\n'
      '                    &artifact,\n'
      '                    expected_network_id,\n'
      '                    expected_epoch,',
      '            current_proposal,\n        ))')),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::native_amx_participant_applications_diagnostics_once",
        (
            "pending_native_source_hashes",
            "merge_entry_by_hash_without_append_repair",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "durable_autonomous_lane_merge_source",
            "read_native_amx_participant_application_receipt",
            "read_structural_native_amx_participant_application_receipt",
            "HistoricalNativeAmxSourceAuthority::CertifiedCoordinator",
            "authenticated_native_amx_participant_application_rows_from_merge_entry",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "MergeLedgerLog::entry_by_hash_without_append_repair",
        (
            "self.append_recovery_offset.is_some()",
            "passive diagnostics cannot repair it",
            "entry_by_hash_with_append_repair_policy(hash, false)",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::merge_entry_by_hash_without_append_repair",
        (
            "ensure_prune_recovery_not_required",
            "ensure_canonical_storage_not_poisoned",
            "read_pending_merge_entry_path",
            "entry_by_hash_without_append_repair",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "read_structural_native_amx_participant_application_receipt",
        (
            "prune_recovery_is_required",
            "regular_sidecar_metadata_for",
            "decode_structural_native_amx_receipt_file_locked",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "read_native_amx_participant_application_receipt",
        (
            "read_native_amx_participant_application_receipt_from_paths_locked",
            "native_amx_participant_application_receipt_matches_available_evidence_under_prune_guard",
            "read_structural()? == artifact",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "method",
        "Kura::durable_autonomous_lane_merge_source",
        (
            "prune_lock.lock",
            "durable_autonomous_lane_merge_source_under_prune_guard",
            "None",
            "true",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_lane_block_execution_input_without_sidecar_repair",
        (
            "read_lane_block_execution_input_with_repair_policy",
            "false",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_lane_block_execution_preflight_without_sidecar_repair",
        (
            "read_lane_block_execution_preflight_with_repair_policy",
            "false",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
        (
            "lane_block_predecessor_application_receipt_available_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_lane_block_execution_preflight_without_sidecar_repair",
            "read_lane_block_execution_input_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::latest_lane_block_artifact_matching_without_sidecar_repair",
        (
            "ensure_bound_progress_pair_has_no_recovery_artifacts_locked",
            "read_active_lane_block_artifact_from_bound_without_repair_locked",
            "bound_progress_sidecar_unchanged",
            "read_lane_block_artifact_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
        (
            "PASSIVE_DIAGNOSTIC_CERTIFIED_RESULT_BUDGET",
            "PASSIVE_DIAGNOSTIC_CERTIFIED_SCAN_BUDGET",
            "ensure_bound_progress_pair_has_no_recovery_artifacts_locked",
            "read_active_certified_lane_block_artifact_from_bound_locked",
            "bound_progress_sidecar_unchanged",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        "method",
        "Kura::autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
        (
            "read_lane_block_application_receipt_without_sidecar_repair",
            "LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
            "lane_block_application_receipt_matches_merge_log_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "lane_block_payload_is_recoverable",
        (
            "recover_lane_block_payload_with_sidecar_repair(proposal, false)",
            ".is_ok()",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "ExecutionStatus::StateAppliedByCanonicalBlock",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
            "lane_block_execution_preflight_has_rejections_without_sidecar_repair",
            "lane_block_execution_input_available_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::durable_lane_diagnostics",
        (
            "latest_lane_block_artifact_matching_without_sidecar_repair",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "durable_lane_diagnostic_execution_status",
            "DurableLaneDiagnosticsSnapshot",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::committed_lane_block_status_snapshot",
        (
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_lane_block_application_receipt_without_sidecar_repair",
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "read_lane_block_execution_preflight_without_sidecar_repair",
            "read_lane_block_execution_input_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::autonomous_lane_execution_diagnostics_once",
        (
            "lane_consensus_lifecycle_snapshot",
            "routes.truncate(SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX)",
            "latest_autonomous_lane_block_artifacts_snapshot",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "pending_certified_merge_entries_bounded",
            "merge_ledger_latest_snapshot",
            "source_budget",
            "decode_autonomous_lane_merge_bundle",
            "read_lane_block_application_receipt_without_sidecar_repair",
            "LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
            "lane_reservation_diagnostic_groups_bounded",
            "AutonomousLaneDiagnosticEvidence::from_reservation_group",
            "lane_reservation_group_is_finalized_for_diagnostics",
            "AutonomousLaneDiagnosticEvidence::finish",
            "rows.sort_by_key",
            "row.validate()",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_torii/src/routing.rs",
        "fn",
        "handle_v1_sumeragi_diagnostics",
        (
            "native_amx_participant_applications_diagnostics",
            "durable_lane_diagnostics",
            "Option::as_ref(&durable_queue)",
            "autonomous_lane_execution_diagnostics",
            "autonomous_lane_execution_diagnostics_with_queue",
            "validate_autonomous_lane_executions",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "struct",
        "HistoricalRecoveryRequestCadence",
        (
            "reason: HistoricalRecoveryWaitReason",
            "retained_attempts: u32",
            "next_retry_at: Instant",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "HistoricalRecoveryWait::retry_delay",
        (
            "let ceiling = ceiling.max(floor)",
            "consecutive_attempts.saturating_sub(1)",
            "retry_tier_attempts.get()",
            "min(self.max_retry_tier.get())",
            "floor.saturating_mul(1_u32 << tier).min(ceiling)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "HistoricalRecoveryRequestCadence::after_retained_attempt",
        (
            "self.retained_attempts.saturating_add(1)",
            "consecutive_attempts: retained_attempts",
            "retry_delay(floor, ceiling)",
            "now.checked_add(delay)",
            "reason: observation.reason",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::service_next_historical_recovery_at_with_archive_targets",
        (
            "persist_historical_recovery_session",
            "historical_recovery_diagnostics.complete(identity)",
            "retire_historical_recovery_request(identity)",
            "historical_recovery_diagnostics",
            ".observe(identity, reason)",
            "schedule_historical_recovery_request",
            "historical_recovery_sessions.push_back(session)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::schedule_historical_recovery_request",
        (
            "observation: HistoricalRecoveryWait",
            "now: Instant",
            "HistoricalRecoveryRetry::LocalState",
            "retire_historical_recovery_request(identity)",
            "existing.request == request",
            "existing.request_hash == request_hash",
            "existing.cadence.reason == observation.reason",
            "HistoricalRecoveryRequestCadence::immediate(observation.reason, now)",
            "if !cadence.due(now)",
            "after_retained_attempt",
            "historical_recovery_retry_floor",
            "historical_recovery_retry_ceiling",
            "let mut scheduled_destinations = BTreeSet::new()",
            "if !self.push_effect(",
            "scheduled_destinations.insert(peer)",
            "if !scheduled_destinations.is_empty()",
            "owner.cadence = next_cadence",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "lane_work_limits",
        (
            "historical_recovery_retry_floor: Duration",
            "historical_recovery_retry_ceiling: Duration",
            "V2LaneWorkLimits::new",
            "historical_recovery_retry_floor",
            "historical_recovery_retry_ceiling",
            "historical_recovery_retry_tier_attempts",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "run_inner",
        (
            "public_key: genesis_public_key",
            "block_cadence",
            "sumeragi_v2_timing_ms(block_cadence_ms)",
            "let round_timeout = Duration::from_millis(round_timeout_ms)",
            "let retransmit_interval = Duration::from_millis(retransmit_interval_ms)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "if reservation_reconciliation_pending",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "reconcile_autonomous_lifecycle_startup(",
            "apply_lane_reservation_reconciliation_plan(",
            "reservation_reconciliation_pending = false;",
            "reconcile_executor_locked_body(executor, services)?",
            "preactivation.initialize_recovered_local_proposal(setup_runner)?",
            "preactivation.activate(height_started_at, local_proposal)?",
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_kura_lifecycle_height",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_lifecycle_active_height",
        (
            "native.take_service_publication(services)",
            "native.service_sources(services, now)?",
            "native.poll(native_global, native_network, now, receiver)?",
            "native.next_deadline().map_or(IDLE_POLL, |deadline|",
            "wake_rx.recv_timeout(native_wait)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_active_height",
        (
            "native.take_service_publication(services)",
            "native.service_sources(services, Instant::now())",
            "native.poll(native_global, native_network, Instant::now(), receiver)?",
            "wake_rx.recv_timeout(IDLE_POLL)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/canonical_recovery_ingress.rs",
        "fn",
        "service_historical_recovery_tick",
        (
            "has_pending_historical_recovery()",
            "services.current_archive_targets()",
            "service_next_historical_recovery_with_archive_targets(&current_archive_targets)",
            "map_err(V2RunnerError::from)",
        ),
    ),
)

# Native retry retains its original request/ticket. Only an instance target
# with its original State family and authenticated frozen-context closure may
# release that duplicate request. Candidate and Validate completion remain owned.
NATIVE_RECOVERY_BINDINGS = (
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_process.rs',
        'method',
        'NativeRunnerProcess::poll',
        (
            'self.settle_pending_publication()?',
            'self.note_current_observation(observed.as_ref())',
            'self.awaiting_current_observation |= source_gate == LaneCurrentGate::ObservationChanged',
            'self.note_current_observation(Some(&observed))',
            'self.poll_candidate()?',
            'self.service_native_ingress(receiver)?',
            'NativeSourceRequest::retire_closed_instance(',
            'if source_gate != LaneCurrentGate::ObservationChanged',
            'if let Some(source) = self.source.as_mut()',
            'source.poll(network, &self.guard, now, self.retransmit)?',
            'if let Some(prepared) = self.pending_ingress.take()',
            'self.consume_native_ingress(prepared, receiver)?',
            '.poll(&observed, now)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_process.rs',
        'method',
        'NativeRunnerProcess::service_sources',
        (
            'NativeSourceRequest::targets_instance',
            'self.driver.process().is_productive(id)',
            'let observed = if needs_current',
            'NativeSourceRequest::retire_closed_instance(',
            '== LaneCurrentGate::ObservationChanged',
            'self.awaiting_current_observation = true',
            '.source_recovery_target(id, observed.as_ref()?)?',
            'if let Some(mut source) = self.source.take()',
            'source.settle(&mut self.driver, services, &mut self.recovered_sources)',
            'Ok(false) => self.source = Some(source)',
            """self.source = Some(source);
                    return Err(error);""",
            'if self.source.is_some()',
            'services.native_source_requirement()',
            'instance.source_recovery_requirement()',
            'self.candidate_source_requirement()',
            'self.source = Some(NativeSourceRequest::new(',
            'services.current_archive_targets()',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_process.rs',
        'method',
        'NativeRunnerProcess::next_deadline',
        (
            'if self.awaiting_current_observation',
            'return None',
            """self.driver
            .next_deadline()""",
            '.and_then(NativeSourceRequest::next_deadline)',
            '.min()',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::new',
        (
            'let artifact = source.finality()',
            'subject: artifact.subject',
            'certificate: artifact.commit_qc.clone()',
            '&request.signature_preimage()',
            'authenticate_certified_body_request_with_validator_pops(',
            '&artifact.height_context',
            '&artifact.validator_set_pops',
            'request.request().clone()',
            '.chain(archives)',
            '.filter(|peer| peer != local)',
            '.collect::<BTreeSet<_>>()',
            'request: Some(request)',
            'next_retry: now',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::accept',
        (
            'let request = self.request.as_ref().ok_or_else(',
            'response.request_hash != request.request_hash() || self.response.is_some()',
            'request.authenticate_response(',
            '&self.source.finality().height_context',
            'Ok(response) => self.response = Some(response)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::poll',
        (
            'if self.response.is_some() || self.peers.is_empty() || now < self.next_retry',
            """guard
            .begin_fail_stop_operation()""",
            'let peer = &self.peers[self.cursor]',
            'self.returned.take().unwrap_or_else(|| Post {',
            'NetworkMessage::SumeragiBlock(Arc::clone(&self.message))',
            'network.post_recoverable(post, self.ticket.take())',
            'self.cursor = (self.cursor + 1) % self.peers.len()',
            """self.next_retry = if self.cursor == 0 {
                    deadline_after(now, retransmit)
                } else {
                    now
                };""",
            'NetworkActorAdmissionError::Backpressured',
            'NetworkActorAdmissionError::Closed',
            'NetworkActorAdmissionError::Rejected',
            'post.peer_id == *peer',
            'post.priority == Priority::High',
            'Arc::ptr_eq(message, &self.message)',
            'self.returned = Some(post)',
            'self.ticket = ticket',
            'if !exact',
            """if result.is_ok() {
            operation.complete();""",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::next_deadline',
        (
            '(self.response.is_none() && !self.peers.is_empty()).then_some(self.next_retry)',
        ),
    ),
)
NATIVE_RECOVERY_BINDINGS += (
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lane_process.rs',
        'struct',
        'LaneSourceRecoveryTarget',
        (
            'state_owner: crate::state::NativeLaneStateOwner',
            'verified: Arc<VerifiedLaneContext>',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lane_process.rs',
        'method',
        'LaneProcessOwner::source_recovery_target',
        (
            'let _lease = self.state.consensus_publication_lease()',
            'let Owner::Active(owner) = &self.entries.get(&id)?.owner else',
            'owner.source_recovery_requirement().is_none()',
            'owner.current_gate(&self.state, observed) != LaneCurrentGate::Current',
            'state_owner: owner.state_owner.clone()',
            'verified: Arc::clone(&owner.verified)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_lane_process.rs',
        'method',
        'LaneProcessOwner::source_recovery_target_gate',
        (
            'let _lease = self.state.consensus_publication_lease()',
            'if !target.state_owner.matches_state(&self.state)',
            'return LaneCurrentGate::ObservationChanged',
            'LaneInstance::gate_for(&target.verified, &self.state, observed)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'enum',
        'NativeSourceTarget',
        (
            'Instance(LaneSourceRecoveryTarget)',
            'Candidate',
            'Validation(Box<wire::BlockSubject>)',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::targets_instance',
        (
            'matches!(self.target, NativeSourceTarget::Instance(_))',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::retire_closed_instance',
        (
            'retained: &mut Option<Self>',
            'target: NativeSourceTarget::Instance(target)',
            'retained.as_ref()',
            'let Some(observed) = observed else',
            'return LaneCurrentGate::ObservationChanged',
            'process.source_recovery_target_gate(target, observed)',
            'if gate == LaneCurrentGate::InstanceClosed',
            'retained.take()',
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'method',
        'NativeSourceRequest::settle',
        (
            'match &self.target',
            'NativeSourceTarget::Instance(target)',
            '.complete_source_recovery(',
            'target.instance_id()',
            'NativeSourceTarget::Validation(subject)',
            'services.complete_native_source(**subject, request, response)',
            'self.request = Some(request)',
            'self.response = Some(response)',
        ),
    ),
)
NATIVE_RECOVERY_BINDINGS += (
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/native_process.rs",
        "method", "NativeRunnerProcess::note_current_observation",
        (
            "self.awaiting_current_observation =",
            "observed.is_none_or(|observed| !observed.is_current(&self.state))",
        ),
    ),
)
PASSIVE_RECOVERY_MODEL_BINDINGS += NATIVE_RECOVERY_BINDINGS

NATIVE_QUIET_LOOP_PREFIXES = (
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs',
        'run_lifecycle_active_height',
        """loop {
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            activated.into_clean_shutdown(&mut active_runner)?;
            return Ok(HeightRunOutcome::Shutdown);
        }
        let now = Instant::now();
        activated.with_runner_runtime(
            &mut active_runner,
            |_owner, _executor, services, _local_proposal| {
                native.take_service_publication(services);
                native.service_sources(services, now)?;
                Ok::<_, V2RunnerError>(())
            },
        )?;
        native.poll(native_global, native_network, now, receiver)?;
        liveness_watchdog.poll(now);""",
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs',
        'run_lifecycle_active_height',
        """loop {
                cleanup_supervisor.reap_finished();
                if output_guard.restart_required() {
                    return Err(V2RunnerError::RestartRequired);
                }
                if shutdown_signal.is_sent() {
                    activated.into_clean_shutdown(&mut active_runner)?;
                    return Ok(HeightRunOutcome::Shutdown);
                }
                let now = Instant::now();
                native.poll(native_global, native_network, now, receiver)?;
                liveness_watchdog.poll(now);""",
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs',
        'run_pending_active_height',
        """loop {
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            activated.into_clean_shutdown(&mut active_runner)?;
            return Ok(HeightRunOutcome::Shutdown);
        }
        liveness_watchdog.poll(Instant::now());
        activated.with_runner_runtime(&mut active_runner, |_executor, services| {
            native.take_service_publication(services);
            native.service_sources(services, Instant::now())
        })?;
        native.poll(native_global, native_network, Instant::now(), receiver)?;""",
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs',
        'run_pending_active_height',
        """loop {
            cleanup_supervisor.reap_finished();
            if output_guard.restart_required() {
                return Err(V2RunnerError::RestartRequired);
            }
            if shutdown_signal.is_sent() {
                activated.into_clean_shutdown(&mut active_runner)?;
                return Ok(HeightRunOutcome::Shutdown);
            }
            native.poll(native_global, native_network, Instant::now(), receiver)?;
            liveness_watchdog.poll(Instant::now());""",
    ),
)

NATIVE_QUIET_WAIT_CHECKS = (
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs',
        'run_lifecycle_active_height',
        """let native_wait = native.next_deadline().map_or(IDLE_POLL, |deadline| {
                deadline.saturating_duration_since(Instant::now()).min(IDLE_POLL)
            });
            let _ = wake_rx.recv_timeout(native_wait);""",
        8,
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs',
        'run_pending_active_height',
        """let _ = wake_rx.recv_timeout(IDLE_POLL);""",
        4,
    ),
)

NATIVE_SOURCE_RETIREMENT_TRANSITIONS = (
    (
        'crates/iroha_core/src/sumeragi/v2_runner/native_source.rs',
        'NativeSourceRequest::retire_closed_instance',
        """let Some(Self { target: NativeSourceTarget::Instance(target), .. }) = retained.as_ref() else {
            return LaneCurrentGate::Current;
        };
        let Some(observed) = observed else {
            return LaneCurrentGate::ObservationChanged;
        };
        let gate = process.source_recovery_target_gate(target, observed);
        if gate == LaneCurrentGate::InstanceClosed { retained.take(); }
        gate""",
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lane_process.rs',
        'LaneProcessOwner::source_recovery_target',
        """let Owner::Active(owner) = &self.entries.get(&id)?.owner else { return None; };
        if owner.source_recovery_requirement().is_none() || owner.current_gate(&self.state, observed) != LaneCurrentGate::Current { return None; }""",
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lane_process.rs',
        'LaneProcessOwner::source_recovery_target_gate',
        """let _lease = self.state.consensus_publication_lease();
        if !target.state_owner.matches_state(&self.state) { return LaneCurrentGate::ObservationChanged; }
        LaneInstance::gate_for(&target.verified, &self.state, observed)""",
    ),
)

NATIVE_SOURCE_RETIREMENT_TRANSITIONS += (
    (
        "crates/iroha_core/src/sumeragi/v2_runner/native_process.rs",
        "NativeRunnerProcess::next_deadline",
        """if self.awaiting_current_observation { return None; }
        self.driver.next_deadline()""",
    ),
)

NATIVE_RECOVERY_TRANSITIONS = (
    (
        "NativeSourceRequest::poll",
        """match network.post_recoverable(post, self.ticket.take()) {
            Ok(()) => {
                self.cursor = (self.cursor + 1) % self.peers.len();
                self.next_retry = if self.cursor == 0 {
                    deadline_after(now, retransmit)
                } else {
                    now
                };
                Ok(())
            }""",
    ),
    (
        "NativeSourceRequest::poll",
        """let exact = post.peer_id == *peer
                    && post.priority == Priority::High
                    && matches!(&post.data, NetworkMessage::SumeragiBlock(message) if Arc::ptr_eq(message, &self.message));
                self.returned = Some(post);
                self.ticket = ticket;
                if !exact {""",
    ),
)

PASSIVE_RECOVERY_ORDERED_CHECKS = (
    (
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'recover_finalized_lifecycle_equivocations',
        (
            'let view = state.view()',
            'for height in first_height..=height',
            '.v2_finality_artifact(height)',
            'recover_context_lifecycle_equivocations(',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'recover_context_lifecycle_equivocations',
        (
            'if &context.network_id != state.network_id_ref()',
            'LifecycleLedgerV1::read_completed_equivocations(',
            'retain_sumeragi_v2_equivocation(state, context, proofs_of_possession, proof)',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'retain_sumeragi_v2_equivocation',
        (
            'validate_v2_equivocation(&payload)?',
            'canonicalize_v2_equivocation_evidence(&payload)',
            'retain_validated_local_evidence(state, canonical)',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs',
        'method',
        'LifecycleLedgerV1::read_completed_equivocations',
        (
            'LifecycleLedgerStoreV1::read_existing(root, context)?',
            'record.terminal() != Some(Some(TerminalOutcome::Advanced))',
            '.into_equivocation_proof()',
            'proofs.push(proof)',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs',
        'method',
        'LifecycleLedgerStoreV1::read_existing',
        (
            'BoundLifecycleLedgerDirectory::bind(root, false)?',
            'let guard = directory.lock()?',
            '.read_bounded_locked(LEDGER_FILE, MAX_LEDGER_FRAME_BYTES)?',
            'decode_frame(&bytes, MAX_LEDGER_FRAME_BYTES)?',
            'if ledger.context() != context',
            'ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?',
            'Ok(Some(ledger))',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/mod.rs',
        'method',
        'SumeragiStartArgs::start',
        (
            'evidence::recover_finalized_lifecycle_equivocations(state.as_ref())',
            'FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(',
            'SumeragiWorker',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs',
        'method',
        'ProductionV2Services::start_with_apply_service',
        (
            '!state.matches_kura_instance(&kura)',
            '!apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)',
            'Self::start_inner(',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs',
        'method',
        'ProductionV2Services::start_inner',
        (
            'Self::freeze_timeout_certificate_targets(',
            'super::evidence::recover_context_lifecycle_equivocations(',
            '            state.as_ref(),\n            &context,\n            &validator_set_pops,\n        )?;',
            'let io = V2IoHandle::spawn(',
        ),
    ),

    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::service_next_historical_recovery_at_with_archive_targets",
        (
            "persist_historical_recovery_session(&session)",
            "historical_recovery_diagnostics.complete(identity)",
            "retire_historical_recovery_request(identity)",
            ".observe(identity, reason)",
            "schedule_historical_recovery_request(",
            "historical_recovery_sessions.push_back(session)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::schedule_historical_recovery_request",
        (
            "let retained_request_matches",
            "existing.request == request",
            "existing.request_hash == request_hash",
            "existing.cadence.reason == observation.reason",
            "self.retire_historical_recovery_request(identity)",
            "HistoricalRecoveryRequestCadence::immediate(observation.reason, now)",
            "if !cadence.due(now)",
            ".after_retained_attempt(",
            "let mut scheduled_destinations = BTreeSet::new()",
            "if !self.push_effect(",
            "scheduled_destinations.insert(peer)",
            "if !scheduled_destinations.is_empty()",
            "owner.cadence = next_cadence",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "run_inner",
        (
            "public_key: genesis_public_key",
            "block_cadence",
            "sumeragi_v2_timing_ms(block_cadence_ms)",
            "let round_timeout = Duration::from_millis(round_timeout_ms)",
            "let retransmit_interval = Duration::from_millis(retransmit_interval_ms)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_kura_lifecycle_height",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_lifecycle_active_height",
        (
            "native.take_service_publication(services)",
            "native.service_sources(services, now)?",
            "native.poll(native_global, native_network, now, receiver)?",
            "dispatch_queue_plan_admission_effects(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_active_height",
        (
            "native.take_service_publication(services)",
            "native.service_sources(services, Instant::now())",
            "native.poll(native_global, native_network, Instant::now(), receiver)?",
            "dispatch_queue_plan_admission_effects(",
        ),
    ),
    (
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "ExecutionStatus::StateAppliedByCanonicalBlock",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
            "lane_block_execution_preflight_has_rejections_without_sidecar_repair",
            "lane_block_execution_input_available_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
)

PASSIVE_RECOVERY_ORDERED_CHECKS += (
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "reservation_reconciliation_pending = false;",
            "reconcile_executor_locked_body(executor, services)?",
            "preactivation.initialize_recovered_local_proposal(setup_runner)?",
            "preactivation.activate(height_started_at, local_proposal)?",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/native_process.rs",
        "method",
        "NativeRunnerProcess::poll",
        (
            "self.service_native_ingress(receiver)?",
            "NativeSourceRequest::retire_closed_instance(",
            "source_gate != LaneCurrentGate::ObservationChanged",
            "source.poll(network, &self.guard, now, self.retransmit)?",
            "let Some(observed) = observed else",
            ".poll(&observed, now)",
            "self.note_current_observation(Some(&observed))",
        ),
    ),
)

PASSIVE_RECOVERY_ORDERED_CHECKS += (
    (
        "crates/iroha_core/src/sumeragi/v2_runner/native_process.rs",
        "method", "NativeRunnerProcess::service_sources",
        (
            "let observed = if needs_current",
            "NativeSourceRequest::retire_closed_instance(",
            "== LaneCurrentGate::ObservationChanged",
            "if let Some(mut source) = self.source.take()",
            "source.settle(&mut self.driver, services, &mut self.recovered_sources)",
            ".source_recovery_target(id, observed.as_ref()?)?",
            "NativeSourceTarget::Instance(target)",
            "self.source = Some(NativeSourceRequest::new(",
        ),
    ),
)

PASSIVE_RECOVERY_FORBIDDEN_CHECKS = (
    (
        'crates/iroha_core/src/sumeragi/evidence.rs',
        'fn',
        'recover_finalized_lifecycle_equivocations',
        (
            'sumeragi_v2_context',
            'context_store',
            'persist(',
            'remove_file(',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs',
        'method',
        'LifecycleLedgerV1::read_completed_equivocations',
        (
            'PendingRuntimeEffectBinding',
            'InitialLifecycleState::Ready',
            '.persist(',
            'finish_terminal(',
            '::open(',
        ),
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs',
        'method',
        'LifecycleLedgerStoreV1::read_existing',
        (
            'open_or_create(',
            'load_with_frame_presence',
            'remove_stale_temporary_locked(',
            'remove_file(',
            'create_dir',
            '.persist(',
        ),
    ),

    (
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
            ".read_lane_block_execution_input(",
            ".read_lane_block_execution_preflight(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::committed_lane_block_status_snapshot",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
            ".read_lane_block_execution_input(",
            ".read_lane_block_execution_preflight(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "method",
        "State::native_amx_participant_applications_diagnostics_once",
        (
            ".merge_entry_by_hash(",
            ".latest_certified_lane_block_artifacts_matching(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "method",
        "State::autonomous_lane_execution_diagnostics_once",
        (
            ".latest_certified_lane_block_artifacts_matching(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_torii/src/routing.rs",
        "fn",
        "handle_v1_sumeragi_diagnostics",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
        ),
    ),
)

PASSIVE_RECOVERY_INCLUDE_RELATIONS = (
    (
        'crates/iroha_core/src/sumeragi/mod.rs',
        'pub(crate) mod evidence;',
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs',
        '#[path = "v2_lifecycle_ledger.rs"]\nmod ledger;',
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs',
        '#[path = "v2_lifecycle_replay_authority.rs"]\n#[cfg_attr(not(test), allow(dead_code))]\nmod replay_authority;',
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs',
        'include!("v2_lifecycle_ledger_store.rs");',
    ),
    (
        'crates/iroha_core/src/sumeragi/v2_worker.rs',
        'include!("v2_worker_services_impl.rs");',
    ),
    (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/autonomous_application_evidence.rs");',
    ),
    (
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        'include!("passive_diagnostic_reads.rs");',
    ),
    (
        "crates/iroha_core/src/state.rs",
        'include!("state/passive_lane_diagnostic_methods.rs");',
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        'include!("v2_runner/canonical_recovery_ingress.rs");',
    ),
)

PASSIVE_RECOVERY_RAW_TEST_CHECKS = (
    (
        'crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs',
        'completed_equivocation_recovers_into_new_state_without_reopening_lifecycle',
        (
            'complete_cold_evidence_report(&fixture, &original)',
            'cold_evidence_finality(&fixture, &kura)',
            'drop(original)',
            'let cold = cold_evidence_state(',
            'assert!(cold.sumeragi_v2_pending_evidence.lock().is_empty())',
            'evidence::recover_finalized_lifecycle_equivocations(&cold).unwrap()',
            'evidence::validate_v2_evidence_admissions(&cold, 2, &selected).unwrap()',
            'assert!(cold.world.consensus_evidence.view().iter().next().is_none())',
            'assert_eq!(cold.state_view_generation(), generation)',
            'snapshot_files(&root),\n        before,',
        ),
    ),
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_native_diagnostic_tests.rs",
        "assert_passive_state_diagnostics",
        (
            "std::fs::rename(&ownership_data, &ownership_data_temp)",
            "let passive_revision = kura.committed_lane_status_revision()",
            "for _ in 0..2",
            "state.durable_lane_diagnostics()",
            "native_amx_participant_applications_diagnostics()",
            "autonomous_lane_execution_diagnostics()",
            "assert!(!ownership_data.exists())",
            "kura.committed_lane_status_revision()",
            "kura.recover_lane_block_payload(&session.proposal)",
            "assert!(ownership_data.is_file())",
        ),
    ),
    (
        "crates/iroha_torii/src/tests/routing.rs",
        "permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state",
        (
            "install_passive_diagnostic_lane_artifact",
            "std::fs::rename(&ownership_data, &ownership_data_temp)",
            "for _ in 0..2",
            "handle_v1_sumeragi_diagnostics(",
            "assert!(!ownership_data.exists())",
            "kura.recover_lane_block_payload(&proposal)",
            "assert!(ownership_data.is_file())",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/historical_recovery_and_carrier_tests.rs",
        "historical_missing_canonical_block_schedules_authenticated_retry_then_completes",
        (
            "first_cadence.retained_attempts, 1",
            "service_next_historical_recovery_at(before_deadline)",
            "must not fan out",
            "a full effect queue must not advance the retry deadline",
            "retry must preserve the exact peer order and request bytes",
            "second_cadence.retained_attempts, 2",
            "the next deadline is anchored at the service turn",
            "local completion is never gated by the network deadline",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_runner_upstream_recovery.rs",
        "quiet_retransmission_tick_services_one_retained_historical_session",
        (
            "quiet_historical_recovery_fixture",
            "service_historical_recovery_tick",
            "CanonicalBlockPending",
            "has_pending_historical_recovery",
        ),
    ),
)

PASSIVE_RECOVERY_RAW_TEST_CHECKS += (
    (
        "crates/iroha_core/src/state/lane_driver_tests.rs",
        "native_source_retirement_fixture",
        (
            "native_process_fixture(false, now)",
            "NativeSourceRequestTestProbe::instance(",
            ".assert_observation_deadline(",
            "retained.backpressure()",
            "NativeSourceRequestTestProbe::non_instance(",
            "driver.hold_next_body_completion_for_test(",
            "native_process_advance(&fixture, false)",
            "native_process_advance(&fixture, true)",
            "ticket.ticket_drop_cancellations()",
            "body.requires_recovery()",
            "guard.restart_required()",
        ),
    ),
)

PASSIVE_RECOVERY_SOURCE_RELATIVES = tuple(
    Path(relative)
    for relative in sorted(
        {
            *(binding[1] for binding in PASSIVE_RECOVERY_MODEL_BINDINGS),
            *(check[0] for check in PASSIVE_RECOVERY_INCLUDE_RELATIONS),
            *(check[0] for check in PASSIVE_RECOVERY_RAW_TEST_CHECKS),
            PASSIVE_RECOVERY_CONTRACT_RELATIVE.as_posix(),
            PASSIVE_RECOVERY_TEST_RELATIVE.as_posix(),
        }
    )
)

RustBindingItem = Callable[
    [Path, str, str, str, str, list[str]], Optional[str]
]


def _models_by_name(models: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(models, list):
        return {}
    return {
        model["module"]: model
        for model in models
        if isinstance(model, dict) and isinstance(model.get("module"), str)
    }


def _binding_items(
    root: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> dict[tuple[str, str, str], str]:
    by_name = _models_by_name(models)
    items: dict[tuple[str, str, str], str] = {}
    for module, relative, kind, symbol, expected_tokens in (
        PASSIVE_RECOVERY_MODEL_BINDINGS
    ):
        model = by_name.get(module)
        bindings = model.get("production_symbols") if model is not None else None
        matches = [
            binding
            for binding in bindings or ()
            if isinstance(binding, dict)
            and binding.get("path") == relative
            and binding.get("kind") == kind
            and binding.get("symbol") == symbol
        ]
        if len(matches) != 1:
            errors.append(
                f"{module}: passive/recovery binding {relative}!{symbol} must "
                f"occur exactly once, found {len(matches)}"
            )
        elif tuple(matches[0].get("required_tokens", ())) != expected_tokens:
            errors.append(
                f"{module}: passive/recovery tokens changed for {relative}!{symbol}"
            )
        key = (relative, kind, symbol)
        if key in items:
            continue
        item = rust_binding_item(
            root, relative, kind, symbol, "passive/recovery source binding", errors
        )
        if item is None:
            continue
        items[key] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: passive/recovery item {symbol} is "
                    f"missing source-bound token {token!r}"
                )
    return items


def _item_for(
    root: Path,
    items: dict[tuple[str, str, str], str],
    relative: str,
    kind: str,
    symbol: str,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> Optional[str]:
    return items.get((relative, kind, symbol)) or rust_binding_item(
        root, relative, kind, symbol, "passive/recovery relation", errors
    )


def _validate_source_relations(
    root: Path,
    items: dict[tuple[str, str, str], str],
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> None:
    for relative, kind, symbol, tokens in PASSIVE_RECOVERY_ORDERED_CHECKS:
        item = _item_for(
            root, items, relative, kind, symbol, errors, rust_binding_item
        )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: passive/recovery item {symbol} "
                    f"is missing or reorders token {token!r}"
                )
                break
            cursor = position

    # The lifecycle functions contain additional timing uses after the
    # constructor call, so a whole-item subsequence check can accidentally
    # match those later occurrences after the constructor arguments are
    # reordered.  Bind the complete, whitespace-insensitive call instead: the
    # retry floor and ceiling must be derived from retransmission and round
    # timing in this exact order in both startup corridors.
    lane_work_limits_call = "".join(
        """
        let lane_work_limits = lane_work_limits(
            &shared_config,
            network.reply_route_source_capacity(),
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            retransmit_interval,
            round_timeout,
        )?;
        """.split()
    )
    for relative, symbol in (
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "run_non_pending_lifecycle_loop",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "run_pending_kura_lifecycle_height",
        ),
    ):
        item = _item_for(
            root, items, relative, "fn", symbol, errors, rust_binding_item
        )
        if item is None:
            continue
        if "".join(item.split()).count(lane_work_limits_call) != 1:
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} is "
                "missing or reorders the exact lane_work_limits source-bound "
                "token sequence"
            )

    for relative, kind, symbol, tokens in PASSIVE_RECOVERY_FORBIDDEN_CHECKS:
        item = _item_for(
            root, items, relative, kind, symbol, errors, rust_binding_item
        )
        if item is None:
            continue
        for token in tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: passive diagnostic item {symbol} "
                    f"contains repair-capable token {token!r}"
                )

    for relative, symbol, transition in NATIVE_SOURCE_RETIREMENT_TRANSITIONS:
        item = _item_for(root, items, relative, "method", symbol, errors, rust_binding_item)
        if item is None:
            continue
        compact = "".join(_mask_rust_comments(item).split())
        if (compact.count("".join(transition.split())) != 1
                or (symbol == "NativeSourceRequest::retire_closed_instance"
                    and compact.count("retained.take()") != 1)):
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} must preserve "
                "the exact authenticated source-retirement transition"
            )

    for symbol, transition in NATIVE_RECOVERY_TRANSITIONS:
        relative = "crates/iroha_core/src/sumeragi/v2_runner/native_source.rs"
        item = _item_for(root, items, relative, "method", symbol, errors, rust_binding_item)
        if item is None:
            continue
        # Mask comments with the authenticated shared lexer; whitespace does
        # not carry Rust semantics, but the branch and statements do.
        if "".join(transition.split()) not in "".join(_mask_rust_comments(item).split()):
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} must preserve "
                "the exact retained Native retry transition"
            )

    # Both outer loops select/settle sources before ingress, and both finalized
    # drains continue polling retained requests and physical Native work. Match
    # each reviewed loop prefix rather than borrowing a call from another loop.
    for relative, symbol, prefix in NATIVE_QUIET_LOOP_PREFIXES:
        item = _item_for(root, items, relative, "fn", symbol, errors, rust_binding_item)
        if item is not None and "".join(item.split()).count("".join(prefix.split())) != 1:
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} must preserve "
                "the exact Native quiet-loop prefix"
            )

    for relative, symbol, wait, expected_count in NATIVE_QUIET_WAIT_CHECKS:
        item = _item_for(root, items, relative, "fn", symbol, errors, rust_binding_item)
        if item is None:
            continue
        compact = "".join(item.split())
        if (compact.count("".join(wait.split())) != expected_count
                or len(re.findall(r"\bwake_rx\s*\.\s*recv(?:_timeout)?\s*\(", item)) != expected_count):
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} must preserve "
                f"all {expected_count} bounded Native quiet waits"
            )


def _validate_includes(root: Path, errors: list[str]) -> None:
    for relative, token in PASSIVE_RECOVERY_INCLUDE_RELATIONS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read passive provider include: {error}")
            continue
        if source.count(token) != 1:
            errors.append(
                f"{path}: passive provider include {token!r} must occur exactly once"
            )


def _validate_raw_tests(
    root: Path, errors: list[str], rust_binding_item: RustBindingItem
) -> None:
    for relative, symbol, tokens in PASSIVE_RECOVERY_RAW_TEST_CHECKS:
        item = rust_binding_item(
            root,
            relative,
            "fn",
            symbol,
            "passive/recovery focused Rust control",
            errors,
        )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: passive/recovery focused control "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position


def validate_passive_recovery_contract(
    root: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> None:
    """Validate passive diagnostics and deadline-driven recovery bindings."""

    items = _binding_items(root, models, errors, rust_binding_item)
    _validate_source_relations(root, items, errors, rust_binding_item)
    _validate_includes(root, errors)
    _validate_raw_tests(root, errors, rust_binding_item)
