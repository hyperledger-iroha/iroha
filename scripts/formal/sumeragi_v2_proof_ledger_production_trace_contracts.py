# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# Canonical one-to-one operational-correspondence mapping. The source snapshot
# below proves that every model action appears exactly once, every Rust action
# tag and numeric discriminant appears exactly once, and every mapped tag is an
# arm of the shared composed transition kernel. Multiple concrete production
# call sites may refine one action, but they cannot mint a second mapping.
PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS = (
    ("SelectQueuePlanV4Conjunction", "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4", 1),
    ("FsyncReservationV5", "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5", 2),
    ("ActivateKura", "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA", 3),
    ("FanoutFromProducer", "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER", 4),
    ("ServeLateBody", "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY", 5),
    ("PersistExecutionInput", "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT", 6),
    ("AuthorizeReady", "IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY", 7),
    ("SignReady", "IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY", 8),
    ("PersistReadyQc", "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC", 9),
    ("Crash", "IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH", 10),
    ("Recover", "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER", 11),
    (
        "RecoverReservationSnapshot",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
        25,
    ),
    (
        "ReleaseReservationDirect",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
        26,
    ),
    (
        "RehydrateLocalKuraCustody",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY",
        27,
    ),
    ("LaneCommit", "IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT", 12),
    ("ApplyCarrier", "IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER", 13),
    (
        "PersistReservationCommitted",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED",
        14,
    ),
    (
        "PersistPlanTombstone",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE",
        15,
    ),
    (
        "ForgetReservationCommit",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT",
        16,
    ),
    (
        "PersistKuraRetirement",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT",
        17,
    ),
    (
        "AdvanceReleasePendingPrefix",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING",
        18,
    ),
    (
        "PrepareReservationRelease",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE",
        19,
    ),
    (
        "AdvanceReleasedPrefix",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED",
        20,
    ),
    (
        "CompleteReservationRelease",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE",
        21,
    ),
    (
        "RestoreReleasedFifo",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO",
        22,
    ),
    (
        "ForgetReservationRelease",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE",
        23,
    ),
    (
        "RepairPostCarrierEvidence",
        "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        24,
    ),
)

def _retained_effect_frontier_adapter_contracts(
    effects_path: Path,
    source: str,
    generic_executor_context: tuple[tuple[str, ...], ...],
    errors: list[str],
) -> None:
    """Bind the test adapter and parked-debt frontier semantics independently."""

    retain_wrapper = _require_rust_item(
        effects_path, source, "retain_effect_batch", errors
    )
    _require_rust_item_context(
        effects_path,
        retain_wrapper,
        generic_executor_context,
        "test-only retained effect batch frontier adapter",
        errors,
        expected_attributes=("#[cfg(test)]",),
    )
    _require_rust_token_sequence(
        effects_path,
        retain_wrapper,
        """
let frontier = self
    .runtime
    .reconciliation_frontier()
    .map_err(EffectExecutorError::Runtime)?;
self.retain_effect_batch_at_frontier(effects, ownership, frontier)
""",
        "test-only retained effect adapter must delegate with the exact runtime frontier",
        errors,
    )
    preflight = _require_rust_item(
        effects_path, source, "preflight_effect_batch_frontier", errors
    )
    _require_rust_item_context(
        effects_path,
        preflight,
        generic_executor_context,
        "retained effect frontier preflight",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        preflight,
        """
let entering_view = Self::entering_view_tag(effects)?;
if entering_view.is_some()
    && !matches!(effects.first(), Some(AdapterEffect::EnterView { .. }))
{
    return Err(EffectExecutorError::Contract(
        "EnterView must be the first effect in its reducer macro-step".to_owned(),
    ));
}
""",
        "frontier preflight must require the unique EnterView at the batch head",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        preflight,
        """
(Some(current), Some(next)) if next.strictly_advances(current) => {
    if entering_view != Some(next) {
        return Err(EffectExecutorError::Contract(
            "an advancing reducer frontier omitted its leading EnterView".to_owned(),
        ));
    }
}
""",
        "frontier preflight must bind every strict reducer advance to its leading EnterView",
        errors,
    )
    prepare_parked = _require_rust_item(
        effects_path, source, "prepare_parked_effects_for_frontier", errors
    )
    _require_rust_item_context(
        effects_path,
        prepare_parked,
        generic_executor_context,
        "parked effect frontier preflight and retirement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        prepare_parked,
        "let entering_view = self.preflight_effect_batch_frontier(effects, frontier)?;",
        "parked effect retirement must reuse the exact batch frontier preflight",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        prepare_parked,
        """
if entering_view
    .is_some_and(|tag| Self::parked_effect_is_retired_by_view(owned, tag))
{
    return false;
}
if let Some(decision) = frontier.decision
    && !Self::effect_survives_decision(&owned.effect, decision)
{
    return false;
}
if let Some((superseded, replacement)) = lock_transition
    && Self::adapter_effect_body_key(&owned.effect).is_some_and(|key| {
        protected_lock_retires_body_key(superseded, replacement, key)
    })
{
    return false;
}
true
""",
        "parked effect retirement must preserve only view-, decision-, and protected-lock-live debt",
        errors,
    )

# The production multilane trace theorem depends on safety, durability, and
# deterministic-execution obligations only. Generic partial-synchrony and
# leader-progress liveness obligations remain part of the strict whole-
# Sumeragi release ledger, but must not make this independently scoped theorem
# impossible to certify. Keep this dependency list exact and source ordered.
PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES = (
    ("dual-quorum-definition", "tlaps_proved"),
    ("quorum-honest-intersection", "tlaps_proved"),
    ("durable-vote-append-kernel", "tlaps_proved"),
    ("validity-availability-kernel", "tlaps_proved"),
    ("durable-vote-uniqueness", "tlaps_proved"),
    ("external-validity", "tlaps_proved"),
    ("certified-body-availability", "tlaps_proved"),
    ("certificate-uniqueness", "tlaps_proved"),
    ("agreement", "tlaps_proved"),
    ("no-conflicting-commit-qcs", "tlaps_proved"),
    ("chain-prefix", "tlaps_proved"),
    ("crash-restart", "tlaps_proved"),
    ("epoch-boundary", "tlaps_proved"),
    ("cryptography", "trusted_contract"),
    ("durability-system-call", "trusted_contract"),
    ("deterministic-execution", "trusted_contract"),
)

PRODUCTION_TRACE_EXTRACTION_BINDINGS = (
    {
        "id": "queue_plan_selection_and_reservation_fsync",
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "reserve_transactions_for_lane_bounded",
        "model_actions": (
            "SelectQueuePlanV4Conjunction",
            "FsyncReservationV5",
        ),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5",
        ),
        "checked_transition_count": 2,
        "additional_tokens": (
            "AutonomousLaneReservationSelectionAuthorization",
            "MAX_MERGE_EXECUTION_ENTRYPOINTS",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "put_batch",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/lane_planner.rs",
            "impl": "AutonomousLaneReservationSlotPlan",
            "symbol": "selection_authorization",
            "required_tokens": (
                "validator_set",
                "author",
                "checked_shl",
                "reservation_scope",
            ),
        },
    },
    {
        "id": "reservation_cleanup_prefixes",
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "commit_lane_reservation",
        "model_actions": (
            "PersistReservationCommitted",
            "PersistPlanTombstone",
            "ForgetReservationCommit",
        ),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT",
        ),
        "checked_transition_count": 3,
        "additional_tokens": (
            "forget_commit",
            "remove_plan_journal_for_reservation_commit",
            "LaneQueueReservationGroupIdentityV1::from_key",
            "cleanup_gate.authenticates_applied_group",
        ),
        "commit_sink": {
            "path": "crates/iroha_core/src/queue/canonical_terminal_cleanup.rs",
            "impl": "Queue",
            "symbol": "commit_prepared_lane_reservation_carriers",
            "required_tokens": (
                "validate_lane_queue_carrier_cleanup_batch_bounds",
                "LaneQueueReservationGroupIdentityV1::from_key",
                "cleanup_gate.authenticates_applied_group",
                "begin_durability_transition_locked",
                "preflight_lane_reservation_plan_journal",
                "commit_lane_reservation",
            ),
            "ordered_tokens": (
                "validate_lane_queue_carrier_cleanup_batch_bounds(",
                "for group in carriers.iter().flatten()",
                "cleanup_gate.authenticates_applied_group(group.group_binding)",
                "begin_durability_transition_locked(",
                "preflight_lane_reservation_plan_journal(&journal_preflight)",
                "for group in carriers.into_iter().flatten()",
                "self.commit_lane_reservation(",
                "drop(durability_transition)",
            ),
        },
    },
    {
        "id": "pre_kura_direct_reservation_release",
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "release_pre_kura_autonomous_reservation_batch",
        "model_actions": ("ReleaseReservationDirect",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "PreKuraDirectReleaseContext",
            "revalidate_complete_live_pre_kura_group_locked",
            "canonical_lane_queue_reservation_group_identity_projection",
            "production_in_flight_first_release_terminal_owner",
            "begin_durability_transition_locked",
            "journal.release_batch",
            "replace_fifo_locked",
        ),
    },
    {
        "id": "producer_kura_activation",
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "V2LaneWorkAdapter",
        "symbol": "drive_pending_autonomous_reservation_batch",
        "model_actions": ("ActivateKura",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "authorize_lane_reservation_kura_activation",
            "height_context_id",
            "AutonomousLifecycleAttemptBindingV1::from_payload",
            "reservation_group",
            "canonical_lane_queue_reservation_group_identity_projection",
            "replicated_carrier_owners: validator_mask & !producer",
            "persist_autonomous_lifecycle_bootstrap",
            "authenticate_autonomous_lifecycle_bootstrap_recovery",
            "complete_autonomous_lifecycle_bootstrap",
            "insert_autonomous_lane_payload",
            "fanout_producer_lane_executable_payload",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/queue.rs",
            "impl": "Queue",
            "symbol": "authorize_lane_reservation_kura_activation",
            "required_tokens": (
                "revalidate_complete_live_pre_kura_group_locked",
                "begin_durability_transition_locked",
                "AutonomousLaneKuraActivationAuthorization",
                "authorization.height_context_id()",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "complete_autonomous_lifecycle_bootstrap",
            "required_tokens": (
                "let AutonomousLifecycleBootstrapCompletionPermit { authority, fence } = permit",
                "revalidate_autonomous_lifecycle_bootstrap_for_completion",
                "persist_lane_executable_payload_impl",
                "publish_autonomous_lifecycle_bootstrap_cursor_stage",
                "AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable",
                "AutonomousLifecycleBootstrapRecoveryStage::LiveDurable",
                "delete_completed_autonomous_lifecycle_bootstrap",
                "read_autonomous_lifecycle_cursor",
                "cursor_read.cursor() != Some(&authority.bootstrap.body.live_activate)",
                "lane_block_application_receipt_available",
                "consume_autonomous_lifecycle_bootstrap_completion_fence",
                "AutonomousLifecycleBootstrapCompletionOutcome::AlreadyTerminal",
                "AutonomousLifecycleBootstrapCompletionOutcome::Completed",
            ),
            "ordered_tokens": (
                "let AutonomousLifecycleBootstrapCompletionPermit { authority, fence } = permit; let revalidation = self.revalidate_autonomous_lifecycle_bootstrap_for_completion(authority)",
                "let payload_outcome = self.persist_lane_executable_payload_impl",
                "let prepared_outcome = self.publish_autonomous_lifecycle_bootstrap_cursor_stage",
                "if !matches!(authority.stage, AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable | AutonomousLifecycleBootstrapRecoveryStage::LiveDurable)",
                "let live_outcome = self.publish_autonomous_lifecycle_bootstrap_cursor_stage",
                "if authority.stage != AutonomousLifecycleBootstrapRecoveryStage::LiveDurable",
                "delete_completed_autonomous_lifecycle_bootstrap",
                "let cursor_read = self.read_autonomous_lifecycle_cursor",
                "if cursor_read.cursor() != Some(&authority.bootstrap.body.live_activate)",
                "self.lane_block_application_receipt_available(&payload.origin_proposal)",
                "Self::consume_autonomous_lifecycle_bootstrap_completion_fence(fence)",
                "AutonomousLifecycleBootstrapCompletionOutcome::AlreadyTerminal",
                "AutonomousLifecycleBootstrapCompletionOutcome::Completed",
            ),
        },
    },
    {
        "id": "startup_generation_crash_cas",
        "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "impl": None,
        "symbol": "check_production_in_flight_first_release_crash_transition",
        "model_actions": ("Crash",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "after.session.crashed |= actor",
            "after.session.bodies &= !actor",
            "after.session.ready_authorized &= !actor",
            "after.session.producer_alive = before.session.producer_alive && actor != before.producer",
            "check_derived_production_in_flight_first_release_transition",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "signed lifecycle CAS cursor constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
                "impl": None,
                "symbol": "sign_lifecycle_cursor",
                "required_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "previous_cursor_hash",
                    "signing_preimage",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
                "ordered_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "signing_preimage()",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
            },
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "reconcile_autonomous_lifecycle_startup",
            "required_tokens": (
                "into_reconciliation_receipt",
                "let mut recovered_attempts = 0_usize",
                "recover_one_attempt",
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "RecoveredAutonomousLifecycleStartup",
            ),
            "ordered_tokens": (
                "let receipt = if let Some(recovery) = recovery_authorization { recovery.into_reconciliation_receipt()",
                "let mut recovered_attempts = 0_usize",
                "if recover_one_attempt(",
                "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
                "Ok(RecoveredAutonomousLifecycleStartup { snapshot, receipt, deferred_terminal_recovery, completed_bootstraps, recovered_attempts, })",
            ),
        },
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "recover_one_attempt",
            "required_tokens": (
                "owner_generation < current_generation",
                "check_production_in_flight_first_release_crash_transition",
                "AutonomousLifecycleCursorPhaseV2::crashed",
                "compare_and_swap_phase",
            ),
            "ordered_tokens": (
                "if owner_generation < current_generation",
                "check_production_in_flight_first_release_crash_transition",
                "AutonomousLifecycleCursorPhaseV2::crashed(",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "compare_and_swap_phase",
            "required_tokens": (
                "read.into_parts",
                "checked_add(1)",
                "previous_cursor_hash",
                "sign_lifecycle_cursor",
                "compare_and_swap_autonomous_lifecycle_cursor",
            ),
            "ordered_tokens": (
                "let (current, lease) = read.into_parts()",
                "let sequence = current",
                "let previous_cursor_hash = current.as_ref().map",
                "let binding = current",
                "let next = sign_lifecycle_cursor(",
                "kura.compare_and_swap_autonomous_lifecycle_cursor(lease, next)",
            ),
        },
    },
    {
        "id": "startup_generation_recover_cas",
        "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "impl": None,
        "symbol": "check_production_in_flight_first_release_recover_transition",
        "model_actions": ("Recover",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "after.session.crashed &= !actor",
            "check_derived_production_in_flight_first_release_transition",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "signed lifecycle CAS cursor constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
                "impl": None,
                "symbol": "sign_lifecycle_cursor",
                "required_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "previous_cursor_hash",
                    "signing_preimage",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
                "ordered_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "signing_preimage()",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
            },
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "reconcile_autonomous_lifecycle_startup",
            "required_tokens": (
                "into_reconciliation_receipt",
                "let mut recovered_attempts = 0_usize",
                "recover_one_attempt",
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "RecoveredAutonomousLifecycleStartup",
            ),
            "ordered_tokens": (
                "let receipt = if let Some(recovery) = recovery_authorization { recovery.into_reconciliation_receipt()",
                "let mut recovered_attempts = 0_usize",
                "if recover_one_attempt(",
                "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
                "Ok(RecoveredAutonomousLifecycleStartup { snapshot, receipt, deferred_terminal_recovery, completed_bootstraps, recovered_attempts, })",
            ),
        },
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "recover_one_attempt",
            "required_tokens": (
                "AutonomousLifecycleCursorPhaseKindV2::Crashed",
                "check_production_in_flight_first_release_recover_transition",
                "AutonomousLifecycleCursorPhaseV2::prepared(current_generation, recover)",
                "compare_and_swap_phase",
            ),
            "ordered_tokens": (
                "AutonomousLifecycleCursorPhaseKindV2::Crashed =>",
                "check_production_in_flight_first_release_recover_transition",
                "let phase = AutonomousLifecycleCursorPhaseV2::prepared(current_generation, recover).map_err(|reason| lifecycle_error(\"\", reason))?; let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "compare_and_swap_phase",
            "required_tokens": (
                "read.into_parts",
                "checked_add(1)",
                "previous_cursor_hash",
                "sign_lifecycle_cursor",
                "compare_and_swap_autonomous_lifecycle_cursor",
            ),
            "ordered_tokens": (
                "let (current, lease) = read.into_parts()",
                "let sequence = current",
                "let previous_cursor_hash = current.as_ref().map",
                "let binding = current",
                "let next = sign_lifecycle_cursor(",
                "kura.compare_and_swap_autonomous_lifecycle_cursor(lease, next)",
            ),
        },
    },
    {
        "id": "startup_snapshot_recovery_authorization",
        "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "impl": None,
        "symbol": "check_production_in_flight_first_release_recover_reservation_snapshot_transition",
        "model_actions": ("RecoverReservationSnapshot",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "check_derived_production_in_flight_first_release_transition",
            "before,",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "configured-role lifecycle process-generation claim",
                "path": "crates/iroha_core/src/sumeragi/v2_runner.rs",
                "impl": None,
                "symbol": "claim_runner_lifecycle_process_generation",
                "required_tokens": (
                    "context: &wire::HeightContext",
                    "match role",
                    "NodeRole::Observer => Ok(None)",
                    "NodeRole::Validator =>",
                    "kura.claim_autonomous_lifecycle_process_generation",
                    "context.network_id",
                    "local_peer",
                    ".map(Some)",
                ),
                "forbidden_tokens": (
                    "context.roster",
                    "local_validator_index",
                    "_initial_local_validator",
                    "context.chain_id",
                    "lifecycle_chain_id",
                    "Hash::new",
                    "NetworkId::default",
                    "Default::default",
                    "synthetic_network_id",
                ),
                "ordered_tokens": (
                    "context: &wire::HeightContext",
                    "match role",
                    "NodeRole::Observer => Ok(None)",
                    "NodeRole::Validator =>",
                    "kura.claim_autonomous_lifecycle_process_generation(",
                    "context.network_id, local_peer)",
                    ".map(Some)",
                ),
            },
            {
                "role": "typed Kura lifecycle process-generation claim propagation",
                "path": "crates/iroha_core/src/kura.rs",
                "impl": "Kura",
                "symbol": "claim_autonomous_lifecycle_process_generation",
                "required_tokens": (
                    "network_id: iroha_data_model::NetworkId",
                    "Result<AutonomousLifecycleProcessGenerationClaim>",
                    "claim.network_id != network_id",
                    "AutonomousLifecycleProcessGenerationRecordV1::new",
                    "let claim = AutonomousLifecycleProcessGenerationClaim { store_root: self.store_root.clone(), network_id, local_peer_id: local_peer_id.clone(), generation, record_hash: next.record_hash, }",
                    "self.validate_autonomous_lifecycle_process_generation_claim(&claim)",
                    "Ok(claim)",
                ),
                "forbidden_tokens": (
                    "NetworkId::default",
                    "Default::default",
                    "synthetic_network_id",
                    "context.chain_id",
                ),
                "ordered_tokens": (
                    "network_id: iroha_data_model::NetworkId",
                    "Result<AutonomousLifecycleProcessGenerationClaim>",
                    "if claim.network_id != network_id",
                    "AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer_id.clone(), generation,)",
                    "let claim = AutonomousLifecycleProcessGenerationClaim { store_root: self.store_root.clone(), network_id, local_peer_id: local_peer_id.clone(), generation, record_hash: next.record_hash, }",
                    "self.validate_autonomous_lifecycle_process_generation_claim(&claim)",
                    "Ok(claim)",
                ),
            },
            {
                "role": "runner lifecycle process-generation branch handoff",
                "path": "crates/iroha_core/src/sumeragi/v2_runner.rs",
                "impl": None,
                "symbol": "run_inner",
                "required_tokens": (
                    "claim_runner_lifecycle_process_generation",
                    "let reservation_reconciliation_pending = true",
                    "match pending_kura_apply",
                    "lifecycle_run_inner::run_non_pending_lifecycle_loop",
                    "lifecycle_pending_kura::run_pending_kura_lifecycle_height",
                    "_lifecycle_process_generation",
                    "reservation_reconciliation_pending",
                ),
                "ordered_tokens": (
                    "let _initial_local_validator = local_validator_index(verified_context.context(), &local_peer, config.role)?",
                    "let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(",
                    "config.role, kura.as_ref(), verified_context.context(), &local_peer,",
                    "let reservation_reconciliation_pending = true",
                    "match pending_kura_apply",
                    "None => lifecycle_run_inner::run_non_pending_lifecycle_loop(",
                    "Some(pending) => lifecycle_pending_kura::run_pending_kura_lifecycle_height(",
                ),
            },
            {
                "role": "ordinary lifecycle process-generation startup propagation",
                "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
                "impl": None,
                "symbol": "run_non_pending_lifecycle_loop",
                "required_tokens": (
                    "let context = verified_context.context().clone",
                    "reconcile_autonomous_lifecycle_startup",
                    "lifecycle_process_generation.as_ref",
                    "V2LaneWorkAdapter::new_with_output_guard_and_transport",
                    "lifecycle_process_generation.clone",
                ),
                "ordered_tokens": (
                    "let context = verified_context.context().clone()",
                    "reconcile_autonomous_lifecycle_startup(",
                    "lifecycle_process_generation.as_ref(),",
                    "V2LaneWorkAdapter::new_with_output_guard_and_transport(",
                    "lifecycle_process_generation.clone(),",
                ),
            },
            {
                "role": "pending Kura lifecycle reconciliation propagation",
                "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
                "impl": None,
                "symbol": "reconcile_pending_lane_startup",
                "required_tokens": (
                    "reconcile_autonomous_lifecycle_startup",
                    "lifecycle_process_generation",
                    "plan_lane_reservation_ownership",
                    "Some(lifecycle)",
                ),
                "ordered_tokens": (
                    "let deferred_terminal_recovery = reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
                    "let planning = plan_lane_reservation_ownership(",
                    "let lifecycle = reconcile_autonomous_lifecycle_startup(",
                    "lifecycle_process_generation,",
                    "Some(lifecycle),",
                ),
            },
            {
                "role": "pending Kura lifecycle process-generation lane handoff",
                "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
                "impl": None,
                "symbol": "run_pending_kura_lifecycle_height",
                "required_tokens": (
                    "reconcile_pending_lane_startup",
                    "lifecycle_process_generation.as_ref",
                    "pending.prepare_lane_recovery",
                    "V2LaneWorkAdapter::new_with_output_guard_and_transport",
                    "lifecycle_process_generation.clone",
                ),
                "ordered_tokens": (
                    "let (pending, control) = reconcile_pending_lane_startup(",
                    "lifecycle_process_generation.as_ref(),",
                    "let mut prepared = pending.prepare_lane_recovery(",
                    "V2LaneWorkAdapter::new_with_output_guard_and_transport(",
                    "lifecycle_process_generation.clone(),",
                ),
            },
            {
                "role": "signed startup lifecycle recovery coordinator",
                "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
                "impl": None,
                "symbol": "reconcile_autonomous_lifecycle_startup",
                "required_tokens": (
                    "authorize_lane_reservation_snapshot_recovery",
                    "paired_lifecycle_group_identities",
                    "observer_retirement_lifecycle_projections",
                    "context: &wire::HeightContext",
                    "let network_id = context.network_id",
                    "let Some(process_generation) = process_generation else",
                    "process_generation.network_id() != network_id",
                    "authorize_recovered_producer_queue_lifecycle_bootstrap",
                    "authenticate_autonomous_lifecycle_bootstrap_recovery",
                    "complete_autonomous_lifecycle_bootstrap",
                    "into_reconciliation_receipt",
                    "recover_one_attempt",
                    "revalidate_lane_reservation_startup_reconciliation_receipt",
                ),
                "forbidden_tokens": (
                    "NetworkId::default",
                    "Default::default",
                    "synthetic_network_id",
                    "context.chain_id",
                ),
                "ordered_tokens": (
                    "let network_id = context.network_id",
                    "let Some(process_generation) = process_generation else",
                    "let recovery = queue.authorize_lane_reservation_snapshot_recovery(",
                    "let receipt = recovery.into_reconciliation_receipt()",
                    "process_generation.local_peer_id() != local_peer",
                    "process_generation.network_id() != network_id",
                    "let mut recovery_authorization = if snapshot.is_empty()",
                    "Some(queue.authorize_lane_reservation_snapshot_recovery(",
                    "authorize_recovered_producer_queue_lifecycle_bootstrap(",
                    "authenticate_autonomous_lifecycle_bootstrap_recovery(",
                    "complete_autonomous_lifecycle_bootstrap(permit)",
                    "let receipt = if let Some(recovery) = recovery_authorization { recovery.into_reconciliation_receipt()",
                    "if recover_one_attempt(",
                    "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
                ),
            },
            {
                "role": "recovered ProducerQueue lifecycle bootstrap fence",
                "path": "crates/iroha_core/src/queue.rs",
                "impl": "LaneReservationSnapshotRecoveryAuthorization",
                "symbol": "authorize_recovered_producer_queue_lifecycle_bootstrap",
                "required_tokens": (
                    "accepted.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                    "accepted.before != accepted.after",
                    "revalidate_lane_reservation_startup_reconciliation_receipt",
                    "revalidate_complete_live_pre_kura_group_locked",
                    "begin_durability_transition_locked",
                    "AutonomousLaneKuraActivationAuthorization",
                ),
                "ordered_tokens": (
                    "checked_group.checked.accepted_projection()",
                    "revalidate_lane_reservation_startup_reconciliation_receipt(",
                    "revalidate_complete_live_pre_kura_group_locked(",
                    "begin_durability_transition_locked(",
                    "Ok(AutonomousLaneKuraActivationAuthorization {",
                ),
            },
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/queue.rs",
            "impl": "Queue",
            "symbol": "authorize_lane_reservation_snapshot_recovery",
            "required_tokens": (
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "planner_group.requires_lifecycle_pair",
                "lane_reservation_recovery_phase_map",
                "lane_reservation_snapshot_group_phase_agrees",
                "paired_lifecycle_by_group",
                "lane_reservation_snapshot_release_retirement_hash",
                "anchor.actor_indices",
                "check_production_in_flight_first_release_recover_reservation_snapshot_transition",
                "accepted.before != accepted.after",
                "covered_owners.len() != phases.len()",
                "checked_by_group.into_values().collect()",
                "checked_planner_groups",
            ),
            "ordered_tokens": (
                "revalidate_lane_reservation_startup_reconciliation_receipt(",
                "lane_reservation_recovery_phase_map(&snapshot)",
                "let mut paired_lifecycle_by_group = BTreeMap::new()",
                "for lifecycle in lifecycle_projections",
                "let group_coverage = lane_reservation_snapshot_group_phase_agrees(&snapshot, &phases, lifecycle.reservation_group, &lifecycle.ordered_keys, lifecycle.recovered_state,)?",
                "let checked = check_production_in_flight_first_release_recover_reservation_snapshot_transition(lifecycle.recovered_state,)",
                "for planner_group in planner_groups",
                "let group_coverage = lane_reservation_snapshot_group_phase_agrees(&snapshot, &phases, reservation_group, &ordered_keys, recovered_state,)?",
                "let checked = check_production_in_flight_first_release_recover_reservation_snapshot_transition(recovered_state,)",
                "if !paired_lifecycle_by_group.is_empty()",
                "covered_owners.len() != phases.len()",
                "checked_by_group.into_values().collect()",
            ),
        },
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/queue.rs",
            "impl": "LaneReservationSnapshotRecoveryAuthorization",
            "symbol": "into_reconciliation_receipt",
            "required_tokens": (
                "checked_group.checked.into_projection",
                "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                "accepted.actor != 0",
                "accepted.target != 0",
                "accepted.before != accepted.after",
                "checked_group.lifecycle.recovered_state",
                "for checked_group in self.checked_planner_groups",
                "checked_group.recovered_state",
                "self.reconciliation_receipt",
            ),
            "ordered_tokens": (
                "for checked_group in self.checked_groups { let accepted = checked_group.checked.into_projection(); if accepted.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                "accepted.before != checked_group.lifecycle.recovered_state",
                "for checked_group in self.checked_planner_groups { let accepted = checked_group.checked.into_projection(); if accepted.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                "accepted.before != checked_group.recovered_state",
                "Ok(self.reconciliation_receipt)",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": None,
            "symbol": "apply_lane_reservation_reconciliation_plan",
            "required_tokens": (
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "let mut authorized_commit_groups = Vec::new()",
                "finalize_startup_committed_canonical_carriers",
                "release_strictly_absent_lane_reservations_in_order",
                "lane_reservation_reconciliation_snapshot",
                "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions",
                "complete_lane_reservation_startup_reconciliation",
            ),
            "ordered_tokens": (
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "let mut authorized_commit_groups = Vec::new()",
                "finalize_startup_committed_canonical_carriers(",
                "release_strictly_absent_lane_reservations_in_order",
                "lane_reservation_reconciliation_snapshot",
                "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
                "complete_lane_reservation_startup_reconciliation(replay_receipt)",
            ),
        },
    },
    {
        "id": "startup_local_kura_custody_rehydration_cas",
        "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "impl": None,
        "symbol": "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition",
        "model_actions": ("RehydrateLocalKuraCustody",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "after.session.bodies |= actor",
            "actor == before.producer",
            "after.session.producer_alive = true",
            "check_derived_production_in_flight_first_release_transition",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "signed lifecycle CAS cursor constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
                "impl": None,
                "symbol": "sign_lifecycle_cursor",
                "required_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "previous_cursor_hash",
                    "signing_preimage",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
                "ordered_tokens": (
                    "AutonomousLifecycleCursorUnsignedV2::new",
                    "signing_preimage()",
                    "Signature::try_new",
                    "<[u8; 96]>::try_from",
                    "unsigned.finalize(signature, validator_set)",
                ),
            },
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "reconcile_autonomous_lifecycle_startup",
            "required_tokens": (
                "into_reconciliation_receipt",
                "let mut recovered_attempts = 0_usize",
                "recover_one_attempt",
                "revalidate_lane_reservation_startup_reconciliation_receipt",
                "RecoveredAutonomousLifecycleStartup",
            ),
            "ordered_tokens": (
                "let receipt = if let Some(recovery) = recovery_authorization { recovery.into_reconciliation_receipt()",
                "let mut recovered_attempts = 0_usize",
                "if recover_one_attempt(",
                "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
                "Ok(RecoveredAutonomousLifecycleStartup { snapshot, receipt, deferred_terminal_recovery, completed_bootstraps, recovered_attempts, })",
            ),
        },
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "recover_one_attempt",
            "required_tokens": (
                "AutonomousLifecycleCursorPhaseKindV2::Live",
                "before.session.bodies & local_actor",
                "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition",
                "AutonomousLifecycleCursorPhaseV2::prepared(current_generation, rehydrate)",
                "compare_and_swap_phase",
            ),
            "ordered_tokens": (
                "AutonomousLifecycleCursorPhaseKindV2::Live =>",
                "before.session.bodies & local_actor != 0",
                "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(",
                "let phase = AutonomousLifecycleCursorPhaseV2::prepared(current_generation, rehydrate).map_err(|reason| { lifecycle_error(\"\", reason) })?; let _ = compare_and_swap_phase(kura, key_pair, local_peer, payload, read, phase)",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
            "impl": None,
            "symbol": "compare_and_swap_phase",
            "required_tokens": (
                "read.into_parts",
                "checked_add(1)",
                "previous_cursor_hash",
                "sign_lifecycle_cursor",
                "compare_and_swap_autonomous_lifecycle_cursor",
            ),
            "ordered_tokens": (
                "let (current, lease) = read.into_parts()",
                "let sequence = current",
                "let previous_cursor_hash = current.as_ref().map",
                "let binding = current",
                "let next = sign_lifecycle_cursor(",
                "kura.compare_and_swap_autonomous_lifecycle_cursor(lease, next)",
            ),
        },
    },
    {
        "id": "producer_payload_transport_fanout",
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "<'queue> FirstReleaseFanoutFromProducerAuthorization<'queue>",
        "symbol": "new",
        "model_actions": ("FanoutFromProducer",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLanePayloadFanoutAuthorization",
            "reservation_authorization.facts",
            "current_autonomous_lane_payload",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "check_production_in_flight_first_release_fanout_from_producer_transition",
            "lane_work_effect_key",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "action-specific FanoutFromProducer constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                "impl": None,
                "symbol": "check_production_in_flight_first_release_fanout_from_producer_transition",
                "required_tokens": (
                    "ProductionInFlightFirstReleaseTransitionProjection",
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
                "ordered_tokens": (
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
            },
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "FirstReleaseBodyPublicationAuthorization for FirstReleaseFanoutFromProducerAuthorization<'_>",
            "symbol": "publish",
            "required_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
            "ordered_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "fanout_producer_lane_executable_payload",
            "required_tokens": (
                "authorize_lane_reservation_payload_fanout",
                "push_effect_with_fresh_authorization",
                "FirstReleaseFanoutFromProducerAuthorization::new",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "push_effect_with_fresh_authorization",
            "required_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
            "ordered_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
        },
    },
    {
        "id": "producer_payload_fanout_queue_fence",
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "<'queue> FirstReleaseFanoutFromProducerAuthorization<'queue>",
        "symbol": "new",
        "model_actions": ("FanoutFromProducer",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLanePayloadFanoutAuthorization",
            "reservation_authorization.facts",
            "current_autonomous_lane_payload",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "check_production_in_flight_first_release_fanout_from_producer_transition",
            "lane_work_effect_key",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "action-specific FanoutFromProducer constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                "impl": None,
                "symbol": "check_production_in_flight_first_release_fanout_from_producer_transition",
                "required_tokens": (
                    "ProductionInFlightFirstReleaseTransitionProjection",
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
                "ordered_tokens": (
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
            },
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "FirstReleaseBodyPublicationAuthorization for FirstReleaseFanoutFromProducerAuthorization<'_>",
            "symbol": "publish",
            "required_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
            "ordered_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/queue.rs",
            "impl": "Queue",
            "symbol": "authorize_lane_reservation_payload_fanout",
            "required_tokens": (
                "authorize_lane_reservation_kura_activation",
                "AutonomousLaneKuraActivationAuthorization",
                "AutonomousLanePayloadFanoutAuthorization",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "push_effect_with_fresh_authorization",
            "required_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
            "ordered_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
        },
    },
    {
        "id": "producer_payload_retransmission_fanout",
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "<'queue> FirstReleaseFanoutFromProducerAuthorization<'queue>",
        "symbol": "new",
        "model_actions": ("FanoutFromProducer",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLanePayloadFanoutAuthorization",
            "reservation_authorization.facts",
            "current_autonomous_lane_payload",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "check_production_in_flight_first_release_fanout_from_producer_transition",
            "lane_work_effect_key",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "action-specific FanoutFromProducer constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                "impl": None,
                "symbol": "check_production_in_flight_first_release_fanout_from_producer_transition",
                "required_tokens": (
                    "ProductionInFlightFirstReleaseTransitionProjection",
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
                "ordered_tokens": (
                    "after.session.bodies |= replica",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
                    "replica,",
                ),
            },
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "FirstReleaseBodyPublicationAuthorization for FirstReleaseFanoutFromProducerAuthorization<'_>",
            "symbol": "publish",
            "required_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
            "ordered_tokens": (
                "checked.into_projection",
                "publish()",
                "drop(reservation_authorization)",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "schedule_lane_artifact_retransmissions",
            "required_tokens": (
                "plan_autonomous_lane_reservation_slot",
                "autonomous_proposal_matches_reservation_slot",
                "fanout_producer_lane_executable_payload",
            ),
            "ordered_tokens": (
                "plan_autonomous_lane_reservation_slot",
                "autonomous_proposal_matches_reservation_slot",
                "fanout_producer_lane_executable_payload",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "push_effect_with_fresh_authorization",
            "required_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
            "ordered_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
        },
    },
    {
        "id": "authenticated_autonomous_late_body_service",
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "FirstReleaseServeLateBodyAuthorization",
        "symbol": "new",
        "model_actions": ("ServeLateBody",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "validate_certified_lane_block_artifact",
            "read_certified_lane_block_artifact",
            "current_autonomous_lane_payload",
            "read_autonomous_lane_block_artifact",
            "signers_bitmap",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "check_production_in_flight_first_release_serve_late_body_transition",
            "lane_work_effect_key",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "impl": None,
            "symbol": "check_derived_production_in_flight_first_release_transition",
            "required_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection",
                "action: u8",
                "actor: u128",
                "target: u128",
                "before: ProductionInFlightFirstReleaseStateProjection",
                "after: ProductionInFlightFirstReleaseStateProjection",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition",
                "ProductionInFlightFirstReleaseTransitionProjection {",
                "action,",
                "actor,",
                "target,",
                "before,",
                "after,",
            ),
            "transition_projection_count": 2,
        },
        "supporting_sources": (
            {
                "role": "action-specific ServeLateBody constructor",
                "path": "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
                "impl": None,
                "symbol": "check_production_in_flight_first_release_serve_late_body_transition",
                "required_tokens": (
                    "ProductionInFlightFirstReleaseTransitionProjection",
                    "after.session.bodies |= target",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY",
                    "source,",
                    "target,",
                ),
                "ordered_tokens": (
                    "after.session.bodies |= target",
                    "check_derived_production_in_flight_first_release_transition",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY",
                    "source,",
                    "target,",
                ),
            },
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "FirstReleaseBodyPublicationAuthorization for FirstReleaseServeLateBodyAuthorization",
            "symbol": "publish",
            "required_tokens": (
                "self.checked.into_projection",
                "publish()",
            ),
            "ordered_tokens": (
                "self.checked.into_projection",
                "publish()",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "serve_historical_recovery_request",
            "required_tokens": (
                "LaneHistoricalRecoveryPayloadV1::AutonomousPayload",
                "push_effect_with_fresh_authorization",
                "FirstReleaseServeLateBodyAuthorization::new",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "impl": "V2LaneWorkAdapter",
            "symbol": "push_effect_with_fresh_authorization",
            "required_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
            "ordered_tokens": (
                "preflight_effect_insertion",
                "authorization.matches_effect",
                "authorization.publish",
                "self.effect_keys.insert(key)",
                "self.effects.push_back(effect)",
            ),
        },
    },
    {
        "id": "execution_input_persistence",
        "path": "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "impl": "Kura",
        "symbol": "write_lane_block_execution_input_artifact",
        "model_actions": ("PersistExecutionInput",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLaneExecutionInputPersistenceAuthorization",
            "authorization.matches_input",
            "consume_for_persistence",
            "autonomous_input",
            "preflight_lane_block_execution_input_publication_locked",
            "append_indexed_progress_sidecar",
            "&publication.namespace",
        ),
        "supporting_sources": (
            {
                "role": "bound progress append adapter",
                "path": "crates/iroha_core/src/kura/indexed_sidecar_io.rs",
                "impl": "Kura",
                "symbol": "append_indexed_progress_sidecar",
                "required_tokens": (
                    "progress_mutation_namespace_unchanged",
                    "append_indexed_bound_progress_sidecar",
                ),
                "ordered_tokens": (
                    "if retention.is_some() || !Self::progress_mutation_namespace_unchanged(namespace)",
                    "let wrote = Self::append_indexed_bound_progress_sidecar",
                    "wrote && Self::progress_mutation_namespace_unchanged(namespace)",
                ),
            },
            {
                "role": "journaled bound progress append planner",
                "path": "crates/iroha_core/src/kura/indexed_sidecar_io.rs",
                "impl": "Kura",
                "symbol": "append_indexed_bound_progress_sidecar",
                "required_tokens": (
                    "BoundProgressAppendIntentV1",
                    "payload_digest",
                    ".seal",
                    "execute_bound_progress_append",
                ),
                "ordered_tokens": (
                    "old_index_bytes: entry.to_bytes().to_vec(), new_index_bytes: new_entry.to_bytes().to_vec()",
                    "return Self::execute_bound_progress_append(data_path, index_path, payload, kind, namespace, intent, data, index,)",
                    "let mut new_index_bytes = Vec::new()",
                    "old_index_bytes: Vec::new(), new_index_bytes, integrity_hash: Hash::prehashed([0; Hash::LENGTH]), }.seal(); Self::execute_bound_progress_append",
                ),
            },
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
            "impl": "Kura",
            "symbol": "authorize_autonomous_execution_input_persistence",
            "required_tokens": (
                "AutonomousLaneExecutionInputPersistenceAuthorization",
                "autonomous_lane_block_execution_input_candidate",
                "local_peer_id",
                "lane_queue_reservation_group_binding_from_ordered_keys",
                "canonical_lane_queue_reservation_group_identity_projection",
                "ProductionInFlightFirstReleaseTransitionProjection",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura/indexed_sidecar_io.rs",
            "impl": "Kura",
            "symbol": "execute_bound_progress_append",
            "required_tokens": (
                "intent.validate_for",
                "publish_bound_progress_append_intent",
                "data.write_all(payload)",
                "sync_bound_progress_append_data",
                "index.write_all(&intent.new_index_bytes)",
                "sync_bound_progress_append_index",
                "bound_sidecar_index_snapshot",
                "sync_indexed_sidecar_bound_mutation",
                "remove_bound_progress_temp_if_present",
                "sync_bound_progress_intent_directories",
                "progress_mutation_namespace_unchanged",
            ),
            "ordered_tokens": (
                "intent.validate_for(namespace, data_path, index_path)",
                "Self::publish_bound_progress_append_intent(namespace, index_path, &intent, kind)",
                "data.write_all(payload)",
                "sync_bound_progress_append_data(data)",
                "index.write_all(&intent.new_index_bytes)",
                "sync_bound_progress_append_index(index)",
                "Self::bound_sidecar_index_snapshot",
                "Self::sync_indexed_sidecar_bound_mutation(data, index, namespace, kind)",
                "drop(intent_file)",
                "Self::remove_bound_progress_temp_if_present(namespace, &intent_path)",
                "Self::sync_bound_progress_intent_directories(namespace)",
            ),
        },
    },
    {
        "id": "durable_autonomous_bundle",
        "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "impl": "Kura",
        "symbol": "durable_autonomous_lane_merge_source_under_prune_guard",
        "model_actions": ("PersistReadyQc", "LaneCommit"),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT",
        ),
        "checked_transition_count": 2,
        "additional_tokens": (
            "AutonomousLaneMergeBundleV1",
            "validate_autonomous_lane_merge_bundle",
            "source_bundle",
            "bundle_hash",
            "canonical_lane_queue_reservation_group_identity_projection",
        ),
    },
    {
        "id": "ready_qc_persistence",
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "persist_lane_payload_availability_certificate",
        "model_actions": ("PersistReadyQc",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "authorize_lane_payload_availability_certificate_persistence",
            "consume_for_persistence",
            "artifact.availability_certificate = Some(certificate)",
            "write_autonomous_lane_block_view_state_locked",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
            "impl": "Kura",
            "symbol": "authorize_lane_payload_availability_certificate_persistence",
            "required_tokens": (
                "AutonomousLaneReadyQcPersistenceAuthorization",
                "validate_lane_payload_availability_certificate",
                "signers_bitmap",
                "lane_queue_reservation_group_binding_from_ordered_keys",
                "canonical_lane_queue_reservation_group_identity_projection",
                "ProductionInFlightFirstReleaseTransitionProjection",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "write_autonomous_lane_block_view_state_locked",
            "required_tokens": (
                "validate_autonomous_lane_block_artifact",
                "write_autonomous_lane_block_view_state_record_locked",
            ),
        },
    },
    {
        "id": "lane_commit_persistence",
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "write_certified_lane_block_artifact_with_authority_under_prune_guard",
        "model_actions": ("LaneCommit",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLaneCommitPersistenceAuthorization",
            "consume_for_persistence",
            "existing_exact",
            "publish_certified_frontier_and_consume_capacity_locked",
            "append_indexed_progress_sidecar",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
            "impl": "Kura",
            "symbol": "authorize_autonomous_lane_commit_persistence",
            "required_tokens": (
                "AutonomousLaneCommitPersistenceAuthorization",
                "validate_autonomous_lane_merge_bundle",
                "autonomous_lane_block_execution_input_candidate",
                "signers_bitmap",
                "lane_queue_reservation_group_binding_from_ordered_keys",
                "canonical_lane_queue_reservation_group_identity_projection",
                "ProductionInFlightFirstReleaseTransitionProjection",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
            "impl": "Kura",
            "symbol": "publish_certified_frontier_and_consume_capacity_locked",
            "required_tokens": (
                "publish_latest_certified_lane_block_frontier_locked",
                "read_latest_certified_lane_block_frontier_structural_locked",
                "confirm_latest_certified_lane_block_frontier_read_locked",
                "consume_certified_bundle_frontier_capacity",
            ),
        },
    },
    {
        "id": "kura_slot_retirement_persistence",
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "persist_autonomous_lane_slot_retirement",
        "model_actions": ("PersistKuraRetirement",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "authorize_autonomous_lane_slot_retirement_persistence",
            "consume_for_persistence",
            "record.artifact.executable_payload",
            "record.view_state_path",
            "write_autonomous_lane_block_view_state_record_locked",
            "prepare_autonomous_lane_entrypoint_claim_release_locked",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
            "impl": "AutonomousLaneReleaseProjectionContext",
            "symbol": "retirement_authorization",
            "required_tokens": (
                "AutonomousLaneSlotRetirementPersistenceAuthorization",
                "canonical_lane_queue_reservation_group_identity_projection",
                "self.reservation_group",
                "payload.payload_hash",
                "payload.origin_proposal.proposal_hash",
                "retirement.clone",
                "view_state_path.to_path_buf",
                "check_production_in_flight_first_release_transition",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "write_autonomous_lane_block_view_state_record_locked",
            "required_tokens": (
                "state.matches_payload(payload)",
                "retirement.matches_payload(payload)",
                "validate_autonomous_lane_block_artifact",
                "norito::encode_canonical(state)",
                "write_atomic_synced_replace",
            ),
        },
    },
    {
        "id": "kura_claim_release_prefixes",
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "transition_autonomous_lane_entrypoint_claims_locked",
        "model_actions": (
            "AdvanceReleasePendingPrefix",
            "AdvanceReleasedPrefix",
        ),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLaneReleaseProjectionContext::from_payload",
            "unique_paths",
            "claim.stage > previous_stage",
            "saw_released && saw_active",
            "finalize_release && saw_active",
            "claim_transition_authorization",
            "norito::encode_canonical",
            "consume_for_persistence",
            "write_atomic_synced_replace",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
            "impl": "AutonomousLaneReleaseProjectionContext",
            "symbol": "claim_transition_authorization",
            "required_tokens": (
                "AutonomousLaneEntrypointClaimTransitionAuthorization",
                "canonical_lane_queue_reservation_group_identity_projection",
                "replacement.retirement_hash()",
                "self.retirement_hash",
                "self.reservation_group.reservation_count",
                "path.to_path_buf",
                "replacement.clone",
                "check_production_in_flight_first_release_transition",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
            "impl": None,
            "symbol": "write_atomic_synced_impl_with_prefix",
            "required_tokens": (
                "write_all(bytes)",
                "sync_all",
                "persist(path)",
                "symlink_metadata(path)",
                "sync_dir(parent)",
            ),
        },
    },
    {
        "id": "queue_release_preparation_handoff",
        "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "impl": "AutonomousLaneReleaseProjectionContext",
        "symbol": "queue_preparation_authorization",
        "model_actions": ("PrepareReservationRelease",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "AutonomousLaneQueueReleasePreparationAuthorization",
            "retirement.queue_release_barrier",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "claims_fully_released",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_release_authority.rs",
            "impl": "Kura",
            "symbol": "authorize_autonomous_lane_queue_release_preparation",
            "required_tokens": (
                "record.retirement.as_ref()",
                "prepare_autonomous_lane_entrypoint_claim_release_locked",
                "autonomous_lane_entrypoint_claim_release_progress_locked",
                "require_autonomous_lane_release_completed_or_superseded_locked",
                "queue_preparation_authorization",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/queue.rs",
            "impl": "Queue",
            "symbol": "prepare_lane_reservation_release_barrier_inner",
            "required_tokens": (
                "LaneQueueReleasePreparationGate",
                "begin_durability_transition_locked",
                "release_barrier_has_exact_fifo_ownership_locked",
                "check_production_in_flight_first_release_transition",
                "journal.prepare_release(barrier.clone())",
                "DurableLaneQueueReleaseBarrierAuthorization::durable",
            ),
        },
    },
    {
        "id": "queue_release_completion_publication",
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "finalize_lane_reservation_release_barrier_inner",
        "model_actions": (
            "CompleteReservationRelease",
            "RestoreReleasedFifo",
            "ForgetReservationRelease",
        ),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE",
        ),
        "checked_transition_count": 3,
        "additional_tokens": (
            "LaneQueueReleaseFinalizationGate",
            "canonical_lane_queue_reservation_group_identity_projection",
            "journal.complete_release(completion.clone())",
            "fifo_snapshot_locked",
            "fifo_with_released_reservations_locked",
            "replace_fifo_locked",
            "release_barrier_has_exact_fifo_ownership_locked",
            "journal.forget_release(completion.barrier.clone())",
        ),
        "authorization_source": {
            "path": "crates/iroha_core/src/kura/autonomous_release_authority.rs",
            "impl": "Kura",
            "symbol": "finalize_autonomous_lane_slot_release_inner",
            "required_tokens": (
                "AutonomousLaneQueueReleaseBarrierGate",
                "consume_for_claim_transition",
                "autonomous_lane_entrypoint_claim_release_progress_locked",
                "finalize_autonomous_lane_entrypoint_claim_release_locked",
                "require_autonomous_lane_release_completed_or_superseded_locked",
                "queue_finalization_authorization",
            ),
        },
    },
    {
        "id": "ready_authorization",
        "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "impl": "Kura",
        "symbol": "mint_lane_ready_authorization",
        "model_actions": ("AuthorizeReady",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "LaneReadyAuthorization",
            "read_lane_block_execution_input_with_repair_policy",
            "durable_execution_input_hash",
            "canonical_lane_queue_reservation_group_identity_projection",
        ),
    },
    {
        "id": "ready_signature",
        "path": "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "impl": "LaneReadyAuthorization",
        "symbol": "consume_signing_request",
        "model_actions": ("SignReady",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "matches_signing_request",
            "reservation_group",
            "producer",
            "canonical_lane_queue_reservation_group_identity_projection",
        ),
        "commit_sink": {
            "path": "crates/iroha_core/src/lane_consensus.rs",
            "impl": "LanePayloadAvailabilityVoteV1",
            "symbol": "new_signed_with_authorization",
            "required_tokens": (
                "authorization.consume_signing_request",
                "Signature::try_new",
                "body.signature_preimage",
            ),
        },
    },
    {
        "id": "canonical_wsv_commit_authorization",
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": None,
        "symbol": "authenticated_autonomous_carrier_application_projections",
        "model_actions": ("ApplyCarrier",),
        "action_tags": ("IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER",),
        "checked_transition_count": 1,
        "additional_tokens": (
            "reference.matches_entry(entry)",
            "Kura::decode_autonomous_lane_merge_bundle",
            "authenticated_bundle.bundle_hash",
            "payload.native_amx_receipts != lane.native_amx_receipts",
            "availability_qc.signers_bitmap",
            "commit_qc.signers_bitmap",
            "lane_commit_candidates",
            "lane_queue_reservation_group_binding_from_ordered_keys",
            "canonical_lane_queue_reservation_group_identity_projection",
            "ProductionInFlightFirstReleaseTransitionProjection",
            "application.checked_transition()",
            "checked.into_projection()",
            "applications.push(application)",
        ),
        "checked_transition_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "AuthenticatedCarrierApplicationProjection",
            "symbol": "checked_transition",
            "required_tokens": (
                "CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>",
                "check_production_in_flight_first_release_transition(self.projection)",
                "ok_or_else",
                "to_owned",
            ),
            "ordered_tokens": (
                "check_production_in_flight_first_release_transition(self.projection)",
                "ok_or_else",
                "to_owned",
            ),
            "transition_projection_count": 1,
        },
        "supporting_sources": (
            {
                "role": "finality-to-State ApplyCarrier orchestration",
                "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
                "impl": "V2ApplyService",
                "symbol": "execute_exact_apply",
                "required_tokens": (
                    "check_production_application_transition",
                    "CheckedCarrierApplications::for_block",
                    "authenticated_autonomous_carrier_application_projections",
                    "checked_carrier_applications.bind_execution_batch(reference, applications.len())",
                    "application.checked_transition()",
                    "application.projection",
                    "checked_carrier_applications.push",
                    "validate_and_apply",
                ),
                "ordered_tokens": (
                    "check_production_application_transition",
                    "CheckedCarrierApplications::for_block",
                    "authenticated_autonomous_carrier_application_projections",
                    "checked_carrier_applications.bind_execution_batch(reference, applications.len())",
                    "checked_carrier_applications.push",
                    "application.checked_transition()",
                    "application.projection",
                    "validate_and_apply",
                ),
            },
            {
                "role": "ordinary Apply completion wrapper",
                "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
                "impl": "V2ApplyService",
                "symbol": "execute",
                "required_tokens": (
                    "self.execute_exact_apply(context, body_store, ExactApplyTaskRef::Ordinary(task))",
                    "material.ordinary_projection",
                    "self.finish_durable_apply_completion_against(evidence, prospective_application)",
                ),
                "ordered_tokens": (
                    "self.execute_exact_apply(context, body_store, ExactApplyTaskRef::Ordinary(task))",
                    "material.ordinary_projection",
                    "self.finish_durable_apply_completion_against(evidence, prospective_application)",
                ),
            },
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "CheckedCarrierApplications",
            "symbol": "consume_for_state_commit",
            "required_tokens": (
                "carrier_block_hash",
                "staged_merge_entry",
                "CertifiedMergeLedgerReference::new(entry)",
                "batch.lanes.len()",
                "checked.into_projection()",
            ),
            "ordered_tokens": (
                "if carrier_block_hash != self.carrier_block_hash",
                "let committed_execution_reference = staged_merge_entry",
                "CertifiedMergeLedgerReference::new(entry)",
                "batch.lanes.len()",
                "match (self.execution_reference.as_ref()",
                "for CheckedCarrierApplication",
                "if checked.into_projection() != projection",
            ),
        },
        "checked_transition_adapter": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "StateBlockCommitAuthorization for CheckedCarrierApplications",
            "symbol": "consume_for_state_commit",
            "required_tokens": (
                "self: Box<Self>",
                "carrier_block_hash",
                "staged_merge_entry",
                "CheckedCarrierApplications::consume_for_state_commit",
                "*self",
                "map_err(str::to_owned)",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "V2ApplyService",
            "symbol": "validate_and_apply",
            "required_tokens": (
                "pending_autoscale_retirement_binding",
                "Box<dyn StateBlockCommitAuthorization>",
                "Box::new(checked_carrier_applications)",
                "if carries_scale_in",
                "lock_lane_retirement_observer",
                "commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto",
                "commit_with_state_commit_authorization",
            ),
            "ordered_tokens": (
                "apply_without_execution_with_verified_v2_finality",
                "pending_autoscale_retirement_binding",
                "Box::new(checked_carrier_applications)",
                "if carries_scale_in",
                "lock_lane_retirement_observer",
                "commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto",
                "state_block.commit_with_state_commit_authorization",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/state.rs",
            "impl": None,
            "symbol": "commit_inner",
            "required_tokens": (
                "state_commit_authorization: Option<Box<dyn StateBlockCommitAuthorization>>",
                "let _state_commit_lock = state_ref.state_commit_lock.lock()",
                "let autoscale_lifecycle_guard",
                "autoscale_retirement_queue_veto.as_mut()",
                "state_commit_authorization.take()",
                "authorization.consume_for_state_commit",
                "apply_committed_autoscale_lane_geometry",
                "transactions.commit()",
            ),
            "ordered_tokens": (
                "let _state_commit_lock = state_ref.state_commit_lock.lock()",
                "let autoscale_lifecycle_guard",
                "autoscale_retirement_queue_veto.as_mut()",
                "state_commit_authorization.take()",
                "authorization.consume_for_state_commit",
                "state_ref.apply_committed_autoscale_lane_geometry",
                "transactions.commit()",
            ),
        },
    },
    {
        "id": "live_post_carrier_evidence_repair",
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": "PostCarrierEvidenceRepairAuthorization",
        "symbol": "from_authenticated",
        "model_actions": ("RepairPostCarrierEvidence",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "check_production_in_flight_first_release_repair_post_carrier_evidence_transition",
            "application.projection.after",
            "entry_hash",
            "carrier_block_height",
            "carrier_block_hash",
            "reservation_group",
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "PostCarrierEvidenceRepairAuthorization",
            "symbol": "consume_for_kura",
            "required_tokens": (
                "expected_entry_hash",
                "expected_carrier_block_height",
                "expected_carrier_block_hash",
                "expected_reservation_group",
                "checked_repair.into_projection()",
                "canonical_lane_queue_reservation_group_identity_projection",
                "projection.before == projection.after",
                "projection.before.decision.wsv_committed",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "V2ApplyService",
            "symbol": "publish_committed_block_merge_entry",
            "required_tokens": (
                "ensure_globally_committed_merge_entry_applied",
                "post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
                "record_globally_committed_merge_entry",
            ),
            "ordered_tokens": (
                "ensure_globally_committed_merge_entry_applied",
                "post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
                "record_globally_committed_merge_entry",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
            "required_tokens": (
                "merge_log.lock().entry_by_hash",
                "merge_entry_for_carrier",
                "consume_post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
            "ordered_tokens": (
                "merge_log.lock().entry_by_hash",
                "merge_entry_for_carrier",
                "consume_post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
        },
    },
    {
        "id": "startup_reverse_carrier_evidence_repair",
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": "PostCarrierEvidenceRepairAuthorization",
        "symbol": "from_authenticated",
        "model_actions": ("RepairPostCarrierEvidence",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "check_production_in_flight_first_release_repair_post_carrier_evidence_transition",
            "application.projection.after",
            "entry_hash",
            "carrier_block_height",
            "carrier_block_hash",
            "reservation_group",
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "PostCarrierEvidenceRepairAuthorization",
            "symbol": "consume_for_kura",
            "required_tokens": (
                "expected_entry_hash",
                "expected_carrier_block_height",
                "expected_carrier_block_hash",
                "expected_reservation_group",
                "checked_repair.into_projection()",
                "canonical_lane_queue_reservation_group_identity_projection",
                "projection.before == projection.after",
                "projection.before.decision.wsv_committed",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs",
            "impl": None,
            "symbol": "plan_lane_application_evidence_repair",
            "required_tokens": (
                "preflight_finalized_merge_carrier_repairs",
                "ensure_committed_merge_execution_applied",
                "post_carrier_evidence_repair_authorizations",
                "merge_carrier_repair_authorizations.push(post_carrier_evidence_repair_authorizations",
            ),
            "ordered_tokens": (
                "preflight_finalized_merge_carrier_repairs",
                "ensure_committed_merge_execution_applied",
                "merge_carrier_repair_authorizations.push(post_carrier_evidence_repair_authorizations",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "apply_finalized_merge_carrier_repairs",
            "required_tokens": (
                "preflight_finalized_merge_carrier_at_under_prune_and_canonical_guards",
                "consume_post_carrier_evidence_repair_authorizations",
                "append_committed_merge_entry_for_block_if_missing",
                "set_transaction_entrypoint_index_entry_with_merge",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
            "ordered_tokens": (
                "preflight_finalized_merge_carrier_at_under_prune_and_canonical_guards",
                "consume_post_carrier_evidence_repair_authorizations",
                "append_committed_merge_entry_for_block_if_missing",
                "set_transaction_entrypoint_index_entry_with_merge",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
        },
    },
    {
        "id": "state_replay_post_carrier_evidence_repair",
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": "PostCarrierEvidenceRepairAuthorization",
        "symbol": "from_authenticated",
        "model_actions": ("RepairPostCarrierEvidence",),
        "action_tags": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
        ),
        "checked_transition_count": 1,
        "additional_tokens": (
            "check_production_in_flight_first_release_repair_post_carrier_evidence_transition",
            "application.projection.after",
            "entry_hash",
            "carrier_block_height",
            "carrier_block_hash",
            "reservation_group",
        ),
        "checked_transition_consumer": {
            "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "impl": "PostCarrierEvidenceRepairAuthorization",
            "symbol": "consume_for_kura",
            "required_tokens": (
                "expected_entry_hash",
                "expected_carrier_block_height",
                "expected_carrier_block_hash",
                "expected_reservation_group",
                "checked_repair.into_projection()",
                "canonical_lane_queue_reservation_group_identity_projection",
                "projection.before == projection.after",
                "projection.before.decision.wsv_committed",
            ),
        },
        "authorization_source": {
            "path": "crates/iroha_core/src/state.rs",
            "impl": "State",
            "symbol": "replay_persisted_merge_settlements",
            "required_tokens": (
                "merge_execution_already_applied(&entry, batch)",
                "post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
            ),
            "ordered_tokens": (
                "merge_execution_already_applied(&entry, batch)",
                "post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
            ),
        },
        "commit_sink": {
            "path": "crates/iroha_core/src/kura.rs",
            "impl": "Kura",
            "symbol": "persist_merge_lane_block_application_receipts_from_committed_log_with_authorizations",
            "required_tokens": (
                "merge_log.lock().entry_by_hash",
                "merge_entry_for_carrier",
                "consume_post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
            "ordered_tokens": (
                "merge_log.lock().entry_by_hash",
                "merge_entry_for_carrier",
                "consume_post_carrier_evidence_repair_authorizations",
                "persist_merge_lane_block_application_receipts_after_repair_authorization",
            ),
        },
    },
)

# Full startup lifecycle extraction. The journal-replay portion remains a
# parametric noninterference proof, while the production bindings below also
# authenticate the independently constructed signed lifecycle state, Queue
# action-25 authorization, bootstrap completion, generation takeover/body
# rehydration CAS, receipt handoff, final Queue application, and carrier-silent
# adapter activation order.
PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS = (
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": "IndexedReservationReplayState",
        "symbol": "check_in_flight_transition",
        "required_tokens": (
            "LaneQueueReservationJournalFrameV6::Snapshot",
            "candidate.transition_snapshot",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
            "retain_in_flight_owner_transition",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": "IndexedReservationReplayState",
        "symbol": "from_replay",
        "required_tokens": (
            "prepare_checked_transition",
            "apply_checked_transition",
            "canonical_reconciliation_owners_from_state",
            "canonical_reconciliation_identity",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": None,
        "symbol": "canonical_reconciliation_owners_from_state",
        "required_tokens": (
            "DurableReservationOwnership::Live",
            "DurableReservationOwnership::Committed",
            "DurableReservationOwnership::Prepared",
            "DurableReservationOwnership::Completed",
            "canonical_reconciliation_record_identity",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": None,
        "symbol": "canonical_reconciliation_owners_from_snapshot",
        "required_tokens": (
            "snapshot.ordered_records",
            "snapshot.ordered_groups",
            "snapshot.commit_barriers",
            "snapshot.prepared_release_barriers",
            "snapshot.completed_releases",
            "snapshot.ordered_owner_phases",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": None,
        "symbol": "canonical_reconciliation_identity",
        "required_tokens": (
            "SNAPSHOT_RECONCILIATION_EMPTY_DOMAIN",
            "SNAPSHOT_RECONCILIATION_STEP_DOMAIN",
            "checked_owner_projection_digest",
            "owner.record_identity",
            "SNAPSHOT_RECONCILIATION_FINAL_DOMAIN",
            "owners.len",
        ),
        "ordered_tokens": (
            "SNAPSHOT_RECONCILIATION_EMPTY_DOMAIN",
            "for owner in owners.values()",
            "let record_present",
            "rolling = match owner.record_identity",
            "let count = u64::try_from(owners.len())",
            "SNAPSHOT_RECONCILIATION_FINAL_DOMAIN",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": None,
        "symbol": "recover_snapshot_transition_projection",
        "required_tokens": (
            "ownership.release_digest",
            "release_refinement_identity",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
            "reservation_refinement_identity(ownership.key())",
            "optional_owner_refinement_projection(None)",
            "ownership.refinement_projection()",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": None,
        "symbol": "transition_projection_coverage_identity",
        "required_tokens": (
            "CHECKED_TRANSITION_COVERAGE_EMPTY_DOMAIN",
            "checked_transition_projection_digest",
            "CHECKED_TRANSITION_COVERAGE_STEP_DOMAIN",
            "count.checked_add",
            "CHECKED_TRANSITION_COVERAGE_FINAL_DOMAIN",
        ),
        "ordered_tokens": (
            "CHECKED_TRANSITION_COVERAGE_EMPTY_DOMAIN",
            "for transition in transitions",
            "checked_transition_projection_digest(transition)",
            "CHECKED_TRANSITION_COVERAGE_STEP_DOMAIN",
            "count.checked_add(1)",
            "CHECKED_TRANSITION_COVERAGE_FINAL_DOMAIN",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": "LaneReservationSnapshotReplayReceipt",
        "symbol": "binds_reconciliation_snapshot",
        "required_tokens": (
            "canonical_reconciliation_owners_from_snapshot",
            "recover_snapshot_transition_projection",
            "transition_projection_coverage_identity",
            "canonical_reconciliation_identity",
        ),
        "ordered_tokens": (
            "canonical_reconciliation_owners_from_snapshot(snapshot)",
            "transition_projection_coverage_identity(projections)",
            "canonical_reconciliation_identity(&owners)",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue/reservation_journal.rs",
        "impl": "LaneQueueReservationJournal",
        "symbol": "consume_snapshot_replay_seal",
        "required_tokens": (
            "LaneReservationSnapshotReplaySeal",
            "checked_file_content_identity",
            "replay_open_file",
            "self.replay_state.replay",
            "transition.receipt.frame_digest",
        ),
        "ordered_tokens": (
            "let current_content_identity = checked_file_content_identity",
            "if current_content_identity != file_content_identity",
            "let replay = replay_open_file",
            "if replay != self.replay_state.replay()",
            "checked_transition_frame_digest(&frame)? != transition.receipt.frame_digest",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "install_lane_reservation_journal",
        "required_tokens": (
            "consume_snapshot_replay_seal",
            "apply_durable_fifo_order_reconciliation_locked",
            "remove_hashes_from_fifo_locked",
            "lane_reservation_snapshot_replay_receipt",
        ),
        "ordered_tokens": (
            "let replay_receipt = journal.consume_snapshot_replay_seal(replay_seal)?",
            "self.apply_durable_fifo_order_reconciliation_locked(fifo_plan)",
            "self.remove_hashes_from_fifo_locked(&hashes)",
            "*store = candidate_store",
            "*self.lane_reservation_snapshot_replay_receipt.lock() = Some(replay_receipt)",
            "*journal_guard = Some(journal)",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "bind_lane_reservation_startup_reconciliation_receipt",
        "required_tokens": (
            "durable_owner_count",
            "binds_reconciliation_snapshot",
            "queue_plan_startup_replay_receipt",
            "revalidate_queue_plan_startup_replay_receipt",
            "LaneReservationStartupReconciliationReceipt",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "revalidate_queue_plan_startup_replay_receipt",
        "required_tokens": (
            "queue_plan_startup_live_claim_identities_locked",
            "binds_live_claims",
            "binds_reservation_phases",
            "revalidate_startup_replay_receipt",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "select_lane_reservation_snapshot_lifecycle_projection",
        "required_tokens": (
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "lane_reservation_recovery_phase_map",
            "cursor.before_projection",
            "cursor.after_projection",
            "select_unique_lane_reservation_snapshot_recovered_state",
            "LaneReservationSnapshotLifecycleProjectionV1::from_authenticated_cursor",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "authorize_lane_reservation_snapshot_recovery",
        "required_tokens": (
            "LaneReservationSnapshotLifecycleProjectionV1",
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "lane_reservation_recovery_phase_map",
            "lane_reservation_snapshot_group_phase_agrees",
            "check_production_in_flight_first_release_recover_reservation_snapshot_transition",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "accepted.before != accepted.after",
            "checked_by_group",
            "LaneReservationSnapshotRecoveryAuthorization",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "impl": "Kura",
        "symbol": "publish_autonomous_lifecycle_bootstrap_cursor_stage",
        "required_tokens": (
            "authority: &AutonomousLifecycleBootstrapRecoveryAuthority",
            "target: AutonomousLifecycleBootstrapRecoveryStage",
            "authority.store_root != self.store_root",
            "validate_autonomous_lifecycle_process_generation_claim",
            "validate_signed_bootstrap_payload_persistence_locked",
            "AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable",
            "&authority.bootstrap.body.prepared_activate",
            "AutonomousLifecycleBootstrapRecoveryStage::LiveDurable",
            "&authority.bootstrap.body.live_activate",
            "write_atomic_synced_replace",
            "write_atomic_synced_noclobber",
            "decode_autonomous_lifecycle_cursor",
            "*next",
        ),
        "ordered_tokens": (
            "let current_stage = self.validate_signed_bootstrap_payload_persistence_locked",
            "let (next, replacing_existing) = match (target, current_stage)",
            "AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable, ) => (&authority.bootstrap.body.prepared_activate, false)",
            "AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable, ) => (&authority.bootstrap.body.live_activate, true)",
            "let cursor_path = Self::autonomous_lifecycle_cursor_path_for_entry",
            "self.write_atomic_synced_replace(&cursor_path, &next_bytes)",
            "let readback = self.read_regular_sidecar_bytes",
            "if Self::decode_autonomous_lifecycle_cursor(&cursor_path, &readback)? != *next",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs",
        "impl": "AutonomousLifecycleBootstrapRecoveryAuthority",
        "symbol": "live_cursor",
        "required_tokens": (
            "AutonomousLifecycleCursorV2",
            "self.bootstrap.body.live_activate",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "LaneReservationSnapshotRecoveryAuthorization",
        "symbol": "authorize_recovered_producer_queue_lifecycle_bootstrap",
        "required_tokens": (
            "AutonomousLifecycleAttemptBindingV1",
            "accepted.before != accepted.after",
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "revalidate_complete_live_pre_kura_group_locked",
            "begin_durability_transition_locked",
            "AutonomousLaneKuraActivationAuthorization",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "impl": "Kura",
        "symbol": "revalidate_autonomous_lifecycle_bootstrap_for_completion",
        "required_tokens": (
            "authority.store_root != self.store_root",
            "validate_autonomous_lifecycle_process_generation_claim",
            "prune_lock.lock",
            "ensure_prune_recovery_not_required",
            "canonical_chain_lock.lock",
            "lane_block_application_receipt_available_under_prune_and_canonical_guards",
            "lane_geometry_lock.lock",
            "autonomous_lane_attempt_inventory_counts_locked",
            "read_regular_sidecar_bytes",
            "bytes != authority.expected_bytes",
            "Hash::new(&bytes) != authority.expected_bytes_hash",
            "decode_autonomous_lifecycle_bootstrap",
            "bootstrap != authority.bootstrap",
            "autonomous_lifecycle_bootstrap_authority_locked",
            "receipt_terminal: already_terminal",
        ),
        "ordered_tokens": (
            "if authority.store_root != self.store_root",
            "self.validate_autonomous_lifecycle_process_generation_claim(&authority.process_generation)",
            "let _prune_guard = self.prune_lock.lock()",
            "let _canonical_chain_guard = self.canonical_chain_lock.lock()",
            "let already_terminal = self.lane_block_application_receipt_available_under_prune_and_canonical_guards(proposal)",
            "let _geometry_guard = self.lane_geometry_lock.lock()",
            "let _namespace_budget = self.autonomous_lane_attempt_inventory_counts_locked",
            "let bytes = self.read_regular_sidecar_bytes",
            "if bytes != authority.expected_bytes || Hash::new(&bytes) != authority.expected_bytes_hash",
            "let bootstrap = Self::decode_autonomous_lifecycle_bootstrap(&authority.path, &bytes)",
            "if bootstrap != authority.bootstrap",
            "let authority = self.autonomous_lifecycle_bootstrap_authority_locked",
            "receipt_terminal: already_terminal",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "authenticate_autonomous_lifecycle_bootstrap_recovery",
        "required_tokens": (
            "let expected_stage = authority.stage",
            "revalidate_autonomous_lifecycle_bootstrap_for_completion",
            "authority.stage != expected_stage",
            "authorization.facts",
            "validate_autonomous_lifecycle_bootstrap_producer_queue_authentication_facts",
            "AutonomousLifecycleBootstrapCompletionFence::ProducerQueue",
        ),
        "ordered_tokens": (
            "let expected_stage = authority.stage",
            "revalidate_autonomous_lifecycle_bootstrap_for_completion(authority)",
            "if authority.stage != expected_stage",
            "authorization.facts()",
            "validate_autonomous_lifecycle_bootstrap_producer_queue_authentication_facts(",
            "AutonomousLifecycleBootstrapCompletionFence::ProducerQueue(authorization)",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody",
        "required_tokens": (
            "let expected_stage = authority.stage",
            "revalidate_autonomous_lifecycle_bootstrap_for_completion",
            "authority.stage != expected_stage",
            "AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue",
            "AutonomousLifecycleBootstrapCompletionFence::DurablePayloadCustody",
        ),
        "ordered_tokens": (
            "let expected_stage = authority.stage",
            "revalidate_autonomous_lifecycle_bootstrap_for_completion(authority)",
            "if authority.stage != expected_stage",
            "AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue",
            "AutonomousLifecycleBootstrapCompletionFence::DurablePayloadCustody",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "consume_autonomous_lifecycle_bootstrap_completion_fence",
        "required_tokens": (
            "match fence {",
            "AutonomousLifecycleBootstrapCompletionFence::ProducerQueue(authorization)",
            "drop(authorization);",
            "AutonomousLifecycleBootstrapCompletionFence::DurablePayloadCustody => {}",
        ),
        "ordered_tokens": (
            "match fence {",
            "AutonomousLifecycleBootstrapCompletionFence::ProducerQueue(authorization)",
            "drop(authorization);",
            "AutonomousLifecycleBootstrapCompletionFence::DurablePayloadCustody => {}",
        ),
    },
    {
        "path": "crates/iroha_core/src/kura.rs",
        "impl": "Kura",
        "symbol": "complete_autonomous_lifecycle_bootstrap",
        "required_tokens": (
            "let AutonomousLifecycleBootstrapCompletionPermit { authority, fence } = permit",
            "revalidate_autonomous_lifecycle_bootstrap_for_completion",
            "persist_lane_executable_payload_impl",
            "publish_autonomous_lifecycle_bootstrap_cursor_stage",
            "AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable",
            "AutonomousLifecycleBootstrapRecoveryStage::LiveDurable",
            "delete_completed_autonomous_lifecycle_bootstrap",
            "read_autonomous_lifecycle_cursor",
            "cursor_read.cursor() != Some(&authority.bootstrap.body.live_activate)",
            "lane_block_application_receipt_available",
            "consume_autonomous_lifecycle_bootstrap_completion_fence",
            "AutonomousLifecycleBootstrapCompletionOutcome::AlreadyTerminal",
            "AutonomousLifecycleBootstrapCompletionOutcome::Completed",
        ),
        "ordered_tokens": (
            "let AutonomousLifecycleBootstrapCompletionPermit { authority, fence } = permit; let revalidation = self.revalidate_autonomous_lifecycle_bootstrap_for_completion(authority)",
            "let payload_outcome = self.persist_lane_executable_payload_impl",
            "let prepared_outcome = self.publish_autonomous_lifecycle_bootstrap_cursor_stage",
            "if !matches!(authority.stage, AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable | AutonomousLifecycleBootstrapRecoveryStage::LiveDurable)",
            "let live_outcome = self.publish_autonomous_lifecycle_bootstrap_cursor_stage",
            "if authority.stage != AutonomousLifecycleBootstrapRecoveryStage::LiveDurable",
            "delete_completed_autonomous_lifecycle_bootstrap",
            "let cursor_read = self.read_autonomous_lifecycle_cursor",
            "if cursor_read.cursor() != Some(&authority.bootstrap.body.live_activate)",
            "self.lane_block_application_receipt_available(&payload.origin_proposal)",
            "Self::consume_autonomous_lifecycle_bootstrap_completion_fence(fence)",
            "AutonomousLifecycleBootstrapCompletionOutcome::AlreadyTerminal",
            "AutonomousLifecycleBootstrapCompletionOutcome::Completed",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "LaneReservationSnapshotRecoveryAuthorization",
        "symbol": "into_reconciliation_receipt",
        "required_tokens": (
            "checked_group.checked.into_projection",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "accepted.actor != 0",
            "accepted.target != 0",
            "accepted.before != accepted.after",
            "checked_group.lifecycle.recovered_state",
            "for checked_group in self.checked_planner_groups",
            "checked_group.recovered_state",
            "self.reconciliation_receipt",
        ),
        "ordered_tokens": (
            "for checked_group in self.checked_groups { let accepted = checked_group.checked.into_projection(); if accepted.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "accepted.before != checked_group.lifecycle.recovered_state",
            "for checked_group in self.checked_planner_groups { let accepted = checked_group.checked.into_projection(); if accepted.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "accepted.before != checked_group.recovered_state",
            "Ok(self.reconciliation_receipt)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "impl": None,
        "symbol": "sign_lifecycle_cursor",
        "required_tokens": (
            "AutonomousLifecycleCursorUnsignedV2::new",
            "previous_cursor_hash",
            "signing_preimage",
            "Signature::try_new",
            "<[u8; 96]>::try_from",
            "unsigned.finalize(signature, validator_set)",
        ),
        "ordered_tokens": (
            "AutonomousLifecycleCursorUnsignedV2::new",
            "signing_preimage()",
            "Signature::try_new",
            "<[u8; 96]>::try_from",
            "unsigned.finalize(signature, validator_set)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "impl": None,
        "symbol": "compare_and_swap_phase",
        "required_tokens": (
            "read.into_parts",
            "checked_add(1)",
            "previous_cursor_hash",
            "sign_lifecycle_cursor",
            "compare_and_swap_autonomous_lifecycle_cursor",
        ),
        "ordered_tokens": (
            "let (current, lease) = read.into_parts()",
            "let sequence = current",
            "let previous_cursor_hash = current.as_ref().map",
            "let binding = current",
            "let next = sign_lifecycle_cursor(",
            "kura.compare_and_swap_autonomous_lifecycle_cursor(lease, next)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "impl": None,
        "symbol": "recover_one_attempt",
        "required_tokens": (
            "read_autonomous_lifecycle_cursor",
            "check_production_in_flight_first_release_crash_transition",
            "check_production_in_flight_first_release_recover_transition",
            "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition",
            "AutonomousLifecycleCursorPhaseV2::crashed",
            "AutonomousLifecycleCursorPhaseV2::prepared",
            "AutonomousLifecycleCursorPhaseV2::live",
            "compare_and_swap_phase",
            "for _ in 0..8",
        ),
        "ordered_tokens": (
            "read_autonomous_lifecycle_cursor(payload, binding, process_generation)",
            "check_production_in_flight_first_release_crash_transition(",
            "AutonomousLifecycleCursorPhaseV2::crashed(",
            "check_production_in_flight_first_release_recover_transition(",
            "AutonomousLifecycleCursorPhaseV2::prepared(current_generation, recover)",
            "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(",
            "AutonomousLifecycleCursorPhaseV2::prepared(current_generation, rehydrate)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "impl": None,
        "symbol": "reconcile_autonomous_lifecycle_startup",
        "required_tokens": (
            "lane_reservation_reconciliation_snapshot",
            "bind_lane_reservation_startup_reconciliation_receipt",
            "authorize_lane_reservation_snapshot_recovery",
            "observer_retirement_lifecycle_projections",
            "authorize_recovered_producer_queue_lifecycle_bootstrap",
            "authenticate_autonomous_lifecycle_bootstrap_recovery",
            "complete_autonomous_lifecycle_bootstrap",
            "completion.cursor() != &expected_live",
            "into_reconciliation_receipt",
            "recover_one_attempt",
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "RecoveredAutonomousLifecycleStartup",
        ),
        "ordered_tokens": (
            "lane_reservation_reconciliation_snapshot()",
            "bind_lane_reservation_startup_reconciliation_receipt(&snapshot)",
            "let Some(process_generation) = process_generation else",
            "let recovery = queue.authorize_lane_reservation_snapshot_recovery(",
            "let receipt = recovery.into_reconciliation_receipt()",
            "let mut recovery_authorization = if snapshot.is_empty()",
            "Some(queue.authorize_lane_reservation_snapshot_recovery(",
            "authorize_recovered_producer_queue_lifecycle_bootstrap(",
            "authenticate_autonomous_lifecycle_bootstrap_recovery(",
            "complete_autonomous_lifecycle_bootstrap(permit)",
            "completion.cursor() != &expected_live",
            "let receipt = if let Some(recovery) = recovery_authorization { recovery.into_reconciliation_receipt()",
            "let mut recovered_attempts = 0_usize",
            "if recover_one_attempt(",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
            "Ok(RecoveredAutonomousLifecycleStartup { snapshot, receipt, deferred_terminal_recovery, completed_bootstraps, recovered_attempts, })",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "impl": "RecoveredAutonomousLifecycleStartup",
        "symbol": "into_queue_handoff",
        "required_tokens": (
            "LaneQueueReservationReconciliationSnapshotV1",
            "LaneReservationStartupReconciliationReceipt",
            "AutonomousLifecycleDeferredTerminalRecoveryHandoff",
            "self.snapshot",
            "self.receipt",
            "self.deferred_terminal_recovery",
        ),
        "ordered_tokens": (
            "LaneQueueReservationReconciliationSnapshotV1",
            "LaneReservationStartupReconciliationReceipt",
            "AutonomousLifecycleDeferredTerminalRecoveryHandoff",
            "(self.snapshot, self.receipt, self.deferred_terminal_recovery)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": None,
        "symbol": "plan_lane_reservation_ownership",
        "required_tokens": (
            "lifecycle_handoff: Option<RecoveredAutonomousLifecycleStartup>",
            "handoff.into_queue_handoff",
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "bind_lane_reservation_startup_reconciliation_receipt",
            "LaneReservationReconciliationPlan",
            "recovered_receipt",
            "deferred_terminal_recovery",
            "replay_receipt",
        ),
        "ordered_tokens": (
            "let current_snapshot = queue.lane_reservation_reconciliation_snapshot()",
            "let (snapshot, recovered_receipt, deferred_terminal_recovery) = match lifecycle_handoff",
            "Some(handoff) =>",
            "let (snapshot, receipt, deferred_terminal_recovery) = handoff.into_queue_handoff()",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "(snapshot, Some(receipt), deferred_terminal_recovery)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "impl": None,
        "symbol": "apply_lane_reservation_reconciliation_plan",
        "required_tokens": (
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "finalize_startup_committed_canonical_carriers",
            "release_strictly_absent_lane_reservations_in_order",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions",
            "complete_lane_reservation_startup_reconciliation",
        ),
        "ordered_tokens": (
            "revalidate_lane_reservation_startup_reconciliation_receipt",
            "finalize_startup_committed_canonical_carriers(",
            "release_strictly_absent_lane_reservations_in_order",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
            "complete_lane_reservation_startup_reconciliation(replay_receipt)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "impl": None,
        "symbol": "run_non_pending_lifecycle_loop",
        "required_tokens": (
            "reservation_reconciliation_pending",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning",
            "plan_lane_reservation_ownership",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan)",
            "pre_lifecycle_plan.startup_snapshot_recovery_evidence",
            "planner_evidence",
            "reconcile_autonomous_lifecycle_startup",
            "deferred_terminal_recovery",
            "Some(lifecycle)",
            "apply_lane_reservation_reconciliation_plan",
            "construct_after_pending_tip_application_recovery",
            "install_lane_drain_queue",
            "activate_after_lane_drain_queue_install",
        ),
        "ordered_tokens": (
            "if reservation_reconciliation_pending { let summary = loop {",
            "let deferred_terminal_recovery = reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(state.as_ref(), queue.as_ref(), kura.as_ref(), &verified_context, None,)?",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) =>",
            "let planner_evidence = pre_lifecycle_plan.startup_snapshot_recovery_evidence()?",
            "let lifecycle = reconcile_autonomous_lifecycle_startup(",
            "planner_evidence, deferred_terminal_recovery,",
            "plan_lane_reservation_ownership(state.as_ref(), queue.as_ref(), kura.as_ref(), &verified_context, Some(lifecycle),)?",
            "apply_lane_reservation_reconciliation_plan(",
            "let mut lane_work = construct_after_pending_tip_application_recovery(",
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))",
            "lane_work.activate_after_lane_drain_queue_install(&queue)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "impl": None,
        "symbol": "reconcile_pending_lane_startup",
        "required_tokens": (
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning",
            "plan_lane_reservation_ownership",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan)",
            "pre_lifecycle_plan.startup_snapshot_recovery_evidence",
            "reconcile_autonomous_lifecycle_startup",
            "deferred_terminal_recovery",
            "Some(lifecycle)",
            "apply_lane_reservation_reconciliation_plan",
        ),
        "ordered_tokens": (
            "let deferred_terminal_recovery = reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) =>",
            "let planner_evidence = pre_lifecycle_plan.startup_snapshot_recovery_evidence()?",
            "let lifecycle = reconcile_autonomous_lifecycle_startup(",
            "deferred_terminal_recovery,",
            "plan_lane_reservation_ownership(state.as_ref(), queue.as_ref(), kura.as_ref(), verified_context, Some(lifecycle),)?",
            "apply_lane_reservation_reconciliation_plan(",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "impl": None,
        "symbol": "run_pending_kura_lifecycle_height",
        "required_tokens": (
            "reconcile_pending_lane_startup",
            "pending.prepare_lane_recovery",
            "V2LaneWorkAdapter::new_with_output_guard_and_transport",
        ),
        "ordered_tokens": (
            "let (pending, control) = reconcile_pending_lane_startup(",
            "let mut prepared = pending.prepare_lane_recovery(",
            "V2LaneWorkAdapter::new_with_output_guard_and_transport(",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "impl": "PendingKuraProductionLifecycleV1",
        "symbol": "prepare_lane_recovery",
        "required_tokens": (
            "lane_work.install_lane_drain_queue",
            "lane_work.activate_after_lane_drain_queue_install",
            "matches_lifecycle_lane_work",
        ),
        "ordered_tokens": (
            "if !services.matches_lifecycle_lane_work(&lane_work)",
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))?",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?",
            "Ok(lane_work)",
        ),
    },
    {
        "path": "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "impl": "V2LaneWorkAdapter",
        "symbol": "activate_after_lane_drain_queue_install",
        "required_tokens": (
            "startup_activation_complete",
            "lane_drain_queue",
            "Arc::ptr_eq",
            "begin_fail_stop_operation",
            "hydrate_canonical_lane_artifacts",
            "revalidate_hydrated_autonomous_queue_owners",
            "drive_lane_sessions",
            "activation.complete",
        ),
        "ordered_tokens": (
            "if self.startup_activation_complete",
            "self.lane_drain_queue.as_ref()",
            "Arc::ptr_eq(installed_queue, queue)",
            "begin_fail_stop_operation()",
            "self.hydrate_canonical_lane_artifacts()",
            "self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())",
            "self.startup_activation_complete = true",
            "self.drive_lane_sessions()",
            "activation.complete()",
        ),
    },
    {
        "path": "crates/iroha_core/src/queue.rs",
        "impl": "Queue",
        "symbol": "complete_lane_reservation_startup_reconciliation",
        "required_tokens": (
            "receipt.replay_receipt",
            "receipt.initial_snapshot",
            "store.commit_barriers",
            "store.release_barriers",
            "store.completed_releases",
            "store.missing_payload_hashes",
            "store.live_by_hash",
        ),
    },
)

def _production_trace_canonical_json_bytes(value: Any) -> bytes:
    """Encode the theorem certificate in its one accepted byte representation."""

    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def _production_trace_path_and_ancestor_snapshot(
    path: Path, *, label: str
) -> tuple[Path, tuple[tuple[Path, int, int], ...]]:
    """Resolve no links while pinning every directory leading to ``path``."""

    absolute = Path(os.path.abspath(path))
    if absolute.name in {"", ".", ".."}:
        raise ValueError(f"{label} path has no safe final component: {path}")
    parent = absolute.parent
    try:
        resolved_parent = parent.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"{label} parent is unavailable: {parent}: {error}") from error
    if resolved_parent != parent:
        raise ValueError(
            f"{label} parent path contains a symlink component: {parent}"
        )
    snapshot: list[tuple[Path, int, int]] = []
    for ancestor in reversed((parent, *parent.parents)):
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ValueError(
                f"{label} ancestor is unavailable: {ancestor}: {error}"
            ) from error
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise ValueError(
                f"{label} ancestor must be a real directory: {ancestor}"
            )
        snapshot.append((ancestor, metadata.st_dev, metadata.st_ino))
    return absolute, tuple(snapshot)


def _production_trace_revalidate_ancestors(
    snapshot: tuple[tuple[Path, int, int], ...], *, label: str
) -> None:
    """Reject directory replacement or link insertion during evidence access."""

    for ancestor, expected_device, expected_inode in snapshot:
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise ValueError(
                f"{label} ancestor disappeared: {ancestor}: {error}"
            ) from error
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or (metadata.st_dev, metadata.st_ino)
            != (expected_device, expected_inode)
        ):
            raise ValueError(f"{label} ancestor changed identity: {ancestor}")


def _bounded_regular_file_bytes(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
    allow_empty: bool = False,
) -> bytes:
    """Read one stable, singly linked file through a link-free path."""

    absolute, ancestors = _production_trace_path_and_ancestor_snapshot(
        path, label=label
    )
    try:
        named_before = absolute.lstat()
    except OSError as error:
        raise ValueError(
            f"{label} is not an available non-symlink file: {path}: {error}"
        ) from error
    if stat.S_ISLNK(named_before.st_mode) or not stat.S_ISREG(named_before.st_mode):
        raise ValueError(f"{label} is not a regular non-symlink file: {path}")
    if named_before.st_nlink != 1:
        raise ValueError(f"{label} must have exactly one hard link: {path}")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(absolute, flags)
    except OSError as error:
        raise ValueError(
            f"{label} is not an available non-symlink file: {path}: {error}"
        ) from error
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or (before.st_dev, before.st_ino)
            != (named_before.st_dev, named_before.st_ino)
            or before.st_mode != named_before.st_mode
            or before.st_uid != named_before.st_uid
            or before.st_nlink != 1
        ):
            raise ValueError(f"{label} changed while it was opened: {path}")
        if (
            (not allow_empty and before.st_size == 0)
            or before.st_size > maximum_bytes
        ):
            qualifier = "non-empty and " if not allow_empty else ""
            raise ValueError(
                f"{label} must be {qualifier}at most {maximum_bytes} bytes: "
                f"{path} has {before.st_size} bytes"
            )
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining > 0:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        if len(payload) > maximum_bytes:
            raise ValueError(f"{label} exceeds {maximum_bytes} bytes: {path}")
        after = os.fstat(descriptor)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_uid",
            "st_nlink",
        )
        if any(
            getattr(before, field) != getattr(after, field)
            for field in stable_fields
        ):
            raise ValueError(f"{label} changed while it was being read: {path}")
    finally:
        os.close(descriptor)
    try:
        named = absolute.lstat()
    except OSError as error:
        raise ValueError(
            f"{label} disappeared after it was read: {path}: {error}"
        ) from error
    if (
        stat.S_ISLNK(named.st_mode)
        or not stat.S_ISREG(named.st_mode)
        or named.st_nlink != 1
    ):
        raise ValueError(
            f"{label} path is no longer a regular non-symlink file: {path}"
        )
    named_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_uid",
        "st_nlink",
    )
    if any(getattr(named, field) != getattr(after, field) for field in named_fields):
        raise ValueError(
            f"{label} path changed identity while it was being read: {path}"
        )
    _production_trace_revalidate_ancestors(ancestors, label=label)
    return payload


def load_production_trace_extraction_evidence(path: Path) -> dict[str, Any]:
    """Load one bounded certificate and reject every non-canonical encoding."""

    payload = _bounded_regular_file_bytes(
        path,
        label="production trace-extraction evidence",
        maximum_bytes=PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES,
    )
    try:
        source = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(
            "production trace-extraction evidence is not UTF-8"
        ) from error
    try:
        document = json.loads(source, object_pairs_hook=_unique_object)
    except (json.JSONDecodeError, DuplicateKeyError) as error:
        raise ValueError(
            f"production trace-extraction evidence is invalid JSON: {error}"
        ) from error
    if not isinstance(document, dict):
        raise ValueError(
            "production trace-extraction evidence must be a JSON object"
        )
    if payload != _production_trace_canonical_json_bytes(document):
        raise ValueError(
            "production trace-extraction evidence is not canonical compact "
            "sorted-key JSON with one LF"
        )
    return document


def write_production_trace_extraction_evidence(
    path: Path, document: dict[str, Any]
) -> None:
    """Atomically publish one bounded canonical theorem certificate."""

    payload = _production_trace_canonical_json_bytes(document)
    if not payload or len(payload) > PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES:
        raise ValueError(
            "production trace-extraction evidence exceeds its canonical "
            f"{PRODUCTION_TRACE_EXTRACTION_EVIDENCE_MAX_BYTES}-byte bound"
        )
    absolute, ancestors = _production_trace_path_and_ancestor_snapshot(
        path, label="production trace-extraction evidence output"
    )
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        directory_descriptor = os.open(absolute.parent, directory_flags)
    except OSError as error:
        raise ValueError(
            "production trace-extraction evidence parent could not be opened safely"
        ) from error
    temporary_name = f".{absolute.name}.{secrets.token_hex(16)}.partial"
    temporary_identity: tuple[int, int] | None = None
    try:
        opened_parent = os.fstat(directory_descriptor)
        _, parent_device, parent_inode = ancestors[-1]
        if (
            not stat.S_ISDIR(opened_parent.st_mode)
            or (opened_parent.st_dev, opened_parent.st_ino)
            != (parent_device, parent_inode)
            or opened_parent.st_uid != os.geteuid()
            or stat.S_IMODE(opened_parent.st_mode) & 0o022
        ):
            raise ValueError(
                "production trace-extraction evidence parent must remain an "
                "owner-owned, non-group-writable real directory"
            )
        try:
            existing = os.stat(
                absolute.name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            existing = None
        if existing is not None:
            if stat.S_ISLNK(existing.st_mode) or not stat.S_ISREG(existing.st_mode):
                raise ValueError(
                    "refusing to replace a non-regular or symlinked production "
                    "trace certificate"
                )
            if existing.st_nlink != 1:
                raise ValueError(
                    "production trace-extraction evidence output must have "
                    "exactly one hard link"
                )
        create_flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(
            temporary_name,
            create_flags,
            0o600,
            dir_fd=directory_descriptor,
        )
        try:
            opened = os.fstat(descriptor)
            temporary_identity = (opened.st_dev, opened.st_ino)
            if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
                raise ValueError(
                    "production trace-extraction evidence stage is not one "
                    "regular file"
                )
            written_bytes = 0
            while written_bytes < len(payload):
                count = os.write(descriptor, payload[written_bytes:])
                if count <= 0:
                    raise ValueError(
                        "production trace-extraction evidence stage write stalled"
                    )
                written_bytes += count
            os.fsync(descriptor)
            written = os.fstat(descriptor)
            if (
                (written.st_dev, written.st_ino) != temporary_identity
                or written.st_nlink != 1
                or written.st_size != len(payload)
            ):
                raise ValueError(
                    "production trace-extraction evidence stage metadata changed"
                )
            os.lseek(descriptor, 0, os.SEEK_SET)
            readback = bytearray()
            while len(readback) < len(payload):
                chunk = os.read(
                    descriptor, min(1024 * 1024, len(payload) - len(readback))
                )
                if not chunk:
                    break
                readback.extend(chunk)
            if bytes(readback) != payload:
                raise ValueError(
                    "production trace-extraction evidence stage failed byte "
                    "verification"
                )
        finally:
            os.close(descriptor)
        try:
            current = os.stat(
                absolute.name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            current = None
        if existing is None and current is not None:
            raise ValueError(
                "production trace-extraction evidence output appeared before "
                "publication"
            )
        if existing is not None and (
            current is None
            or (current.st_dev, current.st_ino, current.st_nlink)
            != (existing.st_dev, existing.st_ino, 1)
        ):
            raise ValueError(
                "production trace-extraction evidence output changed before publication"
            )
        _production_trace_revalidate_ancestors(
            ancestors, label="production trace-extraction evidence output"
        )
        staged = os.stat(
            temporary_name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            temporary_identity is None
            or not stat.S_ISREG(staged.st_mode)
            or (staged.st_dev, staged.st_ino) != temporary_identity
            or staged.st_nlink != 1
            or staged.st_size != len(payload)
        ):
            raise ValueError(
                "production trace-extraction evidence stage changed before "
                "publication"
            )
        os.replace(
            temporary_name,
            absolute.name,
            src_dir_fd=directory_descriptor,
            dst_dir_fd=directory_descriptor,
        )
        os.fsync(directory_descriptor)
        published = os.stat(
            absolute.name,
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            temporary_identity is None
            or not stat.S_ISREG(published.st_mode)
            or (published.st_dev, published.st_ino) != temporary_identity
            or published.st_nlink != 1
            or published.st_size != len(payload)
        ):
            raise ValueError(
                "production trace-extraction evidence publication identity is invalid"
            )
        _production_trace_revalidate_ancestors(
            ancestors, label="production trace-extraction evidence output"
        )
    finally:
        try:
            staged = os.stat(
                temporary_name,
                dir_fd=directory_descriptor,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            staged = None
        if (
            staged is not None
            and temporary_identity is not None
            and (staged.st_dev, staged.st_ino) == temporary_identity
        ):
            os.unlink(temporary_name, dir_fd=directory_descriptor)
        os.close(directory_descriptor)


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _production_trace_artifact_entry(role: str, path: Path) -> dict[str, Any]:
    payload = _bounded_regular_file_bytes(
        path,
        label=f"production trace component {role}",
        maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
    )
    return {
        "role": role,
        "sha256": _sha256_bytes(payload),
        "size_bytes": len(payload),
    }


def _production_trace_rust_item_entry(
    *, path: str, kind: str, symbol: str, item: RustItem
) -> dict[str, Any]:
    return {
        "path": path,
        "kind": kind,
        "symbol": symbol,
        "source_sha256": _sha256_bytes(item.source.encode("utf-8")),
        "token_sha256": _rust_sealed_item_token_sha256(item),
    }


def _production_trace_tla_symbol_entry(
    path: str, symbol: str, body: str
) -> dict[str, Any]:
    tokens = (symbol, *tla_code_tokens(body))
    return {
        "path": path,
        "kind": "tla_operator",
        "symbol": symbol,
        "token_sha256": _sha256_bytes("\0".join(tokens).encode("utf-8")),
    }


def _load_multilane_model_checker() -> Any:
    path = Path(__file__).with_name("check_sumeragi_v2_multilane_models.py")
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"multilane source checker is unavailable: {path}")
    spec = importlib.util.spec_from_file_location(
        "_sumeragi_v2_multilane_models_for_trace_certificate", path
    )
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load multilane source checker: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _production_trace_unique_function(
    *,
    root_dir: Path,
    relative: str,
    symbol: str,
    impl_name: str | None,
    errors: list[str],
) -> RustItem | None:
    path = root_dir / relative
    try:
        payload = _bounded_regular_file_bytes(
            path,
            label=f"production trace source {relative}",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        source = payload.decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))
        return None
    items = rust_items(source, symbol)
    expected_context = (
        None
        if impl_name is None
        else (tuple(rust_code_tokens(f"impl {impl_name}")),)
    )
    matches = [
        item
        for item in items
        if expected_context is None or item.brace_context == expected_context
    ]
    qualified = symbol if impl_name is None else f"{impl_name}::{symbol}"
    if len(matches) != 1:
        errors.append(
            f"production trace-extraction theorem requires exactly one non-macro "
            f"item {relative}!{qualified}; found {len(matches)}"
        )
        return None
    item = matches[0]
    if _rust_item_is_test_only(item):
        errors.append(
            f"production trace-extraction theorem item {relative}!{qualified} "
            "is test-only"
        )
        return None
    gated = [
        attribute
        for attribute in (*item.attributes, *item.ancestor_inner_attributes)
        if re.search(r"(?s)^#\s*!?\s*\[\s*cfg(?:_attr)?\b", attribute)
    ]
    if gated:
        errors.append(
            f"production trace-extraction theorem item {relative}!{qualified} "
            f"is configuration-gated: {gated!r}"
        )
        return None
    return item


def _production_trace_extraction_source_snapshot(
    *, root_dir: Path = ROOT_DIR, formal_dir: Path = FORMAL_DIR
) -> dict[str, Any]:
    """Authenticate and hash the exact cross-language trace-extraction seam."""

    errors: list[str] = []
    root_dir = root_dir.resolve()
    model_relative = "formal/sumeragi_v2/SumeragiV2InFlightFirstRelease.tla"
    bindings_relative = "formal/sumeragi_v2/multilane_source_bindings.json"
    model_path = root_dir / model_relative
    try:
        model_payload = _bounded_regular_file_bytes(
            model_path,
            label="in-flight first-release production model",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        model_source = model_payload.decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))
        model_payload = b""
        model_source = ""

    try:
        binding_ledger = json.loads(
            _bounded_regular_file_bytes(
                root_dir / bindings_relative,
                label="multilane source-binding ledger",
                maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
            ).decode("utf-8")
        )
    except (OSError, UnicodeDecodeError, ValueError) as error:
        errors.append(
            "production trace-extraction theorem cannot read its model-action "
            f"inventory: {error}"
        )
    else:
        layout_contract = (
            binding_ledger.get("inflight_first_release_layout_contract")
            if isinstance(binding_ledger, dict)
            else None
        )
        ledger_actions = (
            layout_contract.get("required_actions")
            if isinstance(layout_contract, dict)
            else None
        )
        if not isinstance(ledger_actions, list) or tuple(ledger_actions) != (
            PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
        ):
            errors.append(
                "production trace-extraction required model actions differ from "
                "the multilane source-binding ledger"
            )

    errors.extend(_production_trace_extraction_action_partition_errors())
    ordered_actions: list[str] = []
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        for action in binding["model_actions"]:
            if action not in ordered_actions:
                ordered_actions.append(action)
    for open_model_symbol in (
        "RecoverReservationSnapshot",
        "RehydrateLocalKuraCustody",
    ):
        if open_model_symbol not in ordered_actions:
            ordered_actions.append(open_model_symbol)
    model_symbols: list[dict[str, Any]] = []
    for symbol in (
        *ordered_actions,
        "Next",
        "ConflictingPayloadBindingMutation",
        "MLExecutionInputBeforeReadyAuthorization",
        "MLLaneCommitBeforeAtomicWsvCarrierApplication",
        "MLExactlyOnceCarrierApplication",
        "MLPostCarrierCommitCleanupOrder",
    ):
        extracted = _top_level_operator_body(model_source, symbol)
        if extracted is None:
            errors.append(
                f"production trace-extraction theorem model lacks operator {symbol}"
            )
            continue
        model_symbols.append(
            _production_trace_tla_symbol_entry(model_relative, symbol, extracted[0])
        )
        if symbol == "ConflictingPayloadBindingMutation":
            mutation_tokens = tla_code_tokens(extracted[0])
            for required in (
                'Mode = "PayloadBindingConflict"',
                "payloadBinding'",
                "BindingB",
            ):
                if _token_sequence_count(
                    mutation_tokens,
                    tla_code_tokens(required),
                ) != 1:
                    errors.append(
                        "production operational-correspondence requires the "
                        "unmapped payload-binding mutation to remain explicitly "
                        f"test-mode-only: missing {required!r}"
                    )

    core_relative = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    core_items: list[dict[str, Any]] = []
    for symbol in (
        "check_production_in_flight_first_release_transition",
        "check_production_in_flight_first_release_rehydrate_local_kura_custody_transition",
        "production_in_flight_first_release_transition_kernel",
        "production_in_flight_first_release_witness_binding_kernel",
        "production_in_flight_first_release_terminal_owner",
    ):
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=core_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is not None:
            core_items.append(
                _production_trace_rust_item_entry(
                    path=core_relative, kind="fn", symbol=symbol, item=item
                )
            )
    _core_path, core_source = _read_reviewed_rust_source(
        root_dir,
        core_relative,
        errors,
        "production first-release refinement kernel",
    )
    if len(core_source.encode("utf-8")) > PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES:
        errors.append(
            "production first-release refinement kernel exceeds the bounded "
            "trace-extraction component limit"
        )
        core_source = ""
    transition_macros = rust_macro_items(
        core_source, "production_in_flight_first_release_transition_body"
    )
    if len(transition_macros) != 1:
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "production_in_flight_first_release_transition_body macro; "
            f"found {len(transition_macros)}"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="macro",
                symbol="production_in_flight_first_release_transition_body",
                item=transition_macros[0],
            )
        )

    witness_structs = rust_struct_items(
        core_source,
        "ProductionInFlightFirstReleaseTransitionWitnessV1",
    )
    if len(witness_structs) != 1 or _rust_item_is_test_only(witness_structs[0]):
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "non-test ProductionInFlightFirstReleaseTransitionWitnessV1 struct"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="struct",
                symbol="ProductionInFlightFirstReleaseTransitionWitnessV1",
                item=witness_structs[0],
            )
        )
    witness_binding_macros = rust_macro_items(
        core_source,
        "production_in_flight_first_release_witness_binding_body",
    )
    if len(witness_binding_macros) != 1:
        errors.append(
            "production trace-extraction theorem requires exactly one "
            "production_in_flight_first_release_witness_binding_body macro"
        )
    else:
        core_items.append(
            _production_trace_rust_item_entry(
                path=core_relative,
                kind="macro",
                symbol="production_in_flight_first_release_witness_binding_body",
                item=witness_binding_macros[0],
            )
        )

    action_mappings = PRODUCTION_TRACE_EXTRACTION_ACTION_WITNESS_MAPPINGS
    mapped_model_actions = tuple(mapping[0] for mapping in action_mappings)
    mapped_action_tags = tuple(mapping[1] for mapping in action_mappings)
    mapped_discriminants = tuple(mapping[2] for mapping in action_mappings)
    if mapped_model_actions != PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS:
        errors.append(
            "production operational-correspondence mapping has an unmapped, "
            "unexpected, or reordered model action"
        )
    if len(set(mapped_model_actions)) != len(mapped_model_actions):
        errors.append(
            "production operational-correspondence mapping contains a duplicate model action"
        )
    if len(set(mapped_action_tags)) != len(mapped_action_tags):
        errors.append(
            "production operational-correspondence mapping contains a duplicate Rust action tag"
        )
    if set(mapped_discriminants) != set(range(1, 28)) or len(
        set(mapped_discriminants)
    ) != len(mapped_discriminants):
        errors.append(
            "production operational-correspondence mapping must use each V1 "
            "discriminant from 1 through 27 exactly once"
        )

    core_statements = rust_top_level_statements(core_source)
    action_mapping_entries: list[dict[str, Any]] = []
    transition_macro_tokens = (
        () if len(transition_macros) != 1 else rust_code_tokens(transition_macros[0].source)
    )
    for model_action, action_tag, discriminant in action_mappings:
        expected_statement = rust_code_tokens(
            f"pub(crate) const {action_tag}: u8 = {discriminant};"
        )
        matching_statements = [
            statement
            for statement in core_statements
            if statement.tokens == expected_statement
        ]
        kernel_occurrences = _token_sequence_count(
            transition_macro_tokens,
            rust_code_tokens(f"refinement_tag_value!({action_tag})"),
        )
        if len(matching_statements) != 1:
            errors.append(
                "production operational-correspondence action tag definition "
                f"is missing or ambiguous for {model_action}: {action_tag}={discriminant}"
            )
            continue
        if kernel_occurrences != 1:
            errors.append(
                "production operational-correspondence action must have exactly "
                f"one shared-kernel arm for {model_action}; found {kernel_occurrences}"
            )
            continue
        statement = matching_statements[0]
        action_mapping_entries.append(
            {
                "model_action": model_action,
                "rust_action_tag": action_tag,
                "discriminant": discriminant,
                "tag_source_sha256": _sha256_bytes(
                    statement.source.encode("utf-8")
                ),
                "tag_token_sha256": _rust_statement_token_sha256(statement),
                "shared_kernel_occurrences": kernel_occurrences,
            }
        )

    verus_relative = (
        "crates/iroha_sumeragi_core/src/verus_proofs/"
        "in_flight_first_release_proofs.rs"
    )
    verus_items: list[dict[str, Any]] = []
    for symbol in (
        "production_in_flight_first_release_transition_refines_named_next",
        "production_in_flight_first_release_witness_refines_named_next",
        "production_in_flight_reservation_snapshot_replay_refines_composed_stutter",
        "production_in_flight_first_release_snapshot_recovery_is_stutter",
        "production_in_flight_first_release_local_kura_rehydration_is_exact",
        "production_in_flight_first_release_local_kura_rehydration_rejects_missing_payload",
        "production_in_flight_first_release_local_kura_rehydration_rejects_volatile_drift",
        "production_in_flight_first_release_local_kura_rehydration_rejects_terminal_state",
        "production_in_flight_first_release_terminal_owner_is_exclusive",
    ):
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=verus_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is None:
            continue
        verus_items.append(
            _production_trace_rust_item_entry(
                path=verus_relative, kind="verus_proof_fn", symbol=symbol, item=item
            )
        )
    if verus_items:
        first_theorem = _production_trace_unique_function(
            root_dir=root_dir,
            relative=verus_relative,
            symbol="production_in_flight_first_release_transition_refines_named_next",
            impl_name=None,
            errors=[],
        )
        if first_theorem is not None:
            required = rust_code_tokens(
                "check_production_in_flight_first_release_transition(projection) "
                "== Some(projection) ==> "
                "production_in_flight_first_release_transition_kernel(projection)"
            )
            if (
                _token_sequence_count(
                    rust_code_tokens(first_theorem.source), required
                )
                != 1
            ):
                errors.append(
                    "Verus production_in_flight_first_release_transition_"
                    "refines_named_next "
                    "does not retain its exact checked-transition implication"
                )

    witness_theorem = _production_trace_unique_function(
        root_dir=root_dir,
        relative=verus_relative,
        symbol="production_in_flight_first_release_witness_refines_named_next",
        impl_name=None,
        errors=[],
    )
    if witness_theorem is not None:
        witness_theorem_tokens = rust_code_tokens(witness_theorem.source)
        for required in (
            "production_in_flight_first_release_transition_kernel(projection)",
            "production_in_flight_first_release_witness_binding_kernel(projection, witness)",
            "witness.action == projection.action",
            "witness.actor == projection.actor",
            "witness.target == projection.target",
        ):
            if _token_sequence_count(
                witness_theorem_tokens,
                rust_code_tokens(required),
            ) == 0:
                errors.append(
                    "Verus first-release witness theorem lost required structural "
                    f"binding {required!r}"
                )

    verus_kernel_relative = verus_relative
    verus_witness_kernel = _production_trace_unique_function(
        root_dir=root_dir,
        relative=verus_kernel_relative,
        symbol="production_in_flight_first_release_witness_binding_kernel",
        impl_name=None,
        errors=errors,
    )
    if verus_witness_kernel is not None:
        verus_items.append(
            _production_trace_rust_item_entry(
                path=verus_kernel_relative,
                kind="verus_spec_fn",
                symbol="production_in_flight_first_release_witness_binding_kernel",
                item=verus_witness_kernel,
            )
        )

    shared_identity_relative = "crates/iroha_core/src/queue.rs"
    shared_identity_symbol = (
        "canonical_lane_queue_reservation_group_identity_projection"
    )
    shared_identity_item = _production_trace_unique_function(
        root_dir=root_dir,
        relative=shared_identity_relative,
        symbol=shared_identity_symbol,
        impl_name=None,
        errors=errors,
    )
    shared_identity_entry = None
    if shared_identity_item is not None:
        shared_identity_tokens = rust_code_tokens(shared_identity_item.source)
        for required_token in (
            "reservation_group_hash",
            "IDENTITY_DOMAIN_PAYLOAD",
            "IDENTITY_KIND_CANONICAL_PAYLOAD",
        ):
            if _token_sequence_count(
                shared_identity_tokens, rust_code_tokens(required_token)
            ) != 1:
                errors.append(
                    "production trace-extraction shared carrier identity must "
                    f"contain exactly one {required_token} token"
                )
        shared_identity_entry = _production_trace_rust_item_entry(
            path=shared_identity_relative,
            kind="fn",
            symbol=shared_identity_symbol,
            item=shared_identity_item,
        )

    production_items: list[dict[str, Any]] = []
    operational_relative = "crates/iroha_core/src/sumeragi/v2_core.rs"
    operational_items: list[dict[str, Any]] = []
    operational_source = ""
    try:
        operational_source = _bounded_regular_file_bytes(
            root_dir / operational_relative,
            label="production first-release operational-correspondence wrapper",
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        ).decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))

    operational_required_tokens = {
        "canonical_first_release_state_bytes_v1": (
            "state.validator_count",
            "state.producer",
            "state.producer_selected_owner",
            "state.replicated_carrier_owners",
            "state.payload_binding_a",
            "state.binding_a",
            "state.queue.plan_state",
            "state.queue.selected_count",
            "state.queue.reservation_state",
            "state.carrier.kura_active",
            "state.carrier.execution_input_durable",
            "state.carrier.ready_qc_durable",
            "state.session.bodies",
            "state.session.ready_authorized",
            "state.session.crashed",
            "state.session.producer_alive",
            "state.history.ever_queue_plan_v4",
            "state.history.ever_reservation_v5",
            "state.history.ever_execution_input_durable",
            "state.history.ever_ready_authorized",
            "state.history.ready_signed",
            "state.history.ever_ready_qc_durable",
            "state.history.reservation_committed_prefix",
            "state.history.queue_plan_tombstoned_prefix",
            "state.history.reservation_commit_forgotten_prefix",
            "state.history.pending_high_water",
            "state.history.released_high_water",
            "state.decision.lane_commit_scope",
            "state.decision.release_scope",
            "state.decision.lane_commit_owner",
            "state.decision.release_owner",
            "state.decision.wsv_committed",
            "state.decision.application_count",
            "state.decision.applied_by",
            "state.release.kura_retired",
            "state.release.pending_prefix",
            "state.release.released_prefix",
            "state.release.fifo_restored",
        ),
        "production_in_flight_first_release_state_digest_v1": (
            "iroha_crypto::sha256(canonical_first_release_state_bytes_v1(state))",
        ),
        "production_in_flight_first_release_transition_witness_v1": (
            "schema_version: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION",
            "action: projection.action",
            "actor: projection.actor",
            "target: projection.target",
            "before_state_digest: production_in_flight_first_release_state_digest_v1(projection.before)",
            "after_state_digest: production_in_flight_first_release_state_digest_v1(projection.after)",
            "source_identity: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256",
        ),
        "authenticate_production_in_flight_first_release_transition_witness_v1": (
            "refinement::production_in_flight_first_release_transition_kernel(projection)",
            "production_in_flight_first_release_witness_binding_kernel(projection, witness)",
            "witness == production_in_flight_first_release_transition_witness_v1(projection)",
        ),
        "check_production_in_flight_first_release_replay_step_v1": (
            "ProductionInFlightFirstReleaseReplayStepV1::ComposedNext",
            "projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
            "projection.before != projection.after",
            "ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter",
            "projection.before == projection.after",
            "ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter",
            "refinement::check_production_in_flight_first_release_transition(projection)",
            "authenticate_production_in_flight_first_release_transition_witness_v1(projection, witness)",
            "checked.with_first_release_witness(witness)",
        ),
        "check_production_in_flight_first_release_transition": (
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
            "ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter",
            "_ => ProductionInFlightFirstReleaseReplayStepV1::ComposedNext",
            "check_production_in_flight_first_release_replay_step_v1(projection, classification)",
        ),
    }
    for symbol, required_tokens in operational_required_tokens.items():
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=operational_relative,
            symbol=symbol,
            impl_name=None,
            errors=errors,
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing = [
            token
            for token in required_tokens
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0
        ]
        if missing:
            errors.append(
                "production operational-correspondence wrapper is incomplete at "
                f"{symbol}: missing exact code tokens {missing!r}"
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=operational_relative,
            kind="fn",
            symbol=symbol,
            item=item,
        )
        operational_items.append(entry)
        production_items.append(entry)

    replay_enums = rust_enum_items(
        operational_source,
        "ProductionInFlightFirstReleaseReplayStepV1",
    )
    if len(replay_enums) != 1 or _rust_item_is_test_only(replay_enums[0]):
        errors.append(
            "production operational-correspondence requires exactly one non-test "
            "ProductionInFlightFirstReleaseReplayStepV1 enum"
        )
    else:
        replay_enum_tokens = rust_code_tokens(replay_enums[0].source)
        for variant in (
            "ComposedNext",
            "RecoverReservationSnapshotStutter",
            "RepairPostCarrierEvidenceStutter",
        ):
            if _token_sequence_count(replay_enum_tokens, rust_code_tokens(variant)) != 1:
                errors.append(
                    "production trace replay classification must define exactly "
                    f"one {variant} variant"
                )
        entry = _production_trace_rust_item_entry(
            path=operational_relative,
            kind="enum",
            symbol="ProductionInFlightFirstReleaseReplayStepV1",
            item=replay_enums[0],
        )
        operational_items.append(entry)
        production_items.append(entry)

    operational_statements = rust_top_level_statements(operational_source)
    model_source_identity = _sha256_bytes(model_payload)
    identity_words = [
        model_source_identity[offset : offset + 16]
        for offset in range(0, 64, 16)
    ]
    identity_literals = [
        "0x" + "_".join(word[index : index + 4] for index in range(0, 16, 4))
        for word in identity_words
    ]
    expected_operational_statements = {
        "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION": rust_code_tokens(
            "pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION: u16 = 1;"
        ),
        "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256": rust_code_tokens(
            "pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256: "
            "ProductionDigest256Projection = ProductionDigest256Projection { "
            f"word0: {identity_literals[0]}, word1: {identity_literals[1]}, "
            f"word2: {identity_literals[2]}, word3: {identity_literals[3]}, }};"
        ),
    }
    for symbol, expected_tokens in expected_operational_statements.items():
        matches = [
            statement
            for statement in operational_statements
            if statement.tokens == expected_tokens
        ]
        if len(matches) != 1:
            errors.append(
                "production operational-correspondence constant is missing, "
                f"ambiguous, or stale for {symbol}"
            )
            continue
        statement = matches[0]
        entry = {
            "path": operational_relative,
            "kind": "const",
            "symbol": symbol,
            "source_sha256": _sha256_bytes(statement.source.encode("utf-8")),
            "token_sha256": _rust_statement_token_sha256(statement),
        }
        operational_items.append(entry)
        production_items.append(entry)

    if shared_identity_entry is not None:
        production_items.append(shared_identity_entry)
    source_bindings: list[dict[str, Any]] = []
    model_by_symbol = {entry["symbol"]: entry for entry in model_symbols}
    core_by_symbol = {entry["symbol"]: entry for entry in core_items}
    verus_by_symbol = {entry["symbol"]: entry for entry in verus_items}
    operational_by_symbol = {
        entry["symbol"]: entry for entry in operational_items
    }
    operational_correspondence = {
        "id": "first_release_transition_witness_v1",
        "schema_version": 1,
        "model_source_sha256": model_source_identity,
        "action_mappings": action_mapping_entries,
        "canonical_state_encoder": operational_by_symbol.get(
            "canonical_first_release_state_bytes_v1"
        ),
        "state_digest_builder": operational_by_symbol.get(
            "production_in_flight_first_release_state_digest_v1"
        ),
        "witness_builder": operational_by_symbol.get(
            "production_in_flight_first_release_transition_witness_v1"
        ),
        "witness_authenticator": operational_by_symbol.get(
            "authenticate_production_in_flight_first_release_transition_witness_v1"
        ),
        "trace_replay_reducer": operational_by_symbol.get(
            "check_production_in_flight_first_release_replay_step_v1"
        ),
        "production_transition_checker": operational_by_symbol.get(
            "check_production_in_flight_first_release_transition"
        ),
        "replay_classification": operational_by_symbol.get(
            "ProductionInFlightFirstReleaseReplayStepV1"
        ),
        "witness_schema_version": operational_by_symbol.get(
            "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION"
        ),
        "model_source_identity": operational_by_symbol.get(
            "PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256"
        ),
        "shared_transition_kernel": core_by_symbol.get(
            "production_in_flight_first_release_transition_kernel"
        ),
        "shared_witness_binding_kernel": core_by_symbol.get(
            "production_in_flight_first_release_witness_binding_kernel"
        ),
        "verus_witness_binding_kernel": verus_by_symbol.get(
            "production_in_flight_first_release_witness_binding_kernel"
        ),
        "verus_witness_theorem": verus_by_symbol.get(
            "production_in_flight_first_release_witness_refines_named_next"
        ),
        "digest_proof_boundary": "canonical-recomputation-plus-trusted-cryptography-contract",
        "authenticated": True,
    }
    snapshot_recovery_bridge_entries: list[dict[str, Any]] = []
    for binding in PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS:
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=binding["path"],
            symbol=binding["symbol"],
            impl_name=binding["impl"],
            errors=errors,
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing_tokens = [
            token
            for token in binding["required_tokens"]
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0
        ]
        order_error = _production_trace_ordered_token_sequence_error(
            item_tokens,
            binding.get("ordered_tokens", ()),
        )
        qualified = (
            binding["symbol"]
            if binding["impl"] is None
            else f"{binding['impl']}::{binding['symbol']}"
        )
        if missing_tokens or order_error is not None:
            detail = []
            if missing_tokens:
                detail.append(f"missing exact code tokens {missing_tokens!r}")
            if order_error is not None:
                detail.append(order_error)
            errors.append(
                "RecoverReservationSnapshot parametric bridge is incomplete at "
                f"{binding['path']}!{qualified}: " + "; ".join(detail)
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=binding["path"],
            kind="fn" if binding["impl"] is None else "method",
            symbol=qualified,
            item=item,
        )
        snapshot_recovery_bridge_entries.append(entry)
        production_items.append(entry)
    if len(snapshot_recovery_bridge_entries) == len(
        PRODUCTION_SNAPSHOT_RECOVERY_BRIDGE_BINDINGS
    ):
        snapshot_recovery_bridge_by_symbol = {
            entry["symbol"]: entry for entry in snapshot_recovery_bridge_entries
        }
        source_bindings.append(
            {
                "id": "recover_reservation_snapshot_parametric_noninterference",
                "action_tags": [
                    "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
                    "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
                ],
                "model_symbols": [
                    model_by_symbol.get("RecoverReservationSnapshot")
                ],
                "production_symbol": snapshot_recovery_bridge_by_symbol[
                    "IndexedReservationReplayState::check_in_flight_transition"
                ],
                "authorization_source": snapshot_recovery_bridge_by_symbol[
                    "IndexedReservationReplayState::from_replay"
                ],
                "checked_transition_consumer": snapshot_recovery_bridge_by_symbol[
                    "LaneQueueReservationJournal::consume_snapshot_replay_seal"
                ],
                "checked_transition_adapter": snapshot_recovery_bridge_by_symbol[
                    "LaneReservationSnapshotReplayReceipt::binds_reconciliation_snapshot"
                ],
                "canonical_commit_sink": snapshot_recovery_bridge_by_symbol[
                    "Queue::complete_lane_reservation_startup_reconciliation"
                ],
                "carrier_identity_projection": shared_identity_entry,
                "refinement_kernel": core_by_symbol.get(
                    "check_production_in_flight_first_release_transition"
                ),
                "verus_theorem": verus_by_symbol.get(
                    "production_in_flight_reservation_snapshot_replay_refines_composed_stutter"
                ),
                "operational_correspondence_id": operational_correspondence["id"],
                "bridge_symbols": snapshot_recovery_bridge_entries,
                "authenticated": True,
            }
        )
    for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS:
        item = _production_trace_unique_function(
            root_dir=root_dir,
            relative=binding["path"],
            symbol=binding["symbol"],
            impl_name=binding["impl"],
            errors=errors,
        )
        qualified = (
            binding["symbol"]
            if binding["impl"] is None
            else f"{binding['impl']}::{binding['symbol']}"
        )
        if item is None:
            continue
        item_tokens = rust_code_tokens(item.source)
        missing_tokens: list[str] = []
        for token in (*binding["action_tags"], *binding["additional_tokens"]):
            if _token_sequence_count(item_tokens, rust_code_tokens(token)) == 0:
                missing_tokens.append(token)

        checked_transition_source_entry = None
        checked_transition_source = binding.get("checked_transition_source")
        checked_transition_tokens = item_tokens
        if checked_transition_source is not None:
            checked_source_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_source["path"],
                symbol=checked_transition_source["symbol"],
                impl_name=checked_transition_source["impl"],
                errors=errors,
            )
            if checked_source_item is None:
                continue
            checked_transition_tokens = rust_code_tokens(checked_source_item.source)
            missing_checked_source_tokens = [
                token
                for token in checked_transition_source["required_tokens"]
                if _token_sequence_count(
                    checked_transition_tokens, rust_code_tokens(token)
                )
                == 0
            ]
            checked_source_order_error = _production_trace_ordered_token_sequence_error(
                checked_transition_tokens,
                checked_transition_source.get("ordered_tokens", ()),
            )
            if missing_checked_source_tokens or checked_source_order_error is not None:
                detail = []
                if missing_checked_source_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_checked_source_tokens!r}"
                    )
                if checked_source_order_error is not None:
                    detail.append(checked_source_order_error)
                checked_source_qualified = (
                    checked_transition_source["symbol"]
                    if checked_transition_source["impl"] is None
                    else (
                        f"{checked_transition_source['impl']}::"
                        f"{checked_transition_source['symbol']}"
                    )
                )
                errors.append(
                    "production trace-extraction theorem missing exact checked-transition "
                    f"source {binding['id']} at {checked_transition_source['path']}!"
                    f"{checked_source_qualified}: " + "; ".join(detail)
                )
                continue
            checked_source_qualified = (
                checked_transition_source["symbol"]
                if checked_transition_source["impl"] is None
                else (
                    f"{checked_transition_source['impl']}::"
                    f"{checked_transition_source['symbol']}"
                )
            )
            checked_transition_source_entry = _production_trace_rust_item_entry(
                path=checked_transition_source["path"],
                kind="fn" if checked_transition_source["impl"] is None else "method",
                symbol=checked_source_qualified,
                item=checked_source_item,
            )
        checked_count = _token_sequence_count(
            checked_transition_tokens,
            rust_code_tokens("check_production_in_flight_first_release_transition"),
        )
        projection_count = _token_sequence_count(
            checked_transition_tokens,
            rust_code_tokens("ProductionInFlightFirstReleaseTransitionProjection"),
        )
        consumption_count = _token_sequence_count(
            item_tokens, rust_code_tokens("into_projection")
        )
        expected_count = binding["checked_transition_count"]
        expected_projection_count = (
            checked_transition_source.get("transition_projection_count", expected_count)
            if checked_transition_source is not None
            else expected_count
        )
        has_separate_consumer = binding.get("checked_transition_consumer") is not None
        if (
            missing_tokens
            or checked_count != expected_count
            or projection_count != expected_projection_count
            or (not has_separate_consumer and consumption_count < expected_count)
        ):
            detail: list[str] = []
            if missing_tokens:
                detail.append(f"missing exact code tokens {missing_tokens!r}")
            if checked_count != expected_count:
                detail.append(
                    "checked transition calls "
                    f"expected {expected_count}, found {checked_count}"
                )
            if projection_count != expected_projection_count:
                detail.append(
                    "transition projections "
                    f"expected {expected_projection_count}, found {projection_count}"
                )
            if not has_separate_consumer and consumption_count < expected_count:
                detail.append(
                    "move-only checked projection consumptions "
                    f"expected at least {expected_count}, found {consumption_count}"
                )
            errors.append(
                "production trace-extraction theorem missing authenticated binding "
                f"{binding['id']} at {binding['path']}!{qualified}: "
                + "; ".join(detail)
            )
            continue
        entry = _production_trace_rust_item_entry(
            path=binding["path"],
            kind="fn" if binding["impl"] is None else "method",
            symbol=qualified,
            item=item,
        )
        production_items.append(entry)
        if checked_transition_source_entry is not None:
            production_items.append(checked_transition_source_entry)

        supporting_source_entries = []
        supporting_sources_valid = True
        for supporting_source in binding.get("supporting_sources", ()):
            supporting_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=supporting_source["path"],
                symbol=supporting_source["symbol"],
                impl_name=supporting_source["impl"],
                errors=errors,
            )
            if supporting_item is None:
                supporting_sources_valid = False
                break
            supporting_tokens = rust_code_tokens(supporting_item.source)
            missing_supporting_tokens = [
                token
                for token in supporting_source["required_tokens"]
                if _token_sequence_count(supporting_tokens, rust_code_tokens(token)) == 0
            ]
            forbidden_supporting_tokens = [
                token
                for token in supporting_source.get("forbidden_tokens", ())
                if _token_sequence_count(supporting_tokens, rust_code_tokens(token)) != 0
            ]
            supporting_order_error = _production_trace_ordered_token_sequence_error(
                supporting_tokens,
                supporting_source.get("ordered_tokens", ()),
            )
            supporting_qualified = (
                supporting_source["symbol"]
                if supporting_source["impl"] is None
                else f"{supporting_source['impl']}::{supporting_source['symbol']}"
            )
            if (
                missing_supporting_tokens
                or forbidden_supporting_tokens
                or supporting_order_error is not None
            ):
                detail = []
                if missing_supporting_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_supporting_tokens!r}"
                    )
                if forbidden_supporting_tokens:
                    detail.append(
                        "contains forbidden exact code tokens "
                        f"{forbidden_supporting_tokens!r}"
                    )
                if supporting_order_error is not None:
                    detail.append(supporting_order_error)
                errors.append(
                    "production trace-extraction theorem missing "
                    f"{supporting_source['role']} for {binding['id']} at "
                    f"{supporting_source['path']}!{supporting_qualified}: "
                    + "; ".join(detail)
                )
                supporting_sources_valid = False
                break
            supporting_entry = _production_trace_rust_item_entry(
                path=supporting_source["path"],
                kind="fn" if supporting_source["impl"] is None else "method",
                symbol=supporting_qualified,
                item=supporting_item,
            )
            supporting_source_entries.append(supporting_entry)
            production_items.append(supporting_entry)
        if not supporting_sources_valid:
            continue
        authorization_source_entry = None
        authorization_source = binding.get("authorization_source")
        if authorization_source is not None:
            authorization_source_impl = authorization_source.get("impl")
            authorization_source_qualified = (
                authorization_source["symbol"]
                if authorization_source_impl is None
                else f"{authorization_source_impl}::{authorization_source['symbol']}"
            )
            source_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=authorization_source["path"],
                symbol=authorization_source["symbol"],
                impl_name=authorization_source_impl,
                errors=errors,
            )
            if source_item is None:
                continue
            source_tokens = rust_code_tokens(source_item.source)
            missing_source_tokens = [
                token
                for token in authorization_source["required_tokens"]
                if _token_sequence_count(source_tokens, rust_code_tokens(token)) == 0
            ]
            source_order_error = _production_trace_ordered_token_sequence_error(
                source_tokens,
                authorization_source.get("ordered_tokens", ()),
            )
            if missing_source_tokens or source_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_source_tokens!r}"
                    if missing_source_tokens
                    else source_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing canonical authorization "
                    f"source tokens at "
                    f"{authorization_source['path']}!{authorization_source_qualified}: "
                    f"{detail}"
                )
                continue
            authorization_source_entry = _production_trace_rust_item_entry(
                path=authorization_source["path"],
                kind="fn" if authorization_source_impl is None else "method",
                symbol=authorization_source_qualified,
                item=source_item,
            )
            production_items.append(authorization_source_entry)
        checked_transition_consumer_entry = None
        checked_transition_consumer = binding.get("checked_transition_consumer")
        if checked_transition_consumer is not None:
            checked_transition_consumer_impl = checked_transition_consumer.get("impl")
            checked_transition_consumer_qualified = (
                checked_transition_consumer["symbol"]
                if checked_transition_consumer_impl is None
                else (
                    f"{checked_transition_consumer_impl}::"
                    f"{checked_transition_consumer['symbol']}"
                )
            )
            consumer_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_consumer["path"],
                symbol=checked_transition_consumer["symbol"],
                impl_name=checked_transition_consumer_impl,
                errors=errors,
            )
            if consumer_item is None:
                continue
            consumer_tokens = rust_code_tokens(consumer_item.source)
            missing_consumer_tokens = [
                token
                for token in checked_transition_consumer["required_tokens"]
                if _token_sequence_count(consumer_tokens, rust_code_tokens(token)) == 0
            ]
            consumer_count = _token_sequence_count(
                consumer_tokens, rust_code_tokens("into_projection")
            )
            consumer_order_error = _production_trace_ordered_token_sequence_error(
                consumer_tokens,
                checked_transition_consumer.get("ordered_tokens", ()),
            )
            if (
                missing_consumer_tokens
                or consumer_count < expected_count
                or consumer_order_error is not None
            ):
                detail = []
                if missing_consumer_tokens:
                    detail.append(
                        f"missing exact code tokens {missing_consumer_tokens!r}"
                    )
                if consumer_count < expected_count:
                    detail.append(
                        "move-only checked projection consumptions "
                        f"expected at least {expected_count}, found {consumer_count}"
                    )
                if consumer_order_error is not None:
                    detail.append(consumer_order_error)
                errors.append(
                    "production trace-extraction theorem missing move-only consumer "
                    f"{binding['id']} at {checked_transition_consumer['path']}!"
                    f"{checked_transition_consumer_qualified}: "
                    + "; ".join(detail)
                )
                continue
            checked_transition_consumer_entry = _production_trace_rust_item_entry(
                path=checked_transition_consumer["path"],
                kind="fn" if checked_transition_consumer_impl is None else "method",
                symbol=checked_transition_consumer_qualified,
                item=consumer_item,
            )
            production_items.append(checked_transition_consumer_entry)
        checked_transition_adapter_entry = None
        checked_transition_adapter = binding.get("checked_transition_adapter")
        if checked_transition_adapter is not None:
            checked_transition_adapter_impl = checked_transition_adapter.get("impl")
            checked_transition_adapter_qualified = (
                checked_transition_adapter["symbol"]
                if checked_transition_adapter_impl is None
                else (
                    f"{checked_transition_adapter_impl}::"
                    f"{checked_transition_adapter['symbol']}"
                )
            )
            adapter_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=checked_transition_adapter["path"],
                symbol=checked_transition_adapter["symbol"],
                impl_name=checked_transition_adapter_impl,
                errors=errors,
            )
            if adapter_item is None:
                continue
            adapter_tokens = rust_code_tokens(adapter_item.source)
            missing_adapter_tokens = [
                token
                for token in checked_transition_adapter["required_tokens"]
                if _token_sequence_count(adapter_tokens, rust_code_tokens(token)) == 0
            ]
            adapter_order_error = _production_trace_ordered_token_sequence_error(
                adapter_tokens,
                checked_transition_adapter.get("ordered_tokens", ()),
            )
            if missing_adapter_tokens or adapter_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_adapter_tokens!r}"
                    if missing_adapter_tokens
                    else adapter_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing move-only State "
                    f"commit adapter {binding['id']} at "
                    f"{checked_transition_adapter['path']}!"
                    f"{checked_transition_adapter_qualified}: {detail}"
                )
                continue
            checked_transition_adapter_entry = _production_trace_rust_item_entry(
                path=checked_transition_adapter["path"],
                kind="fn" if checked_transition_adapter_impl is None else "method",
                symbol=checked_transition_adapter_qualified,
                item=adapter_item,
            )
            production_items.append(checked_transition_adapter_entry)
        commit_sink_entry = None
        commit_sink = binding.get("commit_sink")
        if commit_sink is not None:
            commit_sink_impl = commit_sink.get("impl")
            commit_sink_symbol = (
                commit_sink["symbol"]
                if commit_sink_impl is None
                else f"{commit_sink_impl}::{commit_sink['symbol']}"
            )
            sink_item = _production_trace_unique_function(
                root_dir=root_dir,
                relative=commit_sink["path"],
                symbol=commit_sink["symbol"],
                impl_name=commit_sink_impl,
                errors=errors,
            )
            if sink_item is None:
                continue
            sink_tokens = rust_code_tokens(sink_item.source)
            missing_sink_tokens = [
                token
                for token in commit_sink["required_tokens"]
                if _token_sequence_count(sink_tokens, rust_code_tokens(token)) == 0
            ]
            sink_order_error = _production_trace_ordered_token_sequence_error(
                sink_tokens,
                commit_sink.get("ordered_tokens", ()),
            )
            if missing_sink_tokens or sink_order_error is not None:
                detail = (
                    f"missing exact code tokens {missing_sink_tokens!r}"
                    if missing_sink_tokens
                    else sink_order_error
                )
                errors.append(
                    "production trace-extraction theorem missing canonical commit "
                    f"sink tokens at "
                    f"{commit_sink['path']}!{commit_sink_symbol}: {detail}"
                )
                continue
            commit_sink_entry = _production_trace_rust_item_entry(
                path=commit_sink["path"],
                kind="fn" if commit_sink_impl is None else "method",
                symbol=commit_sink_symbol,
                item=sink_item,
            )
            production_items.append(commit_sink_entry)
        missing_model_actions = [
            action
            for action in binding["model_actions"]
            if action not in model_by_symbol
        ]
        if missing_model_actions:
            errors.append(
                "production trace-extraction theorem cannot bind missing model "
                f"operators {missing_model_actions!r} for {binding['id']}"
            )
            continue
        source_bindings.append(
            {
                "id": binding["id"],
                "action_tags": list(binding["action_tags"]),
                "model_symbols": [
                    model_by_symbol[action]
                    for action in binding["model_actions"]
                ],
                "production_symbol": entry,
                "checked_transition_source": checked_transition_source_entry,
                "supporting_sources": supporting_source_entries,
                "authorization_source": authorization_source_entry,
                "checked_transition_consumer": checked_transition_consumer_entry,
                "checked_transition_adapter": checked_transition_adapter_entry,
                "canonical_commit_sink": commit_sink_entry,
                "carrier_identity_projection": shared_identity_entry,
                "refinement_kernel": core_by_symbol.get(
                    "check_production_in_flight_first_release_transition"
                ),
                "verus_theorem": verus_by_symbol.get(
                    "production_in_flight_first_release_transition_refines_named_next"
                ),
                "operational_correspondence_id": operational_correspondence["id"],
                "authenticated": True,
            }
        )

    try:
        multilane_checker = _load_multilane_model_checker()
        # The strict formal launcher has already run the complete multilane
        # structural checker. Recompute its exact source manifest here and
        # independently recheck the theorem seams above; replaying the
        # entire unrelated closure inventory would make certificate validation
        # needlessly unbounded.
        multilane_manifest = multilane_checker.source_manifest_sha256(root_dir)
    except (OSError, UnicodeDecodeError, ValueError, RuntimeError) as error:
        errors.append(f"could not authenticate multilane source bindings: {error}")
        multilane_manifest = None

    if errors:
        raise ValueError("\n".join(errors))

    fixed_relative = "formal/sumeragi_v2/inflight_first_release_fixed.cfg"
    checker_relative = "scripts/formal/check_sumeragi_v2_multilane_models.py"
    model_sources = []
    for relative, label in (
        (model_relative, "in-flight TLA+ model"),
        (fixed_relative, "in-flight positive model config"),
        (bindings_relative, "multilane source-binding ledger"),
        (checker_relative, "multilane source-binding checker"),
    ):
        payload = _bounded_regular_file_bytes(
            root_dir / relative,
            label=label,
            maximum_bytes=PRODUCTION_TRACE_EXTRACTION_COMPONENT_MAX_BYTES,
        )
        model_sources.append(
            {
                "path": relative,
                "sha256": _sha256_bytes(payload),
                "size_bytes": len(payload),
            }
        )
    return {
        "multilane_source_manifest_sha256": multilane_manifest,
        "model_sources": model_sources,
        "model_symbols": model_symbols,
        "refinement_symbols": core_items,
        "production_symbols": production_items,
        "verus_theorems": verus_items,
        "operational_correspondence": operational_correspondence,
        "source_bindings": source_bindings,
    }


def _production_trace_ordered_token_sequence_error(
    source_tokens: Sequence[str], required_tokens: Sequence[str]
) -> str | None:
    """Return a fail-closed error unless each token sequence occurs once in order."""

    cursor = -1
    for required in required_tokens:
        needle = rust_code_tokens(required)
        positions = [
            index
            for index in range(len(source_tokens) - len(needle) + 1)
            if tuple(source_tokens[index : index + len(needle)]) == tuple(needle)
        ]
        if len(positions) != 1:
            return (
                f"ordered code token {required!r} must occur exactly once, "
                f"found {len(positions)}"
            )
        if positions[0] <= cursor:
            return f"ordered code token {required!r} moved before its predecessor"
        cursor = positions[0]
    return None


def _production_trace_extraction_action_partition_errors() -> list[str]:
    """Require concrete bindings plus explicit debt to cover the whole model."""

    required = PRODUCTION_TRACE_EXTRACTION_REQUIRED_MODEL_ACTIONS
    open_actions = PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS
    bound_actions = tuple(
        action
        for binding in PRODUCTION_TRACE_EXTRACTION_BINDINGS
        for action in binding["model_actions"]
    )
    required_set = set(required)
    open_set = set(open_actions)
    bound_set = set(bound_actions)
    errors: list[str] = []
    if len(required_set) != len(required):
        errors.append(
            "production trace-extraction required model-action inventory "
            "contains duplicates"
        )
    if len(open_set) != len(open_actions):
        errors.append(
            "production trace-extraction open model-action inventory contains "
            "duplicates"
        )
    overlap = [
        action
        for action in required
        if action in open_set and action in bound_set
    ]
    missing = [action for action in required if action not in open_set | bound_set]
    unexpected = sorted((open_set | bound_set) - required_set)
    if overlap or missing or unexpected:
        errors.append(
            "production trace-extraction bindings and explicit open actions do "
            "not partition the exact model-action inventory: "
            f"overlap={overlap!r}, missing={missing!r}, unexpected={unexpected!r}"
        )
    return errors


def _production_trace_extraction_ledger_dependency_snapshot(
    ledger: dict[str, Any],
) -> list[dict[str, str]]:
    """Return the exact proved/trusted ledger slice used by this theorem."""

    obligations = ledger.get("obligations")
    if not isinstance(obligations, list):
        raise ValueError(
            "production trace-extraction evidence requires a proof obligation array"
        )
    by_id: dict[str, dict[str, Any]] = {}
    for obligation in obligations:
        if not isinstance(obligation, dict):
            continue
        obligation_id = obligation.get("id")
        if not _nonempty_string(obligation_id):
            continue
        if obligation_id in by_id:
            raise ValueError(
                "production trace-extraction ledger dependency inventory contains "
                f"duplicate obligation {obligation_id}"
            )
        by_id[obligation_id] = obligation

    snapshot: list[dict[str, str]] = []
    for obligation_id, expected_status in (
        PRODUCTION_TRACE_EXTRACTION_LEDGER_DEPENDENCIES
    ):
        obligation = by_id.get(obligation_id)
        if obligation is None:
            raise ValueError(
                "production trace-extraction ledger dependency is missing: "
                f"{obligation_id}"
            )
        observed_status = obligation.get("status")
        if observed_status != expected_status:
            raise ValueError(
                "production trace-extraction ledger dependency status drifted: "
                f"{obligation_id} expected {expected_status}, found "
                f"{observed_status!r}"
            )
        snapshot.append({"id": obligation_id, "status": expected_status})
    return snapshot


def build_production_trace_extraction_evidence(
    ledger: dict[str, Any],
    *,
    tlaps_evidence: dict[str, Any],
    verus_evidence: dict[str, Any],
    cross_tool_evidence: dict[str, Any] | None,
    artifacts: ProductionTraceExtractionArtifactPaths,
    root_dir: Path = ROOT_DIR,
    formal_dir: Path = FORMAL_DIR,
) -> dict[str, Any]:
    """Build the exact source- and backend-bound production theorem certificate."""

    ledger_dependencies = _production_trace_extraction_ledger_dependency_snapshot(
        ledger
    )
    partition_errors = _production_trace_extraction_action_partition_errors()
    if partition_errors:
        raise ValueError("\n".join(partition_errors))
    if PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS:
        raise ValueError(
            "production trace-extraction evidence cannot be certified while "
            "model actions remain unextracted: "
            + ", ".join(PRODUCTION_TRACE_EXTRACTION_OPEN_MODEL_ACTIONS)
        )
    if not all(
        isinstance(value, dict) for value in (tlaps_evidence, verus_evidence)
    ):
        raise ValueError(
            "production trace-extraction evidence requires TLAPS and Verus "
            "evidence objects"
        )
    formal_manifest = tlaps_evidence.get("source_manifest")
    if not isinstance(formal_manifest, dict) or not _nonempty_string(
        formal_manifest.get("sha256")
    ):
        raise ValueError("TLAPS evidence lacks its formal source manifest")
    workspace_manifest = verus_evidence.get("source_manifest_sha256")
    if not _nonempty_string(workspace_manifest):
        raise ValueError("Verus evidence lacks its workspace source manifest")
    tlaps_ledger_sha256 = tlaps_evidence.get("ledger_sha256")
    if not _nonempty_string(tlaps_ledger_sha256):
        raise ValueError("TLAPS evidence lacks its exact proof-ledger digest")
    component_evidence = {
        "tlaps_sha256": _canonical_json_sha256(tlaps_evidence),
        "verus_sha256": _canonical_json_sha256(verus_evidence),
    }
    if cross_tool_evidence is not None:
        if not isinstance(cross_tool_evidence, dict):
            raise ValueError("cross-tool evidence must be an object when supplied")
        source_manifests = cross_tool_evidence.get("source_manifests")
        if source_manifests != {
            "formal_sha256": formal_manifest["sha256"],
            "workspace_sha256": workspace_manifest,
        }:
            raise ValueError(
                "cross-tool evidence does not link the exact formal and workspace "
                "manifests"
            )
        if cross_tool_evidence.get("ledger_sha256") != tlaps_ledger_sha256:
            raise ValueError(
                "cross-tool evidence does not link the exact proof ledger"
            )
        if cross_tool_evidence.get("component_evidence") != component_evidence:
            raise ValueError(
                "cross-tool evidence does not link the exact backend evidence"
            )
    source_snapshot = _production_trace_extraction_source_snapshot(
        root_dir=root_dir, formal_dir=formal_dir
    )
    artifact_entries = [
        _production_trace_artifact_entry("proof_ledger", artifacts.ledger),
        _production_trace_artifact_entry("tlaps_evidence", artifacts.evidence),
        _production_trace_artifact_entry("verus_evidence", artifacts.verus_evidence),
        _production_trace_artifact_entry("verus_log", artifacts.verus_log),
    ]
    if artifacts.cross_tool_evidence is not None:
        artifact_entries.append(
            _production_trace_artifact_entry(
                "cross_tool_evidence", artifacts.cross_tool_evidence
            )
        )
    return {
        "schema_version": PRODUCTION_TRACE_EXTRACTION_EVIDENCE_SCHEMA_VERSION,
        "certificate_type": "production_trace_extraction_theorem",
        "theorem": PRODUCTION_TRACE_EXTRACTION_THEOREM,
        "canonical_encoding": PRODUCTION_TRACE_EXTRACTION_CANONICAL_ENCODING,
        "backend_verification": True,
        "workspace_source_manifest_sha256": workspace_manifest,
        "formal_source_manifest_sha256": formal_manifest["sha256"],
        "multilane_source_manifest_sha256": source_snapshot[
            "multilane_source_manifest_sha256"
        ],
        "artifacts": artifact_entries,
        "model_sources": source_snapshot["model_sources"],
        "model_symbols": source_snapshot["model_symbols"],
        "refinement_symbols": source_snapshot["refinement_symbols"],
        "production_symbols": source_snapshot["production_symbols"],
        "verus_theorems": source_snapshot["verus_theorems"],
        "operational_correspondence": source_snapshot[
            "operational_correspondence"
        ],
        "source_bindings": source_snapshot["source_bindings"],
        "proof_linkage": {
            "ledger_document_sha256": _canonical_json_sha256(ledger),
            "tlaps_document_sha256": _canonical_json_sha256(tlaps_evidence),
            "verus_document_sha256": _canonical_json_sha256(verus_evidence),
            "cross_tool_document_sha256": (
                None
                if cross_tool_evidence is None
                else _canonical_json_sha256(cross_tool_evidence)
            ),
            "cross_tool_ledger_sha256": (
                None
                if cross_tool_evidence is None
                else cross_tool_evidence["ledger_sha256"]
            ),
            "component_evidence": component_evidence,
            "verus_log_sha256": verus_evidence.get("log_sha256"),
            "multilane_dependency_completion": True,
            "multilane_ledger_dependencies": ledger_dependencies,
            "global_machine_checked_completion": ledger.get(
                "machine_checked_completion"
            ),
        },
    }


def _production_trace_extraction_evidence_errors(
    ledger: dict[str, Any],
    observed: dict[str, Any] | None,
    *,
    tlaps_evidence: dict[str, Any] | None,
    verus_evidence: dict[str, Any] | None,
    cross_tool_evidence: dict[str, Any] | None,
    artifacts: ProductionTraceExtractionArtifactPaths | None,
    root_dir: Path = ROOT_DIR,
    formal_dir: Path = FORMAL_DIR,
) -> list[str]:
    if observed is None:
        return []
    if not isinstance(observed, dict):
        return ["production trace-extraction evidence must be a JSON object"]
    if artifacts is None:
        return ["production trace-extraction evidence lacks exact artifact paths"]
    if not all(
        isinstance(value, dict) for value in (tlaps_evidence, verus_evidence)
    ):
        return [
            "production trace-extraction evidence requires linked TLAPS and "
            "Verus evidence"
        ]
    try:
        expected = build_production_trace_extraction_evidence(
            ledger,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            cross_tool_evidence=cross_tool_evidence,
            artifacts=artifacts,
            root_dir=root_dir,
            formal_dir=formal_dir,
        )
    except (OSError, UnicodeDecodeError, ValueError) as error:
        return [f"production trace-extraction theorem cannot be authenticated: {error}"]
    mismatch = _first_json_mismatch(expected, observed)
    if mismatch is not None:
        return [
            "production trace-extraction evidence does not match the canonical "
            f"current theorem certificate at {mismatch}"
        ]
    return []
